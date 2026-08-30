//! Linux ioctl wrappers for hidraw, usbfs, uinput, and evdev.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use std::io;
use std::os::fd::{AsRawFd, BorrowedFd};

use crate::linux::{
    EVIOCGRAB, UI_DEV_CREATE, UI_DEV_DESTROY, UI_SET_ABSBIT, UI_SET_EVBIT, UI_SET_KEYBIT,
    USBDEVFS_BULK, USBDEVFS_CLAIMINTERFACE, USBDEVFS_CONNECT, USBDEVFS_CONTROL,
    USBDEVFS_DISCARDURB, USBDEVFS_DISCONNECT_CLAIM, USBDEVFS_IOCTL, USBDEVFS_REAPURBNDELAY,
    USBDEVFS_RELEASEINTERFACE, USBDEVFS_SUBMITURB, UsbBulkTransfer, UsbCtrlTransfer,
    UsbDisconnectClaim, UsbIoctl, UsbUrb,
};

const HID_SET_REPORT: u8 = 0x09;
const HID_GET_REPORT: u8 = 0x01;
const HID_REPORT_FEATURE: u16 = 0x03;
const USB_HID_SET_RT: u8 = 0x21;
const USB_HID_GET_RT: u8 = 0xA1;

/// `_IOC(_IOC_READ|_IOC_WRITE, 'H', 0x06, len)` — `HIDIOCSFEATURE(len)`.
const fn hid_ioc_sfeature(len: usize) -> libc::c_ulong {
    const IOC_READWRITE: u64 = 3;
    const DIRSHIFT: u64 = 30;
    const TYPESHIFT: u64 = 8;
    const SIZESHIFT: u64 = 16;
    ((IOC_READWRITE << DIRSHIFT)
        | ((b'H' as u64) << TYPESHIFT)
        | 0x06
        | ((len as u64) << SIZESHIFT)) as libc::c_ulong
}

fn ioctl_ret(rc: libc::c_int) -> io::Result<i32> {
    if rc < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(rc)
    }
}

pub fn hid_set_feature(fd: BorrowedFd<'_>, buf: &mut [u8]) -> io::Result<i32> {
    let req = hid_ioc_sfeature(buf.len());
    // SAFETY: `fd` is a live hidraw fd; `buf` is the feature report the kernel reads/writes.
    ioctl_ret(unsafe { libc::ioctl(fd.as_raw_fd(), req, buf.as_mut_ptr()) })
}

pub fn usb_set_feature(fd: BorrowedFd<'_>, iface: u16, buf: &mut [u8]) -> io::Result<i32> {
    let mut ct = UsbCtrlTransfer {
        b_request_type: USB_HID_SET_RT,
        b_request: HID_SET_REPORT,
        w_value: (HID_REPORT_FEATURE << 8) | u16::from(buf.first().copied().unwrap_or(0)),
        w_index: iface,
        w_length: buf.len() as u16,
        timeout: 80,
        data: buf.as_mut_ptr().cast(),
    };
    // SAFETY: `ct.data` points at `buf` for the duration of the ioctl.
    ioctl_ret(unsafe { libc::ioctl(fd.as_raw_fd(), USBDEVFS_CONTROL, &raw mut ct) })
}

pub fn usb_get_report(
    fd: BorrowedFd<'_>,
    iface: u16,
    report_type: u8,
    report_id: u8,
    buf: &mut [u8],
) -> io::Result<i32> {
    let mut ct = UsbCtrlTransfer {
        b_request_type: USB_HID_GET_RT,
        b_request: HID_GET_REPORT,
        w_value: (u16::from(report_type) << 8) | u16::from(report_id),
        w_index: iface,
        w_length: buf.len() as u16,
        timeout: 80,
        data: buf.as_mut_ptr().cast(),
    };
    // SAFETY: `ct.data` points at `buf` for the duration of the ioctl.
    ioctl_ret(unsafe { libc::ioctl(fd.as_raw_fd(), USBDEVFS_CONTROL, &raw mut ct) })
}

pub fn usb_bulk(fd: BorrowedFd<'_>, ep: u8, buf: &mut [u8], timeout_ms: u32) -> io::Result<i32> {
    let mut bulk = UsbBulkTransfer {
        ep: u32::from(ep),
        len: buf.len() as u32,
        timeout: timeout_ms,
        data: buf.as_mut_ptr().cast(),
    };
    // SAFETY: `bulk.data` points at `buf` for the duration of the ioctl.
    let rc = unsafe { libc::ioctl(fd.as_raw_fd(), USBDEVFS_BULK, &raw mut bulk) };
    if rc < 0 {
        let err = io::Error::last_os_error();
        if err.raw_os_error() == Some(libc::ETIMEDOUT) {
            return Err(io::Error::from(io::ErrorKind::WouldBlock));
        }
        return Err(err);
    }
    Ok(rc)
}

pub fn usb_submit_urb(fd: BorrowedFd<'_>, urb: &mut UsbUrb) -> io::Result<()> {
    ioctl_ret(unsafe { libc::ioctl(fd.as_raw_fd(), USBDEVFS_SUBMITURB, urb) }).map(|_| ())
}

pub fn usb_discard_urb(fd: BorrowedFd<'_>, urb: &mut UsbUrb) -> io::Result<()> {
    ioctl_ret(unsafe { libc::ioctl(fd.as_raw_fd(), USBDEVFS_DISCARDURB, urb) }).map(|_| ())
}

pub fn usb_reap_urb_ndelay(fd: BorrowedFd<'_>) -> io::Result<*mut UsbUrb> {
    let mut ptr: *mut UsbUrb = std::ptr::null_mut();
    let rc = unsafe { libc::ioctl(fd.as_raw_fd(), USBDEVFS_REAPURBNDELAY, &raw mut ptr) };
    if rc < 0 {
        return Err(io::Error::last_os_error());
    }
    if ptr.is_null() {
        return Err(io::Error::from(io::ErrorKind::WouldBlock));
    }
    Ok(ptr)
}

pub fn usb_claim(fd: BorrowedFd<'_>, iface: u32) -> io::Result<()> {
    let mut dc = UsbDisconnectClaim {
        interface: iface,
        flags: 0,
        driver: [0; 256],
    };
    // SAFETY: `dc` is a kernel UAPI struct; iface is a USB interface number on `fd`.
    let rc = unsafe { libc::ioctl(fd.as_raw_fd(), USBDEVFS_DISCONNECT_CLAIM, &raw mut dc) };
    if rc == 0 {
        return Ok(());
    }
    let mut claim = iface;
    ioctl_ret(unsafe { libc::ioctl(fd.as_raw_fd(), USBDEVFS_CLAIMINTERFACE, &raw mut claim) })
        .map(|_| ())
}

pub fn usb_release(fd: BorrowedFd<'_>, iface: u32) -> io::Result<()> {
    let mut iface = iface;
    ioctl_ret(unsafe { libc::ioctl(fd.as_raw_fd(), USBDEVFS_RELEASEINTERFACE, &raw mut iface) })
        .map(|_| ())
}

pub fn usb_connect_iface(fd: BorrowedFd<'_>, iface: i32) -> io::Result<()> {
    let mut io = UsbIoctl {
        ifno: iface,
        ioctl_code: USBDEVFS_CONNECT as i32,
        data: std::ptr::null_mut(),
    };
    ioctl_ret(unsafe { libc::ioctl(fd.as_raw_fd(), USBDEVFS_IOCTL, &raw mut io) }).map(|_| ())
}

pub fn eviocgrab(fd: BorrowedFd<'_>, grab: bool) -> io::Result<()> {
    let arg = libc::c_int::from(grab);
    ioctl_ret(unsafe { libc::ioctl(fd.as_raw_fd(), EVIOCGRAB, arg) }).map(|_| ())
}

pub fn ui_set_evbit(fd: BorrowedFd<'_>, bit: libc::c_int) -> io::Result<()> {
    ioctl_ret(unsafe { libc::ioctl(fd.as_raw_fd(), UI_SET_EVBIT, bit) }).map(|_| ())
}

pub fn ui_set_keybit(fd: BorrowedFd<'_>, bit: libc::c_int) -> io::Result<()> {
    ioctl_ret(unsafe { libc::ioctl(fd.as_raw_fd(), UI_SET_KEYBIT, bit) }).map(|_| ())
}

pub fn ui_set_absbit(fd: BorrowedFd<'_>, bit: libc::c_int) -> io::Result<()> {
    ioctl_ret(unsafe { libc::ioctl(fd.as_raw_fd(), UI_SET_ABSBIT, bit) }).map(|_| ())
}

pub fn ui_dev_create(fd: BorrowedFd<'_>) -> io::Result<()> {
    ioctl_ret(unsafe { libc::ioctl(fd.as_raw_fd(), UI_DEV_CREATE) }).map(|_| ())
}

pub fn ui_dev_destroy(fd: BorrowedFd<'_>) -> io::Result<()> {
    ioctl_ret(unsafe { libc::ioctl(fd.as_raw_fd(), UI_DEV_DESTROY) }).map(|_| ())
}

pub fn poll(fds: &mut [libc::pollfd], timeout_ms: i32) -> io::Result<i32> {
    let rc = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, timeout_ms) };
    ioctl_ret(rc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::os::fd::AsFd;

    #[test]
    fn ioctls_fail_on_dev_null() {
        let file = File::open("/dev/null").unwrap();
        let fd = file.as_fd();
        let mut buf = [0_u8; 8];
        assert!(hid_set_feature(fd, &mut buf).is_err());
        assert!(usb_set_feature(fd, 2, &mut buf).is_err());
        assert!(usb_get_report(fd, 2, 3, 1, &mut buf).is_err());
        assert!(usb_bulk(fd, 0x83, &mut buf, 1).is_err());
        let mut urb = UsbUrb {
            type_: 1,
            endpoint: 0x83,
            status: 0,
            flags: 0,
            buffer: std::ptr::null_mut(),
            buffer_length: 0,
            actual_length: 0,
            start_frame: 0,
            number_of_packets: 0,
            error_count: 0,
            signr: 0,
            usercontext: std::ptr::null_mut(),
        };
        assert!(usb_submit_urb(fd, &mut urb).is_err());
        assert!(usb_discard_urb(fd, &mut urb).is_err());
        assert!(usb_reap_urb_ndelay(fd).is_err());
        assert!(usb_claim(fd, 2).is_err());
        assert!(usb_release(fd, 2).is_err());
        assert!(usb_connect_iface(fd, 2).is_err());
        assert!(eviocgrab(fd, true).is_err());
        assert!(eviocgrab(fd, false).is_err());
        assert!(ui_set_evbit(fd, 1).is_err());
        assert!(ui_set_keybit(fd, 1).is_err());
        assert!(ui_set_absbit(fd, 1).is_err());
        assert!(ui_dev_create(fd).is_err());
        assert!(ui_dev_destroy(fd).is_err());
        assert_eq!(poll(&mut [], 0).unwrap(), 0);
        assert!(ioctl_ret(-1).is_err());
        assert_eq!(ioctl_ret(3).unwrap(), 3);
    }
}

//! Linux UAPI bits libc does not export.

#![allow(clippy::cast_possible_truncation, dead_code, missing_docs)]

use std::mem::size_of;

pub const EV_SYN: u16 = 0x00;
pub const EV_KEY: u16 = 0x01;
pub const EV_ABS: u16 = 0x03;
pub const SYN_REPORT: u16 = 0;

pub const ABS_X: u16 = 0x00;
pub const ABS_Y: u16 = 0x01;
pub const ABS_Z: u16 = 0x02;
pub const ABS_RX: u16 = 0x03;
pub const ABS_RY: u16 = 0x04;
pub const ABS_RZ: u16 = 0x05;
pub const ABS_HAT0X: u16 = 0x10;
pub const ABS_HAT0Y: u16 = 0x11;
pub const ABS_CNT: usize = 0x40;

pub const BUS_USB: u16 = 0x03;

const IOC_NONE: u32 = 0;
const IOC_WRITE: u32 = 1;
const IOC_READ: u32 = 2;

const fn ioc(dir: u32, typ: u8, nr: u32, size: u32) -> libc::c_ulong {
    (((dir as u64) << 30) | ((typ as u64) << 8) | (nr as u64) | ((size as u64) << 16))
        as libc::c_ulong
}

const fn io(typ: u8, nr: u32) -> libc::c_ulong {
    ioc(IOC_NONE, typ, nr, 0)
}

const fn iow(typ: u8, nr: u32, size: u32) -> libc::c_ulong {
    ioc(IOC_WRITE, typ, nr, size)
}

const fn ior(typ: u8, nr: u32, size: u32) -> libc::c_ulong {
    ioc(IOC_READ, typ, nr, size)
}

const fn iowr(typ: u8, nr: u32, size: u32) -> libc::c_ulong {
    ioc(IOC_READ | IOC_WRITE, typ, nr, size)
}

pub const UI_DEV_CREATE: libc::c_ulong = io(b'U', 1);
pub const UI_DEV_DESTROY: libc::c_ulong = io(b'U', 2);
pub const UI_SET_EVBIT: libc::c_ulong = iow(b'U', 100, size_of::<libc::c_int>() as u32);
pub const UI_SET_KEYBIT: libc::c_ulong = iow(b'U', 101, size_of::<libc::c_int>() as u32);
pub const UI_SET_ABSBIT: libc::c_ulong = iow(b'U', 103, size_of::<libc::c_int>() as u32);

pub const EVIOCGRAB: libc::c_ulong = iow(b'E', 0x90, size_of::<libc::c_int>() as u32);

pub const USBDEVFS_CONTROL: libc::c_ulong = iowr(b'U', 0, size_of::<UsbCtrlTransfer>() as u32);
pub const USBDEVFS_BULK: libc::c_ulong = iowr(b'U', 2, size_of::<UsbBulkTransfer>() as u32);
pub const USBDEVFS_SUBMITURB: libc::c_ulong = ior(b'U', 10, size_of::<UsbUrb>() as u32);
pub const USBDEVFS_DISCARDURB: libc::c_ulong = io(b'U', 11);
pub const USBDEVFS_REAPURBNDELAY: libc::c_ulong =
    iow(b'U', 13, size_of::<*mut libc::c_void>() as u32);
pub const USBDEVFS_CLAIMINTERFACE: libc::c_ulong = ior(b'U', 15, size_of::<u32>() as u32);
pub const USBDEVFS_URB_TYPE_INTERRUPT: u8 = 1;
pub const USBDEVFS_URB_TYPE_BULK: u8 = 3;
pub const USBDEVFS_RELEASEINTERFACE: libc::c_ulong = ior(b'U', 16, size_of::<u32>() as u32);
pub const USBDEVFS_IOCTL: libc::c_ulong = iowr(b'U', 18, size_of::<UsbIoctl>() as u32);
pub const USBDEVFS_CONNECT: libc::c_ulong = io(b'U', 23);
pub const USBDEVFS_DISCONNECT_CLAIM: libc::c_ulong =
    ior(b'U', 27, size_of::<UsbDisconnectClaim>() as u32);

#[repr(C)]
#[derive(Clone, Copy)]
pub struct InputEvent {
    pub time: libc::timeval,
    pub type_: u16,
    pub code: u16,
    pub value: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct InputId {
    pub bustype: u16,
    pub vendor: u16,
    pub product: u16,
    pub version: u16,
}

#[repr(C)]
pub struct UinputUserDev {
    pub name: [libc::c_char; 80],
    pub id: InputId,
    pub ff_effects_max: u32,
    pub absmax: [i32; ABS_CNT],
    pub absmin: [i32; ABS_CNT],
    pub absfuzz: [i32; ABS_CNT],
    pub absflat: [i32; ABS_CNT],
}

#[repr(C)]
pub struct UsbCtrlTransfer {
    pub b_request_type: u8,
    pub b_request: u8,
    pub w_value: u16,
    pub w_index: u16,
    pub w_length: u16,
    pub timeout: u32,
    pub data: *mut libc::c_void,
}

#[repr(C)]
pub struct UsbBulkTransfer {
    pub ep: u32,
    pub len: u32,
    pub timeout: u32,
    pub data: *mut libc::c_void,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct UsbUrb {
    pub type_: u8,
    pub endpoint: u8,
    pub status: i32,
    pub flags: u32,
    pub buffer: *mut libc::c_void,
    pub buffer_length: i32,
    pub actual_length: i32,
    pub start_frame: i32,
    pub number_of_packets: i32,
    pub error_count: i32,
    pub signr: u32,
    pub usercontext: *mut libc::c_void,
}

#[repr(C)]
pub struct UsbIoctl {
    pub ifno: i32,
    pub ioctl_code: i32,
    pub data: *mut libc::c_void,
}

#[repr(C)]
pub struct UsbDisconnectClaim {
    pub interface: u32,
    pub flags: u32,
    pub driver: [u8; 256],
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    #[test]
    fn uapi_struct_sizes() {
        assert_eq!(size_of::<UsbCtrlTransfer>(), 24);
        assert_eq!(size_of::<UsbBulkTransfer>(), 24);
        assert_eq!(size_of::<UsbUrb>(), 56);
        assert_eq!(std::mem::offset_of!(UsbUrb, buffer), 16);
        assert_eq!(std::mem::offset_of!(UsbUrb, usercontext), 48);
        assert_eq!(size_of::<UsbIoctl>(), 16);
        assert_eq!(size_of::<UsbDisconnectClaim>(), 264);
        assert_eq!(size_of::<InputEvent>(), 24);
        assert_eq!(size_of::<UinputUserDev>(), 1116);
        assert_ne!(UI_DEV_CREATE, 0);
        assert_ne!(UI_DEV_DESTROY, 0);
        assert_ne!(UI_SET_EVBIT, 0);
        assert_ne!(UI_SET_KEYBIT, 0);
        assert_ne!(UI_SET_ABSBIT, 0);
        assert_ne!(EVIOCGRAB, 0);
        assert_ne!(USBDEVFS_CONTROL, 0);
        assert_ne!(USBDEVFS_BULK, 0);
        assert_ne!(USBDEVFS_SUBMITURB, 0);
        assert_ne!(USBDEVFS_DISCARDURB, 0);
        assert_ne!(USBDEVFS_REAPURBNDELAY, 0);
        assert_ne!(USBDEVFS_CLAIMINTERFACE, 0);
        assert_ne!(USBDEVFS_RELEASEINTERFACE, 0);
        assert_ne!(USBDEVFS_IOCTL, 0);
        assert_ne!(USBDEVFS_CONNECT, 0);
        assert_ne!(USBDEVFS_DISCONNECT_CLAIM, 0);
        assert_eq!(io(b'U', 1), UI_DEV_CREATE);
        assert_eq!(
            iow(b'U', 100, size_of::<libc::c_int>() as u32),
            UI_SET_EVBIT
        );
        assert_eq!(
            ior(b'U', 15, size_of::<u32>() as u32),
            USBDEVFS_CLAIMINTERFACE
        );
        assert_eq!(
            iowr(b'U', 0, size_of::<UsbCtrlTransfer>() as u32),
            USBDEVFS_CONTROL
        );
        assert_eq!(ioc(IOC_NONE, b'U', 11, 0), USBDEVFS_DISCARDURB);
    }
}

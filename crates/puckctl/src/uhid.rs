//! Virtual hidraw (`/dev/uhid`) so SDL can read Triton reports while usbfs
//! holds the real dongle. Presented as wired Triton (`28de:1302`) because SDL
//! only accepts Proteus/Nereid (`1304`/`1305`) when USB interface is 2..=5,
//! and uhid has no USB parent.

use std::fs::File;
use std::io::{self, Write};
use std::os::fd::{AsFd, AsRawFd};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use puckctl_protocol::{PID_TRITON_WIRED, VALVE_VID};

use crate::linux::BUS_USB;
use crate::log::logln;
use crate::sys;
use crate::usb::UsbDevice;

const UHID_DESTROY: u32 = 1;
const UHID_OUTPUT: u32 = 6;
const UHID_GET_REPORT: u32 = 8;
const UHID_GET_REPORT_REPLY: u32 = 9;
const UHID_CREATE2: u32 = 11;
const UHID_INPUT2: u32 = 12;
const UHID_SET_REPORT: u32 = 13;
const UHID_SET_REPORT_REPLY: u32 = 14;

const UHID_DATA_MAX: usize = 4096;
const CREATE2_HEAD: usize = 128 + 64 + 64 + 2 + 2 + 16;
const USB_REPORT_FEATURE: u8 = 3;

#[derive(Debug)]
pub struct UhidDevice {
    fd: File,
}

impl UhidDevice {
    pub fn create(iface: i32) -> io::Result<Self> {
        if !crate::hw::allowed() {
            return Err(io::Error::other("uhid disabled in tests"));
        }
        let mut fd = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(Path::new("/dev/uhid"))
            .map_err(|err| {
                logln(format!(
                    "open /dev/uhid failed: {err} (install udev/60-puckctl.rules?)"
                ));
                err
            })?;
        fd.write_all(&encode_create2(iface))?;
        logln(format!(
            "virtual hidraw 28de:{PID_TRITON_WIRED:04x} (gyro) for interface {iface}"
        ));
        Ok(Self { fd })
    }

    #[must_use]
    pub fn raw_fd(&self) -> i32 {
        self.fd.as_raw_fd()
    }

    pub fn input(&self, data: &[u8]) -> io::Result<()> {
        write_all(&self.fd, &encode_input2(data))
    }

    #[cfg(test)]
    fn from_file(fd: File) -> Self {
        Self { fd }
    }

    pub fn pump(&self, usb: Option<&UsbDevice>, iface: i32, ep_out: u8) {
        let mut buf = vec![0_u8; 4 + CREATE2_HEAD + UHID_DATA_MAX];
        loop {
            let n = unsafe { libc::read(self.fd.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len()) };
            if n < 0 {
                let err = io::Error::last_os_error();
                if err.kind() != io::ErrorKind::WouldBlock {
                    logln(format!("uhid read: {err}"));
                }
                return;
            }
            if n == 0 {
                return;
            }
            handle_event(&self.fd, &buf[..n as usize], usb, iface, ep_out);
        }
    }
}

impl Drop for UhidDevice {
    fn drop(&mut self) {
        let _ = write_all(&self.fd, &UHID_DESTROY.to_le_bytes());
    }
}

fn write_all(fd: &File, bytes: &[u8]) -> io::Result<()> {
    let n = unsafe { libc::write(fd.as_raw_fd(), bytes.as_ptr().cast(), bytes.len()) };
    if n < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn put_str(dst: &mut [u8], s: &str) {
    let raw = s.as_bytes();
    let n = raw.len().min(dst.len().saturating_sub(1));
    dst[..n].copy_from_slice(&raw[..n]);
}

pub(crate) fn report_descriptor() -> Vec<u8> {
    let mut d = vec![
        0x06, 0x00, 0xFF, 0x09, 0x01, 0xA1, 0x01, 0x15, 0x00, 0x26, 0xFF, 0x00, 0x75, 0x08,
    ];
    for id in [0x42_u8, 0x43, 0x45, 0x46, 0x47, 0x79] {
        d.extend([0x85, id, 0x09, 0x01, 0x95, 63, 0x81, 0x02]);
    }
    d.extend([0x85, 0x01, 0x09, 0x02, 0x95, 63, 0xB1, 0x02]);
    d.extend([0x85, 0x80, 0x09, 0x03, 0x95, 9, 0x91, 0x02]);
    d.push(0xC0);
    d
}

pub(crate) fn encode_create2(iface: i32) -> Vec<u8> {
    let rd = report_descriptor();
    let mut ev = vec![0_u8; 4 + CREATE2_HEAD + rd.len()];
    ev[..4].copy_from_slice(&UHID_CREATE2.to_le_bytes());
    put_str(&mut ev[4..132], "Valve Software Steam Controller");
    put_str(&mut ev[132..196], &format!("usb-puckctl/input{iface}"));
    put_str(&mut ev[196..260], &format!("puckctl-if{iface}"));
    ev[260..262].copy_from_slice(&(rd.len() as u16).to_le_bytes());
    ev[262..264].copy_from_slice(&BUS_USB.to_le_bytes());
    ev[264..268].copy_from_slice(&u32::from(VALVE_VID).to_le_bytes());
    ev[268..272].copy_from_slice(&u32::from(PID_TRITON_WIRED).to_le_bytes());
    ev[272..276].copy_from_slice(&0x0110_u32.to_le_bytes());
    ev[280..280 + rd.len()].copy_from_slice(&rd);
    ev
}

pub(crate) fn encode_input2(data: &[u8]) -> Vec<u8> {
    let n = data.len().min(UHID_DATA_MAX);
    let mut ev = vec![0_u8; 6 + n];
    ev[..4].copy_from_slice(&UHID_INPUT2.to_le_bytes());
    ev[4..6].copy_from_slice(&(n as u16).to_le_bytes());
    ev[6..].copy_from_slice(&data[..n]);
    ev
}

fn uhid_to_usb_rtype(rtype: u8) -> u8 {
    match rtype {
        0 => USB_REPORT_FEATURE,
        1 => 2,
        _ => 1,
    }
}

fn handle_event(fd: &File, ev: &[u8], usb: Option<&UsbDevice>, iface: i32, ep_out: u8) {
    let Some(kind) = ev
        .get(..4)
        .and_then(|b| b.try_into().ok().map(u32::from_le_bytes))
    else {
        return;
    };
    let body = ev.get(4..).unwrap_or(&[]);
    match kind {
        UHID_OUTPUT => {
            let size = body
                .get(UHID_DATA_MAX..UHID_DATA_MAX + 2)
                .and_then(|b| b.try_into().ok())
                .map(u16::from_le_bytes)
                .unwrap_or(0) as usize;
            if let (Some(usb), Some(data)) = (usb, body.get(..size.min(UHID_DATA_MAX)))
                && !data.is_empty()
            {
                let mut buf = data.to_vec();
                let _ = sys::usb_bulk(usb.fd.as_fd(), ep_out, &mut buf, 40);
            }
        }
        UHID_SET_REPORT => {
            let Some(id) = body
                .get(..4)
                .and_then(|b| b.try_into().ok().map(u32::from_le_bytes))
            else {
                return;
            };
            let size = body
                .get(6..8)
                .and_then(|b| b.try_into().ok())
                .map(u16::from_le_bytes)
                .unwrap_or(0) as usize;
            let mut err = 0_u16;
            if let (Some(usb), Some(data)) = (usb, body.get(8..8 + size)) {
                let mut buf = data.to_vec();
                if sys::usb_set_feature(usb.fd.as_fd(), u16::try_from(iface).unwrap_or(0), &mut buf)
                    .is_err()
                {
                    err = 1;
                }
            }
            let _ = write_all(fd, &encode_set_reply(id, err));
        }
        UHID_GET_REPORT => {
            let Some(id) = body
                .get(..4)
                .and_then(|b| b.try_into().ok().map(u32::from_le_bytes))
            else {
                return;
            };
            let rnum = body.get(4).copied().unwrap_or(0);
            let rtype = uhid_to_usb_rtype(body.get(5).copied().unwrap_or(0));
            let mut data = [0_u8; 64];
            let n = usb
                .and_then(|u| {
                    sys::usb_get_report(
                        u.fd.as_fd(),
                        u16::try_from(iface).unwrap_or(0),
                        rtype,
                        rnum,
                        &mut data,
                    )
                    .ok()
                })
                .and_then(|n| usize::try_from(n).ok())
                .unwrap_or(0);
            let _ = write_all(fd, &encode_get_reply(id, 0, &data[..n.min(64)]));
        }
        _ => {}
    }
}

fn encode_set_reply(id: u32, err: u16) -> Vec<u8> {
    let mut ev = vec![0_u8; 10];
    ev[..4].copy_from_slice(&UHID_SET_REPORT_REPLY.to_le_bytes());
    ev[4..8].copy_from_slice(&id.to_le_bytes());
    ev[8..10].copy_from_slice(&err.to_le_bytes());
    ev
}

fn encode_get_reply(id: u32, err: u16, data: &[u8]) -> Vec<u8> {
    let n = data.len().min(UHID_DATA_MAX);
    let mut ev = vec![0_u8; 12 + n];
    ev[..4].copy_from_slice(&UHID_GET_REPORT_REPLY.to_le_bytes());
    ev[4..8].copy_from_slice(&id.to_le_bytes());
    ev[8..10].copy_from_slice(&err.to_le_bytes());
    ev[10..12].copy_from_slice(&(n as u16).to_le_bytes());
    ev[12..].copy_from_slice(&data[..n]);
    ev
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_and_create2_are_wired_triton() {
        let rd = report_descriptor();
        assert!(rd.contains(&0x42));
        assert!(rd.contains(&0x80));
        assert_eq!(*rd.last().unwrap(), 0xC0);
        let ev = encode_create2(2);
        assert_eq!(&ev[..4], &UHID_CREATE2.to_le_bytes());
        assert!(ev[4..132].starts_with(b"Valve Software Steam Controller"));
        assert!(ev[132..196].starts_with(b"usb-puckctl/input2"));
        assert!(ev[196..260].starts_with(b"puckctl-if2"));
        assert_eq!(&ev[268..272], &u32::from(PID_TRITON_WIRED).to_le_bytes());
        assert_eq!(&ev[264..268], &u32::from(VALVE_VID).to_le_bytes());
        assert!(UhidDevice::create(2).is_err());
        assert_eq!(uhid_to_usb_rtype(0), USB_REPORT_FEATURE);
        assert_eq!(uhid_to_usb_rtype(1), 2);
        assert_eq!(uhid_to_usb_rtype(2), 1);
    }

    #[test]
    fn input_and_replies_encode() {
        let ev = encode_input2(&[0x42, 1, 2, 3]);
        assert_eq!(&ev[..4], &UHID_INPUT2.to_le_bytes());
        assert_eq!(&ev[4..6], &4_u16.to_le_bytes());
        assert_eq!(&ev[6..], &[0x42, 1, 2, 3]);
        let set = encode_set_reply(9, 1);
        assert_eq!(&set[..4], &UHID_SET_REPORT_REPLY.to_le_bytes());
        assert_eq!(&set[4..8], &9_u32.to_le_bytes());
        assert_eq!(&set[8..], &1_u16.to_le_bytes());
        let get = encode_get_reply(3, 0, &[1, 2]);
        assert_eq!(&get[..4], &UHID_GET_REPORT_REPLY.to_le_bytes());
        assert_eq!(&get[10..12], &2_u16.to_le_bytes());
        assert_eq!(&get[12..], &[1, 2]);
        let mut dst = [0_u8; 8];
        put_str(&mut dst, "hi");
        assert_eq!(&dst[..2], b"hi");
        let null = File::open("/dev/null").unwrap();
        handle_event(&null, &[0, 0, 0, 0], None, 2, 0x02);
        handle_event(&null, &[], None, 2, 0x02);
        let mut output = vec![0_u8; 4 + UHID_DATA_MAX + 2];
        output[..4].copy_from_slice(&UHID_OUTPUT.to_le_bytes());
        output[4] = 0x80;
        output[4 + UHID_DATA_MAX..4 + UHID_DATA_MAX + 2].copy_from_slice(&1_u16.to_le_bytes());
        handle_event(&null, &output, None, 2, 0x02);
        let mut set = vec![0_u8; 12];
        set[..4].copy_from_slice(&UHID_SET_REPORT.to_le_bytes());
        set[4..8].copy_from_slice(&1_u32.to_le_bytes());
        set[8] = 1;
        set[10..12].copy_from_slice(&0_u16.to_le_bytes());
        handle_event(&null, &set, None, 2, 0x02);
        let mut get = vec![0_u8; 10];
        get[..4].copy_from_slice(&UHID_GET_REPORT.to_le_bytes());
        get[4..8].copy_from_slice(&2_u32.to_le_bytes());
        get[8] = 1;
        handle_event(&null, &get, None, 2, 0x02);
        let usb = crate::usb::UsbDevice::new(File::open("/dev/null").unwrap(), vec![2]);
        output[4 + UHID_DATA_MAX..4 + UHID_DATA_MAX + 2].copy_from_slice(&2_u16.to_le_bytes());
        handle_event(&null, &output, Some(&usb), 2, 0x02);
        let mut set_data = vec![0_u8; 13];
        set_data[..4].copy_from_slice(&UHID_SET_REPORT.to_le_bytes());
        set_data[10..12].copy_from_slice(&1_u16.to_le_bytes());
        set_data[12] = 0x01;
        handle_event(&null, &set_data, Some(&usb), 2, 0x02);
        handle_event(&null, &get, Some(&usb), 2, 0x02);
        let (r, w) = crate::test_env::nonblock_pipe();
        let dev = UhidDevice::from_file(w);
        assert!(dev.raw_fd() >= 0);
        let _ = dev.input(&[0x42, 1]);
        dev.pump(None, 2, 0x02);
        dev.pump(Some(&usb), 2, 0x02);
        drop(dev);
        drop(r);
    }
}

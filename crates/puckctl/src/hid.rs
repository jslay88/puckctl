use std::fs::File;
use std::io;
use std::os::fd::{AsFd, AsRawFd};

use crate::sys;
use crate::usb::UsbDevice;

#[derive(Debug)]
pub enum Transport {
    Hidraw(File),
    Usbfs { ep_in: u8 },
}

impl Transport {
    #[must_use]
    pub fn is_usbfs(&self) -> bool {
        matches!(self, Self::Usbfs { .. })
    }

    #[must_use]
    pub fn hidraw_raw(&self) -> Option<i32> {
        match self {
            Self::Hidraw(file) => Some(file.as_raw_fd()),
            Self::Usbfs { .. } => None,
        }
    }
}

pub fn send_feature(
    transport: &Transport,
    usb: Option<&UsbDevice>,
    iface: i32,
    buf: &mut [u8],
) -> io::Result<i32> {
    match transport {
        Transport::Hidraw(file) => sys::hid_set_feature(file.as_fd(), buf),
        Transport::Usbfs { .. } => {
            let usb = usb.ok_or_else(|| io::Error::other("usbfs device missing"))?;
            sys::usb_set_feature(usb.fd.as_fd(), u16::try_from(iface).unwrap_or(0), buf)
        }
    }
}

pub fn read_input(
    transport: &Transport,
    usb: Option<&UsbDevice>,
    buf: &mut [u8],
) -> io::Result<usize> {
    match transport {
        Transport::Hidraw(file) => raw_read(file, buf),
        Transport::Usbfs { ep_in } => {
            let usb = usb.ok_or_else(|| io::Error::other("usbfs device missing"))?;
            let n = sys::usb_bulk(usb.fd.as_fd(), *ep_in, buf, 1)?;
            Ok(usize::try_from(n).unwrap_or(0))
        }
    }
}

fn raw_read(file: &File, buf: &mut [u8]) -> io::Result<usize> {
    let n = unsafe { libc::read(file.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len()) };
    if n < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(n as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn transport_and_io() {
        let (r, mut w) = crate::test_env::nonblock_pipe();
        let hid = Transport::Hidraw(r);
        assert!(!hid.is_usbfs());
        assert!(hid.hidraw_raw().is_some());
        w.write_all(&[0x42, 1, 2, 3]).unwrap();
        let mut buf = [0_u8; 8];
        assert_eq!(read_input(&hid, None, &mut buf).unwrap(), 4);
        let mut feat = [0_u8; 8];
        assert!(send_feature(&hid, None, 2, &mut feat).is_err());

        let usb = Transport::Usbfs { ep_in: 0x83 };
        assert!(usb.is_usbfs());
        assert!(usb.hidraw_raw().is_none());
        assert!(read_input(&usb, None, &mut buf).is_err());
        assert!(send_feature(&usb, None, 2, &mut feat).is_err());
    }
}

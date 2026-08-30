#![allow(dead_code)]

use std::io;
use std::os::fd::BorrowedFd;
use std::time::{Duration, Instant};

use crate::linux::{USBDEVFS_URB_TYPE_BULK, USBDEVFS_URB_TYPE_INTERRUPT, UsbUrb};
use crate::sys;

const IN_LEN: i32 = 128;
const ERROR_BACKOFF: Duration = Duration::from_millis(20);

pub struct InUrb {
    urb: UsbUrb,
    buf: [u8; 128],
    pub slot: usize,
    pub submitted: bool,
    next_submit: Option<Instant>,
}

// SAFETY: `urb.buffer` always points at `buf` on this same struct.
unsafe impl Send for InUrb {}

impl InUrb {
    #[must_use]
    pub fn new(slot: usize, ep: u8) -> Self {
        Self {
            urb: UsbUrb {
                type_: USBDEVFS_URB_TYPE_INTERRUPT,
                endpoint: ep,
                status: 0,
                flags: 0,
                buffer: std::ptr::null_mut(),
                buffer_length: IN_LEN,
                actual_length: 0,
                start_frame: 0,
                number_of_packets: 0,
                error_count: 0,
                signr: 0,
                usercontext: slot as *mut libc::c_void,
            },
            buf: [0; 128],
            slot,
            submitted: false,
            next_submit: None,
        }
    }

    pub fn submit(&mut self, fd: BorrowedFd<'_>) -> io::Result<()> {
        self.urb.buffer = self.buf.as_mut_ptr().cast();
        self.urb.buffer_length = IN_LEN;
        self.urb.actual_length = 0;
        self.urb.status = 0;
        self.urb.usercontext = self.slot as *mut libc::c_void;
        if let Err(err) = sys::usb_submit_urb(fd, &mut self.urb) {
            self.urb.type_ = USBDEVFS_URB_TYPE_BULK;
            sys::usb_submit_urb(fd, &mut self.urb)?;
            let _ = err;
        }
        self.submitted = true;
        self.next_submit = None;
        Ok(())
    }

    pub fn note_idle(&mut self) {
        self.submitted = false;
        self.next_submit = Some(Instant::now() + ERROR_BACKOFF);
    }

    #[must_use]
    pub fn due(&self) -> bool {
        !self.submitted && self.next_submit.is_none_or(|t| Instant::now() >= t)
    }

    pub fn discard(&mut self, fd: BorrowedFd<'_>) {
        if !self.submitted {
            return;
        }
        let _ = sys::usb_discard_urb(fd, &mut self.urb);
        self.submitted = false;
    }
}

pub fn slot_of(urb: &UsbUrb) -> usize {
    urb.usercontext as usize
}

pub fn copy_report(urb: &UsbUrb) -> Option<(usize, [u8; 128], usize)> {
    if urb.status < 0 || urb.actual_length <= 0 {
        return None;
    }
    let n = usize::try_from(urb.actual_length).ok()?.min(128);
    let buf = unsafe { std::slice::from_raw_parts(urb.buffer.cast::<u8>(), n) };
    let mut out = [0_u8; 128];
    out[..n].copy_from_slice(buf);
    Some((slot_of(urb), out, n))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::os::fd::AsFd;

    #[test]
    fn urb_lifecycle_and_copy() {
        let mut pending = InUrb::new(2, 0x83);
        assert_eq!(pending.slot, 2);
        assert!(pending.due());
        pending.note_idle();
        assert!(!pending.due());
        pending.next_submit = Some(Instant::now() - Duration::from_millis(1));
        assert!(pending.due());
        let file = File::open("/dev/null").unwrap();
        pending.discard(file.as_fd());
        pending.submitted = true;
        pending.discard(file.as_fd());
        assert!(!pending.submitted);
        assert!(pending.submit(file.as_fd()).is_err());

        let mut urb = pending.urb;
        urb.usercontext = 3 as *mut libc::c_void;
        assert_eq!(slot_of(&urb), 3);
        urb.status = -1;
        urb.actual_length = 4;
        assert!(copy_report(&urb).is_none());
        urb.status = 0;
        urb.actual_length = 0;
        assert!(copy_report(&urb).is_none());
        let mut data = [9_u8, 8, 7, 6];
        urb.actual_length = 4;
        urb.buffer = data.as_mut_ptr().cast();
        let (slot, buf, n) = copy_report(&urb).unwrap();
        assert_eq!(slot, 3);
        assert_eq!(n, 4);
        assert_eq!(&buf[..4], &[9, 8, 7, 6]);
    }
}

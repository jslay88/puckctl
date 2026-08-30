use std::time::{Duration, Instant};

use puckctl_protocol::{
    LIZARD_HEARTBEAT_MS, LIZARD_HEARTBEAT_STEAM_MS, lizard_off_feature, lizard_on_feature,
    load_default_settings_feature,
};

use crate::hid::{self, Transport};
use crate::pad::VirtualPad;
use crate::uhid::UhidDevice;
use crate::usb::UsbDevice;

pub const MAX_SLOTS: usize = 4;

#[derive(Debug)]
pub struct Slot {
    pub path: String,
    pub iface: i32,
    pub transport: Transport,
    pub pad: Option<VirtualPad>,
    pub uhid: Option<UhidDevice>,
    pub connected: bool,
    pub last_lizard: Option<Instant>,
    pub last_buttons: u32,
    pub last_abs: [i32; 6],
    pub last_hat: [i32; 2],
}

impl Slot {
    #[must_use]
    pub fn new(path: String, iface: i32, transport: Transport) -> Self {
        Self {
            path,
            iface,
            transport,
            pad: None,
            uhid: None,
            connected: false,
            last_lizard: None,
            last_buttons: 0,
            last_abs: [0; 6],
            last_hat: [0; 2],
        }
    }

    fn log_feature_err(op: &str, path: &str, err: &std::io::Error) {
        match err.raw_os_error() {
            Some(libc::EPIPE | libc::ENODEV | libc::EAGAIN | libc::ETIMEDOUT) => {}
            _ => crate::log::logln(format!("{op} on {path}: {err}")),
        }
    }

    pub fn send_lizard_off(&mut self, usb: Option<&UsbDevice>) {
        let mut buf = lizard_off_feature();
        if let Err(err) = hid::send_feature(&self.transport, usb, self.iface, &mut buf) {
            Self::log_feature_err("lizard off", &self.path, &err);
        }
        self.last_lizard = Some(Instant::now());
    }

    pub fn send_lizard_on(&mut self, usb: Option<&UsbDevice>) {
        let mut set = lizard_on_feature();
        if let Err(err) = hid::send_feature(&self.transport, usb, self.iface, &mut set) {
            Self::log_feature_err("lizard on", &self.path, &err);
        }
        let mut def = load_default_settings_feature();
        if let Err(err) = hid::send_feature(&self.transport, usb, self.iface, &mut def) {
            Self::log_feature_err("load defaults", &self.path, &err);
        }
        self.last_lizard = Some(Instant::now());
    }

    pub fn lizard_due(&self, steam_override: bool, steam_seen: bool, paused: bool) -> bool {
        let ms = if steam_seen && (steam_override || paused) {
            LIZARD_HEARTBEAT_STEAM_MS
        } else {
            LIZARD_HEARTBEAT_MS
        };
        self.last_lizard
            .is_none_or(|t| t.elapsed() >= Duration::from_millis(ms))
    }

    pub fn read_report(&self, usb: Option<&UsbDevice>, buf: &mut [u8]) -> std::io::Result<usize> {
        hid::read_input(&self.transport, usb, buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hid::Transport;

    #[test]
    fn lizard_due_and_feature_errors() {
        let (r, _w) = crate::test_env::nonblock_pipe();
        let mut slot = Slot::new("pipe".into(), 2, Transport::Hidraw(r));
        assert!(slot.lizard_due(false, false, false));
        assert!(slot.lizard_due(true, true, false));
        assert!(slot.lizard_due(false, true, true));
        slot.send_lizard_off(None);
        assert!(!slot.lizard_due(true, true, false));
        assert!(!slot.lizard_due(false, true, true));
        slot.last_lizard = Some(Instant::now() - Duration::from_secs(4));
        assert!(slot.lizard_due(false, false, false));
        slot.send_lizard_on(None);
        Slot::log_feature_err("op", "p", &std::io::Error::from_raw_os_error(libc::EPIPE));
        Slot::log_feature_err("op", "p", &std::io::Error::other("nope"));
        let mut buf = [0_u8; 8];
        let _ = slot.read_report(None, &mut buf);
    }
}

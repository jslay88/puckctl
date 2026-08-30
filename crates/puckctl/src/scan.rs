use std::fs;
use std::path::Path;
use std::thread;
use std::time::Duration;

use puckctl_protocol::{VALVE_VID, is_bridge_target, is_puck_pid};

use crate::hid::Transport;
use crate::log::logln;
use crate::mode::Mode;
use crate::slot::{MAX_SLOTS, Slot};
use crate::usb::{self, UsbDevice, open_rw_nonblock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HidInfo {
    pub vid: u16,
    pub pid: u16,
    pub iface: i32,
}

#[must_use]
pub fn parse_uevent(hidraw_name: &str) -> Option<HidInfo> {
    let path = format!("/sys/class/hidraw/{hidraw_name}/device/uevent");
    parse_uevent_text(&fs::read_to_string(path).ok()?)
}

#[must_use]
pub fn parse_uevent_text(text: &str) -> Option<HidInfo> {
    let mut vid = 0;
    let mut pid = 0;
    let mut iface = -1;
    let mut have_id = false;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("HID_ID=") {
            let parts: Vec<&str> = rest.split(':').collect();
            if parts.len() == 3
                && let (Ok(v), Ok(p)) = (
                    u16::from_str_radix(parts[1], 16),
                    u16::from_str_radix(parts[2], 16),
                )
            {
                vid = v;
                pid = p;
                have_id = true;
            }
        } else if let Some(rest) = line.strip_prefix("HID_PHYS=")
            && let Some(idx) = rest.find("/input")
        {
            let n = rest[idx + 6..].chars().take_while(|c| c.is_ascii_digit());
            iface = n.collect::<String>().parse().unwrap_or(-1);
        }
    }
    have_id.then_some(HidInfo { vid, pid, iface })
}

#[must_use]
pub fn puck_hidraw_present() -> bool {
    let Ok(dir) = fs::read_dir("/sys/class/hidraw") else {
        return false;
    };
    dir.flatten().any(|entry| {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        name.starts_with("hidraw")
            && parse_uevent(&name)
                .is_some_and(|info| info.vid == VALVE_VID && is_puck_pid(info.pid))
    })
}

fn scan_devices_hidraw() -> Vec<Slot> {
    let Ok(dir) = fs::read_dir("/sys/class/hidraw") else {
        return Vec::new();
    };
    let mut slots = Vec::new();
    for entry in dir.flatten() {
        if slots.len() >= MAX_SLOTS {
            break;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with("hidraw") {
            continue;
        }
        let Some(info) = parse_uevent(&name) else {
            continue;
        };
        if !is_bridge_target(info.vid, info.pid, info.iface) {
            continue;
        }
        let path = format!("/dev/{name}");
        let file = {
            let mut opened = None;
            for _ in 0..8 {
                match open_rw_nonblock(Path::new(&path)) {
                    Ok(file) => {
                        opened = Some(file);
                        break;
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
                        thread::sleep(Duration::from_millis(50));
                    }
                    Err(err) => {
                        logln(format!("open {path} failed: {err}"));
                        break;
                    }
                }
            }
            opened
        };
        let Some(file) = file else {
            continue;
        };
        logln(format!(
            "slot {}: {path} (pid {:04x}, usb interface {})",
            slots.len(),
            info.pid,
            info.iface
        ));
        slots.push(Slot::new(path, info.iface, Transport::Hidraw(file)));
    }
    slots
}

pub fn scan_devices(
    override_steam: bool,
    requested: Mode,
    usb: &mut Option<UsbDevice>,
) -> Vec<Slot> {
    usb::usbfs_release(usb);
    if override_steam && requested == Mode::Gamepad {
        match usb::scan_devices_usbfs() {
            Ok((slots, claimed)) if !slots.is_empty() => {
                *usb = claimed;
                return slots;
            }
            Ok(_) => logln("Steam override: usbfs claim failed, falling back to hidraw"),
            Err(err) => logln(format!("Steam override: usbfs claim failed: {err}")),
        }
    }

    let mut slots = scan_devices_hidraw();
    if slots.is_empty() && requested == Mode::Lizard && usb::restore_hid_drivers() {
        slots = scan_devices_hidraw();
    }
    slots
}

pub fn close_all(
    slots: &mut Vec<Slot>,
    grabs: &mut Vec<std::fs::File>,
    usb: &mut Option<UsbDevice>,
) {
    crate::grab::ungrab(grabs);
    let mut via_usbfs = false;
    for slot in slots.drain(..) {
        let mut slot = slot;
        crate::pad::destroy_pad(&mut slot);
        if slot.transport.is_usbfs() {
            via_usbfs = true;
        }
    }
    if via_usbfs {
        usb::usbfs_release(usb);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uevent_parses_id_and_iface() {
        let text = "\
HID_NAME=Valve Software Steam Controller
HID_ID=0003:000028DE:00001304
HID_PHYS=usb-0000:00:14.0-1/input2
";
        let info = parse_uevent_text(text).expect("uevent");
        assert_eq!(info.vid, 0x28DE);
        assert_eq!(info.pid, 0x1304);
        assert_eq!(info.iface, 2);
    }

    #[test]
    fn uevent_requires_hid_id() {
        assert!(parse_uevent_text("HID_PHYS=usb-1/input2\n").is_none());
        assert!(parse_uevent_text("HID_ID=bad\n").is_none());
        let info = parse_uevent_text("HID_ID=0003:000028DE:00001305\n").unwrap();
        assert_eq!(info.pid, 0x1305);
        assert_eq!(info.iface, -1);
        assert!(parse_uevent("no-such-hidraw").is_none());
        let _ = puck_hidraw_present();
    }

    #[test]
    fn close_all_clears_slots() {
        let (r, _w) = crate::test_env::nonblock_pipe();
        let mut slots = vec![Slot::new("pipe".into(), 2, Transport::Hidraw(r))];
        let mut grabs = Vec::new();
        let mut usb = None;
        close_all(&mut slots, &mut grabs, &mut usb);
        assert!(slots.is_empty());
        let mut slots = vec![Slot::new("usb".into(), 2, Transport::Usbfs { ep_in: 0x83 })];
        close_all(&mut slots, &mut grabs, &mut usb);
        assert!(slots.is_empty());
    }

    #[test]
    fn scan_hidraw_does_not_panic() {
        let mut usb = None;
        let _ = scan_devices(false, Mode::Gamepad, &mut usb);
        let _ = scan_devices(false, Mode::Lizard, &mut usb);
    }
}

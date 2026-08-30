use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::fd::AsFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use puckctl_protocol::{VALVE_VID, is_puck_pid};

use crate::hid::Transport;
use crate::log::logln;
use crate::scan::{parse_uevent, puck_hidraw_present};
use crate::slot::{MAX_SLOTS, Slot};
use crate::sys;
use crate::urb::{self, InUrb};

pub struct UsbDevice {
    pub fd: File,
    pub claimed: Vec<u32>,
    pub async_in: bool,
    ins: Vec<InUrb>,
}

// SAFETY: URB pointers only refer to buffers owned by the same `UsbDevice`.
unsafe impl Send for UsbDevice {}

#[allow(dead_code)]
impl UsbDevice {
    pub(crate) fn new(fd: File, claimed: Vec<u32>) -> Self {
        Self {
            fd,
            claimed,
            async_in: false,
            ins: Vec::new(),
        }
    }

    pub fn start_reads(&mut self, slots: &[Slot]) -> bool {
        self.stop_reads();
        for (i, slot) in slots.iter().enumerate() {
            let Transport::Usbfs { ep_in, .. } = slot.transport else {
                continue;
            };
            self.ins.push(InUrb::new(i, ep_in));
        }
        for pending in &mut self.ins {
            if pending.submit(self.fd.as_fd()).is_err() {
                self.stop_reads();
                return false;
            }
        }
        self.async_in = !self.ins.is_empty();
        self.async_in
    }

    pub fn stop_reads(&mut self) {
        for pending in &mut self.ins {
            pending.discard(self.fd.as_fd());
        }
        while sys::usb_reap_urb_ndelay(self.fd.as_fd()).is_ok() {}
        self.ins.clear();
        self.async_in = false;
    }

    pub fn reap_reports(&mut self) -> Vec<(usize, [u8; 128], usize)> {
        let mut out = Vec::new();
        while let Ok(ptr) = sys::usb_reap_urb_ndelay(self.fd.as_fd()) {
            let urb = unsafe { &*ptr };
            let slot = urb::slot_of(urb);
            let report = urb::copy_report(urb);
            if let Some(pending) = self.ins.iter_mut().find(|u| u.slot == slot) {
                pending.submitted = false;
                if let Some(rep) = report {
                    out.push(rep);
                    if pending.submit(self.fd.as_fd()).is_err() {
                        pending.note_idle();
                    }
                } else {
                    pending.note_idle();
                }
            }
        }
        self.submit_due();
        out
    }

    pub fn submit_due(&mut self) {
        for pending in &mut self.ins {
            if pending.due() && pending.submit(self.fd.as_fd()).is_err() {
                pending.note_idle();
            }
        }
    }
}

/// Claim then release so Steam's hidraw fds die and the hid driver rebinds.
pub fn kick_hid_drivers() {
    if !crate::hw::allowed() {
        return;
    }
    match scan_devices_usbfs() {
        Ok((_, mut held)) if held.is_some() => {
            logln("Steam override: kicked USB hid so Steam drops the device");
            usbfs_release(&mut held);
        }
        Ok(_) => {}
        Err(err) => logln(format!("Steam override: hid kick failed: {err}")),
    }
}

pub fn usbfs_release(usb: &mut Option<UsbDevice>) {
    let Some(mut dev) = usb.take() else {
        return;
    };
    dev.stop_reads();
    for iface in &dev.claimed {
        let _ = sys::usb_release(dev.fd.as_fd(), *iface);
    }
    drop(dev);
    let _ = restore_hid_drivers();
}

pub fn restore_hid_drivers() -> bool {
    if !crate::hw::allowed() {
        return crate::scan::puck_hidraw_present();
    }
    thread::sleep(Duration::from_millis(250));
    if puck_hidraw_present() {
        return true;
    }
    let Some((_sys, node)) = find_puck_usb_sysfs() else {
        return false;
    };
    let Ok(fd) = OpenOptions::new().read(true).write(true).open(&node) else {
        return false;
    };
    let mut n = 0;
    for iface in 2..=6 {
        if sys::usb_connect_iface(fd.as_fd(), iface).is_ok() {
            n += 1;
        }
    }
    drop(fd);
    thread::sleep(Duration::from_millis(250));
    if puck_hidraw_present() {
        if n > 0 {
            logln(format!("rebound {n} HID interfaces on {node}"));
        }
        return true;
    }
    false
}

pub fn scan_devices_usbfs() -> io::Result<(Vec<Slot>, Option<UsbDevice>)> {
    scan_usbfs_with(
        find_puck_usb,
        |node| OpenOptions::new().read(true).write(true).open(node),
        |fd, iface| sys::usb_claim(fd, iface).is_ok(),
    )
}

/// Locate / open / claim are injected so tests can fake a Proteus dongle
/// without touching a real `/dev/bus/usb` node.
pub(crate) fn scan_usbfs_with(
    locate: impl FnOnce() -> Option<(PathBuf, String)>,
    open: impl FnOnce(&str) -> io::Result<File>,
    mut claim: impl FnMut(std::os::fd::BorrowedFd<'_>, u32) -> bool,
) -> io::Result<(Vec<Slot>, Option<UsbDevice>)> {
    let Some((usb_sys, node)) = locate() else {
        return Ok((Vec::new(), None));
    };

    let mut eps = [(0_u8, 0_u8); 4];
    for (i, ep) in eps.iter_mut().enumerate() {
        let iface = i32::try_from(i + 2).unwrap_or(2);
        *ep = find_hid_eps(&usb_sys, iface).unwrap_or((
            0x83 + u8::try_from(i).unwrap_or(0),
            0x02 + u8::try_from(i).unwrap_or(0),
        ));
    }

    let fd = match open(&node) {
        Ok(fd) => fd,
        Err(err) => {
            logln(format!("open {node} failed: {err}"));
            return Ok((Vec::new(), None));
        }
    };

    let mut claimed = Vec::new();
    for iface in 2..=6_u32 {
        if claim(fd.as_fd(), iface) {
            claimed.push(iface);
        } else {
            logln(format!(
                "claim usb iface {iface} failed: {}",
                io::Error::last_os_error()
            ));
        }
    }
    if claimed.is_empty() {
        return Ok((Vec::new(), None));
    }

    logln(format!(
        "claimed {node} exclusively ({} HID interfaces) — Steam cannot use it",
        claimed.len()
    ));

    let mut slots = Vec::new();
    for (i, (ep_in, ep_out)) in eps.into_iter().enumerate() {
        if slots.len() >= MAX_SLOTS {
            break;
        }
        let iface = i32::try_from(i + 2).unwrap_or(2);
        let path = format!("{node}:if{iface}");
        logln(format!(
            "slot {}: {path} (usbfs iface {iface} ep_in={ep_in:02x} ep_out={ep_out:02x})",
            slots.len()
        ));
        slots.push(Slot::new(path, iface, Transport::Usbfs { ep_in, ep_out }));
    }

    Ok((slots, Some(UsbDevice::new(fd, claimed))))
}

pub(crate) fn find_hid_eps(usb_sys: &Path, iface: i32) -> Option<(u8, u8)> {
    let base = usb_sys.file_name()?.to_string_lossy();
    let ifdir = usb_sys.join(format!("{base}:1.{iface}"));
    let mut ep_in = 0_u8;
    let mut ep_out = 0_u8;
    let entries = fs::read_dir(&ifdir).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with("ep_") {
            continue;
        }
        let addr = fs::read_to_string(entry.path().join("bEndpointAddress")).ok()?;
        let ep = u8::from_str_radix(addr.trim().trim_start_matches("0x"), 16).ok()?;
        if ep & 0x80 != 0 {
            ep_in = ep;
        } else {
            ep_out = ep;
        }
    }
    (ep_in != 0 && ep_out != 0).then_some((ep_in, ep_out))
}

pub(crate) fn find_puck_usb() -> Option<(PathBuf, String)> {
    find_puck_usb_via_hidraw().or_else(find_puck_usb_sysfs)
}

pub(crate) fn find_puck_usb_sysfs() -> Option<(PathBuf, String)> {
    find_puck_usb_in_bus(Path::new("/sys/bus/usb/devices"))
}

pub(crate) fn find_puck_usb_in_bus(bus: &Path) -> Option<(PathBuf, String)> {
    let dir = fs::read_dir(bus).ok()?;
    for entry in dir.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') || name.contains(':') {
            continue;
        }
        let path = entry.path();
        let vid = read_hex_u16(&path.join("idVendor"))?;
        let pid = read_hex_u16(&path.join("idProduct"))?;
        if vid != VALVE_VID || !is_puck_pid(pid) {
            continue;
        }
        let bus = read_u32(&path.join("busnum"))?;
        let dev = read_u32(&path.join("devnum"))?;
        if bus == 0 || dev == 0 {
            continue;
        }
        let node = format!("/dev/bus/usb/{bus:03}/{dev:03}");
        return Some((path, node));
    }
    None
}

pub(crate) fn find_puck_usb_via_hidraw() -> Option<(PathBuf, String)> {
    let dir = fs::read_dir("/sys/class/hidraw").ok()?;
    for entry in dir.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with("hidraw") {
            continue;
        }
        let Some(info) = parse_uevent(&name) else {
            continue;
        };
        if info.virtual_clone || info.vid != VALVE_VID || !is_puck_pid(info.pid) {
            continue;
        }
        let mut walk = PathBuf::from(format!("/sys/class/hidraw/{name}/device"));
        for _ in 0..12 {
            let bus = read_u32(&walk.join("busnum"));
            let dev = read_u32(&walk.join("devnum"));
            if let (Some(bus), Some(dev)) = (bus, dev)
                && bus != 0
                && dev != 0
            {
                let node = format!("/dev/bus/usb/{bus:03}/{dev:03}");
                return Some((walk, node));
            }
            walk = walk.join("..");
        }
    }
    None
}

pub(crate) fn read_hex_u16(path: &Path) -> Option<u16> {
    let s = fs::read_to_string(path).ok()?;
    u16::from_str_radix(s.trim(), 16).ok()
}

pub(crate) fn read_u32(path: &Path) -> Option<u32> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

pub fn open_rw_nonblock(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hid::Transport;
    use crate::slot::Slot;

    #[test]
    fn sysfs_helpers_and_eps() {
        crate::test_env::isolated(|root| {
            let usb_sys = root.join("1-4");
            std::fs::create_dir_all(usb_sys.join("1-4:1.2/ep_83")).unwrap();
            std::fs::create_dir_all(usb_sys.join("1-4:1.2/ep_02")).unwrap();
            std::fs::create_dir_all(usb_sys.join("1-4:1.2/other")).unwrap();
            crate::test_env::write(&usb_sys.join("1-4:1.2/ep_83/bEndpointAddress"), "0x83\n");
            crate::test_env::write(&usb_sys.join("1-4:1.2/ep_02/bEndpointAddress"), "0x02\n");
            crate::test_env::write(&usb_sys.join("idVendor"), "28de\n");
            crate::test_env::write(&usb_sys.join("busnum"), "3\n");
            assert_eq!(find_hid_eps(&usb_sys, 2), Some((0x83, 0x02)));
            assert!(find_hid_eps(&usb_sys, 9).is_none());
            std::fs::create_dir_all(usb_sys.join("1-4:1.3/ep_84")).unwrap();
            crate::test_env::write(&usb_sys.join("1-4:1.3/ep_84/bEndpointAddress"), "84\n");
            assert!(find_hid_eps(&usb_sys, 3).is_none());
            std::fs::create_dir_all(usb_sys.join("1-4:1.4/ep_03")).unwrap();
            crate::test_env::write(&usb_sys.join("1-4:1.4/ep_03/bEndpointAddress"), "nope\n");
            assert!(find_hid_eps(&usb_sys, 4).is_none());
            assert_eq!(read_hex_u16(&usb_sys.join("idVendor")), Some(0x28de));
            assert!(read_hex_u16(&usb_sys.join("missing")).is_none());
            assert_eq!(read_u32(&usb_sys.join("busnum")), Some(3));
            assert!(read_u32(&usb_sys.join("missing")).is_none());
        });
    }

    fn fake_puck_bus(root: &Path) -> PathBuf {
        let bus = root.join("bus");
        let dev = bus.join("1-4");
        std::fs::create_dir_all(bus.join("1-4:1.0")).unwrap();
        std::fs::create_dir_all(bus.join(".skip")).unwrap();
        std::fs::create_dir_all(&dev).unwrap();
        crate::test_env::write(&dev.join("idVendor"), "28de\n");
        crate::test_env::write(&dev.join("idProduct"), "1304\n");
        crate::test_env::write(&dev.join("busnum"), "1\n");
        crate::test_env::write(&dev.join("devnum"), "5\n");
        for iface in 2..=5 {
            let ifdir = dev.join(format!("1-4:1.{iface}"));
            std::fs::create_dir_all(ifdir.join(format!("ep_{:02x}", 0x80 + iface))).unwrap();
            std::fs::create_dir_all(ifdir.join("ep_02")).unwrap();
            crate::test_env::write(
                &ifdir.join(format!("ep_{:02x}/bEndpointAddress", 0x80 + iface)),
                &format!("0x{:02x}\n", 0x80 + iface),
            );
            crate::test_env::write(&ifdir.join("ep_02/bEndpointAddress"), "0x02\n");
        }
        let other = bus.join("2-1");
        std::fs::create_dir_all(&other).unwrap();
        crate::test_env::write(&other.join("idVendor"), "045e\n");
        crate::test_env::write(&other.join("idProduct"), "028e\n");
        bus
    }

    #[test]
    fn finds_puck_in_fake_sysfs_bus() {
        crate::test_env::isolated(|root| {
            let bus = fake_puck_bus(root);
            let (path, node) = find_puck_usb_in_bus(&bus).expect("puck");
            assert!(path.ends_with("1-4"));
            assert_eq!(node, "/dev/bus/usb/001/005");
            assert!(find_puck_usb_in_bus(&root.join("missing")).is_none());
            crate::test_env::write(&path.join("busnum"), "0\n");
            assert!(find_puck_usb_in_bus(&bus).is_none());
        });
    }

    #[test]
    fn scan_usbfs_mocks_the_dongle() {
        crate::test_env::isolated(|root| {
            let bus = fake_puck_bus(root);
            let node = root.join("devnode");
            crate::test_env::write(&node, "");
            let empty = scan_usbfs_with(|| None, |_| unreachable!(), |_, _| true).unwrap();
            assert!(empty.0.is_empty());
            let opened = scan_usbfs_with(
                || Some((bus.join("1-4"), node.to_string_lossy().into())),
                |path| File::open(path),
                |_, iface| iface <= 5,
            )
            .unwrap();
            assert_eq!(opened.0.len(), 4);
            assert!(opened.0.iter().all(|s| s.transport.is_usbfs()));
            assert_eq!(opened.1.as_ref().unwrap().claimed, vec![2, 3, 4, 5]);
            let no_claim = scan_usbfs_with(
                || Some((bus.join("1-4"), node.to_string_lossy().into())),
                |path| File::open(path),
                |_, _| false,
            )
            .unwrap();
            assert!(no_claim.0.is_empty());
            let missing = scan_usbfs_with(
                || Some((bus.join("1-4"), root.join("nope").to_string_lossy().into())),
                |path| File::open(path),
                |_, _| true,
            )
            .unwrap();
            assert!(missing.0.is_empty());
        });
    }

    #[test]
    fn device_methods_and_release() {
        let file = File::open("/dev/null").unwrap();
        let mut dev = UsbDevice::new(file, vec![2, 3]);
        let slots = [
            Slot::new(
                "u".into(),
                2,
                Transport::Usbfs {
                    ep_in: 0x83,
                    ep_out: 0x02,
                },
            ),
            Slot::new(
                "h".into(),
                3,
                Transport::Hidraw(File::open("/dev/null").unwrap()),
            ),
        ];
        assert!(!dev.start_reads(&slots));
        assert!(!dev.async_in);
        let _ = dev.reap_reports();
        dev.submit_due();
        dev.stop_reads();
        let mut opt = Some(dev);
        usbfs_release(&mut opt);
        assert!(opt.is_none());
        usbfs_release(&mut opt);
        let _ = restore_hid_drivers();
        kick_hid_drivers();
        let _ = find_puck_usb();
        let _ = find_puck_usb_sysfs();
        let _ = find_puck_usb_via_hidraw();
        crate::test_env::isolated(|root| {
            let path = root.join("run/rw");
            crate::test_env::write(&path, "");
            assert!(open_rw_nonblock(&path).is_ok());
        });
    }
}

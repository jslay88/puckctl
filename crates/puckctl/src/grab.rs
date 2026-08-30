use std::fs::{self, File, OpenOptions};
use std::os::fd::AsFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use puckctl_protocol::VALVE_VID;

use crate::log::logln;
use crate::sys;

const MAX_GRABS: usize = 16;

#[must_use]
pub(crate) fn is_lizard_node(vendor: Option<u16>, name: &str) -> bool {
    vendor == Some(VALVE_VID) && (name.contains("Keyboard") || name.contains("Mouse"))
}

pub fn ungrab(grabs: &mut Vec<File>) {
    for fd in grabs.drain(..) {
        let _ = sys::eviocgrab(fd.as_fd(), false);
    }
}

pub fn grab_lizard_inputs(grabs: &mut Vec<File>) {
    if !crate::hw::allowed() || !grabs.is_empty() {
        return;
    }
    grab_lizard_from(
        Path::new("/sys/class/input"),
        Path::new("/dev/input"),
        grabs,
        |file| sys::eviocgrab(file.as_fd(), true).is_ok(),
    );
}

pub(crate) fn grab_lizard_from(
    class: &Path,
    devdir: &Path,
    grabs: &mut Vec<File>,
    mut try_grab: impl FnMut(&File) -> bool,
) {
    let Ok(dir) = fs::read_dir(class) else {
        return;
    };
    for entry in dir.flatten() {
        if grabs.len() >= MAX_GRABS {
            break;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with("event") {
            continue;
        }
        let node = class.join(&*name);
        let vendor = fs::read_to_string(node.join("device/id/vendor"))
            .ok()
            .and_then(|s| u16::from_str_radix(s.trim(), 16).ok());
        let dev_name = fs::read_to_string(node.join("device/name")).unwrap_or_default();
        if !is_lizard_node(vendor, &dev_name) {
            continue;
        }
        let path = devdir.join(&*name);
        let Ok(file) = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(&path)
        else {
            continue;
        };
        if !try_grab(&file) {
            continue;
        }
        grabs.push(file);
    }
    if !grabs.is_empty() {
        logln(format!(
            "grabbed {} lizard keyboard/mouse nodes",
            grabs.len()
        ));
    }
}

pub fn drain_grab(file: &File) {
    let mut junk = [0_u8; 64];
    loop {
        let n = unsafe {
            libc::read(
                std::os::fd::AsRawFd::as_raw_fd(file),
                junk.as_mut_ptr().cast(),
                junk.len(),
            )
        };
        if n <= 0 {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn lizard_node_filter() {
        assert!(is_lizard_node(Some(VALVE_VID), "Steam Controller Keyboard"));
        assert!(is_lizard_node(Some(VALVE_VID), "Steam Controller Mouse"));
        assert!(!is_lizard_node(Some(VALVE_VID), "Steam Controller"));
        assert!(!is_lizard_node(Some(0x045e), "Keyboard"));
        assert!(!is_lizard_node(None, "Keyboard"));
    }

    #[test]
    fn ungrab_and_drain() {
        let mut grabs = Vec::new();
        ungrab(&mut grabs);
        grab_lizard_inputs(&mut grabs);
        assert!(grabs.is_empty());
        let (r, mut w) = crate::test_env::nonblock_pipe();
        w.write_all(&[1, 2, 3, 4]).unwrap();
        drain_grab(&r);
        drain_grab(&r);
        let _ = r;
    }

    fn event_node(class: &Path, dev: &Path, name: &str, vendor: &str, node_name: &str) {
        let dir = class.join(name).join("device/id");
        std::fs::create_dir_all(&dir).unwrap();
        crate::test_env::write(&dir.join("vendor"), vendor);
        crate::test_env::write(&class.join(name).join("device/name"), node_name);
        crate::test_env::write(&dev.join(name), "");
    }

    #[test]
    fn grab_from_fake_sysfs() {
        crate::test_env::isolated(|root| {
            let class = root.join("class");
            let dev = root.join("dev");
            std::fs::create_dir_all(&dev).unwrap();
            event_node(
                &class,
                &dev,
                "event0",
                "28de\n",
                "Steam Controller Keyboard\n",
            );
            event_node(&class, &dev, "event1", "28de\n", "Steam Controller Mouse\n");
            event_node(&class, &dev, "event2", "045e\n", "Keyboard\n");
            std::fs::create_dir_all(class.join("mice")).unwrap();
            event_node(
                &class,
                &dev,
                "event99",
                "28de\n",
                "Steam Controller Keyboard\n",
            );
            std::fs::remove_file(dev.join("event99")).unwrap();

            let mut grabs = Vec::new();
            grab_lizard_from(&root.join("missing"), &dev, &mut grabs, |_| true);
            assert!(grabs.is_empty());
            grab_lizard_from(&class, &dev, &mut grabs, |_| false);
            assert!(grabs.is_empty());

            let mut all = Vec::new();
            grab_lizard_from(&class, &dev, &mut all, |_| true);
            assert_eq!(all.len(), 2);
            ungrab(&mut all);
            assert!(all.is_empty());

            let mut capped = Vec::new();
            for i in 0..MAX_GRABS + 2 {
                event_node(
                    &class,
                    &dev,
                    &format!("event{i}"),
                    "28de\n",
                    "Steam Controller Keyboard\n",
                );
            }
            grab_lizard_from(&class, &dev, &mut capped, |_| true);
            assert_eq!(capped.len(), MAX_GRABS);
        });
    }
}

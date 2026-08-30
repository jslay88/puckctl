use std::io::ErrorKind;
use std::os::fd::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::control::{self, CommandKind, MAX_CLIENTS};
use crate::daemon::Daemon;
use crate::grab;
use crate::log::logln;
use crate::pad;
use crate::scan;
use crate::steam_cfg;
use crate::sys;

enum PollTag {
    Listen,
    Client(usize),
    Hid(usize),
    Uhid(usize),
    Grab(usize),
}

impl Daemon {
    pub fn run(mut self, listener: UnixListener) -> i32 {
        if self.override_steam {
            steam_cfg::hide_steam_desktop_config(true);
            self.steam_seen = crate::steam::steam_is_running();
        } else {
            steam_cfg::hide_steam_desktop_config(false);
        }
        logln(format!(
            "puckctl starting{} (mode={})",
            if self.dump { " (dump mode)" } else { "" },
            self.requested.name()
        ));

        let mut clients: Vec<Option<UnixStream>> = (0..MAX_CLIENTS).map(|_| None).collect();
        install_signals();

        while self.running && still_running() {
            self.steam_tick();

            if !self.paused && self.slots.is_empty() && crate::hw::allowed() {
                self.slots = scan::scan_devices(false, self.requested, &mut self.usb);
                if !self.slots.is_empty() {
                    if self.dump {
                        self.lizard_all(false);
                    } else {
                        self.apply_requested_mode();
                    }
                }
            }

            let mut pfds = Vec::new();
            let mut tags = Vec::new();
            pfds.push(libc::pollfd {
                fd: listener.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            });
            tags.push(PollTag::Listen);
            for (i, client) in clients.iter().enumerate() {
                if let Some(stream) = client {
                    pfds.push(libc::pollfd {
                        fd: stream.as_raw_fd(),
                        events: libc::POLLIN,
                        revents: 0,
                    });
                    tags.push(PollTag::Client(i));
                }
            }
            if !self.paused {
                for (i, slot) in self.slots.iter().enumerate() {
                    if let Some(fd) = slot.transport.hidraw_raw() {
                        pfds.push(libc::pollfd {
                            fd,
                            events: libc::POLLIN,
                            revents: 0,
                        });
                        tags.push(PollTag::Hid(i));
                    }
                    if let Some(uhid) = &slot.uhid {
                        pfds.push(libc::pollfd {
                            fd: uhid.raw_fd(),
                            events: libc::POLLIN,
                            revents: 0,
                        });
                        tags.push(PollTag::Uhid(i));
                    }
                }
                for (i, grab) in self.grabs.iter().enumerate() {
                    pfds.push(libc::pollfd {
                        fd: grab.as_raw_fd(),
                        events: libc::POLLIN,
                        revents: 0,
                    });
                    tags.push(PollTag::Grab(i));
                }
            }

            let timeout = if self.slots.iter().any(|slot| slot.transport.is_usbfs()) {
                1
            } else {
                50
            };
            match sys::poll(&mut pfds, timeout) {
                Err(err) if err.kind() == ErrorKind::Interrupted => continue,
                Err(err) => {
                    logln(format!("poll: {err}"));
                    break;
                }
                Ok(_) => {}
            }

            let mut device_lost = false;
            for (pfd, tag) in pfds.iter().zip(tags.iter()) {
                if pfd.revents & (libc::POLLIN | libc::POLLERR | libc::POLLHUP) == 0 {
                    continue;
                }
                match *tag {
                    PollTag::Listen => {
                        if let Ok((stream, _)) = listener.accept() {
                            if let Some(slot) = clients.iter_mut().find(|c| c.is_none()) {
                                let _ = stream.set_nonblocking(true);
                                *slot = Some(stream);
                            }
                        }
                    }
                    PollTag::Client(i) => {
                        if let Some(mut stream) = clients[i].take() {
                            let cmd =
                                control::read_command(&mut stream).unwrap_or(CommandKind::Unknown);
                            let reply = self.handle_command(cmd);
                            control::write_reply(&mut stream, &reply);
                        }
                    }
                    PollTag::Grab(i) => {
                        if let Some(file) = self.grabs.get(i) {
                            grab::drain_grab(file);
                        }
                    }
                    PollTag::Hid(i) => {
                        if self.read_hid(i) {
                            device_lost = true;
                        }
                    }
                    PollTag::Uhid(i) => self.pump_uhid(i),
                }
            }

            if self.read_usbfs_slots() {
                device_lost = true;
            }

            self.lizard_heartbeat();

            if device_lost {
                self.close_all();
            }
        }

        if !self.paused && !self.slots.is_empty() {
            self.lizard_all(true);
        }
        self.close_all();
        control::unlink_socket();
        logln("puckctl stopping");
        0
    }

    fn read_usbfs_slots(&mut self) -> bool {
        let probe = self.last_usbfs_probe.elapsed() >= std::time::Duration::from_millis(50);
        if probe {
            self.last_usbfs_probe = std::time::Instant::now();
        }
        let mut i = 0;
        while i < self.slots.len() {
            let usbfs = self.slots[i].transport.is_usbfs();
            let connected = self.slots[i].connected;
            if usbfs && (connected || probe) && self.read_usbfs(i) {
                return true;
            }
            i += 1;
        }
        false
    }

    fn read_hid(&mut self, i: usize) -> bool {
        if self.slots.get(i).is_none() {
            return false;
        }
        let mut buf = [0_u8; 128];
        loop {
            let n = {
                let slot = &self.slots[i];
                match slot.read_report(self.usb.as_ref(), &mut buf) {
                    Ok(0) => return false,
                    Ok(n) => n,
                    Err(err) if err.kind() == ErrorKind::WouldBlock => return false,
                    Err(err) => {
                        logln(format!(
                            "read {}: {err} — device lost, rescanning",
                            self.slots[i].path
                        ));
                        return true;
                    }
                }
            };
            let requested = self.requested;
            let paused = self.paused;
            let dump = self.dump;
            let usb = self.usb.as_ref();
            let prev = self.slots[i].last_buttons;
            pad::ingest_report(&mut self.slots[i], &buf[..n], requested, paused, dump, usb);
            let now = self.slots[i].last_buttons;
            if self.consider_combo(prev, now) {
                return false;
            }
        }
    }

    fn read_usbfs(&mut self, i: usize) -> bool {
        if self.slots.get(i).is_none() {
            return false;
        }
        let mut buf = [0_u8; 128];
        let mut n = match self.usbfs_recv(i, &mut buf) {
            Some(Err(())) => return true,
            None | Some(Ok(0)) => return false,
            Some(Ok(n)) => n,
        };
        loop {
            let prev = self.slots[i].last_buttons;
            pad::ingest_report(
                &mut self.slots[i],
                &buf[..n],
                self.requested,
                self.paused,
                self.dump,
                self.usb.as_ref(),
            );
            let now = self.slots[i].last_buttons;
            if self.consider_combo(prev, now) {
                return false;
            }
            match self.usbfs_recv(i, &mut buf) {
                None | Some(Ok(0)) => return false,
                Some(Ok(more)) => n = more,
                Some(Err(())) => return true,
            }
        }
    }

    fn pump_uhid(&mut self, i: usize) {
        let Some(slot) = self.slots.get(i) else {
            return;
        };
        let Some(uhid) = slot.uhid.as_ref() else {
            return;
        };
        let iface = slot.iface;
        let ep_out = slot.transport.usbfs_ep_out().unwrap_or(0);
        uhid.pump(self.usb.as_ref(), iface, ep_out);
    }

    fn usbfs_recv(&self, i: usize, buf: &mut [u8]) -> Option<Result<usize, ()>> {
        let slot = self.slots.get(i)?;
        match slot.read_report(self.usb.as_ref(), buf) {
            Ok(n) => Some(Ok(n)),
            Err(err) if err.kind() == ErrorKind::WouldBlock => None,
            Err(err) => match err.raw_os_error() {
                Some(libc::EPIPE | libc::ENODEV | libc::ECONNRESET) if !slot.connected => None,
                _ => {
                    logln(format!(
                        "usbfs read {}: {err} — device lost, rescanning",
                        slot.path
                    ));
                    Some(Err(()))
                }
            },
        }
    }
}

fn install_signals() {
    // SAFETY: handler only stores false in RUNNING.
    unsafe {
        libc::signal(libc::SIGINT, handle_signal as *const () as usize);
        libc::signal(libc::SIGTERM, handle_signal as *const () as usize);
        libc::signal(libc::SIGHUP, handle_signal as *const () as usize);
    }
}

static RUNNING: AtomicBool = AtomicBool::new(true);

extern "C" fn handle_signal(_: libc::c_int) {
    RUNNING.store(false, Ordering::Relaxed);
}

fn still_running() -> bool {
    RUNNING.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control;
    use crate::daemon::Daemon;
    use crate::hid::Transport;
    use crate::paths::socket_path;
    use crate::slot::Slot;
    use puckctl_protocol::{BTN_A, BTN_STEAM, ID_CONTROLLER_STATE, OFF_BUTTONS, STATE_MIN_LEN};
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;
    use std::thread;

    fn state_bytes(buttons: u32) -> Vec<u8> {
        let mut d = vec![0_u8; STATE_MIN_LEN];
        d[0] = ID_CONTROLLER_STATE;
        d[OFF_BUTTONS..OFF_BUTTONS + 4].copy_from_slice(&buttons.to_le_bytes());
        d
    }

    #[test]
    fn signal_flag_and_direct_reads() {
        assert!(still_running());
        handle_signal(libc::SIGTERM);
        assert!(!still_running());
        RUNNING.store(true, Ordering::Relaxed);

        crate::test_env::isolated(|_| {
            let mut d = Daemon::new(true, true);
            assert!(!d.read_hid(3));
            assert!(!d.read_usbfs(3));
            assert!(d.usbfs_recv(3, &mut [0; 8]).is_none());
            d.pump_uhid(0);
            d.pump_uhid(99);

            let (r, _w) = crate::test_env::nonblock_pipe();
            d.slots
                .push(Slot::new("hid".into(), 2, Transport::Hidraw(r)));
            assert!(!d.read_hid(0));

            d.slots.push(Slot::new(
                "usb".into(),
                3,
                Transport::Usbfs {
                    ep_in: 0x83,
                    ep_out: 0x03,
                },
            ));
            assert!(d.read_usbfs(1));
            d.slots[1].connected = false;
            assert!(d.read_usbfs_slots());

            let path = crate::test_env::temp_file(
                &std::env::var_os("XDG_RUNTIME_DIR")
                    .map(std::path::PathBuf::from)
                    .unwrap(),
                "wronly",
            );
            let write_only = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
            d.slots[0].transport = Transport::Hidraw(write_only);
            assert!(d.read_hid(0));
        });
    }

    #[test]
    fn run_loop_quit_and_input() {
        crate::test_env::isolated(|_| {
            RUNNING.store(true, Ordering::Relaxed);
            let listener = control::open_listen_socket().expect("listen");
            let (hid_r, mut hid_w) = crate::test_env::nonblock_pipe();
            let (grab_r, mut grab_w) = crate::test_env::nonblock_pipe();
            let mut d = Daemon::new(false, false);
            d.dump = true;
            d.slots
                .push(Slot::new("hid".into(), 2, Transport::Hidraw(hid_r)));
            d.slots.push(Slot::new(
                "usb".into(),
                3,
                Transport::Usbfs {
                    ep_in: 0x83,
                    ep_out: 0x03,
                },
            ));
            d.grabs.push(grab_r);
            hid_w.write_all(&state_bytes(BTN_A)).unwrap();
            grab_w.write_all(&[1, 2, 3, 4]).unwrap();
            let thread = thread::spawn(move || d.run(listener));
            let timeout = std::time::Duration::from_secs(2);
            let mut client = UnixStream::connect(socket_path()).expect("client");
            client.set_read_timeout(Some(timeout)).unwrap();
            client.set_write_timeout(Some(timeout)).unwrap();
            client.write_all(b"status\n").unwrap();
            let mut reply = String::new();
            let _ = client.read_to_string(&mut reply);
            let mut client = UnixStream::connect(socket_path()).expect("quit");
            client.set_read_timeout(Some(timeout)).unwrap();
            client.set_write_timeout(Some(timeout)).unwrap();
            client.write_all(b"quit\n").unwrap();
            let _ = client.read_to_string(&mut String::new());
            let code = thread.join().expect("join");
            assert_eq!(code, 0);
            RUNNING.store(true, Ordering::Relaxed);
        });
    }

    #[test]
    fn hid_combo_stops_read() {
        crate::test_env::isolated(|_| {
            let (r, mut w) = crate::test_env::nonblock_pipe();
            let mut d = Daemon::new(true, true);
            d.arm_combo(BTN_A | BTN_STEAM);
            d.slots
                .push(Slot::new("hid".into(), 2, Transport::Hidraw(r)));
            w.write_all(&state_bytes(BTN_A | BTN_STEAM)).unwrap();
            assert!(!d.read_hid(0));
            assert_eq!(d.requested, crate::mode::Mode::Lizard);
        });
    }
}

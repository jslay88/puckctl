use std::fs::File;
use std::time::{Duration, Instant};

use crate::combo;
use crate::control::{self, CommandKind};
use crate::grab;
use crate::log::logln;
use crate::mode::{self, Mode};
use crate::pad;
use crate::scan;
use crate::slot::Slot;
use crate::steam;
use crate::usb::UsbDevice;

pub struct Daemon {
    pub slots: Vec<Slot>,
    pub usb: Option<UsbDevice>,
    pub grabs: Vec<File>,
    pub requested: Mode,
    pub override_steam: bool,
    pub paused: bool,
    pub steam_seen: bool,
    pub dump: bool,
    pub running: bool,
    pub(crate) last_usbfs_probe: Instant,
    combo: u32,
    combo_armed: bool,
    combo_watch: Option<Instant>,
    last_steam_check: Instant,
}

impl Daemon {
    #[must_use]
    pub fn new(dump: bool, steam_check: bool) -> Self {
        let mut override_steam = false;
        let mut requested = Mode::Gamepad;
        if !dump {
            requested = mode::load_requested_mode();
            override_steam = steam::load_override();
            if !steam_check {
                override_steam = true;
            }
        }
        Self {
            slots: Vec::new(),
            usb: None,
            grabs: Vec::new(),
            requested,
            override_steam,
            paused: false,
            steam_seen: false,
            dump,
            running: true,
            combo: combo::load(),
            combo_armed: false,
            combo_watch: None,
            last_steam_check: Instant::now() - Duration::from_secs(3),
            last_usbfs_probe: Instant::now() - Duration::from_secs(1),
        }
    }

    #[cfg(test)]
    pub(crate) fn arm_combo(&mut self, mask: u32) {
        self.dump = false;
        self.paused = false;
        self.requested = Mode::Gamepad;
        self.combo = mask;
        self.combo_armed = true;
        self.combo_watch = None;
    }

    fn effective_name(&self) -> &'static str {
        if self.paused {
            "steam"
        } else {
            self.requested.name()
        }
    }

    fn any_connected(&self) -> bool {
        self.slots.iter().any(|slot| slot.connected)
    }

    fn status_line(&self) -> String {
        control::format_status(
            self.effective_name(),
            self.requested,
            steam::steam_is_running(),
            self.override_steam,
            self.any_connected(),
            self.combo,
        )
    }

    fn current_buttons(&self) -> u32 {
        self.slots
            .iter()
            .filter(|slot| slot.connected)
            .fold(0, |acc, slot| acc | slot.last_buttons)
    }

    fn buttons_line(&mut self) -> String {
        self.combo_watch = Some(Instant::now());
        format!(
            "OK buttons={:x} connected={} combo={:x}\n",
            self.current_buttons(),
            u8::from(self.any_connected()),
            self.combo
        )
    }

    fn combo_line(&self) -> String {
        format!("OK combo={:x}\n", self.combo)
    }

    fn set_combo(&mut self, mask: u32) -> String {
        self.combo = puckctl_protocol::sanitize_buttons(mask);
        self.combo_armed = false;
        combo::save(self.combo);
        logln(format!(
            "combo set to {:#x} ({})",
            self.combo,
            puckctl_protocol::format_buttons(self.combo)
        ));
        self.combo_line()
    }

    fn combo_watching(&self) -> bool {
        self.combo_watch
            .is_some_and(|t| t.elapsed() < Duration::from_secs(2))
    }

    pub(crate) fn consider_combo(&mut self, prev: u32, now: u32) -> bool {
        let now = puckctl_protocol::sanitize_buttons(now);
        if self.combo == 0 || now != self.combo {
            self.combo_armed = true;
            return false;
        }
        if self.dump || self.paused || self.requested != Mode::Gamepad || self.combo_watching() {
            return false;
        }
        if !self.combo_armed || !combo::triggered(prev, now, self.combo) {
            return false;
        }
        self.combo_armed = false;
        logln(format!(
            "combo {} — switching to lizard",
            puckctl_protocol::format_buttons(self.combo)
        ));
        self.set_requested(Mode::Lizard);
        true
    }

    pub(crate) fn close_all(&mut self) {
        scan::close_all(&mut self.slots, &mut self.grabs, &mut self.usb);
    }

    pub(crate) fn claim_from_steam(&self) -> bool {
        self.override_steam && self.steam_seen
    }

    pub(crate) fn apply_requested_mode(&mut self) {
        if self.paused || self.dump {
            return;
        }
        if self.requested == Mode::Gamepad {
            let claim = self.claim_from_steam();
            if claim && (self.slots.is_empty() || !self.slots[0].transport.is_usbfs()) {
                self.close_all();
                self.slots = scan::scan_devices(claim, self.requested, &mut self.usb);
            }
            self.lizard_all(false);
            grab::grab_lizard_inputs(&mut self.grabs);
            let hidraw_only = self.slots.iter().any(|slot| !slot.transport.is_usbfs());
            for slot in &mut self.slots {
                if !slot.connected || slot.pad.is_some() || !slot.transport.is_usbfs() {
                    continue;
                }
                if let Ok(pad) = pad::create_uinput() {
                    slot.last_buttons = 0;
                    slot.last_abs = [0; 6];
                    slot.last_hat = [0; 2];
                    logln(format!(
                        "virtual Steam Controller pad created for {}",
                        slot.path
                    ));
                    slot.pad = Some(pad);
                }
            }
            if hidraw_only {
                logln("mode: gamepad (hidraw, SDL keeps gyro)");
            } else {
                logln("mode: gamepad");
            }
        } else {
            for slot in &mut self.slots {
                pad::destroy_pad(slot);
            }
            grab::ungrab(&mut self.grabs);
            self.lizard_all(true);
            if self.slots.first().is_some_and(|s| s.transport.is_usbfs()) {
                logln("releasing USB claim so lizard keyboard/mouse can bind");
                self.close_all();
            }
            logln("mode: lizard");
        }
    }

    pub(crate) fn lizard_all(&mut self, enable: bool) {
        let usb = self.usb.as_ref();
        for slot in &mut self.slots {
            if enable {
                slot.send_lizard_on(usb);
            } else {
                slot.send_lizard_off(usb);
            }
        }
    }

    fn set_requested(&mut self, next: Mode) {
        self.requested = next;
        mode::save_requested_mode(next);
        self.apply_requested_mode();
    }

    fn set_override(&mut self, on: bool) {
        let change = steam::set_override(on);
        self.override_steam = change.override_steam;
        self.paused = change.paused;
        self.steam_seen = change.steam_seen;
        self.close_all();
    }

    pub(crate) fn handle_command(&mut self, cmd: CommandKind) -> String {
        match cmd {
            CommandKind::Status => self.status_line(),
            CommandKind::Gamepad => {
                self.set_requested(Mode::Gamepad);
                self.status_line()
            }
            CommandKind::Lizard => {
                self.set_requested(Mode::Lizard);
                self.status_line()
            }
            CommandKind::Toggle => {
                self.set_requested(self.requested.toggle());
                self.status_line()
            }
            CommandKind::OverrideOn => {
                self.set_override(true);
                self.status_line()
            }
            CommandKind::OverrideOff => {
                self.set_override(false);
                self.status_line()
            }
            CommandKind::OverrideToggle => {
                self.set_override(!self.override_steam);
                self.status_line()
            }
            CommandKind::Buttons => self.buttons_line(),
            CommandKind::Combo => self.combo_line(),
            CommandKind::ComboSet(mask) => self.set_combo(mask),
            CommandKind::Quit => {
                self.running = false;
                "OK quitting\n".into()
            }
            CommandKind::Unknown => "ERR unknown command\n".into(),
        }
    }

    pub(crate) fn steam_tick(&mut self) {
        if self.dump || self.last_steam_check.elapsed() <= Duration::from_millis(2000) {
            return;
        }
        self.last_steam_check = Instant::now();
        self.steam_seen = steam::steam_is_running();
        if self.override_steam {
            let want_usbfs = self.steam_seen && self.requested == Mode::Gamepad && !self.paused;
            let have_usbfs = self.slots.first().is_some_and(|s| s.transport.is_usbfs());
            if want_usbfs != have_usbfs && !self.slots.is_empty() {
                if want_usbfs {
                    logln("Steam is running — claiming USB so Steam cannot use the puck");
                } else {
                    logln("Steam exited — releasing USB so games can use hidraw gyro");
                }
                self.close_all();
            } else if self.requested == Mode::Gamepad && !self.paused {
                grab::grab_lizard_inputs(&mut self.grabs);
            }
        } else if self.steam_seen && !self.paused {
            logln("Steam client detected — releasing controller");
            self.close_all();
            self.paused = true;
        } else if !self.steam_seen && self.paused {
            logln("Steam exited — reclaiming controller");
            self.paused = false;
        }
    }

    pub(crate) fn lizard_heartbeat(&mut self) {
        if self.requested != Mode::Gamepad {
            return;
        }
        let override_steam = self.override_steam;
        let steam_seen = self.steam_seen;
        let usb = self.usb.as_ref();
        for slot in &mut self.slots {
            if slot.connected && slot.lizard_due(override_steam, steam_seen) {
                slot.send_lizard_off(usb);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hid::Transport;
    use puckctl_protocol::{BTN_A, BTN_STEAM};

    fn dump_daemon() -> Daemon {
        Daemon::new(true, true)
    }

    #[test]
    fn commands_and_combo() {
        crate::test_env::isolated(|_| {
            let mut d = dump_daemon();
            assert!(d.dump);
            assert!(!d.claim_from_steam());
            assert!(
                d.handle_command(CommandKind::Status)
                    .contains("effective=gamepad")
            );
            assert!(d.handle_command(CommandKind::Buttons).contains("buttons=0"));
            assert!(d.handle_command(CommandKind::Combo).contains("combo=0"));
            let combo = BTN_A | BTN_STEAM;
            assert!(
                d.handle_command(CommandKind::ComboSet(combo))
                    .contains("combo=")
            );
            assert_eq!(d.combo, combo);
            assert!(d.handle_command(CommandKind::Unknown).starts_with("ERR"));
            assert_eq!(d.handle_command(CommandKind::Quit), "OK quitting\n");
            assert!(!d.running);

            let mut d = dump_daemon();
            d.dump = false;
            d.requested = Mode::Gamepad;
            d.combo = combo;
            d.combo_armed = true;
            d.combo_watch = None;
            assert!(d.consider_combo(0, combo));
            assert_eq!(d.requested, Mode::Lizard);

            let mut d = dump_daemon();
            assert!(!d.consider_combo(0, combo));
            d.dump = false;
            d.combo = combo;
            d.combo_armed = false;
            assert!(!d.consider_combo(0, combo));
            d.combo_armed = true;
            d.paused = true;
            assert!(!d.consider_combo(0, combo));
            d.paused = false;
            d.requested = Mode::Lizard;
            assert!(!d.consider_combo(0, combo));
            d.requested = Mode::Gamepad;
            d.combo_watch = Some(Instant::now());
            assert!(d.combo_watching());
            assert!(!d.consider_combo(0, combo));
            d.combo = 0;
            assert!(!d.consider_combo(0, 1));
        });
    }

    #[test]
    fn mode_override_and_steam_tick() {
        crate::test_env::isolated(|_| {
            let mut d = Daemon::new(false, false);
            assert!(d.override_steam);
            assert!(
                d.handle_command(CommandKind::Lizard)
                    .contains("requested=lizard")
            );
            assert_eq!(d.requested, Mode::Lizard);
            let _ = d.handle_command(CommandKind::Gamepad);
            let _ = d.handle_command(CommandKind::Toggle);
            let _ = d.handle_command(CommandKind::OverrideOff);
            let _ = d.handle_command(CommandKind::OverrideOn);
            let _ = d.handle_command(CommandKind::OverrideToggle);
            d.paused = true;
            assert_eq!(d.effective_name(), "steam");
            d.paused = false;
            d.dump = true;
            d.steam_tick();
            d.dump = false;
            d.last_steam_check = Instant::now() - Duration::from_secs(3);
            d.override_steam = true;
            d.requested = Mode::Gamepad;
            d.paused = false;
            let (r, _w) = crate::test_env::nonblock_pipe();
            d.slots.push(Slot::new("h".into(), 2, Transport::Hidraw(r)));
            d.slots[0].connected = true;
            d.steam_tick();
            d.override_steam = false;
            d.paused = false;
            d.last_steam_check = Instant::now() - Duration::from_secs(3);
            d.steam_tick();
            d.paused = true;
            d.last_steam_check = Instant::now() - Duration::from_secs(3);
            d.steam_tick();
            d.lizard_heartbeat();
            d.requested = Mode::Lizard;
            d.lizard_heartbeat();
            d.requested = Mode::Gamepad;
            d.apply_requested_mode();
            d.dump = true;
            d.apply_requested_mode();
            d.dump = false;
            d.paused = true;
            d.apply_requested_mode();
            d.paused = false;
            d.requested = Mode::Lizard;
            d.slots.clear();
            d.slots
                .push(Slot::new("u".into(), 2, Transport::Usbfs { ep_in: 0x83 }));
            d.apply_requested_mode();
            d.close_all();
            let _ = d.any_connected();
            let _ = d.current_buttons();
            let _ = d.status_line();
        });
    }
}

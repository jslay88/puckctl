#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use std::fs::File;
use std::io;
use std::mem;
use std::os::fd::{AsFd, AsRawFd};
use std::path::Path;

use puckctl_protocol::{
    BUTTON_MAP, ControllerState, EV_BTN_A, EV_BTN_B, EV_BTN_MODE, EV_BTN_SELECT, EV_BTN_START,
    EV_BTN_THUMBL, EV_BTN_THUMBR, EV_BTN_TL, EV_BTN_TR, EV_BTN_TRIGGER_HAPPY1,
    EV_BTN_TRIGGER_HAPPY2, EV_BTN_TRIGGER_HAPPY3, EV_BTN_TRIGGER_HAPPY4, EV_BTN_TRIGGER_HAPPY5,
    EV_BTN_X, EV_BTN_Y, PID_PROTEUS_DONGLE, Report, VALVE_VID, WirelessStatus, classify,
};

use crate::linux::{
    ABS_HAT0X, ABS_HAT0Y, ABS_RX, ABS_RY, ABS_RZ, ABS_X, ABS_Y, ABS_Z, BUS_USB, EV_ABS, EV_KEY,
    EV_SYN, InputEvent, InputId, SYN_REPORT, UinputUserDev,
};
use crate::log::logln;
use crate::mode::Mode;
use crate::slot::Slot;
use crate::sys;
use crate::usb::{UsbDevice, open_rw_nonblock};

const UINPUT_NAME: &[u8] = b"Steam Controller";

#[derive(Debug)]
pub struct VirtualPad {
    pub fd: File,
}

fn steam_controller_uidev() -> UinputUserDev {
    let mut ud: UinputUserDev = unsafe { mem::zeroed() };
    for (i, b) in UINPUT_NAME.iter().enumerate() {
        ud.name[i] = *b as libc::c_char;
    }
    ud.id = InputId {
        bustype: BUS_USB,
        vendor: VALVE_VID,
        product: PID_PROTEUS_DONGLE,
        version: 0x0110,
    };
    ud.ff_effects_max = 0;
    for axis in [ABS_X, ABS_Y, ABS_RX, ABS_RY] {
        let i = axis as usize;
        ud.absmin[i] = -32768;
        ud.absmax[i] = 32767;
        ud.absfuzz[i] = 16;
        ud.absflat[i] = 128;
    }
    ud.absmin[ABS_Z as usize] = 0;
    ud.absmax[ABS_Z as usize] = 255;
    ud.absmin[ABS_RZ as usize] = 0;
    ud.absmax[ABS_RZ as usize] = 255;
    ud.absmin[ABS_HAT0X as usize] = -1;
    ud.absmax[ABS_HAT0X as usize] = 1;
    ud.absmin[ABS_HAT0Y as usize] = -1;
    ud.absmax[ABS_HAT0Y as usize] = 1;
    ud
}

pub fn create_uinput() -> io::Result<VirtualPad> {
    if !crate::hw::allowed() {
        return Err(io::Error::other("uinput disabled in tests"));
    }
    let fd = open_rw_nonblock(Path::new("/dev/uinput")).map_err(|err| {
        logln(format!(
            "open /dev/uinput failed: {err} (is the uaccess ACL present?)"
        ));
        err
    })?;
    let borrowed = fd.as_fd();
    sys::ui_set_evbit(borrowed, i32::from(EV_KEY))?;
    sys::ui_set_evbit(borrowed, i32::from(EV_ABS))?;
    sys::ui_set_evbit(borrowed, i32::from(EV_SYN))?;

    for key in [
        EV_BTN_A.0,
        EV_BTN_B.0,
        EV_BTN_X.0,
        EV_BTN_Y.0,
        EV_BTN_TL.0,
        EV_BTN_TR.0,
        EV_BTN_SELECT.0,
        EV_BTN_START.0,
        EV_BTN_MODE.0,
        EV_BTN_THUMBL.0,
        EV_BTN_THUMBR.0,
        EV_BTN_TRIGGER_HAPPY1.0,
        EV_BTN_TRIGGER_HAPPY2.0,
        EV_BTN_TRIGGER_HAPPY3.0,
        EV_BTN_TRIGGER_HAPPY4.0,
        EV_BTN_TRIGGER_HAPPY5.0,
    ] {
        sys::ui_set_keybit(borrowed, i32::from(key))?;
    }

    for axis in [
        ABS_X, ABS_Y, ABS_RX, ABS_RY, ABS_Z, ABS_RZ, ABS_HAT0X, ABS_HAT0Y,
    ] {
        sys::ui_set_absbit(borrowed, i32::from(axis))?;
    }
    let ud = steam_controller_uidev();

    let wrote = unsafe {
        libc::write(
            fd.as_raw_fd(),
            (&raw const ud).cast(),
            mem::size_of::<UinputUserDev>(),
        )
    };
    if wrote != mem::size_of::<UinputUserDev>() as isize || sys::ui_dev_create(borrowed).is_err() {
        logln("uinput device creation failed");
        return Err(io::Error::other("uinput device creation failed"));
    }
    Ok(VirtualPad { fd })
}

pub fn destroy_pad(slot: &mut Slot) {
    if let Some(pad) = slot.pad.take() {
        let _ = sys::ui_dev_destroy(pad.fd.as_fd());
    }
}

fn ev(type_: u16, code: u16, value: i32) -> InputEvent {
    InputEvent {
        time: libc::timeval {
            tv_sec: 0,
            tv_usec: 0,
        },
        type_,
        code,
        value,
    }
}

fn emit_state(slot: &mut Slot, fd: i32, state: &ControllerState) {
    let mut evs = [ev(0, 0, 0); 32];
    let mut n = 0;
    if state.buttons != slot.last_buttons {
        for button in BUTTON_MAP {
            if (state.buttons ^ slot.last_buttons) & button.triton != 0 {
                evs[n] = ev(
                    EV_KEY,
                    button.evdev.0,
                    i32::from(state.pressed(button.triton)),
                );
                n += 1;
            }
        }
    }
    let abs = [
        state.left_stick.0,
        state.left_stick.1,
        state.right_stick.0,
        state.right_stick.1,
        state.left_trigger,
        state.right_trigger,
    ];
    let abs_codes = [ABS_X, ABS_Y, ABS_RX, ABS_RY, ABS_Z, ABS_RZ];
    for i in 0..6 {
        if abs[i] != slot.last_abs[i] {
            evs[n] = ev(EV_ABS, abs_codes[i], abs[i]);
            slot.last_abs[i] = abs[i];
            n += 1;
        }
    }
    if state.hat.0 != slot.last_hat[0] {
        evs[n] = ev(EV_ABS, ABS_HAT0X, state.hat.0);
        slot.last_hat[0] = state.hat.0;
        n += 1;
    }
    if state.hat.1 != slot.last_hat[1] {
        evs[n] = ev(EV_ABS, ABS_HAT0Y, state.hat.1);
        slot.last_hat[1] = state.hat.1;
        n += 1;
    }
    slot.last_buttons = state.buttons;
    if n == 0 {
        return;
    }
    evs[n] = ev(EV_SYN, SYN_REPORT, 0);
    n += 1;
    let _ = unsafe { libc::write(fd, evs.as_ptr().cast(), n * mem::size_of::<InputEvent>()) };
}

pub fn set_connected(
    slot: &mut Slot,
    connected: bool,
    requested: Mode,
    paused: bool,
    dump: bool,
    usb: Option<&UsbDevice>,
) {
    if slot.connected == connected {
        return;
    }
    slot.connected = connected;
    if connected {
        logln(format!(
            "controller connected on {} (interface {})",
            slot.path, slot.iface
        ));
        if requested == Mode::Gamepad && !paused {
            slot.send_lizard_off(usb);
            if !dump && slot.pad.is_none() && slot.transport.is_usbfs() {
                match create_uinput() {
                    Ok(pad) => {
                        slot.last_buttons = 0;
                        slot.last_abs = [0; 6];
                        slot.last_hat = [0; 2];
                        logln(format!(
                            "virtual Steam Controller pad created for {}",
                            slot.path
                        ));
                        slot.pad = Some(pad);
                    }
                    Err(_) => {}
                }
            }
        }
    } else {
        logln(format!("controller disconnected on {}", slot.path));
        slot.last_buttons = 0;
        destroy_pad(slot);
    }
}

pub fn handle_state(slot: &mut Slot, data: &[u8]) {
    let Some(state) = puckctl_protocol::parse_state(data) else {
        return;
    };
    if let Some(fd) = slot.pad.as_ref().map(|pad| pad.fd.as_raw_fd()) {
        emit_state(slot, fd, &state);
    } else {
        slot.last_buttons = state.buttons;
    }
}

pub fn ingest_report(
    slot: &mut Slot,
    data: &[u8],
    requested: Mode,
    paused: bool,
    dump: bool,
    usb: Option<&UsbDevice>,
) {
    if dump {
        dump_report(slot, data);
        return;
    }
    match classify(data) {
        Some(Report::State(_)) => {
            set_connected(slot, true, requested, paused, dump, usb);
            handle_state(slot, data);
        }
        Some(Report::Wireless(WirelessStatus::Connect)) => {
            set_connected(slot, true, requested, paused, dump, usb);
        }
        Some(Report::Wireless(WirelessStatus::Disconnect)) => {
            set_connected(slot, false, requested, paused, dump, usb);
        }
        _ => {}
    }
}

pub fn dump_report(slot: &Slot, data: &[u8]) {
    let n = data.len().min(64);
    let hex: String = data[..n].iter().map(|b| format!("{b:02x} ")).collect();
    logln(format!(
        "[{} if{}] len={}  {}",
        slot.path,
        slot.iface,
        data.len(),
        hex
    ));
    match classify(data) {
        Some(Report::State(state)) => logln(format!(
            "    state: buttons={:08x} trigL={} trigR={} LS=({},{}) RS=({},{})",
            state.buttons,
            state.left_trigger,
            state.right_trigger,
            state.left_stick.0,
            state.left_stick.1,
            state.right_stick.0,
            state.right_stick.1
        )),
        Some(Report::Wireless(WirelessStatus::Connect)) => {
            logln("    wireless status: CONNECT");
        }
        Some(Report::Wireless(WirelessStatus::Disconnect)) => {
            logln("    wireless status: DISCONNECT");
        }
        Some(Report::Battery {
            charge_state,
            level_pct,
        }) => logln(format!(
            "    battery: charge_state={charge_state} level={level_pct}%"
        )),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hid::Transport;
    use puckctl_protocol::{
        BTN_A, BTN_B, ID_BATTERY_STATUS, ID_CONTROLLER_STATE, ID_WIRELESS_STATUS, OFF_BUTTONS,
        STATE_MIN_LEN, WIRELESS_CONNECT, WIRELESS_DISCONNECT,
    };
    use std::io::Read;

    fn state_bytes(buttons: u32) -> Vec<u8> {
        let mut d = vec![0_u8; STATE_MIN_LEN];
        d[0] = ID_CONTROLLER_STATE;
        d[OFF_BUTTONS..OFF_BUTTONS + 4].copy_from_slice(&buttons.to_le_bytes());
        d
    }

    fn hid_slot() -> Slot {
        let (r, _w) = crate::test_env::nonblock_pipe();
        Slot::new("pipe".into(), 2, Transport::Hidraw(r))
    }

    #[test]
    fn uidev_looks_like_steam_controller() {
        let ud = steam_controller_uidev();
        assert_eq!(ud.id.vendor, VALVE_VID);
        assert_eq!(ud.id.product, PID_PROTEUS_DONGLE);
        assert_eq!(ud.ff_effects_max, 0);
        assert_eq!(ud.absmax[ABS_Z as usize], 255);
        assert_eq!(ud.absmin[ABS_HAT0X as usize], -1);
        assert!(create_uinput().is_err());
    }

    #[test]
    fn usbfs_connect_skips_uinput_in_tests() {
        let mut slot = Slot::new("usb".into(), 2, Transport::Usbfs { ep_in: 0x83 });
        set_connected(&mut slot, true, Mode::Gamepad, false, false, None);
        assert!(slot.connected);
        assert!(slot.pad.is_none());
        set_connected(&mut slot, true, Mode::Gamepad, false, false, None);
    }

    #[test]
    fn ingest_connect_state_and_dump() {
        let mut slot = hid_slot();
        ingest_report(
            &mut slot,
            &state_bytes(BTN_A),
            Mode::Gamepad,
            false,
            false,
            None,
        );
        assert!(slot.connected);
        assert_eq!(slot.last_buttons, BTN_A);
        ingest_report(
            &mut slot,
            &state_bytes(BTN_A),
            Mode::Gamepad,
            false,
            false,
            None,
        );
        ingest_report(
            &mut slot,
            &[ID_WIRELESS_STATUS, WIRELESS_DISCONNECT],
            Mode::Gamepad,
            false,
            false,
            None,
        );
        assert!(!slot.connected);
        ingest_report(
            &mut slot,
            &[ID_WIRELESS_STATUS, WIRELESS_CONNECT],
            Mode::Lizard,
            false,
            false,
            None,
        );
        assert!(slot.connected);
        ingest_report(&mut slot, &[0xff], Mode::Gamepad, false, false, None);
        let mut dump = hid_slot();
        ingest_report(
            &mut dump,
            &state_bytes(BTN_B),
            Mode::Gamepad,
            false,
            true,
            None,
        );
        dump_report(&dump, &[ID_WIRELESS_STATUS, WIRELESS_CONNECT]);
        dump_report(&dump, &[ID_WIRELESS_STATUS, WIRELESS_DISCONNECT]);
        dump_report(&dump, &[ID_BATTERY_STATUS, 1, 50]);
        dump_report(&dump, &[0x00]);
        destroy_pad(&mut dump);
    }

    #[test]
    fn emit_writes_evdev_events() {
        let (mut r, w) = crate::test_env::nonblock_pipe();
        let mut slot = hid_slot();
        slot.pad = Some(VirtualPad { fd: w });
        let mut state = ControllerState {
            buttons: BTN_A,
            left_stick: (10, -20),
            right_stick: (30, -40),
            left_trigger: 5,
            right_trigger: 6,
            hat: (1, -1),
            accel: None,
            gyro: None,
        };
        handle_state(&mut slot, &state_bytes(BTN_A));
        let fd = slot.pad.as_ref().unwrap().fd.as_raw_fd();
        emit_state(&mut slot, fd, &state);
        state.buttons = BTN_A | BTN_B;
        state.left_stick = (11, -21);
        state.hat = (0, 0);
        emit_state(&mut slot, fd, &state);
        emit_state(&mut slot, fd, &state);
        let mut buf = [0_u8; 512];
        let n = r.read(&mut buf).unwrap_or(0);
        assert!(n >= mem::size_of::<InputEvent>());
        set_connected(&mut slot, true, Mode::Gamepad, false, false, None);
        set_connected(&mut slot, false, Mode::Gamepad, false, false, None);
        assert!(slot.pad.is_none());
    }
}

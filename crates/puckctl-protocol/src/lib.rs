//! Triton HID constants and parsers for the Steam Controller Puck.
#![forbid(unsafe_code)]
//!
//! Report IDs, setting numbers, and field offsets come from SDL 3's
//! `SDL_hidapi_steam_triton.c` (zlib, Valve / Sam Lantinga).

mod buttons;
mod lizard;
mod parse;

pub use buttons::{
    ALL_BUTTONS, BUTTON_LABELS, MIN_COMBO_BUTTONS, button_count, format_buttons, is_settable_combo,
    just_matched_combo, sanitize_buttons,
};
pub use lizard::{
    FEATURE_REPORT_LEN, lizard_off_feature, lizard_on_feature, load_default_settings_feature,
};
pub use parse::{
    BUTTON_MAP, Button, ControllerState, EV_BTN_A, EV_BTN_B, EV_BTN_MODE, EV_BTN_SELECT,
    EV_BTN_START, EV_BTN_THUMBL, EV_BTN_THUMBR, EV_BTN_TL, EV_BTN_TR, EV_BTN_TRIGGER_HAPPY1,
    EV_BTN_TRIGGER_HAPPY2, EV_BTN_TRIGGER_HAPPY3, EV_BTN_TRIGGER_HAPPY4, EV_BTN_TRIGGER_HAPPY5,
    EV_BTN_X, EV_BTN_Y, EvdevKey, Report, WirelessStatus, classify, parse_state,
};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub const VALVE_VID: u16 = 0x28DE;
pub const PID_TRITON_WIRED: u16 = 0x1302;
pub const PID_TRITON_BLE: u16 = 0x1303;
pub const PID_PROTEUS_DONGLE: u16 = 0x1304;
pub const PID_NEREID_DONGLE: u16 = 0x1305;

pub const FEATURE_REPORT_ID: u8 = 0x01;
pub const ID_SET_SETTINGS_VALUES: u8 = 0x87;
pub const ID_LOAD_DEFAULT_SETTINGS: u8 = 0x8E;
pub const SETTING_LIZARD_MODE: u8 = 9;
pub const SETTING_IMU_MODE: u8 = 48;
pub const LIZARD_MODE_OFF: u8 = 0;
pub const LIZARD_MODE_ON: u8 = 1;
pub const GYRO_MODE_OFF: u16 = 0;
pub const GYRO_MODE_RAW_ACCEL: u16 = 0x0008;
pub const GYRO_MODE_RAW_GYRO: u16 = 0x0010;
pub const GYRO_MODE_RAW: u16 = GYRO_MODE_RAW_ACCEL | GYRO_MODE_RAW_GYRO;
pub const LIZARD_HEARTBEAT_MS: u64 = 3000;
pub const LIZARD_HEARTBEAT_STEAM_MS: u64 = 250;

pub const ID_CONTROLLER_STATE: u8 = 0x42;
pub const ID_BATTERY_STATUS: u8 = 0x43;
pub const ID_CONTROLLER_STATE_BLE: u8 = 0x45;
pub const ID_WIRELESS_STATUS_X: u8 = 0x46;
pub const ID_CONTROLLER_STATE_TIMESTAMP: u8 = 0x47;
pub const ID_WIRELESS_STATUS: u8 = 0x79;

pub const WIRELESS_DISCONNECT: u8 = 1;
pub const WIRELESS_CONNECT: u8 = 2;

pub const OFF_BUTTONS: usize = 2;
pub const OFF_TRIG_L: usize = 6;
pub const OFF_TRIG_R: usize = 8;
pub const OFF_LSTICK_X: usize = 10;
pub const OFF_LSTICK_Y: usize = 12;
pub const OFF_RSTICK_X: usize = 14;
pub const OFF_RSTICK_Y: usize = 16;
pub const STATE_MIN_LEN: usize = 18;
pub const STATE_IMU_LEN: usize = 46;
pub const OFF_IMU_ACCEL: usize = 34;
pub const OFF_IMU_GYRO: usize = 40;

pub const BTN_A: u32 = 0x0000_0001;
pub const BTN_B: u32 = 0x0000_0002;
pub const BTN_X: u32 = 0x0000_0004;
pub const BTN_Y: u32 = 0x0000_0008;
pub const BTN_QAM: u32 = 0x0000_0010;
pub const BTN_R3: u32 = 0x0000_0020;
pub const BTN_VIEW: u32 = 0x0000_0040;
pub const BTN_R4: u32 = 0x0000_0080;
pub const BTN_R5: u32 = 0x0000_0100;
pub const BTN_R: u32 = 0x0000_0200;
pub const BTN_DPAD_DOWN: u32 = 0x0000_0400;
pub const BTN_DPAD_RIGHT: u32 = 0x0000_0800;
pub const BTN_DPAD_LEFT: u32 = 0x0000_1000;
pub const BTN_DPAD_UP: u32 = 0x0000_2000;
pub const BTN_MENU: u32 = 0x0000_4000;
pub const BTN_L3: u32 = 0x0000_8000;
pub const BTN_STEAM: u32 = 0x0001_0000;
pub const BTN_L4: u32 = 0x0002_0000;
pub const BTN_L5: u32 = 0x0004_0000;
pub const BTN_L: u32 = 0x0008_0000;

pub const ID_HAPTIC_RUMBLE: u8 = 0x80;
pub const RUMBLE_REPORT_LEN: usize = 10;
pub const RUMBLE_RESEND_MS: u64 = 40;

/// systemd `RestartPreventExitStatus` when the control socket cannot bind.
pub const EXIT_ALREADY_RUNNING: i32 = 75;

/// Dongle controller slots are USB interfaces 2..=5. Interface 6 is the dongle.
#[must_use]
pub fn is_controller_interface(iface: i32) -> bool {
    (2..=5).contains(&iface)
}

#[must_use]
pub fn is_puck_pid(pid: u16) -> bool {
    matches!(
        pid,
        PID_TRITON_WIRED | PID_TRITON_BLE | PID_PROTEUS_DONGLE | PID_NEREID_DONGLE
    )
}

#[must_use]
pub fn is_dongle_pid(pid: u16) -> bool {
    matches!(pid, PID_PROTEUS_DONGLE | PID_NEREID_DONGLE)
}

#[must_use]
pub fn is_bridge_target(vid: u16, pid: u16, iface: i32) -> bool {
    if vid != VALVE_VID || !is_puck_pid(pid) {
        return false;
    }
    if is_dongle_pid(pid) {
        is_controller_interface(iface)
    } else {
        true
    }
}

#[must_use]
pub fn haptic_rumble(left: u16, right: u16) -> [u8; RUMBLE_REPORT_LEN] {
    let mut buf = [0_u8; RUMBLE_REPORT_LEN];
    buf[0] = ID_HAPTIC_RUMBLE;
    buf[4..6].copy_from_slice(&left.to_le_bytes());
    buf[7..9].copy_from_slice(&right.to_le_bytes());
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proteus_is_a_puck() {
        assert!(is_puck_pid(PID_PROTEUS_DONGLE));
        assert!(is_dongle_pid(PID_PROTEUS_DONGLE));
        assert!(is_dongle_pid(PID_NEREID_DONGLE));
        assert!(!is_dongle_pid(PID_TRITON_WIRED));
        assert!(is_controller_interface(2));
        assert!(is_controller_interface(5));
        assert!(!is_controller_interface(1));
        assert!(!is_controller_interface(6));
        assert!(is_bridge_target(VALVE_VID, PID_PROTEUS_DONGLE, 2));
        assert!(!is_bridge_target(VALVE_VID, PID_PROTEUS_DONGLE, 6));
        assert!(is_bridge_target(VALVE_VID, PID_TRITON_WIRED, 0));
        assert!(is_bridge_target(VALVE_VID, PID_TRITON_BLE, 1));
        assert!(!is_bridge_target(0x045e, PID_PROTEUS_DONGLE, 2));
        assert!(!is_bridge_target(VALVE_VID, 0x1102, 2));
        assert!(!VERSION.is_empty());
    }

    #[test]
    fn haptic_rumble_layout() {
        let buf = haptic_rumble(0x0201, 0x0403);
        assert_eq!(buf[0], ID_HAPTIC_RUMBLE);
        assert_eq!(&buf[4..9], &[0x01, 0x02, 0, 0x03, 0x04]);
    }
}

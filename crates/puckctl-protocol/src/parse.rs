use crate::{
    BTN_A, BTN_B, BTN_DPAD_DOWN, BTN_DPAD_LEFT, BTN_DPAD_RIGHT, BTN_DPAD_UP, BTN_L, BTN_L3, BTN_L4,
    BTN_L5, BTN_MENU, BTN_QAM, BTN_R, BTN_R3, BTN_R4, BTN_R5, BTN_STEAM, BTN_VIEW, BTN_X, BTN_Y,
    ID_BATTERY_STATUS, ID_CONTROLLER_STATE, ID_CONTROLLER_STATE_BLE, ID_CONTROLLER_STATE_TIMESTAMP,
    ID_WIRELESS_STATUS, ID_WIRELESS_STATUS_X, OFF_BUTTONS, OFF_LSTICK_X, OFF_LSTICK_Y,
    OFF_RSTICK_X, OFF_RSTICK_Y, OFF_TRIG_L, OFF_TRIG_R, STATE_MIN_LEN, WIRELESS_CONNECT,
    WIRELESS_DISCONNECT,
};

/// Linux evdev key codes (`linux/input-event-codes.h`). Kept here so parse
/// tests do not need kernel headers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvdevKey(pub u16);

pub const EV_BTN_A: EvdevKey = EvdevKey(0x130);
pub const EV_BTN_B: EvdevKey = EvdevKey(0x131);
pub const EV_BTN_X: EvdevKey = EvdevKey(0x133);
pub const EV_BTN_Y: EvdevKey = EvdevKey(0x134);
pub const EV_BTN_TL: EvdevKey = EvdevKey(0x136);
pub const EV_BTN_TR: EvdevKey = EvdevKey(0x137);
pub const EV_BTN_SELECT: EvdevKey = EvdevKey(0x13a);
pub const EV_BTN_START: EvdevKey = EvdevKey(0x13b);
pub const EV_BTN_MODE: EvdevKey = EvdevKey(0x13c);
pub const EV_BTN_THUMBL: EvdevKey = EvdevKey(0x13d);
pub const EV_BTN_THUMBR: EvdevKey = EvdevKey(0x13e);
pub const EV_BTN_TRIGGER_HAPPY1: EvdevKey = EvdevKey(0x2c0);
pub const EV_BTN_TRIGGER_HAPPY2: EvdevKey = EvdevKey(0x2c1);
pub const EV_BTN_TRIGGER_HAPPY3: EvdevKey = EvdevKey(0x2c2);
pub const EV_BTN_TRIGGER_HAPPY4: EvdevKey = EvdevKey(0x2c3);
pub const EV_BTN_TRIGGER_HAPPY5: EvdevKey = EvdevKey(0x2c4);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Button {
    pub triton: u32,
    pub evdev: EvdevKey,
}

pub const BUTTON_MAP: &[Button] = &[
    Button {
        triton: BTN_A,
        evdev: EV_BTN_A,
    },
    Button {
        triton: BTN_B,
        evdev: EV_BTN_B,
    },
    Button {
        triton: BTN_X,
        evdev: EV_BTN_X,
    },
    Button {
        triton: BTN_Y,
        evdev: EV_BTN_Y,
    },
    Button {
        triton: BTN_L,
        evdev: EV_BTN_TL,
    },
    Button {
        triton: BTN_R,
        evdev: EV_BTN_TR,
    },
    Button {
        triton: BTN_MENU,
        evdev: EV_BTN_SELECT,
    },
    Button {
        triton: BTN_VIEW,
        evdev: EV_BTN_START,
    },
    Button {
        triton: BTN_STEAM,
        evdev: EV_BTN_MODE,
    },
    Button {
        triton: BTN_L3,
        evdev: EV_BTN_THUMBL,
    },
    Button {
        triton: BTN_R3,
        evdev: EV_BTN_THUMBR,
    },
    Button {
        triton: BTN_R4,
        evdev: EV_BTN_TRIGGER_HAPPY1,
    },
    Button {
        triton: BTN_L4,
        evdev: EV_BTN_TRIGGER_HAPPY2,
    },
    Button {
        triton: BTN_R5,
        evdev: EV_BTN_TRIGGER_HAPPY3,
    },
    Button {
        triton: BTN_L5,
        evdev: EV_BTN_TRIGGER_HAPPY4,
    },
    Button {
        triton: BTN_QAM,
        evdev: EV_BTN_TRIGGER_HAPPY5,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControllerState {
    pub buttons: u32,
    pub left_stick: (i32, i32),
    pub right_stick: (i32, i32),
    pub left_trigger: i32,
    pub right_trigger: i32,
    pub hat: (i32, i32),
}

impl ControllerState {
    #[must_use]
    pub fn pressed(&self, bit: u32) -> bool {
        self.buttons & bit != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WirelessStatus {
    Connect,
    Disconnect,
    Unknown(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Report {
    State(ControllerState),
    Wireless(WirelessStatus),
    Battery { charge_state: u8, level_pct: u8 },
}

fn rd16(data: &[u8], off: usize) -> i16 {
    i16::from_le_bytes([data[off], data[off + 1]])
}

fn rd32(data: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
}

fn neg16(v: i16) -> i32 {
    if v == i16::MIN {
        i32::from(i16::MAX)
    } else {
        i32::from(-v)
    }
}

fn trigger_to_x360(v: i16) -> i32 {
    i32::from(v.max(0)) >> 7
}

#[must_use]
pub fn parse_state(data: &[u8]) -> Option<ControllerState> {
    if data.len() < STATE_MIN_LEN {
        return None;
    }
    let buttons = rd32(data, OFF_BUTTONS);
    let hat_x = if buttons & BTN_DPAD_RIGHT != 0 {
        1
    } else if buttons & BTN_DPAD_LEFT != 0 {
        -1
    } else {
        0
    };
    let hat_y = if buttons & BTN_DPAD_DOWN != 0 {
        1
    } else if buttons & BTN_DPAD_UP != 0 {
        -1
    } else {
        0
    };
    Some(ControllerState {
        buttons,
        left_stick: (
            i32::from(rd16(data, OFF_LSTICK_X)),
            neg16(rd16(data, OFF_LSTICK_Y)),
        ),
        right_stick: (
            i32::from(rd16(data, OFF_RSTICK_X)),
            neg16(rd16(data, OFF_RSTICK_Y)),
        ),
        left_trigger: trigger_to_x360(rd16(data, OFF_TRIG_L)),
        right_trigger: trigger_to_x360(rd16(data, OFF_TRIG_R)),
        hat: (hat_x, hat_y),
    })
}

#[must_use]
pub fn classify(data: &[u8]) -> Option<Report> {
    let id = *data.first()?;
    match id {
        ID_CONTROLLER_STATE | ID_CONTROLLER_STATE_BLE | ID_CONTROLLER_STATE_TIMESTAMP => {
            parse_state(data).map(Report::State)
        }
        ID_WIRELESS_STATUS | ID_WIRELESS_STATUS_X => {
            let code = *data.get(1)?;
            Some(Report::Wireless(match code {
                WIRELESS_CONNECT => WirelessStatus::Connect,
                WIRELESS_DISCONNECT => WirelessStatus::Disconnect,
                other => WirelessStatus::Unknown(other),
            }))
        }
        ID_BATTERY_STATUS if data.len() >= 3 => Some(Report::Battery {
            charge_state: data[1],
            level_pct: data[2],
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_report(buttons: u32, lx: i16, ly: i16, rx: i16, ry: i16, tl: i16, tr: i16) -> Vec<u8> {
        let mut d = vec![0_u8; STATE_MIN_LEN];
        d[0] = ID_CONTROLLER_STATE;
        d[OFF_BUTTONS..OFF_BUTTONS + 4].copy_from_slice(&buttons.to_le_bytes());
        d[OFF_TRIG_L..OFF_TRIG_L + 2].copy_from_slice(&tl.to_le_bytes());
        d[OFF_TRIG_R..OFF_TRIG_R + 2].copy_from_slice(&tr.to_le_bytes());
        d[OFF_LSTICK_X..OFF_LSTICK_X + 2].copy_from_slice(&lx.to_le_bytes());
        d[OFF_LSTICK_Y..OFF_LSTICK_Y + 2].copy_from_slice(&ly.to_le_bytes());
        d[OFF_RSTICK_X..OFF_RSTICK_X + 2].copy_from_slice(&rx.to_le_bytes());
        d[OFF_RSTICK_Y..OFF_RSTICK_Y + 2].copy_from_slice(&ry.to_le_bytes());
        d
    }

    #[test]
    fn short_report_is_none() {
        assert!(parse_state(&[ID_CONTROLLER_STATE, 0]).is_none());
    }

    #[test]
    fn a_button_and_triggers() {
        let d = state_report(BTN_A, 0, 0, 0, 0, 32767, 0);
        let s = parse_state(&d).expect("state");
        assert!(s.pressed(BTN_A));
        assert!(!s.pressed(BTN_B));
        assert_eq!(s.left_trigger, 32767 >> 7);
        assert_eq!(s.right_trigger, 0);
        assert_eq!(s.hat, (0, 0));
    }

    #[test]
    fn stick_y_is_negated_for_evdev() {
        let d = state_report(0, 100, 200, -30, i16::MIN, 0, 0);
        let s = parse_state(&d).expect("state");
        assert_eq!(s.left_stick, (100, -200));
        assert_eq!(s.right_stick, (-30, i32::from(i16::MAX)));
    }

    #[test]
    fn dpad_hat() {
        let d = state_report(BTN_DPAD_RIGHT | BTN_DPAD_UP, 0, 0, 0, 0, 0, 0);
        let s = parse_state(&d).expect("state");
        assert_eq!(s.hat, (1, -1));
    }

    #[test]
    fn negative_trigger_clamps() {
        let d = state_report(0, 0, 0, 0, 0, -5, -1);
        let s = parse_state(&d).expect("state");
        assert_eq!(s.left_trigger, 0);
        assert_eq!(s.right_trigger, 0);
    }

    #[test]
    fn classify_wireless_and_battery() {
        assert_eq!(
            classify(&[ID_WIRELESS_STATUS, WIRELESS_CONNECT]),
            Some(Report::Wireless(WirelessStatus::Connect))
        );
        assert_eq!(
            classify(&[ID_WIRELESS_STATUS_X, WIRELESS_DISCONNECT]),
            Some(Report::Wireless(WirelessStatus::Disconnect))
        );
        assert_eq!(
            classify(&[ID_BATTERY_STATUS, 1, 80]),
            Some(Report::Battery {
                charge_state: 1,
                level_pct: 80
            })
        );
    }

    #[test]
    fn button_map_covers_face_and_paddles() {
        let bits: u32 = BUTTON_MAP.iter().map(|b| b.triton).fold(0, |a, b| a | b);
        assert_eq!(bits & BTN_A, BTN_A);
        assert_eq!(bits & BTN_QAM, BTN_QAM);
        assert_eq!(bits & BTN_L4, BTN_L4);
        assert_eq!(BUTTON_MAP.len(), 16);
    }
}

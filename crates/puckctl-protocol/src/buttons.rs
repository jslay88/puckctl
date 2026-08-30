use crate::{
    BTN_A, BTN_B, BTN_DPAD_DOWN, BTN_DPAD_LEFT, BTN_DPAD_RIGHT, BTN_DPAD_UP, BTN_L, BTN_L3, BTN_L4,
    BTN_L5, BTN_MENU, BTN_QAM, BTN_R, BTN_R3, BTN_R4, BTN_R5, BTN_STEAM, BTN_VIEW, BTN_X, BTN_Y,
};

/// Known Triton bits, in the order they are shown in the tray.
pub const BUTTON_LABELS: &[(u32, &str)] = &[
    (BTN_A, "A"),
    (BTN_B, "B"),
    (BTN_X, "X"),
    (BTN_Y, "Y"),
    (BTN_L, "L"),
    (BTN_R, "R"),
    (BTN_L3, "L3"),
    (BTN_R3, "R3"),
    (BTN_L4, "L4"),
    (BTN_R4, "R4"),
    (BTN_L5, "L5"),
    (BTN_R5, "R5"),
    (BTN_MENU, "Menu"),
    (BTN_VIEW, "View"),
    (BTN_STEAM, "Steam"),
    (BTN_QAM, "QAM"),
    (BTN_DPAD_UP, "Up"),
    (BTN_DPAD_DOWN, "Down"),
    (BTN_DPAD_LEFT, "Left"),
    (BTN_DPAD_RIGHT, "Right"),
];

pub const ALL_BUTTONS: u32 = BTN_A
    | BTN_B
    | BTN_X
    | BTN_Y
    | BTN_QAM
    | BTN_R3
    | BTN_VIEW
    | BTN_R4
    | BTN_R5
    | BTN_R
    | BTN_DPAD_DOWN
    | BTN_DPAD_RIGHT
    | BTN_DPAD_LEFT
    | BTN_DPAD_UP
    | BTN_MENU
    | BTN_L3
    | BTN_STEAM
    | BTN_L4
    | BTN_L5
    | BTN_L;

pub const MIN_COMBO_BUTTONS: u32 = 2;

#[must_use]
pub fn sanitize_buttons(mask: u32) -> u32 {
    mask & ALL_BUTTONS
}

#[must_use]
pub fn button_count(mask: u32) -> u32 {
    sanitize_buttons(mask).count_ones()
}

#[must_use]
pub fn is_settable_combo(mask: u32) -> bool {
    button_count(mask) >= MIN_COMBO_BUTTONS
}

#[must_use]
pub fn format_buttons(mask: u32) -> String {
    let names: Vec<&str> = BUTTON_LABELS
        .iter()
        .filter(|(bit, _)| mask & bit != 0)
        .map(|(_, name)| *name)
        .collect();
    names.join(" + ")
}

#[must_use]
pub fn just_matched_combo(prev: u32, now: u32, combo: u32) -> bool {
    let combo = sanitize_buttons(combo);
    combo != 0 && sanitize_buttons(now) == combo && sanitize_buttons(prev) != combo
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_sanitize() {
        assert_eq!(format_buttons(0), "");
        assert_eq!(format_buttons(BTN_A | BTN_STEAM), "A + Steam");
        assert_eq!(sanitize_buttons(0xFFFF_FFFF), ALL_BUTTONS);
        assert!(!is_settable_combo(BTN_A));
        assert!(is_settable_combo(BTN_A | BTN_STEAM));
        assert_eq!(button_count(BTN_A | BTN_B | 0xF000_0000), 2);
        assert_eq!(BUTTON_LABELS.len(), 20);
    }

    #[test]
    fn rising_edge_only() {
        let combo = BTN_A | BTN_L4;
        assert!(!just_matched_combo(0, 0, combo));
        assert!(just_matched_combo(0, combo, combo));
        assert!(!just_matched_combo(combo, combo, combo));
        assert!(!just_matched_combo(0, combo | BTN_B, combo));
        assert!(!just_matched_combo(0, combo, 0));
    }
}

use puckctl_protocol::{just_matched_combo, sanitize_buttons};

use crate::paths::{self, combo_file};

#[must_use]
pub fn load() -> u32 {
    paths::read_trimmed(&combo_file())
        .and_then(|text| parse_mask(&text))
        .map(sanitize_buttons)
        .unwrap_or(0)
}

pub fn save(mask: u32) {
    let mask = sanitize_buttons(mask);
    paths::write_text(&combo_file(), &format!("{mask:x}\n"));
}

#[must_use]
pub fn parse_mask(text: &str) -> Option<u32> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let hex = text
        .strip_prefix("0x")
        .or_else(|| text.strip_prefix("0X"))
        .unwrap_or(text);
    u32::from_str_radix(hex, 16).ok().map(sanitize_buttons)
}

#[must_use]
pub fn triggered(prev: u32, now: u32, combo: u32) -> bool {
    just_matched_combo(prev, now, combo)
}

#[cfg(test)]
mod tests {
    use puckctl_protocol::{BTN_A, BTN_STEAM};

    use super::*;

    #[test]
    fn parses_hex_masks() {
        assert_eq!(parse_mask("0x10001"), Some(BTN_A | BTN_STEAM));
        assert_eq!(parse_mask("10001"), Some(BTN_A | BTN_STEAM));
        assert_eq!(parse_mask(""), None);
        assert_eq!(parse_mask("zz"), None);
    }

    #[test]
    fn trigger_is_rising_edge() {
        let combo = BTN_A | BTN_STEAM;
        assert!(triggered(0, combo, combo));
        assert!(!triggered(combo, combo, combo));
        assert!(!triggered(0, combo, 0));
    }

    #[test]
    fn load_and_save_round_trip() {
        crate::test_env::isolated(|_| {
            assert_eq!(load(), 0);
            save(BTN_A | BTN_STEAM | 0xF000_0000);
            assert_eq!(load(), BTN_A | BTN_STEAM);
            crate::paths::write_text(&crate::paths::combo_file(), "not-hex");
            assert_eq!(load(), 0);
        });
    }
}

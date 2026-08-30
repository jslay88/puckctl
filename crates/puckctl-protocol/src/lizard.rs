use crate::{
    FEATURE_REPORT_ID, ID_LOAD_DEFAULT_SETTINGS, ID_SET_SETTINGS_VALUES, LIZARD_MODE_OFF,
    LIZARD_MODE_ON, SETTING_LIZARD_MODE,
};

pub const FEATURE_REPORT_LEN: usize = 64;

fn blank() -> [u8; FEATURE_REPORT_LEN] {
    [0; FEATURE_REPORT_LEN]
}

/// `ID_SET_SETTINGS_VALUES` with lizard off. Re-send on the watchdog interval.
#[must_use]
pub fn lizard_off_feature() -> [u8; FEATURE_REPORT_LEN] {
    let mut buf = blank();
    buf[0] = FEATURE_REPORT_ID;
    buf[1] = ID_SET_SETTINGS_VALUES;
    buf[2] = 3;
    buf[3] = SETTING_LIZARD_MODE;
    buf[4] = LIZARD_MODE_OFF;
    buf
}

/// `ID_SET_SETTINGS_VALUES` with lizard on. Pair with [`load_default_settings_feature`].
#[must_use]
pub fn lizard_on_feature() -> [u8; FEATURE_REPORT_LEN] {
    let mut buf = blank();
    buf[0] = FEATURE_REPORT_ID;
    buf[1] = ID_SET_SETTINGS_VALUES;
    buf[2] = 3;
    buf[3] = SETTING_LIZARD_MODE;
    buf[4] = LIZARD_MODE_ON;
    buf
}

/// `ID_LOAD_DEFAULT_SETTINGS` — firmware keyboard/mouse comes back immediately.
#[must_use]
pub fn load_default_settings_feature() -> [u8; FEATURE_REPORT_LEN] {
    let mut buf = blank();
    buf[0] = FEATURE_REPORT_ID;
    buf[1] = ID_LOAD_DEFAULT_SETTINGS;
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lizard_off_matches_known_bytes() {
        let buf = lizard_off_feature();
        assert_eq!(&buf[..6], &[0x01, 0x87, 3, 9, 0, 0]);
        assert!(buf[6..].iter().all(|&b| b == 0));
    }

    #[test]
    fn lizard_on_then_defaults() {
        let on = lizard_on_feature();
        assert_eq!(&on[..5], &[0x01, 0x87, 3, 9, 1]);
        let def = load_default_settings_feature();
        assert_eq!(&def[..2], &[0x01, 0x8E]);
    }
}

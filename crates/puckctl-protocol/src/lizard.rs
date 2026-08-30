use crate::{
    FEATURE_REPORT_ID, GYRO_MODE_RAW, ID_LOAD_DEFAULT_SETTINGS, ID_SET_SETTINGS_VALUES,
    LIZARD_MODE_OFF, LIZARD_MODE_ON, SETTING_IMU_MODE, SETTING_LIZARD_MODE,
};

pub const FEATURE_REPORT_LEN: usize = 64;

fn blank() -> [u8; FEATURE_REPORT_LEN] {
    [0; FEATURE_REPORT_LEN]
}

fn put_setting(buf: &mut [u8], off: usize, num: u8, value: u16) {
    buf[off] = num;
    buf[off + 1..off + 3].copy_from_slice(&value.to_le_bytes());
}

/// `ID_SET_SETTINGS_VALUES` with lizard off and raw IMU on.
/// Re-send on the watchdog interval so firmware does not drop gyro.
#[must_use]
pub fn lizard_off_feature() -> [u8; FEATURE_REPORT_LEN] {
    let mut buf = blank();
    buf[0] = FEATURE_REPORT_ID;
    buf[1] = ID_SET_SETTINGS_VALUES;
    buf[2] = 6;
    put_setting(&mut buf, 3, SETTING_LIZARD_MODE, u16::from(LIZARD_MODE_OFF));
    put_setting(&mut buf, 6, SETTING_IMU_MODE, GYRO_MODE_RAW);
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
        assert_eq!(&buf[..9], &[0x01, 0x87, 6, 9, 0, 0, 48, 0x18, 0]);
        assert!(buf[9..].iter().all(|&b| b == 0));
    }

    #[test]
    fn lizard_on_then_defaults() {
        let on = lizard_on_feature();
        assert_eq!(&on[..5], &[0x01, 0x87, 3, 9, 1]);
        let def = load_default_settings_feature();
        assert_eq!(&def[..2], &[0x01, 0x8E]);
    }
}

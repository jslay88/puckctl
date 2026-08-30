use std::fs;

use crate::log::logln;
use crate::paths::{self, legacy_state_dir, override_file};
use crate::steam_cfg;

#[must_use]
pub fn steam_is_running() -> bool {
    let Ok(dir) = fs::read_dir("/proc") else {
        return false;
    };
    dir.flatten().any(|entry| {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.bytes().next().is_some_and(|b| b.is_ascii_digit()) {
            return false;
        }
        fs::read_to_string(format!("/proc/{name}/comm")).is_ok_and(|comm| comm.trim() == "steam")
    })
}

#[must_use]
pub fn load_override() -> bool {
    if let Some(text) = paths::read_trimmed(&override_file()) {
        return text == "on";
    }
    paths::read_trimmed(&legacy_state_dir().join("override")).is_some_and(|t| t == "on")
}

pub fn save_override(on: bool) {
    paths::write_text(&override_file(), if on { "on\n" } else { "off\n" });
}

pub fn set_override(on: bool) -> OverrideChange {
    save_override(on);
    steam_cfg::hide_steam_desktop_config(on);
    let steam = steam_is_running();
    if on {
        logln(format!(
            "Steam override on{}",
            if steam {
                " — taking the controller from Steam"
            } else {
                ""
            }
        ));
        OverrideChange {
            override_steam: true,
            paused: false,
            steam_seen: steam,
        }
    } else if steam {
        logln("Steam override off — releasing controller to Steam");
        OverrideChange {
            override_steam: false,
            paused: true,
            steam_seen: true,
        }
    } else {
        logln("Steam override off");
        OverrideChange {
            override_steam: false,
            paused: false,
            steam_seen: false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct OverrideChange {
    pub override_steam: bool,
    pub paused: bool,
    pub steam_seen: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_file_and_set() {
        crate::test_env::isolated(|_| {
            assert!(!load_override());
            save_override(true);
            assert!(load_override());
            save_override(false);
            assert!(!load_override());
            crate::paths::write_text(&legacy_state_dir().join("override"), "on\n");
            let _ = std::fs::remove_file(override_file());
            assert!(load_override());

            let on = set_override(true);
            assert!(on.override_steam);
            assert!(!on.paused);
            let off = set_override(false);
            assert!(!off.override_steam);
            if steam_is_running() {
                assert!(off.paused);
                assert!(off.steam_seen);
            } else {
                assert!(!off.paused);
            }
        });
    }
}

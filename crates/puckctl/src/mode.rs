use crate::paths::{self, legacy_state_dir, mode_file};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Gamepad,
    Lizard,
}

impl Mode {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Gamepad => "gamepad",
            Self::Lizard => "lizard",
        }
    }

    #[must_use]
    pub fn toggle(self) -> Self {
        match self {
            Self::Gamepad => Self::Lizard,
            Self::Lizard => Self::Gamepad,
        }
    }

    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim() {
            "lizard" => Some(Self::Lizard),
            "gamepad" => Some(Self::Gamepad),
            _ => None,
        }
    }
}

#[must_use]
pub fn load_requested_mode() -> Mode {
    if let Some(text) = paths::read_trimmed(&mode_file())
        && let Some(mode) = Mode::parse(&text)
    {
        return mode;
    }
    paths::read_trimmed(&legacy_state_dir().join("mode"))
        .and_then(|text| Mode::parse(&text))
        .unwrap_or(Mode::Gamepad)
}

pub fn save_requested_mode(mode: Mode) {
    paths::write_text(&mode_file(), &format!("{}\n", mode.name()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_toggle() {
        assert_eq!(Mode::parse("lizard"), Some(Mode::Lizard));
        assert_eq!(Mode::Gamepad.toggle(), Mode::Lizard);
        assert_eq!(Mode::Lizard.name(), "lizard");
        assert_eq!(Mode::parse("gamepad"), Some(Mode::Gamepad));
        assert_eq!(Mode::parse(" no "), None);
        assert_eq!(Mode::Lizard.toggle(), Mode::Gamepad);
        assert_eq!(Mode::Gamepad.name(), "gamepad");
    }

    #[test]
    fn load_prefers_current_then_legacy() {
        crate::test_env::isolated(|_| {
            assert_eq!(load_requested_mode(), Mode::Gamepad);
            save_requested_mode(Mode::Lizard);
            assert_eq!(load_requested_mode(), Mode::Lizard);
            let _ = std::fs::remove_file(mode_file());
            crate::paths::write_text(&legacy_state_dir().join("mode"), "gamepad\n");
            assert_eq!(load_requested_mode(), Mode::Gamepad);
        });
    }
}

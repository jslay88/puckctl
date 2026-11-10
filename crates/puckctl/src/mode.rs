use crate::paths::{self, legacy_state_dir, mode_file, pad_file};

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

/// Who the virtual evdev pad claims to be.
///
/// Xbox matches SDL2 game-controller profiles. Steam is the Valve VID/PID
/// plus a Triton hidraw clone, which is how hidapi gyro works.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PadIdentity {
    Xbox,
    Steam,
}

impl PadIdentity {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Xbox => "xbox",
            Self::Steam => "steam",
        }
    }

    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim() {
            "xbox" => Some(Self::Xbox),
            "steam" => Some(Self::Steam),
            _ => None,
        }
    }

    #[must_use]
    pub fn wants_uhid(self) -> bool {
        matches!(self, Self::Steam)
    }
}

#[must_use]
pub fn load_pad_identity() -> PadIdentity {
    paths::read_trimmed(&pad_file())
        .and_then(|text| PadIdentity::parse(&text))
        .unwrap_or(PadIdentity::Xbox)
}

pub fn save_pad_identity(identity: PadIdentity) {
    paths::write_text(&pad_file(), &format!("{}\n", identity.name()));
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
        assert_eq!(PadIdentity::parse("xbox"), Some(PadIdentity::Xbox));
        assert_eq!(PadIdentity::parse("steam"), Some(PadIdentity::Steam));
        assert_eq!(PadIdentity::parse("no"), None);
        assert_eq!(PadIdentity::Xbox.name(), "xbox");
        assert!(!PadIdentity::Xbox.wants_uhid());
        assert!(PadIdentity::Steam.wants_uhid());
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
            assert_eq!(load_pad_identity(), PadIdentity::Xbox);
            save_pad_identity(PadIdentity::Steam);
            assert_eq!(load_pad_identity(), PadIdentity::Steam);
        });
    }
}

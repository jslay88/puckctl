/// Live USB / evdev / uinput. Off in the test binary so unit tests do not
/// claim the puck, grab lizard nodes, or spawn a virtual pad.
#[must_use]
pub fn allowed() -> bool {
    !cfg!(test)
}

#[cfg(test)]
mod tests {
    #[test]
    fn disabled_in_unit_tests() {
        assert!(!super::allowed());
    }
}

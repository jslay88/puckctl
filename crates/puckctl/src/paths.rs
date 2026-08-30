use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const APP: &str = "puckctl";
const LEGACY: &str = "steam-puck-bridge";

#[must_use]
pub fn runtime_dir() -> PathBuf {
    env::var_os("XDG_RUNTIME_DIR")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
}

#[must_use]
pub fn socket_path() -> PathBuf {
    runtime_dir().join("puckctl.sock")
}

#[must_use]
pub fn log_path() -> PathBuf {
    runtime_dir().join("puckctl.log")
}

#[must_use]
pub fn state_dir() -> PathBuf {
    if let Some(dir) = env::var_os("STATE_DIRECTORY")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
    {
        return first_dir(&dir);
    }
    if let Some(dir) = env::var_os("XDG_STATE_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
    {
        return dir.join(APP);
    }
    home_dir().join(".local/state").join(APP)
}

#[must_use]
pub fn legacy_state_dir() -> PathBuf {
    if let Some(dir) = env::var_os("XDG_STATE_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
    {
        return dir.join(LEGACY);
    }
    home_dir().join(".local/state").join(LEGACY)
}

#[must_use]
pub fn mode_file() -> PathBuf {
    state_dir().join("mode")
}

#[must_use]
pub fn override_file() -> PathBuf {
    state_dir().join("override")
}

#[must_use]
pub fn combo_file() -> PathBuf {
    state_dir().join("combo")
}

#[must_use]
pub fn cfgbak_dir() -> PathBuf {
    state_dir().join("cfgbak")
}

pub fn ensure_parent(path: &Path) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
}

pub fn read_trimmed(path: &Path) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub fn write_text(path: &Path, text: &str) {
    ensure_parent(path);
    let _ = fs::write(path, text);
}

fn home_dir() -> PathBuf {
    env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
}

fn first_dir(state: &Path) -> PathBuf {
    // systemd can pass colon-separated StateDirectory values.
    match state.to_str().and_then(|s| s.split(':').next()) {
        Some(first) if !first.is_empty() => PathBuf::from(first),
        _ => state.to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socket_lives_under_runtime() {
        let sock = socket_path();
        assert!(sock.ends_with("puckctl.sock"));
    }

    #[test]
    fn isolated_paths_and_io() {
        crate::test_env::isolated(|root| {
            assert!(socket_path().starts_with(root.join("run")));
            assert!(log_path().ends_with("puckctl.log"));
            assert!(state_dir().ends_with("puckctl"));
            assert!(legacy_state_dir().ends_with("steam-puck-bridge"));
            assert!(mode_file().ends_with("mode"));
            assert!(override_file().ends_with("override"));
            assert!(combo_file().ends_with("combo"));
            assert!(cfgbak_dir().ends_with("cfgbak"));
            assert!(read_trimmed(&mode_file()).is_none());
            write_text(&mode_file(), "  lizard  \n");
            assert_eq!(read_trimmed(&mode_file()).as_deref(), Some("lizard"));
            write_text(&combo_file(), "   \n");
            assert!(read_trimmed(&combo_file()).is_none());
            ensure_parent(&root.join("missing").join("file"));
            assert!(root.join("missing").is_dir());
        });
    }

    #[test]
    fn state_directory_takes_first_colon_entry() {
        crate::test_env::isolated(|_| {
            // SAFETY: isolated() holds the env lock.
            unsafe {
                std::env::set_var("STATE_DIRECTORY", "/tmp/puck-a:/tmp/puck-b");
            }
            assert_eq!(state_dir(), PathBuf::from("/tmp/puck-a"));
            unsafe {
                std::env::set_var("STATE_DIRECTORY", "");
                std::env::remove_var("XDG_STATE_HOME");
                std::env::set_var("HOME", "/home/tester");
            }
            assert_eq!(
                state_dir(),
                PathBuf::from("/home/tester/.local/state/puckctl")
            );
            assert_eq!(
                legacy_state_dir(),
                PathBuf::from("/home/tester/.local/state/steam-puck-bridge")
            );
            unsafe {
                std::env::remove_var("HOME");
            }
            assert_eq!(home_dir(), PathBuf::from("/tmp"));
            assert_eq!(first_dir(Path::new("")), PathBuf::from(""));
            unsafe {
                std::env::remove_var("XDG_RUNTIME_DIR");
            }
            assert_eq!(runtime_dir(), PathBuf::from("/tmp"));
        });
    }
}

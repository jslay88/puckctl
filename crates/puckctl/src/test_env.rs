use std::fs::{self, File};
use std::os::fd::{FromRawFd, OwnedFd};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

static LOCK: Mutex<()> = Mutex::new(());
static NEXT: AtomicU64 = AtomicU64::new(1);

struct Snapshot {
    runtime: Option<std::ffi::OsString>,
    state: Option<std::ffi::OsString>,
    statedir: Option<std::ffi::OsString>,
    home: Option<std::ffi::OsString>,
    puckctl: Option<std::ffi::OsString>,
}

impl Snapshot {
    fn take() -> Self {
        Self {
            runtime: std::env::var_os("XDG_RUNTIME_DIR"),
            state: std::env::var_os("XDG_STATE_HOME"),
            statedir: std::env::var_os("STATE_DIRECTORY"),
            home: std::env::var_os("HOME"),
            puckctl: std::env::var_os("PUCKCTL"),
        }
    }

    fn restore(self) {
        set_or_remove("XDG_RUNTIME_DIR", self.runtime);
        set_or_remove("XDG_STATE_HOME", self.state);
        set_or_remove("STATE_DIRECTORY", self.statedir);
        set_or_remove("HOME", self.home);
        set_or_remove("PUCKCTL", self.puckctl);
    }
}

fn set_or_remove(key: &str, value: Option<std::ffi::OsString>) {
    // SAFETY: tests hold LOCK, so only one thread mutates the environment.
    unsafe {
        match value {
            Some(v) => std::env::set_var(key, v),
            None => std::env::remove_var(key),
        }
    }
}

pub(crate) fn isolated<F, T>(f: F) -> T
where
    F: FnOnce(&Path) -> T,
{
    let _guard = LOCK.lock().expect("test env lock");
    let root = std::env::temp_dir().join(format!(
        "puckctl-cov-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let runtime = root.join("run");
    let state = root.join("state");
    let home = root.join("home");
    fs::create_dir_all(&runtime).unwrap();
    fs::create_dir_all(&state).unwrap();
    fs::create_dir_all(&home).unwrap();
    let prev = Snapshot::take();
    set_or_remove("XDG_RUNTIME_DIR", Some(runtime.into_os_string()));
    set_or_remove("XDG_STATE_HOME", Some(state.into_os_string()));
    set_or_remove("STATE_DIRECTORY", None);
    set_or_remove("HOME", Some(home.into_os_string()));
    set_or_remove("PUCKCTL", None);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&root)));
    prev.restore();
    let _ = fs::remove_dir_all(&root);
    match result {
        Ok(value) => value,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

pub(crate) fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::write(path, text).unwrap();
}

pub(crate) fn nonblock_pipe() -> (File, File) {
    let mut fds = [0; 2];
    let rc = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC | libc::O_NONBLOCK) };
    assert_eq!(rc, 0, "pipe2");
    unsafe {
        (
            File::from(OwnedFd::from_raw_fd(fds[0])),
            File::from(OwnedFd::from_raw_fd(fds[1])),
        )
    }
}

pub(crate) fn temp_file(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    File::create(&path).unwrap();
    path
}

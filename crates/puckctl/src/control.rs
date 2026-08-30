use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use puckctl_protocol::EXIT_ALREADY_RUNNING;

use crate::mode::Mode;
use crate::paths::{self, log_path, socket_path};
use crate::steam;

pub const MAX_CLIENTS: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandKind {
    Status,
    Gamepad,
    Lizard,
    Toggle,
    OverrideOn,
    OverrideOff,
    OverrideToggle,
    Buttons,
    Combo,
    ComboSet(u32),
    Quit,
    Unknown,
}

impl CommandKind {
    #[must_use]
    pub fn parse(line: &str) -> Self {
        let mut parts = line.split_whitespace();
        match parts.next().unwrap_or("") {
            "status" => Self::Status,
            "gamepad" => Self::Gamepad,
            "lizard" => Self::Lizard,
            "toggle" => Self::Toggle,
            "override-on" => Self::OverrideOn,
            "override-off" => Self::OverrideOff,
            "override-toggle" => Self::OverrideToggle,
            "buttons" => Self::Buttons,
            "combo" => Self::Combo,
            "combo-clear" => Self::ComboSet(0),
            "combo-set" => crate::combo::parse_mask(parts.next().unwrap_or(""))
                .map_or(Self::Unknown, Self::ComboSet),
            "quit" => Self::Quit,
            _ => Self::Unknown,
        }
    }
}

#[must_use]
pub fn format_status(
    effective: &str,
    requested: Mode,
    steam: bool,
    override_steam: bool,
    connected: bool,
    combo: u32,
) -> String {
    format!(
        "OK effective={effective} requested={} steam={} override={} connected={} combo={combo:x} daemon=1\n",
        requested.name(),
        u8::from(steam),
        u8::from(override_steam),
        u8::from(connected)
    )
}

pub fn open_listen_socket() -> Result<UnixListener, i32> {
    let path = socket_path();
    paths::ensure_parent(&path);
    let _ = fs::remove_file(&path);
    let listener = UnixListener::bind(&path).map_err(|_| EXIT_ALREADY_RUNNING)?;
    if listener.set_nonblocking(true).is_err() {
        return Err(EXIT_ALREADY_RUNNING);
    }
    let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    Ok(listener)
}

pub fn read_command(stream: &mut UnixStream) -> io::Result<CommandKind> {
    let mut buf = [0_u8; 128];
    let n = stream.read(&mut buf)?;
    if n == 0 {
        return Ok(CommandKind::Unknown);
    }
    let text = String::from_utf8_lossy(&buf[..n]);
    let line = text.split('\n').next().unwrap_or("");
    Ok(CommandKind::parse(line))
}

pub fn write_reply(stream: &mut UnixStream, reply: &str) {
    let _ = stream.write_all(reply.as_bytes());
}

fn connect_daemon() -> io::Result<UnixStream> {
    UnixStream::connect(socket_path())
}

pub fn send_cmd(cmd: &str) -> io::Result<String> {
    let mut stream = connect_daemon()?;
    stream.write_all(format!("{cmd}\n").as_bytes())?;
    let mut reply = String::new();
    stream.read_to_string(&mut reply)?;
    Ok(reply)
}

fn wait_for_daemon() -> bool {
    let tries = if cfg!(test) { 3 } else { 50 };
    let delay = if cfg!(test) {
        Duration::from_millis(1)
    } else {
        Duration::from_millis(20)
    };
    wait_for_connect(tries, delay)
}

fn wait_for_connect(tries: u32, delay: Duration) -> bool {
    for _ in 0..tries {
        if connect_daemon().is_ok() {
            return true;
        }
        if !delay.is_zero() {
            thread::sleep(delay);
        }
    }
    false
}

fn attach_daemon_log(cmd: &mut Command, log: Option<fs::File>) {
    match log {
        Some(file) => match file.try_clone() {
            Ok(clone) => {
                cmd.stdout(clone);
                cmd.stderr(file);
            }
            Err(_) => {
                cmd.stdout(file);
                cmd.stderr(Stdio::null());
            }
        },
        None => {
            cmd.stdout(Stdio::null());
            cmd.stderr(Stdio::null());
        }
    }
}

fn prepare_daemon_command(exe: &Path) -> Command {
    paths::ensure_parent(&log_path());
    let log = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path())
        .ok();
    let mut cmd = Command::new(exe);
    cmd.stdin(Stdio::null());
    attach_daemon_log(&mut cmd, log);
    // Detach from the CLI so systemd-less starts survive.
    unsafe {
        use std::os::unix::process::CommandExt;
        cmd.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    cmd
}

fn launch_daemon_exe(exe: &Path) -> bool {
    prepare_daemon_command(exe).spawn().is_ok() && wait_for_daemon()
}

fn spawn_daemon() -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    launch_daemon_exe(&exe)
}

const fn systemd_start_args() -> [&'static str; 3] {
    ["--user", "start", "puckctl.service"]
}

fn accepted_systemd_start(started: bool) -> bool {
    started && wait_for_daemon()
}

fn try_start_via_systemd() -> bool {
    let started = Command::new("systemctl")
        .args(systemd_start_args())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    accepted_systemd_start(started)
}

pub fn ensure_daemon() -> bool {
    if connect_daemon().is_ok() {
        return true;
    }
    try_start_via_systemd() || spawn_daemon()
}

pub fn cli(cmd: &str) -> i32 {
    if cmd == "status" {
        return match send_cmd("status") {
            Ok(reply) => {
                print!("{reply}");
                0
            }
            Err(_) => {
                let steam = steam::steam_is_running();
                println!(
                    "OK effective={} requested=unknown steam={} override=0 connected=0 combo=0 daemon=0",
                    if steam { "steam" } else { "lizard" },
                    u8::from(steam)
                );
                0
            }
        };
    }
    if cmd == "quit" {
        return match send_cmd("quit") {
            Ok(reply) => {
                print!("{reply}");
                0
            }
            Err(_) => {
                eprintln!("daemon not running");
                1
            }
        };
    }
    if cmd == "lizard" {
        return match send_cmd("lizard") {
            Ok(reply) => {
                print!("{reply}");
                0
            }
            Err(_) => {
                println!(
                    "OK effective=lizard requested=lizard steam=0 override=0 connected=0 combo=0 daemon=0"
                );
                0
            }
        };
    }
    if !ensure_daemon() {
        eprintln!("puckctl: failed to start daemon");
        return 1;
    }
    match send_cmd(cmd) {
        Ok(reply) => {
            print!("{reply}");
            i32::from(!reply.starts_with("OK"))
        }
        Err(_) => {
            eprintln!("puckctl: failed to talk to daemon");
            1
        }
    }
}

pub fn unlink_socket() {
    let _ = fs::remove_file(Path::new(&socket_path()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_includes_connected() {
        let line = format_status("gamepad", Mode::Gamepad, false, true, true, 0x11);
        assert!(line.contains("connected=1"));
        assert!(line.contains("override=1"));
        assert!(line.contains("combo=11"));
        let line = format_status("lizard", Mode::Lizard, false, false, false, 0);
        assert!(line.contains("connected=0"));
        assert!(line.contains("combo=0"));
    }

    #[test]
    fn parses_combo_commands() {
        assert_eq!(CommandKind::parse("buttons"), CommandKind::Buttons);
        assert_eq!(CommandKind::parse("combo"), CommandKind::Combo);
        assert_eq!(CommandKind::parse("combo-clear"), CommandKind::ComboSet(0));
        assert_eq!(
            CommandKind::parse("combo-set 0x11"),
            CommandKind::ComboSet(0x11)
        );
        assert_eq!(CommandKind::parse("combo-set zz"), CommandKind::Unknown);
        assert_eq!(CommandKind::parse("status"), CommandKind::Status);
        assert_eq!(CommandKind::parse("gamepad"), CommandKind::Gamepad);
        assert_eq!(CommandKind::parse("lizard"), CommandKind::Lizard);
        assert_eq!(CommandKind::parse("toggle"), CommandKind::Toggle);
        assert_eq!(CommandKind::parse("override-on"), CommandKind::OverrideOn);
        assert_eq!(CommandKind::parse("override-off"), CommandKind::OverrideOff);
        assert_eq!(
            CommandKind::parse("override-toggle"),
            CommandKind::OverrideToggle
        );
        assert_eq!(CommandKind::parse("quit"), CommandKind::Quit);
        assert_eq!(CommandKind::parse(""), CommandKind::Unknown);
        assert_eq!(CommandKind::parse("combo-set"), CommandKind::Unknown);
    }

    #[test]
    fn socket_round_trip_and_offline_cli() {
        crate::test_env::isolated(|_| {
            let listener = open_listen_socket().expect("bind");
            let mut client = connect_daemon().expect("connect");
            write_reply(&mut client, "ping");
            client.shutdown(std::net::Shutdown::Write).unwrap();
            let (mut server, _) = listener.accept().unwrap();
            let mut buf = String::new();
            server.read_to_string(&mut buf).unwrap();
            assert_eq!(buf, "ping");

            let mut client = connect_daemon().unwrap();
            client.write_all(b"status\n").unwrap();
            let (mut server, _) = listener.accept().unwrap();
            assert_eq!(read_command(&mut server).unwrap(), CommandKind::Status);

            let client = connect_daemon().unwrap();
            client.shutdown(std::net::Shutdown::Write).unwrap();
            let (mut server, _) = listener.accept().unwrap();
            assert_eq!(read_command(&mut server).unwrap(), CommandKind::Unknown);

            unlink_socket();
            assert!(send_cmd("status").is_err());
            assert_eq!(cli("status"), 0);
            assert_eq!(cli("lizard"), 0);
            assert_eq!(cli("quit"), 1);
        });
    }

    fn reply_n(listener: UnixListener, body: &'static str, n: usize) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            let _ = listener.set_nonblocking(false);
            for _ in 0..n {
                if let Ok((mut stream, _)) = listener.accept() {
                    let _ = read_command(&mut stream);
                    write_reply(&mut stream, body);
                    let _ = stream.shutdown(std::net::Shutdown::Write);
                }
            }
        })
    }

    #[test]
    fn send_cmd_and_cli_against_fake_daemon() {
        crate::test_env::isolated(|_| {
            let listener = open_listen_socket().expect("bind");
            let worker = reply_n(
                listener,
                "OK effective=gamepad requested=gamepad steam=0 override=1 connected=1 combo=0 daemon=1\n",
                1,
            );
            assert!(send_cmd("status").unwrap().starts_with("OK"));
            worker.join().unwrap();

            let listener = open_listen_socket().expect("bind");
            let worker = reply_n(listener, "OK done\n", 1);
            assert_eq!(cli("status"), 0);
            worker.join().unwrap();

            let listener = open_listen_socket().expect("bind");
            let worker = reply_n(listener, "OK quitting\n", 1);
            assert_eq!(cli("quit"), 0);
            worker.join().unwrap();

            let listener = open_listen_socket().expect("bind");
            let worker = reply_n(
                listener,
                "OK effective=lizard requested=lizard steam=0 override=0 connected=0 combo=0 daemon=1\n",
                1,
            );
            assert_eq!(cli("lizard"), 0);
            worker.join().unwrap();

            let listener = open_listen_socket().expect("bind");
            let worker = reply_n(listener, "OK toggled\n", 2);
            assert_eq!(cli("toggle"), 0);
            worker.join().unwrap();

            let listener = open_listen_socket().expect("bind");
            let worker = reply_n(listener, "ERR no\n", 2);
            assert_eq!(cli("gamepad"), 1);
            worker.join().unwrap();
        });
    }

    #[test]
    fn ensure_and_spawn_helpers() {
        crate::test_env::isolated(|_| {
            assert!(!wait_for_connect(2, Duration::ZERO));
            let listener = open_listen_socket().expect("bind");
            assert!(wait_for_connect(2, Duration::ZERO));
            assert!(wait_for_daemon());
            assert!(ensure_daemon());
            drop(listener);

            attach_daemon_log(&mut Command::new("/bin/true"), None);
            let log = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_path())
                .unwrap();
            attach_daemon_log(&mut Command::new("/bin/true"), Some(log));
            let _ = prepare_daemon_command(Path::new("/bin/true"));
            assert_eq!(systemd_start_args(), ["--user", "start", "puckctl.service"]);
            assert!(!accepted_systemd_start(false));
            assert!(!accepted_systemd_start(true));
            assert!(!launch_daemon_exe(Path::new("/bin/true")));
        });
    }
}

use puckctl_protocol::VERSION;

use crate::control;
use crate::daemon::Daemon;

fn usage(out: &mut impl std::io::Write) -> std::io::Result<()> {
    writeln!(
        out,
        "\
usage: puckctl [command] [--dump] [--no-steam-check]

Gamepad / desktop mode tool for the Steam Controller Puck.
With no command it runs the daemon (gamepad mode). Steam running
takes the device unless override is on.

commands:
  gamepad            disable lizard mode, expose a virtual pad
  lizard             restore firmware keyboard/mouse
  toggle             flip between gamepad and lizard
  override on|off|toggle
                     keep control even while Steam is running
  combo [clear|HEX]  show, clear, or set the gamepad-to-desktop combo
  buttons            print the current button mask
  status             print effective / requested mode
  quit               stop a running daemon

options:
  --dump             protocol debug: hexdump and parse every report;
                     no virtual pad is created
  --no-steam-check   keep running even when a Steam client is up
  -h, --help         this text
  -V, --version      print version and exit

Logs to stdout; normally run via the systemd user unit."
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Action {
    Help,
    Version,
    Daemon { dump: bool, steam_check: bool },
    Client(String),
    BadUsage,
}

pub(crate) fn parse_args<I, S>(args: I) -> Action
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut cmd: Option<String> = None;
    let mut arg: Option<String> = None;
    let mut dump = false;
    let mut steam_check = true;

    for item in args {
        match item.as_ref() {
            "--dump" => dump = true,
            "--no-steam-check" => steam_check = false,
            "-h" | "--help" => return Action::Help,
            "-V" | "--version" => return Action::Version,
            flag if flag.starts_with('-') => return Action::BadUsage,
            other if cmd.is_none() => cmd = Some(other.to_string()),
            other if arg.is_none() => arg = Some(other.to_string()),
            _ => return Action::BadUsage,
        }
    }

    let Some(cmd) = cmd else {
        return Action::Daemon { dump, steam_check };
    };

    if cmd == "override" {
        return match arg.as_deref() {
            None | Some("status") => Action::Client("status".into()),
            Some("on") => Action::Client("override-on".into()),
            Some("off") => Action::Client("override-off".into()),
            Some("toggle") => Action::Client("override-toggle".into()),
            _ => Action::BadUsage,
        };
    }

    if cmd == "combo" {
        return match arg.as_deref() {
            None => Action::Client("combo".into()),
            Some("clear") => Action::Client("combo-clear".into()),
            Some(mask) => {
                if crate::combo::parse_mask(mask).is_none() {
                    Action::BadUsage
                } else {
                    Action::Client(format!("combo-set {mask}"))
                }
            }
        };
    }

    if arg.is_some() {
        return Action::BadUsage;
    }

    match cmd.as_str() {
        "gamepad" | "lizard" | "toggle" | "status" | "quit" | "buttons" | "override-on"
        | "override-off" | "override-toggle" => Action::Client(cmd),
        _ => Action::BadUsage,
    }
}

pub(crate) fn run_action(action: Action) -> i32 {
    match action {
        Action::Help => {
            let _ = usage(&mut std::io::stdout());
            0
        }
        Action::Version => {
            println!("puckctl {VERSION}");
            0
        }
        Action::Daemon { dump, steam_check } => run_daemon(dump, steam_check),
        Action::Client(cmd) => control::cli(&cmd),
        Action::BadUsage => {
            let _ = usage(&mut std::io::stderr());
            2
        }
    }
}

pub fn main() {
    std::process::exit(run_action(parse_args(std::env::args().skip(1))));
}

fn run_daemon(dump: bool, steam_check: bool) -> i32 {
    let listener = match control::open_listen_socket() {
        Ok(listener) => listener,
        Err(code) => {
            crate::log::logln("control socket failed (is another instance running?)");
            return code;
        }
    };
    Daemon::new(dump, steam_check).run(listener)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_daemon_and_flags() {
        assert_eq!(
            parse_args(Vec::<&str>::new()),
            Action::Daemon {
                dump: false,
                steam_check: true
            }
        );
        assert_eq!(
            parse_args(["--dump", "--no-steam-check"]),
            Action::Daemon {
                dump: true,
                steam_check: false
            }
        );
        assert_eq!(parse_args(["-h"]), Action::Help);
        assert_eq!(parse_args(["--help"]), Action::Help);
        assert_eq!(parse_args(["-V"]), Action::Version);
        assert_eq!(parse_args(["--unknown"]), Action::BadUsage);
    }

    #[test]
    fn parses_client_commands() {
        assert_eq!(parse_args(["status"]), Action::Client("status".into()));
        assert_eq!(parse_args(["gamepad"]), Action::Client("gamepad".into()));
        assert_eq!(parse_args(["lizard"]), Action::Client("lizard".into()));
        assert_eq!(parse_args(["toggle"]), Action::Client("toggle".into()));
        assert_eq!(parse_args(["buttons"]), Action::Client("buttons".into()));
        assert_eq!(parse_args(["quit"]), Action::Client("quit".into()));
        assert_eq!(parse_args(["override"]), Action::Client("status".into()));
        assert_eq!(
            parse_args(["override", "on"]),
            Action::Client("override-on".into())
        );
        assert_eq!(
            parse_args(["override", "off"]),
            Action::Client("override-off".into())
        );
        assert_eq!(
            parse_args(["override", "toggle"]),
            Action::Client("override-toggle".into())
        );
        assert_eq!(
            parse_args(["override", "status"]),
            Action::Client("status".into())
        );
        assert_eq!(parse_args(["override", "maybe"]), Action::BadUsage);
        assert_eq!(parse_args(["combo"]), Action::Client("combo".into()));
        assert_eq!(
            parse_args(["combo", "clear"]),
            Action::Client("combo-clear".into())
        );
        assert_eq!(
            parse_args(["combo", "0x11"]),
            Action::Client("combo-set 0x11".into())
        );
        assert_eq!(parse_args(["combo", "zz"]), Action::BadUsage);
        assert_eq!(parse_args(["status", "extra"]), Action::BadUsage);
        assert_eq!(parse_args(["nope"]), Action::BadUsage);
        assert_eq!(parse_args(["a", "b", "c"]), Action::BadUsage);
    }

    #[test]
    fn usage_mentions_combo() {
        let mut buf = Vec::new();
        usage(&mut buf).unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert!(text.contains("combo"));
        assert!(text.contains("--dump"));
    }

    #[test]
    fn run_action_help_version_usage() {
        crate::test_env::isolated(|_| {
            assert_eq!(run_action(Action::Help), 0);
            assert_eq!(run_action(Action::Version), 0);
            assert_eq!(run_action(Action::BadUsage), 2);
            assert_eq!(run_action(Action::Client("status".into())), 0);
            assert_eq!(run_action(Action::Client("quit".into())), 1);
        });
    }
}

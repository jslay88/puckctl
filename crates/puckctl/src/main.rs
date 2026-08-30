use puckctl_protocol::VERSION;

fn usage(out: &mut impl std::io::Write) -> std::io::Result<()> {
    writeln!(
        out,
        "\
usage: puckctl [command]

Gamepad / desktop mode tool for the Steam Controller Puck.
Daemon and CLI are not wired up yet.

commands (planned):
  gamepad    virtual Xbox 360 pad
  lizard     firmware keyboard/mouse
  toggle
  override on|off|toggle
  status
  quit

  -h, --help       this text
  -V, --version    print version and exit"
    )
}

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("-h" | "--help") => {
            let _ = usage(&mut std::io::stdout());
        }
        Some("-V" | "--version") => {
            println!("puckctl {VERSION}");
        }
        Some(other) => {
            eprintln!("puckctl: {other}: not implemented yet");
            std::process::exit(2);
        }
        None => {
            eprintln!("puckctl: daemon not implemented yet (try --help)");
            std::process::exit(2);
        }
    }
}

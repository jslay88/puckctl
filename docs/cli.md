# CLI

`puckctl` is the daemon and the client. With no command it starts the
daemon (normally you let the systemd user unit do that). Any other command
talks to a running daemon, and will try to start one if needed.

```
puckctl [command] [--dump] [--no-steam-check]
```

## Commands

| Command | What it does |
|---|---|
| `gamepad` | Desktop firmware off, virtual Steam Controller pad |
| `lizard` | Firmware keyboard and mouse (desktop mode) |
| `toggle` | Flip between those two |
| `override on` / `off` / `toggle` | Keep control while Steam is running ([steam.md](steam.md)) |
| `combo` | Print the saved gamepad-to-desktop combo |
| `combo HEX` | Set that combo (example: `combo 0x11`) |
| `combo clear` | Remove the combo |
| `buttons` | Current button mask from the controller |
| `status` | Effective mode, requested mode, Steam, override, connected, combo |
| `quit` | Stop the daemon |
| `-h` / `--help` | Usage |
| `-V` / `--version` | Version |

`--dump` is a debug daemon: hexdump and parse every report, no virtual pad.
`--no-steam-check` keeps running as if override were on, even with Steam up.

The tray is a separate binary, `puckctl-tray`. `--set-combo` opens the
hold-to-set window (the tray menu does this for you).

## Status line

`puckctl status` prints one line, for example:

```
OK effective=gamepad requested=gamepad steam=0 override=1 connected=1 combo=11 daemon=1
```

`effective` is what is live (`gamepad`, `lizard`, or `steam` when yielded).
`combo` is hex. `daemon=0` means the client printed a fallback because
nothing was listening.

## State files

Under `~/.local/state/puckctl` (or `$XDG_STATE_HOME/puckctl`):

| File | Contents |
|---|---|
| `mode` | Last requested mode |
| `override` | `on` or `off` |
| `combo` | Hex mask |
| `cfgbak/` | Steam config backups while override is on |

If you used steam-puck-bridge before, override can still be read from
`~/.local/state/steam-puck-bridge/override` when puckctl has no file yet.

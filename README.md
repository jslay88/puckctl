# puckctl

Gamepad or desktop keyboard/mouse for the 2nd-generation Steam Controller
(the wireless Puck). Linux only.

Unofficial and not affiliated with Valve. "Steam" and "Steam Controller" are
trademarks of Valve Corporation.

This is a new Rust project. It is not a fork of
[steam-puck-bridge](https://github.com/benashby/steam-puck-bridge).

## What you get

- A user daemon that can run the Puck as a gamepad, or leave it in firmware
  desktop mode (keyboard and mouse)
- A tray icon to switch modes, set a button combo back to desktop, and
  optionally keep control while Steam is running
- Saved mode, Steam override, and combo across restarts

Daily hardware is the Proteus dongle (`28de:1304`). Other Triton Pucks should
work and are not what we test every day. See [docs/install.md](docs/install.md).

## Install

Linux, x86_64, a user systemd session, and GTK 4 at runtime (the tray).
KDE works as-is. GNOME needs an AppIndicator extension. More in
[docs/install.md](docs/install.md).

Arch / CachyOS: `yay -S puckctl` once the [AUR package](https://aur.archlinux.org/packages/puckctl)
is up (CI publishes it on each GitHub Release).

**Release binary** (no Rust toolchain):

```sh
# from https://github.com/jslay88/puckctl/releases
tar -xzf puckctl-*-x86_64-linux.tar.gz
cd puckctl-*-x86_64-linux
./install.sh
```

`./install.sh --udev` also installs device rules (sudo). `./install.sh
--download` fetches the latest release if you only have this repo's
`scripts/install.sh`.

**From source** (Rust 1.98 + GTK 4 headers):

```sh
make enable-tray
```

## Tray

The icon follows the current mode (gamepad, desktop, or Steam). Click it to
toggle gamepad and desktop. The menu sets mode, Steam override, start on
login, and the desktop combo (hold at least two buttons for five seconds).

With Override Steam on, switching desktop (keyboard/mouse) to gamepad can
make Steam report the Steam Controller disconnected. That is Steam losing
its mapping, not puckctl dropping the device. Gyro still comes from hidraw.
The tray stays the yellow gamepad icon.

CLI and Steam behavior: [docs/cli.md](docs/cli.md), [docs/steam.md](docs/steam.md).

## Uninstall

From a release tree: `./install.sh --uninstall` (add `--udev` if you
installed the rules). From a source checkout: `make uninstall`, and
`sudo make uninstall-udev` if needed.

## Docs

| Doc | For |
|---|---|
| [docs/install.md](docs/install.md) | Build, udev, systemd, desktops |
| [docs/cli.md](docs/cli.md) | `puckctl` commands |
| [docs/steam.md](docs/steam.md) | Steam yield vs override |
| [docs/development.md](docs/development.md) | Tests, coverage, CI, cutting a release |
| [CONTRIBUTING.md](CONTRIBUTING.md) | How to send a change |
| [SECURITY.md](SECURITY.md) | Vulnerability reports |

## License

[MIT](LICENSE). Third-party notices (SDL Triton layout, steam-puck-bridge,
trademarks): [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).

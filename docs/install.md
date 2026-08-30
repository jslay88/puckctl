# Install

puckctl is a user-session daemon plus a StatusNotifier tray. No system
service. Prebuilt x86_64 Linux binaries come from GitHub Releases.

On Arch / CachyOS, after the AUR package is published:

```sh
yay -S puckctl
# or: paru -S puckctl
```

## Requirements

- Linux with a systemd **user** session
- x86_64 for release binaries (other arches: build from source)
- GTK 4 runtime for the tray (binaries are built on Ubuntu 24.04)
- A tray that speaks StatusNotifierItem:
  - KDE Plasma: works
  - GNOME: needs an AppIndicator / Ayatana extension
  - Other SNI hosts (Waybar, etc.) usually work

## Release binary

Download `puckctl-<ver>-x86_64-linux.tar.gz` (or `puckctl-x86_64-linux.tar.gz`)
from [Releases](https://github.com/jslay88/puckctl/releases). Check
`SHA256SUMS`.

```sh
tar -xzf puckctl-*-x86_64-linux.tar.gz
cd puckctl-*-x86_64-linux
./install.sh
```

That copies `puckctl` and `puckctl-tray` to `~/.local/bin`, writes user
units under `~/.config/systemd/user`, and `enable --now` both.
`~/.local/bin` needs to be on your `PATH`. `--prefix` only changes the
binary/data dest; units still go to the user systemd dir unless you set
`UNITDIR`.

```sh
./install.sh --udev        # udev rules (sudo)
./install.sh --no-enable   # files only
./install.sh --uninstall
```

From a git checkout, without building:

```sh
sh scripts/install.sh --download
```

Do not pipe `curl` to `sh` as the main path. Download the tarball, read
`install.sh`, then run it.

## Build from source

You need [Rust 1.98](https://www.rust-lang.org/tools/install)
(`rust-toolchain.toml` / `.tool-versions`) and GTK 4 **headers**.
Debian/Ubuntu: `build-essential`, `libgtk-4-dev`, `pkg-config`. Fedora:
`gtk4-devel`. Arch: `gtk4` and `base-devel`.

From the repo root, with `~/.local/bin` on your `PATH`:

```sh
make enable-tray
```

That runs a release build, copies `puckctl` and `puckctl-tray` to
`~/.local/bin`, installs user units under `~/.config/systemd/user`, and
enables both.

Daemon only (no tray):

```sh
make enable
```

Build without installing:

```sh
make
```

Binaries land in the repo root and in `target/release/`.

## Device access (udev)

Gamepad mode creates a uinput pad and talks to hidraw (and, while Steam
override is on and Steam is running, the USB device node). Your user needs
access to those nodes.

Many machines already have Valve's `steam-devices` package and a uinput
rule. Check first:

```sh
getfacl /dev/uinput
```

If your user is in the ACL (or you can already use Steam Input), you can
skip our rules.

Otherwise, as root:

```sh
sudo make install-udev
```

That installs [`udev/60-puckctl.rules`](../udev/60-puckctl.rules) for
hidraw `28de:1302`–`1305` and uinput, then reloads udev. Unplug and replug
the dongle, or log out and back in, if access does not appear immediately.

Remove those rules with `sudo make uninstall-udev`. Do not do that if you
still need Steam's own udev package.

## Hardware

| Device | USB ID | Status |
|---|---|---|
| Proteus wireless dongle | `28de:1304` | Daily test hardware |
| Nereid dongle | `28de:1305` | Same reports, not daily-tested |
| Wired Triton | `28de:1302` | Same |
| BLE Triton | `28de:1303` | Same |

## After install

`systemctl --user status puckctl.service` should be active when a session is
up. The tray unit is `puckctl-tray.service`.

Mode, Steam override, and the desktop combo persist under
`~/.local/state/puckctl` (or `$XDG_STATE_HOME/puckctl`). The control socket
is `$XDG_RUNTIME_DIR/puckctl.sock`.

Start on login is a tray checkbox. It enables or disables the two user
units.

## Uninstall

Release tree:

```sh
./install.sh --uninstall
./install.sh --uninstall --udev
```

Source checkout:

```sh
make uninstall
sudo make uninstall-udev   # only if you installed our rules
```

That stops the user units and removes the binaries, icon, and unit files.
State under `~/.local/state/puckctl` is left alone so a reinstall keeps
your mode and combo. Delete that directory if you want a clean slate.

## Troubleshooting

- **No tray icon on GNOME:** install an AppIndicator extension and restart
  the shell.
- **Permission denied on uinput or hidraw:** see [Device access](#device-access-udev).
- **Daemon exits immediately:** another instance may already own the
  socket. `puckctl status` talks to a running daemon. `systemctl --user
  restart puckctl.service` if you need a clean start.
- **Tray cannot find `puckctl`:** both binaries need to sit in the same
  directory (the Makefile does this). You can also set `PUCKCTL` to the
  daemon path.

# Changelog

## Unreleased

## 0.1.2 - 2026-08-30

- Gamepad + Steam override holds USB and clones Triton HID onto a virtual
  hidraw (`uhid`, `28de:1302`) so SDL gyro still works
- Turning override off restores Steam desktop and keeps lizard on so
  Steam hidapi does not snap the Puck back to gamepad
- Tray tooltip no longer repeats "Steam has the controller"

## 0.1.1 - 2026-08-30

- Gamepad mode keeps hidraw when Steam override is on, so SDL gyro still works
- CI publishes `puckctl` to the AUR on each GitHub Release

## 0.1.0 - 2026-08-30

- Daemon and CLI (`puckctl`): gamepad or firmware desktop (lizard), Steam
  yield and override, saved button combo back to desktop
- StatusNotifier tray (`puckctl-tray`): mode, override, start on login,
  combo capture window
- systemd user units, optional udev rules, `install.sh` for GitHub Releases
- CI: rustfmt, clippy `-D warnings`, llvm-cov 80% line floor, release compile
- GitHub Release on `v*` tags (x86_64 tarball + `SHA256SUMS`)

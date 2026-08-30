# Security

This daemon talks to USB hidraw/usbfs, uinput, and a Unix socket in
`$XDG_RUNTIME_DIR`. Treat reports that involve those paths as security
issues, not feature requests.

## Report

Email justin.slay@gmail.com. Do not open a public issue for anything that
looks remotely exploitable (socket auth, udev, crafted HID reports, local
privilege).

Include OS, puckctl version (`puckctl -V`), and enough to reproduce. I will
reply when I have looked at it. There is no bug bounty.

## Not in scope

- "Steam can take the controller" (that is [docs/steam.md](docs/steam.md))
- Missing distro packages
- Games that don't see gyro when `/dev/uhid` is not writable (udev)

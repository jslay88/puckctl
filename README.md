# puckctl

Linux userspace driver for the 2nd-generation Steam Controller (the wireless
"Puck" dongle, `28de:1304`). It can run as a virtual Xbox 360 pad, or leave
the firmware in desktop keyboard/mouse ("lizard") mode, including while Steam
is open.

This is a new project, written in Rust. It is **not** a fork of
[steam-puck-bridge](https://github.com/benashby/steam-puck-bridge). That
daemon (Ben Ashby) is how we learned the hidraw / usbfs / uinput path and the
Triton report layout. Protocol constants also come from SDL 3's
`SDL_hidapi_steam_triton.c` (zlib, Valve / Sam Lantinga). See
[THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).

"Steam" and "Steam Controller" are trademarks of Valve Corporation. This is
unofficial and not affiliated with Valve.

## Status

Early. The workspace, protocol crate, and unit tests are in place. The daemon,
CLI socket, usbfs claim/release, and StatusNotifier tray are next.

```sh
cargo test
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check
```

## Crates

| Crate | Role |
|---|---|
| `puckctl-protocol` | Triton constants, state parse, lizard feature reports |
| `puckctl` | daemon + CLI (same binary) |
| `puckctl-tray` | StatusNotifierItem tray |

## Hardware

Proteus dongle `28de:1304` is what we develop on. Nereid `1305` and wired/BLE
Triton (`1302` / `1303`) use the same reports and should work; they are not
the daily test hardware.

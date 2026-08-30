# Development

Workspace members:

| Crate | Role |
|---|---|
| `puckctl-protocol` | Triton constants, report parse, lizard feature reports |
| `puckctl` | Daemon and CLI (one binary) |
| `puckctl-tray` | StatusNotifier tray and combo window |

`puckctl-protocol` forbids `unsafe`. The daemon crate allows it for ioctls
(uinput, hidraw, usbfs). Layouts live in `linux.rs`; wrappers in `sys.rs`.

## Checks

Same gates as CI:

```sh
make check
make cover
```

Or by hand:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace -- --test-threads=1
cargo llvm-cov --workspace --all-targets --fail-under-lines 80 --summary-only -- --test-threads=1
```

`--test-threads=1` is required. Tests isolate `$XDG_RUNTIME_DIR` and
`$XDG_STATE_HOME` behind a mutex; parallel tests would clobber each other.

Unit tests do not claim USB, grab live lizard nodes, or create uinput.
`hw::allowed()` is false in the test binary.

## Display and GTK

`puckctl-tray` has a GTK combo-window test. With `DISPLAY` set you will see
that window for a fraction of a second. That is expected.

CI (and `make check` / `make cover` when `DISPLAY` is unset) wraps tests in
`xvfb-run`. Locally, if you want the same:

```sh
xvfb-run -a --server-args="-screen 0 1280x720x24" cargo test --workspace -- --test-threads=1
```

`GTK_A11Y=none` is set in CI to avoid at-spi noise.

Coverage needs `llvm-tools-preview` (in `rust-toolchain.toml`) and
[`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov). Line floor is
80%. CI runs fmt, clippy, llvm-cov (that **is** the unit-test run), and a
separate `cargo build --release` that uploads a tarball artifact.

## Release

1. Set `version` in the workspace `Cargo.toml` (and changelog).
2. Tag `v` plus that version: `git tag v0.1.0 && git push origin v0.1.0`.
3. `.github/workflows/release.yml` builds on Ubuntu 24.04, packs
   `dist/puckctl-<ver>-x86_64-linux.tar.gz`, and publishes a GitHub Release
   with `SHA256SUMS`. The tag must match Cargo's version.

Local pack (after `cargo build --release --workspace`):

```sh
make dist
# or: sh scripts/pack-release.sh
```

## AUR

`aur/PKGBUILD` is the Arch package. The [AUR workflow](../.github/workflows/aur.yml)
pushes it to [aur.archlinux.org/puckctl](https://aur.archlinux.org/packages/puckctl)
when a GitHub Release is published (and via workflow_dispatch for a tag).

One-time setup:

1. Register at https://aur.archlinux.org/account/register/
2. Make a deploy key (do not reuse your laptop key):

   ```sh
   ssh-keygen -t ed25519 -f aur-puckctl -C "github-actions puckctl" -N ""
   ```

3. Paste `aur-puckctl.pub` into the AUR account SSH keys page.
4. Repo Settings → Secrets and variables → Actions:
   - `AUR_SSH_PRIVATE_KEY` — private key file (including `BEGIN` / `END`)
   - `AUR_USERNAME` — AUR account name
   - `AUR_EMAIL` — AUR account email

Until `AUR_SSH_PRIVATE_KEY` exists, the AUR job is skipped. After the
secrets are set, run **Actions → AUR → Run workflow** with tag `v0.1.0`
to publish the current release, or cut the next tag and it will publish
on its own.

`RUSTUP_TOOLCHAIN=stable` in the PKGBUILD so `makepkg` uses distro rust
instead of the repo `rust-toolchain.toml` pin.

## File size

Keep new modules near 300 lines. Over that is fine when the file is still
one job (report parse, usbfs scan, GTK window). Do not split a single path
across files just to win the counter. Unit tests stay in the same file
under `#[cfg(test)]`; that is normal Rust.

## Layout (daemon)

| Module | Job |
|---|---|
| `cli` | argv |
| `control` | Unix socket, client, daemon spawn |
| `daemon` | Mode, combo, Steam tick, commands |
| `poll` | Event loop |
| `hid` / `usb` / `urb` | hidraw and usbfs I/O |
| `pad` | Virtual Steam Controller (uinput) |
| `grab` | Lizard keyboard/mouse evdev grab |
| `scan` / `slot` | Device discovery |
| `steam` / `steam_cfg` | Steam process + VDF patch |
| `mode` / `combo` / `paths` | Prefs |

## Logging

The daemon logs to stdout (journal when run from systemd). A CLI-spawned
daemon also appends `$XDG_RUNTIME_DIR/puckctl.log`.

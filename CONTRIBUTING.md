# Contributing

Behave as in [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md). Bugs and patches are welcome. Open an issue if the change is large or
touches USB/Steam behavior, so we can agree on the approach first.

## Setup

Rust 1.98 via rustup (`rust-toolchain.toml`) or asdf (`.tool-versions`).
GTK 4 headers to build the tray. See [docs/install.md](docs/install.md).

```sh
make check
```

## Patch

- Match the style around the code you touch. `cargo fmt` is mandatory.
- `cargo clippy --workspace --all-targets -- -D warnings` must stay clean.
- Tests for the behavior you change. Keep them off live hardware: no USB
  claim, no real evdev grab, no `systemctl enable/disable` of the user's
  units. Fake sysfs and injectable helpers are how the existing tests work.
- `--test-threads=1` when you run tests.
- New files around 300 lines; see [docs/development.md](docs/development.md).
- No `Made with Cursor` / agent trailers on commits or PR text.

## PR

Use the pull request template. Say why, not a file dump. Note how you
tested (and on which dongle, if it is hardware-related).

CI runs fmt, clippy, tests, and llvm-cov with an 80% line floor.

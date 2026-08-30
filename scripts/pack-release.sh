#!/bin/sh
# Assemble dist/puckctl-<ver>-x86_64-linux.tar.gz from a release build.
set -eu

ROOT=$(CDPATH= cd "$(dirname "$0")/.." && pwd)
VERSION=${1:-}
ARCH=${ARCH:-x86_64}

if [ -z "$VERSION" ]; then
    VERSION=$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$ROOT/Cargo.toml" | head -n 1)
fi
VERSION=${VERSION#v}
[ -n "$VERSION" ] || {
    echo "pack-release.sh: could not read version" >&2
    exit 1
}

BIN="$ROOT/target/release/puckctl"
TRAY="$ROOT/target/release/puckctl-tray"
[ -x "$BIN" ] && [ -x "$TRAY" ] || {
    echo "pack-release.sh: build first: cargo build --release --workspace" >&2
    exit 1
}

STAGE="$ROOT/dist/puckctl-${VERSION}-${ARCH}-linux"
rm -rf "$STAGE"
mkdir -p "$STAGE/systemd" "$STAGE/udev" "$STAGE/assets"

install -m755 "$BIN" "$STAGE/puckctl"
install -m755 "$TRAY" "$STAGE/puckctl-tray"
install -m755 "$ROOT/scripts/install.sh" "$STAGE/install.sh"
install -m644 "$ROOT/systemd/puckctl.service.in" "$STAGE/systemd/puckctl.service.in"
install -m644 "$ROOT/systemd/puckctl-tray.service.in" "$STAGE/systemd/puckctl-tray.service.in"
install -m644 "$ROOT/udev/60-puckctl.rules" "$STAGE/udev/60-puckctl.rules"
install -m644 "$ROOT/assets/steam-controller.png" "$STAGE/assets/steam-controller.png"
install -m644 "$ROOT/LICENSE" "$STAGE/LICENSE"
install -m644 "$ROOT/THIRD-PARTY-NOTICES.md" "$STAGE/THIRD-PARTY-NOTICES.md"

TAR="puckctl-${VERSION}-${ARCH}-linux.tar.gz"
STABLE="puckctl-${ARCH}-linux.tar.gz"
(
    cd "$ROOT/dist"
    tar -czf "$TAR" "puckctl-${VERSION}-${ARCH}-linux"
    cp -f "$TAR" "$STABLE"
    sha256sum "$TAR" "$STABLE" >SHA256SUMS
)

echo "$ROOT/dist/$TAR"

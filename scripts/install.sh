#!/bin/sh
# Install puckctl + tray into ~/.local and enable the user units.
# Run from a release tarball, or pass --download to fetch the latest GitHub release.
set -eu

REPO="${PUCKCTL_REPO:-jslay88/puckctl}"
PREFIX="${PREFIX:-$HOME/.local}"
ENABLE=1
UDEV=0
UNINSTALL=0
DOWNLOAD=0
ASSET="puckctl-x86_64-linux.tar.gz"

usage() {
    cat <<'EOF'
usage: install.sh [--download] [--udev] [--no-enable] [--uninstall] [--prefix DIR]

  --download   fetch the latest GitHub release (needed unless you are in a release tarball)
  --udev       install udev rules (needs sudo)
  --no-enable  copy files only; do not systemctl enable --now
  --uninstall  stop units and remove installed files
  --prefix DIR install prefix (default: ~/.local)

Environment: PREFIX, PUCKCTL_REPO (default jslay88/puckctl).
EOF
}

while [ $# -gt 0 ]; do
    case "$1" in
        --download) DOWNLOAD=1; shift ;;
        --udev) UDEV=1; shift ;;
        --no-enable) ENABLE=0; shift ;;
        --uninstall) UNINSTALL=1; shift ;;
        --prefix)
            PREFIX=$2
            shift 2
            ;;
        --prefix=*)
            PREFIX=${1#--prefix=}
            shift
            ;;
        -h | --help)
            usage
            exit 0
            ;;
        *)
            echo "install.sh: unknown option: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

BINDIR="${BINDIR:-$PREFIX/bin}"
DATADIR="${DATADIR:-$PREFIX/share/puckctl}"
UNITDIR="${UNITDIR:-$HOME/.config/systemd/user}"
SCRIPT_DIR=$(CDPATH= cd "$(dirname "$0")" && pwd)

die() {
    echo "install.sh: $*" >&2
    exit 1
}

fetch() {
    url=$1
    dest=$2
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "$url" -o "$dest"
    elif command -v wget >/dev/null 2>&1; then
        wget -qO "$dest" "$url"
    else
        die "need curl or wget to download a release"
    fi
}

unit_bindir() {
    case "$BINDIR" in
        "$HOME"/*) printf '%s\n' "%h${BINDIR#"$HOME"}" ;;
        *) printf '%s\n' "$BINDIR" ;;
    esac
}

write_unit() {
    src=$1
    dest=$2
    [ -f "$src" ] || die "missing $src"
    mkdir -p "$(dirname "$dest")"
    sed "s|@BINDIR@|$(unit_bindir)|g" "$src" >"$dest"
}

have_payload() {
    [ -x "$1/puckctl" ] && [ -x "$1/puckctl-tray" ] && [ -f "$1/systemd/puckctl.service.in" ]
}

do_uninstall() {
    if command -v systemctl >/dev/null 2>&1; then
        systemctl --user disable --now puckctl-tray.service 2>/dev/null || true
        systemctl --user disable --now puckctl.service 2>/dev/null || true
    fi
    rm -f "$BINDIR/puckctl" "$BINDIR/puckctl-tray"
    rm -f "$DATADIR/steam-controller.png"
    rmdir "$DATADIR" 2>/dev/null || true
    rm -f "$UNITDIR/puckctl.service" "$UNITDIR/puckctl-tray.service"
    if command -v systemctl >/dev/null 2>&1; then
        systemctl --user daemon-reload 2>/dev/null || true
    fi
    if [ "$UDEV" -eq 1 ]; then
        sudo rm -f /etc/udev/rules.d/60-puckctl.rules
        sudo udevadm control --reload-rules
    fi
    echo "removed puckctl from $PREFIX"
}

install_udev_from() {
    rules=$1/udev/60-puckctl.rules
    [ -f "$rules" ] || die "missing $rules"
    sudo install -m644 "$rules" /etc/udev/rules.d/60-puckctl.rules
    sudo udevadm control --reload-rules
    sudo udevadm trigger
}

do_install() {
    root=$1
    have_payload "$root" || die "release files not found in $root"
    mkdir -p "$BINDIR" "$DATADIR" "$UNITDIR"
    install -m755 "$root/puckctl" "$BINDIR/puckctl"
    install -m755 "$root/puckctl-tray" "$BINDIR/puckctl-tray"
    if [ -f "$root/assets/steam-controller.png" ]; then
        install -m644 "$root/assets/steam-controller.png" "$DATADIR/steam-controller.png"
    fi
    write_unit "$root/systemd/puckctl.service.in" "$UNITDIR/puckctl.service"
    write_unit "$root/systemd/puckctl-tray.service.in" "$UNITDIR/puckctl-tray.service"
    if [ "$UDEV" -eq 1 ]; then
        install_udev_from "$root"
    fi
    if [ "$ENABLE" -eq 1 ] && command -v systemctl >/dev/null 2>&1; then
        systemctl --user daemon-reload
        systemctl --user enable --now puckctl.service
        systemctl --user enable --now puckctl-tray.service
    fi
    echo "installed to $BINDIR"
    echo "put $BINDIR on PATH if 'puckctl' is not found"
}

CLEANUP=
cleanup() {
    if [ -n "$CLEANUP" ]; then
        rm -rf "$CLEANUP"
    fi
}
trap cleanup EXIT

download_release() {
    machine=$(uname -m)
    [ "$machine" = "x86_64" ] || die "prebuilt binaries are x86_64 only (this machine is $machine); build from source"
    [ "$(uname -s)" = "Linux" ] || die "Linux only"
    tmp=$(mktemp -d)
    CLEANUP=$tmp
    base="https://github.com/${REPO}/releases/latest/download"
    echo "fetching ${base}/${ASSET}"
    fetch "${base}/${ASSET}" "$tmp/$ASSET"
    fetch "${base}/SHA256SUMS" "$tmp/SHA256SUMS"
    (
        cd "$tmp"
        grep -F "$ASSET" SHA256SUMS | sha256sum -c -
    )
    tar -xzf "$tmp/$ASSET" -C "$tmp"
    for dir in "$tmp"/puckctl-*-linux; do
        if have_payload "$dir"; then
            printf '%s\n' "$dir"
            return 0
        fi
    done
    die "tarball had no puckctl payload"
}

if [ "$UNINSTALL" -eq 1 ]; then
    do_uninstall
    exit 0
fi

if have_payload "$SCRIPT_DIR"; then
    ROOT=$SCRIPT_DIR
elif [ "$DOWNLOAD" -eq 1 ]; then
    ROOT=$(download_release)
else
    die "no release files next to this script; pass --download or run ./install.sh from a release tarball"
fi

do_install "$ROOT"

PREFIX    ?= $(HOME)/.local
BINDIR    ?= $(PREFIX)/bin
DATADIR   ?= $(PREFIX)/share/puckctl
UNITDIR   ?= $(HOME)/.config/systemd/user
UDEVDIR   ?= /etc/udev/rules.d
DESTDIR   ?=
INSTALL   ?= install
CARGO     ?= cargo

BIN       := puckctl
TRAY      := puckctl-tray
UNIT_IN   := systemd/puckctl.service.in
UNIT      := systemd/puckctl.service
TRAY_UNIT_IN := systemd/puckctl-tray.service.in
TRAY_UNIT := systemd/puckctl-tray.service
RULES     := udev/60-puckctl.rules
ICON      := assets/steam-controller.png

ifeq ($(strip $(BINDIR)),$(strip $(HOME)/.local/bin))
UNIT_BINDIR := %h/.local/bin
else
UNIT_BINDIR := $(BINDIR)
endif

SKIP_RELOAD ?=

all: $(BIN) $(TRAY) $(UNIT) $(TRAY_UNIT)

$(BIN) $(TRAY):
	$(CARGO) build --release --workspace
	cp -f target/release/$(BIN) $(BIN)
	cp -f target/release/$(TRAY) $(TRAY)

$(UNIT): $(UNIT_IN) .bindir-stamp
	sed 's|@BINDIR@|$(UNIT_BINDIR)|g' $< > $@

$(TRAY_UNIT): $(TRAY_UNIT_IN) .bindir-stamp
	sed 's|@BINDIR@|$(UNIT_BINDIR)|g' $< > $@

.bindir-stamp: FORCE
	@echo '$(UNIT_BINDIR)' | cmp -s - $@ 2>/dev/null || echo '$(UNIT_BINDIR)' > $@

FORCE:

install: all
	$(INSTALL) -Dm755 $(BIN) $(DESTDIR)$(BINDIR)/$(BIN)
	$(INSTALL) -Dm755 $(TRAY) $(DESTDIR)$(BINDIR)/$(TRAY)
	$(INSTALL) -Dm644 $(ICON) $(DESTDIR)$(DATADIR)/steam-controller.png
	$(INSTALL) -Dm644 $(UNIT) $(DESTDIR)$(UNITDIR)/puckctl.service
	$(INSTALL) -Dm644 $(TRAY_UNIT) $(DESTDIR)$(UNITDIR)/puckctl-tray.service
ifeq ($(SKIP_RELOAD),)
	systemctl --user daemon-reload
endif

enable: install
	systemctl --user enable --now puckctl.service

enable-tray: enable
	systemctl --user enable --now puckctl-tray.service

uninstall:
	-systemctl --user disable --now puckctl-tray.service
	-systemctl --user disable --now puckctl.service
	rm -f $(DESTDIR)$(BINDIR)/$(BIN)
	rm -f $(DESTDIR)$(BINDIR)/$(TRAY)
	rm -f $(DESTDIR)$(DATADIR)/steam-controller.png
	-rmdir $(DESTDIR)$(DATADIR) 2>/dev/null || true
	rm -f $(DESTDIR)$(UNITDIR)/puckctl.service
	rm -f $(DESTDIR)$(UNITDIR)/puckctl-tray.service
ifeq ($(SKIP_RELOAD),)
	systemctl --user daemon-reload
endif

install-udev:
	$(INSTALL) -Dm644 $(RULES) $(DESTDIR)$(UDEVDIR)/60-puckctl.rules
ifeq ($(SKIP_RELOAD),)
	udevadm control --reload-rules
	udevadm trigger
endif

uninstall-udev:
	rm -f $(DESTDIR)$(UDEVDIR)/60-puckctl.rules
ifeq ($(SKIP_RELOAD),)
	udevadm control --reload-rules
endif

# Headless GTK tests: Xvfb when DISPLAY is unset (same as GitHub Actions).
ifeq ($(DISPLAY),)
  ifneq ($(shell command -v xvfb-run 2>/dev/null),)
    XVFB := xvfb-run -a --server-args="-screen 0 1280x720x24"
  endif
endif

dist: all
	sh scripts/pack-release.sh

check:
	$(CARGO) fmt --all -- --check
	$(CARGO) clippy --workspace --all-targets -- -D warnings
	$(XVFB) $(CARGO) test --workspace -- --test-threads=1

cover:
	$(XVFB) $(CARGO) llvm-cov --workspace --all-targets --fail-under-lines 80 --summary-only -- --test-threads=1

clean:
	rm -f $(BIN) $(TRAY) $(UNIT) $(TRAY_UNIT) .bindir-stamp
	$(CARGO) clean

.PHONY: all dist install enable enable-tray uninstall install-udev uninstall-udev check cover clean FORCE

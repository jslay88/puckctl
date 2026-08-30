# Steam

puckctl and Steam cannot both drive the Puck at once. Default is: if Steam
is running, puckctl gets out of the way.

## Yield (default)

Steam running, override off:

- The daemon does not keep exclusive USB
- Firmware desktop keyboard/mouse comes back
- The tray shows that Steam has the controller
- Games that use Steam Input see the controller the way Steam configured it

When Steam exits, puckctl resumes the last mode you asked for (gamepad or
desktop).

## Override

Tray: **Override Steam**. CLI: `puckctl override on`.

Override stays saved. puckctl does not yield when Steam is running.

Gamepad mode claims USB exclusively so Steam cannot keep desktop mapping
(touchpad mouse, trigger click). The real hidraw node is gone while that
claim is held. puckctl clones the reports onto a virtual hidraw
(`28de:1302`) so SDL hidapi still sees a Steam Controller with gyro.

Desktop (lizard) mode kicks hid, then binds hidraw again so firmware
keyboard/mouse can attach. The puck is also added to Steam's
`controller_blacklist` and desktop layout `769` is emptied.

Steam will usually say the Steam Controller disconnected. That is Steam
losing the device, not puckctl dropping hidraw. Expected. The tray stays
the yellow gamepad icon. The dark icon is only for a controller that is
actually off or unpaired.

Turning override off restores Steam's desktop layout (`769` /
`desktop.vdf`), clears the blacklist, turns firmware lizard back on, and
kicks hid so Steam re-binds. If a backup is missing, `desktop.vdf` is
written anyway so Steam does not stay on the empty layout.

Desktop (lizard) is sent again on reconnect and on a short watchdog while
Steam is open. Otherwise Steam turns lizard off after about a second and
the tray still says desktop.

## What to pick

- **Playing through Steam Input:** leave override off. Let Steam have it.
- **Playing a game that talks to the Puck itself (gyro, hidapi):** gamepad
  mode. Quit Steam, or leave override on so puckctl keeps the USB device
  and exposes a virtual hidraw for the game.
- **Steam is open, you want puckctl's gamepad, not Steam desktop:**
  override on.

## Gyro and rumble

Gyro comes from hidapi on a Triton hidraw node. When Steam is closed,
that is the real `28de:1304` device. When override is on and Steam is
open, puckctl holds USB (so Steam cannot remount desktop) and clones the
same reports onto a virtual hidraw `28de:1302`. SDL treats that PID as a
wired Triton without needing USB interface 2–5.

`/dev/uhid` needs the same kind of uaccess rule as `/dev/uinput`. See
[install.md](install.md#device-access-udev).

Rumble is not a Linux force-feedback effect on the virtual pad. Games that
rumble do it the same way Steam/SDL do (hidapi on the real device). If a
title only shakes an Xbox-style evdev pad, it will not shake ours.

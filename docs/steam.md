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
claim is held. In Steam pad identity, puckctl clones the reports onto a
virtual hidraw (`28de:1302`) so SDL hidapi still sees a Steam Controller
with gyro. Xbox identity skips that clone.

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
- **Playing a game that talks to the Puck itself (gyro, hidapi):**
  `puckctl gamepad steam`. Quit Steam, or leave override on so puckctl
  keeps the USB device and exposes a virtual hidraw for the game.
- **Playing SDL2 / 2s2h / most native ports:** `puckctl gamepad xbox`.
  Those titles only auto-map a well-known Xbox pad. No gyro on that
  device.
- **Steam is open, you want puckctl's gamepad, not Steam desktop:**
  override on.

## Gyro and rumble

Gyro comes from hidapi on a Triton hidraw node, not from the evdev pad.
It only works when the pad identity is Steam (`puckctl gamepad steam`).
Xbox identity is a 360-style evdev pad with no IMU, and puckctl does not
clone a hidraw in that mode.

When Steam is closed, hidapi uses the real `28de:1304` device. When
override is on, identity is Steam, and USB is claimed, puckctl clones the
same reports onto a virtual hidraw `28de:1302`. SDL treats that PID as a
wired Triton without needing USB interface 2–5.

`/dev/uhid` needs the same kind of uaccess rule as `/dev/uinput`. See
[install.md](install.md#device-access-udev).

Rumble is not a Linux force-feedback effect on the virtual pad. Games that
rumble do it the same way Steam/SDL do (hidapi on the real device). If a
title only shakes an Xbox-style evdev pad, it will not shake ours.

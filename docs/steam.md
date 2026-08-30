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

Override stays saved. puckctl does not yield when Steam is running: lizard
stays off, firmware keyboard/mouse stay grabbed, and Steam's desktop
config is patched (see below).

Gamepad mode keeps the real hidraw node so SDL hidapi still sees gyro and
rumble, including while Steam is open. USB is only claimed if hidraw is
already gone (buttons still work; gyro will not).

Override also patches Steam's desktop-config VDF (setting `769` to an empty
official config) so Steam is less likely to fight for lizard/mouse. Originals
are copied under `~/.local/state/puckctl/cfgbak/`. Turning override off
restores those files.

Switching desktop (keyboard/mouse) to gamepad while override is on can
make Steam say the Steam Controller disconnected. That is Steam losing
its mapping, not puckctl dropping hidraw. Expected. The tray stays the
yellow gamepad icon. The dark icon is only for a controller that is
actually off or unpaired.

## What to pick

- **Playing through Steam Input:** leave override off. Let Steam have it.
- **Playing a game that talks to the Puck itself (gyro, hidapi) and Steam
  is closed:** gamepad mode, override does not matter.
- **Steam is open but you want puckctl's gamepad (and gyro):** override on.

## Gyro and rumble

Gyro comes from the real hidraw device (SDL hidapi on `28de:1304`). Gamepad
mode leaves that node in place so motion works with or without Steam.

Rumble is not a Linux force-feedback effect on the virtual pad. Games that
rumble do it the same way Steam/SDL do (hidapi on the real device). If a
title only shakes an Xbox-style evdev pad, it will not shake ours.

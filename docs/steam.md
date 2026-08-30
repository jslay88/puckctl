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

Override stays saved. While it is on **and** Steam is running, puckctl
claims the USB device so Steam cannot. The virtual pad is a Steam
Controller (`28de:1304`), not an Xbox 360 pad.

While Steam is **not** running, override does not steal the device. Gamepad
mode can leave hidraw alone so games (and SDL) still see gyro on the real
controller.

Override also patches Steam's desktop-config VDF (setting `769` to an empty
official config) so Steam is less likely to fight for lizard/mouse. Originals
are copied under `~/.local/state/puckctl/cfgbak/`. Turning override off
restores those files.

Switching desktop (keyboard/mouse) to gamepad while override is on makes
Steam say the Steam Controller disconnected. That is the real hidraw device
going away when puckctl claims USB. Expected. The tray keeps the yellow
gamepad icon; the dark icon is only for a controller that is actually off
or unpaired.

## What to pick

- **Playing through Steam Input:** leave override off. Let Steam have it.
- **Playing a game that talks to the Puck itself (gyro, hidapi) and Steam
  is closed:** gamepad mode, override does not matter.
- **Steam is open but you want puckctl's pad anyway:** override on. You
  lose hidraw for that stretch; that is the trade.

## Gyro and rumble

Gyro comes from the real hidraw device. That is why override only claims
USB while Steam is actually running.

Rumble is not a Linux force-feedback effect on the virtual pad. Games that
rumble do it the same way Steam/SDL do (hidapi on the real device). If a
title only shakes an Xbox-style evdev pad, it will not shake ours.

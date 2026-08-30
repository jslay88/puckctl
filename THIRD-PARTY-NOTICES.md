# Third-party notices

## Triton wire format (SDL 3)

Report IDs, setting numbers, button bits, and field offsets are derived from
SDL 3's `src/joystick/hidapi/SDL_hidapi_steam_triton.c` and
`src/joystick/hidapi/steam/controller_{constants,structs}.h`.

Those files are zlib-licensed, Copyright (C) Valve Corporation / Sam Lantinga.

https://github.com/libsdl-org/SDL

We did not copy SDL source. Constants and layouts are facts about the
hardware.

## steam-puck-bridge

[steam-puck-bridge](https://github.com/benashby/steam-puck-bridge) (MIT,
Copyright 2026 Ben Ashby) is a hidraw-to-uinput daemon for the same
controller. We used it while figuring out lizard mode, usbfs exclusive claim,
and hid-generic rebind. puckctl is a separate codebase and is not a
continuation of that repository.

## Trademarks

"Steam", "Steam Controller", "Steam Deck", and "SteamOS" are trademarks of
Valve Corporation. This project is independent and unofficial.

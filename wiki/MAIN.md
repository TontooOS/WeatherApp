# Weather – Wiki

Weather is the macOS Weather-style app for TontooOS. Sidebar with saved
locations, detail with dynamic condition background, hourly strip, 10-day
forecast and detail tiles.

- Repository: https://github.com/TontooOS/MicroApps (Weather)
- License: TCL v26.1
- Version: 26.1.0

## Feature Index

| Feature | File | Description |
|---|---|---|
| Main index | [MAIN.md](MAIN.md) | This page |
| Rules | [RULE.md](RULE.md) | Development and usage rules |
| Weather | [Weather.md](Weather.md) | Layout, conditions, storage and localization |

## Quick Start

Build and run on TontooOS / Arch Linux (WSL ArchLinux):

```bash
wsl -d archlinux bash -c "cd /mnt/c/Users/arlo1/Documents/TontooMicroApps/Weather && cargo run"
```

Add a location with the `+` button next to the traffic lights, pick the
place from the search dialog. State persists via CoreData with a JSON
fallback. See [Weather.md](Weather.md) for details.

## Changelog

- 2026-09-10: Traffic light crash (red/yellow/green click aborted with `RefCell already borrowed`) fixed in UIKit `dispatch_custom` (commit `8c10991` in the UIKit repo): window handle and action are snapshotted under a short shared borrow, window methods run borrow-free.
- 2026-09-09: Initial wiki, Weather app with sidebar, dynamic backgrounds, MapsKit panel and Accessibility localization.

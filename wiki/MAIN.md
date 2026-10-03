# Weather – Wiki

Weather is the macOS Weather-style app for TontooOS. A TontooUI `Sidebar` with
the saved locations, condition-driven detail with an hourly strip, 10-day
forecast and detail tiles.

- Repository: https://github.com/TontooOS/MicroApps (Weather)
- License: TCL v27.0
- Version: 27.0.0

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

Add a location with the `+` pill next to the traffic lights and pick it from
the search sheet; remove one by right-clicking its sidebar entry and clicking
`Delete` in the context menu, which confirms with an alert. State persists via
CoreData. See [Weather.md](Weather.md) for details.

## Changelog

- 2026-10-03: Locations are removed through a sidebar context menu.
  Right-clicking a sidebar entry selects it and opens a `ContextMenu`
  with one destructive `Delete` row (macOS system red); the row asks
  with the `ActionAlert` before `remove_place` runs. The trash button
  is gone, so the detail header is just the city name. Needs the
  TontooUI additions `Sidebar::item_at` and `Menu::destructive`. See
  [Weather.md](Weather.md).

- 2026-10-02: Ported to the new TontooUI API (wgpu/Vello renderer, `View`
  tree, no UIKit and no GTK). `Sidebar` owns the column and one detail page
  per location; `GradientView` (the single custom view) paints the condition
  gradient behind a `ScrollView`; add-location is a `BasicSheet` with a
  `SearchField`, remove is an `ActionAlert`. `BasicText` grew `size` /
  `weight` in TontooUI for the 72 px temperature. `gtk4`, `glib`, `serde`,
  `serde_json`, `once_cell` and the SDK `UIKit` feature are gone; CoreData
  comes from the SDK re-export. Weekday names, clock format and UV wording
  moved into the lang files. See [Weather.md](Weather.md).
- 2026-09-10: Duplicate places after restart fixed in CoreData `FicoStore` (commit `6084f6c` in the CoreData repo): `fetch_all` excludes soft-deleted rows, `save` purges tombstones.
- 2026-09-10: Traffic light crash (red/yellow/green click aborted with `RefCell already borrowed`) fixed in UIKit `dispatch_custom` (commit `8c10991` in the UIKit repo): window handle and action are snapshotted under a short shared borrow, window methods run borrow-free.
- 2026-09-09: Initial wiki, Weather app with sidebar, dynamic backgrounds, MapsKit panel and Accessibility localization.
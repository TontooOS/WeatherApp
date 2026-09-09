# Weather

macOS Weather-style app for TontooOS: sidebar with saved locations,
dynamic condition background, hourly strip, 10-day forecast, detail
tiles and a MapsKit precipitation map.

## Made for TontooOS

Explore more at https://github.com/TontooOS/Libs

## Wiki

See [wiki/MAIN.md](wiki/MAIN.md).

## Adding to Your Project

Add to your `Cargo.toml`:

```toml
[dependencies]
sdk = { path = "/Library/System/sdk", features = ["Weather"] }
```

Then at the crate root:

```rust
sdk::preinclude!();
use WeatherKit::{WeatherKit};
```

## Build

```bash
wsl -d archlinux bash -c "cd /mnt/c/Users/arlo1/Documents/TontooMicroApps/Weather && cargo build"
```

TBuild packages the app from `tontoo.proj` (`com.tontoo.weather`).

## License

TCL v26.1

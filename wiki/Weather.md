# Weather

Weather recreates the macOS Weather layout with TontooUI and UIKit:
sidebar with saved locations on the left and condition-driven detail
on the right.

## Layout

The root widget (`src/views.rs`, `WeatherRoot`) is a horizontal box.
Default window size is `1280x720`.

| Column | Width | Content |
|---|---|---|
| Sidebar | Fixed `260px` | Traffic lights, `+` add button, saved locations |
| Detail | Flexible | Header, summary, hourly, 10-day, tiles |

The app disables the UIKit window bar (`no_window_bar`), so the
sidebar draws the only traffic lights.

```rust
let mut app = App::with_delegate(lang::t("app.title"), 1120, 700, WeatherDelegate);
app.auto_color_scheme();
app.run();
```

## Sidebar

The sidebar uses UIKit `TrafficLights` and a TontooUI `Button` (`+`)
in the top row, next to the traffic lights where macOS leaves space.
Below, one row per saved location shows the name, current time plus
condition, live temperature and the daily high/low.

- Left click selects the location and refreshes the detail view.
- Right click opens a context menu with a delete option; confirming in
  the dialog removes the location (at least one location is kept).
- The `+` button opens a centered search overlay: a text input for city
  names or postal codes with matching places as text rows below.
  Clicking a row adds it and persists via `store::save_places`.
  `WeatherKit::search_places` resolves names and postal codes
  (Open-Meteo geocoding first, Nominatim second).

## Conditions and Backgrounds

`src/weather.rs` maps WMO weather codes to a `Condition`:

| Code | Condition |
|---|---|
| `0`, `1` | `Clear` |
| `2` | `PartlyCloudy` |
| `3` | `Cloudy` |
| `45`, `48` | `Fog` |
| `51`, `53`, `55`, `56`, `57` | `Drizzle` |
| `61`, `63`, `65`, `66`, `67` | `Rain` |
| `71`, `73`, `75`, `77`, `85`, `86` | `Snow` |
| `80`, `81`, `82` | `Showers` |
| `95`, `96`, `99` | `Thunderstorm` |

### Backgrounds

`background_for(condition, is_day, dark)` returns the gradient pair for
the detail background. Dark pairs echo the macOS Weather artwork; light
pairs are lifted toward `#ececec`.

```rust
let (top, bottom) = background_for(Condition::Rain, true, dark());
```

### Icons

`sf_symbol(condition, is_day)` returns the SF Symbol name
(`sun.max.fill`, `cloud.rain.fill`, ...) rendered through CoreIcon as a
white glyph in `weather::weather_icon_path`. Returns `None` when the
symbol is missing; callers skip the image in that case.

## Data and Storage

`weather::fetch_current_fast` loads current conditions first (single
request, header and sidebar appear immediately), then
`weather::fetch_rest` loads hourly, daily, air quality and sun times in
parallel threads. Results reach the UI through an `mpsc` channel drained
by a `150ms` GTK poll. A process-lifetime cache (10 minute TTL) makes
re-selects instant. Offline requests fall back to demo data and show the
`error.offline` badge.

`store::load_places` and `store::save_places` persist `SavedPlace`
entities (`name`, `country`, `lat`, `lon`, `is_current`) in the CoreData
store `com.tontoo.weather`
(`~/Library/Preferences/com.tontoo.weather/storage.fico`). There is no
other fallback. First launch seeds CoreLocation plus Berlin, New York
and Tokyo.

## Localization

All strings come from the Accessibility `LangStore`
(`src/lang.rs`, `lang::t`). Only `en_us` and `de_de` exist. The
canonical files live in `Resources/lang/` so TBuild copies them into
the `.app` bundle; the root `lang/` copies cover `cargo run`.

```json
{ "lang": "en_us", "translations": { "app.title": "Weather" } }
```

## TBuild

`tontoo.proj` produces the `.app` bundle:

```json
{
  "bundle_id": "com.tontoo.weather",
  "name": "Weather",
  "version": "26.1.0",
  "icon": "Resources/app_icon.png"
}
```

## Cross References

- [MAIN.md](MAIN.md) – wiki entry point
- [RULE.md](RULE.md) – wiki design system

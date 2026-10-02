# Weather

Weather recreates the macOS Weather layout on the TontooUI renderer: the
`Sidebar` element owns the navigation column on the left and the condition
driven detail of the selected location on the right. All strings come from
the Accessibility `LangStore`, saved locations live in CoreData and the
weather data is fetched through WeatherKit on worker threads.

## Window and Entry Point

`src/main.rs` opens the TontooUI window with `run` and nothing else; there is
no window bar, because the sidebar draws the traffic lights.

```rust
fn main() {
  lang::init();
  if let Err(err) = run(&lang::t("app.title"), 1200, 675, views::WeatherApp::new()) {
    eprintln!("weather: {err}");
    std::process::exit(1);
  }
}
```

| Setting | Value |
|---|---|
| Logical size | `1200 x 675` (clamped by the shell to the screen minus `SCREEN_MARGIN`) |
| Window bar | None, the sidebar owns the decoration |
| Drag region | `Sidebar::drag_rect`, so only the traffic row drags the window |
| Backdrop | `App::wants_backdrop` returns true (sidebar pills, sheet search capsule) |

## Sidebar

The sidebar is the TontooUI `Sidebar` element, rebuilt by
`WeatherApp::rebuild` whenever data, the place list or the theme changes.

| Piece | Source |
|---|---|
| Entry per saved location | `SidebarItem::new(place.name, sf_symbol(condition, is_day))`, `mappin` while loading |
| Page per entry | `detail_page` (see below), so selecting an entry shows its own detail |
| Traffic lights | Built into the element, mapped to `WindowCommand` through `Sidebar::press` |
| Add pill | `left_button(0, "plus", ...)`, opens the search sheet |
| Column width | `SIDEBAR_W` (260 px), drag-resizable, collapsible |
| Filter field | The element search row filters saved locations by name |

`rebuild` keeps width, collapse state, selection and the filter query, then
re-applies `set_theme`, `set_glass` and `set_focused` with the live palette.
Colors are baked into the tree at build time; only the theme change path
rebuilds it.

## Detail Page

One page per location, returned by `detail_page` in `src/views.rs`:

```text
GradientView            condition gradient, painted behind everything
└── ScrollView           vertical scroll, integrated scrollbar
    └── VStack (Center)  10 px spacing
        ├── HStack       remove button + city name (26 px semibold)
        ├── BasicText    temperature (72 px, weight 100)
        ├── BasicText    condition (17 px)
        ├── BasicText    H:.. L:.. (15 px, 75% alpha)
        ├── BasicText    summary line (13 px, wrapped to the card width)
        ├── BasicText    "Hourly Forecast" caption
        ├── Background   hourly strip card (8 cells)
        ├── BasicText    "10-Day Forecast" caption
        ├── Background   10-day rows card
        └── VStack       ten detail tiles in rows of two
```

The column width is fixed so every card lines up:

| Constant | Value | Use |
|---|---|---|
| `CARD_W` | 444 px | Outer width of every card |
| `CARD_PAD` | 14 px | Inner padding, `Background::new(Padding::all(..), fill)` |
| `INNER_W` | 416 px | Wrap width of the summary text and the 10-day row |
| `CELL_W` | 46 px | Hourly cell width (label width sets it) |
| `TILE_INNER_W` | 194 px | Tile caption width, tiles land at 218 px |

Stacks never stretch a child to their width, so the width-setting child does
it: hourly cells carry `.width(CELL_W)`, the day name `.width(98.0)`, the low
and high values `.width(46.0)`, tiles `.width(TILE_INNER_W)`. That is why the
10-day row is `98 + 26 + 46 + 160 + 46` wide plus four 10 px gaps.

## Gradient Background

`GradientView` in `src/views.rs` is the only custom `View` in the app:
TontooUI's `Background` fills a solid color, the condition artwork needs a
vertical gradient. It fills the placed rect with `GradientPaint::vertical`
and forwards every event to its child, exactly like `Background` does.

```rust
GradientView::new(ScrollView::new(column), color(top), color(bottom))
```

## Add Location

The `+` pill opens a `BasicSheet<VStack>` (`SheetSize::Half`, white card)
holding five children by index:

| Index | Child |
|---|---|
| 0 | Title text |
| 1 | `SearchField` with `on_change` |
| 2 | Message line (hint, or "no results") |
| 3 | `VStack` of result buttons, replaced on every search |
| 4 | Close button |

- Typing debounces `450 ms` (`SEARCH_DEBOUNCE`) before a worker thread runs
  `WeatherKit::search_places`; Enter searches immediately. Every request
  carries a sequence number and stale results are dropped.
- The field is focused once after opening with a press at its center,
  because `SearchField` has no public focus call.
- Clicking a result writes it into a shared cell; `draw` picks it up,
  dismisses the sheet, dedupes by coordinates and adds the place.
- ESC closes through `BasicSheet::key`.

## Remove Location

The trash button in the detail header sets a shared cell. `draw` opens an
`ActionAlert` with the place name in the message (Cancel plus a red Delete)
and stores the coordinates. The Delete press (event index 1) removes the
place, keeping at least one location, and persists through
`store::save_places`.

## Data Flow

| Piece | Detail |
|---|---|
| Channel | `std::sync::mpsc`, drained with `try_recv` in `draw` (no timer needed, the shell redraws continuously) |
| `Msg::Current` | Stage 1, updates `current` in place so the header appears first |
| `Msg::Place` | Full snapshot from `assemble`, stored in the 10 minute process cache |
| `Msg::MyLocation` | CoreLocation network result, prepends itself once (`is_current`) |
| `Msg::Search` | Search results for the open sheet |
| Offline | `weather::demo_weather` plus the `error.offline` badge |
| Threads | Every network and geolocation call runs on a worker thread; the UI thread never blocks |

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

`background_for(condition, is_day, dark)` returns the gradient pair for the
detail background. Dark pairs echo the macOS Weather artwork; light pairs are
the same hues lifted toward the light surface.

```rust
let (top, bottom) = background_for(condition, is_day, dark);
```

### Icons

`sf_symbol(condition, is_day)` returns the SF Symbol name
(`sun.max.fill`, `cloud.rain.fill`, `trash.fill`, ...) and TontooUI resolves
the artwork through CoreIcon: `SFSymbolImage` in the detail, the sidebar item
icon in the column. A missing symbol simply draws nothing.

## Storage

`store::load_places` and `store::save_places` persist `SavedPlace` entities
(`name`, `country`, `lat`, `lon`, `is_current`) in the CoreData store
`com.tontoo.weather`
(`~/Library/Preferences/com.tontoo.weather/storage.fico`) through the SDK
`CoreData` re-export. Malformed objects are skipped instead of dropping the
store. First launch seeds Berlin, New York and Tokyo; the live position is
prepended once when CoreLocation resolves.

## Localization

All strings come from the Accessibility `LangStore` (`src/lang.rs`,
`lang::t`). Only `en_us` and `de_de` exist. The canonical files live in
`Resources/lang/` so TBuild copies them into the `.app` bundle; the root
`lang/` copies cover `cargo run`.

```json
{ "lang": "en_us", "translations": { "app.title": "Weather" } }
```

| Key group | Purpose |
|---|---|
| `hour.format` | `12h` or `24h`; `lang::uses_24h_clock` drives `clock_time` and `hour_label` |
| `hour.am`, `hour.pm` | Suffixes, empty for the 24 hour locale |
| `day.sun` ... `day.sat` | Short weekday names in the 10-day forecast |
| `uv.low` ... `uv.extreme` | UV index wording in the tiles |
| `cond.*` | Condition labels |
| `unit.*` | Units in the tiles |

## Element Map

| TontooUI element | Where |
|---|---|
| `Sidebar`, `SidebarItem` | Navigation column, traffic lights, filter row |
| `GradientPaint` | Condition gradient (inside the custom `GradientView`) |
| `ScrollView` | Detail column |
| `VStack`, `HStack`, `Padding`, `Background` | Layout and cards |
| `BasicText` | Every text, `size` and `weight` overrides for the display temperature |
| `SFSymbolImage` | Condition symbols |
| `LinearProgress` | Daily temperature bar |
| `Button`, `ButtonStyle` | Add pill result rows, close button, remove button |
| `BasicSheet`, `SheetSize`, `SearchField` | Add-location sheet |
| `ActionAlert`, `AlertButton` | Remove confirmation |
| `ThemeWatcher`, `Palette` | Dark/light, accent, glass stage, window background |

## TBuild

`tontoo.proj` produces the `.app` bundle:

```json
{
  "bundle_id": "com.tontoo.weather",
  "name": "Weather",
  "version": "27.0.0",
  "icon": "Resources/app_icon.png"
}
```

## Cross References

- [MAIN.md](MAIN.md) – wiki entry point
- [RULE.md](RULE.md) – wiki design system
//! Weather root view: macOS Weather-style layout on the TontooUI
//! renderer.
//!
//! Left: the TontooUI `Sidebar` (traffic lights, `+` pill, one entry per
//! saved location). Right: the page slot of the selected entry, holding
//! the detail for that place: a condition gradient background wrapping a
//! `ScrollView` with the current conditions, the hourly strip, the 10-day
//! forecast and the detail tiles. Adding a location opens a `BasicSheet`
//! with a search field, removing one a `ActionAlert`.
//!
//! All network and geolocation work runs on worker threads and reports
//! through an `mpsc` channel that `draw` drains, so the UI thread never
//! blocks. Data, theme and place-list changes rebuild the view tree with
//! the colors of the current theme baked in.

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use crate::lang;
use crate::store::{load_places, save_places, SavedPlace};
use crate::weather::{
  self, background_for, clock_time, compass, condition_for, day_name, fmt_temp, hour_label,
  label_key, sf_symbol, Condition, FoundPlace, PlaceWeather,
};
use crate::WeatherKit::{CurrentWeather, ForecastDay, HourPoint};

use crate::TontooUI::elements::{
  ActionAlert, AlertButton, Align, Background, BasicSheet, BasicText, Button, ButtonStyle,
  GradientPaint, HStack, LinearProgress, Padding, ScrollView, SearchField, SheetSize, Sidebar,
  SidebarItem, SFSymbolImage, TextAlignment, TrafficAction, VStack, View, BUTTON_BG_LIGHT,
};
use crate::TontooUI::kurbo::{Affine, Rect};
use crate::TontooUI::peniko::Fill;
use crate::TontooUI::renderer::window::{App, CursorKind, Key, Viewport, WindowCommand};
use crate::TontooUI::renderer::{FontSystem, ImageLoader};
use crate::TontooUI::theme::{Theme, ThemeMode, ThemeWatcher};
use crate::TontooUI::{Color, Scene};

/// Sidebar column width in logical px.
const SIDEBAR_W: f32 = 260.0;
/// Content column width in logical px (the widest card plus its padding).
const CARD_W: f32 = 444.0;
/// Card inner padding in logical px.
const CARD_PAD: f32 = 14.0;
/// Inner width shared by the cards in logical px.
const INNER_W: f32 = CARD_W - CARD_PAD * 2.0;
/// Hourly cell width in logical px.
const CELL_W: f32 = 46.0;
/// Detail tile inner width in logical px (plus its own padding).
const TILE_INNER_W: f32 = 194.0;
/// Debounce before a typed query hits the network.
const SEARCH_DEBOUNCE: Duration = Duration::from_millis(450);

// ── colors ───────────────────────────────────────────────────────────

/// Parse a `#RRGGBB` string into an opaque color.
fn color(hex: &str) -> Color {
  let bytes = hex.as_bytes();
  if bytes.len() == 7 && bytes[0] == b'#' {
    let channel = |at: usize| u8::from_str_radix(&hex[at..at + 2], 16).unwrap_or(0);
    Color::from_rgb8(channel(1), channel(3), channel(5))
  } else {
    Color::from_rgb8(0x1b, 0x20, 0x22)
  }
}

/// Same hue with a scaled alpha (captions on the gradient).
fn fade(value: Color, alpha: f32) -> Color {
  let c = value.to_rgba8();
  Color::from_rgba8(
    c.r,
    c.g,
    c.b,
    ((c.a as f32) * alpha.clamp(0.0, 1.0)).round() as u8,
  )
}

/// Linear blend between two colors.
fn mix(from: Color, to: Color, t: f32) -> Color {
  let (a, b) = (from.to_rgba8(), to.to_rgba8());
  let channel = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t.clamp(0.0, 1.0)).round() as u8;
  Color::from_rgba8(
    channel(a.r, b.r),
    channel(a.g, b.g),
    channel(a.b, b.b),
    channel(a.a, b.a),
  )
}

/// Body text on the detail gradient: white in dark mode, near-black in light.
fn detail_text(dark: bool) -> Color {
  if dark {
    color("#FFFFFF")
  } else {
    color("#1E1E1E")
  }
}

/// Translucent card fill over the gradient.
fn card_fill(dark: bool) -> Color {
  if dark {
    Color::from_rgba8(255, 255, 255, 20)
  } else {
    Color::from_rgba8(255, 255, 255, 166)
  }
}

fn precip_color(dark: bool) -> Color {
  if dark {
    color("#7DD3FC")
  } else {
    color("#0277BD")
  }
}

fn offline_color(dark: bool) -> Color {
  if dark {
    color("#FFD60A")
  } else {
    color("#B26A00")
  }
}

/// Cold to hot endpoints for the daily temperature bars.
fn temp_cold() -> Color {
  color("#4DA3FF")
}

fn temp_hot() -> Color {
  color("#FFD60A")
}

// ── gradient background view ─────────────────────────────────────────

/// Vertical gradient panel sized to the placed rect, painted behind its
/// child. TontooUI's `Background` fills a solid color only, so the
/// condition artwork needs this one custom view; everything below it is
/// plain TontooUI elements.
struct GradientView {
  top: Color,
  bottom: Color,
  child: Box<dyn View>,
  rect: (f32, f32, f32, f32),
}

impl GradientView {
  fn new(child: impl View + 'static, top: Color, bottom: Color) -> Self {
    Self {
      top,
      bottom,
      child: Box::new(child),
      rect: (0.0, 0.0, 0.0, 0.0),
    }
  }
}

impl View for GradientView {
  fn measure(&mut self, fonts: &mut FontSystem) -> (f32, f32) {
    self.child.measure(fonts)
  }

  fn place(&mut self, fonts: &mut FontSystem, x: f32, y: f32, width: f32, height: f32) {
    self.rect = (x, y, width.max(0.0), height.max(0.0));
    self.child.place(fonts, x, y, width, height);
  }

  fn draw(&mut self, scene: &mut Scene, fonts: &mut FontSystem, images: &mut ImageLoader<'_>) {
    let (x, y, width, height) = self.rect;
    if width > 0.0 && height > 0.0 {
      let scale = fonts.scale as f64;
      let brush =
        GradientPaint::vertical(vec![self.top, self.bottom]).brush(x, y, width, height, fonts.scale);
      scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        &brush,
        None,
        &Rect::new(
          x as f64 * scale,
          y as f64 * scale,
          (x + width) as f64 * scale,
          (y + height) as f64 * scale,
        ),
      );
    }
    self.child.draw(scene, fonts, images);
  }

  fn mouse_down(&mut self, x: f64, y: f64) {
    self.child.mouse_down(x, y);
  }

  fn mouse_up(&mut self, x: f64, y: f64) {
    self.child.mouse_up(x, y);
  }

  fn set_hover(&mut self, x: f32, y: f32) {
    self.child.set_hover(x, y);
  }

  fn mouse_wheel(&mut self, dx: f64, dy: f64) {
    self.child.mouse_wheel(dx, dy);
  }

  fn text(&mut self, text: &str) {
    self.child.text(text);
  }

  fn key(&mut self, key: Key) -> bool {
    self.child.key(key)
  }

  fn as_any_mut(&mut self) -> &mut dyn Any {
    self
  }
}

// ── shared state ─────────────────────────────────────────────────────

struct Shared {
  places: Vec<SavedPlace>,
  selected: usize,
  data: Vec<Option<PlaceWeather>>,
  fetching: HashSet<usize>,
}

impl Shared {
  fn fresh() -> Self {
    let places = load_places();
    let count = places.len();
    Self {
      places,
      selected: 0,
      data: vec![None; count],
      fetching: HashSet::new(),
    }
  }

  fn clamp_selected(&mut self) {
    let last = self.places.len().saturating_sub(1);
    self.selected = self.selected.min(last);
  }
}

enum Msg {
  Place(usize, PlaceWeather),
  Current(usize, CurrentWeather),
  MyLocation(String, String, f64, f64),
  Search(u64, Vec<FoundPlace>),
}

// ── detail page pieces ───────────────────────────────────────────────

/// Rounded translucent card around one detail section.
fn card(child: impl View + 'static, fill: Color) -> Background {
  Background::new(Padding::all(child, CARD_PAD), fill).radius(14.0)
}

/// Centered caption above a card.
fn caption(text: &str, color: Color) -> BasicText {
  BasicText::new(text)
    .size(11.0)
    .weight(600.0)
    .foreground_color(color)
}

/// Precipitation chance as a percentage, empty below 20 percent.
fn chance_text(chance: i32) -> String {
  if chance >= 20 {
    format!("{chance}%")
  } else {
    String::new()
  }
}

/// One hourly cell: hour, symbol, precipitation chance, temperature.
fn hourly_cell(hour: &HourPoint, index: usize, is_day: bool, text: Color, dim: Color, precip: Color) -> VStack {
  let title = if index == 0 {
    lang::t("detail.hourly_now")
  } else {
    hour_label(&hour.time)
  };
  let symbol = SFSymbolImage::new(sf_symbol(condition_for(hour.weather_code, ""), is_day))
    .size(28.0)
    .color(text);
  VStack::new()
    .spacing(4.0)
    .align(Align::Center)
    .child(
      BasicText::new(title)
        .size(12.0)
        .weight(600.0)
        .width(CELL_W)
        .alignment(TextAlignment::Center)
        .foreground_color(dim),
    )
    .child(symbol)
    .child(
      BasicText::new(chance_text(hour.precip_probability_pct.unwrap_or(0)))
        .size(11.0)
        .weight(600.0)
        .width(CELL_W)
        .alignment(TextAlignment::Center)
        .foreground_color(precip),
    )
    .child(
      BasicText::new(fmt_temp(hour.temperature_c))
        .size(15.0)
        .weight(600.0)
        .width(CELL_W)
        .alignment(TextAlignment::Center)
        .foreground_color(text),
    )
}

/// Hourly strip inside one card, or a placeholder while loading.
fn hourly_card(snapshot: &PlaceWeather, dark: bool) -> Background {
  let text = detail_text(dark);
  let dim = fade(text, 0.75);
  let precip = precip_color(dark);
  let mut strip = HStack::new().spacing(8.0).align(Align::Leading);
  if snapshot.hourly.is_empty() {
    strip = strip.child(BasicText::new("...").size(13.0).foreground_color(dim));
  } else {
    for (index, hour) in snapshot.hourly.iter().take(8).enumerate() {
      strip = strip.child(hourly_cell(hour, index, snapshot.current.is_day, text, dim, precip));
    }
  }
  card(strip, card_fill(dark))
}

/// One 10-day row: day, symbol, low, temperature bar, chance, high. The
/// bar fill shows how warm the day is relative to the coldest and
/// warmest day of the week.
fn daily_row(
  day: &ForecastDay,
  index: usize,
  coldest: f64,
  span: f64,
  text: Color,
  dim: Color,
  precip: Color,
) -> HStack {
  let symbol = SFSymbolImage::new(sf_symbol(condition_for(day.weather_code, ""), true))
    .size(26.0)
    .color(text);
  let average = (day.temp_min_c + day.temp_max_c) / 2.0;
  let warmth = ((average - coldest) / span).clamp(0.0, 1.0);
  let mut bar = LinearProgress::new()
    .speed(6.0)
    .fill(mix(temp_cold(), temp_hot(), warmth as f32))
    .track_color(fade(text, 0.25));
  bar.set_progress(warmth);
  HStack::new()
    .spacing(10.0)
    .align(Align::Center)
    .child(
      BasicText::new(day_name(&day.date, index))
        .size(14.0)
        .width(98.0)
        .foreground_color(text),
    )
    .child(symbol)
    .child(
      BasicText::new(format!("{}°", day.temp_min_c.round() as i64))
        .size(14.0)
        .width(46.0)
        .alignment(TextAlignment::Trailing)
        .foreground_color(dim),
    )
    .child(bar)
    .child(
      BasicText::new(chance_text(day.precip_probability_pct.unwrap_or(0)))
        .size(11.0)
        .weight(600.0)
        .width(46.0)
        .alignment(TextAlignment::Center)
        .foreground_color(precip),
    )
    .child(
      BasicText::new(format!("{}°", day.temp_max_c.round() as i64))
        .size(14.0)
        .weight(600.0)
        .width(46.0)
        .alignment(TextAlignment::Trailing)
        .foreground_color(text),
    )
}

/// 10-day forecast rows inside one card.
fn daily_card(snapshot: &PlaceWeather, dark: bool) -> Background {
  let text = detail_text(dark);
  let dim = fade(text, 0.75);
  let precip = precip_color(dark);
  let coldest = snapshot
    .daily
    .iter()
    .map(|day| day.temp_min_c)
    .fold(f64::INFINITY, f64::min);
  let warmest = snapshot
    .daily
    .iter()
    .map(|day| day.temp_max_c)
    .fold(f64::NEG_INFINITY, f64::max);
  let coldest = if coldest.is_finite() { coldest } else { 0.0 };
  let span = (warmest - coldest).max(1.0);
  let mut rows = VStack::new().spacing(8.0);
  for (index, day) in snapshot.daily.iter().enumerate() {
    rows = rows.child(daily_row(day, index, coldest, span, text, dim, precip));
  }
  card(rows, card_fill(dark))
}

/// One detail tile: caption plus value inside its own rounded card.
fn tile(title: &str, value: &str, dark: bool) -> Background {
  let text = detail_text(dark);
  let dim = fade(text, 0.7);
  Background::new(
    Padding::all(
      VStack::new()
        .spacing(3.0)
        .align(Align::Leading)
        .child(
          BasicText::new(title)
            .size(11.0)
            .width(TILE_INNER_W)
            .foreground_color(dim),
        )
        .child(
          BasicText::new(value)
            .size(19.0)
            .weight(300.0)
            .width(TILE_INNER_W)
            .foreground_color(text),
        ),
      12.0,
    ),
    card_fill(dark),
  )
  .radius(12.0)
}

/// Detail tiles in rows of two.
fn tiles_block(titles: &[String], values: &[String], dark: bool) -> VStack {
  let mut block = VStack::new().spacing(8.0).align(Align::Center);
  let mut row = HStack::new().spacing(8.0).align(Align::Leading);
  let mut filled = 0usize;
  for (title, value) in titles.iter().zip(values.iter()) {
    row = row.child(tile(title, value, dark));
    filled += 1;
    if filled % 2 == 0 {
      block = block.child(row);
      row = HStack::new().spacing(8.0).align(Align::Leading);
    }
  }
  if filled % 2 == 1 {
    block = block.child(row);
  }
  block
}

/// The ten localized tile captions, in `tile_values` order.
fn tile_titles() -> Vec<String> {
  [
    "detail.tile_wind",
    "detail.tile_humidity",
    "detail.tile_feels",
    "detail.tile_uv",
    "detail.tile_visibility",
    "detail.tile_sunrise",
    "detail.tile_sunset",
    "detail.tile_air",
    "detail.tile_pressure",
    "detail.tile_precip",
  ]
  .iter()
  .map(|key| lang::t(key))
  .collect()
}

/// Tile values for the current snapshot, in `tile_titles` order.
fn tile_values(snapshot: &PlaceWeather) -> Vec<String> {
  let current = &snapshot.current;
  let uv = current.uv_index.unwrap_or(0.0);
  let uv_word = if uv < 3.0 {
    "uv.low"
  } else if uv < 6.0 {
    "uv.moderate"
  } else if uv < 8.0 {
    "uv.high"
  } else if uv < 11.0 {
    "uv.very_high"
  } else {
    "uv.extreme"
  };
  vec![
    format!(
      "{:.0} {} {}",
      current.wind_kmh,
      lang::t("unit.kmh"),
      compass(current.wind_direction_deg)
    ),
    format!("{}%", current.humidity_pct),
    fmt_temp(current.feels_like_c),
    format!("{uv:.0} {}", lang::t(uv_word)),
    match current.visibility_m {
      Some(meters) => format!("{:.0} {}", meters / 1000.0, lang::t("unit.km")),
      None => "--".to_string(),
    },
    snapshot
      .sunrise
      .map(|(hour, minute)| clock_time(hour, minute))
      .unwrap_or_else(|| "--".to_string()),
    snapshot
      .sunset
      .map(|(hour, minute)| clock_time(hour, minute))
      .unwrap_or_else(|| "--".to_string()),
    match snapshot
      .air
      .as_ref()
      .and_then(|air| air.us_aqi.or(air.european_aqi))
    {
      Some(aqi) => format!("{aqi}"),
      None => "--".to_string(),
    },
    format!("{:.0} {}", current.pressure_hpa, lang::t("unit.hpa")),
    format!("{:.1} mm", current.precipitation_mm),
  ]
}

/// Detail page for one place: gradient background plus the scrollable
/// condition column.
fn detail_page(
  place: &SavedPlace,
  snapshot: Option<&PlaceWeather>,
  theme: &Theme,
  remove_flag: Rc<Cell<Option<(f64, f64)>>>,
) -> GradientView {
  let dark = theme.mode == ThemeMode::Dark;
  let text = detail_text(dark);
  let dim = fade(text, 0.75);
  let condition = snapshot
    .map(|data| condition_for(data.current.weather_code, &data.current.condition))
    .unwrap_or(Condition::Unknown);
  let is_day = snapshot.map(|data| data.current.is_day).unwrap_or(true);
  let (top, bottom) = background_for(condition, is_day, dark);
  let (top, bottom) = (color(top), color(bottom));

  let mut column = VStack::new().spacing(10.0).align(Align::Center);

  // Header: remove button plus the city name.
  let mut remove = Button::new("")
    .icon("trash.fill")
    .style(ButtonStyle::Plain)
    .hover_effect(true);
  remove.set_palette(Color::TRANSPARENT, text);
  let remove = {
    let flag = remove_flag.clone();
    let (lat, lon) = (place.lat, place.lon);
    remove.on_press(move || flag.set(Some((lat, lon))))
  };
  column = column.child(
    HStack::new()
      .spacing(10.0)
      .align(Align::Center)
      .child(remove)
      .child(
        BasicText::new(place.name.clone())
          .size(26.0)
          .weight(600.0)
          .foreground_color(text),
      ),
  );

  let Some(data) = snapshot else {
    column = column
      .child(BasicText::new("--°").size(72.0).weight(100.0).foreground_color(text))
      .child(BasicText::new("...").size(17.0).foreground_color(dim));
    return GradientView::new(ScrollView::new(column), top, bottom);
  };

  let condition_text = lang::t(label_key(condition));
  let gusts = data.current.wind_gusts_kmh.unwrap_or(data.current.wind_kmh);
  let (high, low) = match data.daily.first() {
    Some(day) => (fmt_temp(day.temp_max_c), fmt_temp(day.temp_min_c)),
    None => ("--°".to_string(), "--°".to_string()),
  };
  let summary = format!(
    "{} {} {:.0} {}",
    condition_text,
    lang::t("detail.summary_wind"),
    gusts,
    compass(data.current.wind_direction_deg),
  );

  column = column
    .child(
      BasicText::new(fmt_temp(data.current.temperature_c))
        .size(72.0)
        .weight(100.0)
        .foreground_color(text),
    )
    .child(BasicText::new(condition_text.clone()).size(17.0).foreground_color(text))
    .child(
      BasicText::new(format!(
        "{}:{}  {}:{}",
        lang::t("detail.high"),
        high,
        lang::t("detail.low"),
        low
      ))
      .size(15.0)
      .foreground_color(dim),
    );

  if data.offline {
    column = column.child(
      BasicText::new(lang::t("error.offline"))
        .size(12.0)
        .weight(600.0)
        .foreground_color(offline_color(dark)),
    );
  }

  column = column
    .child(
      BasicText::new(summary)
        .size(13.0)
        .width(INNER_W)
        .alignment(TextAlignment::Center)
        .foreground_color(text),
    )
    .child(caption(&lang::t("detail.hourly_title"), dim))
    .child(hourly_card(data, dark))
    .child(caption(&lang::t("detail.daily_title"), dim))
    .child(daily_card(data, dark))
    .child(tiles_block(&tile_titles(), &tile_values(data), dark));

  GradientView::new(ScrollView::new(column), top, bottom)
}

// ── app ──────────────────────────────────────────────────────────────

/// The Weather app: one `Sidebar` holding every saved location plus the
/// modal search sheet and the remove confirmation.
pub struct WeatherApp {
  shared: Shared,
  tx: Sender<Msg>,
  rx: Receiver<Msg>,
  sidebar: Sidebar,
  search: BasicSheet<VStack>,
  delete: ActionAlert,
  add_flag: Rc<Cell<bool>>,
  remove_flag: Rc<Cell<Option<(f64, f64)>>>,
  pending_remove: Option<(f64, f64)>,
  picked: Rc<RefCell<Option<FoundPlace>>>,
  close_flag: Rc<Cell<bool>>,
  query: Rc<RefCell<String>>,
  query_changed: Rc<Cell<Option<Instant>>>,
  searching: u64,
  results_seq: u64,
  focus_field: bool,
  text_cursor: bool,
  dirty: bool,
  watcher: ThemeWatcher,
  focused: bool,
  bg: Color,
  command: Option<WindowCommand>,
}

impl WeatherApp {
  pub fn new() -> Self {
    let (tx, rx) = std::sync::mpsc::channel::<Msg>();
    let add_flag = Rc::new(Cell::new(false));
    let remove_flag = Rc::new(Cell::new(None));
    let picked = Rc::new(RefCell::new(None));
    let close_flag = Rc::new(Cell::new(false));
    let query = Rc::new(RefCell::new(String::new()));
    let query_changed = Rc::new(Cell::new(None));
    let watcher = ThemeWatcher::new();
    let theme = watcher.theme();

    // Search sheet content: title, field, message, results, close.
    let field_query = query.clone();
    let field_changed = query_changed.clone();
    let content = VStack::new()
      .spacing(12.0)
      .align(Align::Leading)
      .child(
        BasicText::new(lang::t("search.title"))
          .size(16.0)
          .weight(600.0)
          .foreground_color(color("#272727")),
      )
      .child(SearchField::new(lang::t("search.placeholder")).on_change(move |text| {
        *field_query.borrow_mut() = text.to_string();
        field_changed.set(Some(Instant::now()));
      }))
      .child(
        BasicText::new(lang::t("search.hint"))
          .size(12.0)
          .foreground_color(color("#6E6E73")),
      )
      .child(VStack::new().spacing(6.0))
      .child({
        let flag = close_flag.clone();
        Button::new(lang::t("search.close"))
          .style(ButtonStyle::Bordered)
          .on_press(move || flag.set(true))
      });

    let delete = ActionAlert::new(
      lang::t("delete.title"),
      lang::t("delete.message"),
      AlertButton::cancel(lang::t("delete.cancel")),
      AlertButton::ok(lang::t("delete.remove")).color(color("#FF3B30")),
    );

    let mut app = Self {
      shared: Shared::fresh(),
      tx,
      rx,
      sidebar: Sidebar::new(Vec::new()).width(SIDEBAR_W),
      search: BasicSheet::new(content)
        .size(SheetSize::Half)
        .background(color("#FFFFFF")),
      delete,
      add_flag,
      remove_flag,
      pending_remove: None,
      picked,
      close_flag,
      query,
      query_changed,
      searching: 0,
      results_seq: 0,
      focus_field: false,
      text_cursor: false,
      dirty: true,
      watcher,
      focused: true,
      bg: theme.palette().bg,
      command: None,
    };
    app.rebuild(theme);
    app.fetch_all();
    app.fetch_my_location();
    app
  }

  /// Rebuild the sidebar and its detail pages from the shared state,
  /// baking in the colors of the current theme. Keeps width, selection,
  /// collapse state and the sidebar filter query.
  fn rebuild(&mut self, theme: Theme) {
    let dark = theme.mode == ThemeMode::Dark;
    let width = self.sidebar.width_value();
    let collapsed = self.sidebar.is_collapsed();
    let query = self.sidebar.search_text().to_string();
    let accent = theme.accent.color();

    self.shared.clamp_selected();
    let selected = self.shared.selected;
    let items: Vec<SidebarItem> = (0..self.shared.places.len())
      .map(|index| {
        let snapshot = self.shared.data.get(index).and_then(|slot| slot.as_ref());
        let symbol = match snapshot {
          Some(data) => sf_symbol(
            condition_for(data.current.weather_code, &data.current.condition),
            data.current.is_day,
          ),
          None => "mappin",
        };
        SidebarItem::new(self.shared.places[index].name.clone(), symbol)
      })
      .collect();

    let mut sidebar = Sidebar::new(items)
      .width(width)
      .search_field(true)
      .collapsible(true);
    for index in 0..self.shared.places.len() {
      let place = self.shared.places[index].clone();
      let snapshot = self.shared.data.get(index).and_then(|slot| slot.clone());
      sidebar = sidebar.page(detail_page(
        &place,
        snapshot.as_ref(),
        &theme,
        self.remove_flag.clone(),
      ));
    }
    let add_flag = self.add_flag.clone();
    sidebar = sidebar.left_button(0, "plus", move || add_flag.set(true));
    sidebar.set_collapsed(collapsed);
    sidebar.set_glass(theme.mode, theme.glass);
    sidebar.set_theme(accent, dark);
    sidebar.set_focused(self.focused);
    let _ = sidebar.select(selected);
    if !query.is_empty() {
      sidebar.set_search_text(query);
    }
    self.sidebar = sidebar;
    self.dirty = false;
  }

  /// Drain worker messages into the shared state.
  fn drain(&mut self) {
    while let Ok(msg) = self.rx.try_recv() {
      match msg {
        Msg::Current(index, current) => {
          let Some(slot) = self.shared.data.get_mut(index) else {
            continue;
          };
          match slot.as_mut() {
            Some(existing) => {
              existing.current = current;
              existing.offline = false;
            }
            None => {
              *slot = Some(PlaceWeather {
                current,
                hourly: Vec::new(),
                daily: Vec::new(),
                air: None,
                sunrise: None,
                sunset: None,
                offline: false,
              });
            }
          }
          self.dirty = true;
        }
        Msg::Place(index, weather) => {
          if index < self.shared.data.len() {
            self.shared.data[index] = Some(weather);
            self.shared.fetching.remove(&index);
            self.dirty = true;
          }
        }
        Msg::MyLocation(name, country, lat, lon) => {
          self.insert_my_location(name, country, lat, lon);
        }
        Msg::Search(seq, found) => {
          if seq >= self.results_seq {
            self.results_seq = seq;
            self.set_results(found);
          }
        }
      }
    }
  }

  /// Live position: reuse a nearby entry, else prepend it once.
  fn insert_my_location(&mut self, name: String, country: String, lat: f64, lon: f64) {
    let nearby = self
      .shared
      .places
      .iter()
      .position(|place| (place.lat - lat).abs() < 0.05 && (place.lon - lon).abs() < 0.05);
    match nearby {
      Some(index) => {
        if !self.shared.places[index].is_current {
          self.shared.places[index].is_current = true;
          save_places(&self.shared.places);
          self.dirty = true;
        }
      }
      None => {
        if self.shared.places.iter().any(|place| place.is_current) {
          return;
        }
        let mut mine = SavedPlace::new(name, country, lat, lon);
        mine.is_current = true;
        self.shared.places.insert(0, mine);
        self.shared.data.insert(0, None);
        self.shared.selected += 1;
        save_places(&self.shared.places);
        self.dirty = true;
        self.load_index(0);
      }
    }
  }

  /// Staged load: a cache hit sends the whole snapshot at once, otherwise
  /// current conditions go out first and the rest follows.
  fn load_index(&mut self, index: usize) {
    if self.shared.fetching.contains(&index) {
      return;
    }
    let Some(place) = self.shared.places.get(index).cloned() else {
      return;
    };
    if let Some(hit) = weather::cached_place(place.lat, place.lon) {
      let _ = self.tx.send(Msg::Place(index, hit));
      return;
    }
    self.shared.fetching.insert(index);
    let tx = self.tx.clone();
    std::thread::spawn(move || {
      let Some(current) = weather::fetch_current_fast(place.lat, place.lon) else {
        let _ = tx.send(Msg::Place(index, weather::demo_weather()));
        return;
      };
      let _ = tx.send(Msg::Current(index, current.clone()));
      let rest = weather::fetch_rest(place.lat, place.lon);
      let full = weather::assemble(current, rest, false);
      weather::store_place(place.lat, place.lon, &full);
      let _ = tx.send(Msg::Place(index, full));
    });
  }

  fn fetch_all(&mut self) {
    for index in 0..self.shared.places.len() {
      self.load_index(index);
    }
  }

  /// Resolve the live position on a worker thread, then prepend it.
  fn fetch_my_location(&mut self) {
    let tx = self.tx.clone();
    std::thread::spawn(move || {
      if let Ok(location) = crate::CoreLocation::CoreLocation::new().get_location() {
        let name = location
          .city
          .clone()
          .unwrap_or_else(|| crate::lang::t("sidebar.my_location"));
        let _ = tx.send(Msg::MyLocation(
          name,
          location.country.clone().unwrap_or_default(),
          location.coordinates.latitude,
          location.coordinates.longitude,
        ));
      }
    });
  }

  // ── search sheet ───────────────────────────────────────────────────

  /// Send the current query to the network.
  fn dispatch_search(&mut self) {
    let query = self.query.borrow().trim().to_string();
    self.searching += 1;
    self.results_seq = self.searching;
    if query.chars().count() < 2 {
      self.set_results(Vec::new());
      return;
    }
    let seq = self.searching;
    let tx = self.tx.clone();
    std::thread::spawn(move || {
      let found = weather::search_places(&query);
      let _ = tx.send(Msg::Search(seq, found));
    });
  }

  /// Swap the result rows and the message line inside the sheet.
  fn set_results(&mut self, found: Vec<FoundPlace>) {
    let query = self.query.borrow().trim().to_string();
    let message = if query.chars().count() < 2 {
      lang::t("search.hint")
    } else if found.is_empty() {
      lang::t("search.empty")
    } else {
      String::new()
    };
    let mut rows = VStack::new().spacing(6.0).align(Align::Leading);
    for place in found {
      let picked = self.picked.clone();
      let chosen = place.clone();
      let label = if place.country.is_empty() {
        place.name.clone()
      } else {
        format!("{}, {}", place.name, place.country)
      };
      rows = rows.child(
        Button::new(label)
          .style(ButtonStyle::Bordered)
          .on_press(move || *picked.borrow_mut() = Some(chosen.clone())),
      );
    }
    let content = self.search.child_mut();
    if let Some(text) = content.child_mut::<BasicText>(2) {
      text.set_text(message);
    }
    if let Some(slot) = content.child_mut::<VStack>(3) {
      *slot = rows;
    }
  }

  /// Focus the sheet field once it has a rect: `SearchField` has no
  /// public focus call, so a press at its center does what a click does.
  fn focus_search_field(&mut self) {
    if !self.focus_field || !self.search.is_visible() {
      return;
    }
    let rect = self
      .search
      .child_mut()
      .child_mut::<SearchField>(1)
      .map(|field| field.rect());
    let Some((x, y, width, height)) = rect else {
      return;
    };
    if width <= 0.0 || height <= 0.0 {
      return;
    }
    if let Some(field) = self.search.child_mut().child_mut::<SearchField>(1) {
      field.mouse_down((x + width / 2.0) as f64, (y + height / 2.0) as f64);
    }
    self.focus_field = false;
  }

  /// Show the search sheet with a clean field.
  fn open_search(&mut self) {
    self.query.borrow_mut().clear();
    self.query_changed.set(None);
    self.results_seq = self.searching;
    if let Some(field) = self.search.child_mut().child_mut::<SearchField>(1) {
      field.set_text("");
    }
    self.set_results(Vec::new());
    self.search.show();
    self.focus_field = true;
  }

  /// Add a place (dedupe by coordinates) and select it.
  fn add_place(&mut self, place: FoundPlace) {
    let existing = self
      .shared
      .places
      .iter()
      .position(|saved| (saved.lat - place.lat).abs() < 0.05 && (saved.lon - place.lon).abs() < 0.05);
    let index = match existing {
      Some(index) => index,
      None => {
        let index = self.shared.places.len();
        self.shared.places.push(SavedPlace::new(
          place.name.clone(),
          place.country.clone(),
          place.lat,
          place.lon,
        ));
        self.shared.data.push(None);
        save_places(&self.shared.places);
        index
      }
    };
    self.shared.selected = index;
    self.dirty = true;
    self.load_index(index);
  }

  /// Remove a location, keeping at least one.
  fn remove_place(&mut self, lat: f64, lon: f64) {
    if self.shared.places.len() <= 1 {
      return;
    }
    let Some(index) = self
      .shared
      .places
      .iter()
      .position(|saved| (saved.lat - lat).abs() < 0.0001 && (saved.lon - lon).abs() < 0.0001)
    else {
      return;
    };
    self.shared.places.remove(index);
    self.shared.data.remove(index);
    self.shared.clamp_selected();
    save_places(&self.shared.places);
    self.dirty = true;
  }

  /// Apply the flags the view tree set during the last frame.
  fn apply_flags(&mut self) {
    if self.add_flag.replace(false) {
      self.open_search();
    }
    if self.close_flag.replace(false) {
      self.search.dismiss();
    }
    let chosen = self.picked.borrow_mut().take();
    if let Some(place) = chosen {
      self.search.dismiss();
      self.add_place(place);
    }
    if let Some((lat, lon)) = self.remove_flag.take() {
      let name = self
        .shared
        .places
        .iter()
        .find(|saved| (saved.lat - lat).abs() < 0.0001 && (saved.lon - lon).abs() < 0.0001)
        .map(|saved| saved.name.clone())
        .unwrap_or_default();
      self.delete
        .set_message(lang::t("delete.message").replace("%name%", &name));
      self.pending_remove = Some((lat, lon));
      self.delete.show();
    }
  }

  /// Dispatch the debounced query once the pause elapsed.
  fn poll_query(&mut self) {
    let Some(changed) = self.query_changed.get() else {
      return;
    };
    if changed.elapsed() < SEARCH_DEBOUNCE {
      return;
    }
    self.query_changed.set(None);
    self.dispatch_search();
  }

  /// Theme the sheet contents: the field needs the glass stage, the
  /// result buttons a light card fill.
  fn theme_sheet(&mut self, theme: &Theme, accent: Color) {
    self.search.set_theme(theme.mode == ThemeMode::Dark);
    self.delete
      .set_theme(theme.mode, accent, theme.glass);
    let dark = theme.mode == ThemeMode::Dark;
    let content = self.search.child_mut();
    if let Some(field) = content.child_mut::<SearchField>(1) {
      field.set_theme(theme.mode, accent, theme.glass);
      field.set_focused(self.focused);
    }
    if let Some(button) = content.child_mut::<Button>(4) {
      button.set_theme(accent, dark);
      button.set_palette(BUTTON_BG_LIGHT, color("#272727"));
      button.set_focused(self.focused);
    }
    if let Some(rows) = content.child_mut::<VStack>(3) {
      for index in 0..rows.len() {
        if let Some(button) = rows.child_mut::<Button>(index) {
          button.set_theme(accent, dark);
          button.set_palette(BUTTON_BG_LIGHT, color("#272727"));
          button.set_focused(self.focused);
        }
      }
    }
  }
}

impl Default for WeatherApp {
  fn default() -> Self {
    Self::new()
  }
}

impl App for WeatherApp {
  fn draw(
    &mut self,
    scene: &mut Scene,
    fonts: &mut FontSystem,
    images: &mut ImageLoader<'_>,
    viewport: Viewport,
    time_secs: f64,
  ) {
    if self.watcher.poll(time_secs) {
      self.dirty = true;
    }
    self.watcher.set_focused(self.focused, time_secs);
    let palette = self.watcher.palette(time_secs);
    self.bg = palette.bg;
    let theme = self.watcher.theme();
    let dark = theme.mode == ThemeMode::Dark;

    self.drain();
    self.poll_query();
    self.apply_flags();
    if self.dirty {
      self.rebuild(theme);
    }
    self.theme_sheet(&theme, palette.accent);

    self.sidebar.set_theme(palette.accent, dark);
    self.sidebar.set_glass(theme.mode, theme.glass);
    self.sidebar.set_focused(self.focused);
    // Full-bleed: the sidebar owns the traffic lights, no titlebar.
    self.sidebar
      .place(fonts, viewport.x, viewport.y, viewport.width, viewport.height);
    self.sidebar.draw(scene, fonts, images);

    if self.search.is_visible() {
      self.search
        .set_viewport(viewport.x, viewport.y, viewport.width, viewport.height);
      self.search.draw(scene, fonts, images);
      self.focus_search_field();
    }
    if self.delete.is_visible() {
      self.delete
        .set_viewport(viewport.x, viewport.y, viewport.width, viewport.height);
      self.delete.draw(scene, fonts, images);
    }
  }

  fn background(&self) -> Color {
    self.bg
  }

  fn wants_backdrop(&self) -> bool {
    // Sidebar pills and the sheet search capsule need the blur pass.
    true
  }

  fn drag_region(&self) -> Option<(f32, f32, f32, f32)> {
    Some(self.sidebar.drag_rect())
  }

  fn poll_window_command(&mut self) -> Option<WindowCommand> {
    self.command.take()
  }

  fn cursor(&self, x: f64, y: f64) -> CursorKind {
    if self.search.is_visible() {
      return if self.text_cursor {
        CursorKind::Text
      } else {
        CursorKind::Default
      };
    }
    if self.sidebar.wants_resize_cursor(x, y) {
      CursorKind::ResizeColumn
    } else if self.text_cursor {
      CursorKind::Text
    } else {
      CursorKind::Default
    }
  }

  fn mouse_down(&mut self, x: f64, y: f64) {
    if self.search.is_visible() {
      self.search.mouse_down(x, y);
      return;
    }
    if self.delete.is_visible() {
      self.delete.mouse_down(x, y);
      return;
    }
    // Traffic lights first: a hit must never reach the page.
    if let Some(action) = self.sidebar.press(x, y) {
      self.command = Some(match action {
        TrafficAction::Close => WindowCommand::Close,
        TrafficAction::Minimize => WindowCommand::Minimize,
        TrafficAction::Maximize => WindowCommand::ToggleMaximize,
      });
      return;
    }
    self.sidebar.mouse_down(x, y);
  }

  fn mouse_move(&mut self, x: f64, y: f64) {
    self.sidebar.mouse_move(x, y);
    if self.search.is_visible() {
      let mut hovered = false;
      if let Some(field) = self.search.child_mut().child_mut::<SearchField>(1) {
        field.set_hover(x as f32, y as f32);
        hovered = field.wants_text_cursor();
      }
      self.text_cursor = hovered;
      return;
    }
    self.sidebar.set_hover(x as f32, y as f32);
    self.text_cursor = self.sidebar.search_text_cursor();
  }

  fn set_focused(&mut self, focused: bool) {
    self.focused = focused;
    self.sidebar.set_focused(focused);
    self.search.set_focused(focused);
    self.delete.set_focused(focused);
  }

  fn mouse_up(&mut self, x: f64, y: f64) {
    if self.search.is_visible() {
      self.search.mouse_up(x, y);
      return;
    }
    if self.delete.is_visible() {
      if let Some(event) = self.delete.mouse_up(x, y) {
        if event.index == 1 {
          if let Some((lat, lon)) = self.pending_remove.take() {
            self.remove_place(lat, lon);
          }
        }
        self.delete.dismiss();
      }
      return;
    }
    self.sidebar.mouse_up(x, y);
  }

  fn mouse_wheel(&mut self, dx: f64, dy: f64) {
    if self.search.is_visible() || self.delete.is_visible() {
      return;
    }
    self.sidebar.mouse_wheel(dx, dy);
  }

  fn text(&mut self, text: &str) {
    if self.search.is_visible() {
      if let Some(field) = self.search.child_mut().child_mut::<SearchField>(1) {
        field.type_text(text);
      }
      return;
    }
    self.sidebar.page_text(text);
  }

  fn key(&mut self, key: Key) {
    if self.search.is_visible() {
      if self.search.key(key) {
        return;
      }
      if key == Key::Enter {
        self.query_changed.set(None);
        self.dispatch_search();
        return;
      }
      if let Some(field) = self.search.child_mut().child_mut::<SearchField>(1) {
        field.key(key);
      }
      return;
    }
    if self.delete.is_visible() {
      if key == Key::Escape {
        self.delete.dismiss();
      }
      return;
    }
    self.sidebar.page_key(key);
  }
}
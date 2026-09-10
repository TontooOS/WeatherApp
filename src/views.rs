//! Weather root view: macOS Weather-style layout.
//!
//! Left: TontooUI/UIKit sidebar (traffic lights + add button on top,
//! saved locations with live temperature below). Right: detail with a
//! background gradient that follows the current condition (clear, cloudy,
//! rain, snow, thunderstorm, ...), hourly strip, 10-day forecast and
//! detail tiles.

use crate::lang;
use crate::store::{load_places, save_places, SavedPlace};
use crate::weather::{
  self, background_for, compass, condition_for, day_name, fmt_temp, hour_label, label_key,
  sf_symbol, Condition, FoundPlace, PlaceWeather,
};
use crate::TontooUI::Button;
use crate::UIKit::prelude::*;
use crate::UIKit::widget::{next_widget_id, WidgetId};
use gtk::prelude::*;
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

const SF_PRO: &str = "SF Pro Display";
const SIDEBAR_W: i32 = 260;

fn dark() -> bool {
  crate::UIKit::app::current_color_scheme()
    .unwrap_or_else(ColorScheme::detect_system)
    == ColorScheme::Dark
}

fn pal() -> (&'static str, &'static str) {
  if dark() {
    ("#F5F5F7", "#A1A1A6")
  } else {
    ("#1E1E1E", "#6E6E73")
  }
}

fn apply_class(widget: &impl IsA<gtk::Widget>, class: &str, rules: &str) {
  let provider = gtk::CssProvider::new();
  provider.load_from_string(&format!(".{class} {{ {rules} }}"));
  gtk::StyleContext::add_provider_for_display(
    &gtk::gdk::Display::default().expect("gdk display"),
    &provider,
    gtk::STYLE_PROVIDER_PRIORITY_USER as u32,
  );
  widget.add_css_class(class);
}

fn label(text: &str, size: u32, weight: &str, color: &str) -> gtk::Label {
  let widget = gtk::Label::new(None);
  widget.set_use_markup(true);
  widget.set_markup(&format!(
    "<span font_desc=\"{} {} {}\" foreground=\"{}\">{}</span>",
    SF_PRO,
    weight,
    size,
    color,
    glib::markup_escape_text(text),
  ));
  widget
}

fn card() -> gtk::Box {
  let outer = gtk::Box::new(gtk::Orientation::Vertical, 0);
  if dark() {
    apply_class(&outer, "wx-card", "background-color: rgba(255,255,255,0.08); border-radius: 12px;");
  } else {
    apply_class(&outer, "wx-card", "background-color: rgba(255,255,255,0.65); border-radius: 12px;");
  }
  outer.set_margin_start(12);
  outer.set_margin_end(12);
  outer
}

fn local_time() -> String {
  let secs = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .map(|d| d.as_secs())
    .unwrap_or(0);
  let mins = (secs / 60) % (24 * 60);
  let hour = (mins / 60) as i32;
  let minute = (mins % 60) as i32;
  if hour == 0 {
    format!("12:{minute:02} AM")
  } else if hour < 12 {
    format!("{hour}:{minute:02} AM")
  } else if hour == 12 {
    format!("12:{minute:02} PM")
  } else {
    format!("{}:{minute:02} PM", hour - 12)
  }
}

fn clock_time(hour: u32, minute: u32) -> String {
  if hour == 0 {
    format!("12:{minute:02} AM")
  } else if hour < 12 {
    format!("{hour}:{minute:02} AM")
  } else if hour == 12 {
    format!("12:{minute:02} PM")
  } else {
    format!("{}:{minute:02} PM", hour - 12)
  }
}

// ── shared state ──────────────────────────────────────────────────────

#[derive(Clone)]
struct Shared {
  places: Rc<RefCell<Vec<SavedPlace>>>,
  selected: Rc<RefCell<usize>>,
  data: Rc<RefCell<Vec<Option<PlaceWeather>>>>,
  fetching: Rc<RefCell<HashSet<usize>>>,
  tx: std::sync::mpsc::Sender<Msg>,
}

impl Shared {
  fn fresh(tx: std::sync::mpsc::Sender<Msg>) -> Self {
    let places = load_places();
    let count = places.len();
    Self {
      places: Rc::new(RefCell::new(places)),
      selected: Rc::new(RefCell::new(0)),
      data: Rc::new(RefCell::new(vec![None; count])),
      fetching: Rc::new(RefCell::new(HashSet::new())),
      tx,
    }
  }
}

enum Msg {
  Place(usize, PlaceWeather),
  Search(u64, Vec<FoundPlace>),
}

// ── sidebar rows ──────────────────────────────────────────────────────

struct RowH {
  row: gtk::Box,
  sub: gtk::Label,
  temp: gtk::Label,
  hilo: gtk::Label,
}

fn paint_selection(rows: &[RowH], selected: usize) {
  for (index, handle) in rows.iter().enumerate() {
    if index == selected {
      apply_class(
        &handle.row,
        "wx-sel",
        "background-color: rgba(10,132,255,0.85); border-radius: 10px;",
      );
    } else {
      handle.row.remove_css_class("wx-sel");
    }
  }
}

fn refresh_rows(rows: &[RowH], shared: &Shared) {
  let places = shared.places.borrow();
  let data = shared.data.borrow();
  let selected = *shared.selected.borrow();
  let time = local_time();
  for (index, handle) in rows.iter().enumerate() {
    let Some(place) = places.get(index) else { continue };
    let is_selected = index == selected;
    let fg = if is_selected { "#FFFFFF" } else { pal().0 };
    let secondary = if is_selected { "#FFFFFF" } else { pal().1 };
    let cond_text = match data.get(index).and_then(|d| d.as_ref()) {
      Some(weather) => {
        let cond = condition_for(weather.current.weather_code, &weather.current.condition);
        lang::t(label_key(cond))
      }
      None => place.country.clone(),
    };
    handle.sub.set_markup(&format!(
      "<span font_desc=\"{} normal 11\" foreground=\"{}\">{}</span>",
      SF_PRO,
      secondary,
      glib::markup_escape_text(&format!("{time}  {cond_text}")),
    ));
    let temp_text = match data.get(index).and_then(|d| d.as_ref()) {
      Some(weather) => fmt_temp(weather.current.temperature_c),
      None => "--°".to_string(),
    };
    handle.temp.set_markup(&format!(
      "<span font_desc=\"{} 300 24\" foreground=\"{}\">{}</span>",
      SF_PRO,
      fg,
      glib::markup_escape_text(&temp_text),
    ));
    let hilo_text = match data.get(index).and_then(|d| d.as_ref()).and_then(|w| w.daily.first()) {
      Some(day) => format!("H:{} L:{}", fmt_temp(day.temp_max_c), fmt_temp(day.temp_min_c)),
      None => String::new(),
    };
    handle.hilo.set_markup(&format!(
      "<span font_desc=\"{} normal 11\" foreground=\"{}\">{}</span>",
      SF_PRO,
      secondary,
      glib::markup_escape_text(&hilo_text),
    ));
  }
}

// ── detail handles ────────────────────────────────────────────────────

struct DetailH {
  bg: gtk::Box,
  bg_provider: gtk::CssProvider,
  city: gtk::Label,
  temp: gtk::Label,
  cond: gtk::Label,
  hilo: gtk::Label,
  summary: gtk::Label,
  offline: gtk::Label,
  hourly: gtk::Box,
  daily: gtk::Box,
  tiles: gtk::Grid,
}

fn tile(parent: &gtk::Grid, col: i32, row: i32, title: &str) -> (gtk::Label, gtk::Label) {
  let box_ = gtk::Box::new(gtk::Orientation::Vertical, 2);
  if dark() {
    apply_class(&box_, "wx-tile", "background-color: rgba(255,255,255,0.08); border-radius: 12px; padding: 10px;");
  } else {
    apply_class(&box_, "wx-tile", "background-color: rgba(255,255,255,0.65); border-radius: 12px; padding: 10px;");
  }
  box_.set_hexpand(true);
  let title_label = label(title, 11, "normal", pal().1);
  title_label.set_halign(gtk::Align::Start);
  let value_label = label("--", 20, "300", pal().0);
  value_label.set_halign(gtk::Align::Start);
  box_.append(&title_label);
  box_.append(&value_label);
  parent.attach(&box_, col, row, 1, 1);
  (value_label, title_label)
}

fn set_tile_value(value_label: &gtk::Label, value: &str) {
  value_label.set_markup(&format!(
    "<span font_desc=\"{} 300 20\" foreground=\"{}\">{}</span>",
    SF_PRO,
    pal().0,
    glib::markup_escape_text(value),
  ));
}

fn paint_background(detail: &DetailH, condition: Condition, is_day: bool) {
  let (top, bottom) = background_for(condition, is_day, dark());
  detail.bg_provider.load_from_string(&format!(
    ".wx-bg {{ background-image: linear-gradient(to bottom, {top}, {bottom}); }}"
  ));
}

fn refresh_detail(detail: &DetailH, shared: &Shared) {
  let selected = *shared.selected.borrow();
  let places = shared.places.borrow();
  let Some(place) = places.get(selected).cloned() else { return };
  let data = shared.data.borrow();
  let weather = data.get(selected).and_then(|d| d.clone());
  drop(data);

  detail.city.set_markup(&format!(
    "<span font_desc=\"{} 600 28\" foreground=\"#FFFFFF\">{}</span>",
    SF_PRO,
    glib::markup_escape_text(&place.name),
  ));

  let Some(weather) = weather else {
    detail.temp.set_markup(&format!("<span font_desc=\"{SF_PRO} 100 72\" foreground=\"#FFFFFF\">--°</span>"));
    detail.cond.set_markup(&format!("<span font_desc=\"{SF_PRO} normal 17\" foreground=\"#FFFFFF\">...</span>"));
    return;
  };

  let cond = condition_for(weather.current.weather_code, &weather.current.condition);
  paint_background(detail, cond, weather.current.is_day);
  let cond_text = lang::t(label_key(cond));

  detail.temp.set_markup(&format!(
    "<span font_desc=\"{SF_PRO} 100 72\" foreground=\"#FFFFFF\">{}</span>",
    glib::markup_escape_text(&fmt_temp(weather.current.temperature_c)),
  ));
  detail.cond.set_markup(&format!(
    "<span font_desc=\"{SF_PRO} normal 17\" foreground=\"#FFFFFF\">{}</span>",
    glib::markup_escape_text(&cond_text),
  ));

  let (hi, lo) = match weather.daily.first() {
    Some(day) => (fmt_temp(day.temp_max_c), fmt_temp(day.temp_min_c)),
    None => ("--°".to_string(), "--°".to_string()),
  };
  detail.hilo.set_markup(&format!(
    "<span font_desc=\"{SF_PRO} normal 15\" foreground=\"#FFFFFF\">H:{hi}  L:{lo}</span>"
  ));
  detail.offline.set_visible(weather.offline);

  let gusts = weather.current.wind_gusts_kmh.unwrap_or(weather.current.wind_kmh);
  detail.summary.set_markup(&format!(
    "<span font_desc=\"{SF_PRO} normal 13\" foreground=\"#FFFFFF\">{}  {} {} {:.0} {}</span>",
    glib::markup_escape_text(&cond_text),
    glib::markup_escape_text(&lang::t("detail.summary_wind")),
    "",
    gusts,
    glib::markup_escape_text(&lang::t("unit.kmh")),
  ));

  // Hourly strip.
  while let Some(child) = detail.hourly.first_child() {
    detail.hourly.remove(&child);
  }
  for (index, hour) in weather.hourly.iter().take(12).enumerate() {
    let cell = gtk::Box::new(gtk::Orientation::Vertical, 2);
    cell.set_size_request(64, -1);
    let title = if index == 0 {
      lang::t("detail.hourly_now")
    } else {
      hour_label(&hour.time)
    };
    let hour_label = label(&title, 12, "600", "#FFFFFF");
    hour_label.set_halign(gtk::Align::Center);
    cell.append(&hour_label);
    let symbol = sf_symbol(condition_for(hour.weather_code, ""), weather.current.is_day);
    if let Some(path) = weather::weather_icon_path(symbol, 28) {
      let image = gtk::Image::from_file(&path);
      image.set_pixel_size(28);
      image.set_halign(gtk::Align::Center);
      cell.append(&image);
    }
    let prob = hour.precip_probability_pct.unwrap_or(0);
    let prob_text = if prob >= 20 { format!("{prob}%") } else { String::new() };
    let prob_label = label(&prob_text, 11, "600", "#7DD3FC");
    prob_label.set_halign(gtk::Align::Center);
    cell.append(&prob_label);
    let temp_label = label(&fmt_temp(hour.temperature_c), 15, "600", "#FFFFFF");
    temp_label.set_halign(gtk::Align::Center);
    cell.append(&temp_label);
    detail.hourly.append(&cell);
  }

  // 10-day forecast with temperature range bars.
  while let Some(child) = detail.daily.first_child() {
    detail.daily.remove(&child);
  }
  let week_min = weather.daily.iter().map(|d| d.temp_min_c).fold(f64::INFINITY, f64::min);
  let week_max = weather.daily.iter().map(|d| d.temp_max_c).fold(f64::NEG_INFINITY, f64::max);
  let span = (week_max - week_min).max(1.0);
  for (index, day) in weather.daily.iter().enumerate() {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.set_margin_start(12);
    row.set_margin_end(12);
    row.set_margin_top(7);
    row.set_margin_bottom(7);
    let name = label(&day_name(&day.date, index), 14, "normal", "#FFFFFF");
    name.set_size_request(64, -1);
    name.set_halign(gtk::Align::Start);
    row.append(&name);
    let icon_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    icon_box.set_size_request(44, -1);
    let symbol = sf_symbol(condition_for(day.weather_code, ""), true);
    if let Some(path) = weather::weather_icon_path(symbol, 20) {
      let image = gtk::Image::from_file(&path);
      image.set_pixel_size(20);
      image.set_halign(gtk::Align::Center);
      icon_box.append(&image);
    }
    if day.precip_probability_pct.unwrap_or(0) >= 20 {
      let prob = label(&format!("{}%", day.precip_probability_pct.unwrap_or(0)), 10, "600", "#7DD3FC");
      prob.set_halign(gtk::Align::Center);
      icon_box.append(&prob);
    }
    row.append(&icon_box);
    let min_label = label(&format!("{:.0}°", day.temp_min_c.round()), 14, "normal", "rgba(255,255,255,0.7)");
    min_label.set_size_request(40, -1);
    min_label.set_halign(gtk::Align::End);
    row.append(&min_label);
    let track = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    track.set_hexpand(true);
    track.set_valign(gtk::Align::Center);
    track.set_size_request(-1, 4);
    apply_class(&track, "wx-track", "background-color: rgba(255,255,255,0.25); border-radius: 2px;");
    let left = ((day.temp_min_c - week_min) / span * 100.0).clamp(0.0, 100.0);
    let right = ((week_max - day.temp_max_c) / span * 100.0).clamp(0.0, 100.0);
    let fill = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    fill.set_hexpand(true);
    fill.set_margin_start(left as i32);
    fill.set_margin_end(right as i32);
    fill.set_size_request(-1, 4);
    apply_class(
      &fill,
      "wx-fill",
      "background-image: linear-gradient(to right, #4DA3FF, #FFD60A); border-radius: 2px;",
    );
    track.append(&fill);
    row.append(&track);
    let max_label = label(&format!("{:.0}°", day.temp_max_c.round()), 14, "600", "#FFFFFF");
    max_label.set_size_request(40, -1);
    max_label.set_halign(gtk::Align::End);
    row.append(&max_label);
    detail.daily.append(&row);
    if index + 1 < weather.daily.len() {
      let sep = gtk::Separator::new(gtk::Orientation::Horizontal);
      sep.set_opacity(0.25);
      detail.daily.append(&sep);
    }
  }

  // Tiles.
  let children: Vec<gtk::Widget> = {
    let mut out = Vec::new();
    let mut child = detail.tiles.first_child();
    while let Some(widget) = child {
      out.push(widget.clone());
      child = widget.next_sibling();
    }
    out
  };
  let mut values: Vec<gtk::Label> = Vec::new();
  for tile_box in children {
    let mut inner = tile_box.first_child();
    let mut last: Option<gtk::Label> = None;
    while let Some(widget) = inner {
      if let Ok(found) = widget.clone().downcast::<gtk::Label>() {
        last = Some(found);
      }
      inner = widget.next_sibling();
    }
    if let Some(value) = last {
      values.push(value);
    }
  }
  let uv = weather.current.uv_index.unwrap_or(0.0);
  let uv_word = if uv < 3.0 {
    "Low"
  } else if uv < 6.0 {
    "Moderate"
  } else if uv < 8.0 {
    "High"
  } else if uv < 11.0 {
    "Very High"
  } else {
    "Extreme"
  };
  let tile_texts = [
    format!("{:.0} {} {}", weather.current.wind_kmh, lang::t("unit.kmh"), compass(weather.current.wind_direction_deg)),
    format!("{}%", weather.current.humidity_pct),
    fmt_temp(weather.current.feels_like_c),
    format!("{uv:.0} {uv_word}"),
    match weather.current.visibility_m {
      Some(m) => format!("{:.0} {}", m / 1000.0, lang::t("unit.km")),
      None => "--".to_string(),
    },
    weather.sunrise.map(|(h, m)| clock_time(h, m)).unwrap_or_else(|| "--".to_string()),
    weather.sunset.map(|(h, m)| clock_time(h, m)).unwrap_or_else(|| "--".to_string()),
    match weather.air.as_ref().and_then(|a| a.us_aqi.or(a.european_aqi)) {
      Some(aqi) => format!("{aqi}"),
      None => "--".to_string(),
    },
    format!("{:.0} {}", weather.current.pressure_hpa, lang::t("unit.hpa")),
    format!("{:.1} mm", weather.current.precipitation_mm),
  ];
  for (value_label, text) in values.iter().zip(tile_texts.iter()) {
    set_tile_value(value_label, text);
  }
}

// ── search dialog ─────────────────────────────────────────────────────

fn open_search(
  shared: Shared,
  tx: std::sync::mpsc::Sender<Msg>,
  rebuild: Rc<dyn Fn()>,
) {
  let window = gtk::Window::new();
  window.set_title(Some(&lang::t("search.title")));
  window.set_default_size(380, 460);
  window.set_modal(true);
  apply_class(&window, "wx-dialog", "background-color: #1d1d1d; border-radius: 12px;");

  let outer = gtk::Box::new(gtk::Orientation::Vertical, 8);
  outer.set_margin_top(16);
  outer.set_margin_bottom(16);
  outer.set_margin_start(16);
  outer.set_margin_end(16);

  let entry = gtk::SearchEntry::new();
  entry.set_placeholder_text(Some(&lang::t("search.placeholder")));
  outer.append(&entry);
  let hint = label(&lang::t("search.hint"), 12, "normal", pal().1);
  hint.set_halign(gtk::Align::Start);
  outer.append(&hint);

  let results = gtk::ListBox::new();
  results.set_selection_mode(gtk::SelectionMode::None);
  let scroll = gtk::ScrolledWindow::new();
  scroll.set_child(Some(&results));
  scroll.set_vexpand(true);
  outer.append(&scroll);

  let close_button = Button::new(lang::t("search.close"));
  let close_gtk = close_button.to_gtk();
  let window_close = window.clone();
  let close_g = gtk::GestureClick::new();
  close_g.connect_released(move |_, _, _, _| window_close.close());
  close_gtk.add_controller(close_g);
  outer.append(&close_gtk);

  window.set_child(Some(&outer));
  window.present();

  let seq = Rc::new(RefCell::new(0u64));
  let search_state: Rc<RefCell<Vec<FoundPlace>>> = Rc::new(RefCell::new(Vec::new()));
  entry.connect_search_changed({
    let seq = seq.clone();
    let tx = tx.clone();
    move |input| {
      let query = input.text().to_string();
      *seq.borrow_mut() += 1;
      let current = *seq.borrow();
      if query.trim().len() < 2 {
        let _ = tx.send(Msg::Search(current, Vec::new()));
        return;
      }
      let tx = tx.clone();
      std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(450));
        let found = weather::search_places(&query);
        let _ = tx.send(Msg::Search(current, found));
      });
    }
  });

  // Poll for search results on the main thread. Results are forwarded
  // by the shared poll loop via the sink registry below.
  let results_poll = results.clone();
  let state_poll = search_state.clone();
  let shared_poll = shared.clone();
  let rebuild_poll = rebuild.clone();
  let window_poll = window.clone();
  SEARCH_SINK.with(|sink| {
    *sink.borrow_mut() = Some(Box::new(move |found: Vec<FoundPlace>| {
      *state_poll.borrow_mut() = found.clone();
      while let Some(child) = results_poll.first_child() {
        results_poll.remove(&child);
      }
      if found.is_empty() {
        let empty = label(&lang::t("search.empty"), 13, "normal", pal().1);
        empty.set_halign(gtk::Align::Center);
        results_poll.append(&empty);
        return;
      }
      for place in found {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.set_margin_top(8);
        row.set_margin_bottom(8);
        row.set_margin_start(8);
        row.set_margin_end(8);
        let name = label(&place.name, 14, "600", pal().0);
        name.set_hexpand(true);
        name.set_halign(gtk::Align::Start);
        row.append(&name);
        let country = label(&place.country, 12, "normal", pal().1);
        row.append(&country);
        let add = label(&lang::t("search.add"), 13, "600", "#0A84FF");
        row.append(&add);
        results_poll.append(&row);
        let click = gtk::GestureClick::new();
        click.set_button(1);
        let shared = shared_poll.clone();
        let rebuild = rebuild_poll.clone();
        let window = window_poll.clone();
        let found_place = place.clone();
        click.connect_released(move |_, _, _, _| {
          // Dedupe: select the existing entry instead of adding twice.
          let existing = shared.places.borrow().iter().position(|p| {
            (p.lat - found_place.lat).abs() < 0.05 && (p.lon - found_place.lon).abs() < 0.05
          });
          let select_index = match existing {
            Some(index) => index,
            None => {
              let index = shared.places.borrow().len();
              shared.places.borrow_mut().push(SavedPlace::new(
                found_place.name.clone(),
                found_place.country.clone(),
                found_place.lat,
                found_place.lon,
              ));
              shared.data.borrow_mut().push(None);
              save_places(&shared.places.borrow());
              index
            }
          };
          *shared.selected.borrow_mut() = select_index;
          rebuild();
          window.close();
        });
        row.add_controller(click);
      }
    }));
  });
  window.connect_close_request(|_| {
    SEARCH_SINK.with(|sink| {
      *sink.borrow_mut() = None;
    });
    glib::Propagation::Proceed
  });
}

thread_local! {
  static SEARCH_SINK: RefCell<Option<Box<dyn Fn(Vec<FoundPlace>)>>> = RefCell::new(None);
}

// ── root widget ───────────────────────────────────────────────────────

/// Root widget: sidebar + detail + map.
pub struct WeatherRoot {
  id: WidgetId,
}

impl WeatherRoot {
  pub fn new() -> Self {
    Self { id: next_widget_id() }
  }
}

impl Default for WeatherRoot {
  fn default() -> Self {
    Self::new()
  }
}

impl Widget for WeatherRoot {
  fn id(&self) -> WidgetId {
    self.id
  }

  fn to_gtk(&self) -> gtk::Widget {
    let (tx, rx) = std::sync::mpsc::channel::<Msg>();
    let shared = Shared::fresh(tx.clone());

    let outer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    outer.set_hexpand(true);
    outer.set_vexpand(true);

    // ── sidebar (fixed width, never expands) ──
    let sidebar = gtk::Box::new(gtk::Orientation::Vertical, 0);
    sidebar.set_size_request(SIDEBAR_W, -1);
    sidebar.set_hexpand(false);
    if dark() {
      apply_class(&sidebar, "wx-sidebar", "background-color: #1d1d1d;");
    } else {
      apply_class(&sidebar, "wx-sidebar", "background-color: #ececec;");
    }

    let top = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    top.set_margin_top(12);
    top.set_margin_start(14);
    top.set_margin_end(10);
    top.set_margin_bottom(6);
    let lights = crate::UIKit::widgets::TrafficLights::new().at(0.0, 0.0).size(13.0);
    top.append(&lights.to_gtk());
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    top.append(&spacer);
    let add_button = Button::new("+").width(30.0);
    let add_gtk = add_button.to_gtk();
    add_gtk.set_tooltip_text(Some(&lang::t("sidebar.add")));
    top.append(&add_gtk);
    sidebar.append(&top);

    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::None);
    apply_class(&list, "wx-list", "background-color: transparent;");
    let list_scroll = gtk::ScrolledWindow::new();
    list_scroll.set_child(Some(&list));
    list_scroll.set_vexpand(true);
    list_scroll.set_margin_start(8);
    list_scroll.set_margin_end(8);
    sidebar.append(&list_scroll);
    outer.append(&sidebar);

    // ── detail ──
    let bg_provider = gtk::CssProvider::new();
    bg_provider.load_from_string(".wx-bg { background-image: linear-gradient(to bottom, #2B3644, #5A6E82); }");
    let bg = gtk::Box::new(gtk::Orientation::Vertical, 0);
    bg.set_hexpand(true);
    bg.set_vexpand(true);
    gtk::StyleContext::add_provider_for_display(
      &gtk::gdk::Display::default().expect("gdk display"),
      &bg_provider,
      gtk::STYLE_PROVIDER_PRIORITY_USER as u32,
    );
    bg.add_css_class("wx-bg");

    let detail_scroll = gtk::ScrolledWindow::new();
    detail_scroll.set_child(Some(&bg));
    detail_scroll.set_hexpand(true);
    detail_scroll.set_vexpand(true);
    outer.append(&detail_scroll);

    let city = label("", 28, "600", "#FFFFFF");
    city.set_halign(gtk::Align::Center);
    city.set_margin_top(36);
    bg.append(&city);
    let temp = label("--°", 72, "100", "#FFFFFF");
    temp.set_halign(gtk::Align::Center);
    bg.append(&temp);
    let cond = label("", 17, "normal", "#FFFFFF");
    cond.set_halign(gtk::Align::Center);
    bg.append(&cond);
    let hilo = label("", 15, "normal", "#FFFFFF");
    hilo.set_halign(gtk::Align::Center);
    bg.append(&hilo);
    let offline = label(&lang::t("error.offline"), 12, "600", "#FFD60A");
    offline.set_halign(gtk::Align::Center);
    offline.set_visible(false);
    bg.append(&offline);

    let gap = gtk::Box::new(gtk::Orientation::Vertical, 0);
    gap.set_size_request(-1, 12);
    bg.append(&gap);

    let summary_card = card();
    let summary = label("", 13, "normal", "#FFFFFF");
    summary.set_wrap(true);
    summary.set_margin_top(10);
    summary.set_margin_bottom(10);
    summary.set_margin_start(12);
    summary.set_margin_end(12);
    summary_card.append(&summary);
    bg.append(&summary_card);

    let hourly_title = label(&lang::t("detail.hourly_title"), 11, "600", "rgba(255,255,255,0.7)");
    hourly_title.set_halign(gtk::Align::Start);
    hourly_title.set_margin_start(24);
    hourly_title.set_margin_top(10);
    bg.append(&hourly_title);
    let hourly_card = card();
    let hourly_scroll = gtk::ScrolledWindow::new();
    hourly_scroll.set_hscrollbar_policy(gtk::PolicyType::Automatic);
    hourly_scroll.set_vscrollbar_policy(gtk::PolicyType::Never);
    hourly_scroll.set_margin_top(8);
    hourly_scroll.set_margin_bottom(8);
    let hourly = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    hourly.set_margin_start(8);
    hourly.set_margin_end(8);
    hourly_scroll.set_child(Some(&hourly));
    hourly_card.append(&hourly_scroll);
    bg.append(&hourly_card);

    let daily_title = label(&lang::t("detail.daily_title"), 11, "600", "rgba(255,255,255,0.7)");
    daily_title.set_halign(gtk::Align::Start);
    daily_title.set_margin_start(24);
    daily_title.set_margin_top(10);
    bg.append(&daily_title);
    let daily_card = card();
    let daily = gtk::Box::new(gtk::Orientation::Vertical, 0);
    daily_card.append(&daily);
    bg.append(&daily_card);

    let tiles = gtk::Grid::new();
    tiles.set_column_spacing(8);
    tiles.set_row_spacing(8);
    tiles.set_column_homogeneous(true);
    tiles.set_margin_top(12);
    tiles.set_margin_bottom(24);
    tiles.set_margin_start(12);
    tiles.set_margin_end(12);
    let tile_titles = [
      lang::t("detail.tile_wind"),
      lang::t("detail.tile_humidity"),
      lang::t("detail.tile_feels"),
      lang::t("detail.tile_uv"),
      lang::t("detail.tile_visibility"),
      lang::t("detail.tile_sunrise"),
      lang::t("detail.tile_sunset"),
      lang::t("detail.tile_air"),
      lang::t("detail.tile_pressure"),
      lang::t("detail.tile_precip"),
    ];
    for (index, title) in tile_titles.iter().enumerate() {
      tile(&tiles, (index % 2) as i32, (index / 2) as i32, title);
    }
    bg.append(&tiles);

    let detail = Rc::new(DetailH {
      bg: bg.clone(),
      bg_provider: bg_provider.clone(),
      city,
      temp,
      cond,
      hilo,
      summary,
      offline,
      hourly,
      daily,
      tiles,
    });

    // Sidebar rows.
    let rows: Rc<RefCell<Vec<RowH>>> = Rc::new(RefCell::new(Vec::new()));
    let rebuild_slot: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
    let select = {
      let shared = shared.clone();
      let rows = rows.clone();
      let detail = detail.clone();
      Rc::new(move |index: usize| {
        *shared.selected.borrow_mut() = index;
        paint_selection(&rows.borrow(), index);
        refresh_rows(&rows.borrow(), &shared);
        refresh_detail(&detail, &shared);
        fetch_missing(&shared);
      })
    };

    let rebuild = {
      let shared = shared.clone();
      let rows = rows.clone();
      let detail = detail.clone();
      let list = list.clone();
      let select = select.clone();
      let rebuild_slot = rebuild_slot.clone();
      Rc::new(move || {
        while let Some(child) = list.first_child() {
          list.remove(&child);
        }
        rows.borrow_mut().clear();
        let count = shared.places.borrow().len();
        for index in 0..count {
          let places = shared.places.borrow();
          let Some(place) = places.get(index).cloned() else { continue };
          drop(places);
          let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
          row.set_margin_top(6);
          row.set_margin_bottom(6);
          row.set_margin_start(8);
          row.set_margin_end(8);
          let left = gtk::Box::new(gtk::Orientation::Vertical, 1);
          left.set_hexpand(true);
          let name = label(&place.name, 13, "600", pal().0);
          name.set_halign(gtk::Align::Start);
          name.set_hexpand(true);
          name.set_ellipsize(gtk::pango::EllipsizeMode::End);
          left.append(&name);
          let sub = label("", 11, "normal", pal().1);
          sub.set_halign(gtk::Align::Start);
          sub.set_hexpand(true);
          sub.set_ellipsize(gtk::pango::EllipsizeMode::End);
          sub.set_max_width_chars(24);
          left.append(&sub);
          row.append(&left);
          let right = gtk::Box::new(gtk::Orientation::Vertical, 1);
          let temp_label = label("--°", 24, "300", pal().0);
          temp_label.set_halign(gtk::Align::End);
          right.append(&temp_label);
          let hilo_label = label("", 11, "normal", pal().1);
          hilo_label.set_halign(gtk::Align::End);
          right.append(&hilo_label);
          row.append(&right);
          list.append(&row);
          rows.borrow_mut().push(RowH { row: row.clone(), sub, temp: temp_label, hilo: hilo_label });

          let pick = gtk::GestureClick::new();
          pick.set_button(1);
          let select = select.clone();
          pick.connect_released(move |_, _, _, _| select(index));
          row.add_controller(pick);

          let delete = gtk::GestureClick::new();
          delete.set_button(3);
          let shared = shared.clone();
          let rebuild_slot = rebuild_slot.clone();
          delete.connect_released(move |_, _, _, _| {
            let mut places = shared.places.borrow_mut();
            if places.len() > 1 {
              places.remove(index);
              drop(places);
              shared.data.borrow_mut().remove(index);
              let selected = (*shared.selected.borrow()).min(shared.places.borrow().len() - 1);
              *shared.selected.borrow_mut() = selected;
              save_places(&shared.places.borrow());
              if let Some(rebuild) = rebuild_slot.borrow().as_ref().cloned() {
                rebuild();
              }
            }
          });
          row.add_controller(delete);
        }
        let selected = (*shared.selected.borrow()).min(count.saturating_sub(1));
        *shared.selected.borrow_mut() = selected;
        paint_selection(&rows.borrow(), selected);
        refresh_rows(&rows.borrow(), &shared);
        refresh_detail(&detail, &shared);
        fetch_missing(&shared);
      })
    };

    // Add button opens the search dialog.
    {
      let shared = shared.clone();
      let tx = tx.clone();
      let rebuild = rebuild.clone();
      let click = gtk::GestureClick::new();
      click.connect_released(move |_, _, _, _| {
        open_search(shared.clone(), tx.clone(), rebuild.clone());
      });
      add_gtk.add_controller(click);
    }

    // Initial build + fetch.
    *rebuild_slot.borrow_mut() = Some(rebuild.clone());
    rebuild();
    fetch_all(&shared);

    // Main poll loop: place data and search results.
    let shared_poll = shared.clone();
    let detail_poll = detail.clone();
    let rows_poll = rows.clone();
    let seen_search: Rc<RefCell<u64>> = Rc::new(RefCell::new(0));
    glib::timeout_add_local(std::time::Duration::from_millis(150), move || {
      while let Ok(msg) = rx.try_recv() {
        match msg {
          Msg::Place(index, weather) => {
            let mut data = shared_poll.data.borrow_mut();
            if index < data.len() {
              data[index] = Some(weather);
            }
            drop(data);
            shared_poll.fetching.borrow_mut().remove(&index);
            refresh_rows(&rows_poll.borrow(), &shared_poll);
            if index == *shared_poll.selected.borrow() {
              refresh_detail(&detail_poll, &shared_poll);
            }
          }
          Msg::Search(seq, found) => {
            if seq >= *seen_search.borrow() {
              *seen_search.borrow_mut() = seq;
              SEARCH_SINK.with(|sink| {
                if let Some(callback) = sink.borrow().as_ref() {
                  callback(found);
                }
              });
            }
          }
        }
      }
      glib::ControlFlow::Continue
    });

    outer.upcast()
  }
}

fn fetch_missing(shared: &Shared) {
  let selected = *shared.selected.borrow();
  let has_data = shared.data.borrow().get(selected).and_then(|d| d.clone()).is_some();
  if has_data || shared.fetching.borrow().contains(&selected) {
    return;
  }
  let Some(place) = shared.places.borrow().get(selected).cloned() else { return };
  shared.fetching.borrow_mut().insert(selected);
  let tx = shared.tx.clone();
  std::thread::spawn(move || {
    let weather = weather::fetch_place(place.lat, place.lon);
    let _ = tx.send(Msg::Place(selected, weather));
  });
}

fn fetch_all(shared: &Shared) {
  let places = shared.places.borrow().clone();
  for (index, place) in places.iter().enumerate() {
    if shared.data.borrow().get(index).and_then(|d| d.clone()).is_some()
      || shared.fetching.borrow().contains(&index)
    {
      continue;
    }
    shared.fetching.borrow_mut().insert(index);
    let tx = shared.tx.clone();
    let (lat, lon) = (place.lat, place.lon);
    std::thread::spawn(move || {
      let weather = weather::fetch_place(lat, lon);
      let _ = tx.send(Msg::Place(index, weather));
    });
  }
}

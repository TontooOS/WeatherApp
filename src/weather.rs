//! Weather data layer for the Weather app.
//!
//! Wraps WeatherKit (current, hourly, daily, air quality, astronomy, place
//! search) and maps WMO weather codes to conditions, SF Symbols, localized
//! labels and dynamic background gradients.

use crate::WeatherKit::{AirQuality, CurrentWeather, ForecastDay, HourPoint, Place, WeatherKit};

/// Condition groups used for backgrounds, icons and labels.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Condition {
  Clear,
  PartlyCloudy,
  Cloudy,
  Fog,
  Drizzle,
  Rain,
  Showers,
  Snow,
  Thunderstorm,
  Unknown,
}

/// Map a WMO weather code (plus the provider condition text as fallback)
/// to a condition group.
pub fn condition_for(code: Option<i32>, condition_text: &str) -> Condition {
  if let Some(code) = code {
    return match code {
      0 | 1 => Condition::Clear,
      2 => Condition::PartlyCloudy,
      3 => Condition::Cloudy,
      45 | 48 => Condition::Fog,
      51 | 53 | 55 | 56 | 57 => Condition::Drizzle,
      61 | 63 | 65 | 66 | 67 => Condition::Rain,
      71 | 73 | 75 | 77 | 85 | 86 => Condition::Snow,
      80 | 81 | 82 => Condition::Showers,
      95 | 96 | 99 => Condition::Thunderstorm,
      _ => Condition::Unknown,
    };
  }
  let lower = condition_text.to_lowercase();
  if lower.contains("thunder") || lower.contains("storm") || lower.contains("gewitter") {
    Condition::Thunderstorm
  } else if lower.contains("snow") || lower.contains("schnee") {
    Condition::Snow
  } else if lower.contains("shower") || lower.contains("schauer") {
    Condition::Showers
  } else if lower.contains("rain") || lower.contains("regen") {
    Condition::Rain
  } else if lower.contains("drizzle") || lower.contains("niesel") {
    Condition::Drizzle
  } else if lower.contains("fog") || lower.contains("nebel") || lower.contains("mist") {
    Condition::Fog
  } else if lower.contains("overcast") || lower.contains("bedeckt") {
    Condition::Cloudy
  } else if lower.contains("cloud") || lower.contains("wolk") || lower.contains("bewoelkt") {
    Condition::PartlyCloudy
  } else if lower.contains("clear") || lower.contains("klar") || lower.contains("sun") || lower.contains("sonn") {
    Condition::Clear
  } else {
    Condition::Unknown
  }
}

/// SF Symbol name for a condition (day/night aware for clear skies).
pub fn sf_symbol(condition: Condition, is_day: bool) -> &'static str {
  match condition {
    Condition::Clear => {
      if is_day {
        "sun.max.fill"
      } else {
        "moon.fill"
      }
    }
    Condition::PartlyCloudy => {
      if is_day {
        "cloud.sun.fill"
      } else {
        "cloud.moon.fill"
      }
    }
    Condition::Cloudy => "cloud.fill",
    Condition::Fog => "cloud.fog.fill",
    Condition::Drizzle => "cloud.drizzle.fill",
    Condition::Rain => "cloud.rain.fill",
    Condition::Showers => "cloud.heavyrain.fill",
    Condition::Snow => "cloud.snow.fill",
    Condition::Thunderstorm => "cloud.bolt.rain.fill",
    Condition::Unknown => "cloud.fill",
  }
}

/// Localized label key for a condition.
pub fn label_key(condition: Condition) -> &'static str {
  match condition {
    Condition::Clear => "cond.clear",
    Condition::PartlyCloudy => "cond.partly_cloudy",
    Condition::Cloudy => "cond.cloudy",
    Condition::Fog => "cond.fog",
    Condition::Drizzle => "cond.drizzle",
    Condition::Rain => "cond.rain",
    Condition::Showers => "cond.showers",
    Condition::Snow => "cond.snow",
    Condition::Thunderstorm => "cond.thunderstorm",
    Condition::Unknown => "cond.unknown",
  }
}

/// Dynamic background gradient (top, bottom) per condition and color scheme.
/// Dark pairs echo the macOS Weather artwork; light pairs are the same hues
/// lifted toward `#ececec` (per AGENTS.md light mode).
pub fn background_for(condition: Condition, is_day: bool, dark: bool) -> (&'static str, &'static str) {
  match (condition, is_day, dark) {
    (Condition::Clear, true, true) => ("#2E7CC2", "#9FD0F0"),
    (Condition::Clear, true, false) => ("#7FB2E0", "#D8EAF8"),
    (Condition::Clear, false, true) => ("#0B1A33", "#274B73"),
    (Condition::Clear, false, false) => ("#3A4F6E", "#8FA6C2"),
    (Condition::PartlyCloudy, true, true) => ("#3E6E9E", "#A8C4DE"),
    (Condition::PartlyCloudy, true, false) => ("#7FA3C4", "#D7E4F0"),
    (Condition::PartlyCloudy, false, true) => ("#101E33", "#2E4A68"),
    (Condition::PartlyCloudy, false, false) => ("#44586F", "#96A9BE"),
    (Condition::Cloudy, _, true) => ("#2B3644", "#5A6E82"),
    (Condition::Cloudy, _, false) => ("#6E8296", "#C6D2DD"),
    (Condition::Fog, _, true) => ("#3A4149", "#6B7683"),
    (Condition::Fog, _, false) => ("#B9C1CA", "#DDE3E9"),
    (Condition::Drizzle | Condition::Rain | Condition::Showers, _, true) => ("#232F42", "#4E6A8A"),
    (Condition::Drizzle | Condition::Rain | Condition::Showers, _, false) => ("#7E96B3", "#C3D2E2"),
    (Condition::Snow, _, true) => ("#3C4657", "#8A97A8"),
    (Condition::Snow, _, false) => ("#A9B7C7", "#E4EAF1"),
    (Condition::Thunderstorm, _, true) => ("#1B1B26", "#3E3E5C"),
    (Condition::Thunderstorm, _, false) => ("#5C5C78", "#A9A9C4"),
    (Condition::Unknown, _, true) => ("#2B3644", "#5A6E82"),
    (Condition::Unknown, _, false) => ("#6E8296", "#C6D2DD"),
  }
}

/// Celsius temperature formatted like macOS Weather (`21°`).
pub fn fmt_temp(celsius: f64) -> String {
  format!("{}°", celsius.round() as i64)
}

/// Hour label from an ISO time (`2026-09-09T20:00` -> `8PM`).
pub fn hour_label(iso: &str) -> String {
  let hour: i32 = iso.get(11..13).and_then(|h| h.parse().ok()).unwrap_or(0);
  if hour == 0 {
    "12AM".to_string()
  } else if hour < 12 {
    format!("{hour}AM")
  } else if hour == 12 {
    "12PM".to_string()
  } else {
    format!("{}PM", hour - 12)
  }
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
  let y = if month <= 2 { year - 1 } else { year };
  let era = if y >= 0 { y } else { y - 399 } / 400;
  let yoe = y - era * 400;
  let mp = (month + 9) % 12;
  let doy = (153 * mp + 2) / 5 + day - 1;
  let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
  era * 146_097 + doe - 719_468
}

/// Weekday name for an ISO date (`2026-09-09` -> `Wed`); index 0 is Today.
pub fn day_name(iso_date: &str, index: usize) -> String {
  if index == 0 {
    return crate::lang::t("detail.today");
  }
  let parts: Vec<&str> = iso_date.split('-').collect();
  if parts.len() != 3 {
    return iso_date.to_string();
  }
  let (y, m, d) = (
    parts[0].parse().unwrap_or(2026),
    parts[1].parse().unwrap_or(1),
    parts[2].parse().unwrap_or(1),
  );
  // 1970-01-01 was a Thursday (index 4 with Sunday = 0).
  let weekday = (days_from_civil(y, m, d) + 4).rem_euclid(7);
  match weekday {
    0 => "Sun",
    1 => "Mon",
    2 => "Tue",
    3 => "Wed",
    4 => "Thu",
    5 => "Fri",
    _ => "Sat",
  }
  .to_string()
}

/// Compass point for meteorological wind degrees.
pub fn compass(degrees: i32) -> &'static str {
  match degrees.rem_euclid(360) {
    0..=11 | 349..=359 => "N",
    12..=33 => "NNE",
    34..=56 => "NE",
    57..=78 => "ENE",
    79..=101 => "E",
    102..=123 => "ESE",
    124..=146 => "SE",
    147..=168 => "SSE",
    169..=191 => "S",
    192..=213 => "SSW",
    214..=236 => "SW",
    237..=258 => "WSW",
    259..=281 => "W",
    282..=303 => "WNW",
    304..=326 => "NW",
    _ => "NNW",
  }
}

/// Today's date as (year, month, day) for astronomy queries.
pub fn today_ymd() -> (i32, u32, u32) {
  let secs = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .map(|d| d.as_secs() as i64)
    .unwrap_or(0);
  let mut days = secs / 86400;
  let mut year: i64 = 1970;
  loop {
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let year_days = if leap { 366 } else { 365 };
    if days < year_days {
      break;
    }
    days -= year_days;
    year += 1;
  }
  let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
  let months = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
  let mut month = 1u32;
  for days_in_month in months {
    if days < days_in_month as i64 {
      break;
    }
    days -= days_in_month as i64;
    month += 1;
  }
  (year as i32, month, (days + 1) as u32)
}

/// Full weather snapshot for one place.
#[derive(Clone, Debug)]
pub struct PlaceWeather {
  pub current: CurrentWeather,
  pub hourly: Vec<HourPoint>,
  pub daily: Vec<ForecastDay>,
  pub air: Option<AirQuality>,
  pub sunrise: Option<(u32, u32)>,
  pub sunset: Option<(u32, u32)>,
  pub offline: bool,
}

fn demo_weather() -> PlaceWeather {
  PlaceWeather {
    current: CurrentWeather {
      temperature_c: 15.0,
      feels_like_c: 13.0,
      humidity_pct: 72,
      pressure_hpa: 1012.0,
      wind_kmh: 9.0,
      wind_direction_deg: 240,
      wind_gusts_kmh: Some(14.0),
      precipitation_mm: 0.0,
      cloud_cover_pct: Some(80),
      visibility_m: Some(16000.0),
      uv_index: Some(1.5),
      is_day: false,
      weather_code: Some(3),
      condition: "Overcast".to_string(),
      source: "demo".to_string(),
    },
    hourly: (0..12)
      .map(|h| HourPoint {
        time: format!("2026-09-09T{:02}:00", (19 + h) % 24),
        temperature_c: 15.0 - h as f64 * 0.4,
        precipitation_mm: 0.0,
        precip_probability_pct: Some(if h > 6 { 30 } else { 10 }),
        wind_kmh: 9.0,
        weather_code: Some(3),
      })
      .collect(),
    daily: (0..10)
      .map(|d| ForecastDay {
        date: format!("2026-09-{:02}", 9 + d),
        temp_max_c: 18.0 - d as f64 * 0.5,
        temp_min_c: 9.0 + d as f64 * 0.3,
        precipitation_mm: Some(0.0),
        precip_probability_pct: Some(20),
        wind_max_kmh: 12.0,
        weather_code: Some(if d % 3 == 0 { 61 } else { 2 }),
        sunrise_utc: None,
        sunset_utc: None,
      })
      .collect(),
    air: None,
    sunrise: Some((6, 42)),
    sunset: Some((19, 31)),
    offline: true,
  }
}

/// Fetch everything for fixed coordinates; falls back to demo data offline.
pub fn fetch_place(lat: f64, lon: f64) -> PlaceWeather {
  let kit = WeatherKit::at(lat, lon);
  let current = match kit.current_weather() {
    Ok(current) => current,
    Err(_) => return demo_weather(),
  };
  let hourly = kit.hourly_forecast(24).unwrap_or_default();
  let daily = kit.daily_forecast(10).unwrap_or_default();
  let air = kit.air_quality().ok();
  let (year, month, day) = today_ymd();
  let (sunrise, sunset) = match kit.sun_times(year, month, day) {
    Ok(times) => (Some(times.sunrise), Some(times.sunset)),
    Err(_) => (None, None),
  };
  PlaceWeather {
    current,
    hourly,
    daily,
    air,
    sunrise,
    sunset,
    offline: false,
  }
}

/// Search result for the add-location dialog.
#[derive(Clone, Debug)]
pub struct FoundPlace {
  pub name: String,
  pub country: String,
  pub lat: f64,
  pub lon: f64,
}

/// Search places by name via WeatherKit (empty when offline).
pub fn search_places(query: &str) -> Vec<FoundPlace> {
  let kit = WeatherKit::new();
  let results: Vec<Place> = kit.search_places(query).unwrap_or_default();
  results
    .into_iter()
    .map(|place| FoundPlace {
      name: place.name,
      country: place.country.or(place.region).unwrap_or_default(),
      lat: place.latitude,
      lon: place.longitude,
    })
    .collect()
}

/// Render a CoreIcon SF Symbol glyph (transparent tile) to a temp PNG.
/// White glyph in dark mode, near-black glyph in light mode for contrast.
/// Returns the file path, or `None` when CoreIcon has no such symbol.
pub fn weather_icon_path(symbol: &str, display_px: u32, dark: bool) -> Option<String> {
  let sf = crate::CoreIcon::SFSymbol::from_name(symbol)?;
  let assets = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("../../TontooLibs/CoreIcon/assets/icons");
  if assets.exists() {
    unsafe {
      crate::CoreIcon::generator::ASSETS_DIR =
        Box::leak(assets.to_str()?.to_string().into_boxed_str());
    }
  }
  let px = display_px.clamp(8, 256);
  let key = format!("wx_{}_{px}_{}", symbol.replace('.', "_"), if dark { "d" } else { "l" });
  let out_path = std::env::temp_dir().join(format!("{key}.png"));
  if out_path.exists() {
    return Some(out_path.to_str()?.to_string());
  }
  let glyph = if dark {
    crate::CoreIcon::Color::new(1.0, 1.0, 1.0, 1.0)
  } else {
    crate::CoreIcon::Color::new(0.11, 0.11, 0.11, 1.0)
  };
  let clear = crate::CoreIcon::Color::new(0.0, 0.0, 0.0, 0.0);
  let canvas = crate::CoreIcon::generator::IconCanvas::new()
    .background(crate::CoreIcon::generator::Background::color(clear))
    .corner_radius(220.0)
    .layer(
      crate::CoreIcon::generator::Layer::new(crate::CoreIcon::generator::LayerContent::icon(sf))
        .position(120.0, 120.0)
        .size(784.0, 784.0)
        .tint(glyph),
    );
  canvas.save(&out_path).ok()?;
  Some(out_path.to_str()?.to_string())
}

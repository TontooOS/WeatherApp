//! Saved locations for Weather.
//!
//! Persistence is CoreData only: `SavedPlace` entities in the
//! `com.tontoo.weather` store
//! (`~/Library/Preferences/com.tontoo.weather/storage.fico`).
//! Defaults (Tokyo, Berlin, New York) are seeded only when the store is
//! empty.

pub const BUNDLE_ID: &str = "com.tontoo.weather";

#[derive(Clone, Debug)]
pub struct SavedPlace {
  pub name: String,
  pub country: String,
  pub lat: f64,
  pub lon: f64,
  pub is_current: bool,
}

impl SavedPlace {
  pub fn new(
    name: impl Into<String>,
    country: impl Into<String>,
    lat: f64,
    lon: f64,
  ) -> Self {
    Self {
      name: name.into(),
      country: country.into(),
      lat,
      lon,
      is_current: false,
    }
  }
}

fn coredata_load() -> Option<Vec<SavedPlace>> {
  let mut container = match coredata::PersistentContainer::new_with_bundle(
    BUNDLE_ID.to_string(),
    coredata::StoreType::Fico,
  ) {
    Ok(container) => container,
    Err(err) => {
      eprintln!("weather: coredata open failed: {err}");
      return None;
    }
  };
  let objects = match container.view_context().fetch_all("SavedPlace") {
    Ok(objects) => objects,
    Err(err) => {
      eprintln!("weather: coredata fetch failed: {err}");
      return None;
    }
  };
  let mut places = Vec::new();
  for obj in objects {
    // Skip malformed objects instead of dropping the whole store.
    let (Some(name), Some(lat), Some(lon)) =
      (obj.get_str("name"), obj.get_f64("lat"), obj.get_f64("lon"))
    else {
      eprintln!("weather: skipping malformed SavedPlace {}", obj.object_id);
      continue;
    };
    places.push(SavedPlace {
      name: name.to_string(),
      country: obj.get_str("country").unwrap_or("").to_string(),
      lat,
      lon,
      is_current: obj.get_bool("is_current").unwrap_or(false),
    });
  }
  eprintln!("weather: coredata loaded {} places", places.len());
  Some(places)
}

fn coredata_save(places: &[SavedPlace]) {
  let mut container = match coredata::PersistentContainer::new_with_bundle(
    BUNDLE_ID.to_string(),
    coredata::StoreType::Fico,
  ) {
    Ok(container) => container,
    Err(err) => {
      eprintln!("weather: coredata open failed: {err}");
      return;
    }
  };
  let existing = container
    .view_context()
    .fetch_all("SavedPlace")
    .unwrap_or_default()
    .iter()
    .map(|o| o.object_id.clone())
    .collect::<Vec<_>>();
  let mut ctx = container.view_context();
  for id in existing {
    if let Err(err) = ctx.delete(&id) {
      eprintln!("weather: coredata delete failed: {err}");
    }
  }
  for place in places {
    let mut obj = ctx.create("SavedPlace");
    obj.set("name", place.name.clone());
    obj.set("country", place.country.clone());
    obj.set("lat", place.lat);
    obj.set("lon", place.lon);
    obj.set("is_current", place.is_current);
    if let Err(err) = ctx.save_object(obj) {
      eprintln!("weather: coredata save_object failed: {err}");
    }
  }
  if let Err(err) = ctx.save() {
    eprintln!("weather: coredata save failed: {err}");
  } else {
    eprintln!("weather: coredata saved {} places", places.len());
  }
}

/// Default list seeded on first launch (empty store only).
pub fn seed_places() -> Vec<SavedPlace> {
  vec![
    SavedPlace::new("Tokyo", "Japan", 35.6762, 139.6503),
    SavedPlace::new("Berlin", "Germany", 52.52, 13.405),
    SavedPlace::new("New York", "United States", 40.7128, -74.006),
  ]
}

/// Load saved locations from CoreData, seeding only when empty.
pub fn load_places() -> Vec<SavedPlace> {
  match coredata_load() {
    Some(places) if !places.is_empty() => places,
    Some(_) => {
      eprintln!("weather: store empty, seeding defaults");
      let seeded = seed_places();
      coredata_save(&seeded);
      seeded
    }
    None => {
      eprintln!("weather: store unreadable, using defaults for this run");
      seed_places()
    }
  }
}

/// Persist locations to CoreData.
pub fn save_places(places: &[SavedPlace]) {
  coredata_save(places);
}

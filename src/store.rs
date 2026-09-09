//! Saved locations for Weather.
//!
//! Persistence is CoreData only: `SavedPlace` entities in the
//! `com.tontoo.weather` store
//! (`~/Library/Preferences/com.tontoo.weather/storage.fico`).

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
  let mut container =
    coredata::PersistentContainer::new_with_bundle(BUNDLE_ID.to_string(), coredata::StoreType::Fico)
      .ok()?;
  let ctx = container.view_context();
  let objects = ctx.fetch_all("SavedPlace").ok()?;
  let mut places = Vec::new();
  for obj in objects {
    let name = obj.get_str("name")?.to_string();
    let lat = obj.get_f64("lat")?;
    let lon = obj.get_f64("lon")?;
    places.push(SavedPlace {
      name,
      country: obj.get_str("country").unwrap_or("").to_string(),
      lat,
      lon,
      is_current: obj.get_bool("is_current").unwrap_or(false),
    });
  }
  Some(places)
}

fn coredata_save(places: &[SavedPlace]) {
  let mut container = match coredata::PersistentContainer::new_with_bundle(
    BUNDLE_ID.to_string(),
    coredata::StoreType::Fico,
  ) {
    Ok(container) => container,
    Err(_) => return,
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
    let _ = ctx.delete(&id);
  }
  for place in places {
    let mut obj = ctx.create("SavedPlace");
    obj.set("name", place.name.clone());
    obj.set("country", place.country.clone());
    obj.set("lat", place.lat);
    obj.set("lon", place.lon);
    obj.set("is_current", place.is_current);
    let _ = ctx.save_object(obj);
  }
  let _ = ctx.save();
}

/// Seed list used on first launch: CoreLocation position plus favorites.
pub fn seed_places() -> Vec<SavedPlace> {
  let mut places = Vec::new();
  if let Ok(loc) = crate::CoreLocation::CoreLocation::new().get_location() {
    let name = loc.city.clone().unwrap_or_else(|| crate::lang::t("sidebar.my_location"));
    let mut mine = SavedPlace::new(name, loc.country.clone().unwrap_or_default(), loc.coordinates.latitude, loc.coordinates.longitude);
    mine.is_current = true;
    places.push(mine);
  } else {
    let mut mine = SavedPlace::new(crate::lang::t("sidebar.my_location"), "", 52.52, 13.40);
    mine.is_current = true;
    places.push(mine);
  }
  places.push(SavedPlace::new("Berlin", "Germany", 52.52, 13.405));
  places.push(SavedPlace::new("New York", "United States", 40.7128, -74.006));
  places.push(SavedPlace::new("Tokyo", "Japan", 35.6762, 139.6503));
  places
}

/// Load saved locations from CoreData, seeding on first launch.
pub fn load_places() -> Vec<SavedPlace> {
  if let Some(places) = coredata_load() {
    if !places.is_empty() {
      return places;
    }
  }
  let seeded = seed_places();
  coredata_save(&seeded);
  seeded
}

/// Persist locations to CoreData.
pub fn save_places(places: &[SavedPlace]) {
  coredata_save(places);
}

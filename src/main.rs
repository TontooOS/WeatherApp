//! Weather for TontooOS: macOS Weather-style app built on the TontooUI
//! renderer.
//!
//! A TontooUI `Sidebar` owns the navigation column (traffic lights, add
//! pill, one page per saved location) and the condition-driven detail
//! page lives in that page slot: a gradient background view wrapping a
//! `ScrollView` with the current conditions, hourly strip, 10-day
//! forecast and detail tiles. Adding a location opens a `BasicSheet`
//! with a search field, removing one a `ActionAlert`. All text uses SF
//! Pro with `en_us` and `de_de` strings from `Resources/lang` via the
//! Accessibility framework.

mod lang;
mod store;
mod views;
mod weather;

sdk::preinclude!();

use TontooUI::renderer::window::run;

fn main() {
  lang::init();
  if let Err(err) = run(&lang::t("app.title"), 1200, 675, views::WeatherApp::new()) {
    eprintln!("weather: {err}");
    std::process::exit(1);
  }
}
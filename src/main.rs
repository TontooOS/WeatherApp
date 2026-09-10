//! Weather for TontooOS: macOS Weather-style app built with TontooUI.
//!
//! Sidebar on the left (traffic lights + add button, saved locations with
//! live temperatures), detail in the center (dynamic condition background,
//! hourly strip, 10-day forecast, detail tiles), MapsKit map on the right.
//! All text uses SF Pro Display with `en_us` and `de_de` strings from
//! `Resources/lang` via the Accessibility framework.

mod lang;
mod store;
mod views;
mod weather;

sdk::preinclude!();

use UIKit::prelude::*;

struct WeatherDelegate;

impl AppDelegate for WeatherDelegate {
  fn view(&self) -> Box<dyn Widget> {
    Box::new(views::WeatherRoot::new())
  }
}

fn main() {
  lang::init();
  let mut app = App::with_delegate(lang::t("app.title"), 1024, 640, WeatherDelegate);
  // No extra window bar: the sidebar draws the only traffic lights.
  app.no_window_bar();
  app.auto_color_scheme();
  app.run();
}

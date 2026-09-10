//! Locale store for Weather, backed by the Accessibility framework.
//!
//! Loads `lang/en_us.json` and `lang/de_de.json` (Accessibility `LangFile`
//! format: `{ "lang": ..., "translations": ... }`) based on the system locale
//! (`LANGUAGE`, `LC_ALL`, `LANG`, `/etc/locale.conf`). Falls back to an
//! embedded English table when no file is found. Only `en_us` and `de_de`
//! are supported.

use crate::Accessibility::{LangFile, LangStore};
use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::path::PathBuf;

static LOCALE: OnceCell<String> = OnceCell::new();
static READY: OnceCell<()> = OnceCell::new();

/// Detect the system locale. Returns `de_de` for German, `en_us` otherwise.
pub fn detect_locale() -> String {
  for key in ["LANGUAGE", "LC_ALL", "LANG"] {
    if let Ok(value) = std::env::var(key) {
      let lower = value.to_lowercase();
      if lower.starts_with("de") {
        return "de_de".to_string();
      }
      if lower.starts_with("en") {
        return "en_us".to_string();
      }
    }
  }
  if let Ok(content) = std::fs::read_to_string("/etc/locale.conf") {
    if content.to_lowercase().contains("lang=de") {
      return "de_de".to_string();
    }
  }
  "en_us".to_string()
}

/// Active locale code (`en_us` or `de_de`).
pub fn locale() -> String {
  LOCALE.get().cloned().unwrap_or_else(detect_locale)
}

/// Candidate directories holding the `lang/` folder. `Resources/lang` is
/// canonical (TBuild copies `Resources/` into the `.app` bundle); the root
/// `lang/` folder covers `cargo run` from the repository directory.
fn lang_dirs() -> Vec<PathBuf> {
  let mut dirs = Vec::new();
  // Compile-time project dir: reliable for dev runs regardless of cwd.
  dirs.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Resources").join("lang"));
  dirs.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lang"));
  if let Ok(cwd) = std::env::current_dir() {
    dirs.push(cwd.join("Resources").join("lang"));
    dirs.push(cwd.join("lang"));
  }
  if let Ok(exe) = std::env::current_exe() {
    if let Some(parent) = exe.parent() {
      dirs.push(parent.join("lang"));
      if let Some(grand) = parent.parent() {
        dirs.push(grand.join("lang"));
        // .app bundle layout: <Name>.app/{App/binary, Resources/lang}.
        dirs.push(grand.join("Resources").join("lang"));
      }
    }
  }
  dirs.push(PathBuf::from("/usr/share/weather/lang"));
  dirs
}

fn builtin_en() -> LangFile {
  let pairs = [
    ("app.title", "Weather"),
    ("detail.hourly_now", "Now"),
    ("detail.today", "Today"),
    ("cond.unknown", "Unknown"),
  ];
  let map: HashMap<String, String> = pairs
    .iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();
  LangFile::new("en_us", map)
}

/// Load strings for the detected locale. Safe to call multiple times.
pub fn init() {
  if READY.get().is_some() {
    return;
  }
  let detected = detect_locale();
  let mut files: Vec<LangFile> = Vec::new();
  for code in ["en_us", "de_de"] {
    let file = format!("{code}.json");
    for dir in lang_dirs() {
      let path = dir.join(&file);
      if path.exists() {
        if let Ok(lang_file) = LangFile::from_file(&path) {
          files.push(lang_file);
          break;
        }
      }
    }
  }
  if !files.iter().any(|f| f.lang == "en_us") {
    files.push(builtin_en());
  }
  let fallback = if files.iter().any(|f| f.lang == detected) {
    detected.clone()
  } else {
    "en_us".to_string()
  };
  eprintln!(
    "weather: locale={detected} langs={:?}",
    files.iter().map(|f| f.lang.clone()).collect::<Vec<_>>(),
  );
  let _ = LOCALE.set(detected);
  let _ = LangStore::init(files, Some(fallback));
  let _ = READY.set(());
}

/// Look up a localized string. Returns the key itself when missing.
pub fn t(key: &str) -> String {
  init();
  LangStore::instance()
    .t(&locale(), key, None)
    .unwrap_or_else(|| key.to_string())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn detect_defaults_to_supported_locale() {
    let locale = detect_locale();
    assert!(locale == "en_us" || locale == "de_de");
  }

  #[test]
  fn missing_key_returns_key() {
    let value = t("missing.key.that.does.not.exist");
    assert_eq!(value, "missing.key.that.does.not.exist");
  }
}

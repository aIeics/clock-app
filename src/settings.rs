//! User settings, saved to `settings.json` (decision D1 in `PLAN.md`).
//!
//! Unknown fields are ignored and missing fields fall back to the defaults, so
//! an old or hand-edited file still loads.

use serde::{Deserialize, Serialize};

use crate::storage::Store;

pub const FILE: &str = "settings.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// 24-hour clock (`true`) or 12-hour with AM/PM (`false`).
    pub clock_24h: bool,
    /// Show seconds in the clock.
    pub show_seconds: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            clock_24h: true,
            show_seconds: true,
        }
    }
}

impl Settings {
    pub fn load(store: &Store) -> Self {
        store.load_or_default(FILE)
    }

    pub fn save(&self, store: &Store) {
        if let Err(err) = store.save(FILE, self) {
            eprintln!("clock-app: could not save {}: {err}", FILE);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_a_24_hour_clock_with_seconds() {
        let settings = Settings::default();
        assert!(settings.clock_24h);
        assert!(settings.show_seconds);
    }

    #[test]
    fn missing_fields_keep_their_defaults() {
        let settings: Settings = serde_json::from_str(r#"{ "clock_24h": false }"#).unwrap();
        assert!(!settings.clock_24h);
        assert!(settings.show_seconds);
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let settings: Settings =
            serde_json::from_str(r#"{ "clock_24h": true, "from_the_future": 1 }"#).unwrap();
        assert_eq!(settings, Settings::default());
    }
}

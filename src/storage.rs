//! Small JSON files in `%APPDATA%\clock-app\` (decision D1 in `PLAN.md`).
//!
//! Files are written atomically: a temp file first, then a rename over the old
//! one, so a crash mid-write can't leave a half-written settings file behind.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;

/// The folder the app saves into. `dir` is `None` on systems without `APPDATA`,
/// in which case nothing is loaded or saved and the app still works.
pub struct Store {
    dir: Option<PathBuf>,
}

impl Store {
    /// The real store: `%APPDATA%\clock-app\`.
    pub fn new() -> Self {
        Self {
            dir: std::env::var_os("APPDATA").map(|base| PathBuf::from(base).join("clock-app")),
        }
    }

    /// A store in an explicit folder, used by tests.
    #[cfg(test)]
    pub fn at(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: Some(dir.into()),
        }
    }

    pub fn path(&self, name: &str) -> Option<PathBuf> {
        self.dir.as_ref().map(|dir| dir.join(name))
    }

    /// Reads `name`, or `T::default()` if the file is missing or unreadable.
    pub fn load_or_default<T: DeserializeOwned + Default>(&self, name: &str) -> T {
        self.load(name).unwrap_or_default()
    }

    pub fn load<T: DeserializeOwned>(&self, name: &str) -> Option<T> {
        let path = self.path(name)?;
        let text = fs::read_to_string(&path).ok()?;
        match serde_json::from_str(&text) {
            Ok(value) => Some(value),
            Err(err) => {
                eprintln!("clock-app: ignoring unreadable {}: {err}", path.display());
                None
            }
        }
    }

    pub fn save<T: Serialize>(&self, name: &str, value: &T) -> io::Result<()> {
        let Some(path) = self.path(name) else {
            return Ok(());
        };
        write_json(&path, value)
    }
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let text = serde_json::to_string_pretty(value).map_err(io::Error::other)?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, text)?;
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store(label: &str) -> Store {
        let dir =
            std::env::temp_dir().join(format!("clock-app-test-{}-{label}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        Store::at(dir)
    }

    #[test]
    fn saves_and_loads_a_file() {
        let store = temp_store("roundtrip");
        let values = vec![1_u32, 2, 3];

        store.save("numbers.json", &values).expect("save");
        let loaded: Vec<u32> = store.load("numbers.json").expect("load");

        assert_eq!(loaded, values);
        let text = fs::read_to_string(store.path("numbers.json").unwrap()).unwrap();
        assert!(text.contains('\n'), "pretty JSON is easier to edit by hand");
    }

    #[test]
    fn missing_file_falls_back_to_the_default() {
        let store = temp_store("missing");
        let loaded: Vec<u32> = store.load_or_default("nothing.json");
        assert!(loaded.is_empty());
    }

    #[test]
    fn broken_file_falls_back_to_the_default() {
        let store = temp_store("broken");
        let path = store.path("broken.json").unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{ not json").unwrap();

        let loaded: Vec<u32> = store.load_or_default("broken.json");
        assert!(loaded.is_empty());
    }
}

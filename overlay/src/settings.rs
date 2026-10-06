//! Menu toggles remembered between runs, in %APPDATA%\UnderBoss\settings.json. Kept out of
//! the exe's folder so a new UnderBoss zip unpacked over the old one keeps them.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

fn path() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("APPDATA")?).join("UnderBoss").join("settings.json"))
}

/// Saved state for each (key, default) pair. Keys the file doesn't have keep their default.
pub fn load<const N: usize>(defaults: [(&str, bool); N]) -> [bool; N] {
    let saved: BTreeMap<String, bool> = path()
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    defaults.map(|(key, on)| saved.get(key).copied().unwrap_or(on))
}

/// Best effort: failing to save only costs the player their toggles next launch.
pub fn save<'a>(values: impl IntoIterator<Item = (&'a str, bool)>) {
    let Some(path) = path() else { return };
    let values: BTreeMap<&str, bool> = values.into_iter().collect();

    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string_pretty(&values) {
        let _ = fs::write(path, json);
    }
}

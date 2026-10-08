//! Display-only preferences for `shunt tui`: which provider sections are
//! hidden and in what order they are drawn.
//!
//! Stored as JSON at `~/.shunt/top.json` (`HOME`, falling back to
//! `USERPROFILE` on Windows). Hiding is purely visual — a hidden provider
//! keeps routing traffic — so this file never touches the gateway, the admin
//! API, or `shunt.toml`. A missing or corrupt file means "show everything in
//! gateway order", never an error.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct DisplayPrefs {
    /// Provider names that are not drawn.
    #[serde(default)]
    pub hidden: Vec<String>,
    /// Desired section order. Names absent from the gateway response keep
    /// their relative order at the end; unknown names are ignored on read.
    #[serde(default)]
    pub order: Vec<String>,
}

/// Deduplicated, trimmed, non-empty names in first-seen order.
fn clean(names: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    names
        .into_iter()
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty() && seen.insert(n.clone()))
        .collect()
}

impl DisplayPrefs {
    fn cleaned(mut self) -> Self {
        self.hidden = clean(std::mem::take(&mut self.hidden));
        self.order = clean(std::mem::take(&mut self.order));
        self
    }
}

pub fn default_path() -> Option<PathBuf> {
    crate::auth::shared::home_dir().map(|home| home.join(".shunt").join("top.json"))
}

pub fn load() -> DisplayPrefs {
    default_path().map_or_else(DisplayPrefs::default, |path| load_from(&path))
}

pub fn load_from(path: &Path) -> DisplayPrefs {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<DisplayPrefs>(&text).ok())
        .map(DisplayPrefs::cleaned)
        .unwrap_or_default()
}

pub fn save(prefs: &DisplayPrefs) -> anyhow::Result<()> {
    let Some(path) = default_path() else {
        return Ok(());
    };
    save_to(&path, prefs)
}

pub fn save_to(path: &Path, prefs: &DisplayPrefs) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(prefs)?;
    std::fs::write(path, text)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_path(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "shunt-top-prefs-{}-{}-{tag}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ))
    }

    #[test]
    fn round_trips_hidden_and_order() {
        let path = tmp_path("roundtrip").join("top.json");
        let prefs = DisplayPrefs {
            hidden: vec!["antigravity".into()],
            order: vec!["codex".into(), "anthropic".into()],
        };
        save_to(&path, &prefs).unwrap();
        assert_eq!(load_from(&path), prefs);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn missing_or_corrupt_files_show_everything() {
        assert_eq!(
            load_from(Path::new("/nonexistent/shunt-top.json")),
            DisplayPrefs::default()
        );
        let path = tmp_path("corrupt").join("top.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "{not json").unwrap();
        assert_eq!(load_from(&path), DisplayPrefs::default());
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn load_dedups_and_trims() {
        let path = tmp_path("clean").join("top.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, r#"{"hidden":[" b ","b",""],"order":["a","a"]}"#).unwrap();
        assert_eq!(
            load_from(&path),
            DisplayPrefs {
                hidden: vec!["b".into()],
                order: vec!["a".into()],
            }
        );
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}

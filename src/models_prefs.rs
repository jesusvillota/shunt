//! Which models of a CLI platform (`codex`, `opencode`, `agy`) the operator's
//! tools may use: a machine-wide on/off list kept by `shunt tui` and read by
//! `shunt models` (and through it by the `goal` tool and `/delegate-cli`).
//!
//! Stored as JSON at `~/.shunt/models.json`. Only the operator's overrides are
//! stored; the default is computed from the live catalog by [`default_active`]
//! (the newest version of each model family is on). A missing or corrupt file
//! means "defaults, filter on", never an error. Nothing here touches gateway
//! routing, `/v1/models`, or `shunt.toml`.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ModelPrefs {
    /// When off, consumers see every model, as if this feature did not exist.
    pub filter: bool,
    /// `platform -> model id -> forced state`, only where it differs from
    /// the computed default.
    pub overrides: BTreeMap<String, BTreeMap<String, bool>>,
}

impl Default for ModelPrefs {
    fn default() -> Self {
        Self {
            filter: true,
            overrides: BTreeMap::new(),
        }
    }
}

/// One catalog entry with its resolved state.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Resolved {
    pub id: String,
    pub active: bool,
    /// `"default"` or `"override"`.
    pub reason: &'static str,
}

impl ModelPrefs {
    /// Resolve every id of one platform, keeping the catalog order.
    pub fn resolve(&self, platform: &str, ids: &[String]) -> Vec<Resolved> {
        let defaults = default_active(ids);
        let overrides = self.overrides.get(platform);
        ids.iter()
            .map(|id| match overrides.and_then(|o| o.get(id)) {
                Some(&active) => Resolved {
                    id: id.clone(),
                    active,
                    reason: "override",
                },
                None => Resolved {
                    id: id.clone(),
                    active: defaults.contains(id),
                    reason: "default",
                },
            })
            .collect()
    }

    /// Force `id` on or off. An override equal to the default is dropped, so a
    /// later catalog change can still move the default.
    pub fn set(&mut self, platform: &str, ids: &[String], id: &str, active: bool) {
        let default = default_active(ids).contains(id);
        let platform_overrides = self.overrides.entry(platform.to_string()).or_default();
        if active == default {
            platform_overrides.remove(id);
        } else {
            platform_overrides.insert(id.to_string(), active);
        }
        if platform_overrides.is_empty() {
            self.overrides.remove(platform);
        }
    }
}

const TIERS: [&str; 6] = ["minimal", "low", "medium", "high", "xhigh", "max"];

/// `(family key, version)` of a model id, or `None` when it carries no version
/// (such a model is always on by default).
///
/// The key is the name without its version and without a trailing effort tier;
/// the version is the first run of numeric tokens (`6.1`, or `4-6` = 4.6).
fn family_and_version(id: &str) -> Option<(String, Vec<u32>)> {
    let name = id.rsplit('/').next().unwrap_or(id);
    let name = name.split("-via-").next().unwrap_or(name);
    let mut tokens: Vec<&str> = name.split('-').collect();
    while tokens.len() > 1 && TIERS.contains(tokens.last()?) {
        tokens.pop();
    }
    let start = tokens.iter().position(|t| version_part(t).is_some())?;
    let mut version = Vec::new();
    let mut end = start;
    while let Some(parts) = tokens.get(end).and_then(|t| version_part(t)) {
        version.extend(parts);
        end += 1;
    }
    let key = tokens[..start]
        .iter()
        .chain(&tokens[end..])
        .copied()
        .collect::<Vec<_>>()
        .join("-");
    Some((key, version))
}

/// `6.1` -> `[6, 1]`, `v2.6` -> `[2, 6]`, `6` -> `[6]`. Long digit strings
/// (dates, build numbers) are not versions.
fn version_part(token: &str) -> Option<Vec<u32>> {
    let token = token.strip_prefix('v').unwrap_or(token);
    if token.is_empty() || token.len() > 4 {
        return None;
    }
    token.split('.').map(|p| p.parse().ok()).collect()
}

/// The ids that are on without an override: for each family the highest
/// version (all effort tiers of it), plus every id that has no version.
pub fn default_active(ids: &[String]) -> BTreeSet<String> {
    let parsed: Vec<_> = ids.iter().map(|id| family_and_version(id)).collect();
    let mut newest: HashMap<&str, &Vec<u32>> = HashMap::new();
    for (key, version) in parsed.iter().flatten() {
        let best = newest.entry(key).or_insert(version);
        if version > *best {
            *best = version;
        }
    }
    ids.iter()
        .zip(&parsed)
        .filter(|(_, p)| match p {
            Some((key, version)) => newest.get(key.as_str()) == Some(&version),
            None => true,
        })
        .map(|(id, _)| id.clone())
        .collect()
}

pub fn default_path() -> Option<PathBuf> {
    crate::auth::shared::home_dir().map(|home| home.join(".shunt").join("models.json"))
}

pub fn load() -> ModelPrefs {
    default_path().map_or_else(ModelPrefs::default, |path| load_from(&path))
}

pub fn load_from(path: &Path) -> ModelPrefs {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save(prefs: &ModelPrefs) -> anyhow::Result<()> {
    match default_path() {
        Some(path) => save_to(&path, prefs),
        None => Ok(()),
    }
}

pub fn save_to(path: &Path, prefs: &ModelPrefs) -> anyhow::Result<()> {
    let text = serde_json::to_string_pretty(prefs)?;
    crate::atomic_file::write_private_atomic(path, text.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    fn active(list: &[&str]) -> Vec<String> {
        default_active(&ids(list)).into_iter().collect()
    }

    #[test]
    fn newest_version_of_a_family_is_on() {
        assert_eq!(active(&["gpt-6-sol", "gpt-6.1-sol"]), ["gpt-6.1-sol"]);
        assert_eq!(
            active(&["gpt-5.6-sol", "gpt-6.1-sol", "gpt-6-sol", "gpt-6.1-terra"]),
            ["gpt-6.1-sol", "gpt-6.1-terra"]
        );
    }

    #[test]
    fn effort_tiers_of_the_newest_version_stay_on() {
        let all = [
            "claude-gemini-3.8-flash-high-via-antigravity",
            "claude-gemini-3.8-flash-low-via-antigravity",
            "claude-gemini-3.5-flash-low-via-antigravity",
            "claude-gemini-3.1-pro-high-via-antigravity",
        ];
        assert_eq!(
            active(&all),
            [
                "claude-gemini-3.1-pro-high-via-antigravity",
                "claude-gemini-3.8-flash-high-via-antigravity",
                "claude-gemini-3.8-flash-low-via-antigravity",
            ]
        );
    }

    #[test]
    fn dashed_and_prefixed_versions_compare_numerically() {
        assert_eq!(
            active(&[
                "claude-sonnet-4-5",
                "claude-sonnet-4-6",
                "claude-sonnet-4-10"
            ]),
            ["claude-sonnet-4-10"]
        );
        assert_eq!(
            active(&[
                "opencode/muse-spark-1.2-free",
                "opencode/muse-spark-1.3-free"
            ]),
            ["opencode/muse-spark-1.3-free"]
        );
    }

    #[test]
    fn unversioned_names_are_always_on() {
        assert_eq!(
            active(&["opencode/big-pickle", "opencode/beam-free"]),
            ["opencode/beam-free", "opencode/big-pickle"]
        );
    }

    #[test]
    fn overrides_win_and_equal_overrides_are_dropped() {
        let all = ids(&["gpt-6-sol", "gpt-6.1-sol"]);
        let mut prefs = ModelPrefs::default();
        prefs.set("codex", &all, "gpt-6-sol", true);
        prefs.set("codex", &all, "gpt-6.1-sol", true); // already the default
        let resolved = prefs.resolve("codex", &all);
        assert_eq!(
            resolved
                .iter()
                .map(|r| (r.active, r.reason))
                .collect::<Vec<_>>(),
            [(true, "override"), (true, "default")]
        );
        prefs.set("codex", &all, "gpt-6-sol", false); // back to the default
        assert!(prefs.overrides.is_empty());
    }

    #[test]
    fn file_round_trip_and_bad_file_means_defaults() {
        let dir = std::env::temp_dir().join(format!("shunt-models-{}", std::process::id()));
        let path = dir.join("models.json");
        assert_eq!(load_from(&path), ModelPrefs::default());
        let mut prefs = ModelPrefs {
            filter: false,
            ..ModelPrefs::default()
        };
        prefs.set(
            "codex",
            &ids(&["gpt-6-sol", "gpt-6.1-sol"]),
            "gpt-6-sol",
            true,
        );
        save_to(&path, &prefs).unwrap();
        assert_eq!(load_from(&path), prefs);
        std::fs::write(&path, "not json").unwrap();
        assert_eq!(load_from(&path), ModelPrefs::default());
        std::fs::remove_dir_all(dir).unwrap();
    }
}

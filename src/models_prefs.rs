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

/// A stored ranking of the OpenCode free models: when the headless agent made
/// it and the validated order it returned.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpenCodeRank {
    /// RFC 3339 UTC timestamp of the run.
    pub ranked_at: String,
    /// Live model ids, best first.
    pub order: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ModelPrefs {
    /// When off, consumers see every model, as if this feature did not exist.
    pub filter: bool,
    /// The headless-agent platform the OpenCode rerank runs: `claude` (default),
    /// `codex`, `opencode` or `agy`.
    pub ranker: String,
    /// `platform -> model id -> forced state`, only where it differs from
    /// the computed default.
    pub overrides: BTreeMap<String, BTreeMap<String, bool>>,
    /// The last OpenCode ranking, or `None` before the first rerank.
    pub opencode_rank: Option<OpenCodeRank>,
}

impl Default for ModelPrefs {
    fn default() -> Self {
        Self {
            filter: true,
            ranker: "claude".to_string(),
            overrides: BTreeMap::new(),
            opencode_rank: None,
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
    /// 1-based position in the stored OpenCode ranking; `None` for the other
    /// platforms, and before the first rerank.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rank: Option<u32>,
}

/// Live `ids` in a stored ranking's order: the ranked ids that are still live,
/// deduplicated, then every live id the ranking does not name, in catalog order.
pub fn rank_order(order: &[String], ids: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(ids.len());
    for id in order {
        if ids.contains(id) && !out.contains(id) {
            out.push(id.clone());
        }
    }
    for id in ids {
        if !out.contains(id) {
            out.push(id.clone());
        }
    }
    out
}

impl ModelPrefs {
    /// Resolve every id of one platform. The OpenCode entries follow a stored
    /// ranking, with `rank` set, when one exists; every other platform keeps
    /// the catalog order.
    pub fn resolve(&self, platform: &str, ids: &[String]) -> Vec<Resolved> {
        let defaults = default_active(ids);
        let overrides = self.overrides.get(platform);
        let rank = (platform == "opencode")
            .then_some(self.opencode_rank.as_ref())
            .flatten();
        let ordered = rank.map_or_else(|| ids.to_vec(), |rank| rank_order(&rank.order, ids));
        ordered
            .iter()
            .enumerate()
            .map(|(at, id)| Resolved {
                id: id.clone(),
                active: overrides
                    .and_then(|o| o.get(id))
                    .copied()
                    .unwrap_or_else(|| defaults.contains(id)),
                reason: if overrides.is_some_and(|o| o.contains_key(id)) {
                    "override"
                } else {
                    "default"
                },
                rank: rank.map(|_| at as u32 + 1),
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

/// Effort levels that ids such as `…-flash-high` carry. `max` and `xhigh` are
/// not here on purpose: `gpt-5.1-codex-max` is a different model, not a tier.
const TIERS: [&str; 3] = ["low", "medium", "high"];

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

/// `6.1` -> `[6, 1]`, `v2.6` -> `[2, 6]`, `6` -> `[6]`. Each part is one to
/// three digits with no leading zero, so dates and build numbers (`0905`,
/// `2411`, `20251001`) are not versions.
fn version_part(token: &str) -> Option<Vec<u32>> {
    let token = token.strip_prefix('v').unwrap_or(token);
    token
        .split('.')
        .map(|part| {
            let plain = !part.is_empty()
                && part.len() <= 3
                && part.bytes().all(|b| b.is_ascii_digit())
                && (part == "0" || !part.starts_with('0'));
            plain.then(|| part.parse().ok()).flatten()
        })
        .collect()
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

static PENDING: std::sync::Mutex<Option<ModelPrefs>> = std::sync::Mutex::new(None);
static WRITING: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Remember the newest snapshot to write. Call it in event order, on the
/// thread that produced the snapshots; it never blocks on disk.
pub fn queue(prefs: ModelPrefs) {
    *PENDING.lock().unwrap_or_else(|e| e.into_inner()) = Some(prefs);
}

/// Write the newest queued snapshot, if any. Writes are serialized, and each
/// one takes the latest snapshot at that moment, so two quick toggles can never
/// leave the older state on disk.
pub fn flush() -> anyhow::Result<()> {
    match default_path() {
        Some(path) => flush_to(&path),
        None => Ok(()),
    }
}

fn flush_to(path: &Path) -> anyhow::Result<()> {
    let _writing = WRITING.lock().unwrap_or_else(|e| e.into_inner());
    let next = PENDING.lock().unwrap_or_else(|e| e.into_inner()).take();
    match next {
        Some(prefs) => save_to(path, &prefs),
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
    fn dates_and_snapshots_are_not_versions_and_max_is_not_a_tier() {
        // 0905 / 0613 / 2411 are dates: no version, so both ids stay on.
        assert_eq!(
            active(&["kimi-k2-0905", "kimi-k2-0711"]),
            ["kimi-k2-0711", "kimi-k2-0905"]
        );
        assert_eq!(active(&["gpt-4-0613", "gpt-4.1"]).len(), 2);
        // codex-max is its own model, not an effort tier of codex.
        assert_eq!(
            active(&["gpt-5.1-codex", "gpt-5.1-codex-max", "gpt-5.2-codex"]),
            ["gpt-5.1-codex-max", "gpt-5.2-codex"]
        );
    }

    #[test]
    fn queued_snapshots_write_newest_once() {
        let dir = std::env::temp_dir().join(format!("shunt-models-q-{}", std::process::id()));
        let path = dir.join("models.json");
        queue(ModelPrefs {
            filter: false,
            ..ModelPrefs::default()
        });
        queue(ModelPrefs::default()); // newer snapshot replaces the older one
        flush_to(&path).unwrap();
        assert!(load_from(&path).filter, "the newest snapshot is written");
        std::fs::remove_file(&path).unwrap();
        flush_to(&path).unwrap(); // nothing queued: no write
        assert!(!path.exists());
        std::fs::remove_dir_all(dir).unwrap();
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

    #[test]
    fn rank_order_drops_unknown_ids_and_appends_missing_ones() {
        let order = ids(&["b", "gone", "a", "b"]);
        assert_eq!(
            rank_order(&order, &ids(&["a", "b", "c"])),
            ["b", "a", "c"],
            "unknown ids go, duplicates collapse, missing ids append in catalog order"
        );
    }

    #[test]
    fn resolve_numbers_opencode_rows_in_stored_order() {
        let all = ids(&["a", "b", "c"]);
        let mut prefs = ModelPrefs::default();
        assert_eq!(prefs.ranker, "claude");
        prefs.opencode_rank = Some(OpenCodeRank {
            ranked_at: "2026-10-17T14:15:37Z".into(),
            order: ids(&["c", "a"]),
        });
        // `c` is unversioned and on; `a` and `b` are versionless too, so all on.
        let rows = prefs.resolve("opencode", &all);
        assert_eq!(
            rows.iter()
                .map(|r| (r.id.as_str(), r.rank))
                .collect::<Vec<_>>(),
            [("c", Some(1)), ("a", Some(2)), ("b", Some(3))]
        );
        // Another platform ignores the ranking and carries no rank.
        assert_eq!(
            prefs
                .resolve("codex", &all)
                .iter()
                .map(|r| r.rank)
                .collect::<Vec<_>>(),
            [None, None, None]
        );
    }

    #[test]
    fn the_ranker_and_ranking_survive_a_file_round_trip() {
        let dir = std::env::temp_dir().join(format!("shunt-rank-{}", std::process::id()));
        let path = dir.join("models.json");
        let prefs = ModelPrefs {
            ranker: "codex".into(),
            opencode_rank: Some(OpenCodeRank {
                ranked_at: "2026-10-17T14:15:37Z".into(),
                order: ids(&["opencode/big-pickle"]),
            }),
            ..ModelPrefs::default()
        };
        save_to(&path, &prefs).unwrap();
        assert_eq!(load_from(&path), prefs);
        // A file written before the ranking existed still loads: both fields
        // fall back to their defaults.
        std::fs::write(&path, r#"{"filter": true, "overrides": {}}"#).unwrap();
        assert_eq!(load_from(&path), ModelPrefs::default());
        std::fs::remove_dir_all(dir).unwrap();
    }
}

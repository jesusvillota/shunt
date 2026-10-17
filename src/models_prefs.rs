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

/// Preserve the original JSON representation until the operator picks a ranker.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum Ranker {
    Legacy(String),
    Selection {
        platform: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        model: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        effort: Option<String>,
    },
}

impl From<&str> for Ranker {
    fn from(platform: &str) -> Self {
        Self::Legacy(platform.into())
    }
}

impl Ranker {
    pub fn platform(&self) -> &str {
        match self {
            Self::Legacy(platform) | Self::Selection { platform, .. } => platform,
        }
    }

    pub fn model(&self) -> Option<&str> {
        match self {
            Self::Legacy(_) => None,
            Self::Selection { model, .. } => model.as_deref(),
        }
    }

    pub fn effort(&self) -> Option<&str> {
        match self {
            Self::Legacy(_) => None,
            Self::Selection { effort, .. } => effort.as_deref(),
        }
    }
}

/// Validated parameter scores and their locally computed weighted mean.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelScore {
    pub parameters: BTreeMap<String, f64>,
    pub total: f64,
}

/// Stored OpenCode ranking, including scores when available.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OpenCodeRank {
    /// RFC 3339 UTC timestamp of the run.
    pub ranked_at: String,
    /// Live model ids, best first.
    pub order: Vec<String>,
    /// Missing in rankings saved before weighted scoring.
    #[serde(default)]
    pub scores: BTreeMap<String, ModelScore>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ModelPrefs {
    /// When off, consumers see every model, as if this feature did not exist.
    pub filter: bool,
    /// The headless-agent platform the OpenCode rerank runs: `claude` (default),
    /// `codex`, `opencode` or `agy`.
    pub ranker: Ranker,
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
            ranker: "claude".into(),
            overrides: BTreeMap::new(),
            opencode_rank: None,
        }
    }
}

/// One catalog entry with its resolved state.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Resolved {
    pub id: String,
    pub active: bool,
    /// `"default"` or `"override"`.
    pub reason: &'static str,
    /// 1-based position in the stored OpenCode ranking; `None` for the other
    /// platforms, and before the first rerank.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rank: Option<u32>,
    /// Weighted mean, or null when this model has no valid scores.
    pub score: Option<f64>,
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
                score: rank.and_then(|r| r.scores.get(id)).map(|s| s.total),
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
mod tests;

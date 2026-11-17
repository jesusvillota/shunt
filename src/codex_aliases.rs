//! Keeps the Codex family aliases in `shunt.toml` on the newest model of each
//! family.
//!
//! A `[[routes]]` entry with `provider = "codex"` and an `upstream_model` such
//! as `gpt-6.1-sol` is an alias: clients ask for a stable name (`codex-sol`) and
//! shunt rewrites it to the current slug. [`sync`] moves each alias to the
//! newest model that is on in `shunt models codex`, so a new Codex release needs
//! no config edit. It changes only the `upstream_model` value, never adds or
//! removes a route, and writes nothing when the live model list is empty.
//!
//! Limits, by design:
//! - The model list is `codex debug models`, which is the catalog built into the
//!   installed `codex` when its live refresh returns nothing (as it does behind
//!   shunt). A model appears here when `codex` is upgraded.
//! - shunt sends a pinned Codex client identity (`CODEX_CLIENT_VERSION`). A new
//!   model that needs a newer client is rejected upstream even though this
//!   module moved the alias to it; bump the pin, or put the alias back by hand.
//!
//! It shells out through [`crate::models_catalog::fetch`], so run it from a
//! login shell (`shunt sync-routes`, `shunt tui`), not from the gateway daemon.

use std::{collections::HashMap, path::Path};

use anyhow::{bail, Context};
use toml_edit::{DocumentMut, Item, Value};

use crate::{
    config::{Config, ConfigFormat},
    models_catalog,
    models_prefs::{self, family_and_version},
};

/// One alias moved to a newer model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub alias: String,
    pub from: String,
    pub to: String,
}

/// What [`retarget`] found in a config text.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Retarget {
    /// The edited text; empty when nothing changed.
    pub text: String,
    pub changes: Vec<Change>,
    /// Aliases left as they are, with the reason.
    pub warnings: Vec<String>,
    /// How many codex routes with an `upstream_model` the file has.
    pub aliases: usize,
}

/// The newest id of each family among `active`, keyed by family.
fn newest_per_family(active: &[String]) -> HashMap<String, (Vec<u32>, &str)> {
    let mut newest: HashMap<String, (Vec<u32>, &str)> = HashMap::new();
    for id in active {
        let Some((family, version)) = family_and_version(id) else {
            continue;
        };
        match newest.get(&family) {
            Some((best, _)) if *best >= version => {}
            _ => {
                newest.insert(family, (version, id));
            }
        }
    }
    newest
}

/// Move every Codex alias in `text` to the newest active model of its family.
///
/// `all` is every model the catalog lists and `active` the ones that are on. An
/// alias whose current model is missing from `all` (the catalog dropped it) only
/// moves forward: to a newer model, never to an older one. An alias whose
/// current model is listed but switched off moves to the newest model that is on.
/// An alias whose `upstream_model` has no version, or whose family has no model
/// that is on, is left alone with a warning.
pub fn retarget(text: &str, all: &[String], active: &[String]) -> anyhow::Result<Retarget> {
    let mut doc = text
        .parse::<DocumentMut>()
        .context("the config file is not valid TOML")?;
    let newest = newest_per_family(active);
    let mut out = Retarget::default();
    if let Some(Item::ArrayOfTables(routes)) = doc.get_mut("routes") {
        for route in routes.iter_mut() {
            if route.get("provider").and_then(Item::as_str) != Some("codex") {
                continue;
            }
            let alias = route
                .get("model")
                .and_then(Item::as_str)
                .unwrap_or_default()
                .to_string();
            let Some(Item::Value(upstream)) = route.get_mut("upstream_model") else {
                continue;
            };
            let Some(current) = upstream.as_str().map(String::from) else {
                continue;
            };
            out.aliases += 1;
            let Some((family, current_version)) = family_and_version(&current) else {
                out.warnings
                    .push(format!("{alias}: {current} has no version; left as it is"));
                continue;
            };
            let Some((version, to)) = newest.get(&family) else {
                out.warnings.push(format!(
                    "{alias}: no model of family {family:?} is on; left on {current}"
                ));
                continue;
            };
            if *to == current {
                continue;
            }
            if !all.contains(&current) && *version < current_version {
                out.warnings.push(format!(
                    "{alias}: {current} is not in the catalog and {to} is older; left as it is"
                ));
                continue;
            }
            if !crate::config::model_supports_tool_search(to) {
                out.warnings.push(format!(
                    "{alias}: {to} is not covered by shunt's native tool_search rule \
                     (`is_gpt6_or_later_slug` in src/config.rs); it uses the slower shim"
                ));
            }
            let decor = upstream.decor().clone();
            let mut replacement = Value::from(*to);
            *replacement.decor_mut() = decor;
            *upstream = replacement;
            out.changes.push(Change {
                alias,
                from: current,
                to: (*to).to_string(),
            });
        }
    }
    if !out.changes.is_empty() {
        out.text = doc.to_string();
    }
    Ok(out)
}

/// Every Codex model the catalog lists, and the ones that are on in
/// `shunt models codex`.
fn codex_models(config: Option<&Path>) -> anyhow::Result<(Vec<String>, Vec<String>)> {
    let all = models_catalog::fetch("codex", config).map_err(anyhow::Error::msg)?;
    if all.is_empty() {
        bail!("`codex debug models` listed no models; leaving the config alone");
    }
    let active: Vec<String> = models_prefs::load()
        .resolve("codex", &all)
        .into_iter()
        .filter(|model| model.active)
        .map(|model| model.id)
        .collect();
    if active.is_empty() {
        bail!("no Codex model is on in `shunt models codex`; leaving the config alone");
    }
    Ok((all, active))
}

/// Move the Codex aliases of the config file to the newest active models.
/// With `dry_run` the file is not written. Returns a report for the operator.
pub fn sync(config: Option<&Path>, dry_run: bool) -> anyhow::Result<String> {
    let path = config
        .map(Path::to_path_buf)
        .or_else(Config::find_config_file)
        .context("no shunt config file found; pass --config <path>")?;
    if ConfigFormat::from_path(&path) != ConfigFormat::Toml {
        bail!(
            "{} is not TOML; sync-routes edits shunt.toml only",
            path.display()
        );
    }
    let (all, active) = codex_models(config)?;
    sync_file(&path, &all, &active, dry_run)
}

/// [`sync`] with the model lists given, so tests need no `codex` binary.
fn sync_file(
    path: &Path,
    all: &[String],
    active: &[String],
    dry_run: bool,
) -> anyhow::Result<String> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let found = retarget(&text, all, active)?;
    if !dry_run && !found.changes.is_empty() {
        crate::atomic_file::write_private_atomic(path, found.text.as_bytes())
            .with_context(|| format!("writing {}", path.display()))?;
    }
    let mut report = String::new();
    for change in &found.changes {
        report.push_str(&format!(
            "{}: {} -> {}\n",
            change.alias, change.from, change.to
        ));
    }
    for warning in &found.warnings {
        report.push_str(&format!("warning: {warning}\n"));
    }
    if found.aliases == 0 {
        report.push_str(&format!(
            "warning: found no codex alias in {} (a [[routes]] table with provider = \"codex\" \
             and an upstream_model); nothing to sync\n",
            path.display()
        ));
    } else if found.changes.is_empty() {
        report.push_str("Codex aliases are up to date.\n");
    } else if dry_run {
        report.push_str("Dry run: nothing written.\n");
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFIG: &str = r#"[[routes]]
model = "gpt-5.6-sol"
provider = "codex"

[[routes]]
model = "codex-sol"
provider = "codex"
upstream_model = "gpt-6.1-sol" # newest sol

[[routes]]
model = "codex-astra"
provider = "codex"
upstream_model = "gpt-6-astra"

[[routes]]
model = "gemini-alias"
provider = "antigravity"
upstream_model = "gpt-6.1-sol"
"#;

    fn ids(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    /// `all` and `active` the same, as for a catalog with nothing switched off.
    fn run(text: &str, models: &[&str]) -> Retarget {
        let models = ids(models);
        retarget(text, &models, &models).unwrap()
    }

    #[test]
    fn moves_an_alias_to_the_newest_model_of_its_family() {
        let result = run(CONFIG, &["gpt-6.1-sol", "gpt-6.2-sol", "gpt-6-astra"]);
        assert_eq!(
            result.changes,
            vec![Change {
                alias: "codex-sol".into(),
                from: "gpt-6.1-sol".into(),
                to: "gpt-6.2-sol".into(),
            }]
        );
        assert_eq!(result.aliases, 2);
        assert!(result
            .text
            .contains("upstream_model = \"gpt-6.2-sol\" # newest sol"));
        // Everything else is untouched, including a non-codex provider.
        assert!(result
            .text
            .contains("model = \"gpt-5.6-sol\"\nprovider = \"codex\"\n"));
        assert!(result
            .text
            .contains("provider = \"antigravity\"\nupstream_model = \"gpt-6.1-sol\""));
    }

    #[test]
    fn an_older_model_in_the_catalog_never_wins() {
        let models = ["gpt-6.1-sol", "gpt-5.6-sol", "gpt-6-astra"];
        let result = run(CONFIG, &models);
        assert!(result.changes.is_empty());
        assert!(result.text.is_empty());
    }

    #[test]
    fn a_model_the_catalog_dropped_is_never_replaced_by_an_older_one() {
        // gpt-6.1-sol vanished from the catalog (a codex downgrade).
        let result = run(CONFIG, &["gpt-6-sol", "gpt-6-astra"]);
        assert!(result.changes.is_empty());
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].contains("is not in the catalog and gpt-6-sol is older"));
    }

    #[test]
    fn a_model_the_catalog_dropped_moves_forward() {
        let text =
            "[[routes]]\nmodel = \"a\"\nprovider = \"codex\"\nupstream_model = \"gpt-5.6-sol\"\n";
        let result = run(text, &["gpt-6.1-sol"]);
        assert_eq!(result.changes[0].to, "gpt-6.1-sol");
    }

    #[test]
    fn a_model_switched_off_moves_to_the_newest_one_that_is_on() {
        let all = ids(&["gpt-6.1-sol", "gpt-6-sol"]);
        let active = ids(&["gpt-6-sol"]);
        let result = retarget(CONFIG, &all, &active).unwrap();
        assert_eq!(result.changes[0].to, "gpt-6-sol");
    }

    #[test]
    fn a_family_with_nothing_on_warns_and_keeps_the_alias() {
        let result = run(CONFIG, &["gpt-6.1-sol"]);
        assert!(result.changes.is_empty());
        assert_eq!(
            result.warnings,
            vec![
                "codex-astra: no model of family \"gpt-astra\" is on; left on gpt-6-astra"
                    .to_string()
            ]
        );
    }

    #[test]
    fn a_new_model_outside_the_tool_search_rule_warns() {
        // gpt-5.3 is older than the gpt-5.4 line where native tool_search starts.
        let text =
            "[[routes]]\nmodel = \"a\"\nprovider = \"codex\"\nupstream_model = \"gpt-5.2-foo\"\n";
        let result = run(text, &["gpt-5.3-foo"]);
        assert_eq!(result.changes[0].to, "gpt-5.3-foo");
        assert!(result
            .warnings
            .iter()
            .any(|w| w.contains("gpt-5.3-foo is not covered by shunt's native tool_search rule")));
    }

    #[test]
    fn a_new_gpt6_or_later_release_needs_no_tool_search_warning() {
        let result = run(CONFIG, &["gpt-6.2-sol", "gpt-6-astra"]);
        assert_eq!(result.changes[0].to, "gpt-6.2-sol");
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn the_family_decides_not_the_alias_name() {
        let text = "[[routes]]\nmodel = \"anything\"\nprovider = \"codex\"\nupstream_model = \"gpt-6-luna\"\n";
        let result = run(text, &["gpt-6.1-luna"]);
        assert_eq!(result.changes[0].to, "gpt-6.1-luna");
    }

    #[test]
    fn an_inline_routes_layout_finds_no_alias() {
        let text = "routes = [{ model = \"codex-sol\", provider = \"codex\", upstream_model = \"gpt-6.1-sol\" }]\n";
        let result = run(text, &["gpt-6.2-sol"]);
        assert_eq!(result.aliases, 0);
        assert!(result.changes.is_empty());
    }

    #[test]
    fn invalid_toml_is_an_error() {
        let models = ids(&["gpt-6.1-sol"]);
        assert!(retarget("[[routes]\n", &models, &models).is_err());
    }

    /// A config file in its own temp directory, removed when dropped.
    struct TempConfig(std::path::PathBuf);

    impl TempConfig {
        fn new(name: &str, text: &str) -> Self {
            let dir = std::env::temp_dir()
                .join(format!("shunt-codex-aliases-{name}-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            let path = dir.join("shunt.toml");
            std::fs::write(&path, text).unwrap();
            Self(path)
        }
    }

    impl Drop for TempConfig {
        fn drop(&mut self) {
            if let Some(dir) = self.0.parent() {
                let _ = std::fs::remove_dir_all(dir);
            }
        }
    }

    #[test]
    fn dry_run_writes_nothing_and_a_real_run_writes_once() {
        let config = TempConfig::new("write", CONFIG);
        let path = &config.0;
        let models = ids(&["gpt-6.2-sol", "gpt-6-astra"]);

        let report = sync_file(path, &models, &models, true).unwrap();
        assert!(report.contains("codex-sol: gpt-6.1-sol -> gpt-6.2-sol"));
        assert!(report.contains("Dry run: nothing written."));
        assert_eq!(std::fs::read_to_string(path).unwrap(), CONFIG);

        let report = sync_file(path, &models, &models, false).unwrap();
        assert!(report.contains("codex-sol: gpt-6.1-sol -> gpt-6.2-sol"));
        assert!(std::fs::read_to_string(path)
            .unwrap()
            .contains("upstream_model = \"gpt-6.2-sol\" # newest sol"));

        let report = sync_file(path, &models, &models, false).unwrap();
        assert!(report.contains("Codex aliases are up to date."));
    }

    #[test]
    fn a_file_with_no_alias_reports_it() {
        let config = TempConfig::new("none", "[server]\nbind = \"127.0.0.1:3001\"\n");
        let models = ids(&["gpt-6.1-sol"]);
        let report = sync_file(&config.0, &models, &models, false).unwrap();
        assert!(report.contains("found no codex alias"));
    }
}

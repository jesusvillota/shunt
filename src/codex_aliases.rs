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
    pub text: String,
    pub changes: Vec<Change>,
    /// Aliases left as they are because no active model is in their family.
    pub warnings: Vec<String>,
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

/// Move every Codex alias in `text` to the newest model of its family in
/// `active`. An alias whose `upstream_model` has no version, or whose family has
/// no active model, is left alone.
pub fn retarget(text: &str, active: &[String]) -> anyhow::Result<Retarget> {
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
            let Some((family, _)) = family_and_version(&current) else {
                continue;
            };
            let Some((_, to)) = newest.get(&family) else {
                out.warnings
                    .push(format!("{alias}: no active model in family {family:?}"));
                continue;
            };
            if *to == current {
                continue;
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

/// The Codex ids that are on in `shunt models codex`.
fn active_codex_ids(config: Option<&Path>) -> anyhow::Result<Vec<String>> {
    let ids = models_catalog::fetch("codex", config).map_err(anyhow::Error::msg)?;
    if ids.is_empty() {
        bail!("`codex debug models` listed no models; leaving the config alone");
    }
    let active: Vec<String> = models_prefs::load()
        .resolve("codex", &ids)
        .into_iter()
        .filter(|model| model.active)
        .map(|model| model.id)
        .collect();
    if active.is_empty() {
        bail!("no Codex model is on in `shunt models codex`; leaving the config alone");
    }
    Ok(active)
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
    let active = active_codex_ids(config)?;
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let found = retarget(&text, &active)?;
    if !dry_run && !found.changes.is_empty() {
        crate::atomic_file::write_private_atomic(&path, found.text.as_bytes())
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
    if found.changes.is_empty() {
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
model = "codex-legacy"
provider = "codex"
upstream_model = "gpt-4o-legacy-model"

[[routes]]
model = "gemini-alias"
provider = "antigravity"
upstream_model = "gpt-6.1-sol"
"#;

    fn ids(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn moves_an_alias_to_the_newest_model_of_its_family() {
        let result = retarget(CONFIG, &ids(&["gpt-6.2-sol", "gpt-6-astra"])).unwrap();
        assert_eq!(
            result.changes,
            vec![Change {
                alias: "codex-sol".into(),
                from: "gpt-6.1-sol".into(),
                to: "gpt-6.2-sol".into(),
            }]
        );
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
    fn an_older_active_model_does_not_win() {
        let result =
            retarget(CONFIG, &ids(&["gpt-6.1-sol", "gpt-5.6-sol", "gpt-6-astra"])).unwrap();
        assert!(result.changes.is_empty());
        assert!(result.text.is_empty());
    }

    #[test]
    fn a_missing_family_warns_and_keeps_the_alias() {
        let result = retarget(CONFIG, &ids(&["gpt-6.1-sol"])).unwrap();
        assert!(result.changes.is_empty());
        assert_eq!(
            result.warnings,
            vec!["codex-astra: no active model in family \"gpt-astra\"".to_string()]
        );
    }

    #[test]
    fn a_family_name_decides_not_the_alias_name() {
        let text = "[[routes]]\nmodel = \"anything\"\nprovider = \"codex\"\nupstream_model = \"gpt-6-luna\"\n";
        let result = retarget(text, &ids(&["gpt-6.1-luna"])).unwrap();
        assert_eq!(result.changes[0].to, "gpt-6.1-luna");
    }

    #[test]
    fn invalid_toml_is_an_error() {
        assert!(retarget("[[routes]\n", &ids(&["gpt-6.1-sol"])).is_err());
    }
}

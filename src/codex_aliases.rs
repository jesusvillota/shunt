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
//! The model list is `codex debug models`, which is the catalog built into the
//! installed `codex` when its live refresh returns nothing (as it does behind
//! shunt). A model appears here when `codex` is upgraded, and that `codex` can be
//! newer than the client identity shunt sends (`CODEX_CLIENT_VERSION`). So an
//! alias moves only after the running gateway has run the new model once with
//! that pinned identity ([`probe_at`]); a model Codex refuses, or one that cannot
//! be tested (quota, gateway down), leaves the alias where it is and is retried
//! on the next run.
//!
//! It shells out through [`crate::models_catalog::fetch`], so run it from a
//! login shell (`shunt sync-routes`, `shunt tui`), not from the gateway daemon.

use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream},
    path::Path,
    time::Duration,
};

use anyhow::{bail, Context};
use toml_edit::{DocumentMut, Item, Value};

use crate::{
    adapters::responses::request::{CODEX_CLIENT_VERSION, CODEX_USER_AGENT},
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
    let doc = text
        .parse::<DocumentMut>()
        .context("the config file is not valid TOML")?;
    let newest = newest_per_family(active);
    let mut out = Retarget::default();
    if let Some(Item::ArrayOfTables(routes)) = doc.get("routes") {
        for route in routes.iter() {
            if route.get("provider").and_then(Item::as_str) != Some("codex") {
                continue;
            }
            let alias = route
                .get("model")
                .and_then(Item::as_str)
                .unwrap_or_default()
                .to_string();
            let Some(current) = route
                .get("upstream_model")
                .and_then(Item::as_str)
                .map(String::from)
            else {
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
            out.changes.push(Change {
                alias,
                from: current,
                to: (*to).to_string(),
            });
        }
    }
    Ok(out)
}

/// `text` with each change applied to its alias. A change whose alias no longer
/// has `from` as its `upstream_model` (the file was edited meanwhile) is
/// skipped; the second value lists the changes that were applied. Only the
/// `upstream_model` value changes, so comments and layout stay.
pub fn apply_changes(text: &str, changes: &[Change]) -> anyhow::Result<(String, Vec<Change>)> {
    let mut doc = text
        .parse::<DocumentMut>()
        .context("the config file is not valid TOML")?;
    let mut applied = Vec::new();
    if let Some(Item::ArrayOfTables(routes)) = doc.get_mut("routes") {
        for route in routes.iter_mut() {
            if route.get("provider").and_then(Item::as_str) != Some("codex") {
                continue;
            }
            let model = route.get("model").and_then(Item::as_str);
            let Some(change) = changes.iter().find(|c| Some(c.alias.as_str()) == model) else {
                continue;
            };
            let Some(Item::Value(upstream)) = route.get_mut("upstream_model") else {
                continue;
            };
            if upstream.as_str() != Some(change.from.as_str()) {
                continue;
            }
            let decor = upstream.decor().clone();
            let mut replacement = Value::from(change.to.as_str());
            *replacement.decor_mut() = decor;
            *upstream = replacement;
            applied.push(change.clone());
        }
    }
    Ok((doc.to_string(), applied))
}

/// What the gateway said when asked to run one model once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Probe {
    /// Codex accepted the model with shunt's pinned client identity.
    Works,
    /// Codex refused the model: it is not served to this account, or the pinned
    /// client version is too old for it.
    Refused,
    /// No answer either way (quota used up, gateway down, no auth), with why.
    Unknown(String),
}

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const ANSWER_TIMEOUT: Duration = Duration::from_secs(45);

/// Run `model` once through the gateway's inbound Codex endpoint at `addr`,
/// which sends the model name as given (no route needed) and forwards our
/// identity headers unchanged. The headers are the ones the outbound path sends,
/// so the answer is the answer real traffic would get. The connection is closed
/// as soon as the answer starts, which cancels the generation.
pub fn probe_at(addr: &str, token: Option<&str>, model: &str) -> Probe {
    let Ok(socket) = addr.parse::<SocketAddr>() else {
        return Probe::Unknown(format!("bad gateway address {addr}"));
    };
    let mut stream = match TcpStream::connect_timeout(&socket, CONNECT_TIMEOUT) {
        Ok(stream) => stream,
        Err(error) => {
            return Probe::Unknown(format!("cannot reach the gateway at {addr}: {error}"))
        }
    };
    let _ = stream.set_read_timeout(Some(ANSWER_TIMEOUT));
    let _ = stream.set_write_timeout(Some(ANSWER_TIMEOUT));
    let body = serde_json::json!({
        "model": model,
        "instructions": "Reply with the word OK.",
        "input": [{"type": "message", "role": "user",
                   "content": [{"type": "input_text", "text": "hi"}]}],
        "store": false,
        "stream": true,
    })
    .to_string();
    let auth = token.map_or_else(String::new, |t| format!("authorization: Bearer {t}\r\n"));
    let request = format!(
        "POST /v1/responses HTTP/1.1\r\nhost: {addr}\r\nconnection: close\r\n\
         content-type: application/json\r\naccept: text/event-stream\r\n\
         originator: codex_cli_rs\r\nuser-agent: {CODEX_USER_AGENT}\r\n\
         version: {CODEX_CLIENT_VERSION}\r\n{auth}content-length: {}\r\n\r\n{body}",
        body.len()
    );
    if let Err(error) = stream.write_all(request.as_bytes()) {
        return Probe::Unknown(format!("cannot send to the gateway: {error}"));
    }
    let mut answer = Vec::new();
    let mut chunk = [0u8; 2048];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => {
                answer.extend_from_slice(&chunk[..read]);
                let ready = answer.windows(4).any(|w| w == b"\r\n\r\n");
                if answer.len() >= 8192 || (ready && status_of(&answer) == Some(200)) {
                    break;
                }
            }
            Err(error) => return Probe::Unknown(format!("no answer from the gateway: {error}")),
        }
    }
    let text = String::from_utf8_lossy(&answer).to_lowercase();
    match status_of(&answer) {
        Some(200) => Probe::Works,
        Some(400 | 404)
            if text.contains("model_not_found")
                || text.contains("not supported")
                || text.contains("model not found") =>
        {
            Probe::Refused
        }
        Some(429) => Probe::Unknown("Codex quota is used up (HTTP 429)".into()),
        Some(code) => Probe::Unknown(format!("the gateway answered HTTP {code}")),
        None => Probe::Unknown("the gateway sent no HTTP answer".into()),
    }
}

/// The status code of an HTTP response that has at least its first line.
fn status_of(answer: &[u8]) -> Option<u16> {
    let line = answer.split(|&b| b == b'\n').next()?;
    String::from_utf8_lossy(line)
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()
}

/// Where the running gateway listens and the client token to use, from its config.
fn gateway(config: Option<&Path>) -> Result<(String, Option<String>), String> {
    // `Config::load` logs through tracing to stderr, which is the TUI screen
    // when this runs inside `shunt tui`.
    let config =
        tracing::subscriber::with_default(tracing::subscriber::NoSubscriber::default(), || {
            Config::load(config)
        })
        .map_err(|e| e.to_string())?;
    let bind: SocketAddr = config
        .server
        .bind
        .parse()
        .map_err(|_| format!("[server] bind {:?} is not an address", config.server.bind))?;
    let host = if bind.ip().is_unspecified() {
        IpAddr::V4(Ipv4Addr::LOCALHOST)
    } else {
        bind.ip()
    };
    let token = config
        .server
        .auth
        .as_ref()
        .and_then(|auth| auth.tokens_file.as_deref())
        .and_then(|path| crate::config::read_token_file(path).ok())
        .and_then(|pairs| {
            let at = pairs.iter().position(|(name, _)| name == "default");
            pairs
                .into_iter()
                .nth(at.unwrap_or(0))
                .map(|(_, token)| token)
        });
    Ok((SocketAddr::new(host, bind.port()).to_string(), token))
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
    let gateway = gateway(config);
    let mut probe = |model: &str| match &gateway {
        Ok((addr, token)) => probe_at(addr, token.as_deref(), model),
        Err(reason) => Probe::Unknown(reason.clone()),
    };
    sync_file(&path, &all, &active, dry_run, &mut probe)
}

/// [`sync`] with the model lists and the model test given, so tests need no
/// `codex` binary and no gateway. Each new model is tested once; an alias moves
/// only when its model works. The file is read again just before the write, so
/// an edit made while the tests ran is kept.
fn sync_file(
    path: &Path,
    all: &[String],
    active: &[String],
    dry_run: bool,
    probe: &mut dyn FnMut(&str) -> Probe,
) -> anyhow::Result<String> {
    let read =
        || std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()));
    let found = retarget(&read()?, all, active)?;
    let mut answers: HashMap<String, Probe> = HashMap::new();
    let mut verified = Vec::new();
    let mut notes = Vec::new();
    for change in &found.changes {
        let answer = answers
            .entry(change.to.clone())
            .or_insert_with(|| probe(&change.to))
            .clone();
        match answer {
            Probe::Works => verified.push(change.clone()),
            Probe::Refused => notes.push(format!(
                "{}: Codex refused {} with shunt's pinned client identity {CODEX_CLIENT_VERSION}; \
                 left on {}. If {} is a new release it may need a newer client: raise \
                 CODEX_CLIENT_VERSION in src/adapters/responses/request.rs",
                change.alias, change.to, change.from, change.to
            )),
            Probe::Unknown(reason) => notes.push(format!(
                "{}: could not test {} ({reason}); left on {}, will retry on the next run",
                change.alias, change.to, change.from
            )),
        }
    }
    let mut moved = verified.clone();
    if !dry_run && !verified.is_empty() {
        let (text, applied) = apply_changes(&read()?, &verified)?;
        if !applied.is_empty() {
            crate::atomic_file::write_private_atomic(path, text.as_bytes())
                .with_context(|| format!("writing {}", path.display()))?;
        }
        for change in verified.iter().filter(|c| !applied.contains(c)) {
            notes.push(format!(
                "{}: changed while syncing; left as it is",
                change.alias
            ));
        }
        moved = applied;
    }
    let mut report = String::new();
    for change in &moved {
        report.push_str(&format!(
            "{}: {} -> {}\n",
            change.alias, change.from, change.to
        ));
    }
    for warning in found.warnings.iter().chain(&notes) {
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
    } else if dry_run && !moved.is_empty() {
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
        let (text, applied) = apply_changes(CONFIG, &result.changes).unwrap();
        assert_eq!(applied, result.changes);
        assert!(text.contains("upstream_model = \"gpt-6.2-sol\" # newest sol"));
        // Everything else is untouched, including a non-codex provider.
        assert!(text.contains("model = \"gpt-5.6-sol\"\nprovider = \"codex\"\n"));
        assert!(text.contains("provider = \"antigravity\"\nupstream_model = \"gpt-6.1-sol\""));
    }

    #[test]
    fn a_change_is_skipped_when_the_file_changed_since_it_was_found() {
        let changes = vec![Change {
            alias: "codex-sol".into(),
            from: "gpt-6-sol".into(),
            to: "gpt-6.2-sol".into(),
        }];
        let (text, applied) = apply_changes(CONFIG, &changes).unwrap();
        assert!(applied.is_empty());
        assert_eq!(text, CONFIG);
    }

    #[test]
    fn an_older_model_in_the_catalog_never_wins() {
        let models = ["gpt-6.1-sol", "gpt-5.6-sol", "gpt-6-astra"];
        let result = run(CONFIG, &models);
        assert!(result.changes.is_empty());
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

    fn works(_: &str) -> Probe {
        Probe::Works
    }

    #[test]
    fn dry_run_writes_nothing_and_a_real_run_writes_once() {
        let config = TempConfig::new("write", CONFIG);
        let path = &config.0;
        let models = ids(&["gpt-6.2-sol", "gpt-6-astra"]);

        let report = sync_file(path, &models, &models, true, &mut works).unwrap();
        assert!(report.contains("codex-sol: gpt-6.1-sol -> gpt-6.2-sol"));
        assert!(report.contains("Dry run: nothing written."));
        assert_eq!(std::fs::read_to_string(path).unwrap(), CONFIG);

        let report = sync_file(path, &models, &models, false, &mut works).unwrap();
        assert!(report.contains("codex-sol: gpt-6.1-sol -> gpt-6.2-sol"));
        assert!(std::fs::read_to_string(path)
            .unwrap()
            .contains("upstream_model = \"gpt-6.2-sol\" # newest sol"));

        let report = sync_file(path, &models, &models, false, &mut works).unwrap();
        assert!(report.contains("Codex aliases are up to date."));
    }

    #[test]
    fn a_model_codex_refuses_leaves_the_alias_and_says_what_to_raise() {
        let config = TempConfig::new("refused", CONFIG);
        let models = ids(&["gpt-6.2-sol", "gpt-6-astra"]);
        let report = sync_file(&config.0, &models, &models, false, &mut |_: &str| {
            Probe::Refused
        })
        .unwrap();
        assert!(!report.contains(" -> "));
        assert!(report.contains("Codex refused gpt-6.2-sol"));
        assert!(report.contains("left on gpt-6.1-sol"));
        assert!(report.contains("CODEX_CLIENT_VERSION"));
        assert_eq!(std::fs::read_to_string(&config.0).unwrap(), CONFIG);
    }

    #[test]
    fn a_model_that_cannot_be_tested_leaves_the_alias_and_retries_later() {
        let config = TempConfig::new("unknown", CONFIG);
        let models = ids(&["gpt-6.2-sol", "gpt-6-astra"]);
        let mut probe = |_: &str| Probe::Unknown("Codex quota is used up (HTTP 429)".into());
        let report = sync_file(&config.0, &models, &models, false, &mut probe).unwrap();
        assert!(report.contains("could not test gpt-6.2-sol (Codex quota is used up (HTTP 429))"));
        assert!(report.contains("will retry on the next run"));
        assert_eq!(std::fs::read_to_string(&config.0).unwrap(), CONFIG);
    }

    #[test]
    fn each_new_model_is_tested_once_and_only_working_ones_move() {
        let text = "[[routes]]\nmodel = \"a\"\nprovider = \"codex\"\nupstream_model = \"gpt-6-sol\"\n\n\
                    [[routes]]\nmodel = \"b\"\nprovider = \"codex\"\nupstream_model = \"gpt-6-sol\"\n\n\
                    [[routes]]\nmodel = \"c\"\nprovider = \"codex\"\nupstream_model = \"gpt-6-luna\"\n";
        let config = TempConfig::new("once", text);
        let models = ids(&["gpt-6.1-sol", "gpt-6.1-luna"]);
        let mut asked = Vec::new();
        let mut probe = |model: &str| {
            asked.push(model.to_string());
            if model == "gpt-6.1-sol" {
                Probe::Works
            } else {
                Probe::Refused
            }
        };
        let report = sync_file(&config.0, &models, &models, false, &mut probe).unwrap();
        assert_eq!(asked, vec!["gpt-6.1-sol", "gpt-6.1-luna"]);
        assert!(report.contains("a: gpt-6-sol -> gpt-6.1-sol"));
        assert!(report.contains("b: gpt-6-sol -> gpt-6.1-sol"));
        assert!(report.contains("Codex refused gpt-6.1-luna"));
        let written = std::fs::read_to_string(&config.0).unwrap();
        assert_eq!(written.matches("gpt-6.1-sol").count(), 2);
        assert!(written.contains("upstream_model = \"gpt-6-luna\""));
    }

    #[test]
    fn a_hand_edit_made_while_models_were_tested_is_kept() {
        let config = TempConfig::new("race", CONFIG);
        let path = config.0.clone();
        let models = ids(&["gpt-6.2-sol", "gpt-6-astra"]);
        let mut probe = |_: &str| {
            // The owner edits the file while the test request runs.
            let edited = std::fs::read_to_string(&path)
                .unwrap()
                .replace("gpt-6-astra", "gpt-6-astra # hand edit");
            std::fs::write(&path, edited).unwrap();
            Probe::Works
        };
        sync_file(&config.0, &models, &models, false, &mut probe).unwrap();
        let written = std::fs::read_to_string(&config.0).unwrap();
        assert!(written.contains("gpt-6-astra # hand edit"));
        assert!(written.contains("upstream_model = \"gpt-6.2-sol\" # newest sol"));
    }

    #[test]
    fn a_file_with_no_alias_reports_it() {
        let config = TempConfig::new("none", "[server]\nbind = \"127.0.0.1:3001\"\n");
        let models = ids(&["gpt-6.1-sol"]);
        let report = sync_file(&config.0, &models, &models, false, &mut works).unwrap();
        assert!(report.contains("found no codex alias"));
    }

    /// One-shot HTTP server on a free loopback port: answers the first request
    /// with `status` and `body`, and hands back the request text it received.
    fn serve(
        status: &'static str,
        body: &'static str,
    ) -> (String, std::thread::JoinHandle<String>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut chunk = [0u8; 1024];
            loop {
                let read = stream.read(&mut chunk).unwrap();
                request.extend_from_slice(&chunk[..read]);
                let text = String::from_utf8_lossy(&request).to_string();
                if let Some((head, rest)) = text.split_once("\r\n\r\n") {
                    let length = head
                        .lines()
                        .find_map(|l| {
                            l.to_lowercase()
                                .strip_prefix("content-length: ")
                                .map(String::from)
                        })
                        .and_then(|v| v.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    if rest.len() >= length {
                        break;
                    }
                }
            }
            let answer = format!(
                "HTTP/1.1 {status}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(answer.as_bytes());
            String::from_utf8_lossy(&request).to_string()
        });
        (addr, handle)
    }

    #[test]
    fn the_test_request_sends_the_pinned_identity_and_the_model() {
        let (addr, server) = serve("200 OK", "event: response.created\n");
        assert_eq!(
            probe_at(&addr, Some("secret-token"), "gpt-6.2-sol"),
            Probe::Works
        );
        let request = server.join().unwrap().to_lowercase();
        assert!(request.starts_with("post /v1/responses http/1.1"));
        assert!(request.contains("originator: codex_cli_rs"));
        assert!(request.contains(&format!("user-agent: {}", CODEX_USER_AGENT.to_lowercase())));
        assert!(request.contains(&format!("version: {CODEX_CLIENT_VERSION}")));
        assert!(request.contains("authorization: bearer secret-token"));
        assert!(request.contains("\"model\":\"gpt-6.2-sol\""));
        assert!(request.contains("\"store\":false"));
    }

    #[test]
    fn the_test_request_tells_refused_from_unknown() {
        let (addr, _s) = serve(
            "400 Bad Request",
            r#"{"error":{"code":"model_not_found","message":"The 'x' model is not supported"}}"#,
        );
        assert_eq!(probe_at(&addr, None, "x"), Probe::Refused);

        let (addr, _s) = serve(
            "429 Too Many Requests",
            r#"{"error":{"type":"usage_limit_reached"}}"#,
        );
        assert_eq!(
            probe_at(&addr, None, "x"),
            Probe::Unknown("Codex quota is used up (HTTP 429)".into())
        );

        // A 404 with no model message is a gateway without the Codex endpoint, not a refusal.
        let (addr, _s) = serve("404 Not Found", "no such route");
        assert!(matches!(probe_at(&addr, None, "x"), Probe::Unknown(r) if r.contains("404")));

        let (addr, _s) = serve("401 Unauthorized", "{}");
        assert!(matches!(probe_at(&addr, None, "x"), Probe::Unknown(r) if r.contains("401")));
    }

    #[test]
    fn a_gateway_that_is_down_is_unknown_not_a_refusal() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        drop(listener);
        assert!(
            matches!(probe_at(&addr, None, "x"), Probe::Unknown(r) if r.contains("cannot reach"))
        );
        assert!(matches!(
            probe_at("not an address", None, "x"),
            Probe::Unknown(_)
        ));
    }
}

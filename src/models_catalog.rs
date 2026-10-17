//! Live model lists for the CLI platforms the operator dispatches work to.
//!
//! Queried fresh on every call, never compiled in: `codex debug models`,
//! `opencode models`, and for `agy` the `[[models]]` the gateway serves as
//! `…-via-antigravity`. These shell out, so call them from `shunt tui` or
//! `shunt models` (a normal login shell), not from the launchd daemon whose
//! `PATH` is minimal.

use std::{
    io::Read,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use serde_json::Value;

pub const PLATFORMS: [&str; 3] = ["codex", "opencode", "agy"];
const TIMEOUT: Duration = Duration::from_secs(30);
const AGY_SUFFIX: &str = "-via-antigravity";

/// One platform's catalog with each model's resolved on/off state, or the
/// reason the live list could not be read.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PlatformModels {
    pub models: Vec<crate::models_prefs::Resolved>,
    pub error: Option<String>,
}

pub fn collect(
    platform: &str,
    prefs: &crate::models_prefs::ModelPrefs,
    config: Option<&Path>,
) -> PlatformModels {
    match fetch(platform, config) {
        Ok(ids) => PlatformModels {
            models: prefs.resolve(platform, &ids),
            error: None,
        },
        Err(error) => PlatformModels {
            models: Vec::new(),
            error: Some(error),
        },
    }
}

/// `shunt models`: the JSON contract (`--json`) read by the `goal` tool, or a
/// plain list. Every model is listed with `active`; a consumer hides the
/// inactive ones only while `filter` is true.
pub fn report(platform: Option<&str>, config: Option<&Path>, json: bool) -> Result<String, String> {
    let prefs = crate::models_prefs::load();
    let platforms: Vec<&str> = match platform {
        Some(p) if PLATFORMS.contains(&p) => vec![p],
        Some(p) => {
            return Err(format!(
                "unknown platform '{p}'; use one of {}",
                PLATFORMS.join(", ")
            ))
        }
        None => PLATFORMS.to_vec(),
    };
    let collected: Vec<_> = platforms
        .iter()
        .map(|p| (*p, collect(p, &prefs, config)))
        .collect();
    if json {
        let platforms: serde_json::Map<_, _> = collected
            .into_iter()
            .map(|(p, c)| (p.to_string(), serde_json::to_value(c).unwrap_or_default()))
            .collect();
        return Ok(format!(
            "{}\n",
            serde_json::json!({"filter": prefs.filter, "platforms": platforms})
        ));
    }
    let mut out = format!("filter: {}\n", if prefs.filter { "on" } else { "off" });
    for (p, c) in collected {
        out.push_str(&format!("{p}:\n"));
        if let Some(error) = c.error {
            out.push_str(&format!(
                "  unavailable: {}\n",
                error.replace(['\n', '\r'], " ")
            ));
        }
        for m in c.models {
            out.push_str(&format!(
                "  [{}] {}\n",
                if m.active { "on " } else { "off" },
                m.id
            ));
        }
    }
    Ok(out)
}

pub fn fetch(platform: &str, config: Option<&Path>) -> Result<Vec<String>, String> {
    match platform {
        "codex" => parse_codex(&run(&["codex", "debug", "models"])?),
        "opencode" => Ok(parse_opencode(&run(&["opencode", "models"])?)),
        "agy" => {
            let config = crate::config::Config::load(config).map_err(|e| e.to_string())?;
            Ok(agy_ids(config.models.iter().map(|m| m.id.as_str())))
        }
        other => Err(format!("unknown platform '{other}'")),
    }
}

/// Visible model slugs from `codex debug models`.
pub fn parse_codex(text: &str) -> Result<Vec<String>, String> {
    let raw: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let list = raw["models"].as_array().ok_or("models must be an array")?;
    Ok(list
        .iter()
        .filter(|m| m["visibility"] != "hide")
        .filter_map(|m| m["slug"].as_str().map(String::from))
        .collect())
}

/// `opencode/*` lines of `opencode models`; other lines are notices or other
/// providers.
pub fn parse_opencode(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| l.starts_with("opencode/"))
        .map(String::from)
        .collect()
}

/// Configured model ids routed through the pooled Antigravity backend.
pub fn agy_ids<'a>(ids: impl Iterator<Item = &'a str>) -> Vec<String> {
    ids.filter(|id| id.ends_with(AGY_SUFFIX))
        .map(String::from)
        .collect()
}

fn run(argv: &[&str]) -> Result<String, String> {
    let mut child = Command::new(argv[0])
        .args(&argv[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("cannot run {}: {e}", argv[0]))?;
    let mut stdout = child.stdout.take().ok_or("no stdout")?;
    // Drain on a thread so a full pipe cannot block the child while we poll.
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stdout.read_to_end(&mut bytes);
        bytes
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(status) => break status,
            None if start.elapsed() > TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("{} timed out", argv[0]));
            }
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    };
    let bytes = reader.join().map_err(|_| "reader thread panicked")?;
    if !status.success() {
        return Err(format!("{} exited with {status}", argv[0]));
    }
    String::from_utf8(bytes).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_keeps_visible_slugs() {
        let text = r#"{"models":[{"slug":"gpt-6.1-sol"},{"slug":"old","visibility":"hide"},{"slug":"gpt-6.1-luna","visibility":"list"}]}"#;
        assert_eq!(parse_codex(text).unwrap(), ["gpt-6.1-sol", "gpt-6.1-luna"]);
        assert!(parse_codex("{}").is_err());
        assert!(parse_codex("nope").is_err());
    }

    #[test]
    fn opencode_keeps_only_zen_lines() {
        let text = "notice\nopenai/gpt-6-sol\n opencode/big-pickle \nopencode-go/kimi\nopencode/beam-free\n";
        assert_eq!(
            parse_opencode(text),
            ["opencode/big-pickle", "opencode/beam-free"]
        );
    }

    #[test]
    fn agy_keeps_pooled_antigravity_ids() {
        let ids = [
            "claude-gemini-3.8-flash-low-via-antigravity",
            "claude-gpt-5.6-sol-via-codex",
        ];
        assert_eq!(
            agy_ids(ids.into_iter()),
            ["claude-gemini-3.8-flash-low-via-antigravity"]
        );
    }
}

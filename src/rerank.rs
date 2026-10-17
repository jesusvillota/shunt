//! Rerank the OpenCode free models with a headless agent.
//!
//! The operator presses `r` on the Models dialog's `opencode` tab. shunt writes
//! the live model list and the criteria into one prompt, runs a headless agent
//! (`claude -p` by default, with web search allowed), validates the answer
//! against the live list, and stores the result in `~/.shunt/models.json`.
//! Nothing here touches gateway routing.
//!
//! The criteria live in `~/.shunt/opencode-rank-criteria.md`, written once with
//! the default text when absent, so the owner can edit the ranking without a
//! code change. The ranker command runs through [`Runner`], so tests inject a
//! fake and never call a real model.

use std::{
    collections::BTreeMap,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant, SystemTime},
};

use crate::models_prefs::{rank_order, ModelScore, OpenCodeRank, Ranker};

/// Default run limit; a stuck agent is killed after this.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// The default Antigravity model the `agy` ranker runs `claude -p` against.
///
/// shortcut: pinned id, follow `opencode models`/`/v1/models` to pick a fresh
/// `…-via-antigravity` id when this one retires.
const AGY_MODEL: &str = "claude-gemini-3.8-flash-medium-via-antigravity";

/// The owner-editable scoring rubric. Only an untouched legacy default migrates.
pub const DEFAULT_CRITERIA: &str = "\
# OpenCode ranking criteria

Score each model from 0 to 10 on every parameter below.
Shunt computes the weighted mean and orders the models best first.

| parameter | weight |
| --- | --- |
| agentic coding | 50% |
| general intelligence | 20% |
| tool use | 20% |
| instruction following | 10% |

The free set changes often, so look up current benchmark data when you can.
Edit this file to change the ranking; weights need not add to 100.
";

const LEGACY_CRITERIA: &str = "\
# OpenCode ranking criteria

Judge each model on these parameters and order the models best first.
The weight is how much the parameter counts towards the position.

| parameter | weight |
| --- | --- |
| coding ability | 30% |
| tool use and agentic reliability | 25% |
| general intelligence | 20% |
| instruction following | 10% |
| speed | 10% |
| availability (how often the model is rate limited) | 5% |

The free set changes often, so look up current benchmark data when you can.
Edit this file to change the ranking; the OpenCode rerank reads it as it is.
";

/// Runs one ranker command and returns its stdout. The real one shells out and
/// enforces the timeout; tests inject a fake and never call a model.
pub trait Runner {
    fn run(&self, argv: &[String], timeout: Duration) -> Result<String, String>;
}

/// The real ranker: spawn the command, capture stdout, kill it after `timeout`.
pub struct ShellRunner;

impl Runner for ShellRunner {
    fn run(&self, argv: &[String], timeout: Duration) -> Result<String, String> {
        let (program, args) = argv.split_first().ok_or("empty ranker command")?;
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("cannot run {program}: {e}"))?;
        let mut stdout = child.stdout.take().ok_or("no stdout")?;
        let mut stderr = child.stderr.take().ok_or("no stderr")?;
        // Drain both pipes on threads so a full pipe cannot block the child.
        let out = std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = stdout.read_to_end(&mut bytes);
            bytes
        });
        let err = std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = stderr.read_to_end(&mut bytes);
            bytes
        });
        let start = Instant::now();
        let status = loop {
            match child.try_wait().map_err(|e| e.to_string())? {
                Some(status) => break status,
                None if start.elapsed() > timeout => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("{program} timed out after {}s", timeout.as_secs()));
                }
                None => std::thread::sleep(Duration::from_millis(100)),
            }
        };
        let out = out.join().map_err(|_| "reader thread panicked")?;
        let stderr = err.join().map_err(|_| "reader thread panicked")?;
        if !status.success() {
            return Err(format!(
                "{program} exited with {status}: {}",
                String::from_utf8_lossy(&stderr).trim()
            ));
        }
        String::from_utf8(out).map_err(|e| e.to_string())
    }
}

pub fn default_criteria_path() -> Option<PathBuf> {
    crate::auth::shared::home_dir()
        .map(|home| home.join(".shunt").join("opencode-rank-criteria.md"))
}

/// The criteria text: read the file, or write [`DEFAULT_CRITERIA`] and return it
/// when absent. Migrate only the byte-identical TASK-004 default.
pub fn criteria_text(path: &Path) -> Result<String, String> {
    match std::fs::read_to_string(path) {
        Ok(text) if text != LEGACY_CRITERIA => Ok(text),
        Ok(_) => {
            std::fs::write(path, DEFAULT_CRITERIA)
                .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
            Ok(DEFAULT_CRITERIA.to_string())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)
                    .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
            }
            std::fs::write(path, DEFAULT_CRITERIA)
                .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
            Ok(DEFAULT_CRITERIA.to_string())
        }
        Err(e) => Err(format!("cannot read {}: {e}", path.display())),
    }
}

/// The one prompt the agent reads: the criteria, the live model list, and the
/// exact JSON shape to answer with.
pub fn prompt(ids: &[String], criteria: &str) -> String {
    format!(
        "Score these OpenCode models from 0 to 10 on each criteria parameter.\n\
         Apply the criteria below and look up current benchmarks if that helps.\n\n\
         # Criteria\n\n{criteria}\n\n\
         # Models\n\n{}\n\n\
         Answer with JSON only, no prose and no code fence, in exactly this shape:\n\
         {{\"scores\": {{\"opencode/<id>\": {{\"<parameter>\": 7.5}}}}}}\n\
         Use the exact parameter names from the table and only the listed model ids.\n\
         Include every parameter for every model. Return scores only; shunt computes the order.",
        ids.join("\n")
    )
}

/// The ranker argv. Preserve TASK-004 arguments when no model or effort is set.
/// OpenCode has no effort flag in the installed CLI; its effort is ignored.
pub fn command(ranker: &Ranker, prompt: &str) -> Result<Vec<String>, String> {
    let mut argv: Vec<&str> = match ranker.platform() {
        // `--allowedTools` is variadic, so the prompt goes before it.
        "claude" => vec!["claude", "-p", prompt, "--allowedTools", "WebSearch"],
        "codex" => vec!["codex", "exec", "--skip-git-repo-check", prompt],
        "opencode" => vec!["opencode", "run", prompt],
        "agy" => vec!["claude", "-p", prompt, "--model", ranker.model().unwrap_or(AGY_MODEL)],
        "chatgptweb" => {
            return Err(
                "the 'chatgptweb' ranker needs a browser session shunt cannot start; use claude, codex, opencode or agy".into(),
            )
        }
        other => {
            return Err(format!(
                "unknown ranker '{other}'; use claude, codex, opencode or agy"
            ))
        }
    };
    if ranker.platform() != "agy" {
        if let Some(model) = ranker.model() {
            argv.extend([
                if ranker.platform() == "claude" {
                    "--model"
                } else {
                    "-m"
                },
                model,
            ]);
        }
    }
    let mut argv: Vec<String> = argv.into_iter().map(String::from).collect();
    if let Some(effort) = ranker.effort() {
        match ranker.platform() {
            "claude" | "agy" => argv.extend(["--effort".into(), effort.into()]),
            "codex" => argv.extend([
                "-c".into(),
                format!(
                    "model_reasoning_effort={}",
                    serde_json::to_string(effort).unwrap()
                ),
            ]),
            // `opencode run --help` has no effort/variant flag. Never invent one.
            _ => {}
        }
    }
    Ok(argv)
}

/// The first JSON object in `text`, so a stray code fence or a sentence around
/// the answer still parses. `None` when the text holds no `{…}`.
fn json_object(text: &str) -> Option<&str> {
    let text = text.trim();
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    (start < end).then(|| &text[start..=end])
}

/// Parse the parameter/weight Markdown table. Reject malformed weights rather
/// than silently changing the owner's rubric. Zero weights are allowed if the
/// sum is positive; negative and non-finite values are not meaningful weights.
pub fn weights(criteria: &str) -> Result<BTreeMap<String, f64>, String> {
    let mut found = false;
    let mut out = BTreeMap::new();
    for line in criteria.lines() {
        let cells: Vec<_> = line
            .trim()
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect();
        if cells == ["parameter", "weight"] {
            found = true;
            continue;
        }
        if !found {
            continue;
        }
        if cells.len() != 2 {
            break;
        }
        if cells
            .iter()
            .all(|c| !c.is_empty() && c.chars().all(|ch| "-: ".contains(ch)))
        {
            continue;
        }
        let weight = cells[1]
            .trim_end_matches('%')
            .trim()
            .parse::<f64>()
            .map_err(|_| format!("invalid weight for '{}': {}", cells[0], cells[1]))?;
        if cells[0].is_empty() || !weight.is_finite() || weight < 0.0 {
            return Err(format!("invalid parameter or weight: {line}"));
        }
        if out.insert(cells[0].to_string(), weight).is_some() {
            return Err(format!("duplicate parameter: {}", cells[0]));
        }
    }
    let sum: f64 = out.values().sum();
    if out.is_empty() || !sum.is_finite() || sum <= 0.0 {
        return Err("no usable weights (the sum must be positive)".into());
    }
    Ok(out)
}

/// Validate per-parameter scores, compute totals locally and sort. A stable
/// sort preserves previous order, then live order, on ties and for unscored ids.
pub fn validate(
    answer: &str,
    ids: &[String],
    previous: &[String],
    weights: &BTreeMap<String, f64>,
) -> Result<(Vec<String>, BTreeMap<String, ModelScore>), String> {
    let json = json_object(answer).ok_or("the ranker answer holds no JSON object")?;
    let value: serde_json::Value = serde_json::from_str(json)
        .map_err(|e| format!("the ranker answer is not valid JSON: {e}"))?;
    let scores = value
        .get("scores")
        .and_then(serde_json::Value::as_object)
        .ok_or("the ranker answer has no \"scores\" object")?;
    let mut out = BTreeMap::new();
    let sum: f64 = weights.values().sum();
    for id in ids {
        let parameters: Option<BTreeMap<String, f64>> = weights
            .keys()
            .map(|parameter| {
                let score = scores.get(id)?.get(parameter)?.as_f64()?;
                (score.is_finite() && (0.0..=10.0).contains(&score))
                    .then(|| (parameter.clone(), score))
            })
            .collect();
        if let Some(parameters) = parameters {
            let total = parameters.iter().map(|(p, s)| s * weights[p]).sum::<f64>() / sum;
            // Keep very large but valid weights from overflowing the numerator.
            let total = if total.is_finite() {
                total
            } else {
                parameters.iter().map(|(p, s)| s * (weights[p] / sum)).sum()
            };
            out.insert(id.clone(), ModelScore { parameters, total });
        }
    }
    let mut order = rank_order(previous, ids);
    order.sort_by(|a, b| match (out.get(a), out.get(b)) {
        (Some(a), Some(b)) => b.total.total_cmp(&a.total),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });
    Ok((order, out))
}

/// One full rerank: read (or create) the criteria, ask the agent, validate the
/// answer, and stamp it. `Err` leaves any previous ranking untouched.
pub fn run(
    criteria_path: &Path,
    ranker: &Ranker,
    ids: &[String],
    previous: &[String],
    runner: &dyn Runner,
    now: SystemTime,
) -> Result<OpenCodeRank, String> {
    let criteria = criteria_text(criteria_path)?;
    let weights = weights(&criteria).map_err(|e| {
        format!(
            "{}: {e}; expected a | parameter | weight | table with numeric weights",
            criteria_path.display()
        )
    })?;
    let argv = command(ranker, &prompt(ids, &criteria))?;
    let answer = runner.run(&argv, DEFAULT_TIMEOUT)?;
    let (order, scores) = validate(&answer, ids, previous, &weights)?;
    Ok(OpenCodeRank {
        ranked_at: crate::auth::shared::format_iso8601(now),
        order,
        scores,
    })
}

#[cfg(test)]
mod tests;

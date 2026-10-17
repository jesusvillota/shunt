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
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant, SystemTime},
};

use crate::models_prefs::OpenCodeRank;

/// Default run limit; a stuck agent is killed after this.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// The default Antigravity model the `agy` ranker runs `claude -p` against.
///
/// shortcut: pinned id, follow `opencode models`/`/v1/models` to pick a fresh
/// `…-via-antigravity` id when this one retires.
const AGY_MODEL: &str = "claude-gemini-3.8-flash-medium-via-antigravity";

/// The default criteria, from DEC-001: the parameters and weights the agent
/// applies. Written to the criteria file only when that file is absent.
pub const DEFAULT_CRITERIA: &str = "\
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
/// when the file is absent. An existing file is never overwritten.
pub fn criteria_text(path: &Path) -> Result<String, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(text),
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
        "Rank these OpenCode models for agentic coding work, best first.\n\
         Apply the criteria below and look up current benchmarks if that helps.\n\n\
         # Criteria\n\n{criteria}\n\n\
         # Models\n\n{}\n\n\
         Answer with JSON only, no prose and no code fence, in exactly this shape:\n\
         {{\"order\": [\"opencode/…\", …]}}\n\
         Use only the model ids listed above, each at most once.",
        ids.join("\n")
    )
}

/// The ranker argv for `platform`, with `prompt` as the last argument. `Err`
/// names why the platform cannot run a ranking.
pub fn command(platform: &str, prompt: &str) -> Result<Vec<String>, String> {
    let argv: Vec<&str> = match platform {
        "claude" => vec!["claude", "-p", "--allowedTools", "WebSearch", prompt],
        "codex" => vec!["codex", "exec", "--skip-git-repo-check", prompt],
        "opencode" => vec!["opencode", "run", prompt],
        "agy" => vec!["claude", "-p", "--model", AGY_MODEL, prompt],
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
    Ok(argv.into_iter().map(String::from).collect())
}

/// The first JSON object in `text`, so a stray code fence or a sentence around
/// the answer still parses. `None` when the text holds no `{…}`.
fn json_object(text: &str) -> Option<&str> {
    let text = text.trim();
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    (start < end).then(|| &text[start..=end])
}

/// Validate the agent's `{"order": [id, …]}` answer against the live list: keep
/// only live ids, drop duplicates, and append any live id the answer missed at
/// the end. `Err` when the answer is not valid JSON or has no `order` array.
pub fn validate(answer: &str, ids: &[String]) -> Result<Vec<String>, String> {
    let json = json_object(answer).ok_or("the ranker answer holds no JSON object")?;
    let value: serde_json::Value = serde_json::from_str(json)
        .map_err(|e| format!("the ranker answer is not valid JSON: {e}"))?;
    let order = value
        .get("order")
        .and_then(serde_json::Value::as_array)
        .ok_or("the ranker answer has no \"order\" array")?;
    let mut out: Vec<String> = Vec::with_capacity(ids.len());
    for id in order.iter().filter_map(serde_json::Value::as_str) {
        if ids.iter().any(|live| live == id) && !out.iter().any(|seen| seen == id) {
            out.push(id.to_string());
        }
    }
    for id in ids {
        if !out.contains(id) {
            out.push(id.clone());
        }
    }
    Ok(out)
}

/// One full rerank: read (or create) the criteria, ask the agent, validate the
/// answer, and stamp it. `Err` leaves any previous ranking untouched.
pub fn run(
    criteria_path: &Path,
    platform: &str,
    ids: &[String],
    runner: &dyn Runner,
    now: SystemTime,
) -> Result<OpenCodeRank, String> {
    let criteria = criteria_text(criteria_path)?;
    let argv = command(platform, &prompt(ids, &criteria))?;
    let answer = runner.run(&argv, DEFAULT_TIMEOUT)?;
    let order = validate(&answer, ids)?;
    Ok(OpenCodeRank {
        ranked_at: crate::auth::shared::format_iso8601(now),
        order,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    fn ids(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_string()).collect()
    }

    /// A fake ranker: returns canned stdout, or an error, and records the argv.
    struct Fake {
        answer: Result<String, String>,
        seen: RefCell<Vec<String>>,
    }

    impl Fake {
        fn answering(answer: &str) -> Self {
            Self {
                answer: Ok(answer.to_string()),
                seen: RefCell::new(Vec::new()),
            }
        }

        fn failing(error: &str) -> Self {
            Self {
                answer: Err(error.to_string()),
                seen: RefCell::new(Vec::new()),
            }
        }
    }

    impl Runner for Fake {
        fn run(&self, argv: &[String], _timeout: Duration) -> Result<String, String> {
            *self.seen.borrow_mut() = argv.to_vec();
            self.answer.clone()
        }
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("shunt-rerank-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_valid_answer_is_stored_in_its_order() {
        let all = ids(&["opencode/a", "opencode/b", "opencode/c"]);
        let fake = Fake::answering(r#"{"order": ["opencode/c", "opencode/b", "opencode/a"]}"#);
        let dir = temp_dir("order");
        let rank = run(
            &dir.join("criteria.md"),
            "claude",
            &all,
            &fake,
            SystemTime::UNIX_EPOCH + Duration::from_secs(1_792_246_537),
        )
        .unwrap();
        assert_eq!(rank.order, ids(&["opencode/c", "opencode/b", "opencode/a"]));
        assert_eq!(rank.ranked_at, "2026-10-17T14:15:37Z");
        assert!(fake.seen.borrow().last().unwrap().contains("opencode/a"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn unknown_ids_are_dropped_and_missing_ids_appended() {
        let all = ids(&["opencode/a", "opencode/b", "opencode/c"]);
        let fake = Fake::answering(
            r#"Here is the ranking:
```json
{"order": ["opencode/b", "opencode/gone", "opencode/b"]}
```"#,
        );
        let order = validate(&fake.answer.clone().unwrap(), &all).unwrap();
        assert_eq!(
            order,
            ids(&["opencode/b", "opencode/a", "opencode/c"]),
            "unknown drops, the duplicate collapses, missing ids append"
        );
    }

    #[test]
    fn an_invalid_answer_is_an_error() {
        let all = ids(&["opencode/a"]);
        assert!(validate("I could not decide", &all).is_err());
        assert!(validate("{\"ranking\": []}", &all).is_err());
        assert!(validate("{}", &all).is_err());
    }

    #[test]
    fn a_failing_or_timing_out_ranker_leaves_no_ranking() {
        let dir = temp_dir("fail");
        let path = dir.join("criteria.md");
        let fake = Fake::failing("claude timed out after 600s");
        let result = run(
            &path,
            "claude",
            &ids(&["opencode/a"]),
            &fake,
            SystemTime::now(),
        );
        assert!(result.unwrap_err().contains("timed out"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_default_criteria_file_is_written_once_and_not_overwritten() {
        let dir = temp_dir("criteria");
        let path = dir.join("opencode-rank-criteria.md");
        assert_eq!(criteria_text(&path).unwrap(), DEFAULT_CRITERIA);
        std::fs::write(&path, "my own weights").unwrap();
        assert_eq!(criteria_text(&path).unwrap(), "my own weights");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn chatgptweb_and_unknown_rankers_are_refused() {
        assert!(command("chatgptweb", "p")
            .unwrap_err()
            .contains("browser session"),);
        assert!(command("nope", "p").unwrap_err().contains("unknown ranker"));
        assert_eq!(command("claude", "p").unwrap()[0], "claude");
        assert_eq!(command("codex", "p").unwrap()[..2], ["codex", "exec"]);
        assert_eq!(command("opencode", "p").unwrap()[..2], ["opencode", "run"]);
        assert_eq!(
            command("agy", "p").unwrap()[..3],
            ["claude", "-p", "--model"]
        );
    }

    /// One-time live check: run the default ranker on the live OpenCode list
    /// and print the validated order. Needs `claude` and `opencode` on `PATH`.
    /// `cargo test --features tui --lib rerank -- --ignored --nocapture`.
    #[test]
    #[ignore = "runs the real claude ranker over the live opencode list"]
    fn live_rerank_prints_the_validated_order() {
        let path = default_criteria_path().expect("a home directory");
        let ids = crate::models_catalog::fetch("opencode", None).expect("live opencode models");
        eprintln!("criteria: {}\nmodels: {} live", path.display(), ids.len());
        let rank = run(&path, "claude", &ids, &ShellRunner, SystemTime::now()).unwrap();
        println!("ranked_at: {}", rank.ranked_at);
        for (at, id) in rank.order.iter().enumerate() {
            println!("  #{} {id}", at + 1);
        }
    }
}

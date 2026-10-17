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
fn legacy_strings_run_with_exact_task004_argv() {
    for (platform, expected) in [
        (
            "claude",
            vec!["claude", "-p", "PROMPT", "--allowedTools", "WebSearch"],
        ),
        (
            "codex",
            vec!["codex", "exec", "--skip-git-repo-check", "PROMPT"],
        ),
        ("opencode", vec!["opencode", "run", "PROMPT"]),
        ("agy", vec!["claude", "-p", "PROMPT", "--model", AGY_MODEL]),
    ] {
        let dir = temp_dir(&format!("legacy-{platform}"));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("models.json");
        std::fs::write(&path, format!(r#"{{"ranker":"{platform}"}}"#)).unwrap();
        let prefs = crate::models_prefs::load_from(&path);
        let fake = Fake::answering(r#"{"order":[]}"#);
        let all = ids(&["opencode/a"]);
        run(
            &dir.join("criteria.md"),
            &prefs.ranker,
            &all,
            &fake,
            SystemTime::now(),
        )
        .unwrap();
        let prompt = prompt(&all, DEFAULT_CRITERIA);
        let expected: Vec<String> = expected
            .into_iter()
            .map(|arg| {
                if arg == "PROMPT" {
                    prompt.clone()
                } else {
                    arg.into()
                }
            })
            .collect();
        assert_eq!(
            *fake.seen.borrow(),
            expected,
            "{platform}: TASK-004 argv unchanged"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn selected_models_and_efforts_reach_each_cli_opencode_ignores_effort() {
    for (platform, model, prefix, model_flag, effort_args) in [
        (
            "claude",
            "sonnet",
            vec!["claude", "-p", "PROMPT", "--allowedTools", "WebSearch"],
            "--model",
            vec!["--effort", "high"],
        ),
        (
            "codex",
            "gpt-6-sol",
            vec!["codex", "exec", "--skip-git-repo-check", "PROMPT"],
            "-m",
            vec!["-c", "model_reasoning_effort=\"high\""],
        ),
        (
            "opencode",
            "opencode/big-pickle",
            vec!["opencode", "run", "PROMPT"],
            "-m",
            vec![],
        ),
        (
            "agy",
            "claude-gemini-3.1-pro-high-via-antigravity",
            vec!["claude", "-p", "PROMPT"],
            "--model",
            vec!["--effort", "high"],
        ),
    ] {
        for (with_model, with_effort) in
            [(false, false), (true, false), (false, true), (true, true)]
        {
            let ranker: Ranker = serde_json::from_value(serde_json::json!({
                "platform": platform,
                "model": with_model.then_some(model),
                "effort": with_effort.then_some("high")
            }))
            .unwrap();
            let dir = temp_dir(&format!("selection-{platform}-{with_model}-{with_effort}"));
            let fake = Fake::answering(r#"{"order":[]}"#);
            let all = ids(&["opencode/a"]);
            run(
                &dir.join("criteria.md"),
                &ranker,
                &all,
                &fake,
                SystemTime::now(),
            )
            .unwrap();
            let prompt = prompt(&all, DEFAULT_CRITERIA);
            let mut expected: Vec<String> = prefix
                .iter()
                .map(|arg| {
                    if *arg == "PROMPT" {
                        prompt.clone()
                    } else {
                        (*arg).into()
                    }
                })
                .collect();
            if with_model || platform == "agy" {
                expected.extend([
                    model_flag.into(),
                    if with_model { model } else { AGY_MODEL }.into(),
                ]);
            }
            if with_effort {
                expected.extend(effort_args.iter().map(|arg| (*arg).into()));
            }
            assert_eq!(
                *fake.seen.borrow(),
                expected,
                "{platform}: model={with_model}, effort={with_effort}"
            );
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}

#[test]
fn chatgptweb_object_is_refused_before_the_runner_runs() {
    let ranker = Ranker::Selection {
        platform: "chatgptweb".into(),
        model: Some("any".into()),
        effort: Some("high".into()),
    };
    let fake = Fake::answering("unused");
    let dir = temp_dir("web-object");
    let error = run(
        &dir.join("criteria.md"),
        &ranker,
        &[],
        &fake,
        SystemTime::now(),
    )
    .unwrap_err();
    assert_eq!(error, "the 'chatgptweb' ranker needs a browser session shunt cannot start; use claude, codex, opencode or agy");
    assert!(fake.seen.borrow().is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_valid_answer_is_stored_in_its_order() {
    let all = ids(&["opencode/a", "opencode/b", "opencode/c"]);
    let fake = Fake::answering(r#"{"order": ["opencode/c", "opencode/b", "opencode/a"]}"#);
    let dir = temp_dir("order");
    let rank = run(
        &dir.join("criteria.md"),
        &"claude".into(),
        &all,
        &fake,
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_792_246_537),
    )
    .unwrap();
    assert_eq!(rank.order, ids(&["opencode/c", "opencode/b", "opencode/a"]));
    assert_eq!(rank.ranked_at, "2026-10-17T14:15:37Z");
    assert!(
        fake.seen
            .borrow()
            .iter()
            .any(|arg| arg.contains("opencode/a")),
        "the prompt names the live models"
    );
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
        &"claude".into(),
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
    assert!(command(&"chatgptweb".into(), "p")
        .unwrap_err()
        .contains("browser session"),);
    assert!(command(&"nope".into(), "p")
        .unwrap_err()
        .contains("unknown ranker"));
    assert_eq!(command(&"claude".into(), "p").unwrap()[0], "claude");
    assert_eq!(
        command(&"codex".into(), "p").unwrap()[..2],
        ["codex", "exec"]
    );
    assert_eq!(
        command(&"opencode".into(), "p").unwrap()[..2],
        ["opencode", "run"]
    );
    assert_eq!(
        command(&"agy".into(), "p").unwrap()[..3],
        ["claude", "-p", "p"]
    );
}

/// One-time live check: run the default ranker over the live OpenCode list,
/// print the validated order, and mirror the TUI by storing it in
/// `~/.shunt/models.json` so `shunt models opencode --json` reports `rank`.
/// Needs `claude` and `opencode` on `PATH`.
/// `cargo test --features tui --lib rerank -- --ignored --nocapture`.
#[test]
#[ignore = "runs the real claude ranker over the live opencode list and stores the result"]
fn live_rerank_prints_the_validated_order() {
    let path = default_criteria_path().expect("a home directory");
    let ids = crate::models_catalog::fetch("opencode", None).expect("live opencode models");
    eprintln!("criteria: {}\nmodels: {} live", path.display(), ids.len());
    let rank = run(
        &path,
        &"claude".into(),
        &ids,
        &ShellRunner,
        SystemTime::now(),
    )
    .unwrap();
    let mut prefs = crate::models_prefs::load();
    prefs.opencode_rank = Some(rank.clone());
    crate::models_prefs::save(&prefs).unwrap();
    println!("ranked_at: {}", rank.ranked_at);
    for (at, id) in rank.order.iter().enumerate() {
        println!("  #{} {id}", at + 1);
    }
}

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
        let fake = Fake::answering(r#"{"scores":{}}"#);
        let all = ids(&["opencode/a"]);
        run(
            &dir.join("criteria.md"),
            &prefs.ranker,
            &all,
            &[],
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
            let fake = Fake::answering(r#"{"scores":{}}"#);
            let all = ids(&["opencode/a"]);
            run(
                &dir.join("criteria.md"),
                &ranker,
                &all,
                &[],
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
fn a_valid_answer_is_stored_in_weighted_order() {
    let all = ids(&["opencode/a", "opencode/b", "opencode/c"]);
    let fake = Fake::answering(&score_answer(&[
        ("opencode/a", [5., 5., 5., 5.]),
        ("opencode/b", [7., 7., 7., 7.]),
        ("opencode/c", [9., 9., 9., 9.]),
    ]));
    let dir = temp_dir("order");
    let rank = run(
        &dir.join("criteria.md"),
        &"claude".into(),
        &all,
        &[],
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
{"scores": {"opencode/b": {"agentic coding": 8, "general intelligence": 8, "tool use": 8, "instruction following": 8}, "opencode/gone": {"agentic coding": 10}}}
```"#,
    );
    let (order, scores) = validate(
        &fake.answer.clone().unwrap(),
        &all,
        &[],
        &weights(DEFAULT_CRITERIA).unwrap(),
    )
    .unwrap();
    assert_eq!(scores.len(), 1);
    assert!(!scores.contains_key("opencode/gone"));
    assert_eq!(
        order,
        ids(&["opencode/b", "opencode/a", "opencode/c"]),
        "unknown scores drop and missing ids append"
    );
}

#[test]
fn an_invalid_answer_is_an_error() {
    let all = ids(&["opencode/a"]);
    assert!(validate(
        "I could not decide",
        &all,
        &[],
        &weights(DEFAULT_CRITERIA).unwrap()
    )
    .is_err());
    assert!(validate(
        "{\"ranking\": []}",
        &all,
        &[],
        &weights(DEFAULT_CRITERIA).unwrap()
    )
    .is_err());
    assert!(validate("{}", &all, &[], &weights(DEFAULT_CRITERIA).unwrap()).is_err());
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
        &[],
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
/// Needs the configured ranker and `opencode` on `PATH`.
/// `cargo test --features tui --lib rerank -- --ignored --nocapture`.
#[test]
#[ignore = "runs the configured ranker over the live opencode list and stores scores"]
fn live_rerank_prints_the_validated_order() {
    let mut prefs = crate::models_prefs::load();
    let path = default_criteria_path().expect("a home directory");
    let ids = crate::models_catalog::fetch("opencode", None).expect("live opencode models");
    eprintln!("criteria: {}\nmodels: {} live", path.display(), ids.len());
    let rank = run(
        &path,
        &prefs.ranker,
        &ids,
        &prefs
            .opencode_rank
            .as_ref()
            .map(|r| r.order.clone())
            .unwrap_or_default(),
        &ShellRunner,
        SystemTime::now(),
    )
    .unwrap();
    prefs.opencode_rank = Some(rank.clone());
    crate::models_prefs::save(&prefs).unwrap();
    println!("{}", serde_json::to_string_pretty(&rank).unwrap());
    println!("ranked_at: {}", rank.ranked_at);
    for (at, id) in rank.order.iter().enumerate() {
        println!("  #{} {id}", at + 1);
    }
}

fn score_answer(models: &[(&str, [f64; 4])]) -> String {
    let parameters = [
        "agentic coding",
        "general intelligence",
        "tool use",
        "instruction following",
    ];
    let scores: BTreeMap<_, BTreeMap<_, _>> = models
        .iter()
        .map(|(id, values)| (*id, parameters.into_iter().zip(*values).collect()))
        .collect();
    serde_json::json!({"scores": scores}).to_string()
}

#[test]
fn weighted_scores_persist_and_resolve_to_json_rank_and_score() {
    let dir = temp_dir("weighted");
    let all = ids(&["opencode/a", "opencode/b", "opencode/c"]);
    let fake = Fake::answering(&score_answer(&[
        ("opencode/a", [10., 2., 2., 2.]), // 6
        ("opencode/b", [6., 9., 9., 9.]),  // 7.5
        ("opencode/c", [8., 5., 5., 5.]),  // 6.5
    ]));
    let rank = run(
        &dir.join("criteria.md"),
        &"claude".into(),
        &all,
        &[],
        &fake,
        SystemTime::now(),
    )
    .unwrap();
    assert_eq!(rank.order, ids(&["opencode/b", "opencode/c", "opencode/a"]));
    let prefs = crate::models_prefs::ModelPrefs {
        opencode_rank: Some(rank),
        ..Default::default()
    };
    let path = dir.join("models.json");
    crate::models_prefs::save_to(&path, &prefs).unwrap();
    let loaded = crate::models_prefs::load_from(&path);
    assert_eq!(loaded, prefs);
    let rows = serde_json::to_value(loaded.resolve("opencode", &all)).unwrap();
    for (at, total) in [7.5, 6.5, 6.].into_iter().enumerate() {
        assert_eq!(rows[at]["rank"], at + 1);
        assert_eq!(rows[at]["score"], total);
    }
    let prompt = fake.seen.borrow().join(" ");
    assert!(prompt.contains("\"scores\"") && !prompt.contains("\"order\""));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn changing_one_weight_changes_the_next_order_with_identical_scores() {
    let dir = temp_dir("change-weight");
    let path = dir.join("criteria.md");
    let all = ids(&["opencode/a", "opencode/b"]);
    let fake = Fake::answering(&score_answer(&[
        ("opencode/a", [10., 2., 2., 2.]),
        ("opencode/b", [6., 9., 9., 9.]),
    ]));
    let first = run(&path, &"claude".into(), &all, &[], &fake, SystemTime::now()).unwrap();
    assert_eq!(first.order, ids(&["opencode/b", "opencode/a"]));
    std::fs::write(&path, DEFAULT_CRITERIA.replace("50%", "200%")).unwrap();
    let second = run(
        &path,
        &"claude".into(),
        &all,
        &first.order,
        &fake,
        SystemTime::now(),
    )
    .unwrap();
    assert_eq!(second.order, ids(&["opencode/a", "opencode/b"]));
    assert!((second.scores["opencode/a"].total - 8.4).abs() < 1e-12);
    // Scaling all weights cannot change a normalized total or order.
    std::fs::write(
        &path,
        DEFAULT_CRITERIA
            .replace("50%", "5")
            .replace("20%", "2")
            .replace("10%", "1"),
    )
    .unwrap();
    let scaled = run(&path, &"claude".into(), &all, &[], &fake, SystemTime::now()).unwrap();
    assert_eq!(scaled.order, first.order);
    assert_eq!(scaled.scores, first.scores);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn ties_keep_previous_order_then_live_order() {
    let all = ids(&["a", "b", "c", "d"]);
    let answer = score_answer(&[
        ("a", [7.; 4]),
        ("b", [7.; 4]),
        ("c", [7.; 4]),
        ("d", [7.; 4]),
    ]);
    let (order, _) = validate(
        &answer,
        &all,
        &ids(&["gone", "c", "b", "c"]),
        &weights(DEFAULT_CRITERIA).unwrap(),
    )
    .unwrap();
    assert_eq!(order, ids(&["c", "b", "a", "d"]));
}

#[test]
fn missing_nonnumeric_and_out_of_range_scores_are_unscored_and_last() {
    let all = ids(&["missing", "high", "low", "text", "partial", "ok"]);
    let mut value: serde_json::Value = serde_json::from_str(&score_answer(&[
        ("ok", [0., 10., 5., 7.5]),
        ("high", [10.1, 5., 5., 5.]),
        ("low", [-0.1, 5., 5., 5.]),
        ("text", [5.; 4]),
        ("partial", [5.; 4]),
    ]))
    .unwrap();
    value["scores"]["text"]["tool use"] = serde_json::json!("7");
    value["scores"]["partial"]
        .as_object_mut()
        .unwrap()
        .remove("tool use");
    let (order, scores) = validate(
        &value.to_string(),
        &all,
        &[],
        &weights(DEFAULT_CRITERIA).unwrap(),
    )
    .unwrap();
    assert_eq!(
        order,
        ids(&["ok", "missing", "high", "low", "text", "partial"])
    );
    assert_eq!(scores.len(), 1);
    assert_eq!(order.len() - scores.len(), 5);
    assert!(scores.contains_key("ok"));
}

#[test]
fn untouched_legacy_default_migrates_once_and_edited_legacy_is_preserved() {
    let dir = temp_dir("migration");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("criteria.md");
    std::fs::write(&path, LEGACY_CRITERIA).unwrap();
    assert_eq!(criteria_text(&path).unwrap(), DEFAULT_CRITERIA);
    let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
    assert_eq!(criteria_text(&path).unwrap(), DEFAULT_CRITERIA);
    assert_eq!(
        std::fs::metadata(&path).unwrap().modified().unwrap(),
        modified
    );
    let edited = format!("{LEGACY_CRITERIA}\nMy addition\n");
    std::fs::write(&path, &edited).unwrap();
    assert_eq!(criteria_text(&path).unwrap(), edited);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), edited);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn invalid_weights_fail_before_calling_the_ranker() {
    let dir = temp_dir("weights-error");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("criteria.md");
    for text in [
        "my own rubric".to_string(),
        "| parameter | weight |\n| --- | --- |".to_string(),
        DEFAULT_CRITERIA.replace("50%", "many"),
        DEFAULT_CRITERIA.replace("50%", "NaN"),
        DEFAULT_CRITERIA.replace("50%", "inf"),
        DEFAULT_CRITERIA.replace("50%", "-5%"),
        DEFAULT_CRITERIA
            .replace("50%", "0")
            .replace("20%", "0")
            .replace("10%", "0"),
    ]
    .into_iter()
    {
        std::fs::write(&path, &text).unwrap();
        let fake = Fake::answering("unused");
        let error = run(&path, &"claude".into(), &[], &[], &fake, SystemTime::now()).unwrap_err();
        assert!(
            error.contains(path.to_str().unwrap()) && error.contains("| parameter | weight |"),
            "{error}"
        );
        assert!(fake.seen.borrow().is_empty());
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[cfg(feature = "tui")]
#[test]
fn rerank_failures_keep_old_scores_and_file_and_show_the_error() {
    use crate::models_prefs::{load_from, save_to, ModelPrefs};
    let dir = temp_dir("keep-old");
    let path = dir.join("criteria.md");
    let all = ids(&["opencode/a"]);
    let rank = run(
        &path,
        &"claude".into(),
        &all,
        &[],
        &Fake::answering(&score_answer(&[("opencode/a", [8.; 4])])),
        SystemTime::now(),
    )
    .unwrap();
    let prefs = ModelPrefs {
        opencode_rank: Some(rank),
        ..Default::default()
    };
    let stored = dir.join("models.json");
    save_to(&stored, &prefs).unwrap();
    let bytes = std::fs::read(&stored).unwrap();
    for (answer, criteria, expected) in [
        (Ok("{not json}"), DEFAULT_CRITERIA, "not valid JSON"),
        (
            Err("claude timed out after 600s"),
            DEFAULT_CRITERIA,
            "timed out",
        ),
        (
            Ok("unused"),
            "my edited criteria",
            "expected a | parameter | weight | table",
        ),
    ] {
        std::fs::write(&path, criteria).unwrap();
        let fake = Fake {
            answer: answer.map(String::from).map_err(String::from),
            seen: RefCell::new(vec![]),
        };
        let current = load_from(&stored);
        let result = run(
            &path,
            &prefs.ranker,
            &all,
            &prefs.opencode_rank.as_ref().unwrap().order,
            &fake,
            SystemTime::now(),
        );
        assert!(result.unwrap_err().contains(expected));
        assert_eq!(current, prefs);
        assert_eq!(std::fs::read(&stored).unwrap(), bytes);
    }
    std::fs::remove_dir_all(dir).unwrap();
}

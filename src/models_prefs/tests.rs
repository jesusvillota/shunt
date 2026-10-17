use super::*;

#[test]
fn ranker_objects_round_trip_and_unselected_strings_stay_strings() {
    for ranker in [
        serde_json::json!("claude"),
        serde_json::json!({"platform":"codex", "model":"gpt-6-sol", "effort":"high"}),
        serde_json::json!({"platform":"agy"}),
    ] {
        let mut prefs: ModelPrefs =
            serde_json::from_value(serde_json::json!({"ranker":ranker})).unwrap();
        prefs.filter = false; // An unrelated edit must preserve the string form.
        let json = serde_json::to_value(&prefs).unwrap();
        assert_eq!(json["ranker"], ranker);
        assert_eq!(serde_json::from_value::<ModelPrefs>(json).unwrap(), prefs);
    }
}

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
    assert_eq!(prefs.ranker, Ranker::from("claude"));
    prefs.opencode_rank = Some(OpenCodeRank {
        ranked_at: "2026-10-17T14:15:37Z".into(),
        order: ids(&["c", "a"]),
        scores: Default::default(),
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
            scores: Default::default(),
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

#[test]
fn old_ranking_without_scores_still_loads() {
    let prefs: ModelPrefs = serde_json::from_str(
        r#"{"opencode_rank":{"ranked_at":"2026-10-17T14:15:37Z","order":["opencode/a"]}}"#,
    )
    .unwrap();
    let rank = prefs.opencode_rank.as_ref().unwrap();
    assert_eq!(rank.order, ["opencode/a"]);
    assert!(rank.scores.is_empty());
    let rows = prefs.resolve("opencode", &ids(&["opencode/a"]));
    assert_eq!(rows[0].rank, Some(1));
    assert_eq!(rows[0].score, None);
}

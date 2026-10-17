## Acceptance

- [x] **1. Weighted order and JSON fields.** `rerank::tests::weighted_scores_persist_and_resolve_to_json_rank_and_score` uses fake scores with 50/20/20/10 weights. It stores and reloads order `b, c, a`, with totals `7.5, 6.5, 6.0`, and checks every JSON `rank` and `score`. `ties_keep_previous_order_then_live_order` checks stable ties. The final live CLI report includes both fields for all ten models.
- [x] **2. Weights control the order.** `changing_one_weight_changes_the_next_order_with_identical_scores` gets `b, a` at default weights, edits only agentic coding from 50 to 200, and gets `a, b` with the same fake answer. Scaling all weights to 5/2/2/1 preserves the default totals and order.
- [x] **3. Unscored models follow scored models.** `missing_nonnumeric_and_out_of_range_scores_are_unscored_and_last` covers absent models, missing parameters, strings and scores below 0 or above 10. Five unscored models follow the valid model. `opencode_section_shows_rank_numbers_in_stored_order_and_ranked_at` checks the rendered total and unscored count. Unknown ids are excluded.
- [x] **4. Safe default migration.** `untouched_legacy_default_migrates_once_and_edited_legacy_is_preserved` checks exact old-default replacement, no second write, and preservation of an edited old file. The existing create-once test also passes. `old_ranking_without_scores_still_loads` verifies old preferences.
- [x] **5. Failures preserve ranking and show errors.** `rerank_failures_keep_old_scores_and_file_and_show_the_error` checks invalid JSON, fake timeout and unparseable weights against stored scores and unchanged file bytes. `invalid_weights_fail_before_calling_the_ranker` checks bad/non-finite/negative/zero weights and empty tables, with the criteria path and expected table in each error. TUI tests keep the old preferences and display the error.
- [x] **6. Exactly one live rerank.** The ignored live test ran once with the configured `"ranker": "claude"` (CLI default model and effort), all ten live OpenCode ids, and the real `ShellRunner`. It passed in 9.78 seconds at `2026-10-17T18:45:01Z`, migrated the untouched local criteria file, and saved scores and order. The table below pastes all parameter scores and the stored order. Totals are rounded to two decimals. An independent local calculation matched stored totals within `1e-12` and confirmed descending order.
- [x] **7. Four-language docs and required checks.** All four READMEs and both affected site pages with their ko/ja/zh-cn copies are updated, plus `docs/pool-tui.md`. All five Verify commands exited 0 together. Clippy passed with `--all-targets --features tui,bench -- -D warnings` and `RUSTFLAGS="-D warnings"`. The full workspace passed 3,831 tests (5 existing ignored tests). The focused rerank run passed 21 tests (1 live test ignored in that ordinary run). The live test ran separately exactly once and passed. Format passed again after the rebase. The final JSON report returned ten scored models with `error: null`.

## Summary

Shunt requests parameter scores instead of an agent-supplied order. It parses the criteria table and computes the weighted mean locally. It preserves prior order for ties, then catalog order. Invalid model scores append as unscored. Stored scores include each parameter and total; legacy rankings still load. The TUI shows totals and an unscored count, and JSON includes `score` beside `rank`.

Plan: implement local weighted scoring, preserve preferences compatibility, test weights and failure cases, update translations, run one configured live rerank, and verify the required commands. No material decision or scope change was needed. The rubric change is explicitly authorized by TASK-006. DEC-001, task specifications, other outcomes and goal.yaml were not edited.

Documentation surfaces considered: four READMEs, English `docs/pool-tui.md`, and the CLI reference and pool-account controls guide in English, Korean, Japanese and Simplified Chinese. Generated `wiki/` is reserved for separate regeneration. Site dependencies are absent, so site rendering was not run.

Verified revision: `149997ca` (implementation), tested in this session. The outcome is a documentation-only follow-up. The branch was rebased onto `origin/main` without conflicts and pushed. No merge was performed. No follow-up is required for this task.

## Live scores and order

| Rank | Model | Agentic coding | Intelligence | Tool use | Instruction following | Total |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | `opencode/big-pickle` | 7.5 | 7.0 | 7.5 | 7.5 | 7.40 |
| 2 | `opencode/deepseek-v4.1-flash-free` | 7.0 | 7.0 | 7.0 | 7.0 | 7.00 |
| 3 | `opencode/glm-5.3-flash-free` | 6.5 | 6.5 | 7.0 | 7.0 | 6.65 |
| 4 | `opencode/nemotron-3-ultra-free` | 6.5 | 7.0 | 6.5 | 7.0 | 6.65 |
| 5 | `opencode/mimo-v2.6-flash-free` | 6.5 | 6.5 | 6.5 | 6.5 | 6.50 |
| 6 | `opencode/longcat-2.5-preview-free` | 6.5 | 6.5 | 6.5 | 6.5 | 6.50 |
| 7 | `opencode/beam-free` | 6.0 | 6.0 | 6.0 | 6.5 | 6.05 |
| 8 | `opencode/space-bunny-free` | 6.0 | 6.0 | 6.0 | 6.0 | 6.00 |
| 9 | `opencode/nemotron-3.5-super-vl-free` | 5.5 | 6.0 | 6.0 | 6.5 | 5.80 |
| 10 | `opencode/nemotron-3.5-lightning-free` | 5.0 | 5.5 | 5.5 | 6.0 | 5.30 |

## Verification

```sh
cargo fmt --all --check
RUSTFLAGS="-D warnings" cargo clippy --all-targets --features tui,bench -- -D warnings
RUSTFLAGS="-D warnings" cargo test --features tui,bench --workspace
cargo test --features tui --lib rerank
cargo run -q -- models opencode --json
```

The workspace test command uses `RUST_TEST_THREADS=1` to avoid the existing token-estimator timing issue recorded by TASK-005. No tests were weakened. Default-feature commands still emit the existing `MAX_TRACKED_CHILD_PINS` unused-import warning; the required warnings-as-errors build with `tui,bench` passed.

The local sequence exited 0. Its tail was:

```text
test result: ok. 21 passed; 0 failed; 1 ignored; 0 measured; 3113 filtered out; finished in 0.07s

{"filter":true,"platforms":{"opencode":{"models":[{"id":"opencode/big-pickle","active":true,"reason":"default","rank":1,"score":7.4},{"id":"opencode/deepseek-v4.1-flash-free","active":true,"reason":"default","rank":2,"score":7.000000000000001},{"id":"opencode/glm-5.3-flash-free","active":true,"reason":"default","rank":3,"score":6.65},{"id":"opencode/nemotron-3-ultra-free","active":true,"reason":"default","rank":4,"score":6.65},{"id":"opencode/mimo-v2.6-flash-free","active":true,"reason":"default","rank":5,"score":6.5},{"id":"opencode/longcat-2.5-preview-free","active":true,"reason":"default","rank":6,"score":6.5},{"id":"opencode/beam-free","active":true,"reason":"default","rank":7,"score":6.050000000000001},{"id":"opencode/space-bunny-free","active":true,"reason":"default","rank":8,"score":6.000000000000001},{"id":"opencode/nemotron-3.5-super-vl-free","active":true,"reason":"default","rank":9,"score":5.800000000000001},{"id":"opencode/nemotron-3.5-lightning-free","active":true,"reason":"default","rank":10,"score":5.300000000000001}],"error":null}}}

TASK-006 VERIFY: all five commands exited 0 (workspace RUST_TEST_THREADS=1)
```

Workspace summary: 3,831 passed, 0 failed, 5 ignored. Focused rerank summary: 21 passed, 0 failed, 1 ignored. The ignored live test was explicitly run once and passed.

PR: https://github.com/jesusvillota/shunt/pull/28, targeting `main`, labeled `goal:model-filter`. GitHub reported `statusCheckRollup: []`; no remote checks are configured. The outcome and implementation are pushed on `goal/model-filter/task-006`.


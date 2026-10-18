## Acceptance

- [x] **1. Validated order is stored.** `rerank::tests::a_valid_order_is_stored_after_validation_and_prompt_uses_plain_criteria` supplies `{"order":["opencode/c","opencode/gone","opencode/b","opencode/c"]}` with live ids `a,b,c,d`. It stores and reloads `c,b,a,d`: unknown ids drop, duplicates collapse, and missing ids append in catalog order. `unknown_ids_are_dropped_and_missing_ids_appended` also covers fenced answers and an empty order.
- [x] **2. Captured prompt asks for order.** The same fake-Runner test captures the actual argv prompt. It checks the exact edited criteria text, every live id, `"order"`, and absence of `score`. Criteria have no required format. Existing exact-argv tests still cover legacy ranker strings and selected platform/model/effort combinations.
- [x] **3. Failures preserve ranking and show errors.** `rerank_failures_keep_old_ranking_and_file_and_show_the_error` checks invalid JSON, fake timeout, and missing `order` against unchanged stored preferences and file bytes. `tui::models::tests::a_failed_rerank_keeps_the_old_ranking_and_shows_the_error` sends those actual fake-Runner failures through the dialog and checks unchanged preferences and the displayed error state. The existing rendered-dialog error test also passes.
- [x] **4. JSON and old-file compatibility.** The real `cargo run -q -- models opencode --json` exited 0 and returned ten live rows with `rank`, no `score`, and `error: null`. A second direct run exited 0, and a JSON assertion checked all ten rows. `models_prefs::tests::old_ranking_with_scores_loads_and_drops_them_on_save` loads an old file containing parameter scores, preserves order and timestamp, saves it, and checks that only `ranked_at` and `order` remain in `opencode_rank`. Existing old files without scores still load.
- [x] **5. Safe one-time migration.** `untouched_legacy_default_migrates_once_and_edited_legacy_is_preserved` checks both TASK-004 and TASK-006 exact defaults. Each is replaced by the plain text, and a second read preserves the modification time. Edited versions of both defaults remain byte-identical. The create-once test also passes.
- [x] **6. Scoring implementation removed.** `grep -rniE 'weighted|score' src/rerank.rs src/models_prefs.rs` returns exactly `src/rerank.rs:46:Score each model from 0 to 10 on every parameter below.` and `src/rerank.rs:47:Shunt computes the weighted mean and orders the models best first.` Both are literal historical text in `TASK006_CRITERIA`, retained solely for byte-identical migration. No parser, score type/field, weighted calculation, score tie-break, unscored-model handling, or dialog score column remains. `src/models_prefs.rs` has no hits. Compatibility tests use historical keys and assert that removed fields are absent.
- [x] **7. Four-language docs and checks.** Updated English, Korean, Japanese, and Simplified Chinese READMEs, plus the CLI reference and pool-account controls site pages in all four languages, and English `docs/pool-tui.md`. All five Verify commands passed together. Format and strict clippy passed. The full workspace suite passed 3,827 tests, with zero failures and five ignored tests. Focused rerank tests passed 16 tests, with zero failures and one ignored live-ranker test. GitHub reports `statusCheckRollup: []`: no PR checks are configured.

## Summary

The agent now returns one best-first order. Shunt validates it against the live catalog and stores `{ranked_at, order}`. Criteria are plain text. Only untouched historical defaults migrate. Edited criteria pass to the agent unchanged. Storage and CLI JSON no longer emit scores, and the dialog shows the rank and timestamp without a score column. The TASK-005 ranker picker and CLI arguments remain unchanged.

This task supersedes the scoring behavior of TASK-006. DEC-001 is unchanged. No task specification, other task outcome, decision, or goal.yaml was edited. No material decision or scope deviation was needed.

Plan summary: remove scoring paths, restore order validation, preserve old-file loading, migrate untouched defaults, add focused fake-Runner coverage, update all affected documentation languages, and run the required checks. The implementation removes more code than it adds.

Verified revision: `328aeb1d5f1cc0ad588311e1f4881cd88baf7bca`. This outcome is a documentation-only follow-up. The branch rebased onto `origin/main` without conflicts. No merge was performed. No follow-up is required.

Documentation surfaces considered: the four root READMEs, English `docs/`, and both affected site pages with ko/ja/zh-cn copies. Generated `wiki/` was not hand-edited. Site dependencies are absent, so a rendered site build was not checked. The five requested commands cover the delivered Rust behavior. No real ranker was needed; the live-ranker test remains ignored.

## Verification

```sh
cargo fmt --all --check
RUSTFLAGS="-D warnings" cargo clippy --all-targets --features tui,bench -- -D warnings
RUSTFLAGS="-D warnings" cargo test --features tui,bench --workspace
cargo test --features tui --lib rerank
cargo run -q -- models opencode --json
```

The sequence exited 0 with `RUST_TEST_THREADS=1`, following TASK-006's recorded timing-sensitive test workaround. No tests were weakened. The ordinary focused/default-feature builds emit the existing `MAX_TRACKED_CHILD_PINS` unused-import warning. The required strict `tui,bench` build passes without warnings.

The sequence tail was:

```text
test tui::view::tests::a_failed_rerank_shows_the_error_and_keeps_the_ranking ... ok

test result: ok. 16 passed; 0 failed; 1 ignored; 0 measured; 3114 filtered out; finished in 0.10s

{"filter":true,"platforms":{"opencode":{"models":[{"id":"opencode/big-pickle","active":true,"reason":"default","rank":1},{"id":"opencode/deepseek-v4.1-flash-free","active":true,"reason":"default","rank":2},{"id":"opencode/glm-5.3-flash-free","active":true,"reason":"default","rank":3},{"id":"opencode/nemotron-3-ultra-free","active":true,"reason":"default","rank":4},{"id":"opencode/mimo-v2.6-flash-free","active":true,"reason":"default","rank":5},{"id":"opencode/longcat-2.5-preview-free","active":true,"reason":"default","rank":6},{"id":"opencode/beam-free","active":true,"reason":"default","rank":7},{"id":"opencode/space-bunny-free","active":true,"reason":"default","rank":8},{"id":"opencode/nemotron-3.5-super-vl-free","active":true,"reason":"default","rank":9},{"id":"opencode/nemotron-3.5-lightning-free","active":true,"reason":"default","rank":10}],"error":null}}}

TASK-007 VERIFY: all five commands exited 0 (RUST_TEST_THREADS=1)
```

PR: https://github.com/jesusvillota/shunt/pull/29, into `main`, labeled `goal:model-filter`.

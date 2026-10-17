# TASK-004 outcome — Rerank the OpenCode models with a headless agent

## Acceptance

- [x] With a fake ranker that answers valid JSON, the stored order matches the answer
  after validation; unknown ids are dropped and missing ids are appended.
  Evidence: `cargo test --features tui --lib rerank` passes (10 passed, 1 ignored).
  `rerank::tests::a_valid_answer_is_stored_in_its_order` runs `rerank::run` with a fake
  `Runner` that answers `{"order": ["opencode/c", "opencode/b", "opencode/a"]}` and asserts
  the returned order equals it and `ranked_at` is the run time.
  `rerank::tests::unknown_ids_are_dropped_and_missing_ids_appended` answers
  `{"order": ["opencode/b", "opencode/gone", "opencode/b"]}` over `a,b,c` and asserts
  `[b, a, c]`: the unknown id drops, the duplicate collapses, and the missing `c` is
  appended. The same validation backs the live run below.
- [x] With a fake ranker that answers invalid JSON or times out, the old ranking stays and
  the TUI shows the error.
  Evidence: `rerank::tests::a_failing_or_timing_out_ranker_leaves_no_ranking` (the fake
  returns `claude timed out after 600s`) makes `rerank::run` return `Err`, so the caller
  never overwrites `opencode_rank`; `rerank::tests::an_invalid_answer_is_an_error` covers
  non-JSON answers. `tui::models::tests::a_failed_rerank_keeps_the_old_ranking_and_shows_the_error`
  calls `ModelsDialog::on_ranked(Err(..))` and asserts `Ranking::Failed` while
  `prefs.opencode_rank` is still `Some`. `tui::view::tests::a_failed_rerank_shows_the_error_and_keeps_the_ranking`
  draws the dialog and asserts the screen holds
  `rerank failed, old ranking kept: claude timed out after 600s` and the rank number `#1`.
- [x] The default criteria file is created once and is not overwritten.
  Evidence: `rerank::tests::the_default_criteria_file_is_written_once_and_not_overwritten`
  reads a missing path (which writes `DEFAULT_CRITERIA`), then writes `my own weights` and
  reads it again, getting the custom text back. The live run below created the real
  `~/.shunt/opencode-rank-criteria.md` from absent, and the file holds the default text.
- [x] The OpenCode section shows rank numbers and `ranked_at`;
  `shunt models opencode --json` includes `rank`.
  Evidence: `tui::view::tests::opencode_section_shows_rank_numbers_in_stored_order_and_ranked_at`
  draws the dialog with a stored ranking and asserts `#1`/`#2`,
  `ranked_at 2026-10-17T14:15:37Z`, and that `opencode/c` precedes `opencode/a`.
  `models_prefs::tests::resolve_numbers_opencode_rows_in_stored_order` asserts the `rank`
  numbers and that another platform carries none. Live CLI after the live rerank
  (`cargo run -q -- models opencode --json`):
  ```
  {"filter":true,"platforms":{"opencode":{"models":[{"id":"opencode/big-pickle","active":true,"reason":"default","rank":1},{"id":"opencode/deepseek-v4.1-flash-free","active":true,"reason":"default","rank":2},{"id":"opencode/glm-5.3-flash-free","active":true,"reason":"default","rank":3},{"id":"opencode/nemotron-3-ultra-free","active":true,"reason":"default","rank":4},{"id":"opencode/mimo-v2.6-flash-free","active":true,"reason":"default","rank":5},{"id":"opencode/longcat-2.5-preview-free","active":true,"reason":"default","rank":6},{"id":"opencode/nemotron-3.5-super-vl-free","active":true,"reason":"default","rank":7},{"id":"opencode/beam-free","active":true,"reason":"default","rank":8},{"id":"opencode/space-bunny-free","active":true,"reason":"default","rank":9},{"id":"opencode/nemotron-3.5-lightning-free","active":true,"reason":"default","rank":10}],"error":null}}}
  ```
- [x] One live rerank with the default ranker is run once and its result is pasted into the
  outcome.
  Evidence: `cargo test --features tui --lib rerank -- --ignored --nocapture` ran the real
  default ranker `claude -p … --allowedTools WebSearch` over the 10 live `opencode models`
  ids and stored the result:
  ```
  running 1 test
  criteria: /Users/jesusvillotamiranda/.shunt/opencode-rank-criteria.md
  models: 10 live
  ranked_at: 2026-10-17T14:37:14Z
    #1 opencode/big-pickle
    #2 opencode/deepseek-v4.1-flash-free
    #3 opencode/glm-5.3-flash-free
    #4 opencode/nemotron-3-ultra-free
    #5 opencode/mimo-v2.6-flash-free
    #6 opencode/longcat-2.5-preview-free
    #7 opencode/nemotron-3.5-super-vl-free
    #8 opencode/beam-free
    #9 opencode/space-bunny-free
    #10 opencode/nemotron-3.5-lightning-free
  test rerank::tests::live_rerank_prints_the_validated_order ... ok
  ```
- [x] fmt, clippy (`--features tui,bench`) and the full tests pass; the docs are updated in
  all four languages.
  Evidence: `cargo fmt --all --check` exits 0;
  `RUSTFLAGS="-D warnings" cargo clippy --all-targets --features tui,bench -- -D warnings`
  exits 0; `RUSTFLAGS="-D warnings" cargo test --features tui,bench --workspace` exits 0
  (48 `test result: ok` lines). Docs: `README.md` + `README.ko.md`/`README.ja.md`/`README.zh-CN.md`,
  `docs/pool-tui.md`, `site/src/content/docs/reference/cli.mdx` and
  `site/src/content/docs/guides/pool-account-controls.md`, each with their `ko`/`ja`/`zh-cn`
  copies. `wiki/` untouched (generated).

## Summary

What

- `src/rerank.rs` (new): the whole rerank. `run(criteria_path, platform, ids, runner, now)`
  reads or creates the criteria, builds one prompt with the criteria and the live ids, runs
  the ranker through the injectable `Runner` trait, validates the answer, and stamps
  `ranked_at` with `format_iso8601`. `ShellRunner` shells out and kills a run after
  `DEFAULT_TIMEOUT` (10 minutes). `command` maps `claude` (`claude -p <prompt> --allowedTools
  WebSearch`), `codex` (`codex exec --skip-git-repo-check`), `opencode` (`opencode run`) and
  `agy` (`claude -p <prompt> --model <pooled Antigravity id>`), and refuses `chatgptweb`
  with a clear message. `validate` keeps live ids, drops duplicates and unknown ids, and
  appends missing live ids; a non-JSON or `order`-less answer is an error. The criteria are
  the DEC-001 rubric and weights, written to `~/.shunt/opencode-rank-criteria.md` only when
  that file is absent.
- `src/models_prefs.rs`: `ModelPrefs` gains `ranker` (default `claude`) and
  `opencode_rank: Option<OpenCodeRank>`; `resolve` reorders `opencode` rows by the stored
  order and sets a 1-based `rank` (new field on `Resolved`, skipped in JSON when absent).
- `src/tui/models.rs`: a `Ranking` state, the `r` key on the `opencode` tab (ignored
  while a run is in flight, so one rerank runs at a time), and `start_ranking` /
  `on_ranked`.
- `src/tui/app.rs` + `src/tui/mod.rs`: `Effect::RerankOpencode` runs `rerank::run` on a
  blocking task, saves `opencode_rank` to `models.json`, and reports `Msg::Ranked`; the
  dialog shows `ranking…` while it runs and the error when it fails.
- `src/tui/view/models.rs`: rank numbers on each opencode row and a line with `ranked_at`
  (or the run state and error).
- `src/models_catalog.rs`: the plain report shows the rank number; `--json` carries `rank`.

How

The ranker is injectable, so every pipeline test uses a fake and never calls a model; the
one live run is an ignored test that mirrors the TUI (fetch live ids, rank, store). The
`claude` argv puts the prompt before `--allowedTools` because that flag is variadic and
otherwise swallows the prompt (this was the first live failure, now fixed and covered by a
unit test).

Deviations

None from the spec. `validate` accepts the JSON object anywhere in the answer (a code fence
or a sentence around it), which is stricter than "JSON only" in wording but never accepts a
non-JSON answer.

Verified revision

Code commit `fe8ef954` plus the docs/outcome commit that carries this file, at the
PR head.

Proposed follow-ups

- `agy` uses one pinned id (`claude-gemini-3.8-flash-medium-via-antigravity`); refresh it
  from `/v1/models` when it retires (marked with a `shortcut:` comment).
- Right now a missing live id is appended at the end and shown with a rank, so a rerank
  silently extends the stored order; a future task could flag "new since ranking".

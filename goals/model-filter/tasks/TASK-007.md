---
id: TASK-007
title: Rank the OpenCode models with one plain order, no weights
milestone: M-4
state: approved
depends_on: [TASK-006]
delivery: pr
---

## Why

Serves SC-4. TASK-006 made the ranker give 0 to 10 scores per parameter and had shunt compute a weighted sum. The owner does not need weights. The owner wants an accurate order of the OpenCode models for agentic coding in real repositories. Absolute scores are mostly invented and add failure cases. One plain order from the agent is simpler and as accurate.

## Scope

Build on TASK-006 (`src/rerank.rs`, `src/rerank/tests.rs`, `src/models_prefs.rs`, `src/tui/models.rs`, `src/tui/view/models.rs`). This task removes code.

- The prompt asks the agent for one final order: JSON only, `{"order": ["opencode/<id>", …]}`, best first. It contains the criteria text and the live model list, as in TASK-004. It does not ask for scores.
- Remove the weight-table parser, the weighted-sum maths, tie-breaks by score, the unscored-model handling and the `scores` / `score` fields. Keep the TASK-004 validation: unknown ids are dropped, duplicates removed, missing live ids appended at the end. Invalid JSON, a timeout, or a missing `order` array keeps the old ranking and shows the error.
- `DEFAULT_CRITERIA` becomes plain text, not a table: rank the models by how well they do agentic coding in a real repository (reading code, editing files, running tools and tests, following instructions), and look up current benchmarks, especially agentic-coding ones. The file is still written only when absent. A file byte-identical to the TASK-006 default (and to the TASK-004 default) is rewritten once to the new text; any other file is left as the owner edited it. The criteria text is passed to the agent as it is; there is no required format.
- Storage: `opencode_rank` is `{ranked_at, order}` again. A stored `scores` key in an old `models.json` must be ignored on load without error, and dropped on the next save.
- `shunt models opencode --json` and the Models dialog keep `rank` and `ranked_at`; the `score` field and the score column are gone.
- Keep the ranker picker from TASK-005 unchanged.
- Tests use the fake `Runner`: valid order stored as given, unknown ids dropped, missing ids appended, duplicates removed, invalid JSON and timeout keep the old ranking, the old-default rewrite and the edited-file no-rewrite, and an old `models.json` with `scores` still loading. Remove only tests of the removed scoring behaviour; do not weaken any other test.
- Update the docs it affects in the same PR: `docs/pool-tui.md`, the four READMEs, and the site pages with their ko/ja/zh-cn copies. They must stop describing weights, scores and the 50/20/20/10 table. Do not edit the accepted DEC-001; say in the outcome that this task supersedes the scoring of TASK-006.

Out of scope: choosing which model ranks (the owner uses the TASK-005 picker); the goal tool.

## Acceptance

- With a fake ranker answering `{"order": [...]}`, the stored order equals the answer after validation: unknown ids dropped, duplicates removed, missing ids appended.
- The prompt a test captures names the criteria text and the live ids, and asks for `order`, not scores.
- Invalid JSON, a timeout, or a missing `order` keep the old ranking and show the error.
- `shunt models opencode --json` carries `rank` and no `score`; an old `models.json` that has `scores` loads without error.
- An untouched TASK-006 default criteria file is rewritten once to the plain text; an edited file is left alone.
- No weight or score code remains (`grep -rniE 'weighted|score' src/rerank.rs src/models_prefs.rs` finds nothing about weights; any hit is explained in the outcome).
- Docs are updated in all four languages; fmt, clippy (`--features tui,bench`) and the full tests pass.

## Verify

```sh
cargo fmt --all --check
RUSTFLAGS="-D warnings" cargo clippy --all-targets --features tui,bench -- -D warnings
RUSTFLAGS="-D warnings" cargo test --features tui,bench --workspace
cargo test --features tui --lib rerank
cargo run -q -- models opencode --json
```

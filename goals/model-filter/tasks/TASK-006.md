---
id: TASK-006
title: Score the OpenCode models per parameter and compute the order from weights
milestone: M-4
state: approved
depends_on: [TASK-005]
delivery: pr
---

## Why

Serves SC-4. DEC-001 accepted option C (the agent scores each parameter, shunt computes the order from the owner's weights), but TASK-004 shipped option B: the agent returns an order and the weights are only text in the prompt. The owner also changed the rubric. This task delivers option C with the new rubric.

## Scope

Build on TASK-005 (`src/rerank.rs`, `src/models_prefs.rs`, the criteria file `~/.shunt/opencode-rank-criteria.md`).

**New rubric (supersedes the table in DEC-001; the owner removed speed and availability):**

| parameter | weight |
| --- | --- |
| agentic coding | 50% |
| general intelligence | 20% |
| tool use | 20% |
| instruction following | 10% |

- The weights live in the criteria file as a `| parameter | weight |` table that shunt parses. The prompt asks the agent for a 0 to 10 score per parameter per model, as JSON only: `{"scores": {"opencode/<id>": {"<parameter>": 7.5, …}, …}}`. The agent does not give an order.
- Shunt computes `sum(score × weight) / sum(weights)` per model, sorts best first, and breaks ties by the previous stored order, then by the live list order. Weights need not add to 100; shunt normalizes them. A weight that is not a number, or a table with no parseable row, is an error shown in the TUI and the old ranking stays.
- Validation: keep only ids in the live list; a model whose scores are missing, not a number, or outside 0 to 10 is appended after the scored models (as TASK-004 appends missing ids) and the dialog says how many were unscored. Invalid JSON or a timeout keeps the old ranking, as today.
- Store the scores and the computed total with the order in `opencode_rank` (`{ranked_at, order, scores}`); old files without `scores` still load.
- Show the total score next to the rank number on the `opencode` tab; `shunt models opencode --json` gains `score` next to `rank`.
- `DEFAULT_CRITERIA` becomes the new rubric. The file is still written only when absent. A file that is byte-identical to the old TASK-004 default is rewritten to the new default once (the owner never edited it); any other file is left alone, and if it has no parseable weights the rerank fails with a message that names the file and the expected table.
- Tests use the fake `Runner`: weighted sum and order from known scores, tie-break, normalizing weights that do not add to 100, unscored models appended, bad weights, the old-default rewrite and the edited-file no-rewrite.
- Update the docs it affects in the same PR (`docs/pool-tui.md`, the four READMEs, the site pages and their ko/ja/zh-cn copies). Do not edit the accepted DEC-001.

Out of scope: the ranker picker (TASK-005); ranking codex or agy models; the goal tool.

## Acceptance

- With a fake ranker answering known scores, the stored order equals the weighted-sum order for the weights 50/20/20/10, and `shunt models opencode --json` shows `rank` and `score` for each model.
- Changing a weight in the criteria file changes the order on the next rerank with the same scores (a test shows two different orders).
- A model with a missing or out-of-range score is listed after the scored models and counted as unscored.
- The old default criteria file is rewritten once to the new rubric; an edited file is untouched.
- Invalid JSON, a timeout, or unparseable weights keep the old ranking and show the error.
- One live rerank with the configured ranker is run once and its scores and order are pasted into the outcome.
- Docs are updated in all four languages; fmt, clippy (`--features tui,bench`) and the full tests pass.

## Verify

```sh
cargo fmt --all --check
RUSTFLAGS="-D warnings" cargo clippy --all-targets --features tui,bench -- -D warnings
RUSTFLAGS="-D warnings" cargo test --features tui,bench --workspace
cargo test --features tui --lib rerank
cargo run -q -- models opencode --json
```

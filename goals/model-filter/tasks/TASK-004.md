---
id: TASK-004
title: Rerank the OpenCode models with a headless agent
milestone: M-3
state: approved
depends_on: [TASK-003, DEC-001]
delivery: pr
---

## Why

Serves SC-3. The operator wants the OpenCode free models ranked, and a key to rank them again when the free set changes. The criteria come from DEC-001.

## Scope

Build on the OpenCode section from TASK-003 and the criteria accepted in DEC-001.

- Key `r` in the OpenCode section starts a rerank in a background task. The UI never blocks, shows `ranking…`, and one rerank runs at a time. A timeout (default 10 minutes) ends a stuck run.
- The ranker is a headless agent. The platform is set by a new `ranker` key in `~/.shunt/models.json`; the default is `claude` (`claude -p` with web search allowed, so it can look up current benchmarks). Also support `codex` (`codex exec`), `opencode` (`opencode run`) and `agy` (`claude -p` on a pooled Antigravity model). `chatgptweb` needs a browser session that shunt cannot start, so report it as unsupported with a clear message.
- The prompt contains the live OpenCode model list and the criteria. The criteria live in `~/.shunt/opencode-rank-criteria.md`, created with the default text from DEC-001 when absent, so the owner can edit them without a code change. The agent must answer with JSON only.
- Validate the answer: keep only ids that are in the live list, drop duplicates, append any missing live ids at the end. If the answer is not valid JSON, keep the old ranking and show the error.
- Store `opencode_rank: {ranked_at, order}` in `~/.shunt/models.json`. The OpenCode section shows models in rank order with the rank number and `ranked_at`. `shunt models --json` gains a `rank` field for opencode models.
- Run the ranker through an injectable command runner so tests use a fake ranker and never call a real model.
- Update the docs in the same PR in all four languages.

Out of scope: ranking on a schedule; ranking codex or agy models; changing the goal tool (the `model-filter-consumer` goal reads `rank` later).

## Acceptance

- With a fake ranker that answers valid JSON, the stored order matches the answer after validation. Unknown ids are dropped and missing ids are appended.
- With a fake ranker that answers invalid JSON or times out, the old ranking stays and the TUI shows the error.
- The default criteria file is created once and is not overwritten.
- The OpenCode section shows rank numbers and `ranked_at`; `shunt models opencode --json` includes `rank`.
- One live rerank with the default ranker is run once and its result is pasted into the outcome.
- fmt, clippy (`--features tui,bench`) and the full tests pass; the docs are updated in all four languages.

## Verify

```sh
cargo fmt --all --check
RUSTFLAGS="-D warnings" cargo clippy --all-targets --features tui,bench -- -D warnings
RUSTFLAGS="-D warnings" cargo test --features tui,bench --workspace
cargo test --features tui --lib rerank
cargo run -q -- models opencode --json
```

---
id: TASK-001
title: 'Models dialog and shunt models command (PR #23)'
milestone: M-1
state: approved
depends_on: []
delivery: pr
pr: https://github.com/jesusvillota/shunt/pull/23
---

## Why

Serves SC-1. The operator needs one place that lists the live codex, opencode and agy models and switches them on or off. Everything later in this goal builds on this code.

## Scope

The work is **already implemented** in shunt PR #23 (branch `feat/models-filter`). This task adopts that PR; no new implementation is needed. The reviewer checks and merges it.

Included in the PR: `src/models_prefs.rs` (state file `~/.shunt/models.json`, the default rule that keeps the newest version of each model family on), `src/models_catalog.rs` (live lists from `codex debug models`, `opencode models` and the `…-via-antigravity` entries of `[[models]]`), the `shunt models [PLATFORM] [--json]` command, the Models dialog (`M`) in `shunt tui`, and the docs with their ko/ja/zh translations.

Out of scope: any change to gateway routing, `/v1/models` or Claude Code's `/model` picker; the goal-tool side (that is the `model-filter-consumer` goal in the config repo).

## Acceptance

- `shunt models --json` prints `filter` and, per platform, the models with `id`, `active` and `reason`.
- With no overrides, `gpt-6.1-sol` is on and `gpt-6-sol` is off; every effort level of the newest version stays on; a name with no version stays on.
- The Models dialog (`M`) lists each platform, `Space` toggles a model, `f` flips the filter, and the choice survives a restart.
- `cargo fmt --all --check`, clippy with `--features tui,bench`, and the full test suite pass.
- The docs (README and ko/ja/zh-CN, `docs/pool-tui.md`, site CLI reference and pool guide with ko/ja/zh-cn copies) describe the feature.

## Verify

```sh
cargo fmt --all --check
RUSTFLAGS="-D warnings" cargo clippy --all-targets --features tui,bench -- -D warnings
RUSTFLAGS="-D warnings" cargo test --features tui,bench --workspace
cargo run -q -- models --json
cargo test --features tui --lib models_
```

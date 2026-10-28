---
id: TASK-016
title: 'Fix discovery catalog ordering and fixtures (issue #73)'
milestone: M-2
state: approved
depends_on: []
delivery: pr
---

## Why

TASK-013's required Verify (`cargo test --all-features --workspace`) fails on `main` in three discovery catalog tests. DEC-007 chose to fix it in its own task so TASK-013 keeps its scope. Fixes jesusvillota/shunt#73.

## Scope

In `src/discovery.rs`, put the four July 2026 Antigravity aliases in newest-first order and update the default and curated response fixtures to match the intended catalog. Keep every assertion. Out of scope: any other change.

## Acceptance

- `discovery::tests::builtin_table_is_newest_first_with_unique_ids`, `default_returns_builtin_models_in_api_order` and `curated_models_precede_and_override_matching_builtins` pass.
- `cargo test --all-features --workspace` passes.

## Verify

`cargo fmt --all --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-features --workspace` (set `PYO3_PYTHON` as in AGENTS.md). Paste the output in the PR, with `Fixes jesusvillota/shunt#73` in the description.

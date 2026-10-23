---
id: TASK-011
title: 'Fix unused stage-store re-export clippy error (issue #52)'
milestone: M-1
state: approved
depends_on: []
delivery: pr
---

## Why

TASK-002's required Verify command (`cargo clippy --all-targets -- -D warnings`) fails on `main` because of an existing unused import. DEC-003 chose to fix it in its own task so TASK-002 keeps its scope. Fixes jesusvillota/shunt#52.

## Scope

In `src/routing/stage/store.rs:50`, gate the `MAX_TRACKED_CHILD_PINS` re-export on `feature = "bench"` alone. Keep the existing test/bench gate for `MAX_TRACKED_SESSIONS`. Do not suppress unused-import warnings. Out of scope: any other change.

## Acceptance

- `cargo clippy --all-targets -- -D warnings` passes with default features.
- `cargo clippy --all-targets --features bench -- -D warnings` and `cargo test` still pass.

## Verify

`cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, `cargo clippy --all-targets --features bench -- -D warnings`, `cargo test`. Paste the output in the PR, with `Fixes jesusvillota/shunt#52` in the description.

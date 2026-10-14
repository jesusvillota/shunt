---
id: TASK-013
title: Repair the goal-rust baseline checks in the config repo
milestone: M-1
state: approved
depends_on: [DEC-006]
delivery: pr
---

## Why

TASK-009 (PR #19 on `jesusvillota/config`) is done but cannot pass its required checks: the goal-rust workspace in the config repo already fails them before TASK-009 touches anything (DEC-005, option 1). This task repairs that baseline so TASK-009 can deliver with every gate intact.

## Scope

All work is in the **config repo** (`~/GitHub/config`, remote `jesusvillota/config`), crate workspace `tools/goal-rust`. The shunt repo gets only this task's outcome file.

- Work in a NEW git worktree of the config repo, branched from its `origin/main`. The main checkout has other people's uncommitted changes (`claude/settings.json`, `codex/config.toml`, `shared/instructions.md`): never touch or stash them.
- Three repairs, as three separate commits so they review easily:
  1. Formatting: `cargo fmt` over `tools/goal-rust` (about 2,100 diff blocks). Pure formatting, no logic change; check with a whitespace-insensitive diff or by confirming the compiled behaviour and tests are unchanged.
  2. Strict clippy: fix the about 77 goal-core lib and 80 lib-test diagnostics (`cargo clippy --all-targets -- -D warnings`). Prefer real fixes; an `#[allow]` needs a one-line reason and must be narrow. No behaviour change.
  3. Failing tests: about 22 of 39 in `execution_differential` fail, most with `unknown codex model 'gpt-6.1-sol'` (and an authority fixture that inherits `GOAL_AGENT`). Find the root cause (fixtures reference a model not in the current model table, or test isolation does not clear session environment variables) and fix the fixtures or isolation. Never weaken, delete or skip a test; never add a fake model entry to production tables just to satisfy a fixture unless the fixture is correct and the table is genuinely stale (state which).
- Do not touch `src/pool.rs` beyond formatting; TASK-009's PR #19 owns it, and it will be rebased onto this.
- Open the PR on `jesusvillota/config` with `gh pr create -R jesusvillota/config`; put its URL in the outcome. The shunt PR carries `Goal-Task: agy-pool/TASK-013` and the outcome only.

Out of scope: new features, other crates or directories in the config repo, shunt code.

## Acceptance

- In the config worktree, `cd tools/goal-rust && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` exits 0 (paste the tail of each; run the tests in a clean environment, with `GOAL_AGENT`, `GOAL_COPILOT` and `CLAUDECODE` unset, and also confirm they pass from an agent shell where those are set).
- The outcome lists the root cause of the test failures and any `#[allow]` added, each with its reason.
- No test removed, ignored or weakened (show the test count before and after).
- The config PR URL is in the outcome and its diff touches only `tools/goal-rust`.

## Verify

`cd <config worktree>/tools/goal-rust && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`

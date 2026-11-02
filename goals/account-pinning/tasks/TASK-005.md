---
id: TASK-005
title: Keeper reads the goal's own account subset for rate-limit resets
milestone: M-1
state: approved
depends_on: [TASK-003, DEC-001]
delivery: pr
repo: goal
route: {platform: codex, model: gpt-6.1-sol, effort: xhigh}
---

## Why

Serves SC-5. With a rate-limited subset, the goal must behave as today: the keeper sleeps and wakes the orchestrator when the goal's own accounts reset. Today the keeper reads all accounts of a platform.

## Scope

Work in the `goal` repo. Facts: `pool::fetch_bounded` (`goal-core/src/pool.rs:19`) runs `shunt pool --json`; `reset_time` (`:89`) and `exhausted` (just below) read every account of the provider; `session.rs:1073-1077` calls them for the keeper.

Work:

1. Pass `--client <slug>` to `shunt pool --json` when the goal has a shunt token (TASK-004's Keychain item `shunt-goal-<slug>`). TASK-003 adds the flag; the JSON shape is unchanged.
2. If the installed shunt rejects the flag (an older binary), fall back to the call without it and log one warning.
3. Tests with a fake `SHUNT_BIN` that records its arguments: the flag is passed when the goal has a token, not passed otherwise, and the fallback works.
4. Docs in the goal repo where the keeper and `goal pool` are described.

Out of scope: shunt changes.

## Acceptance

- Tests show `shunt pool --json --client <slug>` is called for a goal with a token and `shunt pool --json` otherwise.
- A test with a fake pool where only the goal's account C is exhausted shows `exhausted` is true and `reset_time` is C's reset.
- The goal repo's full test suite and lint pass.

## Verify

```sh
cd tools/goal-rust && cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace
```

# TASK-013 outcome

## Summary

**Blocked on one test.** After DEC-006 (option 2), fmt and clippy pass for `goal-rust` and `goal-tui`, and 495 of 496 tests pass. The last failing test needs a doc edit outside the allowed paths.

Config PR: https://github.com/jesusvillota/config/pull/110 (open, draft, base `main`).
Config branch: `jesusvillota/goal-agy-pool-task-013-baseline`. Verified revision: `edfaaa3efa862cf0a09139007434cadeb188368b`, rebased on `origin/main` at `cf27a69`.
Seven commits, one concern each: fmt `goal-rust`, fmt `goal-tui`, clippy `goal-tui`, clippy `goal-rust`, then three test-fixture commits.
`src/pool.rs` has formatting changes only. The main config checkout was not touched.
No README, docs, site or wiki update is needed: formatting, lint and test-fixture changes only.

Re-count after rebase: main now holds #102/#103/#105/#116, which fixed part of the failures (the Orca delivery fixture and the Codex PR-recovery race). Remaining failures were: 3 launcher-argv tests, 1 launcher-push test, 3 `sequential` coordinator tests, and `format_spec_carries_worker_protocol`.

## Decision request

Should TASK-013 touch `shared/skills/goal-setup/references/format.md` to sync it with the worker-protocol template?

`store_validate::format_spec_carries_worker_protocol` compares the "Worker protocol" section of `shared/skills/goal-setup/references/format.md` with `goal_core::templates::README`. Commit 1775a11 (#104) added three lines to the template (the `origin` / `gh pr create --repo` guidance) and did not update `format.md`. The test has failed on `origin/main` since then. The fix is to copy those three lines into `format.md`. That file is outside `tools/goal-rust` and `tools/goal-tui`, so it breaks acceptance item 4.

Options:

1. Widen TASK-013 by one file: add the three lines to `format.md` in PR #110. One PR, all checks green. Open PRs #113 and #114 also edit `format.md` and may conflict on rebase. #113 also edits `templates.rs`, so it may change the same section.
2. Wait for #113, which may re-sync `format.md` and the template, then rebase PR #110 and re-run. No scope change. Depends on another PR landing.
3. Open a separate task for the doc sync. Adds a task and leaves the workspace red until it lands.

### Recommendation

Choose option 1. It is a three-line doc change that the test already requires. If #113 lands first, rebase and drop the edit.

## Acceptance

- [ ] **1. Required checks exit 0 in clean and agent environments.** Not satisfied: only `format_spec_carries_worker_protocol` fails (see the decision request). `cargo fmt --check` exits 0. `cargo clippy --all-targets -- -D warnings` exits 0. `cargo test --no-fail-fast` reports **495 passed, 1 failed, 0 ignored** both in a clean environment (`env -u GOAL_AGENT -u GOAL_COPILOT -u CLAUDECODE -u GOAL_PLAN_ROOT -u CODEX_CI -u OPENCODE`) and with `GOAL_AGENT=1 GOAL_COPILOT=agy-pool CLAUDECODE=1`. Both runs stop with `test exit 101` on that one test.
- [x] **2. Root causes and added allows recorded.** See below.
- [x] **3. No test removed, ignored, or weakened.** `#[test]` count in `tools/goal-rust` and `tools/goal-tui` is **607 at `origin/main` and 607 at HEAD**. 0 ignored. No assertion changed. One fixture was changed to poll differently and then reverted once main fixed the race in production code.
- [x] **4. Config PR URL and scoped diff.** https://github.com/jesusvillota/config/pull/110. `git diff origin/main...HEAD --name-only` lists 99 files, all under `tools/goal-rust/` or `tools/goal-tui/` (0 outside).

## Root causes of the test failures

1. **Stale fixtures assume a Claude default route.** Commit 5e0db48 (#105) made the fallback route `codex/gpt-6.1-sol/high`. Fixtures in `execution_differential.rs`, `launcher_push_differential.rs` and `sequential.rs` only stub `claude` and assert Claude argv (`-p`), so they hit `unknown codex model 'gpt-6.1-sol'` or the wrong argv (`exec`). The production table is current; the fixtures were stale. Fix: each fixture goal now sets `default_route: {platform: claude, model: opus, effort: high}`. The tests that set their own `test-model` Codex route replace that line instead of appending a duplicate key. `failed_child_and_missing_codex_stream_cannot_deliver_ready_pr` needs the Codex fallback, so it removes the pin. No production table changed.
2. **Orca delivery fixture and PR-recovery race** were fixed on main by #116 (TASK-011). No change here.
3. **Worker-protocol doc drift** (open): `format.md` is behind the template since #104.

Environment note: authority fixtures only clear `GOAL_AGENT` and `CLAUDECODE`. `GOAL_PLAN_ROOT`, `CODEX_CI` and `OPENCODE` also change results in an agent shell, so the clean run unsets them too.

## `#[allow]` added

Three, all `#[allow(clippy::too_many_arguments)]` (8 of 7), narrow, on the function:
`dispatch::launch_attempt`, `herdr::launch`, `mutate::add`. Reason: they mirror existing positional call sites; a parameter struct would be a refactor, which the task forbids.

Other lint fixes were real: collapsed `if`s, `strip_prefix`, `let Self { .. }` initializer, `type` aliases for complex types (`Stamp`, `Mutation`, `HelpTarget`), removed an identical-branch `if`, `sort_by_key`, `find` instead of an index loop. One test helper (`suffixes`) is now `#[cfg(feature = "differential")]`, as are its only callers. One unused field was renamed `_dir` (it holds a `TempDir` for drop).

## Overlap with other work

Goal `goal-tui-improvements` TASK-010 (launcher fixtures that assume a Claude default) edits the same fixtures as commit `fix(goal): pin the claude default route in stale launcher fixtures`. Whichever PR lands second needs a rebase.

## Verification tails

From the config worktree's `tools/goal-rust`, at `edfaaa3`:

```text
$ cargo fmt --check
fmt exit 0
$ cargo clippy --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.55s
clippy exit 0
$ cargo test --no-fail-fast   (clean env and agent env)
test format_spec_carries_worker_protocol ... FAILED
test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s
all other binaries ok; total 495 passed, 1 failed, 0 ignored
test exit 101
```

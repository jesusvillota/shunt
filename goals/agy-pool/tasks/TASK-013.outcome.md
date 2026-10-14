# TASK-013 outcome

## Summary

**Delivered.** `goal-rust` and `goal-tui` pass fmt, clippy and the full test suite. DEC-006 (option 2) widened the task to `tools/goal-tui`. DEC-007 (option 1) widened it by the three template lines in `format.md`.

Config PR: https://github.com/jesusvillota/config/pull/110 (ready for review, base `main`).
Config branch: `jesusvillota/goal-agy-pool-task-013-baseline`. Verified revision: `d96dda81ff406ee114713b5f66f9877378e3c015`, rebased on `origin/main` at `29c8e4f`.
Eight commits: fmt `goal-rust`, fmt `goal-tui`, clippy `goal-tui`, clippy `goal-rust`, three test-fixture commits, and one `format.md` sync plus rustfmt fix.
`src/pool.rs` has formatting changes only (the owner is changing goal pool directly; TASK-009 is cancelled).
No README, docs, site or wiki update is needed: formatting, lint, fixture and one template-sync change only.

## Acceptance

- [x] **1. Required checks exit 0 in clean and agent environments.** `cargo fmt --check` exits 0. `cargo clippy --all-targets -- -D warnings` exits 0. With `GOAL_AGENT=claude GOAL_COPILOT=1 CLAUDECODE=1`, `cargo test --no-fail-fast` reports **496 passed, 0 failed, 0 ignored**. In a clean environment (`env -u GOAL_AGENT -u GOAL_COPILOT -u CLAUDECODE sh -c 'cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test'`) the whole chain **exits 0 with 496 passed, 0 failed, 0 ignored** (final run, at `d96dda8`). Two earlier clean full runs failed (12, then 2 tests) because the fake `herdr` server tests have a fixed 5-second timeout and the machine load average was about 10 from other agents' builds; those tests passed when rerun alone (`execution_differential` 40/40, `launcher_push_differential` 4/4). The timeout is fragile under heavy load; I did not change it.
- [x] **2. Root causes and added allows recorded.** See below.
- [x] **3. No test removed, ignored, or weakened.** `#[test]` count in `tools/goal-rust` and `tools/goal-tui` is **604 at `origin/main` (29c8e4f) and 604 at HEAD** (counted with `git grep '#\[test\]'`; the earlier 607 was measured against an older `main` and `main` has since lost tests, none by this PR: against `29c8e4f` this PR's diff has 20 `#[test]` lines added and 20 removed, all from `cargo fmt` expanding one-line test functions, so the net change is 0). 0 ignored. No assertion changed. One fixture was changed to poll differently and then reverted once main fixed the race in production code.
- [x] **4. Config PR URL and scoped diff.** https://github.com/jesusvillota/config/pull/110. `git diff origin/main...HEAD --name-only` lists 100 files: 99 under `tools/goal-rust/` or `tools/goal-tui/`, plus `shared/skills/goal-setup/references/format.md` (+3 lines), which DEC-007 allows.

## Root causes of the test failures

1. **Stale fixtures assume a Claude default route.** Commit 5e0db48 (#105) made the fallback route `codex/gpt-6.1-sol/high`. Fixtures in `execution_differential.rs`, `launcher_push_differential.rs` and `sequential.rs` only stub `claude` and assert Claude argv (`-p`), so they hit `unknown codex model 'gpt-6.1-sol'` or the wrong argv (`exec`). The production table is current; the fixtures were stale. Fix: each fixture goal now sets `default_route: {platform: claude, model: opus, effort: high}`. The tests that set their own `test-model` Codex route replace that line instead of appending a duplicate key. `failed_child_and_missing_codex_stream_cannot_deliver_ready_pr` needs the Codex fallback, so it removes the pin. No production table changed.
2. **Orca delivery fixture and PR-recovery race** were fixed on main by #116 (TASK-011). No change here.
3. **Worker-protocol doc drift:** `format.md` was behind the template since #104. DEC-007 (accepted, option 1) allowed a one-file widening. The three template lines (`origin` / `gh pr create --repo`) are now in `shared/skills/goal-setup/references/format.md`. Open PRs #113 and #114 edit other parts of `format.md`, not these lines, so no sync conflict.
4. **rustfmt after rebase:** one `execution_differential.rs` fixture line needed `cargo fmt`.

Environment note: authority fixtures only clear `GOAL_AGENT` and `CLAUDECODE`. `GOAL_PLAN_ROOT`, `CODEX_CI` and `OPENCODE` also change results in an agent shell, so the clean run unsets them too.

## `#[allow]` added

Three, all `#[allow(clippy::too_many_arguments)]` (8 of 7), narrow, on the function:
`dispatch::launch_attempt`, `herdr::launch`, `mutate::add`. Reason: they mirror existing positional call sites; a parameter struct would be a refactor, which the task forbids.

Other lint fixes were real: collapsed `if`s, `strip_prefix`, `let Self { .. }` initializer, `type` aliases for complex types (`Stamp`, `Mutation`, `HelpTarget`), removed an identical-branch `if`, `sort_by_key`, `find` instead of an index loop. One test helper (`suffixes`) is now `#[cfg(feature = "differential")]`, as are its only callers. One unused field was renamed `_dir` (it holds a `TempDir` for drop).

## Overlap with other work

Goal `goal-tui-improvements` TASK-010 (launcher fixtures that assume a Claude default) edits the same fixtures as commit `fix(goal): pin the claude default route in stale launcher fixtures`. Whichever PR lands second needs a rebase.

## Verification tails

From the config worktree's `tools/goal-rust`, at `d96dda8`:

```text
$ cargo fmt --check
FMT_OK
$ cargo clippy --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.41s
$ GOAL_AGENT=claude GOAL_COPILOT=1 CLAUDECODE=1 cargo test --no-fail-fast
total: 496 passed, 0 failed, 0 ignored
$ env -u GOAL_AGENT -u GOAL_COPILOT -u CLAUDECODE cargo test -p goal --test execution_differential
test result: ok. 40 passed; 0 failed; 0 ignored
$ env -u GOAL_AGENT -u GOAL_COPILOT -u CLAUDECODE sh -c 'cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test'
exit 0; total 496 passed, 0 failed, 0 ignored
```

## Rebase onto moved main (config PR #110)

PR #110 went DIRTY after main moved (#111 and later). The branch now sits on current `origin/main`: it carries only the `cargo fmt` output for `tools/goal-rust` and `tools/goal-tui`, plus the clippy fixes. The `format.md` and sequential-fixture edits were dropped because main already has them. Formatting was regenerated with `cargo fmt`, not hand-merged.

New head: `e88f682f31a49356b1424c17d5876d35c7dafc93`. GitHub shows `mergeStateStatus` CLEAN. PR is open, not a draft, labelled `goal:agy-pool`, body has `Goal-Task: agy-pool/TASK-013`. Diff touches only `tools/goal-rust` and `tools/goal-tui`. `#[test]` count: 629 on main, 629 on the branch; no `#[ignore]`.

Extra root cause found while rebasing: the old fix for `single_element_loop` was not needed on the final tree. Still three `#[allow(clippy::too_many_arguments)]`, same reason as above.

```text
$ cargo fmt --check                                   -> exit 0
$ cargo clippy --all-targets -- -D warnings           -> exit 0 (goal-tui also clean)
$ env -u GOAL_AGENT -u GOAL_COPILOT -u CLAUDECODE cargo test --no-fail-fast
total: 521 passed, 0 failed, 0 ignored
$ GOAL_AGENT=claude GOAL_COPILOT=1 CLAUDECODE=1 sh -c 'cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --no-fail-fast'
exit 0; total: 521 passed, 0 failed, 0 ignored
```

One earlier agent-env run had `execution_differential::all_process_launchers_match_argv_registry_ledger_and_history` fail under heavy machine load; it passed alone (twice) and in the next full run.

## Final state (head e88f682)

Main moved twice more during the work (#120 added a complex `limit_catalog` type, now aliased as `LimitCatalog`). Rebuilt on `origin/main`; `gh pr view 110`: CLEAN, open, not a draft, label `goal:agy-pool`, 0 reviews, 0 comments, 0 inline comments, 0 checks configured. Shunt PR #22: open, not a draft, 0 reviews/comments. Item 4 correction: the earlier line listing `format.md` is obsolete; `git diff origin/main --name-only` now lists only `tools/goal-rust` and `tools/goal-tui`.

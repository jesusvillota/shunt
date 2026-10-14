# TASK-013 outcome

## Summary

**Blocked:** required workspace checks need forbidden `tools/goal-tui` changes.

Config PR: https://github.com/jesusvillota/config/pull/110 (open, draft, base `main`).
Config worktree: `/Users/jesusvillotamiranda/orca/workspaces/config/goal-agy-pool-task-013-baseline`.
Config branch: `jesusvillota/goal-agy-pool-task-013-baseline`.
Verified config revision: `e46892c448e2dc45fc84bf8dd4589e9e8c105b17`.
Base revision: `ce24d9e677e822fdddc13d0103dba5849a43e2cc` (`origin/main` when created).

Only the formatting repair is committed. The strict clippy and test repairs remain incomplete.
The formatting commit changes 78 files, all under `tools/goal-rust`.
`src/pool.rs` has formatting changes only. The main config checkout was not changed or stashed.
An exploratory `cargo clippy --fix` also changed three `goal-tui` files. All exploratory
changes were reverted before the final verification. No lint fixes remain in the PR.
No production behavior, config keys, or provider semantics changed. No README, docs,
site, or generated wiki update is needed for formatting and this blocked outcome.

## Decision request

Should a separate task repair `tools/goal-tui` before TASK-013 resumes?

`tools/goal-rust/Cargo.toml` includes `../goal-tui` as a workspace member.
Thus the exact required commands also check that crate. After all permitted formatting,
`cargo fmt --check` still exits 1 for these six files:

- `tools/goal-tui/src/app.rs`
- `tools/goal-tui/src/keymap.rs`
- `tools/goal-tui/src/main.rs`
- `tools/goal-tui/src/ui.rs`
- `tools/goal-tui/tests/support/actions.rs`
- `tools/goal-tui/tests/support/layout.rs`

The exploratory clippy run also reported `goal-tui` diagnostics, including `type_complexity`
for `help_back` in `src/main.rs`, and proposed fixes in `main.rs`, `ui.rs`, and `layout.rs`.
Repairing only goal-rust cannot make the workspace checks pass. Excluding goal-tui from
checks would change the required validation and reduce test coverage.

Options:

1. Repair the goal-tui baseline in a separate prerequisite task. Keep this task's scope
   and acceptance unchanged. TASK-013 resumes after that repair reaches `origin/main`.
   This adds a task but keeps both PRs focused.
2. Explicitly authorize goal-tui repairs in TASK-013 and revise its diff-scope acceptance.
   This allows one baseline PR, but breaks the current requirement that its diff touch
   only `tools/goal-rust`.

### Recommendation

Choose option 1. Repair goal-tui separately, then resume TASK-013 with the original checks intact.

## Acceptance

- [ ] **1. Required checks exit 0 in clean and agent environments.** Not satisfied.
  At the verified revision, `cargo fmt --check` exits 1 and strict clippy exits 101.
  `env -u GOAL_AGENT -u GOAL_COPILOT -u CLAUDECODE cargo test` exits 101.
  `GOAL_AGENT=1 GOAL_COPILOT=agy-pool CLAUDECODE=1 cargo test` also exits 101.
  Both stop in `cli`: 11 passed, 1 failed, 0 ignored. The exact combined command
  cannot pass because formatting fails first. Scoped formatting passes with
  `cargo fmt --package goal --package goal-core --check`; this is diagnostic evidence,
  not a substitute for the required workspace check.
- [x] **2. Root causes and added allows recorded.** The confirmed fixture faults are below.
  The complete repair and diagnosis of the other execution failures remain pending.
  No new `#[allow]` was added; there are no allowance reasons to list.
- [x] **3. No test removed, ignored, or weakened.** `cargo test -- --list` records
  **492 tests before and 492 after**. Comparing complete test-name multisets gives
  equality. Only formatting is committed. The execution suite still runs all
  **39 tests**: 17 passed, 22 failed, 0 ignored. No assertions were changed.
- [x] **4. Config PR URL and scoped diff.** https://github.com/jesusvillota/config/pull/110.
  `git diff origin/main HEAD --name-only` lists 78 files; all begin with
  `tools/goal-rust/`. `git diff --check origin/main HEAD` passes. The config worktree is clean.

## Confirmed test root causes

1. `goal/tests/execution_differential.rs` defines a WORKER debug catalog that advertises
   only `test-model/high`. Its default goal fixture supplies no `policy.default_route`.
   `goal-core/src/routing.rs` selects `codex/gpt-6.1-sol/high`. Catalog validation then
   rejects the route before launch: `unknown codex model 'gpt-6.1-sol'`.
   The focused `all_process_launchers_match_argv_registry_ledger_and_history` test
   reproduces this error with the three requested markers unset. Repair the fake worker
   catalog to advertise the real default model while retaining `test-model` for route
   tests. No production model-table change is needed.
2. The CLI and route authority fixtures clear `GOAL_AGENT` and `CLAUDECODE`, but inherit
   `CODEX_CI` and `OPENCODE`. `human::COPILOT_MARKERS` treats those as user-authorized
   copilot markers. This shell has nonempty `CODEX_CI`, so clearing the three acceptance
   variables alone still lets calls pass when these tests expect terminal refusal.
   The observed CLI failure is `unwrap_err()` on `Ok(())`. Fix child-process isolation
   to clear all authority markers before setting the exact markers each case tests.
3. The full execution run has 22 failures. Most are consistent with the catalog fault.
   One failure in `failed_child_and_missing_codex_stream_cannot_deliver_ready_pr`
   compares a crash snapshot with a later delivered snapshot from fresh PR facts.
   Its remaining cause has not been isolated. No assertion was weakened to hide it.

## Verification tails

All commands below ran from the config worktree's `tools/goal-rust` directory.

### cargo fmt --check

```text
+            .style(Style::new().bg(pal().panel))
+            .padding(Padding::horizontal(1))
+    };
+    f.render_widget(
+        Paragraph::new(lines)
+            .wrap(Wrap { trim: false })
+            .scroll((app.pane_scroll, 0))
+            .block(block),
+        area,
+    );
 }
 
Exit: 1
```

### cargo clippy --all-targets -- -D warnings

```text

error: could not compile `goal-core` (lib) due to 77 previous errors
warning: build failed, waiting for other jobs to finish...
error: very complex type used. Consider factoring parts into `type` definitions
    --> goal-core/src/session.rs:1691:20
     |
1691 |         let cases: Vec<(&str, Box<dyn Fn(&mut Mem, &mut Cfg, &mut Obs)>)> = vec![
     |                    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
     |
     = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.97.0/index.html#type_complexity

error: could not compile `goal-core` (lib test) due to 80 previous errors
Exit: 101
```

### clean cargo test (three markers unset)

```text
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 11 filtered out; finished in 0.00s

 
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    core_human_context_obeys_cockpit_and_agent_markers

test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.24s

error: test failed, to rerun pass `-p goal --test cli`
Exit: 101
```

### agent cargo test (three markers set)

```text
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 11 filtered out; finished in 0.00s

 
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    core_human_context_obeys_cockpit_and_agent_markers

test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.22s

error: test failed, to rerun pass `-p goal --test cli`
Exit: 101
```

### focused default-model fixture test

```text
{"error":"unknown codex model 'gpt-6.1-sol'"}


note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    all_process_launchers_match_argv_registry_ledger_and_history

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 38 filtered out; finished in 2.32s

error: test failed, to rerun pass `-p goal --test execution_differential`
Exit: 101
```

### execution suite (three markers unset)

```text
    orca_and_herdr_require_ready_task_before_adapter_creation
    orca_setup_failure_and_delivered_ending_match_shared_registry
    print_launcher_condition_argv_ledger_history_and_resume
    registration_and_gate_failure_never_execute_unregistered_worker
    rename_failure_rolls_back_orca_metadata_without_touching_old_branch
    startup_timeouts_close_adapter_and_remove_new_checkout
    stopped_herdr_server_prevents_coordinator_claim
    successful_setup_and_live_capacity_match_before_new_checkout

test result: FAILED. 17 passed; 22 failed; 0 ignored; 0 measured; 0 filtered out; finished in 139.29s

error: test failed, to rerun pass `-p goal --test execution_differential`
Exit: 101
```

Local evidence logs: `/tmp/task013-evidence/`. GitHub Actions were not read or waited on.
Both PRs remain drafts. The native goal is not complete. No PR was merged.

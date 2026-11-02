Goal-Task: account-pinning/TASK-005

## Summary

Make keeper quota reads use the goal's shunt account subset when the current user's Keychain contains `shunt-goal-<slug>`. The lookup checks item metadata without reading the token. Both initial quota recovery and deadline rechecks pass `--client <slug>`. Goals without the item keep the global query. If an older shunt rejects the flag, retry globally and log one warning per keeper process. Update the existing keeper and `goal pool` documentation.

Verified revision: `b5102cc53b99510be6cca0c347ab2a36ed88f9b6` on `goal/account-pinning/task-005`, rebased without conflicts onto `origin/main` at `e811889503bcb43c4b449371ce0edd6adc214567` after PR #35. DEC-001's task-only scope is preserved: only `tools/goal-rust/README.md`, `goal-core/src/pool.rs`, `goal-core/src/session.rs`, and `goal/tests/session.rs` differ. No added files, dependencies, task specs, decisions, or goal files. No proposed follow-ups.

## Acceptance

- [x] Tests show `shunt pool --json --client <slug>` for a goal with a token and `shunt pool --json` otherwise. In this session's full workspace run, `keeper_pool_uses_the_goals_client_only_when_its_token_exists` passed. Its fake `SHUNT_BIN` records and asserts both exact argument forms. It also verifies a second scoped read after a keeper restart with an overdue deadline. `keeper_pool_falls_back_for_an_old_shunt_and_warns_once` passed, checking two rejected scoped queries, their global retries, and exactly one warning.
- [x] A fake pool where only goal account C is exhausted gives `exhausted == true` and C's reset. The same passing scoped-account test supplies healthy A/B in the global snapshot and only exhausted C in the goal snapshot. It directly asserts `goal_core::pool::exhausted` and `reset_time`, then checks the keeper deadline equals C's reset plus 300 seconds on both initial recovery and the deadline recheck. C's fixture reset is one hour ahead of the test clock.
- [x] The goal repository's full test suite and lint pass. At the verified revision in this session, the exact required Verify command exited **0**: workspace formatting passed, Clippy passed with warnings denied, and **671 tests passed, 0 failed**, across 74 test results. `RUST_TEST_THREADS=2` follows the repository's documented test concurrency.

## Verification

Run from the repository root, with `RUST_TEST_THREADS=2`:

```sh
cd tools/goal-rust && cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace
```

Keeper acceptance tests from that run:

```text
test keeper_pool_falls_back_for_an_old_shunt_and_warns_once ... ok
test keeper_pool_uses_the_goals_client_only_when_its_token_exists ... ok
```

Tail of the required Verify run:

```text
   Doc-tests goal_tui

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

Verify exit: 0
```

`git diff --check` passed. The worktree is clean. An independent read-only review of the rebased revision found no Critical, Important, or Minor issues. GitHub had no review comments or unresolved review threads when checked. GitHub Actions were not read or used as a gate.

Live Keychain and gateway access were not tested; executable fixtures cover the process boundary without real credentials. Older shunt versions use global capacity reads until upgraded.

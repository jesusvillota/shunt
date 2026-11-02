## Summary

Give each goal's Claude, Codex and AGY sessions its Keychain shunt token. Headless, tmux, Orca, Herdr, print and interactive orchestrator launches resolve `shunt-goal-<slug>` for account `$USER` at runtime. Stored commands, run records and clipboard text contain the lookup, never the token. Missing items clear inherited goal credentials while preserving unrelated Anthropic headers.

Private Codex worker configs add the shunt environment-header mapping without changing the owner's config. AGY discovery uses the selected goal token, then Claude's `apiKeyHelper`, then `local`. Discovery passes credentials through curl standard input. The existing README documents setup, including the mapping needed for interactive Codex sessions.

Launcher fixtures publish readiness after writing their credential captures. This prevents the missing-file race found in the prior Verify log. Session fixtures isolate OpenCode discovery. Workspace formatting satisfies the required format check.

Goal-Task: account-pinning/TASK-004

## Acceptance

- [x] Each launcher's command references the Keychain lookup for `shunt-goal-<slug>` and contains no token literal. In this session, `goal_shunt_tokens_are_resolved_by_every_launcher_without_storing_secrets` passed for headless, tmux, Orca, Herdr and print across Claude, Codex and AGY. `goal_shunt_orchestrator_start_script_resolves_token_at_runtime` passed across those platforms. `goal_worker_commands_defer_keychain_lookup_for_each_platform` also passed. The tests execute commands and inspect stored commands, scripts and run records.
- [x] Without a Keychain item, no `x-shunt-token` header or `SHUNT_GOAL_TOKEN` is set. `goal_shunt_missing_keychain_item_clears_inherited_pinning`, the orchestrator absent-item cases, and `missing_keychain_item_clears_shunt_headers_with_whitespace` passed in this session. They also check inherited-token cleanup and preservation of unrelated headers.
- [x] AGY discovery sends the goal token when present. `agy_discovery_authenticates_with_goal_then_helper_then_local` passed in this session. A local HTTP listener checks the received headers and verifies goal-token, helper-token and `local` precedence.
- [x] The full test suite and lint pass. The exact required Verify command and the README command below both exited 0 in this session. Each workspace run passed 669 tests with zero failures. Formatting and Clippy with warnings denied passed.

## Verification

Verified and pushed revision: `1e38c0dbc4b7dd1556cbf56d00ef3d8f48740381` on `goal/account-pinning/task-004`. The branch is up to date with its `origin/main` base, with no conflicts. No task specifications or goal files changed.

Required Verify, from the repository root, with `RUST_TEST_THREADS=2`:

```sh
cd tools/goal-rust && cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace
```

Exit 0. Workspace tests: 669 passed; 0 failed. Tail:

```text
   Doc-tests goal_tui

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

README command, from the repository root:

```sh
cargo test --locked --manifest-path tools/goal-rust/Cargo.toml --workspace -- --test-threads=2
```

Exit 0. Workspace tests: 669 passed; 0 failed. Both runs completed in tracked tool session 63945. Local logs: `/tmp/account-pinning-task004-final-verify.log` and `/tmp/account-pinning-task004-final-readme.log`.

Independent code review found no blockers. GitHub has no review comments or unresolved review threads. `git diff --check` passed. The only added files are the shared authentication module and its integration tests.

Live Keychain and live shunt authentication were not tested. Tests use executable stubs and a local HTTP listener.

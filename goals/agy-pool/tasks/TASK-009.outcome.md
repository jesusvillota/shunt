# TASK-009 outcome

Status: blocked. The code change and live pool checks are complete, but required workspace checks fail.

## Acceptance

- [x] With the installed `shunt` pool command from TASK-007 and the gateway running, `goal pool` prints per-family weekly quota for every Antigravity account.
  Evidence: this session's config worktree debug `goal` exited 0. All five accounts show Gemini and Claude + GPT weekly buckets, remaining percentage and reset timestamp. Full stdout is pasted below. The installed `~/.local/bin/shunt` launcher uses `~/GitHub/shunt/target/release/shunt`; no gateway configuration or credentials were edited.
- [x] `goal pool --json` still parses for the orchestrator.
  Evidence: the one-line jq command below printed `true` and exited 0. A stronger captured-snapshot check also confirmed exactly five Antigravity accounts, two weekly buckets each, numeric remaining values and string reset times. The JSON preservation unit test covers unknown fields and high-precision numbers.
- [ ] The complete goal-rust tests pass.
  Evidence: `cargo test` in the isolated config worktree exited 101. An initial run inherited `GOAL_AGENT` and failed an authority fixture. A clean-environment run removed session markers and reached `execution_differential`: 17 passed, 22 failed, including dispatch fixtures reporting `unknown codex model 'gpt-6.1-sol'`. Its tail is pasted below. All six focused pool tests pass. No failing tests were weakened or removed.
- [x] The config PR URL is in this outcome, and it changes no unrelated file.
  Evidence: https://github.com/jesusvillota/config/pull/106. `git diff origin/main..HEAD --name-only` and GitHub's PR file list both contain only `tools/goal-rust/goal-core/src/pool.rs` and `tools/goal-rust/README.md`.

## Summary

`fetch()` now invokes `shunt pool --json`, resolving `shunt` from PATH or `SHUNT_BIN`. Shunt owns token and gateway URL resolution. Errors remain one line for missing shunt, unreachable gateway and refused token; live CLI smoke checks confirm all three exit non-zero.

Text rendering reads Antigravity `quota_buckets` and shows each label, remaining percentage and reset time. Exact-output tests preserve Anthropic and Codex text. The raw JSON snapshot stays a `serde_json::Value` with all fields retained. The retired token-parser assertions move out with the retired token parser; the existing rendering assertions are strengthened to exact output and fetch behavior has focused coverage.

The config worktree is `/Users/jesusvillotamiranda/orca/workspaces/config/goal-agy-pool-task-009`, branched from `origin/main` at `b7f14a22903a5fc4fb69e5a8ebf26e9d64132d73`. Verified code revision: `cf9f5a7c6acb35e10bc9f8b89574ee76c67a8586`. The shared checkout's uncommitted files were untouched. The shunt branch remains `goal/agy-pool/task-009` and changes only this outcome.

Updated the goal CLI README because this changes its runtime dependency and text behavior. Considered shunt README, docs, site and generated wiki: none need edits because shunt behavior is unchanged. No scope expansion. Proposed follow-up: repair baseline formatting, clippy and fixture failures separately, then rebase and rerun this task's exact Verify command. Both PRs remain draft; neither is merged.

## Required verification

Executed in the config worktree's `tools/goal-rust`:

```sh
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

Exit 1 at formatting; subsequent commands in this chain did not run. Baseline formatting already failed before edits, with 2,133 diff blocks across unrelated source files. The changed `pool.rs` passes `rustfmt --edition 2024 --check`.

Executed strict clippy independently before and after edits. Both runs exited 101 with the same counts: 77 goal-core lib errors and 80 lib-test errors in unrelated files. The post-change log contains no pool.rs diagnostic.

```text
error: could not compile `goal-core` (lib) due to 77 previous errors
warning: build failed, waiting for other jobs to finish...
error: could not compile `goal-core` (lib test) due to 80 previous errors
```

Executed the full tests independently in an isolated caller environment:

```sh
env -u GOAL_AGENT -u GOAL_PLAN_ROOT -u CODEX_CI -u CLAUDECODE -u OPENCODE cargo test
```

```text
    startup_timeouts_close_adapter_and_remove_new_checkout
    stopped_herdr_server_prevents_coordinator_claim
    successful_setup_and_live_capacity_match_before_new_checkout

test result: FAILED. 17 passed; 22 failed; 0 ignored; 0 measured; 0 filtered out; finished in 142.07s

error: test failed, to rerun pass `-p goal --test execution_differential`
```

Focused verification:

```sh
cargo test -p goal-core pool::tests
rustfmt --edition 2024 --check goal-core/src/pool.rs
```

```text
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 63 filtered out; finished in 2.40s
```

Logs: `/tmp/agy-task009-baseline-fmt.log`, `/tmp/agy-task009-baseline-clippy.log`, `/tmp/agy-task009-verify.log`, `/tmp/agy-task009-clippy.log`, `/tmp/agy-task009-tests-clean.log`, `/tmp/agy-task009-pool-tests.log`. Baseline formatting/clippy and fixture-model bugs were recorded with `agent-note`; the existing worker-marker note was reused.

## Live pool evidence

Put the config worktree's `tools/goal-rust/target/debug` first on PATH for these commands. This tests the PR code without changing the shared installed `goal` wrapper.

```sh
goal pool
```

```text
anthropic (claude_oauth)
  cemfi          ok                     5h 0.17 (reset 1791984600) · 7d 0.32 (reset 1792162800) · cooldown —
  computing-jvm  near-quota,unavailable 5h 0.02 (reset 1791984600) · 7d 1.0 (reset 1792206000) · cooldown —
  personal       ok                     5h 0.05 (reset 1791970799) · 7d 0.81 (reset 1792281599) · cooldown —
antigravity (antigravity_oauth)
  computingjvm   ok                     Gemini Models · weekly: 0% left (reset 2026-10-20T17:55:30Z) · Claude + GPT Models · weekly: 100% left (reset 2026-10-21T00:23:37Z) · cooldown —
  keepuntouser   ok                     Gemini Models · weekly: 100% left (reset 2026-10-21T00:23:42Z) · Claude + GPT Models · weekly: 100% left (reset 2026-10-21T00:23:44Z) · cooldown —
  tommyshelby    ok                     Gemini Models · weekly: 0% left (reset 2026-10-20T12:41:36Z) · Claude + GPT Models · weekly: 0% left (reset 2026-10-14T23:03:28Z) · cooldown —
  personal       ok                     Gemini Models · weekly: 0% left (reset 2026-10-20T17:09:34Z) · Claude + GPT Models · weekly: 85% left (reset 2026-10-20T23:53:16Z) · cooldown —
  cemfi          ok                     Gemini Models · weekly: 100% left (reset 2026-10-21T00:24:05Z) · Claude + GPT Models · weekly: 100% left (reset 2026-10-21T00:24:06Z) · cooldown —
codex (chatgpt_oauth)
  personal       ok                     5h 0.14 (reset 1791985228) · 7d 0.49 (reset 1792484481) · cooldown —
  cemfi          ok                     5h 0.09 (reset 1791986188) · 7d 0.43 (reset 1792484524) · cooldown —
```

Exit 0. No scratch gateway or synthetic quota response was used.

```sh
goal pool --json | jq -e '(.providers | type == "array") and all(.providers[]; (.accounts | type == "array"))'
```

```text
true
```

Exit 0. Live failures through `SHUNT_BIN` test wrappers also produced exactly one line and exit 1:

```text
unreachable: error: Error: gateway unreachable
refused: error: Error: admin token rejected (401)
missing: error: shunt missing: install shunt with the pool command or set SHUNT_BIN
```

## Decision request

Should a separate task repair the goal-rust baseline checks before TASK-009 delivery?

1. Repair formatting, strict clippy and failing model fixtures in a separate task, then resume TASK-009. This preserves the task's file scope and all required acceptance gates, but delays delivery.
2. Expand TASK-009 to include workspace cleanup. This may unblock the gates but changes many unrelated files and conflicts with the approved narrow scope.
3. Change acceptance to permit scoped checks while baseline failures remain. This delivers sooner but explicitly relaxes the user's current requirements; it needs owner approval.

### Recommendation

Choose option 1. Keep both PRs draft until a separate baseline repair lands. Rebase the config change, run the exact Verify chain and live pool commands, then mark ready only if every requirement passes.

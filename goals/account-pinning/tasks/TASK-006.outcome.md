Goal-Task: account-pinning/TASK-006

## Summary

Add a Codex account-pinning gate in `tests/client_account_pinning.rs`. Two goals pin opposite accounts from a two-account ChatGPT pool. Both `/v1/messages` with outbound `chatgpt_oauth` and inbound `/responses` must send the exact pinned bearer credential and ChatGPT account ID upstream. Each path makes six requests per goal and checks response headers and content.

Verified revision: `ee15f791d9e53df92f2b04feb1288f34dbdd8f95`, rebased onto `origin/main` without conflicts and pushed on `goal/account-pinning/task-006`. Only the test file changes. No deviations or follow-up defects were found. README, engineering docs, site translations, and generated wiki were considered; this test-only change adds no public behavior or configuration.

## Acceptance

- [x] **Codex test passes for both paths.** In this resume session, `cargo test --all-features --test client_account_pinning -- --nocapture` exited 0: **5 passed, 0 failed**. The new case checked all 24 upstream requests against the exact pinned bearer and `chatgpt-account-id`. Each endpoint served six requests with b for goal-x and a for goal-y. Its output appears below.
- [x] **Required smoke-test account sets and table.** In this session, the repository `/run-shunt` driver exited 0 with all smoke checks passed. The separate three-account gateway then served **24 curl requests, all HTTP 200**, using the TASK-001 subsets: **goal-x only A/B, goal-y only C, goal-z only A/C, goal-w A, B and C**. Every `x-shunt-account` matched the exact credential received by the local mock. The table below records all six requests per goal.
- [x] **All required quality checks pass on the PR head.** In this session, the complete four-command Verify block exited 0 on `ee15f791d9e53df92f2b04feb1288f34dbdd8f95`. Format and clippy passed without warnings. `cargo test --all-features --workspace`: **3899 passed, 0 failed, 5 ignored across 49 test results**. The command block and actual tail appear below.

## Validation

The following commands ran in order with `CARGO_TARGET_DIR` set to the existing local build cache. The complete block exited 0. GitHub Actions were not read or used as a delivery gate.

```sh
cargo test --all-features --test client_account_pinning -- --nocapture
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features --workspace
```

Codex output:

```text
/v1/messages goal-x: all 6 upstream requests used account b's credential
/v1/messages goal-y: all 6 upstream requests used account a's credential
/responses goal-x: all 6 upstream requests used account b's credential
/responses goal-y: all 6 upstream requests used account a's credential
test codex_pinning_uses_the_selected_credential_on_both_paths ... ok
test result: ok. 5 passed; 0 failed; 0 ignored
```

Actual Verify tail:

```text
cargo fmt --all --check
format check: exit 0
cargo clippy --all-targets --all-features -- -D warnings
    Checking pyo3 v0.28.3
    Checking prefill-router v0.3.0 (https://github.com/NVIDIA-NeMo/Switchyard?rev=3ddea9d30174ad835cf505a93617ba251c8eb8dd#3ddea9d3)
    Checking shunt-gateway v0.51.1 (/Users/jesusvillotamiranda/orca/workspaces/shunt/goal-account-pinning-task-006)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 30.94s
cargo test --all-features --workspace

running 1 test
test polls_pauses_resumes_and_starts_provisioning ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

   Doc-tests shunt

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

Verify block: exit 0
```

An independent read-only code review found no Critical, Important, or Minor issues. The PR had no review threads or review comments at inspection. The worktree is clean. The five ignored tests retain their existing performance/live-service requirements. The optional live prefill-router checkpoint was not configured. Live provider authentication is outside this mock-based gate.

## Smoke test

Used [the repository `/run-shunt` skill](https://github.com/jesusvillota/shunt/blob/ee15f791d9e53df92f2b04feb1288f34dbdd8f95/.claude/skills/run-shunt/SKILL.md): ran `SHUNT_PORT=0 MOCK_PORT=0 .claude/skills/run-shunt/smoke.sh` and its manual launch-and-curl path with a temporary config, `[server.auth]`, four fake client tokens, three fake token-env Claude accounts, and the TASK-001 subsets. Both runs passed in this resume session.

The per-goal gateway used PID 12979 on port 64052; its local mock used port 64002. The harness stopped the gateway and mock and removed its temporary config and pool state. Account names are lowercase in config and shown as A/B/C here. No real provider quota was used. A before/after check confirmed that the owner's `~/.config/shunt/shunt.toml` bytes and port 3001 listener PIDs were unchanged.

| Goal | Request | Account (`x-shunt-account`) |
| --- | ---: | --- |
| goal-x | 1 | A |
| goal-x | 2 | B |
| goal-x | 3 | A |
| goal-x | 4 | A |
| goal-x | 5 | B |
| goal-x | 6 | A |
| goal-y | 1 | C |
| goal-y | 2 | C |
| goal-y | 3 | C |
| goal-y | 4 | C |
| goal-y | 5 | C |
| goal-y | 6 | C |
| goal-z | 1 | A |
| goal-z | 2 | C |
| goal-z | 3 | C |
| goal-z | 4 | A |
| goal-z | 5 | C |
| goal-z | 6 | C |
| goal-w | 1 | A |
| goal-w | 2 | B |
| goal-w | 3 | C |
| goal-w | 4 | A |
| goal-w | 5 | B |
| goal-w | 6 | C |

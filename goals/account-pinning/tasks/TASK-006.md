---
id: TASK-006
title: 'Gate: Codex end-to-end test and run-shunt smoke test per goal'
milestone: M-1
state: approved
depends_on: [TASK-001, TASK-002, TASK-003]
delivery: pr
kind: gate
repo: shunt
route: {platform: codex, model: gpt-6.1-sol, effort: xhigh}
---

## Why

Serves SC-1, SC-4 and SC-6. A check from a different model family than the tasks that built the feature, before the owner turns it on.

## Scope

1. Add a Codex case to `tests/client_account_pinning.rs` (or `tests/codex_multi_account.rs`): two ChatGPT accounts, a goal pinned to one of them, through both the outbound `chatgpt_oauth` path and the inbound Codex endpoint. Assert which credential the mock upstream received.
2. Run a real smoke test with the `/run-shunt` skill: a separate shunt on a free port with a temporary config. Three token-env Claude accounts point at a local mock upstream (no real quota). `[server.auth]` with tokens goal-x, goal-y, goal-z, goal-w and the TASK-001 subsets. Do not touch the owner's gateway on `127.0.0.1:3001` or `~/.config/shunt/shunt.toml`. Send 6 requests per goal with `curl` and print the `x-shunt-account` header of each response. Stop only the processes you started.
3. Put the smoke-test table in the PR body (goal, request number, account). Fix nothing outside the test file; report any defect as a finding.

## Acceptance

- The Codex test passes for both paths.
- The PR body shows the smoke-test table: goal-x only A/B, goal-y only C, goal-z only A/C, goal-w A, B and C.
- `cargo fmt --all --check`, `cargo clippy --all-targets --all-features -- -D warnings` and `cargo test --all-features --workspace` pass on the PR head.

## Verify

```sh
cargo test --all-features --test client_account_pinning -- --nocapture
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features --workspace
```

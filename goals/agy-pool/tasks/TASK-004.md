---
id: TASK-004
title: Quota-aware account selection and live end-to-end smoke
milestone: M-1
state: approved
depends_on: [TASK-002, TASK-003, TASK-005]
delivery: pr
kind: gate
---

## Why

Prove the whole pool works live and prefer accounts with the most quota left (SC-1, SC-2).

## Scope

Use the quota readings from TASK-002 to rank antigravity accounts (most remaining fraction in the requested model family first, then earliest reset), behind the existing pool selection. Then run a live smoke through the local gateway: send one request per model family repeatedly and show distribution across the 6 accounts. Out of scope: other providers.

## Acceptance

- A test shows an account with lower remaining fraction is chosen after one with higher, per model family.
- Live smoke output shows requests served by at least 3 distinct accounts and per-account quota in `goal pool`.
- fmt, clippy and full tests pass.

## Verify

`cargo fmt --all --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all-features --workspace`\nLive smoke: start `shunt run`, send requests, paste `goal pool` before and after.

---
id: TASK-003
title: Make the agy pool rotate and fail over across all accounts
milestone: M-1
state: approved
depends_on: [TASK-001, DEC-003]
delivery: pr
---

## Why

The pool must actually work across all accounts (SC-2).

## Scope

Fix the defects TASK-001 found in antigravity account selection, rotation and failover so requests use the shared pool of all configured accounts, in `src/accounts.rs`, `src/proxy/failover.rs`, `src/auth/antigravity/`. Account for the two shared model-family pools (Gemini vs Claude+GPT): exhausting one pool must not mark the account dead for the other. Add integration tests with a mock upstream. Update affected docs and locale copies. Out of scope: reading quota (TASK-002) and quota-weighted ranking (TASK-004).

## Acceptance

- An integration test sends N requests with 3 mock accounts and shows each account is used.
- A test shows a 429 on one account fails over to the next and the exhausted account is skipped until reset.
- A test shows exhausting the Gemini pool still allows Claude+GPT model requests on the same account.
- fmt, clippy and full tests pass.

## Verify

`cargo fmt --all --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all-features --workspace`

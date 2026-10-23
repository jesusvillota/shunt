---
id: TASK-002
title: Define the driver protocol and a fake driver
milestone: M-1
state: approved
depends_on: [DEC-002]
delivery: pr
---

## Why

Contract first, so the driver (TASK-003) and the adapter (TASK-004) can be built in parallel. Serves SC-1, SC-2.

## Scope

Per DEC-002. Write the driver protocol (document in `docs/chatgptweb-driver-protocol.md`): the request (prompt text, account/profile, model id, timeout), the streamed events (text delta, done, error with a machine-readable code such as `not_chat_mode`, `logged_out`, `ui_drift`, `rate_limited`, `timeout`) and the health call. Implement a **fake driver** that follows the protocol and replays fixture answers, so the adapter and tests need no browser or account. Include the shunt-side Rust trait or client for the protocol. Out of scope: the real browser code (TASK-003) and the Responses translation (TASK-004).

## Acceptance

- `docs/chatgptweb-driver-protocol.md` defines every request, event and error code, with one example for each.
- A fake driver passes a protocol conformance test that also runs against the real driver later (the test is parameterized over the driver).
- `cargo test` for the new module passes with no network, no browser and no account.

## Verify

`cargo test chatgptweb` (fake driver and conformance test). `cargo clippy --all-targets -- -D warnings` on the touched crate. Paste the output in the PR.

---
id: TASK-005
title: Fix macOS streaming test failures
milestone: M-1
state: approved
depends_on: []
delivery: pr
---

## Why

TASK-002 and TASK-003 cannot pass the full Verify run on macOS because of pre-existing streaming test failures (DEC-003, option 1). Serves SC-1, SC-2 by unblocking their delivery.

## Scope

Diagnose and fix the root cause of these local macOS failures, keeping every protocol assertion intact; do not raise timeouts without diagnosing:

- `tests/responses_chain_stream.rs` (about lines 172, 655, 699): `refused_port_is_deterministically_refused` sees TimedOut instead of ConnectionRefused; both `a_body_error_*` tests receive only an error decoding response body event instead of the expected SSE frames.
- `tests/antigravity_process.rs:324`: `streaming_finishes_when_a_descendant_holds_stdout_open` times out in the full suite (passes alone), likely a child/descendant cleanup or deadline race under load.

Bug notes: `~/GitHub/config/agent-inbox/bugs/` (search "streaming"). Out of scope: the account pool, quota, and any other test.

## Acceptance

- `cargo test --all-features --workspace` passes on macOS, run twice in a row.
- No existing test is weakened or removed; the fix is in the fixtures or the code under test.
- The PR states the root cause of each failure.

## Verify

`cargo fmt --all --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all-features --workspace`

# TASK-002 outcome

Status: delivered. The required local Verify commands pass after rebasing onto
the separate stage-store lint fix selected by DEC-003.

## Acceptance

- [x] `docs/chatgptweb-driver-protocol.md` defines every request, event, and
  error code with an example: two requests, four event types, and nine codes.
  In this resume session, `chatgptweb_documented_json_examples_match_the_wire_types`
  passed. It parses every JSON example, checks exact JSON round trips, and
  verifies coverage of every request type, event type, and error code.
- [x] The fake driver passes a conformance test parameterized over the driver.
  `tests/common/chatgptweb_conformance.rs::assert_case<D: Driver + ?Sized>`
  has no dependency on fake-driver types. TASK-003 can use the same runner
  with a real process client and controlled page states. All seven integration
  tests passed again in this session. They cover success, health, all codes,
  partial failures, uncertain submission, repeated requests, and account/profile
  isolation. The negative test rejects ten malformed streams.
- [x] The new module's tests pass without network, browser, or account.
  In this session, `cargo test chatgptweb` exited 0, with seven new tests passing.
  `CARGO_NET_OFFLINE=true cargo test --test chatgptweb` also exited 0:
  `7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.
  The fake uses memory only. It opens no profile, process, socket, or credential.
- [x] Required delivery Verify: `cargo clippy --all-targets -- -D warnings`
  exited 0 in this session. `cargo fmt --all --check` and `git diff --check`
  also exited 0.

## Summary

Added a versioned JSON Lines contract, typed Rust `Driver` trait, in-memory
fixture driver, shared conformance runner, and seven integration tests.
DEC-002's `chatgptweb_browser` name and helper-process boundary are preserved.
The contract specifies Chat-mode checks before every Send, account/profile
binding, streamed text, terminal events, deadlines, cancellation, and submission
evidence to prevent replay after an uncertain send.

Verified revision in this resume session:
`5cf91c6d1c2983c89145c7eb672a0df7ed574a4e`.
The branch was rebased without conflicts onto `origin/main` at
`f6bc00b9`, which includes the separate issue #52 fix from PR #53
(`cbd4e287`). The implementation, protocol document, and tests are unchanged
from the previously tested implementation at `23c79320`; their diff is empty.
This outcome update records current evidence only.

DEC-003 selected a separate fix for the earlier lint failure. That fix is now
on the base branch, and the required default-feature lint passes. The prior
decision request is resolved. No warning was suppressed and no unrelated
source fix was added to TASK-002.

Local verification from this session:

```text
$ cargo test chatgptweb
test chatgptweb_wire_rejects_unknown_or_malformed_messages ... ok
test chatgptweb_fake_conforms_success_health_and_repeated_turns ... ok
test chatgptweb_documented_json_examples_match_the_wire_types ... ok
test chatgptweb_fake_conforms_partial_and_uncertain_failures ... ok
test chatgptweb_fake_conforms_pre_send_failures_for_generate_and_health ... ok
test chatgptweb_fake_conforms_request_validation_and_account_isolation ... ok
test chatgptweb_conformance_rejects_broken_driver_streams ... ok
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
exit 0

$ CARGO_NET_OFFLINE=true cargo test --test chatgptweb
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
exit 0

$ cargo clippy --all-targets -- -D warnings
    Checking shunt-gateway v0.51.1
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 29.55s
exit 0

$ cargo fmt --all --check
exit 0
$ git diff --check
exit 0
```

Broader all-feature checks were attempted with
`cargo clippy --all-targets --all-features -- -D warnings` and
`cargo test --all-features --workspace`. They cannot compile without the
optional dashboard build at `ui/dist` (both exited 101). This is outside the required TASK-002
Verify commands and does not affect the new module's default-feature tests.
The live embedded-Python router was not tested; this environment also lacks
its Python packages. GitHub Actions were not inspected or used as gates.

PR: <https://github.com/jesusvillota/shunt/pull/51>, base `main`, with
`Goal-Task: chatgptweb-provider/TASK-002` and label `goal:chatgptweb-provider`.
The PR had no review threads, submitted reviews, or issue comments when
checked during this session. The blocker markers are removed, and the PR is
marked ready after local Verify.

Documentation surfaces considered: the engineering contract is in `docs/`.
README and its translations, site and its translations, and milestone specs
need no changes because this module registers no provider or user-facing
capability. Generated `wiki/` was not edited. No task specs, other outcomes,
decisions, or `goal.yaml` were edited.

Limits and follow-ups: TASK-003 implements the real helper and controlled-page
cases. TASK-004 consumes the trait. Actual browser-mode safety, pipe framing,
deadlines, and process cancellation remain specified behavior for TASK-003;
the offline fake does not prove them. No real browser, provider registration,
Responses translation, public config key, or credential writeback changed.

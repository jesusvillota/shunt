# TASK-002 outcome

Status: blocked on a decision about the existing default-feature clippy failure.
The protocol and fake driver are implemented. Delivery is not complete.

## Acceptance

- [x] `docs/chatgptweb-driver-protocol.md` defines every request, event, and
  error code with an example. It covers two requests, four event types, and
  nine error codes. `chatgptweb_documented_json_examples_match_the_wire_types`
  parses all examples, checks exact JSON round trips, and checks that every
  request type, event type, and error code appears. It passed locally.
- [x] The fake driver passes a conformance test parameterized over the driver.
  `tests/common/chatgptweb_conformance.rs::assert_case<D: Driver + ?Sized>`
  has no dependency on fake-driver types. TASK-003 can reuse it with a real
  process client and controlled page states. Fake tests cover success, health,
  all error codes, partial failures, uncertain submission, repeated requests,
  and account/profile isolation. A negative test rejects ten malformed streams.
- [x] The new module's tests pass without network, browser, or account.
  `cargo test chatgptweb` exited 0, with all seven new tests passing.
  `CARGO_NET_OFFLINE=true cargo test --test chatgptweb` also exited 0:
  `7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.
  The fake uses only memory and does not open its fixture profile path.
- [ ] Delivery Verify: `cargo clippy --all-targets -- -D warnings` exited 101
  on the unchanged `src/routing/stage/store.rs:50` re-export. The PR remains
  draft because the required default-feature lint did not pass.

## Summary

Added a versioned JSON Lines contract, typed Rust `Driver` trait, in-memory
fixture driver, shared conformance runner, and seven integration tests.
DEC-002's `chatgptweb_browser` name and helper-process boundary are preserved.
The contract includes Chat-mode checks before every Send, account/profile
binding, streamed text, explicit terminal events, deadlines, cancellation,
and submission evidence to prevent replay after an uncertain send.
No real browser, provider registration, Responses translation, public config
keys, or credential writeback behavior changed.

Verified implementation commit: `bb267da65e448f4bf1745aa65a96c599db094469`.
Rebased without conflicts onto `origin/main` at
`8dce235c4de229c5e6ac09e89b3d4e262c1aafcd`, producing
`23c79320b3a8e8da6109dbd8255d1f6de90710e3`. The base update changes only the
goal dispatch ledger. An empty diff between these implementation commits
confirms that the tested Rust and protocol document are unchanged.
This outcome commit adds evidence only.

Local verification:

```text
$ cargo test chatgptweb
running 7 tests
test chatgptweb_fake_conforms_success_health_and_repeated_turns ... ok
test chatgptweb_wire_rejects_unknown_or_malformed_messages ... ok
test chatgptweb_documented_json_examples_match_the_wire_types ... ok
test chatgptweb_fake_conforms_partial_and_uncertain_failures ... ok
test chatgptweb_fake_conforms_pre_send_failures_for_generate_and_health ... ok
test chatgptweb_conformance_rejects_broken_driver_streams ... ok
test chatgptweb_fake_conforms_request_validation_and_account_isolation ... ok
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
exit 0

$ CARGO_NET_OFFLINE=true cargo test --test chatgptweb
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
exit 0

$ cargo clippy --all-targets -- -D warnings
error: unused import: `MAX_TRACKED_CHILD_PINS`
  --> src/routing/stage/store.rs:50:26
50 | pub(crate) use entries::{MAX_TRACKED_CHILD_PINS, MAX_TRACKED_SESSIONS};
   = note: `-D unused-imports` implied by `-D warnings`
error: could not compile `shunt-gateway` (lib test) due to 1 previous error
exit 101

$ cargo clippy --all-targets --features bench -- -D warnings
exit 0

$ cargo fmt --all --check
exit 0
$ git diff --check
exit 0
```

The additional bench-feature lint passes because that feature uses the
existing re-export. It does not replace the required default-feature command.
Full all-feature tests were not run; `ui/dist` is absent, and those tests also
include optional embedded-Python functionality outside this module.
GitHub Actions were not read, run, or used as gates.

Documentation surfaces considered: the new engineering contract is in `docs/`.
README and its translations, site and its translations, and milestone specs
need no changes because this module registers no provider or user-facing
capability. Generated `wiki/` was not edited. No task specs, other outcomes,
decisions, or `goal.yaml` were edited.

PR: <https://github.com/jesusvillota/shunt/pull/51>, base `main`, with
`Goal-Task: chatgptweb-provider/TASK-002` and label `goal:chatgptweb-provider`.
It was opened as a draft after the first implementation commit and push.
The unrelated lint bug is reported at
<https://github.com/jesusvillota/shunt/issues/52>; duplicate searches found none.
The existing repo rule says to report unrelated bugs without fixing them.
That rule prevents changing the stage store within the current task authority.

Follow-ups: TASK-003 implements the real helper and controlled-page cases.
TASK-004 consumes the trait. Browser-mode safety and process cancellation
are specified here but cannot be proved by this offline fake.

## Decision request

May TASK-002 include the minimal fix for the existing stage-store clippy error,
or must issue #52 be fixed separately before this task resumes?

1. Authorize a narrow scope exception. Split the existing cap re-export gates
   so `MAX_TRACKED_CHILD_PINS` is exported only for the bench feature. This
   clears the required Verify command sooner but changes code outside TASK-002.
2. Fix issue #52 separately, then resume TASK-002, rebase, and rerun Verify.
   This preserves task scope but keeps this PR blocked until that fix arrives.

### Recommendation

Choose option 2. Keep the unrelated stage-store fix separate, then resume this
task and rerun the required default-feature lint before marking the PR ready.
Do not waive the lint, suppress warnings, or treat the bench-feature pass as
equivalent to the required command.

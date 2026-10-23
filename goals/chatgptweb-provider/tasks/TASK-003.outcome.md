# TASK-003 outcome

Status: delivered.

## Acceptance

- [x] The real driver passes TASK-002's unchanged conformance runner.
  `npm test --prefix helpers/chatgptweb-browser` exited 0 in this session.
  Its 18 Node tests use real Chromium against local imitation pages.
  `tests/chatgptweb_browser.rs` implements a test-only process bridge to
  `tests/common/chatgptweb_conformance.rs::assert_case`, also used by the fake.
  That test passed with Chat, Work, logged-out, drifted markup, empty answers,
  health checks, and repeated turns. The fixture does not require a real account.
- [x] Work mode returns `not_chat_mode` with `not_sent` and zero sends.
  The Node fixture asserts the server's physical-send counter is zero for
  both generate and health. Missing, unknown, conflicting selection and a
  mode change during paste also fail closed. Shared conformance confirms
  the exact Work-mode error envelope from the real helper process.
- [x] The driver does not access cookies, storage or credentials.
  The passing Node test `production source has no cookie, storage, credential
  or network extraction access` scans all three production helper modules
  for the prohibited APIs and file/network extraction. Browser-managed
  persistence stays inside Chromium; the helper only inspects the rendered UI.
- [x] `helpers/chatgptweb-browser/README.md`, section `Start one account`,
  gives installation commands, the account-bound launch command, and the
  exact persistent profile location selected by `--profile-dir`.

## Summary

DEC-002's Node helper uses Playwright Chromium behind TASK-002's JSON Lines
protocol. It binds one account/profile pair, validates before page actions,
opens a fresh chat for each turn, pastes and verifies exact prompt text, and
checks selected Chat mode in the same page task as the Send click. It streams
append-only rendered answer text. Completion requires the turn's action bar
and absence of the stop control, without a locale-specific completion label.
Unknown model labels are rejected without substitution. There are no retries,
login automation, challenge bypasses, tools or adapter registration.

Tests found and fixed a shutdown race: concurrent close calls could return
before Chromium closed. EOF, SIGTERM and normal completion now share one
close promise. Missing composer markup also returns `ui_drift` after a bounded
render wait. Timeout closes the active page before the terminal error.

Verified implementation revision: `889929e3d3b38b8f25ba4aa5fadd94f302a6010e`.
This outcome is a later documentation-only commit. The branch remains
`goal/chatgptweb-provider/task-003`. It was rebased without conflicts onto
`origin/main` at `ab4afdd1` before implementation. No task specs, other task
outcomes, decisions or goal.yaml changed. No GitHub Actions were inspected.

Local checks, all exit 0:

- `npm test --prefix helpers/chatgptweb-browser`: 18 Node tests and one Rust
  shared-conformance test, no ignored fixture cases in this command.
- `cargo test chatgptweb`: seven fake/protocol integration tests plus existing
  matching unit tests. The browser test is intentionally ignored in ordinary
  Cargo runs; the helper command explicitly enables it.
- `cargo fmt --all --check` and `git diff --check`.
- `PYO3_PYTHON=/opt/homebrew/bin/python3.13 cargo clippy --all-targets --all-features -- -D warnings`.
- `PYO3_PYTHON=/opt/homebrew/bin/python3.13 cargo test --all-features --workspace`:
  3880 passed, 0 failed, 6 ignored across all test binaries.
  The optional UI was built locally first with `npm ci --prefix ui` and
  `npm run build --prefix ui`. Generated UI files were not committed.

Helper Verify output (tail):

```text
✔ uncertain: terminal and submission evidence (1643.407916ms)
✔ revision: terminal and submission evidence (391.735375ms)
✔ empty: terminal and submission evidence (221.165375ms)
✔ production source has no cookie, storage, credential or network extraction access (1.107958ms)
✔ CLI uses JSONL, keeps stdout clean and exits on malformed input (853.982792ms)
✔ CLI eof cancels an active generation (715.381917ms)
✔ CLI SIGTERM cancels an active generation (806.431667ms)
ℹ tests 18
ℹ suites 0
ℹ pass 18
ℹ fail 0
ℹ cancelled 0
ℹ skipped 0
ℹ todo 0
ℹ duration_ms 16533.98725
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.61s
     Running tests/chatgptweb_browser.rs (/Users/jesusvillotamiranda/orca/workspaces/shunt/goal-chatgptweb-provider-task-003/target/debug/deps/chatgptweb_browser-7d77f011455832e5)

running 1 test
test chatgptweb_browser_uses_shared_fake_conformance_with_local_page ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.71s
exit 0
```

PR: <https://github.com/jesusvillota/shunt/pull/54>, into `main`, carrying
`Goal-Task: chatgptweb-provider/TASK-003` and `goal:chatgptweb-provider`.

Documentation surfaces considered: the new helper README documents its setup;
`docs/chatgptweb-driver-protocol.md` links the helper and its tests. Root README
and maintained translations, site and maintained translations need no provider
setup change here because this task does not register a supported provider.
TASK-004 owns that surface. Generated wiki files were not edited.

Limits and follow-ups: fixtures imitate the page; they are not a captured
signed-in production page. Selected-mode selectors and completion evidence
must be checked live by TASK-006. This task makes no live turn or quota claim.
TASK-005 owns interactive profile login. TASK-004 owns the adapter and its
production process supervisor, including process-group termination of an
unresponsive helper. No new material decision or access blocker was needed.

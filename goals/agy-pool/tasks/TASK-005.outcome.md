# TASK-005 outcome

## Acceptance

- [x] `cargo test --all-features --workspace` passes on macOS twice in a row.
  On 2026-10-14, both consecutive full runs exited 0: 46 test targets,
  3,760 passed, zero failed, and two existing ignored measurement tests per run.
  Session logs: `/tmp/shunt-task005-final-test1.log` and
  `/tmp/shunt-task005-verify.log`. The second run executed the complete Verify
  command below. No test or source edit occurred between the two runs.
- [x] No existing test is weakened or removed; the fix is in the fixtures or
  the code under test. All SSE, status, token, error, and process-death
  assertions remain. All time limits remain. `tests/antigravity_process.rs`
  is unchanged. The refused-port test gains an exclusive-bind assertion.
  The only dependency change is Hyper 1.10.1 to 1.11.1 in `Cargo.lock`.
  Evidence: the diff against `origin/main`, both complete test runs, and
  50 additional successful runs of the unchanged Antigravity target
  (600 test passes, zero failures; `/tmp/shunt-task005-agy-fixed-1.log`
  through `/tmp/shunt-task005-agy-fixed-50.log`).
- [x] The PR states the root cause of each failure.
  PR <https://github.com/jesusvillota/shunt/pull/13> describes the refused
  connection, both truncated-body failures, and the Antigravity timeout.
  The diagnoses and upstream fix are also recorded in
  `docs/notes/macos-streaming-tests.md`.

## Summary

### What and how

- The refused-port fixture used a bound, non-listening socket. macOS drops
  SYNs to that socket, yielding `TimedOut`. The fixture now reserves an
  established connection's client-side port, which refuses new connections.
- Both raw truncated-body responders closed with unread request bytes.
  macOS sent TCP resets, which could discard SSE frames before Hyper exposed
  them. The fixtures now drain the complete request before sending the
  intentionally truncated response. Both original failure assertions
  reproduced before the fix and passed afterward.
- The Antigravity timeout came from Hyper's HTTP/1 write re-check, which
  could buffer terminal bytes and then yield without flushing them. The
  original test reproduced under load. Temporary tracing showed completed
  child cleanup, all 828 bytes at the server observer, and only 521 bytes
  at the client. Hyper 1.11.1 includes the exact upstream fix:
  <https://github.com/hyperium/hyper/pull/4143>.
  Session evidence: `/tmp/task005-antigravity-stress-4.log` and
  `/tmp/task005-agy-observer-1.log`. All temporary instrumentation was removed.

### Verification

Verified code revision: `dc9c4c877be4e1e1a50ae5e442a13082dc77cfb7`.
This outcome adds no executable changes.

```sh
cargo fmt --all --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all-features --workspace
```

Exit status: **0**. Final log tail:

```text
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

   Doc-tests shunt

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

The documented prerequisite UI bundle was built with `npm ci` and
`npm run build` in `ui/`. Generated UI files are untracked and not committed.
Verification ran on macOS Apple Silicon. Linux was not run. The existing
optional live prefill-router test retains its checkpoint-dependent skip.

### Scope and documentation

No account-pool, quota, public config, or credential behavior changes.
README and site surfaces were reviewed. They already describe the intended
behavior and require no change. The English engineering note records the
implementation fixes. Generated wiki files remain unchanged.

The branch was rebased onto `origin/main` without conflicts. The PR targets
`main`, carries `goal:agy-pool`, and contains `Goal-Task: agy-pool/TASK-005`.
GitHub reported an empty `statusCheckRollup`; no PR checks are configured.
The PR is never merged by this task.

### Deviations and follow-ups

The suspected child-cleanup race was an HTTP dependency defect. Updating
the dependency fixed the root cause without changing the Antigravity fixture.

An unrelated cold-tokenizer timing failure appeared under concurrent build
load and passed unchanged on rerun and both final full runs. It was recorded
through `agent-note` for separate investigation, with no out-of-scope fix:
`~/GitHub/config/agent-inbox/bugs/20261014T012255Z-75835-Cold-token-encoder-can-exceed-the-streaming-esti.md`.

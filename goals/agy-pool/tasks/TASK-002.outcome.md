## Acceptance

DEC-002 option A was accepted by the owner on 2026-10-14. It accepts only
upstream-reported windows and separate captured-weekly/four-window schema tests.
These accepted terms govern items 1 and 3 below; no live 5h data is invented.

- [x] **1. Parse the captured `/usage` reply and all four grouped bucket shapes.**
  Evidence from this session: `captured_weekly_reply_preserves_only_reported_windows`
  and `representative_four_window_schema_parses_all_grouped_buckets` passed in the
  all-feature library run. The agy 1.3.4 capture reports two weekly windows.
  The separate representative fixture verifies Gemini 5h/weekly and Claude + GPT
  5h/weekly, remaining fractions, reset time, and canonical order. It is explicitly
  not labeled a live capture. Capture accounting: `num_turns: 0`, all tokens zero.
- [x] **2. A model turn latches the probe off and spends no further calls.**
  Evidence: `model_turn_latches_probe_and_spends_no_further_calls_even_for_another_profile`
  passed. A fake CLI reports `num_turns: 1` with ERROR status and nonzero exit.
  The probe makes one version call and one usage call. Three subsequent probes,
  including another profile, make zero additional subprocess calls.
  `agy_primary_and_http_fallback_preserve_account_buckets` also passed: primary
  success makes no HTTP call; absent/failed profiles and the latch use HTTP.
- [x] **3. Live gateway quota for every configured Antigravity account.**
  Evidence: authenticated `GET http://127.0.0.1:39002/admin/api/pool` returned
  HTTP 200. All six configured accounts have `has_state: true` and both weekly
  grouped buckets with independent reset times. The log confirms agy `/usage`
  supplied personal; accounts without CLI profiles use HTTP fallback.
  No upstream reports 5h, so 5h buckets remain absent, as accepted in DEC-002 A.
  All six accounts have zero inference attempts. Exact observed values are below.
- [ ] **4. Format, all-feature clippy, and all-feature workspace tests pass.**
  `cargo fmt --all --check` passed. `cargo clippy --all-targets --all-features -- -D warnings`
  passed (exit 0). `cargo test --all-features --workspace` failed with exit 101. Library tests:
  3083 passed, 2 ignored; CLI unit tests: 43 passed. All new quota tests passed.
  `tests/responses_chain_stream.rs` failed in
  `refused_port_is_deterministically_refused` (TimedOut vs ConnectionRefused) and
  `a_body_error_before_message_stop_still_emits_the_terminal_error_event`
  (only the error-decoding-response-body event was received, without partial text).
  A second full run after rebase repeated both failures and also failed
  `a_body_error_after_message_stop_ends_the_relay_silently`. All three are already
  assigned to TASK-005. No test or assertion was weakened. Later integration
  targets were not reached because Cargo stops at the failed binary.

## Summary

Implemented accepted DEC-001 C and DEC-002 A. Optional account `profile_dir`
selects that account's existing agy HOME. Profiles are resolved to absolute paths;
missing profiles are never created and no ambient CLI login is used implicitly.
The child sets HOME/USERPROFILE, removes ambient Google/Gemini credentials,
closes stdin, requires agy >= 1.1.11, and runs the slash command without
`--disable-slash-commands`. Output is capped at 1 MiB with a 25-second deadline.

A single poller-owned safety latch persists across ticks and config reloads.
Any reported model turn disables all CLI quota probes until restart, including
error replies and nonzero subprocess exits. Existing HTTP fetch and credential
refresh/writeback semantics are retained. Static `token_env` accounts stay
excluded. Grouped quota stays display-only; account selection is TASK-003.

Verified source revision: `451c780d09ba151d98e5f40f15e37a42f0bc0a9b` (same implementation as pre-rebase
`ec9ee05`). Rebased cleanly onto `origin/main`; only goal metadata changed upstream.
Branch: `goal/agy-pool/task-002`. No other task, spec, decision, or goal.yaml edited.

English, Korean, Japanese, and Simplified Chinese README, configuration reference,
and provider docs were updated together. M17 and the quota observation note were
updated. `npm ci` and `npm run build` in `site/` passed; 209 pages built with MDX
validation. Existing UI assets satisfied all-feature build prerequisites.
Generated `wiki/` and local config files were not committed.

A scratch gateway used this branch's all-feature binary with polling at 60 seconds,
the existing six account names, and the personal account's explicitly mapped CLI
HOME. Production gateway config was not changed. No inference request was sent.
Normal HTTP refresh may write back credentials through its existing approved path.

The PR has no review threads or review comments. The latest queried check list
was empty. GitHub Actions were not inspected or run. PR remains open and draft because the required full local test suite fails. Never merged.

Proposed follow-ups: resume TASK-002 after TASK-005 lands and rerun all checks.
TASK-003 handles account selection; TASK-004 verifies the
combined pool behavior. No unrelated code fix is included.

## Live evidence

Session date: 2026-10-14. API HTTP 200, all accounts `has_state: true`, zero
inference attempts. Fractions below mean quota remaining, not quota consumed.

| Account | Source | Gemini weekly remaining | Gemini reset | Claude + GPT weekly remaining | Claude + GPT reset |
| --- | --- | --- | --- | --- | --- |
| computingjvm | HTTP fallback | 0.000000000 | 2026-10-20T17:55:30Z | 0.999908000 | 2026-10-21T00:23:37Z |
| keepuntouser | HTTP fallback | 0.999992800 | 2026-10-21T00:23:42Z | 0.999862000 | 2026-10-21T00:23:44Z |
| tommyshelby | HTTP fallback | 0.000000000 | 2026-10-20T12:41:36Z | 0.000000000 | 2026-10-14T23:03:28Z |
| computing-jvm | HTTP fallback | 0.000000000 | 2026-10-20T17:55:30Z | 0.999908000 | 2026-10-21T00:23:37Z |
| personal | agy /usage | 0.000000000 | 2026-10-20T17:09:34Z | 0.847411990 | 2026-10-20T23:53:16Z |
| cemfi | HTTP fallback | 0.999988000 | 2026-10-21T00:24:05Z | 0.999862000 | 2026-10-21T00:24:06Z |

No 5h bucket was present in any account's upstream reply. Generic
`utilization_5h`/`utilization_7d` remain null; native `quota_buckets` carry the
reported grouped windows. This avoids combining two independent model budgets.

## Blocked

Required passing macOS workspace-test evidence is unavailable until the existing
TASK-005 streaming failures are fixed. DEC-003 already assigns those fixes to a
separate task/PR; this worker must not edit that task's code or weaken assertions.
The implementation, captured/schema fixtures, latch, HTTP fallback, translated
docs, and live six-account smoke are complete and committed for review.
PR #11 stays draft with `Goal-Blocked: Required macOS workspace tests fail in
pre-existing responses_chain_stream tests owned by TASK-005.` No new decision
request is needed; DEC-001 and DEC-002 were implemented as accepted.

The existing duplicate bug note was found before recording:
`~/GitHub/config/agent-inbox/bugs/20261013T182822Z-76814-Streaming-chain-integration-tests-fail-on-local-.md`.
No duplicate note was created. The scratch gateway was stopped after the smoke.

## Full-suite failure evidence

First run: `cargo test --all-features --workspace`, exit 101.

```text
refused_port_is_deterministically_refused
  tests/responses_chain_stream.rs:181
  left: TimedOut
  right: ConnectionRefused

a_body_error_before_message_stop_still_emits_the_terminal_error_event
  tests/responses_chain_stream.rs:742
  event: error
  data: {"type":"error","error":{"type":"api_error","message":"error decoding response body"}}

test result: FAILED. 14 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out
```

Second full run after rebase, without competing builds: the same two tests plus
`a_body_error_after_message_stop_ends_the_relay_silently` failed. Both full runs
passed all eight `usage_cli` tests, the poller primary/fallback test, the config
profile test, and the Antigravity process integration tests.

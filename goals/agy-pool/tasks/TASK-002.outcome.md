## Acceptance

Session: 2026-10-14, resumed after TASK-005 merged. DEC-001 C and DEC-002 A
remain the accepted requirements. DEC-002 accepts only upstream-reported windows
and separate captured-weekly and representative four-window schema tests.

- [x] **1. Parse captured `/usage` JSON and all four grouped bucket shapes.**
  `captured_weekly_reply_preserves_only_reported_windows` and
  `representative_four_window_schema_parses_all_grouped_buckets` both passed
  in this session's full workspace run. The captured agy 1.3.4 fixture contains
  two weekly windows, `num_turns: 0`, and zero token usage. The separate schema
  fixture verifies Gemini 5h/weekly and Claude + GPT 5h/weekly, remaining
  fractions, reset time, and canonical order. It is not a live capture.
- [x] **2. `num_turns > 0` latches the probe off without further calls.**
  `model_turn_latches_probe_and_spends_no_further_calls_even_for_another_profile`
  passed. Its fake CLI returns `num_turns: 1`, ERROR status, and a nonzero exit.
  Exactly one version call and one usage call occur. Three subsequent probes,
  including another profile, make zero further subprocess calls.
  `agy_primary_and_http_fallback_preserve_account_buckets` also passed.
- [x] **3. Live gateway shows every account's upstream-reported quota.**
  Authenticated `GET http://127.0.0.1:39002/admin/api/pool` returned HTTP 200
  in this session. All six configured Antigravity accounts have `has_state: true`,
  both grouped weekly buckets, independent reset times, and zero inference
  attempts. Assertions checked the exact configured account set and all these
  fields. The gateway log confirms `agy /usage applied account=personal`.
  The other five accounts have no CLI profile and use HTTP fallback.
  Upstream reports no 5h windows; these remain absent under accepted DEC-002 A.
  Exact values are recorded below.
- [x] **4. Format, all-feature clippy, and all-feature workspace tests pass.**
  This exact Verify command completed with exit 0:
  `cargo fmt --all --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all-features --workspace`.
  Format passed. Clippy passed with warnings denied. The workspace run completed
  46 result groups, including doc-tests: 3,770 passed, zero failed, two ignored.
  Library tests: 3,083 passed, two ignored. CLI tests: 43 passed.
  All three former blockers in `tests/responses_chain_stream.rs` passed:
  `refused_port_is_deterministically_refused`,
  `a_body_error_before_message_stop_still_emits_the_terminal_error_event`, and
  `a_body_error_after_message_stop_ends_the_relay_silently`.

## Summary

Rebased `goal/agy-pool/task-002` cleanly onto `origin/main` at
`f6cedf9927d8aa4d70e6b794bbec628caba0701a`, including TASK-005's merged macOS
streaming-test fixes and Hyper lockfile update. Verified source revision:
`25b98802fc8c51a959888b79bbfb0984b8b3c64f`. This resumption changes only this
outcome and the PR metadata; no implementation change was needed.
The previous workspace-test block is resolved.

The implementation uses optional per-account `profile_dir` for agy `/usage`
with the same account's existing HTTP fallback. It never assumes an ambient
CLI login for an account without a profile. It requires agy >= 1.1.11,
closes stdin, removes ambient Google/Gemini credentials, caps output at 1 MiB,
and enforces a 25-second deadline. The poller-owned model-turn latch lasts
through polling ticks and config reloads until restart. HTTP credential
refresh/writeback behavior is unchanged. Static `token_env` accounts remain
excluded. Quota buckets remain display-only; selection belongs to TASK-003.

README, docs, and site documentation surfaces were reviewed. The existing
English, Korean, Japanese, and Simplified Chinese updates remain in this PR.
M17 and the quota observation note match the implementation. This resumption
adds no behavior requiring further docs changes. Generated `wiki/` is untouched.
No task spec, other task, decision, or `goal.yaml` was edited.

The live test used this worktree's freshly rebuilt all-feature gateway binary
and the existing scratch config `/tmp/agy-task002-live.toml`. Its six account
names match the operator's config; personal has an explicitly mapped agy HOME.
`target/debug/shunt check --config /tmp/agy-task002-live.toml` passed.
Polling ran at 60 seconds. Production gateway configuration was untouched.
No inference request was sent. The scratch gateway exited successfully after
SIGTERM. Existing HTTP refresh retains its normal credential writeback.

Session artifacts: `/tmp/shunt-task-002-resume-tests.log`,
`/tmp/shunt-task-002-resume-pool.json`, and `/tmp/shunt-task-002-resume-live.log`.
The test log records all parser/latch tests and the complete workspace results.

PR #11 has no review threads, inline review comments, or submitted reviews.
The queried PR check list is empty. GitHub Actions were not inspected or run.
The obsolete `Goal-Blocked` marker has been removed. The PR retains
`Goal-Task: agy-pool/TASK-002` and label `goal:agy-pool`. Full verification
passed; the PR is ready for review. Never merged. Follow-ups remain TASK-003 selection and TASK-004 combined smoke.

## Live evidence

Authenticated admin API HTTP 200 at 2026-10-14T01:38 UTC. Every account has
`has_state: true` and `requests_attempted: 0`. Fractions mean remaining quota.

| Account | Source | Gemini weekly remaining | Gemini reset | Claude + GPT weekly remaining | Claude + GPT reset |
| --- | --- | --- | --- | --- | --- |
| computingjvm | HTTP fallback | 0.000000000 | 2026-10-20T17:55:30Z | 0.999908000 | 2026-10-21T00:23:37Z |
| keepuntouser | HTTP fallback | 0.999992800 | 2026-10-21T00:23:42Z | 0.999862000 | 2026-10-21T00:23:44Z |
| tommyshelby | HTTP fallback | 0.000000000 | 2026-10-20T12:41:36Z | 0.000000000 | 2026-10-14T23:03:28Z |
| computing-jvm | HTTP fallback | 0.000000000 | 2026-10-20T17:55:30Z | 0.999908000 | 2026-10-21T00:23:37Z |
| personal | agy /usage | 0.000000000 | 2026-10-20T17:09:34Z | 0.847411990 | 2026-10-20T23:53:16Z |
| cemfi | HTTP fallback | 0.999988000 | 2026-10-21T00:24:05Z | 0.999862000 | 2026-10-21T00:24:06Z |

No upstream 5h bucket was present. Generic `utilization_5h`/`utilization_7d`
remain null; `quota_buckets` preserve the two independent model budgets.

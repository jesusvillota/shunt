## Acceptance

- [x] An integration test sends N requests with 3 mock accounts and shows each account is used.
  Evidence from this resumed session (exact Verify): `nine_requests_rotate_across_all_three_accounts` passed. Nine real gateway HTTP requests produce `a,b,c,a,b,c,a,b,c`; upstream bearer tokens and project IDs match each account.
- [x] A test shows a 429 on one account fails over to the next and the exhausted account is skipped until reset.
  Evidence from this resumed session (exact Verify): `exhausted_account_fails_over_is_skipped_and_returns_after_reset` passed. Account `a` returns 429 with `Resets in 2s.`; the gateway serves through `b`, skips `a` for two Gemini models, and uses `a` again after reset. The mock receives exactly four attempts before reset and six overall.
- [x] A test shows exhausting the Gemini pool still allows Claude+GPT model requests on the same account.
  Evidence from this resumed session (exact Verify): `gemini_exhaustion_leaves_claude_and_gpt_available_on_the_same_account` passed. A single account returns a Gemini weekly 429, then serves Claude and GPT with the same bearer. Another Gemini request receives 503 without upstream traffic. The reverse-direction test also passed: a Claude 429 excludes GPT while Gemini remains usable.
- [x] fmt, clippy and full tests pass.
  Evidence from the resumed session on 2026-10-14: the exact Verify command exited 0 after rebasing onto `origin/main` at `f6cedf9927d8aa4d70e6b794bbec628caba0701a`:

  ```sh
  cargo fmt --all --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all-features --workspace
  ```

  Format and clippy passed. The full workspace suite reported 3,767 passed, zero failed, and two existing ignored measurement tests across 47 successful test results. The library suite reported 3,076 passed and two ignored. All four new pool integration tests passed in this run, as did the six existing Antigravity account-pool tests. The three formerly failing macOS tests passed: `a_body_error_after_message_stop_ends_the_relay_silently`, `a_body_error_before_message_stop_still_emits_the_terminal_error_event`, and `refused_port_is_deterministically_refused`. The suite completed through doc tests. Session log: `/tmp/shunt-task003-resume-verify.log`.

## Summary

Added independent, memory-only Gemini and Claude+GPT cooldowns to the shared account pool. Selection excludes the exhausted family until reset; successful traffic in the other family cannot clear the refusal. The adapter uses `Retry-After`, then the observed JSON error message's reset duration, then 60 seconds. Known family cooldowns preserve weekly durations beyond the old one-hour cap. Unknown families retain a bounded per-model cooldown. Authentication and transport failures keep their existing account-wide handling.

Rotation among healthy equal-priority accounts already worked and now has explicit three-account integration coverage. Failover belongs to `src/adapters/gemini/mod.rs`, where the actual Antigravity HTTP status, headers, and body are available; `src/proxy/failover.rs` did not need a change. Added parser unit tests, a weekly cooldown/expiry unit test, and four mock-upstream integration tests. The six existing Antigravity pool tests also passed.

Documentation surfaces considered: updated `docs/m17-antigravity-usage.md` and the Antigravity provider page with its Korean, Japanese, and Chinese copies. The README provider/setup descriptions remain accurate and need no edit. `wiki/` is generated and was not edited. No public config keys, token files, login behavior, or credential writeback changed.

Scope boundaries: usage-source work remains in TASK-002 and quota-weighted ranking remains in TASK-004. Existing identity rules remain unchanged: optional userinfo email is not a verified upstream identity. Duplicate login cleanup remains an operator configuration follow-up; matching emails alone are insufficient evidence to merge physical account identities safely.

Verified implementation revision: `cf5e566`. Exact Verify ran on rebased branch revision `4f48d4a1d847b0197f26350f5002499c3954fba2`. The final outcome commit changes only this report. The rebase onto `origin/main` succeeded without conflicts and includes TASK-005's macOS streaming-test fixes (`22fbcdb`). No live Google account calls were needed; protocol tests use loopback mock upstreams. No learned-router checkpoint was supplied; its optional live test follows the normal skip behavior.

DEC-003 is accepted. The owner chose to fix the macOS streaming tests in a separate task, then resume TASK-003. TASK-005 is merged, and this session's exact Verify passed. The previous blocker and decision request are resolved and removed from the PR body. TASK-003 did not change the streaming tests or implementation.

PR: https://github.com/jesusvillota/shunt/pull/12 — base `main`, label `goal:agy-pool`, marker `Goal-Task: agy-pool/TASK-003`. GitHub reports no checks, comments, reviews, or review threads. This verified report is pushed before marking the open PR ready for review. Never merged.

Proposed follow-ups: TASK-004 can consult model-family usage snapshots for quota-based ranking. Remove duplicate operator logins if confirmed through upstream identity. Cooldowns clear on restart; durable quota metadata remains a separate concern.

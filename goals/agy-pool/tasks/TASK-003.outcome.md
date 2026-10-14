## Acceptance

- [x] An integration test sends N requests with 3 mock accounts and shows each account is used.
  Evidence: `nine_requests_rotate_across_all_three_accounts` passed. Nine real gateway HTTP requests produce `a,b,c,a,b,c,a,b,c`; upstream bearer tokens and project IDs match each account.
- [x] A test shows a 429 on one account fails over to the next and the exhausted account is skipped until reset.
  Evidence: `exhausted_account_fails_over_is_skipped_and_returns_after_reset` passed. Account `a` returns 429 with `Resets in 2s.`; the gateway serves through `b`, skips `a` for two Gemini models, and uses `a` again after reset. The mock receives exactly four attempts before reset and six overall.
- [x] A test shows exhausting the Gemini pool still allows Claude+GPT model requests on the same account.
  Evidence: `gemini_exhaustion_leaves_claude_and_gpt_available_on_the_same_account` passed. A single account returns a Gemini weekly 429, then serves Claude and GPT with the same bearer. Another Gemini request receives 503 without upstream traffic. The reverse-direction test also passed: a Claude 429 excludes GPT while Gemini remains usable.
- [ ] fmt, clippy and full tests pass.
  Evidence: `cargo fmt --all --check` and `cargo clippy --all-targets --all-features -- -D warnings` passed. The exact Verify command exited 101 during `cargo test --all-features --workspace`. The library suite passed (3,076 passed, 2 ignored), and all ten Antigravity integration tests passed. Three unchanged tests in `tests/responses_chain_stream.rs` failed: `a_body_error_after_message_stop_ends_the_relay_silently`, `a_body_error_before_message_stop_still_emits_the_terminal_error_event`, and `refused_port_is_deterministically_refused`. The full suite stopped there, so later integration binaries were not verified. Logs: `/tmp/shunt-task003-full-tests.log`, `/tmp/shunt-task003-protocol.log`, and `/tmp/shunt-task003-chain-recheck.log`.

  Exact Verify tail:
  ```text
  failures:
      a_body_error_after_message_stop_ends_the_relay_silently
      a_body_error_before_message_stop_still_emits_the_terminal_error_event
      refused_port_is_deterministically_refused

  test result: FAILED. 13 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 46.83s

  error: test failed, to rerun pass `--test responses_chain_stream`
  ```

## Summary

Added independent, memory-only Gemini and Claude+GPT cooldowns to the shared account pool. Selection excludes the exhausted family until reset; successful traffic in the other family cannot clear the refusal. The adapter uses `Retry-After`, then the observed JSON error message's reset duration, then 60 seconds. Known family cooldowns preserve weekly durations beyond the old one-hour cap. Unknown families retain a bounded per-model cooldown. Authentication and transport failures keep their existing account-wide handling.

Rotation among healthy equal-priority accounts already worked and now has explicit three-account integration coverage. Failover belongs to `src/adapters/gemini/mod.rs`, where the actual Antigravity HTTP status, headers, and body are available; `src/proxy/failover.rs` did not need a change. Added parser unit tests, a weekly cooldown/expiry unit test, and four mock-upstream integration tests. The six existing Antigravity pool tests also passed.

Documentation surfaces considered: updated `docs/m17-antigravity-usage.md` and the Antigravity provider page with its Korean, Japanese, and Chinese copies. The README provider/setup descriptions remain accurate and need no edit. `wiki/` is generated and was not edited. No public config keys, token files, login behavior, or credential writeback changed.

Scope boundaries: usage-source work remains in TASK-002 and quota-weighted ranking remains in TASK-004. Existing identity rules remain unchanged: optional userinfo email is not a verified upstream identity. Duplicate login cleanup remains an operator configuration follow-up; matching emails alone are insufficient evidence to merge physical account identities safely.

Verified implementation revision: `a2aeb83` (rebased onto `origin/main` without conflicts). The outcome commit adds only this report. No live Google account calls were needed; protocol tests use loopback mock upstreams. A real learned-router checkpoint was not supplied, so its optional live test follows the repository's normal skip behavior.

PR: https://github.com/jesusvillota/shunt/pull/12 — base `main`, label `goal:agy-pool`, marker `Goal-Task: agy-pool/TASK-003`. No remote checks are reported. The PR remains draft because local full verification failed. Never merged.

Proposed follow-ups: TASK-004 can consult model-family usage snapshots for quota-based ranking. Remove duplicate operator logins if confirmed through upstream identity. Cooldowns clear on restart; durable quota metadata remains a separate concern.

## Blocked

Goal-Blocked: decision needed for pre-existing macOS streaming-test failures

The three full-suite failures match the existing bug note
`~/GitHub/config/agent-inbox/bugs/20261013T182822Z-76814-Streaming-chain-integration-tests-fail-on-local-.md`, recorded on `main` before this task. No duplicate note was added. This task changes neither those tests nor the streaming-chain implementation.

The refused-port fixture returns `TimedOut` on this Mac instead of its asserted `ConnectionRefused`. The raw truncated-response fixtures deliver only `error decoding response body`, losing the expected SSE frames. Both body-error failures reproduced with `RUST_TEST_THREADS=1 cargo test --all-features --test responses_chain_stream`, so parallel test scheduling does not explain them. No Docker, Colima, Lima, or Podman executable is available locally for Linux verification.

The supplied AGENTS.md instructions say: "You find a real bug that is outside your task: do not fix it. Record it, then go on with your task." Fixing these independent streaming fixtures would expand TASK-003 beyond its approved account-pool scope. Tests were not weakened, removed, or skipped to claim success. Native goal completion was not called because acceptance item 4 is unmet.

## Decision request

Should TASK-003 include fixes for the known macOS streaming-test failures, or wait for a separate fix?

1. Keep TASK-003 focused and fix the streaming tests in a separate PR. This preserves task isolation and strict protocol assertions, but delivery waits for that fix and a new successful Verify run.
2. Authorize a narrow scope extension to fix the macOS fixtures in this PR. This can unblock local verification sooner, but mixes independent streaming-test work into the account-pool task and needs approval against the current boundary.
3. Provide an accessible Linux verification environment. This keeps source changes focused and may allow the exact Verify command to pass, but leaves the local macOS test defect unresolved and requires an available runtime.

### Recommendation

Use option 1. Resolve the already-recorded streaming-test bug separately, then resume TASK-003 and run the exact Verify command again. Preserve all existing protocol assertions. The account-pool implementation and its focused coverage are committed and ready for review, but this task is not delivered.

# TASK-012 outcome

## Acceptance

- [x] Gateway tests cover every required pool state.
  Evidence: `cargo test --test antigravity_pool_rotation` passed all 8 tests. The final all-features workspace run also passed all 8. `all_cooled_requests_return_earliest_reset_without_upstream_calls` sends second and later requests across Gemini models, checks HTTP 429, Anthropic `rate_limit_error`, the earliest reset (60 seconds versus 7200, adjusted for elapsed time), and unchanged total upstream calls. It also serves Claude and GPT while every account is Gemini-cooled. `all_paused_accounts_report_operator_action_without_upstream_calls` checks HTTP 503, `api_error`, the exact paused/disabled message, absent `Retry-After`, and no upstream calls, even with cached cooldowns. `cooled_and_paused_accounts_use_only_the_unpaused_reset` checks 429 and ignores a paused account's earlier reset. Existing family-isolation tests retain their strict account and attempt assertions; their obsolete 503 expectations now assert 429. The new account-pool unit test covers disabled and paused exclusions, unrelated/unknown families, and expired cooldowns.
- [x] Repeat TASK-010 case 5 on a scratch gateway against the real Google upstream.
  Evidence: `docs/notes/agy-pool-live-test.md` case 5 is PASS, rechecked 2026-10-14 at 10:02:26–10:02:29 UTC. Raw responses and before/after counters are committed under `docs/notes/agy-pool-live-test/task012-case5/`. First request: HTTP 429 in 3.144431s, `Retry-After` absent, body `{"type":"error","error":{"type":"rate_limit_error","message":"Individual quota reached. Please upgrade your subscription to increase your limits. Resets in 151h7m4s."}}`. Second request: HTTP 429 in 0.002427s, `Retry-After: 527947`, body `{"type":"error","error":{"type":"rate_limit_error","message":"Antigravity quota exhausted. Resets in 527947s."}}`. Third request: HTTP 429 in 0.002363s, identical header/body. The earliest reset was tommyshelby's 2026-10-20T12:41:36Z. All attempt/failure/success counters stayed unchanged on requests two and three. Paused accounts were keepuntouser and cemfi. Separate copied credentials, HOME, admin token, and port 39020; scratch PID 94158 was stopped. Production config SHA-256 and launchd PID 57240 stayed unchanged, proven by identical committed production snapshots.
- [x] Document the new behavior on all required surfaces.
  Evidence: the Antigravity provider pages in English, Korean, Japanese, and Simplified Chinese now describe 429, earliest reset seconds rounded up in `Retry-After`, local replies, family isolation, operator exclusions, mixed pools, and the all-paused/disabled 503. `docs/m17-antigravity-usage.md` describes the same behavior. README and its translations were considered; no setup, capability list, or README-described behavior changes. No wiki edit is needed because it is generated separately. No headings or links changed.
- [x] Run the exact required local verification command successfully.
  Evidence: `(cargo fmt --all --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all-features --workspace)` exited 0 on implementation revision `4eb1860c8955059fe2b5b7abe1531977d46bd623`, after rebasing onto `origin/main` without conflicts. Generated `ui/dist` with `npm ci && npm run build` for the all-features build. Full output is `/tmp/task012-final-verify.log` in this session. No checkpoint was supplied for the optional learned-router live test. No GitHub checks are configured (`statusCheckRollup: []`).

Final command tail:

```text
     Running tests/tui_client.rs (target/debug/deps/tui_client-bcc8dbd9a7699c74)

running 1 test
test polls_pauses_resumes_and_starts_provisioning ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

   Doc-tests shunt

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

VERIFY_EXIT=0
```

## Summary

Return a gateway-owned Anthropic 429 when the account loop has no upstream error to relay and the requested family has active cooldowns. Compute the earliest cooldown among enabled, unpaused accounts and round its remaining seconds up for `Retry-After`. All-paused/disabled pools get an explicit 503 message. The first request still relays the last upstream error. Selection order, quota polling, credential writeback, other providers, and the TUI are unchanged.

Verified implementation: `4eb1860c8955059fe2b5b7abe1531977d46bd623`. Live-tested revision: `ff732df`, whose code/test change rebased identically as `445e8fb`; the later unit test and current-main rebase passed final verification. This outcome commit also normalizes captured HTTP header line endings to LF. No material decision or scope deviation was needed. Branch remains `goal/agy-pool/task-012`; PR targets `main` and must not be merged by this worker.

An initial earlier full run failed the unrelated Codex test `pool_http_dispatch_seeds_message_start_input_token_estimate` with zero input tokens under concurrent build load. A focused recheck passed in 0.55s, and the final exact whole-suite run passed it. Recorded for follow-up without changing that code: `~/GitHub/config/agent-inbox/bugs/20261014T100821Z-32904-Codex-input-token-estimate-test-can-fail-under-c.md`. Proposed follow-up: investigate the cold tokenizer and one-second estimation deadline under load.

## Review follow-up (PR #20 blocking bug)

The 429 helper `antigravity_retry_after` (src/accounts.rs) now returns a reset only when EVERY enabled, unpaused account has an active family cooldown. Otherwise the gateway returns 503 `no Antigravity accounts are available` without `Retry-After`. Added: gateway test `cooled_account_with_a_token_failure_on_another_is_not_a_429` (A cooled, B fails token resolution, no upstream call, 503) and unit test `antigravity_retry_after_requires_every_unpaused_account_cooled` (A cooled, B uncooled and not tried: None; pausing B: Some). A gateway-level test for a B skipped by `select_order` was not feasible: select_order does not drop relogin or token_env accounts, and quota skipping cannot be mocked simply, so the unit test covers that case. Docs updated (m17, en/ko/ja/zh-cn provider pages). `src/adapters/gemini/mod.rs` ~201-207 is reached only for `AntigravityOauth` providers (other auth modes return earlier at ~189), so the Antigravity name in that message is correct. Verify rerun: fmt, clippy `-D warnings` and `cargo test --all-features --workspace` exit 0.

### Follow-up evidence (head after this commit)

- Gateway test for review case 1: `cooled_account_with_a_deferred_uncooled_account_is_not_a_429` (A cooled; B uncooled but skipped by storm admission; last candidate C fails token resolution; 503, no upstream call).
- Live recheck with the fixed helper: see "Case 5 addendum" in `docs/notes/agy-pool-live-test.md` (first 429 no `Retry-After`; second and third 429 `Retry-After: 526578`, no upstream call).
- Verify rerun after these changes: fmt, clippy `-D warnings`, `cargo test --all-features --workspace` all exit 0 (48 test binaries ok).

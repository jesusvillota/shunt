# TASK-004 outcome

## Acceptance

- [x] **1. Higher remaining fraction precedes lower fraction, per model family.**
  This session's full run passed `higher_remaining_fraction_precedes_lower_per_model_family`.
  Gemini orders 0.8 before 0.2; Claude and GPT order 0.9 before 0.1 on the same accounts.
  `family_quota_selects_the_highest_account_and_fails_over_to_lower` passed through real
  gateway HTTP requests against a mock upstream. Attempts use bearers/projects in order
  `b,c,a,a,c`: Gemini tries its highest account, then the next highest after 429;
  Claude/GPT use their own highest account. Five helper tests and all five pool integration
  tests passed. Coverage also checks priority, disabled/paused accounts, family cooldowns,
  lowest active window, earliest reset, equivalent time zones, expired/invalid readings,
  unknown readings, equal-value rotation, and isolation from other providers.
- [x] **2. Live requests use at least three distinct accounts; `goal pool` shows quota.**
  Six requests returned HTTP 200 and visible `OK.` responses from `cemfi`, `computing-jvm`,
  and `keepuntouser`. These are three accounts even when the known computing aliases count
  as one. `goal pool --goal agy-pool --json` captured both grouped weekly buckets and reset
  times for all six configured accounts before and after. Successful-request counters
  increased by six. Live smoke exited 0. Captured output is pasted below.
- [x] **3. fmt, clippy and full tests pass.**
  The exact Verify command below exited 0 in this session with `RUST_TEST_THREADS=1`.
  Format passed; all-feature clippy passed with warnings denied. The full workspace run
  completed 47 result groups: 3,783 passed, zero failed, two existing ignored measurement
  tests. An earlier default-thread run also completed all 47 groups with the same totals
  and exit 0. Optional live prefill-router coverage retains its checkpoint-dependent skip.

## Summary

Within each configured priority tier, Antigravity selection now ranks the requested
model family's highest remaining fraction first, then earliest reset. When both windows
exist, their lowest fraction governs. Expired windows are ignored; known readings precede
unknown readings, while accounts without readings remain eligible. Equal readings rotate.
Generic reset-sorting settings do not replace family ranking. Existing cooldown and
eligibility rules remain. The ranking uses stamped Antigravity accounts, including custom
provider names; other providers keep their existing behavior.

Verified implementation revision: `521c0dd623d2a32362f8d2bfcb083f5d49e1d8bc`.
This outcome commit changes only the report. Branch `goal/agy-pool/task-004` was rebased
cleanly onto `origin/main` before implementation. PR #14 targets `main`, carries
`goal:agy-pool`, and contains `Goal-Task: agy-pool/TASK-004`. GitHub returned an empty
`statusCheckRollup`; no checks are configured. No Actions were run or read. Never merged.

Documentation surfaces considered: M17 and all four maintained provider pages now explain
family ranking and `goal pool --json`. README and its translations need no setup/capability
change; their quota-source descriptions remain accurate. Generated `wiki/` is untouched.
No public config key, credential refresh/writeback behavior, task spec, other task,
decision, or `goal.yaml` changed.

Two intervening parallel test runs hit the already-recorded cold-tokenizer timing defect
in `tests/codex_multi_account.rs` (zero initial input-token estimate). Both affected tests
passed unchanged in the final single-thread full run. No tests or time limits were weakened.
The duplicate bug note already exists in the agent inbox; no duplicate was added.
Follow-up: fix that timing defect separately if desired. Existing duplicate operator logins
remain an operator follow-up; this task does not change account identity rules.

## Verification

```sh
export RUST_TEST_THREADS=1
cargo fmt --all --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all-features --workspace
```

Exit status: **0**. Final log tail:

```text
running 1 test
test polls_pauses_resumes_and_starts_provisioning ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s

   Doc-tests shunt

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

```

Built the required UI assets with `npm ci && npm run build` in `ui/`, and rebuilt this
worktree's gateway with `cargo build --all-features`. Generated files are not committed.
Local macOS verification only. Session log: `/tmp/shunt-task004-verify.log`.

## Live smoke

Scratch configuration: `/tmp/agy-task004-live.toml`, derived from the existing TASK-002
scratch configuration. It uses all six operator accounts, native Antigravity inference,
`usage_refresh_seconds = 60`, and the existing explicit personal agy profile. The production
configuration was not edited. The gateway retains its normal credential refresh/writeback.

Commands executed:

```sh
target/debug/shunt check --config /tmp/agy-task004-live.toml
target/debug/shunt run --config /tmp/agy-task004-live.toml
SHUNT_ADMIN_URL=http://127.0.0.1:39004 goal pool --goal agy-pool --json
```

`shunt check` returned `config ok`. The gateway listened on `127.0.0.1:39004` and started
its 60-second usage poller. Three rounds sent one Gemini and one Claude request to
`POST /v1/messages`, using `max_tokens: 128`, `thinking: {"type":"disabled"}`, and
`Reply with OK.`. Rounds waited 55 seconds to observe quota refreshes. The final snapshot
reflects the most recent poll; quota readings can lag inference by the refresh interval.
An initial 16-token probe returned HTTP 200 without visible text; a separate 128-token
probe returned `OK.`. Those two probes precede the smoke's baseline counters.
Upstream reported weekly windows only, as permitted by accepted DEC-002; no 5h value was
invented. Plain `goal pool` text omits grouped buckets, so these captures use its JSON mode.

```text
LIVE round=1 model=gemini-3.8-flash status=200 x-shunt-account=cemfi text=['OK.']
LIVE round=1 model=claude-sonnet-4-6 status=200 x-shunt-account=computing-jvm text=['OK.']
LIVE round=2 model=gemini-3.8-flash status=200 x-shunt-account=cemfi text=['OK.']
LIVE round=2 model=claude-sonnet-4-6 status=200 x-shunt-account=keepuntouser text=['OK.']
LIVE round=3 model=gemini-3.8-flash status=200 x-shunt-account=cemfi text=['OK.']
LIVE round=3 model=claude-sonnet-4-6 status=200 x-shunt-account=cemfi text=['OK.']
SMOKE_EXIT=0 distinct_serving_accounts=['cemfi', 'computing-jvm', 'keepuntouser'] requests=6
```

### `goal pool --json` before

Selected account fields from the captured command output:

```json
{"name":"computingjvm","requests_succeeded":0,"quota_buckets":[{"label":"Gemini Models · weekly","remaining":0.0,"reset_time":"2026-10-20T17:55:30Z"},{"label":"Claude + GPT Models · weekly","remaining":0.999908,"reset_time":"2026-10-21T00:23:37Z"}]}
{"name":"keepuntouser","requests_succeeded":1,"quota_buckets":[{"label":"Gemini Models · weekly","remaining":0.9999736,"reset_time":"2026-10-21T00:23:42Z"},{"label":"Claude + GPT Models · weekly","remaining":0.999862,"reset_time":"2026-10-21T00:23:44Z"}]}
{"name":"tommyshelby","requests_succeeded":0,"quota_buckets":[{"label":"Gemini Models · weekly","remaining":0.0,"reset_time":"2026-10-20T12:41:36Z"},{"label":"Claude + GPT Models · weekly","remaining":0.0,"reset_time":"2026-10-14T23:03:28Z"}]}
{"name":"computing-jvm","requests_succeeded":0,"quota_buckets":[{"label":"Gemini Models · weekly","remaining":0.0,"reset_time":"2026-10-20T17:55:30Z"},{"label":"Claude + GPT Models · weekly","remaining":0.999908,"reset_time":"2026-10-21T00:23:37Z"}]}
{"name":"personal","requests_succeeded":0,"quota_buckets":[{"label":"Gemini Models · weekly","remaining":0.0,"reset_time":"2026-10-20T17:09:34Z"},{"label":"Claude + GPT Models · weekly","remaining":0.8474119901657104,"reset_time":"2026-10-20T23:53:16Z"}]}
{"name":"cemfi","requests_succeeded":1,"quota_buckets":[{"label":"Gemini Models · weekly","remaining":0.999988,"reset_time":"2026-10-21T00:24:05Z"},{"label":"Claude + GPT Models · weekly","remaining":0.999862,"reset_time":"2026-10-21T00:24:06Z"}]}
```

### `goal pool --json` after

Selected account fields from the captured command output:

```json
{"name":"computingjvm","requests_succeeded":0,"quota_buckets":[{"label":"Gemini Models · weekly","remaining":0.0,"reset_time":"2026-10-20T17:55:30Z"},{"label":"Claude + GPT Models · weekly","remaining":0.999854,"reset_time":"2026-10-21T00:23:37Z"}]}
{"name":"keepuntouser","requests_succeeded":2,"quota_buckets":[{"label":"Gemini Models · weekly","remaining":0.9999736,"reset_time":"2026-10-21T00:23:42Z"},{"label":"Claude + GPT Models · weekly","remaining":0.999808,"reset_time":"2026-10-21T00:23:44Z"}]}
{"name":"tommyshelby","requests_succeeded":0,"quota_buckets":[{"label":"Gemini Models · weekly","remaining":0.0,"reset_time":"2026-10-20T12:41:36Z"},{"label":"Claude + GPT Models · weekly","remaining":0.0,"reset_time":"2026-10-14T23:03:28Z"}]}
{"name":"computing-jvm","requests_succeeded":1,"quota_buckets":[{"label":"Gemini Models · weekly","remaining":0.0,"reset_time":"2026-10-20T17:55:30Z"},{"label":"Claude + GPT Models · weekly","remaining":0.999854,"reset_time":"2026-10-21T00:23:37Z"}]}
{"name":"personal","requests_succeeded":0,"quota_buckets":[{"label":"Gemini Models · weekly","remaining":0.0,"reset_time":"2026-10-20T17:09:34Z"},{"label":"Claude + GPT Models · weekly","remaining":0.8474119901657104,"reset_time":"2026-10-20T23:53:16Z"}]}
{"name":"cemfi","requests_succeeded":5,"quota_buckets":[{"label":"Gemini Models · weekly","remaining":0.999976,"reset_time":"2026-10-21T00:24:05Z"},{"label":"Claude + GPT Models · weekly","remaining":0.999862,"reset_time":"2026-10-21T00:24:06Z"}]}
```

The gateway received SIGTERM and exited 0 after graceful drain:

```text
2026-10-14T01:58:26.821740Z  INFO shunt::shutdown: shutdown signal received; draining in-flight requests signal="SIGTERM"
2026-10-14T01:58:26.826566Z  INFO shunt: graceful shutdown drain completed
```

Session artifacts: `/tmp/shunt-task004-smoke-final.log`,
`/tmp/shunt-task004-pool-before.json`, `/tmp/shunt-task004-pool-after.json`, and
`/tmp/shunt-task004-live.log`. They contain the raw command responses; no credential
values are included in this report or commit.

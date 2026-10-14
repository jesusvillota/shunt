# Antigravity pool: live failover and ranking test

**Date:** 2026-10-14 · goal `agy-pool`, TASK-010 · build: this branch (`cargo build`, shunt 0.51.1, debug)
**Result:** cases 1, 2, 3 and 4 PASS; case 5 FAIL (bug recorded with `agent-note bug`).

## Setup

- Scratch gateway only: `HOME=/tmp/t010/home` (a copy of `~/.shunt/accounts/antigravity` and the admin token, so
  token write-back never touches the real files), config `/tmp/t010/shunt.toml`, bind `127.0.0.1:39010`,
  `[server.pool] usage_refresh_seconds = 60`, the five live accounts (`computingjvm`, `keepuntouser`,
  `tommyshelby`, `personal`, `cemfi`), two routes (`gemini-3.8-flash-low`, `claude-sonnet-4-6`).
  Started with `HOME=/tmp/t010/home RUST_LOG=shunt=debug shunt run --config /tmp/t010/shunt.toml`.
- Quota at test start, from `shunt pool --config /tmp/t010/shunt.toml` (confirms the expected exhausted set):

  | Account | Gemini weekly | Claude + GPT weekly |
  | --- | --- | --- |
  | computingjvm | **0%** | 100% |
  | keepuntouser | 100% | 100% |
  | tommyshelby | **0%** | **0%** (resets in ~13h) |
  | personal | **0%** | 85% |
  | cemfi | 100% | 100% |

- Request (prompt is tiny, `max_tokens` 16), `./req.sh gemini|sonnet` expands to:
  ```sh
  curl -sS http://127.0.0.1:39010/v1/messages -H 'content-type: application/json' -H 'anthropic-version: 2023-06-01' \
    -d '{"model":"<claude-gemini-3.8-flash-low-via-antigravity | claude-sonnet-4-6-via-antigravity>","max_tokens":16,"messages":[{"role":"user","content":"Say ok"}]}'
  ```
- **Account attribution.** The gateway log does not name the account that served a request (nor log
  per-account 429s at `info`/`debug`). The serving account is therefore taken from the admin pool counters
  (`shunt pool --json`: `requests_attempted/succeeded/failed` per account) read before and after each request,
  plus the `proxied request`/`upstream request failed` log lines. A 429 is an *attempt that failed*.
- Custom order = per-account `priority` in the scratch config (hot-reloaded).
- Pause: `PATCH /admin/api/pool/antigravity/accounts/<account_ref>` with `{"paused":true}` (scratch gateway, logged as
  `admin: pool account pause flag updated`).

## Case 2 — Ranking (balanced): PASS

Run first, on a fresh gateway, so no cooldown exists yet. No priorities (balanced), 3 Gemini requests.

| # | Request | Serving account | HTTP |
| --- | --- | --- | --- |
| 1 | `./req.sh gemini` | keepuntouser | 200 |
| 2 | `./req.sh gemini` | keepuntouser | 200 |
| 3 | `./req.sh gemini` | keepuntouser | 200 |

Counters after: `keepuntouser att=3 ok=3 fail=0`; the other four `att=0`. Exhausted accounts were never
tried, there was no 429. Log: three `proxied request upstream_status=200` (2423, 2106, 3179 ms) and no WARN.
Pool counters match, so the exhausted accounts were ranked last (not even reached).
**PASS**: first request succeeded with no 429; exhausted accounts last.

## Case 1 — Failover (custom order, exhausted first): PASS

Priorities `computingjvm=1, tommyshelby=2, personal=3, keepuntouser=4, cemfi=5` (the three Gemini-exhausted
accounts first), same gateway process.

Request A: `./req.sh gemini` → **HTTP 200**, 3.75 s. Counter deltas:

| Account | attempted | succeeded | failed |
| --- | --- | --- | --- |
| computingjvm | +1 | 0 | +1 (429) |
| tommyshelby | +1 | 0 | +1 (429) |
| personal | +1 | 0 | +1 (429) |
| keepuntouser | +1 | +1 | 0 |

Each exhausted account answered 429 in priority order, then `keepuntouser` served the request. Log:
`proxied request upstream_status=200 latency_ms=3752` (the per-account 429s inside the failover loop are not logged).

Request B (same command): **HTTP 200** in 1.7 s; delta: only `keepuntouser +1 attempted/+1 succeeded`. The three
exhausted accounts were **not tried again** — they are in a Gemini-family cooldown (they are not cooled account-wide:
`cooldown_secs_remaining` stays `null`, and case 3 shows they still serve Claude). The real upstream message gives
`Resets in 161h...`, so the cooldown lasts days (see case 4 for expiry).
**PASS**.

## Case 3 — Family isolation: PASS

3a (same gateway as case 1, `computingjvm` Gemini-cooled): `./req.sh sonnet` → **HTTP 200**, served by `computingjvm`
(delta `+1 attempted, +1 succeeded`). A Gemini-exhausted/cooled account still serves Claude.

3b (gateway restarted, clean state, priorities `tommyshelby=1, computingjvm=2, personal=3, keepuntouser=4, cemfi=5`;
`tommyshelby` has Claude+GPT *and* Gemini at 0%):

| # | Request | tommyshelby | Served by | HTTP |
| --- | --- | --- | --- | --- |
| 1 | `./req.sh sonnet` | att +1, fail +1 (429) | computingjvm | 200 |
| 2 | `./req.sh sonnet` | no change (skipped: Claude cooled) | computingjvm | 200 |
| 3 | `./req.sh gemini` | att +1, fail +1 (**tried**: Gemini not cooled) | keepuntouser (after computingjvm and personal 429'd) | 200 |

Request 3 shows the Claude 429 on `tommyshelby` did not cool its Gemini family (it was still attempted; it
429'd only because its Gemini quota really is 0%). Log: three `proxied request upstream_status=200` lines.
**PASS**.

## Case 4 — Cooldown reset: PASS (mock upstream, not live Google)

A live Gemini cooldown lasts ~6 days, so expiry cannot be waited out. Instead a scratch gateway
(`/tmp/t010/shunt4.toml`, port 39012, one real account `keepuntouser`, `base_url` = a local mock on 39011 that always
answers 429 `... Resets in 15s.`) drives the same code path. Times are local.

| Time | Request | Result | Mock hit |
| --- | --- | --- | --- |
| 11:22:32 | `gemini` | HTTP 429, Anthropic `rate_limit_error` | yes |
| 11:22:32 | `gemini` | HTTP 503 (family in cooldown, see case 5) | **no** |
| 11:22:49 | `gemini` (17 s later) | HTTP 429 | **yes** — account tried again |

`mock.log`: `11:22:32 MOCK HIT /v1internal:generateContent`, `11:22:49 MOCK HIT /v1internal:generateContent`.
**PASS**: the account is skipped during the cooldown and tried again after it. Limitation: mock upstream; live
expiry is also covered by the TASK-003 unit test for weekly cooldown expiry.

## Case 5 — All exhausted: FAIL

`keepuntouser` and `cemfi` paused through the admin API; the other three are Gemini-exhausted. Clean gateway.

| # | Request | HTTP | Time | Body |
| --- | --- | --- | --- | --- |
| 1 | `./req.sh gemini` | **429** | 2.1 s | `{"type":"error","error":{"type":"rate_limit_error","message":"Individual quota reached. ... Resets in 151h47m15s."}}` |
| 2 | `./req.sh gemini` | **503** | 2 ms | `{"type":"error","error":{"type":"api_error","message":"no Antigravity accounts are available"}}` |

Request 1 is a prompt Anthropic-shaped 429 (all three exhausted accounts tried, counters `+1/+1 failed` each). Request 2,
once they are cooled, is an immediate **5xx**, which the case forbids. Log:
`WARN ... upstream request failed latency_ms=0 error=no Antigravity accounts are available` with
`http.response.status_code=503`. Same result in a second run, and for the 503 in case 4.
Root cause: `src/adapters/gemini/mod.rs` returns that 503 fallback when the loop attempts no account.
Not fixed here (out of scope). Recorded with `agent-note bug` ("Antigravity pool returns 503 api_error instead of 429
once every account is cooled or paused"). **FAIL**.

## Live gateway untouched

- `shasum ~/.config/shunt/shunt.toml` = `84fb6c69f0c78614a7ca3211e66596d551b7a6ee` before and after the test.
- The launchd gateway process (`pid 57240`, started 10:57) was neither restarted nor signalled; all scratch gateways
  were killed by their own command line (`pkill -f 'shunt run --config /tmp/t010'`). Scratch ports 39010/39011/39012.
- Scratch credentials copy used a separate `HOME`; no live account file was written.

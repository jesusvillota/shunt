# Antigravity pool: live failover and ranking test

**Date:** 2026-10-14 · goal `agy-pool`, TASK-010 · build: this branch (`cargo build`, shunt 0.51.1, debug)
**Result:** cases 1, 2, 3 PASS; case 4 PASS **on a mock upstream only**; case 5 FAIL (bug recorded with `agent-note bug`).

All raw evidence is in [`agy-pool-live-test/`](agy-pool-live-test/). This note was re-run from scratch on
2026-10-14 (11:26–11:28 CEST) after review; every number below comes from the committed files.

## Setup

- Scratch gateway only: `HOME=/tmp/t010/home` (a copy of `~/.shunt/accounts/antigravity` and the admin token, so
  token write-back never touches the real files), bind `127.0.0.1:39010`, `[server.pool] usage_refresh_seconds = 60`,
  the five live accounts (`computingjvm`, `keepuntouser`, `tommyshelby`, `personal`, `cemfi`), two routes.
  Started with `HOME=/tmp/t010/home RUST_LOG=shunt=debug shunt run --config /tmp/t010/shunt.toml`.
- Quota at test start (`agy-pool-live-test/start.pool.json`):

  | Account | Gemini weekly | Claude + GPT weekly |
  | --- | --- | --- |
  | computingjvm | **0%** (resets 2026-10-20) | 100% |
  | keepuntouser | 100% | 100% |
  | tommyshelby | **0%** | **0%** (resets 2026-10-14 23:03Z) |
  | personal | **0%** | 85% |
  | cemfi | 100% | 100% |

- **Requests.** Every request is one of these two commands (only the model id differs; `max_tokens` 16). Each
  request's literal command, HTTP status, time and body are in its `<label>.request.txt`.
  ```sh
  # Gemini
  curl -sS -m 90 -o /tmp/t010/body.txt -w 'HTTP %{http_code} in %{time_total}s' http://127.0.0.1:39010/v1/messages -H 'content-type: application/json' -H 'anthropic-version: 2023-06-01' -d '{"model":"claude-gemini-3.8-flash-low-via-antigravity","max_tokens":16,"messages":[{"role":"user","content":"Say ok"}]}'
  # Claude
  curl -sS -m 90 -o /tmp/t010/body.txt -w 'HTTP %{http_code} in %{time_total}s' http://127.0.0.1:39010/v1/messages -H 'content-type: application/json' -H 'anthropic-version: 2023-06-01' -d '{"model":"claude-sonnet-4-6-via-antigravity","max_tokens":16,"messages":[{"role":"user","content":"Say ok"}]}'
  ```
- **Account attribution.** The gateway log does not name the account that served a request, nor log per-account
  429s. So before and after every request the raw pool state was saved:
  `HOME=/tmp/t010/home shunt pool --config /tmp/t010/shunt.toml --json > <label>.before.json` / `<label>.after.json`.
  The serving account is the one whose `requests_succeeded` rose by 1; a 429 is an account whose `requests_failed`
  rose. The deltas below were computed from those two files. No tokens are in the files (grepped for `ya29`, `eyJ`,
  `access_token`, `refresh_token`, `Bearer`, e-mail addresses: no match).
- Custom order = per-account `priority` in the scratch config (hot-reloaded), see `case1.shunt.toml`, `case3b.shunt.toml`.
- Gateway log lines (UTC, filtered): `phase1.gateway.log` (cases 2, 1, 3a), `phase2.gateway.log` (3b),
  `phase3.gateway.log` (case 5), `phase4.gateway.log` (case 4).

## Case 2 — Ranking (balanced): PASS

Fresh gateway, no priorities (balanced), 3 Gemini requests (Gemini command above).

| Label | HTTP | Account that served (delta) | Failed attempts |
| --- | --- | --- | --- |
| `c2-r1` | 200 | cemfi (att +1, ok +1) | none |
| `c2-r2` | 200 | cemfi (att +1, ok +1) | none |
| `c2-r3` | 200 | cemfi (att +1, ok +1) | none |

The three Gemini-exhausted accounts had no attempt in any request: they were ranked last. No 429.
Log (`phase1.gateway.log`): three `proxied request upstream_status=200` (2735, 2121, 2039 ms), no WARN.
**PASS**: first request succeeded with no 429; exhausted accounts last.

## Case 1 — Failover (custom order, exhausted first): PASS

Priorities `computingjvm=1, tommyshelby=2, personal=3, keepuntouser=4, cemfi=5` (the three Gemini-exhausted accounts
first), same gateway process, config hot-reloaded. Gemini command.

`c1-A` → **HTTP 200**, 3.55 s. Deltas:

| Account | attempted | succeeded | failed |
| --- | --- | --- | --- |
| computingjvm | +1 | 0 | +1 (429) |
| tommyshelby | +1 | 0 | +1 (429) |
| personal | +1 | 0 | +1 (429) |
| keepuntouser | +1 | +1 | 0 |

Each exhausted account answered 429 in priority order, then `keepuntouser` served. Log: `proxied request
upstream_status=200 latency_ms=3551` (the per-account 429s inside the failover loop are not logged).

`c1-B` (same command) → **HTTP 200**, 3.07 s; delta: only `keepuntouser` +1 attempted/+1 succeeded. The three exhausted
accounts were **not tried again**, so they are in a Gemini-family cooldown. It is not account-wide:
`cooldown_secs_remaining` stays `null` in `c1-B.after.json`, and case 3a shows `computingjvm` still serves Claude.
The upstream message says `Resets in 151h...`, so this cooldown lasts days (case 4 covers expiry).
**PASS**.

## Case 3 — Family isolation: PASS

3a (same gateway as case 1, `computingjvm` Gemini-cooled): `c3a` (Claude command) → **HTTP 200**, served by
`computingjvm` (delta `+1 attempted, +1 succeeded`, nobody else touched). A Gemini-cooled account serves Claude.

3b (gateway restarted, clean state, `case3b.shunt.toml`: `tommyshelby=1, computingjvm=2, personal=3, keepuntouser=4,
cemfi=5`; `tommyshelby` has Claude+GPT *and* Gemini at 0%):

| Label | Command | tommyshelby | Served by | HTTP |
| --- | --- | --- | --- | --- |
| `c3b-1` | Claude | att +1, failed +1 (429) | computingjvm | 200 |
| `c3b-2` | Claude | no change (skipped: Claude family cooled) | computingjvm | 200 |
| `c3b-3` | Gemini | att +1, failed +1 (**tried**: Gemini not cooled) | keepuntouser (computingjvm and personal also 429'd) | 200 |

`c3b-3` shows the Claude 429 on `tommyshelby` did not cool its Gemini family: it was still attempted, and it 429'd only
because its Gemini quota really is 0%. Log (`phase2.gateway.log`): three `proxied request upstream_status=200`.
**PASS**.

## Case 4 — Cooldown reset: PASS (mock upstream, not live Google)

**Live Google cannot be reset-tested.** The Gemini weekly cooldowns reset on 2026-10-20 (about 6 days away), so no
live cooldown can expire during a test. This case therefore does NOT prove expiry against the real upstream. It
drives the same pool code path against a local mock that always answers HTTP 429 with the real error body but
`Resets in 15s.`

Files: `case4.mock.py` (mock on port 39011, prints a line per generate request), `case4.shunt.toml` (gateway on port
39012, one real account `keepuntouser`, `base_url = "http://127.0.0.1:39011"`), `case4.mock.log`, `phase4.gateway.log`.
Gemini command with port `39012`. Times are local.

| Label | Time | HTTP | Mock hit? | Account delta |
| --- | --- | --- | --- | --- |
| `c4-r1` | 11:28:00 | 429, Anthropic `rate_limit_error` | yes | keepuntouser att +1, failed +1 |
| `c4-r2` | 11:28:00 | 503 (family in cooldown; see case 5) | **no** | none |
| `c4-r3` | 11:28:16 | 429 | **yes** | keepuntouser att +1, failed +1 |

`case4.mock.log`: `11:28:00 MOCK HIT /v1internal:generateContent`, `11:28:16 MOCK HIT /v1internal:generateContent`.
The account was skipped inside the cooldown and tried again 16 s later. **PASS (mock only)**. Live expiry is covered
only by the TASK-003 unit test for weekly cooldown expiry, not by this note.

## Case 5 — All exhausted: FAIL

`keepuntouser` and `cemfi` paused through the admin API; the other three are Gemini-exhausted. Clean gateway.
The pause commands (admin token read from the file, never written to the evidence) and the responses are in
`c5-pause.txt` (both `{"ok":true} HTTP 200`); `c5-paused.pool.json` shows `paused: true` for exactly those two.
Gemini command, three times:

| Label | HTTP | Time | Body |
| --- | --- | --- | --- |
| `c5-r1` | **429** | 1.84 s | `{"type":"error","error":{"type":"rate_limit_error","message":"Individual quota reached. ... Resets in 151h41m46s."}}` |
| `c5-r2` | **503** | 1 ms | `{"type":"error","error":{"type":"api_error","message":"no Antigravity accounts are available"}}` |
| `c5-r3` | **503** | 1 ms | same as `c5-r2` |

`c5-r1` is a prompt Anthropic-shaped 429 (computingjvm, tommyshelby and personal each tried once: failed +1). After
that they are cooled and `c5-r2`/`c5-r3` are an immediate **5xx**, which the case forbids. Log
(`phase3.gateway.log`): `WARN ... upstream request failed latency_ms=0 error=no Antigravity accounts are available`
with `http.response.status_code=503`, twice.
Root cause: `src/adapters/gemini/mod.rs` returns that 503 fallback when the loop attempts no account. Not fixed here
(out of scope). Recorded with `agent-note bug` ("Antigravity pool returns 503 api_error instead of 429 once every
account is cooled or paused"). **FAIL**.

## Live gateway untouched

Evidence: `live-before.txt` (taken before the first scratch gateway started) and `live-after.txt` (taken after the
last one was killed). Both are raw command output.

```
$ shasum ~/.config/shunt/shunt.toml
84fb6c69f0c78614a7ca3211e66596d551b7a6ee  /Users/jesusvillotamiranda/.config/shunt/shunt.toml        # before AND after
$ diff /tmp/t010/live-shunt.toml.before ~/.config/shunt/shunt.toml && echo IDENTICAL
IDENTICAL
$ launchctl list | grep shunt
57240	0	com.jesusvillotamiranda.shunt                                                                # before AND after
$ ps -o pid,lstart,command -p 57240
57240 Wed Oct 14 10:57:13 2026     /Users/jesusvillotamiranda/GitHub/shunt/target/release/shunt run  # before AND after
```

The launchd gateway (pid 57240, started 10:57:13) kept the same pid and start time, so it was not restarted. The
scratch gateways ran on ports 39010/39012 with `HOME=/tmp/t010/home` and were killed with
`pkill -f 'shunt run --config /tmp/t010'`, which cannot match the live process. No live account file was written.

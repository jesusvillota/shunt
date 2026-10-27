# Antigravity pool: live failover and ranking test

**Date:** 2026-10-14 · goal `agy-pool`, TASK-010 · build: this branch (`cargo build`, shunt 0.51.1, debug)
**Result:** cases 1, 2, 3 PASS; case 4 PASS **on a mock upstream only**; case 5 PASS after TASK-012 (live recheck below).

This note was re-run from scratch on 2026-10-14 (11:26–11:28 CEST) after review. The raw files are in git history.

## Setup

- Scratch gateway only: `HOME=/tmp/t010/home` (a copy of `~/.shunt/accounts/antigravity` and the admin token, so
  token write-back never touches the real files), bind `127.0.0.1:39010`, `[server.pool] usage_refresh_seconds = 60`,
  the five live accounts (`computingjvm`, `keepuntouser`, `tommyshelby`, `personal`, `cemfi`), two routes.
  Started with `HOME=/tmp/t010/home RUST_LOG=shunt=debug shunt run --config /tmp/t010/shunt.toml`.
- Quota at test start :

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

## Case 5 — All exhausted: PASS (TASK-012 live recheck)

**Rechecked:** 2026-10-14 10:02:26–10:02:29 UTC, revision `ff732df`.
The original TASK-010 run returned 503 on its second and third requests.
TASK-012 fixes that fallback.

Scratch gateway only: port **39020**, separate HOME
`/var/folders/_j/7j25pgqd5z3dh647gtb8vmbw0000gn/T/shunt-task012-_k5nvecd/home`,
copied account credentials and admin token. `scratch.shunt.toml` records the exact config.
The gateway used the real `https://daily-cloudcode-pa.googleapis.com` upstream, with
`usage_refresh_seconds = 60`. No mock was used. Start command (from the worktree):

```sh
HOME=/var/folders/_j/7j25pgqd5z3dh647gtb8vmbw0000gn/T/shunt-task012-_k5nvecd/home RUST_LOG=shunt=debug ./target/debug/shunt run --config /var/folders/_j/7j25pgqd5z3dh647gtb8vmbw0000gn/T/shunt-task012-_k5nvecd/shunt.toml
```

Paused `keepuntouser` and `cemfi` through the scratch admin API (HTTP 200, `{"ok":true}` each).
`paused.pool.json` confirms these two pauses. The three other accounts reported zero Gemini weekly quota.
Repeated this command three times, changing the output label to `r1`, `r2`, then `r3`:

```sh
T012_SCRATCH=/var/folders/_j/7j25pgqd5z3dh647gtb8vmbw0000gn/T/shunt-task012-_k5nvecd
curl -sS -m 90 -D "$T012_SCRATCH/r1.headers.txt" -o "$T012_SCRATCH/r1.body.json" -w 'HTTP %{http_code} in %{time_total}s' http://127.0.0.1:39020/v1/messages -H 'content-type: application/json' -H 'anthropic-version: 2023-06-01' -d '{"model":"claude-gemini-3.8-flash-low-via-antigravity","max_tokens":16,"messages":[{"role":"user","content":"Say ok"}]}'
```

Each `r*.request.txt` contains the exact status, headers and body. Saved headers use LF line endings.

| Request | HTTP | Time | `Retry-After` | Account attempts |
| --- | --- | --- | --- | --- |
| First (`r1`) | **429** | 3.144431 s | absent (last upstream error relayed) | computingjvm, tommyshelby, personal: +1 failed each |
| Second (`r2`) | **429** | 0.002427 s | **527947** seconds | none |
| Third (`r3`) | **429** | 0.002363 s | **527947** seconds | none |

First response body:

```json
{"type":"error","error":{"type":"rate_limit_error","message":"Individual quota reached. Please upgrade your subscription to increase your limits. Resets in 151h7m4s."}}
```

Second and third response bodies:

```json
{"type":"error","error":{"type":"rate_limit_error","message":"Antigravity quota exhausted. Resets in 527947s."}}
```

The earliest eligible Gemini reset was tommyshelby's `2026-10-20T12:41:36Z`.
At `2026-10-14T10:02:29Z`, it was 527947 seconds away. This is earlier than the other two resets.
The `r*.before.pool.json` and `r*.after.pool.json` snapshots show zero changes to all attempt,
success and failure counters on the second and third requests. The gateway returned both locally.
`gateway.log` records three 429 responses, with the later requests at 0 ms and 1 ms logged latency.
**PASS**: every request returned Anthropic `rate_limit_error`; later requests include the earliest reset and make no inference attempt.

The scratch gateway was stopped by its own PID (94158). `production-before.json` and
`production-after.json` are identical: the live config SHA-256 stayed
`41c267ae3f54974b6644357922a7b85fb501d1e98ab5cbddea243d485a7438b0`,
and the launchd gateway stayed PID 57240. No live credential or config file was written.

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

### Case 5 addendum — recheck after the "every account" fix (2026-10-14)

Re-run with the fixed helper (`antigravity_retry_after` now needs every enabled, unpaused account
cooled). Scratch gateway only: port 39020, the same scratch HOME, `./target/debug/shunt run` at
the fix commit. The admin pause call returned 401 with the copied token, so instead of pausing, the
scratch config omits `keepuntouser` and `cemfi` (the two accounts that still have Gemini quota); the
three exhausted accounts remain.

| Request | HTTP | Time | `Retry-After` | Body message |
| --- | --- | --- | --- | --- |
| First | **429** | 1.83 s | absent | `Individual quota reached. … Resets in 150h44m15s.` (last upstream error) |
| Second | **429** | 0.0011 s | **526578** | `Antigravity quota exhausted. Resets in 526578s.` |
| Third | **429** | 0.0009 s | **526578** | `Antigravity quota exhausted. Resets in 526578s.` |

All bodies are `{"type":"error","error":{"type":"rate_limit_error",…}}`. **PASS.** The live config SHA-256
stayed `41c267ae…438b0` and the launchd gateway stayed PID 57240.

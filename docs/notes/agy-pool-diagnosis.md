# Antigravity pool diagnosis: why quota is missing and the pool misbehaves

**Date:** 2026-10-14
**Status:** diagnosis only (goal `agy-pool`, TASK-001); no production code changed
**Verified against:** `main` @ 039c8b0 (shunt 0.51.1), `agy` 1.3.4, live Google
accounts, the operator's `~/.config/shunt/shunt.toml` (provider `antigravity`,
`auth = "antigravity_oauth"`, 6 accounts) and the running gateway on
`127.0.0.1:3001`.

## Summary

- **Root cause (missing quota):** `[server.pool]` in the live config is present
  but has no `usage_refresh_seconds`. The usage poller is only spawned when that
  key is set (`src/usage_poll.rs::spawn_usage_poller`), so no Antigravity quota
  is ever fetched. The HTTP quota RPC itself works for all six accounts.
- **Root cause (pool not working):** not a login, refresh or HTTP failure.
  Four of the six accounts have exhausted their weekly Gemini quota, and
  selection cannot know that: (a) Antigravity quota is display-only and never
  reaches selection, (b) a 429 cools down the **whole account** for a blind 60 s
  (`Retry-After` is absent) although Gemini and Claude+GPT are separate quota
  pools, and (c) `computingjvm` and `computing-jvm` are the same Google identity
  registered twice.
- **Recommendation for DEC-001:** keep the HTTP quota source (it works today for
  every account). Use `agy /usage` only as a cross-check. See the trade-off table.

## Method

1. Read the account files (metadata only, no tokens printed), the live config and
   the live gateway's admin pool state (`GET /admin/api/pool`).
2. Per account, with a standalone script (not part of the repo): refresh the
   token against `oauth2.googleapis.com/token` with shunt's client, then call
   `daily-cloudcode-pa.googleapis.com/v1internal:retrieveUserQuotaSummary`.
3. Per account, built this branch (`cargo build --release`) and ran a scratch
   gateway on `127.0.0.1:3991` with only that account configured; sent one
   `/v1/messages` request for a Gemini model and one for `claude-sonnet-4-6`.
4. Ran a scratch gateway with all six accounts plus `usage_refresh_seconds = 60`
   to observe the poller, selection and failover.
5. Ran `agy -p /usage --output-format json` (Orca's method).

## 1. Per-account results

All six accounts have a stored refresh token and `project_id = aicode-consumers`.
At probe time (02:20 CEST) every stored access token had already expired (≈8 min
earlier); refresh succeeded for all.

| Account | Google identity | Login (stored refresh token) | Token refresh | Quota fetch (HTTP) | Live request, Gemini | Live request, Sonnet 4.6 |
| --- | --- | --- | --- | --- | --- | --- |
| `computingjvm` | computingjvm@gmail.com | ok | ok, 200 | ok, 200 | **429** | ok, 200 |
| `keepuntouser` | keepuntouser@gmail.com | ok | ok, 200 | ok, 200 | ok, 200 | ok, 200 |
| `tommyshelby` | tommy4444shelby4444@gmail.com | ok | ok, 200 | ok, 200 | **429** | **429** |
| `computing-jvm` | computingjvm@gmail.com (**same as `computingjvm`**) | ok | ok, 200 | ok, 200 | **429** | ok, 200 |
| `personal` | jesusvillotam@gmail.com | ok | ok, 200 | ok, 200 | **429** | ok, 200 |
| `cemfi` | jesus.villota@cemfi.edu.es | ok | ok, 200 | ok, 200 | ok, 200 | ok, 200 |

Exact error text of the failing live requests (gateway-shaped Anthropic error,
HTTP 429):

```
computingjvm  gemini : {"type":"error","error":{"type":"rate_limit_error","message":"Individual quota reached. Please upgrade your subscription to increase your limits. Resets in 161h31m52s."}}
tommyshelby   gemini : ... "Individual quota reached. Please upgrade your subscription to increase your limits. Resets in 156h17m46s."
tommyshelby   sonnet : ... "Individual quota reached. Please upgrade your subscription to increase your limits. Resets in 22h39m37s."
computing-jvm gemini : ... "Individual quota reached. ... Resets in 161h31m35s."
personal      gemini : ... "Individual quota reached. ... Resets in 160h45m33s."
```

Quota returned by `retrieveUserQuotaSummary` (only `weekly` windows are returned
now; no `5h` bucket):

| Account | Gemini weekly remaining (reset) | Claude + GPT weekly remaining (reset) |
| --- | --- | --- |
| `computingjvm` | **0** (2026-10-20T17:55:30Z) | 1.0 |
| `keepuntouser` | 1.0 | 1.0 |
| `tommyshelby` | **0** (2026-10-20T12:41:36Z) | **0** (2026-10-14T23:03:28Z) |
| `computing-jvm` | **0** (2026-10-20T17:55:30Z) | 1.0 |
| `personal` | **0** (2026-10-20T17:09:34Z) | 0.847 |
| `cemfi` | 1.0 | 1.0 |

The quota numbers predict the live results exactly. `computingjvm` and
`computing-jvm` return identical reset times: one upstream account, two entries.

Live gateway state (`GET /admin/api/pool`, provider `antigravity`): all six
accounts show `has_state: false`, `utilization_*: null`, `requests_attempted: 0`,
no `quota_buckets`. The gateway has served no Antigravity traffic since its last
start (2026-10-13T18:43Z) and holds no quota.

Reference: the live gateway log has Antigravity failures only on 2026-09-23/24/28
and 2026-10-01 (429 failovers, one `400 Request contains an invalid argument.`,
one `404 Requested entity was not found.`); none since.

## 2. Root cause: missing quota

**Root cause:** `usage_refresh_seconds` is unset in `[server.pool]`.

Evidence:

- `~/.config/shunt/shunt.toml` ends with an empty `[server.pool]` table (its
  comment says it only enables ranking controls). No `usage_refresh_seconds`.
- `src/usage_poll.rs:56-71`: `spawn_usage_poller` returns early when
  `usage_refresh_interval()` is `None`; `src/config.rs:316` returns `None` for an
  unset or `0` value. The default is off.
- The live gateway logs (`shunt.log`, `shunt.error.log`) contain 0 occurrences of
  `starting OAuth usage-API poller`.
- Live `/admin/api/pool`: `has_state: false` and no `quota_buckets` on all six
  accounts.
- Counter-experiment: the same six accounts on a scratch gateway with
  `usage_refresh_seconds = 60` log `starting OAuth usage-API poller (Claude,
  Codex, Antigravity) interval_seconds=60`, and within 20 s `/admin/api/pool`
  reports `quota_buckets` for every account, for example
  `computingjvm: Gemini Models · weekly remaining 0.0`, `keepuntouser: Gemini
  Models · weekly remaining 0.9999928`.

Ruled out: no 403 (all six quota calls return 200 with a `daily-` host token), no
token problem (all six refresh with 200), no `project_id` problem (all
`aicode-consumers` and accepted), no parse problem (the grouped payload parses to
the expected `Gemini Models · weekly` / `Claude + GPT Models · weekly` labels).

Related, not blocking: Antigravity quota is stored in `quota_buckets` only
(`AccountPool::note_antigravity_usage`, `src/accounts.rs:1578`) and the dashboard
shows it. The `utilization_5h` / `utilization_7d` fields stay `null` for
Antigravity even when polling works, so the generic 5h/7d columns look empty.
The upstream no longer returns a 5h window (weekly only).

## 3. Root cause: pool not working

**Root cause:** selection is quota-blind, and a 429 cools the whole account for
all model pools. Failover itself works.

Evidence (scratch gateway, six accounts, poller on, `usage_refresh_seconds = 60`;
10 sequential requests: 6 Gemini, then 4 Sonnet):

- All 10 requests returned HTTP 200. Failover works: the first Gemini request
  tried each exhausted account once (429) and was served by a healthy one.
- Per-account counters after the run: `computingjvm`, `tommyshelby`,
  `computing-jvm`, `personal` each `requests_attempted: 1, requests_failed: 1`,
  `available: false`, `cooldown_secs_remaining` 43-49; `keepuntouser`
  `attempted 4`, `cemfi` `attempted 6`, all succeeded.

This shows three defects, in order of impact:

1. **Quota never reaches selection.** `src/auth/antigravity/usage.rs` and
   `note_antigravity_usage` are "display-only" by design. Even with the poller
   running and `remaining = 0` known for four accounts, the pool still tries
   them first. Each cooldown expiry re-spends a request (and a 429 round trip)
   on each dead account.
2. **The 429 cooldown is account-wide and blind.** `cooldown_for_antigravity_error`
   (`src/adapters/gemini/mod.rs:479`) calls `AccountPool::cooldown`
   (`CooldownScope::Account`) with `Retry-After` or a default **60 s**. The 429
   body says "Resets in 161h31m52s", but Google sends no `Retry-After`, so the
   fallback applies. Worse, Gemini and Claude+GPT are separate quota pools
   (`retrieveUserQuotaSummary` groups): `computingjvm`, `computing-jvm` and
   `personal` still have Sonnet quota (1.0 / 1.0 / 0.847, and their single-account
   Sonnet probes returned 200), yet after one Gemini 429 they were skipped for
   Sonnet. In the run, all 4 Sonnet requests went to `keepuntouser`/`cemfi`
   only. This is the shared-pool grouping defect: cooldown and selection key on
   the account, not on the (account, model family) pair. A per-model cooldown
   (`AccountPool::cooldown_model`) already exists but the Antigravity path does
   not use it.
3. **Duplicate identity.** `computingjvm.json` and `computing-jvm.json` both hold
   `computingjvm@gmail.com` (identical quota and reset times). The store gives
   Antigravity accounts no `uuid` (`src/auth/antigravity/store.rs::scan_accounts`),
   so the pool counts them as two accounts. They share one quota and fail together,
   which wastes two attempts per rotation and overstates capacity. This is a
   configuration problem (remove one entry), plus a missing identity check.

Capacity today (from the quota table): Gemini models are servable only by
`keepuntouser` and `cemfi`; Claude + GPT models by five of six accounts
(`tommyshelby` is empty until 2026-10-14T23:03Z). A pool that reads quota and
cools down per model family would use all of them.

Not a cause (checked): rotation among healthy accounts works (6 Gemini requests
served by `cemfi`/`keepuntouser` without error), token refresh and credential
writeback worked, and no auth or 401 failures appeared.

## 4. Quota source for DEC-001: HTTP fix vs `agy /usage`

Observed: `agy -p /usage --output-format json` (agy 1.3.4, needs ≥ 1.1.11) exits 0
with `command.data.groups[].buckets[]` carrying `id`, `window`,
`remaining_fraction`, `reset_time`. Its figures for the `agy`-logged-in account
(`personal`) match the HTTP RPC: Gemini 0, Claude+GPT 0.8474. No quota spent.

| Criterion | HTTP `retrieveUserQuotaSummary` (shunt's own login) | `agy -p /usage` |
| --- | --- | --- |
| Works today | Yes, 6/6 accounts, 200 | Yes, but only for the account `agy` is logged into (1 of 6) |
| Credential | The account's own `antigravity_oauth` token, already refreshed by shunt | `agy`'s own login; one per `agy` home. Per-account needs a separate home/profile per account and a second login each |
| Fix needed | Config only: set `usage_refresh_seconds`. Code: feed quota into selection | New subprocess runner, per-account profile setup, JSON parsing; same selection work |
| Latency / cost | One HTTPS call, ~0.3 s, no process | Spawns a CLI process, ~seconds each; 6 accounts every poll |
| Stability risk | Private, undocumented endpoint; payload changed before (5h dropped) | Also reads the same backend; the CLI JSON is the stable-ish contract, but version-gated and CLI-dependent |
| Operational deps | None beyond shunt | `agy` installed, version ≥ 1.1.11, writable per-account homes, no token writeback conflicts (`agy` refreshes in memory only) |
| Failure mode | Visible HTTP status per account | Process exit and parse errors; hangs need timeouts |
| Orca precedent | n/a | Orca uses it because it has only the CLI login |

**Recommendation:** use the HTTP quota source. It already works for all six
accounts, needs no new dependency and no per-account CLI profiles, and returns the
same numbers as `agy /usage`. Treat `agy /usage` as an optional fallback or
cross-check, not as the primary. The TASK-002 design should therefore be
"enable and harden the HTTP poller and feed it into selection", not "add an `agy`
runner". If DEC-001 still chooses `agy /usage`, it must also decide how to give
each of the six accounts its own `agy` profile, which is the dominant cost.

## 5. Follow-ups for the other tasks (not done here)

- TASK-002/003: set `usage_refresh_seconds` in the live config; feed
  `quota_buckets` into selection; pre-skip accounts with `remaining == 0` for the
  requested model family; cool down per (account, model family) using the reset
  time from the 429 body or the quota summary instead of 60 s.
- Deduplicate accounts by upstream identity (email or reset-time fingerprint);
  remove `computing-jvm` or `computingjvm` from `shunt.toml`.
- Map the weekly-only payload to the generic 5h/7d columns or label them.

# M17 — Antigravity pool usage reporting

## What this is

The admin dashboard shows Live/Cooling correctly for `antigravity_oauth` pool
accounts (reactive 429 tracking), but the Usage column always said "No usage
reported yet": no code path fetched or reported usage for this provider family
at all. This milestone adds that path — a third arm on the background usage
poller (`src/usage_poll.rs`, alongside the Claude and Codex arms) that calls
Google's Code Assist quota RPC and surfaces the result on the pool snapshot.

## The RPC

`POST {provider.base_url}/v1internal:retrieveUserQuota` with the account's
OAuth bearer and a `{"project": "<project_id>"}` body (the same endpoint the
Gemini-CLI local-discovery path in `src/auth/observation.rs` uses, but that
path serves `/admin/observed` and is unrelated to the pool).

Two behaviors are live-tested observations, **not a documented Google API
contract** (same caveat class as the Codex wham endpoint — see
`m10-codex-multi-account.md`):

- The call **requires the Antigravity Hub `User-Agent`**
  (`auth::antigravity::version::user_agent()`). The same token/project gets
  `403 PERMISSION_DENIED / SUBSCRIPTION_REQUIRED` without it and `200 OK` with
  it. `X-Goog-Api-Client` is not required.
- The response is ~28 **per-model** buckets (`claude-opus-4-6-thinking`,
  `gemini-3.5-flash`, `gpt-oss-120b-medium`, ...), each with its own
  `remainingFraction` (0.0–1.0 remaining) and its own `resetTime`. There is no
  account-wide "5h window" / "7-day window" to report.

## Why a new field, not a forced fit

Claude/Codex usage is reported as a `UsageSnapshot` of 5h/7d/7d_oi windows.
Blending ~28 per-model buckets into that shape would invent a fake number tied
to no real model, so the buckets are surfaced accurately instead: a new small
`quota_buckets`-shaped field (`accounts::QuotaBucketSnapshot`, the pool-side
twin of `observation::QuotaBucket` with the same wire/JSON shape) on
`AccountSnapshot`, rendered by the dashboard's existing bar code with only a
data-source change. This is Antigravity's *only* possible quota signal, not a
reconciliation layer on an existing one — unlike the other two arms, there is
no reactive-header baseline to reconcile against.

## Eligibility

Only imported (refreshable) logins are polled: `token_env` credentials are
always static (mirroring `resolve_antigravity_account`'s own refusal),
otherwise the credential file must carry a non-empty top-level
`refresh_token` (the Antigravity store's flat schema). Failures degrade
quietly to a debug log, like the other two arms. No extra backend guard is
needed: `Config::validate` already restricts `AuthMode::AntigravityOauth` to
`kind = "antigravity"` and the two vetted Google hosts.

## Deliberate scope limit: display-only

Buckets are applied wholesale (`note_antigravity_usage` replaces the previous
list — Google's response is authoritative each call) and leave `health.quota`
(5h/7d/aggregate status) untouched. Antigravity pool selection/rotation
behavior is unchanged — matching the `/admin/observed` display-only
precedent. Buckets are also memory-only: they never enter `QuotaState`, so no
`state_persist.rs` migration was needed.

## Operator note

Google's Antigravity terms call third-party OAuth use a breach of agreement,
and this adds a periodic background poller's worth of extra calls per account
on top of real chat traffic. Enabled via `[server.pool]
usage_refresh_seconds`, like the other two families.

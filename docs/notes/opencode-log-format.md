# OpenCode log format for provider status

**Last Updated:** 2026-10-17

---

This note records the shape of the `opencode` CLI log, so a reader can turn a log
tail into a per-model status (reachable / rate limited / unknown). OpenCode has no
quota endpoint: the free tier answers 403 to any client except `opencode` itself.
The log is the only passive signal.

Source file: `~/.local/share/opencode/log/opencode.log` (a single append-only file).
Each line is logfmt (`key=value`), one line per event. Bare values are unquoted;
values with spaces are wrapped in `"`. Timestamps are ISO-8601 UTC with a `Z`.

Sanitized real example lines are in `tests/fixtures/opencode/log-sample.txt`. Opaque
per-run and per-session ids are replaced with `REDACTED`; timestamps, model ids and
error strings are unchanged.

## 1. Error line and success line for `providerID=opencode`

Every request line for the OpenCode provider carries these fields:

- `timestamp` — ISO-8601 UTC, e.g. `2026-10-13T23:53:00.040Z`
- `message` — the event kind: `stream` or `"stream error"`
- `providerID=opencode`
- `modelID` — the bare model id (see section 2)
- `session.id`, `small`, `agent`, `mode` — context, not used by the state rule
- `error.error` — present only on error lines, the quoted provider error text

Error line shape:

```
timestamp=2026-10-13T23:53:00.040Z level=ERROR run=REDACTED message="stream error" providerID=opencode modelID=muse-spark-1.3-contributor-free session.id=ses_REDACTED small=false agent=build mode=primary error.error="AI_APICallError: Rate limit exceeded. Please try again later."
timestamp=2026-10-02T08:37:20.804Z level=ERROR run=REDACTED message="stream error" providerID=opencode modelID=muse-spark-1.3-contributor-free session.id=ses_REDACTED small=false agent=plan mode=primary error.error="AI_APICallError: Error from provider (Console): Rate limit exceeded. Please retry after a brief wait."
timestamp=2026-10-13T16:11:13.631Z level=ERROR run=REDACTED message="stream error" providerID=opencode modelID=muse-spark-1.3-contributor-free session.id=ses_REDACTED small=true agent=title mode=primary error.error="AI_RetryError: Failed after 3 attempts. Last error: Rate limit exceeded. Please try again later."
```

Success line shape:

```
timestamp=2026-10-05T01:53:42.907Z level=INFO run=REDACTED message=stream providerID=opencode modelID=big-pickle session.id=ses_REDACTED small=false agent=build mode=primary
timestamp=2026-10-12T07:58:41.884Z level=INFO run=REDACTED message=stream providerID=opencode modelID=space-bunny-free session.id=ses_REDACTED small=false agent=plan mode=primary
timestamp=2026-10-16T14:32:15.718Z level=INFO run=REDACTED message=stream providerID=opencode modelID=muse-spark-1.3-contributor-free session.id=ses_REDACTED small=false agent=plan mode=primary
```

**The important subtlety.** `message=stream` is written when the provider stream
*starts*, not when it succeeds. When the request then fails, a `message="stream
error"` line for the same model follows within a few seconds. So a `stream` line
alone is an *attempt*. The parser must treat a model as succeeding only when its
newest `stream` line is newer than its newest `stream error` line (section 4).
Reading only `stream` lines as successes would mark a failing model as reachable.

For `providerID=opencode`, every observed request either produces one `stream` line
or fails with one `stream error` line. There is no separate "completed" line.

## 2. Does the log model id equal the id from `opencode models`?

No — the log holds the bare id; `opencode models` prints the provider-qualified id.

- log: `modelID=big-pickle`, `modelID=space-bunny-free`, `modelID=nemotron-3-ultra-free`
- `opencode models`: `opencode/big-pickle`, `opencode/space-bunny-free`, `opencode/nemotron-3-ultra-free`

Mapping: the log `modelID` is the `opencode models` id with the `opencode/` prefix
removed. In the log the provider is the separate field `providerID=opencode`, so the
qualified id is `providerID + "/" + modelID`.

The log keeps ids that `opencode models` no longer lists (retired free models seen
in the tail: `muse-spark-1.3-contributor-free`, `union-alpha`, `mimo-v2.5-free`). A
status reader must join on the bare id and must not assume every logged id is still
in `opencode models`.

## 3. Error strings by class

The provider error text lives in `error.error`. It is wrapped by one of two
prefixes, which the parser must strip or ignore:

- `AI_APICallError: ` — a single failed call
- `AI_RetryError: Failed after 3 attempts. Last error: ` — three attempts failed

| Class | Substring test (case-insensitive) | Real example body | Means "rate limited now"? |
|-------|-----------------------------------|-------------------|---------------------------|
| Rate limit | contains `rate limit` | `Rate limit exceeded. Please try again later.` | **Yes** |
| Rate limit | contains `rate limit` | `Output token rate limit exceeded. Please retry after a brief wait.` | **Yes** |
| Rate limit | contains `rate limit` | `Rate limit exceeded. Please retry after a brief wait.` | **Yes** |
| Overload | contains `overloaded` or `service_overloaded` | `The backend is temporarily overloaded. Please retry.` | No |
| Outage | contains `Endpoint is unavailable` | `Upstream request failed: Endpoint is unavailable.` | No |
| Outage | contains `not valid JSON` | `Upstream response was not valid JSON` | No |
| Outage | contains `internal server error`, `socket hang up`, `timed out`, or a `502`/`503`/`504` code | `Streaming response failed: [504] Provider timed out after 336521ms` | No |
| Country block | contains `not available in your country` | `This model is not available in your country` | No |
| Billing | contains `Insufficient balance` | message embeds a workspace id — never copy it into the repo | No |
| Auth | contains `Invalid credential` | `Upstream request failed: Invalid credential` | No |
| Account eligibility | contains `requires Global regions` or `trains on request data` | privacy-settings message | No |

Only a rate-limit line sets the "rate limited" state. Overload and outage are
transient and are retried; a country block, a billing stop or an eligibility gate are
permanent for this model and this account, but none of them is a rate limit. The
state rule below only reads the rate-limit class and ignores the other classes except
that they block a "reachable" verdict.

Note: `[api_error] upstream provider error (HTTP 429)` also means rate limit, seen on
other providers. The match `429` is not used here because the observed OpenCode free
tier always names the limit in words.

## 4. State rule (reachable / rate limited / unknown)

Run over the tail window of section 5. For each model id `M` that appears on a line
with `providerID=opencode`, collect:

- `last_stream(M)` — the newest `timestamp` on a `message=stream` line for `M`
- `last_error(M)` — the newest `timestamp` on a `message="stream error"` line for `M`
- `rate_limited(M)` — true when the `error.error` text of that newest error line
  contains `rate limit` (case-insensitive), after stripping the `AI_APICallError: `
  or `AI_RetryError: Failed after 3 attempts. Last error: ` prefix

Then, with `now` = current time:

- **reachable** — `last_stream(M)` exists AND (`last_error(M)` is absent OR
  `last_stream(M) > last_error(M)`). Any newer `stream` line clears every older
  error, because the rule compares the two newest timestamps.
- **rate limited** — `last_error(M)` exists, it is newer than `last_stream(M)` (or no
  `stream` line exists), AND `rate_limited(M)` is true. Report the age of the last
  error as `now − last_error(M)`.
- **unknown** — no `providerID=opencode` line for `M` in the window; or the newest
  line is a non-rate-limit error (overload, outage, country block, billing,
  eligibility) with no newer `stream` line.

Other rules the parser must apply:

- Compare only within one model id. Never merge models.
- Ignore `session.id`, `agent` and `small` for the state; they do not change the
  verdict, and a `small=true` `title` request counts like any other.
- A newer success clears an older error (rule above). There is no sticky error.

Worked example from the fixture: `big-pickle` has a rate-limit error at
`2026-09-29T00:52:08Z` and a `stream` line at `2026-10-05T01:53:42Z`; the later
stream is newer, so `big-pickle` is **reachable**. `space-bunny-free` has an outage
error at `2026-09-24T14:47:15Z` and a `stream` line at `2026-10-12T07:58:41Z`, so it
is **reachable** too. `muse-spark-1.3-contributor-free` has a rate-limit error at
`2026-10-13T23:53:00Z`; if no newer `stream` line exists in the window, it is
**rate limited**, age `now − 2026-10-13T23:53:00Z`.

## 5. How much of the tail to read, and is `opencode.db` needed?

**Window: 24 hours.** A rate-limit state older than 24 hours is shown as unknown; the
free tier resets, and an old error says nothing about the present.

Measured 2026-10-17 on the live file: 27.8 MB, 128,497 lines, ≈216 bytes per line.
The last 5,000 lines covered 10.5 hours; the busiest recent day produced ≈12,000
lines (≈2.6 MB) per 24 hours.

Read the **last 5 MB** (≈23,000 lines). That covers 24 hours with margin. To be safe
on a busier machine, repeat: if the oldest line read is younger than `now − 24h`,
read another 5 MB before it, until the oldest line is at least 24 h old or the start
of the file is reached. A simple equivalent is `tail -c 5000000`, then `grep` the
`providerID=opencode` lines and check the first `timestamp`.

`opencode.db` (11 GB) is **not needed**. Every request start and every error is
already in the log with model id, time and outcome. Do not open the database.

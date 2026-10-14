---
id: TASK-010
title: Live failover and ranking test of the agy pool
milestone: M-1
state: approved
depends_on: [TASK-007]
delivery: pr
---

## Why

SC-2 requires the agy pool to rotate and fail over, verified by a live smoke. TASK-004's smoke showed ranking; it did not force a failover through real upstream 429s. This task proves it, per model family.

## Scope

Test only; no production code change unless a test finds a real bug (then record it with `agent-note bug` and report in the outcome; do not fix it here). Use a SCRATCH gateway on another port with a copy of the live accounts (never the launchd gateway or the live `shunt.toml`), `usage_refresh_seconds = 60`, and the five accounts. Today's Gemini-exhausted accounts are computingjvm, personal and tommyshelby; confirm with `shunt pool --json` first and adapt to the live numbers. Keep prompts tiny (a few tokens out); 429s cost no quota.

Cases, each with the request, the account that served it, and the log lines:

1. Failover: set a custom order (priority) that puts the Gemini-exhausted accounts first, send Gemini requests: each exhausted account answers 429 and is cooled down for the Gemini family only, and the request still succeeds on an account with quota.
2. Ranking: balanced ranking, same requests: exhausted accounts are tried last, first request succeeds with no 429.
3. Family isolation: an account exhausted for Gemini still serves Claude/GPT requests, and a Claude/GPT 429 does not cool its Gemini family.
4. Cooldown reset: after the family cooldown, the account is tried again.
5. All exhausted: pause or disable every account with quota left; the request fails with a prompt 429 in the Anthropic error shape (not a hang, not a 5xx).

Out of scope: fixes, selection changes, other providers.

## Acceptance

- `docs/notes/agy-pool-live-test.md` lists the five cases with exact commands, per-request account, HTTP status and the log evidence, and a pass/fail line each.
- Any failing case has an `agent-note bug` and is named in the outcome.
- The live gateway and `~/.config/shunt/shunt.toml` are untouched (state this and show `git diff`/`diff` evidence).

## Verify

`test -s docs/notes/agy-pool-live-test.md && grep -c 'PASS\|FAIL' docs/notes/agy-pool-live-test.md`

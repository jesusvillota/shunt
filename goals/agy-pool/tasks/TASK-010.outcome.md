# TASK-010 outcome

## Acceptance

- [x] `docs/notes/agy-pool-live-test.md` lists the five cases with exact commands, per-request account, HTTP status and log evidence, and a pass/fail line each.
  Evidence: the note has all five. Cases 1 (failover), 2 (ranking), 3 (family isolation) and 4 (cooldown reset) PASS; case 5 (all exhausted) FAILS. The gateway log does not name accounts, so the account is taken from raw `shunt pool --json` snapshots before and after every request, committed in `docs/notes/agy-pool-live-test/` with the literal curl command per request, the mock config and `mock.log`. `test -s docs/notes/agy-pool-live-test.md && grep -c 'PASS\|FAIL' docs/notes/agy-pool-live-test.md` printed `11` on this revision (re-run after the review; the earlier note's count differed).
- [x] Any failing case has an `agent-note bug` and is named in the outcome.
  Evidence: case 5 FAILS. After the exhausted accounts are cooled, the next request gets HTTP 503 `api_error` ("no Antigravity accounts are available") in 2 ms, not a 429. Bug recorded: `~/GitHub/config/agent-inbox/bugs/20261014T092309Z-38734-Antigravity-pool-returns-503-api-error-instead-o.md`. Not fixed here (out of scope).
- [x] The live gateway and `~/.config/shunt/shunt.toml` are untouched.
  Evidence: `shasum ~/.config/shunt/shunt.toml` was `84fb6c69f0c78614a7ca3211e66596d551b7a6ee` before and after. The launchd gateway (pid 57240, started 10:57:13) kept the same pid and start time; `diff` of a pre-test copy against the file printed `IDENTICAL` (`live-before.txt`, `live-after.txt`). `git diff` of this branch shows only `docs/notes/agy-pool-live-test.md` and this file. Scratch gateways ran on 39010/39012 with `HOME=/tmp/t010/home`.

## Summary

Test only; no production code changed. Ran a scratch gateway with a copy of the five accounts, `usage_refresh_seconds = 60`, and real Gemini/Claude requests. Case 4 uses a local mock upstream (live Google cannot be reset-tested; weekly resets are days away) returning 429 "Resets in 15s", because a real weekly cooldown lasts about 6 days. Case 5 is a real failure: only the first request after pausing returns 429; later ones (`c5-r2`, `c5-r3`) return a 503.

Observability gap, not a failing case: failover and per-account 429s are not logged, so attribution needs the admin counters. Proposed follow-up: fix the 503 → 429 fallback in `src/adapters/gemini/mod.rs`, and log account and family on cooldown. Verified revision: branch head at PR time.

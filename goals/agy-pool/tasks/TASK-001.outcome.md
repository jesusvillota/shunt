## Acceptance

- [x] `docs/notes/agy-pool-diagnosis.md` lists, per configured account, whether login, token refresh, quota fetch and a live request work, with the exact error text.
  Evidence: section 1 table (6 accounts); refresh 200 and quota 200 for all; live 429 `Individual quota reached. Please upgrade your subscription to increase your limits. Resets in 161h31m52s.` etc.
- [x] The report names the root cause of the missing quota with evidence.
  Evidence: `[server.pool]` has no `usage_refresh_seconds`, so `spawn_usage_poller` never runs; 0 poller log lines on the live gateway; with `usage_refresh_seconds = 60` all six accounts get `quota_buckets` within 20 s.
- [x] The report names the root cause of the pool not working with evidence.
  Evidence: 4/6 accounts have Gemini weekly remaining 0; selection ignores quota; a 429 cools the whole account for a blind 60 s across both model pools; duplicate identity `computingjvm`/`computing-jvm`. Failover itself works (10/10 requests 200).
- [x] The report recommends HTTP-fix vs `agy /usage` with a trade-off table, for DEC-001.
  Evidence: section 4; recommends HTTP. `agy -p /usage` works for 1 of 6 accounts and matches the HTTP numbers.

## Summary

Diagnosis only; the one added file is `docs/notes/agy-pool-diagnosis.md`. Method: account metadata + standalone refresh/quota script, per-account scratch gateways built from this branch (single account each, live Gemini and Sonnet request), a six-account scratch gateway with the poller on, and `agy -p /usage`. No production code or live config changed. Token refresh through the shared account files may have rewritten them via the normal writeback.

Verified revision: 039c8b0. Not checked: Orca's source (the `agy` JSON was observed directly); `agy /usage` for the other five accounts (they have no `agy` profile).

Proposed follow-ups: TASK-002 should harden the HTTP poller, not add an `agy` runner; TASK-003 should add quota-aware, per-model-family cooldowns and identity dedup; set `usage_refresh_seconds` in `shunt.toml`; remove the duplicate account.

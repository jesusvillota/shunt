---
id: TASK-001
title: Diagnose why the agy pool fails and quota is missing
milestone: M-1
state: approved
depends_on: []
delivery: pr
kind: discovery
---

## Why

Nothing else can be planned until we know why the agy pool fails and quota is missing today. Serves SC-1, SC-2.

## Scope

Reproduce against the live gateway (`~/.config/shunt/shunt.toml`: provider `antigravity`, 6 accounts, `antigravity_oauth`). Read `src/auth/antigravity/{usage,auth,store}.rs`, `src/usage_poll.rs`, `src/accounts.rs`, `src/proxy/failover.rs`, `docs/notes/antigravity-quota-observation.md`. Also read Orca's method: `src/main/rate-limits/antigravity-usage-*.ts` in https://github.com/stablyai/orca (`agy -p /usage --output-format json`; payload `command.data.groups[].buckets[]` with id, window, remaining_fraction, reset_time; needs agy >= 1.1.11; no quota spent). Deliver a report; no production code changes. Out of scope: fixes.

## Acceptance

- `docs/notes/agy-pool-diagnosis.md` lists, per configured account, whether login, token refresh, quota fetch and a live request work, with the exact error text.
- The report names the root cause of the missing quota (e.g. empty `usage_refresh_seconds`, 403, token/project_id resolution) with evidence.
- The report names the root cause of the pool not working (selection, failover, or shared-pool grouping) with evidence.
- The report recommends HTTP-fix vs `agy /usage` quota source, with a trade-off table, for DEC-001.

## Verify

`test -s docs/notes/agy-pool-diagnosis.md && grep -c 'Root cause' docs/notes/agy-pool-diagnosis.md`\nPaste the per-account probe output in the PR.

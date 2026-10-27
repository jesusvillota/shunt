---
id: TASK-008
title: 'Plan: decompose M-3'
milestone: M-3
state: approved
depends_on: [TASK-007, TASK-015]
delivery: pr
kind: plan
---

## Why

Rolling wave: M-3 (two-account pool) is detailed after M-2 is planned. Serves SC-4.

## Scope

Write a plan proposal for M-3: two accounts (`personal`, `cemfi`) in one `chatgptweb` pool with rotation and failover like the existing codex pool, without changing the codex pool. Cover: per-account serialization (one tab per account), what happens when an account hits a web limit, health checks, and a dashboard/`goal pool` entry if useful. Use the Plan proposals format.

## Acceptance

- `proposals/TASK-008.yaml` passes `goal proposal check`.
- The proposal includes an integration test with fake drivers for rotation and failover and a live smoke with two accounts.

## Verify

`goal proposal check goals/chatgptweb-provider/proposals/TASK-008.yaml` exits 0.

---
id: TASK-010
title: 'Plan: decompose M-4'
milestone: M-4
state: cancelled
depends_on: [TASK-007, TASK-009, TASK-015]
delivery: pr
kind: plan
reason: "Goal called off by owner 2026-10-28: ChatGPT web is now driven through Orca's browser via the delegate-chatgptweb skill."
---

## Why

Rolling wave: M-4 (Claude Code path) is detailed after M-2 is planned and the feasibility report exists. Serves SC-5.

## Scope

If TASK-009's verdict is feasible, write a plan proposal for M-4. If not feasible, the proposal contains one task that writes the final report and closes SC-5 as documented-infeasible. Use the Plan proposals format.

## Acceptance

- `proposals/TASK-010.yaml` passes `goal proposal check`.

## Verify

`goal proposal check goals/chatgptweb-provider/proposals/TASK-010.yaml` exits 0.

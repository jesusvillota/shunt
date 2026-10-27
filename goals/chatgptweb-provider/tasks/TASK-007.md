---
id: TASK-007
title: 'Plan: decompose M-2'
milestone: M-2
state: approved
depends_on: [DEC-001]
delivery: pr
kind: plan
---

## Why

Rolling wave: M-2 (tool calls for Codex) is detailed only after DEC-001 is accepted and the live smoke passed. Serves SC-3.

## Scope

Write a plan proposal that decomposes M-2 into tasks and decisions, using the accepted DEC-001 approach (B text protocol, or A). Follow the Plan proposals format in goal-setup `references/format.md`. Do not change the real plan.

## Acceptance

- `proposals/TASK-007.yaml` passes `goal proposal check`.
- The proposal gives each task a Verify made of commands, and includes a demo/fake mode and a live smoke.

## Verify

`goal proposal check goals/chatgptweb-provider/proposals/TASK-007.yaml` exits 0.

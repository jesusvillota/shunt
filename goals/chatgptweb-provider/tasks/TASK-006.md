---
id: TASK-006
title: 'Live smoke: Chat-mode turn with Codex quota unchanged'
milestone: M-1
state: approved
depends_on: [TASK-003, TASK-004, TASK-005, DEC-004, TASK-012]
delivery: pr
kind: gate
---

## Why

Proves the central claim: ChatGPT Web through Codex and shunt, in Chat mode, without spending Codex quota. Serves SC-1, SC-2.

## Scope

A gate. Record the account's Codex quota with `goal pool` (and shunt's usage field named in TASK-001) before. Run one text-only turn from Codex through shunt to the `chatgptweb` model, using the live driver and the signed-in profile. Record the quota after. Also run the negative checks live: put the page in Work mode (or a logged-out profile) and show the driver refuses. If the quota moved, stop and report; do not continue to M-2. Out of scope: tool calls.

## Acceptance

- The PR body contains the `goal pool` output before and after the turn, and the Codex quota did not change.
- A transcript or log shows Codex got an answer from the ChatGPT Web model through shunt.
- A live negative check shows `not_chat_mode` (or `logged_out`) and that nothing was sent.

## Verify

Run the commands in the spec and paste the outputs in the PR and in `tasks/TASK-006.outcome.md`.

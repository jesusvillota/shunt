---
id: TASK-003
title: Build the real Chat-mode driver
milestone: M-1
state: approved
depends_on: [TASK-002]
delivery: pr
---

## Why

The part that talks to ChatGPT. Serves SC-1 and SC-2 (fail closed).

## Scope

Implement the real driver behind the TASK-002 protocol, in the form chosen by DEC-002. One logged-in browser profile per account (`profile_dir`). It must: check the mode is **Chat** before every send and refuse otherwise (`not_chat_mode`); detect logged-out and UI drift and return the matching error instead of guessing; open a fresh chat per turn; send the prompt (use the clipboard paste method, not raw typing, if TASK-001 found typing corrupts input); stream the answer; detect completion without matching a locale-specific button label. No bypass of login, challenges or approvals. Do not read cookies, storage or passwords. Out of scope: tool calls, rotation, the adapter.

## Acceptance

- The driver passes the same conformance test as the fake driver, using a local fixture page that imitates the ChatGPT page (Chat mode, Work mode, logged out, drifted markup).
- On the fixture in Work mode the driver returns `not_chat_mode` and sends nothing.
- The driver never touches cookies, storage or credentials (a test or a grep check shows no such access).
- A short README section explains how to start the driver for one account and where its profile lives.

## Verify

`cargo test chatgptweb` (or the helper's own test command from DEC-002) including the fixture-page tests. Paste the output in the PR. A live check is TASK-006, not this task.

---
id: TASK-005
title: Log in the ChatGPT account profiles
milestone: M-1
state: approved
depends_on: []
delivery: manual
---

## Why

An outside blocker: the driver needs logged-in ChatGPT profiles, and only the owner can sign in. Serves SC-1.

## Scope

The owner signs in to ChatGPT in the driver's browser profile for each account that will be used (start with one: `personal`; add `cemfi` for the pool later). Follow the profile path and steps from TASK-003's README section. No agent signs in or handles passwords. Out of scope: any code.

## Acceptance

- The `personal` profile is signed in and the driver health call reports `ok` (the owner confirms in a comment or with `goal done`).
- The owner confirms the ChatGPT page shows **Chat** mode and the model they want.

## Verify

Owner step. Optional: run the driver health command from TASK-003's README and paste the result.

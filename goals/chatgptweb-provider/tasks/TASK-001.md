---
id: TASK-001
title: Survey the web driver approach and shunt adapter seams
milestone: M-1
state: approved
depends_on: []
delivery: pr
kind: discovery
---

## Why

Nothing else can be planned until we know how a ChatGPT Web driver can work and where it plugs into shunt. Serves SC-1, SC-2 and feeds DEC-001, DEC-002.

## Scope

Read the README of this goal first. Then study (a) https://github.com/miuuyy/codex-chatgpt-web (`docs/architecture.md`, `docs/security-model.md`, `TROUBLESHOOTING.md`, the browser worker source): how it sends a prompt, streams the answer, detects completion, picks the model and mode, and handles login and UI drift; and (b) shunt: `src/adapters/` (especially `antigravity` and `responses`), `src/codex_endpoint*`, `src/pool/`, `src/proxy/failover.rs`, `src/accounts.rs`, `src/config/upstreams.rs`, `src/shutdown.rs`, `src/models_catalog.rs`. Deliver a report only. No production code. Out of scope: building the driver.

## Acceptance

- `docs/notes/chatgptweb-survey.md` exists and has the sections `Driver mechanics`, `Chat vs Work mode`, `Shunt seams`, `Tool-call approaches`, `Risks`.
- `Chat vs Work mode` states how to read the active mode from the page, with evidence, and how to prove that Codex quota did not move (which `goal pool` or usage field to compare).
- `Shunt seams` lists, per file, where a new provider kind, account `profile_dir`, model catalog entry, child-process lifecycle and failover hook in, with `file:line` references.
- `Tool-call approaches` compares A (MCP connector + tunnel) and B (text protocol) with the facts found, and says what each needs from shunt.
- The report lists license and reuse notes for the repo's code (MIT) and the options for a provider name that does not collide with the goal tool's `chatgptweb` worker platform.

## Verify

`test -s docs/notes/chatgptweb-survey.md && grep -c -E '^## (Driver mechanics|Chat vs Work mode|Shunt seams|Tool-call approaches|Risks)' docs/notes/chatgptweb-survey.md` (expect 5).

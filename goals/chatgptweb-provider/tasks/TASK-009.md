---
id: TASK-009
title: Check Claude Code feasibility through shunt
milestone: M-4
state: approved
depends_on: [TASK-001]
delivery: pr
kind: discovery
---

## Why

The owner wants Claude Code support only if it is really feasible. Serves SC-5.

## Scope

Find out whether Claude Code can use the `chatgptweb` provider through shunt's Anthropic side (routing by `model` id, `[[models]]` entries like the antigravity ones). Check: Anthropic Messages format with tools and streaming, thinking blocks, how shunt translates for other providers (see `src/adapters/` and `docs/m16-codex-inbound-translation.md`), what the text protocol of approach B needs in Anthropic format, and the cost in context and latency. Deliver a report with a clear verdict: feasible, feasible with limits, or not feasible, with evidence. No production code.

## Acceptance

- `docs/notes/chatgptweb-claude-code-feasibility.md` exists and ends with a `Verdict` section.
- The verdict names what to build for M-4 or the reasons to stop, with `file:line` references to shunt code.

## Verify

`test -s docs/notes/chatgptweb-claude-code-feasibility.md && grep -c '^## Verdict' docs/notes/chatgptweb-claude-code-feasibility.md` (expect 1).

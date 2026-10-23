---
id: TASK-004
title: Add the chatgptweb provider adapter (text only)
milestone: M-1
state: approved
depends_on: [TASK-002]
delivery: pr
---

## Why

Shunt side of the provider: Codex can ask for a ChatGPT Web model and get a text answer. Serves SC-1.

## Scope

Add the provider kind and adapter in shunt, using the name chosen in DEC-002, following the seams in TASK-001's report. Config: provider table with accounts that have `profile_dir`; no secrets in the config. Translate a Responses request into a prompt (whole transcript, since each turn uses a fresh chat) and the driver events into Responses stream events. Add the models to the catalog served to Codex. **Text only**: if the request contains tools, return a clear error or ignore tools as TASK-001 recommends. Use the fake driver for tests. Do not change the behavior of the existing `codex` provider. Out of scope: tool calls, multi-account rotation, Claude Code.

## Acceptance

- A config example in `shunt.toml.example` shows one `chatgptweb` provider with one account.
- An integration test sends a Responses request through `[server.codex_endpoint]` to the fake driver and gets a valid streamed Responses answer.
- The models appear in the catalog Codex reads.
- Existing tests for the `codex` provider still pass unchanged.

## Verify

`cargo test` (full suite) and `cargo clippy --all-targets -- -D warnings`. Paste the output in the PR.

---
id: TASK-003
title: OpenCode section in shunt tui with reachable / rate-limited status
milestone: M-2
state: approved
depends_on: [TASK-001, TASK-002]
delivery: pr
route: {platform: opencode, model: opencode/deepseek-v4.1-flash-free}
---

## Why

Serves SC-2. The operator wants to know whether the free OpenCode models are reachable or rate limited, without opening the opencode TUI.

## Scope

Add an OpenCode section to `shunt tui` (build on the Models code merged in TASK-001: `src/models_catalog.rs`, `src/tui/models.rs`, `src/tui/view/models.rs`) following the rule in `docs/notes/opencode-log-format.md` (TASK-002).

- A pure parser, for example `src/opencode_status.rs`: input is the log tail text and the current time, output is one state per model: reachable, rate limited (with the age of the last error) or unknown. Test it with `tests/fixtures/opencode/log-sample.txt`.
- The TUI section lists every OpenCode model from `shunt models opencode` (live), in that order, with its state and the age of the last event. A model switched off in the Models dialog is shown dimmed. The section also shows one summary line: how many models are reachable.
- The section is **local only**: it reads this machine's log in a background blocking task and refreshes about every 10 seconds. It does not call the gateway or the admin API. Read only the tail the note says is enough.
- If the log is missing or unreadable, show `unknown` for every model and one short reason. Never fail the TUI.
- Update the docs in the same PR: README (+ko/ja/zh-CN), `docs/pool-tui.md`, site `reference/cli.mdx` or the guide, with ko/ja/zh-cn copies. Do not hand-edit `wiki/`.

Out of scope: an active probe that sends a real `opencode run` request; ranking (TASK-004); any change to gateway routing or the daemon.

## Acceptance

- The parser returns the expected state for every fixture case: a recent rate-limit error, an old error cleared by a later success, only successes, and a model absent from the log.
- A render test shows the OpenCode section with the three states and the dimmed off model.
- With no log file the TUI still renders and shows `unknown`.
- A one-time live check is pasted into the outcome: an ignored test (or a documented command) that runs the parser on the real log and prints the state of each model.
- fmt, clippy (`--features tui,bench`) and the full tests pass; the docs are updated in all four languages.

## Verify

```sh
cargo fmt --all --check
RUSTFLAGS="-D warnings" cargo clippy --all-targets --features tui,bench -- -D warnings
RUSTFLAGS="-D warnings" cargo test --features tui,bench --workspace
cargo test --features tui --lib opencode_status
```

---
id: TASK-004
title: Give each goal session its goal's shunt client token
milestone: M-1
state: approved
depends_on: []
delivery: pr
repo: goal
route: {platform: codex, model: gpt-6.1-sol, effort: xhigh}
---

## Why

Serves SC-6. shunt tells goals apart only by the client token each session sends. Today the `goal` CLI hands no token to its sessions, so every session looks the same to shunt.

## Scope

Work in the `goal` repo (`~/GitHub/goal`, Rust crate `tools/goal-rust/goal-core`). Facts:

- `worker_command` (`goal-core/src/dispatch.rs:1275-1305`) builds the `env GOAL_AGENT=1 GOAL_PLAN_ROOT=… <argv>` prefix used by the Orca and herdr launchers; tmux (`:1498-1518`), headless (`:1461-1488`) and print (`:1415-1423`) build their own env. The interactive orchestrator writes `start.sh` (`session.rs:528-603`).
- Claude Code sends `ANTHROPIC_CUSTOM_HEADERS` as extra headers; shunt's dedicated `x-shunt-token` header wins over the `apiKeyHelper` Bearer token, so the owner's `apiKeyHelper` keeps working.

Work:

1. Token source: macOS Keychain generic password with service `shunt-goal-<slug>` (account `$USER`). The client name in shunt's `SHUNT_CLIENT_TOKENS` is the goal slug. No Keychain item means no header (the session uses the owner's default token and all accounts, as today).
2. Every launcher (headless, tmux, orca, herdr, print and the orchestrator `start.sh`) gives Claude sessions `ANTHROPIC_CUSTOM_HEADERS="x-shunt-token: <token>"` and Codex sessions `SHUNT_GOAL_TOKEN=<token>`. The token must never appear in stored command text, Orca terminal titles, logs, run records or the clipboard: resolve it in the launched shell at start time (for example `$(security find-generic-password -s shunt-goal-<slug> -w)`), or in-process for headless.
3. Codex: document that `~/.codex/config.toml` `[model_providers.shunt]` needs `env_http_headers = { "x-shunt-token" = "SHUNT_GOAL_TOKEN" }`; if goal's per-run `CODEX_HOME` copy (`codex_worker.rs:25-28`) can add it safely, do that instead.
4. Agy model discovery (`catalog.rs:132-133`) sends `x-api-key: local`, which shunt rejects once `[server.auth]` is on. Send the goal's token when one exists, else the output of the Claude `apiKeyHelper` from `~/.claude/settings.json` when set, else keep `local`.
5. Tests for each launcher's env text (no token literal in the command), and docs in the goal repo's existing README/docs section on dispatch.

Out of scope: shunt changes, the keeper (TASK-005).

## Acceptance

- Tests show each launcher's command references the Keychain lookup for `shunt-goal-<slug>` and contains no token literal.
- A test shows that without a Keychain item no `x-shunt-token` header or `SHUNT_GOAL_TOKEN` is set.
- A test shows agy discovery sends the goal token when present.
- The goal repo's full test suite and its lint pass (the commands in its AGENTS.md or README).

## Verify

```sh
cd tools/goal-rust && cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace
```

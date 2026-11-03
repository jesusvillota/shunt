---
id: TASK-007
title: Turn on per-goal pinning on this Mac
milestone: M-1
state: done
depends_on: [TASK-004, TASK-005, TASK-006, TASK-008, TASK-009, TASK-010]
delivery: manual
reason: 'one-time setup applied: [server.auth] with tokens_file, default client, Codex auth command, shunt check ok, restarted; pin/clear tested with pool --client'
---

## Why

Serves the milestone exit. The feature only acts once the owner's gateway has `[server.auth]` and a `default` client. This touches live config and secrets, so the owner does it (an interactive copilot may help). After this one-time step the owner chooses each goal's accounts in the goal TUI Policy tab (TASK-010).

## Scope

1. Install the new shunt and goal builds.
2. Put the token that `~/.local/bin/claude-shunt-token` (the Claude `apiKeyHelper`) prints into `SHUNT_CLIENT_TOKENS` as client `default`, so ordinary sessions keep working.
3. Add `[server.auth]` with `tokens_file` to `~/.config/shunt/shunt.toml`, and `env_http_headers` for Codex in `~/.codex/config.toml`.
4. Run `shunt check`, restart shunt, open the goal TUI Policy tab for one goal, switch accounts, save, and dispatch one task of that goal.

## Acceptance

- `shunt check` prints `config ok`.
- An ordinary Claude Code session still works.
- A pinned goal's session responses carry `x-shunt-account` values only from the accounts chosen in the Policy tab (shunt log or `shunt pool --json --client <slug>`).

## Verify

`shunt check`, then `shunt pool --json --client <slug>` while a pinned goal runs.

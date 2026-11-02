---
id: TASK-007
title: Turn on per-goal pinning on this Mac
milestone: M-1
state: approved
depends_on: [TASK-004, TASK-005, TASK-006, TASK-008, TASK-009, TASK-010]
delivery: manual
---

## Why

Serves the milestone exit. The feature only acts once the owner's gateway has `[server.auth]`, client tokens and subsets. This touches live config and secrets, so the owner does it (an interactive copilot may help).

## Scope

1. Install the new shunt and goal builds.
2. Put the token that `~/.local/bin/claude-shunt-token` (the Claude `apiKeyHelper`) prints into `SHUNT_CLIENT_TOKENS` as client `default`, so ordinary sessions keep working.
3. For each pinned goal, create a token, add `<slug>:<token>` to `SHUNT_CLIENT_TOKENS`, and store it in the Keychain as service `shunt-goal-<slug>`.
4. Add `[server.auth]` and `[server.auth.clients.<slug>].accounts.<provider>` to `~/.config/shunt/shunt.toml`, and `env_http_headers` for Codex in `~/.codex/config.toml`.
5. Run `shunt check`, restart shunt, and dispatch one task of a pinned goal.

## Acceptance

- `shunt check` prints `config ok`.
- An ordinary Claude Code session still works.
- A pinned goal's session responses carry `x-shunt-account` values only from its subset (checked in the shunt log or with `shunt pool --client <slug>`).

## Verify

`shunt check`, then `shunt pool --json --client <slug>` while a pinned goal runs.

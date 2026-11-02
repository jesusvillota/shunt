---
id: TASK-008
title: shunt reads client tokens from a file on reload
milestone: M-1
state: approved
depends_on: []
delivery: pr
repo: shunt
route: {platform: chatgptweb}
---

## Why

Serves SC-6 and the owner's wish to pin a goal without restarting shunt. A client token is read from the env var `SHUNT_CLIENT_TOKENS`, which a running process cannot change. A file that shunt re-reads on every config reload lets the goal CLI add a goal's token while shunt runs.

## Scope

Work in the `shunt` repo. Facts: `[server.auth]` has `tokens_env` (`src/config.rs:406`); `resolve_inbound_auth` (`src/config.rs:529`) reads that env var; `src/reload.rs:76` re-resolves inbound auth on every reload (SIGHUP and config-file watch).

1. Add the optional key `tokens_file` to `[server.auth]`: a path, `~` expanded. Each non-empty line is `<client>:<token>`, the same format as `tokens_env`. Entries from the file and from `tokens_env` merge; a client name or token that appears twice is a validation error that names both sources.
2. `shunt check` and reload read the file. A missing file is an empty list, not an error. A file that other users can read (mode wider than 0600) is a `shunt check` error.
3. The reload watcher must also fire when `tokens_file` changes, not only `shunt.toml`. Confirm that a new line takes effect on a running gateway with no restart.
4. Tests: merge of file and env tokens, duplicate rejection, missing file, wide mode, and a reload test that adds a token line and sees the new client authenticate.
5. Docs: `docs/config.md`, `site/src/content/docs/reference/config.md` and its `ko`/`ja`/`zh-cn` copies, and the README auth section if it lists `tokens_env`.

Out of scope: any change to client account subsets or to the admin API.

## Acceptance

- A running gateway accepts a token appended to `tokens_file` after the next reload, with no restart (test output).
- Duplicate client names or tokens across the file and `tokens_env` fail `shunt check`, naming both sources.
- A `tokens_file` with mode wider than 0600 fails `shunt check`.
- `cargo fmt --all --check`, `cargo clippy --all-targets --all-features -- -D warnings` and `cargo test --all-features --workspace` pass.

## Verify

```sh
cargo test --all-features --test check_cli
cargo test --all-features reload
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features --workspace
```
(If the worktree lacks `ui/dist`, run `npm ci && npm run build` in `ui/` first.)

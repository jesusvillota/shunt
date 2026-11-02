---
id: TASK-003
title: "shunt pool --client: show only a client's accounts"
milestone: M-1
state: approved
depends_on: [TASK-001]
delivery: pr
repo: shunt
route: {platform: codex, model: gpt-6.1-sol, effort: xhigh}
---

## Why

Serves SC-5. The goal keeper reads `shunt pool --json` to find when a platform's accounts reset (`~/GitHub/goal/tools/goal-rust/goal-core/src/pool.rs:89` `reset_time`, and `exhausted` below it). It looks at all accounts of a platform. A goal pinned to C would see A and B free and never sleep. The keeper needs a view of only its goal's accounts.

## Scope

- Add `--client <name>` to `shunt pool` (`src/pool/mod.rs`, `Options` at `:8`) and a matching query parameter on `GET /admin/api/pool` (find the handler in `src/admin/`). With it, each provider lists only the accounts in that client's `[server.auth.clients.<name>].accounts.<provider>` subset (TASK-001's accessor). A provider with no entry for that client lists all accounts. An unknown client name lists all accounts (same as "no entry"), so the flag never fails a caller.
- Keep the JSON shape unchanged (only fewer accounts), so the keeper's parser keeps working.
- Tests: an admin-API test and a CLI test (`tests/pool_cli.rs`) with a subset config.
- Docs, same PR: `site/src/content/docs/reference/cli.mdx` (`shunt pool --client`) and the admin endpoint in `site/src/content/docs/reference/endpoints.md`, each with its `ko`, `ja`, `zh-cn` copy if that copy exists; `docs/pool-tui.md` if it documents `shunt pool` flags.

Out of scope: the TUI pool view, the goal repo (TASK-005 uses the flag).

## Acceptance

- `cargo test --all-features --test pool_cli` passes with a case where `--client goal-y` lists only account C under `anthropic`, and no flag lists A, B, C.
- An admin API test shows `?client=goal-y` filters the same way and an unknown client lists all accounts.
- `cargo fmt --all --check`, `cargo clippy --all-targets --all-features -- -D warnings` and `cargo test --all-features --workspace` pass.

## Verify

```sh
cargo test --all-features --test pool_cli
cargo test --all-features --test admin_surface
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features --workspace
```

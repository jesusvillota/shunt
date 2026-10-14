---
id: TASK-007
title: Add shunt pool CLI command
milestone: M-1
state: approved
depends_on: [TASK-006]
delivery: pr
---

## Why

Shunt has no non-interactive way to read the account pool: `shunt tui` needs a terminal, and `goal pool` (in the config repo) re-implements the admin API call and omits Antigravity quota. A first-class `shunt pool` gives users, scripts and agents one correct source. Serves SC-1.

## Scope

Add `shunt pool [--json]` (clap subcommand in `src/main.rs`, implementation in its own module, not behind `--features tui`).

- Auth and URL: reuse the same admin-token and gateway-URL resolution as `shunt tui` (`--token`, `SHUNT_ADMIN_TOKEN`, `SHUNT_ADMIN_TOKENS`, `~/.shunt/admin-token`, configured bind). Read-tier is enough; the command only issues `GET /admin/api/pool`.
- Default output: plain text, one section per provider, one line per account: name, availability (ok, near-quota, paused, disabled, needs relogin), 5h and 7d utilization with resets, cooldown, and, for Antigravity, each reported `quota_buckets` entry (label, remaining, reset) as TASK-006 renders them. Reuse TASK-006's quota formatting; do not duplicate it.
- `--json`: print the gateway's pool snapshot unchanged, stable for scripts.
- Errors: unreachable gateway or missing/refused token exits non-zero with a one-line message, never a stack trace.
- Docs: `site/src/content/docs/reference/cli.mdx` with ko/ja/zh-cn copies, README CLI mention if one exists, `docs/pool-tui.md`.

Out of scope: changing `goal pool` in the config repo (the owner or copilot switches it to call `shunt pool --json` after this merges), mutations (pause, ranking), the TUI.

## Acceptance

- Tests: text rendering for an Anthropic account (5h/7d), a Codex account, and an Antigravity account with weekly-only `quota_buckets`; `--json` passes the snapshot through; missing token and unreachable gateway give a clean non-zero error.
- Live: against a running gateway (scratch config), `shunt pool` and `shunt pool --json` output pasted in the PR, showing Antigravity quota per account.
- `cargo fmt --all --check`, `cargo clippy --all-targets --all-features -- -D warnings` and `cargo test --all-features --workspace` pass; the default build (no `tui` feature) also builds and exposes the command.

## Verify

`cargo fmt --all --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all-features --workspace && cargo build --release && ./target/release/shunt pool --help`

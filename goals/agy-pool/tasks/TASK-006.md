---
id: TASK-006
title: Show Antigravity quota buckets in shunt tui and goal pool
milestone: M-1
state: approved
depends_on: []
delivery: pr
---

## Why

SC-1 says quota must show in the admin/TUI. `GET /admin/api/pool` already returns `quota_buckets` per Antigravity account (live: Gemini and Claude+GPT weekly `remaining` and `reset_time`), but `shunt tui` renders only `utilization_5h`/`utilization_7d`, so every Antigravity row shows `—`.

## Scope

Make `shunt tui` (`src/tui/model.rs`, `src/tui/view.rs`, and the pool client parsing) read `quota_buckets` and show each Antigravity account's per-family quota in its row, e.g. the 5h and 7d columns filled from the matching bucket windows, or a compact "Gemini 0% · Claude+GPT 100%" with reset time where a window is not reported. Only show windows upstream reports; never invent 5h values (DEC-002). Keep Claude and Codex rows unchanged. Also check the web dashboard pool view (`ui/`) and fix it only if it has the same gap and the fix is small. Update docs (`site/` pool-account-controls or tui page with ko/ja/zh-cn copies, `docs/pool-tui.md`). Out of scope: `goal pool` (lives in the config repo, not here), selection logic, poller.

## Acceptance

- Unit and view tests: an account with only weekly `quota_buckets` renders its per-family percentages and reset in the TUI model/view; Claude and Codex rows render as before.
- A live check against a running gateway (scratch config with `usage_refresh_seconds = 60`, or the live one): capture `shunt tui` output or a rendered snapshot showing the Antigravity accounts with quota.
- `cargo fmt --all --check`, `cargo clippy --all-targets --all-features -- -D warnings` and `cargo test --all-features --workspace` pass.

## Verify

`cargo fmt --all --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all-features --workspace`

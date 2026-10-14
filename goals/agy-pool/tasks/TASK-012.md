---
id: TASK-012
title: Return 429 with the reset time when every agy account is exhausted
milestone: M-1
state: approved
depends_on: [TASK-010]
delivery: pr
---

## Why

TASK-010 case 5 FAILED (`docs/notes/agy-pool-live-test.md`, PR #18; bug note "Antigravity pool returns 503 api_error instead of 429 once every account is cooled or paused"). When every Antigravity account is cooled down or paused, the first request gets a correct 429 (it tried each account), but every later request gets an instant `503 api_error "no Antigravity accounts are available"`. A 429 means "quota used up, wait until the reset"; clients handle it. A 503 means "server broken" and invites blind retries. Serves SC-2.

## Scope

In `src/adapters/gemini/mod.rs` (the fallback at the end of the account loop, about line 460, `StatusCode::SERVICE_UNAVAILABLE`) and the pool selection in `src/accounts.rs`/`src/auth/antigravity/pool.rs` as needed:

- When no account is attemptable because accounts are in cooldown (for the requested model family), return `429` with an Anthropic-shaped `rate_limit_error` and a `Retry-After` header equal to the EARLIEST cooldown end among the cooled accounts for that family; the message should say when it resets, like the first 429 does.
- Paused or disabled accounts (operator action) are not quota exhaustion. If the only reason there is no account is that all are paused or disabled or none is configured, keep a non-429 error (503) but make the message say so ("all Antigravity accounts are paused or disabled"). If some are cooled and some are paused, the cooled ones decide: 429 with the earliest reset among the cooled accounts.
- Keep per-family isolation: only the requested family's cooldowns count.
- Keep gateway-owned errors in the Anthropic error shape (AGENTS.md).

Out of scope: selection order, quota polling, other providers, the TUI.

## Acceptance

- Gateway tests (extend `tests/antigravity_pool_rotation.rs`): (a) all accounts cooled for the family: second and later requests return 429 `rate_limit_error` with `Retry-After` equal to the earliest reset, answered without any upstream call; (b) all paused: non-429 with the paused/disabled message; (c) mix of cooled and paused: 429; (d) accounts cooled for Gemini only: a Claude/GPT request is still served.
- A live check on a scratch gateway (never the launchd gateway or `~/.config/shunt/shunt.toml`): repeat TASK-010 case 5 and paste the status, body and `Retry-After` of the first and second requests. Update `docs/notes/agy-pool-live-test.md` case 5 to PASS with the new evidence, or leave a dated addendum if that file is not yet on main.
- Docs: the Antigravity provider pages in en, ko, ja and zh-cn and `docs/m17-antigravity-usage.md` state the new behavior.
- `cargo fmt --all --check`, `cargo clippy --all-targets --all-features -- -D warnings` and `cargo test --all-features --workspace` pass.

## Verify

`cargo fmt --all --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all-features --workspace`

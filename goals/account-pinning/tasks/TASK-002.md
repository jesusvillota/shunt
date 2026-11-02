---
id: TASK-002
title: Keep the client's account subset on router side calls and block forged scope headers
milestone: M-1
state: approved
depends_on: [TASK-001]
delivery: pr
repo: shunt
route: {platform: claude, model: opus, effort: high}
---

## Why

Serves SC-4. A goal must not be able to widen its subset. TASK-001 reads the client from `x-shunt-inbound-client`, but router side calls drop it, so a goal using a router model id would draw from all accounts.

## Scope

Code facts (re-check line numbers):

- Router judge calls build `InboundContext::internal()` (`src/routing/serve.rs:178`) and `judge_headers` strips every reserved slot (`src/routing/serve.rs:384`). `InboundContext::internal` deliberately carries no client so the caller is not attributed for a call it did not make (`src/proxy/failover.rs:797-823`).
- `AdmittedContext` keeps the caller's `InboundContext` (`src/routing/serve.rs:96-120`).

Work:

1. Add a reserved header `x-shunt-account-scope` to `RESERVED_SLOTS` (`src/auth/slots.rs:177`) and update the slot table in that module's doc comment. Stamp it in `headers_for_route` (`src/proxy/failover.rs:927`) whenever a client is known (static token, JWT, or gateway login), separate from the attribution stamp `x-shunt-inbound-client`.
2. Give judge side calls the caller's client for account scope only (not attribution): take it from `AdmittedContext`.
3. Switch TASK-001's filter helper to read `x-shunt-account-scope`.
4. Make sure no upstream request carries `x-shunt-account-scope` (assert it in a test).
5. Tests: in `tests/client_account_pinning.rs`, goal-x sends forged `x-shunt-inbound-client: goal-w` and `x-shunt-account-scope: goal-w` and still reaches only A/B; in the `src/routing/serve` tests (or `tests/router_judge.rs`), a judge call made for goal-y uses only C.
6. Docs, same PR: `docs/m4-inbound-auth.md` (trust boundary: the subset follows the verified token; forged headers are stripped; on one OS user, agents can still read other tokens, so this guards against mistakes, not a hostile agent). Update the site page that TASK-001 added the subset section to, in all 4 languages, only if its wording changes.

Out of scope: config shape, `shunt pool`, the goal repo.

## Acceptance

- The forgery test passes: goal-x still reaches only A/B tokens.
- The router test passes: a judge call for goal-y reaches only C.
- A test shows the upstream request has no `x-shunt-account-scope` header.
- `cargo fmt --all --check`, `cargo clippy --all-targets --all-features -- -D warnings` and `cargo test --all-features --workspace` pass.

## Verify

```sh
cargo test --all-features --test client_account_pinning -- --nocapture
cargo test --all-features --test router_judge
cargo test --all-features --lib routing::serve
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features --workspace
```

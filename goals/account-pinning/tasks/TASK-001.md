---
id: TASK-001
title: 'Pin clients to account subsets: config, validation and filter on every pooled adapter'
milestone: M-1
state: approved
depends_on: []
delivery: pr
repo: shunt
route: {platform: claude, model: opus, effort: high}
---

## Why

Serves SC-1, SC-2 and SC-3. This is the core feature: a client (one goal) draws only from a configured subset of a platform's accounts.

## Scope

Read the goal README first ("Design" section). Code facts (line numbers as of `main` at goal creation; re-check them):

- The caller's verified name reaches adapters in the `x-shunt-inbound-client` header. `check_inbound_auth` (`src/proxy/failover.rs:831`) strips every caller-sent `x-shunt-*` header (`src/auth/slots.rs:177-180`, `:274`); `headers_for_route` (`src/proxy/failover.rs:1003-1009`) stamps the verified name. So this header is trusted inside adapters.
- Account order comes from `AccountPool::select_order*` (`src/accounts.rs:860-928`). Health is keyed per account identity (`account_key`, `src/accounts.rs:3055`), so goals that share an account share its cooldown and quota state with no extra code.

Work:

1. **Config** (`src/config.rs`, `InboundAuthConfig` at `:396`): add `clients: BTreeMap<String, ClientPolicy>` where `ClientPolicy { accounts: BTreeMap<String /*provider*/, Vec<String /*account name*/>> }`. TOML: `[server.auth.clients.goal-x]` then `accounts.anthropic = ["A", "B"]`. The key is the client name (the static token's name from `SHUNT_CLIENT_TOKENS`, or a verified JWT email). Add an accessor such as `Config::client_accounts(client, provider) -> Option<&[String]>`. Absent table or absent entry means all accounts (unchanged behaviour).
2. **Validation** in `Config::validate`, so both `shunt check` and `shunt run` fail: empty `accounts` map; empty list (message: remove the entry to allow all accounts); unknown provider; provider without an account pool (not `claude_oauth`, `chatgpt_oauth`, `kimi_oauth` or antigravity OAuth); an account name not in that provider's configured accounts, and when another provider has that name, say so ("account 'X' belongs to provider 'codex'"). For a provider that scans the account store (no inline `accounts`), `shunt check` scans the store offline, like the existing Antigravity probe in `check` (`src/main.rs:914-936`).
3. **Filter**: one helper in `src/accounts.rs` that keeps only allowed indices of a `select_order*` result (filter the returned order, not the input slice: `sync_enabled_accounts`, `src/accounts.rs:829`, feeds the provider-wide utilization metric). It cancels a re-probe reservation whose account it removed. Apply it before `candidates = order.len()` at every production call site: `src/adapters/anthropic/mod.rs:196` (Claude) and `:851` (Kimi), `src/adapters/gemini/mod.rs:209`, `src/adapters/responses/mod.rs:636`, `src/adapters/responses/pool.rs:51`, `:323`, `:984`, `:1029`, `src/adapters/responses/inbound.rs:152`. Check `src/reload.rs:634` and say in the PR whether it needs the filter. The inbound Codex endpoint does not stamp the header: pass its authenticated client (`src/codex_endpoint.rs:176`, `:361`, `:451`) into `forward_codex_inbound` explicitly.
4. **Exhausted subset: fail, never spill over** (owner decision). Cooling accounts stay in the order and are tried, as today. If all reject, the existing path relays the last upstream 429 with `Retry-After` (`src/adapters/anthropic/mod.rs:745-772`). If the filter leaves nothing (all allowed accounts disabled or paused, or no name matched), return a 503 that names the client, the provider and the allowed accounts. A configured `[[routes]]` fallback to another provider still applies, with that provider's subset.
5. **Tests**: new `tests/client_account_pinning.rs`, reusing the wiremock harness style of `tests/multi_account.rs` (token-env accounts) and `tests/inbound_auth.rs` (`with_inbound_auth`). Three accounts A, B, C with distinct tokens; inbound tokens goal-x, goal-y, goal-z, goal-w; subsets X={A,B}, Y={C}, Z={A,C}, W none. Each goal sends at least 6 requests without a session id, with its own `x-shunt-token`. Assert the set of `Authorization` values the upstream received per goal and the `x-shunt-account` response header. Add the exhausted case: C answers a quota 429 (`anthropic-ratelimit-unified-status: rejected`); goal-y gets 429 and the upstream gets zero requests with A's or B's token from goal-y; a goal-w request afterwards still succeeds. Add the rejected-config cases to `tests/check_cli.rs`.
6. **Docs, same PR** (AGENTS.md "Documentation"): `README.md` feature table row plus `README.ko.md`, `README.ja.md`, `README.zh-CN.md`; `docs/m4-inbound-auth.md` new section; `site/src/content/docs/reference/configuration.md` (`[server.auth.clients]`) and `site/src/content/docs/guides/shared-gateway.mdx`, each with its `ko`, `ja`, `zh-cn` copy. Follow the anchor rule in AGENTS.md. Never hand-edit `wiki/`.

Out of scope: router side calls and the separate scope header (TASK-002), `shunt pool --client` (TASK-003), any `goal` repo change, credential writeback (do not touch it).

## Acceptance

- `cargo test --all-features --test client_account_pinning` passes and shows goal-x received only A/B tokens, goal-y only C, goal-z only A/C, goal-w all three.
- The exhausted-subset test passes: goal-y gets HTTP 429 with `retry-after`, and zero upstream requests from goal-y carry A's or B's token.
- `cargo test --all-features --test check_cli` passes with cases for empty map, empty list, unknown provider, provider without pool, unknown account, and an account that belongs to another provider (message names that provider).
- A config without `[server.auth.clients]` behaves as before: the full existing suite passes.
- `cargo fmt --all --check`, `cargo clippy --all-targets --all-features -- -D warnings` and `cargo test --all-features --workspace` pass.
- `git diff --name-only origin/main...HEAD` lists the README (4 languages), `docs/m4-inbound-auth.md` and the two site pages in 4 languages each.

## Verify

```sh
cargo test --all-features --test client_account_pinning -- --nocapture
cargo test --all-features --test check_cli
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features --workspace
git diff --name-only origin/main...HEAD
```

Goal-Task: account-pinning/TASK-001

## Summary

**What.** A client can now be pinned to a subset of each pooled provider's accounts:

```toml
[server.auth.clients.goal-x]
accounts.anthropic = ["a", "b"]
```

The key is the client name (a static token's name from `tokens_env`, or a verified `[[server.auth.jwt]]` email). A client without an entry, or a provider not listed in an entry, uses every account. Without the table, behaviour is unchanged.

**How.**
- `src/config/clients.rs`: `ClientPolicy`, `InboundAuthConfig::clients`, `Config::client_accounts(client, provider)`, and `Config::validate_client_accounts`. `Config::validate` calls it with the pool names the config fixes (inline `accounts` + `account_scope`); `shunt check` (`src/main.rs`) calls it again with `ProviderConfig::pool_account_names`, which also reads store-backed pools offline. Naming the provider that owns a misplaced account reads the stores on the error path only, so `run` and `check` give the same message. `ProviderConfig::account_store` now holds the per-auth store table that `resolve_pool_accounts` used inline, so both share it.
- `src/accounts/pinning.rs`: `inbound_client(headers)` reads the trusted `x-shunt-inbound-client` stamp; `retain_client_accounts` filters the order returned by `select_order*` (not the input slice), drops a re-probe reservation whose account it removed (its `Drop` cancels it), and returns a `503 api_error` naming the client, provider and accounts when the pinned order is empty. The 503 is marked `AdapterFailure::UpstreamStatus(503)` so the failover chain still tries a `[[routes]]` fallback and returns this 503 when none is left.
- Applied before `candidates = order.len()` at every production site: Claude and Kimi (`src/adapters/anthropic/mod.rs`), Antigravity (`src/adapters/gemini/mod.rs`), Codex chain attempt (`src/adapters/responses/mod.rs`), Codex pool: committed stream, non-WebSocket and WebSocket paths (`src/adapters/responses/pool.rs`), and the inbound Codex endpoint (`src/adapters/responses/inbound.rs`). The Responses adapter carries the client in `TurnHints` → `PoolForward` / `PoolForwardStream`. The inbound Codex endpoint passes its authenticated client explicitly: `post` / websocket `turn` → `forward_turn` → `dispatch_routed` / `forward_codex_inbound`.
- `src/reload.rs:603` (spec's `:634`) is a call inside the `reload_keeps_pool_request_counters` unit test, not a request path, so it needs no filter.
- Exhausted subset: cooling accounts stay in the order and are tried, so the existing path relays the last upstream 429 with `Retry-After`; no spill-over.

**Deviations.**
- The exhausted-subset test sends `anthropic-ratelimit-unified-5h-status: rejected` beside `anthropic-ratelimit-unified-status: rejected`. The aggregate header alone classifies as a throttle (`PauseSame`: sleep `retry-after`, retry the same account), not a quota rotate; a real quota 429 carries both.
- Two functions gained an 8th argument; they use `#[allow(clippy::too_many_arguments)]` like `dispatch_routed` beside them.
- Added a commented `[server.auth.clients]` example to `shunt.toml.example` next to the other `[server.auth]` examples.

**Verified revision:** `e9c19877` (all commands below ran on it).

**Proposed follow-ups.**
- When a Codex or Antigravity pool resolves to zero accounts, the adapters fall back to the single default credential (`~/.codex/auth.json`, `antigravity-auth.json`) for every client, including a pinned one. `shunt check` rejects a pin whose store has no such account, so this needs a store emptied after startup. A pinned client could get the 503 there too.
- `[server.gateway]` login sessions are not stamped into `x-shunt-inbound-client` (unchanged gate behaviour), so they cannot be pinned; only `[server.auth]` tokens and JWTs can.

## Acceptance

- [x] `cargo test --all-features --test client_account_pinning` passes and shows goal-x received only A/B tokens, goal-y only C, goal-z only A/C, goal-w all three.
  Evidence (`-- --nocapture`, accounts named `a`/`b`/`c` because account names must match `[a-z0-9-]+`; 6 session-less requests per client):
  ```
  goal-x: upstream received tokens of {"a", "b"}, x-shunt-account {"a", "b"}
  goal-y: upstream received tokens of {"c"}, x-shunt-account {"c"}
  goal-z: upstream received tokens of {"a", "c"}, x-shunt-account {"a", "c"}
  goal-w: upstream received tokens of {"a", "b", "c"}, x-shunt-account {"a", "b", "c"}
  test result: ok. 3 passed; 0 failed
  ```
- [x] The exhausted-subset test passes: goal-y gets HTTP 429 with `retry-after`, and zero upstream requests from goal-y carry A's or B's token.
  Evidence (`an_exhausted_subset_relays_the_429_instead_of_spilling_over`; it asserts every goal-y upstream request carries `c`'s token):
  ```
  goal-y: HTTP 429 Too Many Requests, retry-after Some("30"), upstream received tokens of ["c"]
  goal-w afterwards: HTTP 200 OK, upstream received tokens of ["b"]
  ```
  `a_pinned_client_with_no_selectable_account_gets_a_503` covers the empty-subset 503 (message names `'goal-y'`, `[c]`, `'anthropic'`; zero upstream requests).
- [x] `cargo test --all-features --test check_cli` passes with cases for empty map, empty list, unknown provider, provider without pool, unknown account, and an account that belongs to another provider (message names that provider).
  Evidence: `check_refuses_a_client_entry_that_cannot_select_an_account` drives the real binary over all six (the "other provider" case twice: inline owner `account 'a' belongs to provider 'claude', not 'codex'`, and store owner `account 'k' belongs to provider 'codex', not 'claude'`). `check_accepts_clients_pinned_to_inline_and_store_accounts` covers a valid inline + store-backed pin. `test result: ok. 7 passed; 0 failed`.
- [x] A config without `[server.auth.clients]` behaves as before: the full existing suite passes.
  Evidence: `cargo test --all-features --workspace` → 49 test binaries, 3893 passed, 0 failed (no existing test changed except adding `clients: Default::default()` to `InboundAuthConfig` literals).
- [x] `cargo fmt --all --check`, `cargo clippy --all-targets --all-features -- -D warnings` and `cargo test --all-features --workspace` pass.
  Evidence: fmt clean; clippy `Finished` with no warnings; workspace suite as above. The whole Verify block exited 0.
- [x] `git diff --name-only origin/main...HEAD` lists the README (4 languages), `docs/m4-inbound-auth.md` and the two site pages in 4 languages each.
  Evidence (non-`src`/`tests` entries):
  ```
  README.ja.md
  README.ko.md
  README.md
  README.zh-CN.md
  docs/m4-inbound-auth.md
  shunt.toml.example
  site/src/content/docs/guides/shared-gateway.mdx
  site/src/content/docs/ja/guides/shared-gateway.mdx
  site/src/content/docs/ja/reference/configuration.md
  site/src/content/docs/ko/guides/shared-gateway.mdx
  site/src/content/docs/ko/reference/configuration.md
  site/src/content/docs/reference/configuration.md
  site/src/content/docs/zh-cn/guides/shared-gateway.mdx
  site/src/content/docs/zh-cn/reference/configuration.md
  ```
  Locale anchors were read from the built site (`npm run build` in `site/`, 209 pages): ko `#serverauthclients-선택`, ja `#serverauthclientsオプション`, zh-cn `#serverauthclients可选`; all 18 added links resolve, and `npm run lint:docs` is clean. `wiki/` is untouched.

🤖 Generated with [Claude Code](https://claude.com/claude-code)

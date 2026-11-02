Goal-Task: account-pinning/TASK-002

## Acceptance

- [x] **The forgery test passes: goal-x still reaches only A/B tokens.** `tests/client_account_pinning.rs::forged_client_headers_do_not_widen_the_subset` sends six goal-x turns with forged `x-shunt-inbound-client: goal-w` and `x-shunt-account-scope: goal-w`. Output: `goal-x with forged goal-w headers: upstream received tokens of {"a", "b"}` and `test forged_client_headers_do_not_widen_the_subset ... ok`. Each upstream request also carries `x-shunt-inbound-client: goal-x`, the verified client.
- [x] **The router test passes: a judge call for goal-y reaches only C.** `tests/router_judge.rs::a_judge_call_keeps_the_callers_account_subset` puts the judge on a Claude OAuth pool of a, b and c, pins goal-y to c, and sends six undecided turns in six sessions. All six judge calls carry account c's bearer: `test a_judge_call_keeps_the_callers_account_subset ... ok`. Mutation check: with `side_call()` dropping the client, the test fails with `left: "Bearer judge-oauth-a"`.
- [x] **A test shows the upstream request has no `x-shunt-account-scope` header.** The forgery test asserts it on every answer request (Anthropic adapter), and the router test asserts it on every judge request. Mutation check: without the strip in `outbound_headers`, the forgery test fails with `the account scope never reaches the upstream`.
- [x] **`cargo fmt --all --check`, `cargo clippy --all-targets --all-features -- -D warnings` and `cargo test --all-features --workspace` pass.** fmt: no output, exit 0. clippy: `Finished dev profile`, exit 0. Full suite: 49 test binaries, 3896 passed, 0 failed, exit 0. `cargo test --all-features --lib routing::serve`: 47 passed. `--test router_judge`: 14 passed. `--test client_account_pinning`: 4 passed.

Verified revision: tree of `88c979de` (rebased onto `origin/main`; the rebase brought in only `goals/account-pinning/decisions.jsonl`, no build input).

## Summary

**What.** A pinned client that used a router model id had its judge call draw from every account of the judge's pool, because judge calls rode a context with no client. Now the account subset follows the verified client on every call, router judge calls included, and a caller cannot widen it with forged headers.

**How.**
- New reserved header `x-shunt-account-scope` (`crate::accounts::ACCOUNT_SCOPE_HEADER`), added to `RESERVED_SLOTS` and the slot table in `src/auth/slots.rs`. `check_inbound_auth` strips a caller-sent copy with the other reserved slots.
- `headers_for_route` stamps it on every credential-injecting attempt whenever a client is known (static token, JWT or gateway login). The `x-shunt-inbound-client` attribution stamp is unchanged: only for a gate credential.
- `InboundContext::internal()` became `InboundContext::side_call(&self)`: the caller's client for account scope only, with no attribution and no gateway claims. `routing::serve::dispatch` takes it from `AdmittedContext` (its `_inbound` field is now read as `inbound`).
- TASK-001's filter helper `accounts::inbound_client` became `accounts::account_scope` and reads the new header (Claude, Kimi, Antigravity, Codex chain attempt).
- The Anthropic adapter's `outbound_headers`, the only adapter that copies inbound headers upstream, removes the header. The others build their upstream headers from allowlists; the inbound Codex endpoint passes its client explicitly and strips reserved slots.
- Tests: the forgery test, the router judge test, and a unit test (`proxy::failover::tests::the_account_scope_is_stamped_for_every_verified_client`) for which contexts get the scope and attribution stamps. Router-serve unit tests use `InboundContext::default()` (test-only) where they used `internal()`.
- Docs: `docs/m4-inbound-auth.md` §2.1 and §7 (trust boundary: the subset follows the verified token, forged headers are stripped, the scope header never reaches an upstream, and on one OS user pinning guards against mistakes, not a hostile agent).

**Deviations.**
- Gateway-login clients are now pinned too, as the spec asks ("static token, JWT, or gateway login"). Before, only static-token and JWT clients were. So the client-key sentence changed on the shared-gateway guide and the configuration reference, in en/ko/ja/zh-CN, plus the `ClientPolicy` doc comment.

**Proposed follow-ups.** None.

🤖 Generated with [Claude Code](https://claude.com/claude-code)

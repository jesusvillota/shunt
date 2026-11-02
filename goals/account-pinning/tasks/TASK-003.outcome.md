Goal-Task: account-pinning/TASK-003

## Summary

Add `shunt pool --client <name>` and `GET /admin/api/pool?client=<name>` so a goal keeper sees only its configured accounts. The handler reuses `Config::client_accounts` and filters resolved accounts before snapshots and subscription-plan lookups. Providers without a client entry and unknown clients retain all accounts. JSON fields and admin authentication stay unchanged.

Update the CLI and endpoint references in English, Korean, Japanese, and Simplified Chinese, all four README capability rows, and `docs/pool-tui.md`. The TUI is outside this task. No config keys, credential writeback, dependencies, goal specs, or other tasks changed.

Verified revision: `9cd62ee282c8108c190a990a43139278b40e86c3`. This branch was rebased onto `origin/main` without conflicts. The only added file is the shared test fixture `tests/common/pool.rs`. No deviations or proposed follow-ups.

## Acceptance

- [x] CLI: `cargo test --all-features --test pool_cli` exited 0: **8 passed, 0 failed**. `client_lists_only_its_accounts_from_the_gateway` uses a real gateway: `--client goal-y` lists only account c under anthropic; no flag lists a/b/c. It checks text and JSON, unknown-client fallback, and the unpinned codex provider. Lowercase account names follow config validation. `json_preserves_snapshot_bytes_and_uses_read_request` also checks URL encoding of `goal+y &z` and unchanged JSON bytes.
- [x] Admin API: `cargo test --all-features --test admin_surface` exited 0: **55 passed, 0 failed**. `admin_pool_filters_client_accounts_and_keeps_unpinned_providers` verifies `?client=goal-y` lists only c, unknown clients and no query list a/b/c, and codex remains complete. Both CLI and API tests use a read-tier admin key. Env-backed fixture accounts avoid real credential-store scans and profile requests.
- [x] Quality checks: `cargo fmt --all --check` exited 0; `cargo clippy --all-targets --all-features -- -D warnings` exited 0 without warnings; `cargo test --all-features --workspace` exited 0: **3895 passed, 0 failed across 49 test results**. The complete five-command Verify block exited 0 in this session.

## Validation

The new CLI and API cases failed against the original code for the missing `--client` flag and missing filter, then passed after implementation. An independent read-only review found no Critical or Important issues. The UI build passed. Site docs lint passed for 126 files; the site build passed for 209 pages. GitHub Actions were not read or used as a gate. The optional live prefill-router checkpoint was not configured.

Commands run in order, with `CARGO_TARGET_DIR` pointing to the existing local build cache:

```sh
cargo test --all-features --test pool_cli
cargo test --all-features --test admin_surface
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features --workspace
```

Tail of the Verify block (exit 0):

```text
    Checking shunt-gateway v0.51.1 (/Users/jesusvillotamiranda/orca/workspaces/shunt/goal-account-pinning-task-003)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 31.83s
cargo test --all-features --workspace
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

   Doc-tests shunt

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

Verify block: exit 0
```

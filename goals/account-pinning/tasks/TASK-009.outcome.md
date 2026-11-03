Goal-Task: account-pinning/TASK-009

## Summary

**What.** `goal pin --goal <slug>` shows, sets (`--set <provider>=<a,b>`, repeatable) or clears (`--clear`) a goal's shunt account pins. The core lives in `goal-core/src/pinning.rs`: `pin_accounts(slug, selection)`, `current_selection(slug)` (for the TUI to prefill) and `available_accounts()` (from `shunt pool --json`).

**How.**
1. Validate first: every named account must be in the running pool for that provider. Unknown names or providers are refused with the valid ones; an empty `--set` list is refused (use `--clear`). Nothing is written on refusal.
2. Token: if Keychain item `shunt-goal-<slug>` (account `$USER`) is missing, create 32 bytes from `/dev/urandom` as hex. Store it through `security -i`, which reads the command on stdin, so the token never enters argv. Append `<slug>:<token>` to `[server.auth] tokens_file` (atomic write, mode 0600). A second run reuses the token and leaves the file unchanged. A different token already registered for the slug is refused, never overwritten. A Keychain error other than "not found" also stops the run, so an existing item is never replaced.
3. `shunt.toml` (`SHUNT_CONFIG` or `~/.config/shunt/shunt.toml`) is edited with `toml_edit`. Each listed provider is set, an empty list removes it, other providers keep their pins, and no keys left removes `[server.auth.clients.<slug>]`. Comments, order and file mode are kept; the write is atomic.
4. Reload: shunt always watches its config directory, so the write triggers the reload. The command reports when shunt is not reachable.

**Deviations from the spec.**
- No SIGHUP fallback: shunt reports no PID, and its config-directory watch cannot be turned off (a failed watch only logs a warning). The command reports when shunt is not running instead.
- Provider keys are the `[providers.<name>]` keys that `shunt pool --json` reports (on this Mac: `anthropic`, `antigravity`, `codex`). shunt rejects `accounts.<provider>` for any other key (`unknown provider`), so the first draft's `anthropic`→`claude` rename was removed. The acceptance test's fake pool calls its provider `claude`.
- `tokens_file` must be an absolute path: shunt (TASK-008, jesusvillota/shunt#95, still open) opens it as written from its own working directory.
- The first draft piped the token to `security add-generic-password -w`. On this Mac that prompts twice and reads the terminal when there is one, so it was replaced by `security -i`.

**Also changed.** `goal/tests/parser_matrix.rs` lists `pin` among the native-only subcommands, like `pool`. README documents the command next to the per-goal token hand-off.

**Verified revision.** 0ac9847. Also smoke-tested once against the real macOS Keychain with a throwaway slug: one 64-hex token, the same token in `tokens_file` (mode 0600), reused on the second run; the item was deleted afterwards.

**Follow-ups.** None for this task. TASK-007 must add `[server.auth] tokens_file = "<absolute path>"` to the owner's `shunt.toml` before `goal pin --set` can run there.

## Acceptance

- [x] **`goal pin --goal x --set claude=a,b` creates the token once (a second run reuses it) and writes exactly `[server.auth.clients.x].accounts.claude = ["a", "b"]` to a temp `shunt.toml`, keeping its comments.** `goal/tests/pin.rs::pin_creates_token_once_writes_table_and_clears_it` runs `--set claude=a,b` twice. After each run the whole file must equal the original (both comments kept) plus `[server.auth.clients.x]` / `accounts.claude = ["a", "b"]`. The fake `security` saw exactly one `-i` store call and two lookups. `tokens_file` holds one line `x:<token>`, mode 0600. `shunt.toml` keeps mode 0640. `goal pin --goal x` then prints `claude=a,b`. Result: `test pin_creates_token_once_writes_table_and_clears_it ... ok`.
- [x] **`--clear` removes the table.** Same test: with shunt down, `--clear` succeeds, says `shunt is not running`, and leaves `shunt.toml` byte-identical to the original. The token entry stays.
- [x] **An unknown account is refused with the list of valid names.** `pin_refuses_unknown_names_and_empty_lists_without_writing`: `--set claude=a,unknown` exits 1 with `error: unknown account unknown for claude; valid names: a, b`. `codex=a` is refused with `valid providers: claude`, and `claude=` asks for `--clear`. No Keychain item, token file or config change follows. Result: `... ok`.
- [x] **The token never appears in test-captured stdout, stderr or process arguments.** After every `--set` and `--clear` run, `Rig::assert_secret` checks that the 64-hex token is absent from captured stdout, stderr and the argv log. The fake `security` and `shunt` append every argv to that log.
- [x] **`cd tools/goal-rust && cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace` passes.** Ran with `RUST_TEST_THREADS=2` at 0ac9847: exit 0, 75 test binaries `ok`, 0 failures. Tail:

```
     Running tests/pin.rs (target/debug/deps/pin-b950c468d570df91)

running 2 tests
test pin_refuses_unknown_names_and_empty_lists_without_writing ... ok
test pin_creates_token_once_writes_table_and_clears_it ... ok
   Doc-tests goal_tui

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
exit=0
```

🤖 Generated with [Claude Code](https://claude.com/claude-code)

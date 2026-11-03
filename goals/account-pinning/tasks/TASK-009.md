---
id: TASK-009
title: 'goal pins accounts: token, Keychain and shunt.toml writer'
milestone: M-1
state: approved
depends_on: [TASK-005]
delivery: pr
repo: goal
route: {platform: claude, model: opus, effort: high}
---

## Why

Serves SC-1 and SC-6. The owner wants to choose a goal's accounts from the goal TUI, not by editing files. This task builds the writer that the TUI calls. It also gives a `goal pin` command so the writer is testable and scriptable.

## Scope

Work in the `goal` repo (`tools/goal-rust/goal-core`, CLI in `tools/goal-rust/goal`). Facts: TASK-004 stores a goal's token in the Keychain as service `shunt-goal-<slug>` and hands it to sessions; TASK-005 reads it for the keeper; `pool::fetch` (`goal-core/src/pool.rs`) runs `shunt pool --json`; TASK-008 adds `[server.auth] tokens_file` to shunt.

1. A core function `pin_accounts(slug, selection)` where `selection` maps provider to the list of allowed account names. It does three things, in order:
   1. Ensure a token: if the Keychain item `shunt-goal-<slug>` is missing, create a random 256-bit hex token, store it, and append `<slug>:<token>` to shunt's `tokens_file` (mode 0600, atomic write). Never print the token or put it in a command line.
   2. Edit `~/.config/shunt/shunt.toml` (path from `SHUNT_CONFIG` or the default) with `toml_edit`, keeping comments and order: set `[server.auth.clients.<slug>].accounts.<provider>`. An empty selection for a provider removes that provider key; no keys left removes the table (the goal is unpinned and uses all accounts). Write atomically; keep the file mode.
   3. Signal the running shunt to reload (config-file watch needs nothing; otherwise SIGHUP to the PID that `shunt` reports). Report if shunt is not running.
2. A read function `current_selection(slug)` for the TUI to prefill, and `available_accounts()` from `shunt pool --json` (providers and their account names).
3. CLI: `goal pin --goal <slug> --set <provider>=<a,b> ...`, `goal pin --goal <slug> --clear`, and `goal pin --goal <slug>` to print the current selection.
4. Refuse a selection that names an account not in the pool, or an empty list for a provider that you did not clear.
5. Tests with a temp config dir, a fake `security`, and a fake `SHUNT_BIN`: token created once and reused; toml edit keeps comments; clear removes the table; unknown account refused; the token never appears in captured output or process arguments.
6. Docs where the goal CLI and shunt hand-off are described.

AGENTS.md of shunt says to ask before changing credential-file writeback: this task writes only the owner's own `shunt.toml` and token file, and the owner asked for it. Out of scope: TUI, shunt changes.

## Acceptance

- `goal pin --goal x --set claude=a,b` creates the token once (a second run reuses it) and writes exactly `[server.auth.clients.x].accounts.claude = ["a", "b"]` to a temp `shunt.toml`, keeping its comments (test output).
- `--clear` removes the table.
- An unknown account is refused with the list of valid names.
- The token never appears in test-captured stdout, stderr or process arguments.
- `cd tools/goal-rust && cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace` passes.

## Verify

```sh
cd tools/goal-rust && cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && RUST_TEST_THREADS=2 cargo test --workspace
```

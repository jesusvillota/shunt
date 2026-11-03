Goal-Task: account-pinning/TASK-008

## Summary
- `[server.auth].tokens_file` (optional, `~` expanded): one `client:token` per line, merged with `tokens_env`.
- A missing file is an empty list. A file with mode wider than 0600 fails `shunt check` and the reload. A client name or token that appears twice across the file and `tokens_env` fails, and the error names both sources.
- The reload watcher also watches the token file's directory, so a new line takes effect with no restart. The watched path is fixed at startup (documented).
- Docs: site `reference/configuration.md` in en/ko/ja/zh-cn, the `README.ko/ja/zh-CN.md` rows, and `docs/config-reload.md`. The spec's `docs/config.md` does not exist; the site pages are named `configuration.md`.
- The earlier commits on this branch never compiled (doubled escapes in a test, struct literals missing the new field). This push fixes that and tightens the merge code.

## Acceptance
- [x] A running gateway accepts a token appended to `tokens_file` after the next reload, with no restart. Evidence: `reload::tests::reload_accepts_new_token_without_restarting_gateway` and `reload::tests::file_watch_task_hot_reloads_when_token_file_changes` pass (the second one runs the real file watcher, changes only the token file in another directory, and sees `bob` authenticate).
- [x] Duplicate client names or tokens across the file and `tokens_env` fail `shunt check`, naming both sources. Evidence: `check_merges_token_file_and_env_and_rejects_collisions_and_wide_mode` in `tests/check_cli.rs` passes; it asserts stderr contains `tokens_file` and `tokens_env` for a duplicate name and a duplicate token.
- [x] A `tokens_file` with mode wider than 0600 fails `shunt check`. Evidence: same test, mode 0644 case asserts failure mentioning `0600`.
- [x] `cargo fmt --all --check`, `cargo clippy --all-targets --all-features -- -D warnings` and `cargo test --all-features --workspace` pass. Evidence: the full Verify sequence exited 0; 3947 tests passed, 0 failed.

## Verified revision
`ccb64540` on `goal/account-pinning/task-008`.

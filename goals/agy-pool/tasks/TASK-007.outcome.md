# TASK-007 outcome

## Acceptance

- [x] Tests cover Anthropic 5h/7d usage and resets, Codex, weekly-only Antigravity buckets, unchanged JSON, and clean non-zero missing-token and unreachable-gateway errors.
  Evidence: `cargo test --test pool_cli` exited 0 with 7 passed, 0 failed. The same seven tests passed under `--all-features`; refused tokens (401/403), availability/cooldowns, configured bind, custom header and token-pair lookup are also covered. Error tests use `RUST_BACKTRACE=1` and assert one stderr line, empty stdout and failure status. The JSON test asserts exact bytes, including a future field and high-precision number. Log: `/tmp/shunt-task007-focused.log`.
- [x] Live text and JSON output from a running scratch gateway show Antigravity quota per account and are pasted in the PR.
  Evidence: both default-release commands below exited 0. Five Antigravity accounts each report two weekly buckets (Gemini and Claude + GPT), with remaining values and reset timestamps. The captured JSON was checked for all five accounts and both buckets. Scratch configuration `/tmp/shunt-task007-live.toml`, bind `127.0.0.1:39007`, refresh interval 60 seconds. `shunt check` returned `config ok`; startup logged `shunt listening` and the usage poller. No production config edit. Captures: `/tmp/shunt-task007-live-text.txt`, `/tmp/shunt-task007-live-json.json`. Full outputs are in https://github.com/jesusvillota/shunt/pull/16.
- [x] Format, clippy, all-feature workspace tests, default release build, and `shunt pool --help` pass.
  Evidence: the exact chain below exited 0 on code revision `89f99a6fdb1317fdd08a86df439e11b1a5ac6814`. Workspace total: 3,795 passed, 0 failed, 3 ignored. Default release exposes the command without `tui`. Build prerequisites: generated dashboard assets (`npm ci --prefix ui`, `npm run build --prefix ui`, both passed), Homebrew shared Python 3.13, and task build cache. Log: `/tmp/shunt-task007-verify.log`.
- [x] Delivery PR targets `main`, carries `Goal-Task: agy-pool/TASK-007` and label `goal:agy-pool`; no checks are configured on this branch.
  Evidence: https://github.com/jesusvillota/shunt/pull/16; `gh pr checks` reports no checks. The final delivery step marks the PR ready and verifies its open, non-draft state after this evidence-only commit.

## Summary

Added `shunt pool [--json]` as an unconditional clap subcommand, implemented in `src/pool/`. It resolves tokens and gateway URLs as the TUI does, performs one authenticated GET, and exits. Text prints one section per provider and one line per account, with availability, usage/reset countdowns, cooldowns, and upstream-reported Antigravity buckets. JSON preserves the response text without reserialization. Errors print one stderr line without an error chain or stack trace.

Moved existing pool DTOs, token lookup, and quota percentage/reset formatting into the unconditional module and re-used them from the TUI. Existing TUI behavior and tests are preserved. No config keys or credential writeback behavior changed.

Updated all four README and site CLI reference languages, and `docs/pool-tui.md`. Considered all documentation surfaces; generated `wiki/` is not edited. No scope deviations or proposed follow-ups. The obsolete `computing-jvm` entry was removed only from the temporary scratch config so every configured Antigravity account in the capture has quota. Branch stayed `goal/agy-pool/task-007`; rebase onto `origin/main` succeeded without conflicts. Verified code revision: `89f99a6fdb1317fdd08a86df439e11b1a5ac6814`. The final commit adds only this outcome evidence.

## Verify command and tail

```sh
export CARGO_TARGET_DIR=/tmp/shunt-target
export PYO3_PYTHON=/opt/homebrew/bin/python3.13
cargo fmt --all --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all-features --workspace && cargo build --release && ./target/release/shunt pool --help
```

```text
    Finished `release` profile [optimized] target(s) in 51.13s
Print the managed account pool from a running gateway (read-tier admin token is enough)

Usage: shunt pool [OPTIONS]

Options:
      --config <CONFIG>  
      --url <URL>        Gateway URL. Default: configured `[server].bind`
      --token <TOKEN>    Admin token: SHUNT_ADMIN_TOKEN, SHUNT_ADMIN_TOKENS, or ~/.shunt/admin-token
      --header <HEADER>  Header carrying the admin token [default: x-shunt-admin-token]
      --json             Print the gateway JSON unchanged
  -h, --help             Print help
VERIFY_EXIT=0
```

## Live commands and text stdout

```sh
./target/release/shunt pool --config /tmp/shunt-task007-live.toml
./target/release/shunt pool --config /tmp/shunt-task007-live.toml --json
```

```text
antigravity:
  computingjvm | ok | Gemini Models · weekly: 0% left · resets in 6d 8h | Claude + GPT Models · weekly: 100% left · resets in 6d 15h
  keepuntouser | ok | Gemini Models · weekly: 100% left · resets in 6d 15h | Claude + GPT Models · weekly: 100% left · resets in 6d 15h
  tommyshelby | ok | Gemini Models · weekly: 0% left · resets in 6d 3h | Claude + GPT Models · weekly: 0% left · resets in 13h 56m
  personal | ok | Gemini Models · weekly: 0% left · resets in 6d 8h | Claude + GPT Models · weekly: 85% left · resets in 6d 14h
  cemfi | ok | Gemini Models · weekly: 100% left · resets in 6d 15h | Claude + GPT Models · weekly: 100% left · resets in 6d 15h

codex:
  cemfi | ok | 5h: 1% used | 7d: 41% used
  personal | ok | 5h: 13% used | 7d: 49% used
```

The PR also contains the complete JSON stdout, with each account's `quota_buckets` and reset timestamps.

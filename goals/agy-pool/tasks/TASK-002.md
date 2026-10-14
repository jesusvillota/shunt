---
id: TASK-002
title: Read agy quota via the agy /usage command per account
milestone: M-1
state: approved
depends_on: [DEC-001, DEC-002]
delivery: pr
---

## Why

Quota tracking per account (SC-1), using the source chosen in DEC-001.

## Scope

Implement the chosen quota source. If DEC-001 picks the Orca method: for each account run `agy -p /usage --output-format json --print-timeout 20s` with that account's `profile_dir`, never `--disable-slash-commands`, gate on `agy --version >= 1.1.11`, latch off if the reply starts a model turn (`num_turns > 0`), parse `command.data.groups[].buckets[]`, and feed `AccountPool::note_antigravity_usage` via `src/usage_poll.rs`. Keep the Gemini vs Claude+GPT grouped labels. Update docs (`docs/notes/antigravity-quota-observation.md`, `site/` reference + ko/ja/zh-cn copies, README if needed). Out of scope: account selection changes (TASK-003).

## Acceptance

- Unit tests parse a captured `/usage` JSON fixture into the four grouped buckets.
- A test proves a reply with `num_turns > 0` latches the probe off and spends no further calls.
- Against a live gateway, `goal pool` (or `GET` admin pool endpoint) shows 5h and weekly quota for every configured antigravity account.
- `cargo fmt --all --check`, `cargo clippy --all-targets --all-features -- -D warnings` and `cargo test --all-features --workspace` pass.

## Verify

`cargo fmt --all --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all-features --workspace`\n`goal pool` with the live gateway running; paste output.

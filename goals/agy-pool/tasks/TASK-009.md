---
id: TASK-009
title: Switch goal pool to call shunt pool --json
milestone: M-1
state: cancelled
depends_on: [TASK-007, DEC-005, TASK-013]
delivery: pr
reason: Owner is switching goal pool to shunt pool directly in the config repo; no longer done through this goal.
---

## Why

`goal pool` re-implements the shunt admin API call with `curl` and omits Antigravity `quota_buckets`, so it shows `—` for every agy account. After TASK-007, `shunt pool --json` is the one correct source.

## Scope

The code change is in the **config repo** (`~/GitHub/config`, remote `jesusvillota/config`), file `tools/goal-rust/goal-core/src/pool.rs` (about 85 lines) and the goal CLI text rendering that uses it. The shunt repo gets only this task's outcome file.

- Work in a NEW git worktree of the config repo, branched from its `origin/main`. The main checkout has other people's uncommitted changes (`claude/settings.json`, `codex/config.toml`, `shared/instructions.md`): never touch or stash them.
- Replace the `curl` call and token/URL resolution in `fetch()` with running `shunt pool --json` (resolve `shunt` from PATH; allow an override env var such as `SHUNT_BIN`), keep returning the same `serde_json::Value`. Keep the error messages one line: shunt missing, gateway unreachable, token refused.
- Render Antigravity `quota_buckets` in the text output (label, remaining, reset) so agy accounts no longer show `—`. Keep Anthropic and Codex lines unchanged. Keep the `--json` output shape the dynamic-routing orchestrator reads.
- Update existing tests in `pool.rs`; add one for a snapshot with weekly-only `quota_buckets`. Never weaken a test. Update the goal CLI README if it describes `goal pool`.
- Open the PR on `jesusvillota/config` with `gh pr create -R jesusvillota/config`; put its URL in the outcome. The shunt PR carries `Goal-Task: agy-pool/TASK-009` and the outcome only.

Out of scope: any shunt code, other goal subcommands, the orchestrator's routing logic.

## Acceptance

- With the TASK-007 build of `shunt` installed and the gateway running, `goal pool` prints per-family weekly quota for every antigravity account (paste the output).
- `goal pool --json` still parses for the orchestrator (paste a one-line `jq` check).
- The goal-rust tests pass (`cargo test` in `~/GitHub/config/tools/goal-rust`, paste the tail).
- The config repo PR URL is in the outcome, and no unrelated file changed in it.

## Verify

`cd <config worktree>/tools/goal-rust && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` and `goal pool` against the running gateway.

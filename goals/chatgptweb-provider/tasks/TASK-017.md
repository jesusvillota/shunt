---
id: TASK-017
title: 'Fix live ui_drift on Codex generate (issue #69)'
milestone: M-2
state: approved
depends_on: [TASK-014]
delivery: pr
---

## Why

TASK-015 failed twice on the same live error (PRs #79 and #80): the first Codex Generate request fails after paste and before Send with `ui_drift`, `submission: not_sent`. SC-3 cannot close until a real Codex turn gets through the driver. Tracks jesusvillota/shunt#69.

## Scope

Fix the driver in `helpers/chatgptweb-browser/` (and the Rust adapter only if the cause is there). First find which open/check/paste guard rejects the full Codex text transcript on the signed-in cemfi Chat page. Capture sanitized rendered markup and the failing guard, add focused fixture coverage that reproduces the failure, then fix the selector or text comparison. Keep the Chat-only guards, and never retry an uncertain physical Send.

Live access is read-only: the cemfi profile on loopback CDP http://127.0.0.1:9333 may be attached, a prompt may be pasted into the composer to observe the guards and then cleared, but the Send button must never be clicked. Do not change the owner config or account pool, and do not close or navigate owner tabs. Do not run the TASK-015 live smoke; that stays a separate task. Out of scope: MCP/tunnel, tool protocol changes.

## Acceptance

- A fixture test reproduces the `ui_drift` on the full Codex transcript shape and fails before the fix, passes after.
- The sanitized outcome names the guard that failed and why, with no raw page text, URLs or profile data.
- A read-only live check against the cemfi profile (no Send) shows the production generate path passes open, check and paste.
- The full Verify list below passes.

## Verify

npm ci --prefix helpers/chatgptweb-browser
npm test --prefix helpers/chatgptweb-browser
cargo test --test chatgptweb --test chatgptweb_adapter
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features --workspace
git diff --check

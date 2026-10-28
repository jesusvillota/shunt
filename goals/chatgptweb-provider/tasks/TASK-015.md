---
id: TASK-015
title: Live smoke a Codex shell and file-edit task in Chat mode
milestone: M-2
state: cancelled
depends_on: [TASK-014, TASK-005, TASK-012, TASK-017]
delivery: pr
kind: run
reason: "Goal called off by owner 2026-10-28: ChatGPT web is now driven through Orca's browser via the delegate-chatgptweb skill."
---

## Why


Close SC-3 with a real Codex tool loop through the merged adapter.
TASK-012 observed a guarded live answer, but did not prove the whole
gateway, clipboard and tool-result path. TASK-006 was waived by the
owner; quota equality is not a gate for this run.

## Scope


Run merged code only. Read docs/chatgptweb-provider.md and the preceding
outcomes, then run the same demo before opting into live. No code fixes
or account provisioning in this task. Use the harness commands below.

Outside prerequisites: installed Codex, Node/helper dependencies and
browser; cemfi already signed in manually to owner Chrome, using
~/.local/share/shunt/chatgptweb-browser/chrome/cemfi and loopback CDP
http://127.0.0.1:9333; no competing owner/worker use of that profile.
Health must establish Chat=true and Work=false before any send.
Login, challenge, unknown mode or inaccessible profile blocks the run.
Do not automate sign-in/challenges, copy cookies or switch to Work.
The earlier TASK-012 one-send authorization is not reusable evidence or
permission for extra probes; this task is the separate bounded smoke.

Budget: one Codex process, one gateway, one attached account, one scratch
repository, one serial run (shard 1/1; no merge step). At most six fresh
chat sends and 15 minutes total, with the merged per-turn deadline.
Stop on the first failure; no automatic rerun after Send. Preserve the
sanitized summary and report the blocker. Resume after a crash only
after reviewing whether a prompt was sent; uncertain sends are not
replayed. A required implementation change goes in a separate follow-up.

Require an actual shell tool execution and fixture file edit through
Codex, tool results in subsequent browser transcripts, and a final answer.
Paste command exits and sanitized harness assertions into this task's
outcome and PR, with source revision, CLI version, observed effort,
send count, elapsed time and owner-browser survival. Do not commit raw
transcripts, profiles or temporary evidence. The existing codex pool and
the owner's config remain unchanged. No second account or Claude Code.
If text protocol reliability prevents completion, use the worker Decision
request protocol with concrete evidence; do not adopt MCP/tunnel here.

## Acceptance


- The merged demo passes immediately before the live attempt, and live health confirms the signed-in cemfi profile in Chat mode.
- Real Codex runs a shell command, edits the fixture to the exact expected bytes, returns tool results and finishes through chatgptweb_browser; the live command exits 0.
- Sanitized outcome evidence records revision, client version, effort, call/result IDs, send count, elapsed time and final file assertion within the six-send/15-minute budget; owner Chrome remains usable.

## Verify


git rev-parse HEAD
codex --version
cargo test --test chatgptweb_codex_smoke demo -- --ignored --exact --nocapture --test-threads=1
SHUNT_CHATGPTWEB_SMOKE_PROFILE="$HOME/.local/share/shunt/chatgptweb-browser/chrome/cemfi" SHUNT_CHATGPTWEB_SMOKE_CDP_URL="http://127.0.0.1:9333" cargo test --test chatgptweb_codex_smoke live -- --ignored --exact --nocapture --test-threads=1
git diff --check

## Attempts


- 2026-10-28: PR #79 closed without merging: Owner authorized a fresh bounded live smoke. The earlier tab loss (#78) was the owner closing a tab by accident; unknown whether before or after the ui_drift failure. Keep the cemfi Chrome profile untouched during the run.
- 2026-10-28: PR #80 closed without merging: Blocked by ui_drift twice (PRs #79, #80); retry after TASK-017 driver fix.

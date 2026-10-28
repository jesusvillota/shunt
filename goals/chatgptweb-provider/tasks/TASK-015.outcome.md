# TASK-015 outcome

Status: BLOCKED. The live driver returned `ui_drift` after successful health.
Failure-stage evidence is missing. A separate driver investigation is required
before another authorized live attempt. No retry or code fix was made.

## Acceptance

1. [x] The merged demo passed immediately before live. Live health confirmed
   the signed-in cemfi binding in Chat mode. Tested source was
   `23c82a8b16c61f4226147f8ba8aa2c62d5dbc3f0`, equal to `origin/main` at setup.
   The demo exited 0, with one passing test, at 01:12:14 UTC on 2026-10-28.
   Live started immediately afterward. A read-only observer of the harness-owned
   health page recorded Chat=true, Work=false and signed-in profile controls.
   The ledger cleared its health latch before the first Generate reservation.
   The harness asserts a successful `health` event before starting Codex.
2. [ ] Real Codex did not complete shell execution or the fixture edit.
   `codex-cli 0.164.0` reached `chatgptweb_browser`, but the live command exited
   101. Codex reported `turn.failed` with `ui_drift` and explicitly said the
   request was not replayed. No tool call or result ID was observed. The ledger
   recorded calls=0 and results=0. No final answer or `turn.completed` occurred.
3. [ ] The bounded failure evidence is recorded, but successful final-file
   evidence is unavailable. The ledger reserved one Generate request, within
   the six-send limit. Physical Send status is unknown; it must not be replayed.
   Live command elapsed time was 4,684 ms; the test reported 4.44 seconds.
   Both are below 15 minutes. The generate page showed `Medium`, Chat=true,
   Work=false and signed-in controls. The last observed fixture bytes were
   `4245464f52450a` (`BEFORE\n`). The final `AFTER\n` assertion was not reached.
   Owner Chrome remained usable: production `inspect()` succeeded on its
   surviving Chat composer, with Medium effort and an empty composer. However,
   only one of two original page targets remained after teardown. No extra
   targets remained. The cause of this tab-count change is unknown.

## Verification

The commands below ran against merged source, before this outcome-only commit.

```text
$ git rev-parse HEAD
23c82a8b16c61f4226147f8ba8aa2c62d5dbc3f0
$ codex --version
codex-cli 0.164.0
$ cargo test --test chatgptweb_codex_smoke demo -- --ignored --exact --nocapture --test-threads=1
demo: real Codex shell, edit BEFORE→AFTER, all returned results, final completion; malformed output cannot edit; elapsed_ms=3353
ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out; finished in 3.35s
exit 0
$ SHUNT_CHATGPTWEB_SMOKE_PROFILE="$HOME/.local/share/shunt/chatgptweb-browser/chrome/cemfi" SHUNT_CHATGPTWEB_SMOKE_CDP_URL="http://127.0.0.1:9333" cargo test --test chatgptweb_codex_smoke live -- --ignored --exact --nocapture --test-threads=1
{"type":"error","message":"stream disconnected before completion: ChatGPT browser failed (ui_drift); request was not replayed"}
{"type":"turn.failed","error":{"message":"stream disconnected before completion: ChatGPT browser failed (ui_drift); request was not replayed"}}
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 4 filtered out; finished in 4.44s
exit 101
$ git diff --check
exit 0; no output
```

`cargo fmt --all --check` and the smoke target build also exited 0.
Helper dependencies were installed with `npm ci`; no tracked dependency changed.
Full workspace tests and clippy were not run for this outcome-only task.
GitHub Actions were not inspected or used as gates.

## Summary

One serial live attempt used one real Codex process, one gateway, one cemfi
account and one scratch repository. The harness used the requested owner Chrome
profile and loopback CDP endpoint. The demo ran first and passed. Live health
passed, then the first Generate failed closed. The harness removed scratch data
and its helper processes. A temporary read-only observer retained only mode,
effort, counters, call/result identifiers and fixture bytes; no raw transcript
or profile data is committed.

The failing task page closed before failure-stage evidence could be collected.
The public error does not identify whether open, check, paste or answer handling
failed. A new send would violate this task's stop-on-first-failure rule.
The missing evidence must come from a separately scoped driver investigation.
Existing issue https://github.com/jesusvillota/shunt/issues/69 covers the same
live `ui_drift` symptom; no duplicate issue was created. Its earlier `not_sent`
finding does not establish this attempt's submission status.

The owner-tab census discrepancy is recorded separately at
https://github.com/jesusvillota/shunt/issues/78.

Proposed follow-ups: investigate issue #69 without weakening Chat-only guards;
investigate issue #78; then authorize a fresh bounded
TASK-015 attempt after the prerequisite is resolved. Text-protocol reliability
was not tested because no model tool answer was obtained. No MCP/tunnel decision
is supported by this result.

Only this required outcome changes. No implementation, task spec, other task,
decision, goal metadata, owner configuration or account pool was edited. README,
engineering docs and translated site pages were considered; no behavior changed,
so they need no update. Generated wiki was not edited. The branch remains
`goal/chatgptweb-provider/task-015`; the PR targets `main` and stays draft.
No merge occurred. Acceptance is incomplete; the native goal remains incomplete.

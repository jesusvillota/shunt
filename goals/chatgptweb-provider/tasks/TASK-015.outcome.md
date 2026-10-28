# TASK-015 outcome

Status: blocked. The first live Codex turn failed with `ui_drift`,
`submission: not_sent`. No retry was made.

## Acceptance

1. [x] The merged demo passed immediately before the live attempt. On
   2026-10-28 at 01:44:57 UTC, `demo` exited 0: one test passed, zero failed,
   with shell execution, file edit, returned results, final completion and
   malformed-output rejection. Its measured demo time was 711 ms.
   Live started at 01:44:59 UTC. Production health returned `health` for
   `live-health`. The rendered-page guard established Chat=true, Work=false,
   signed-in profile controls and effort `Medium`. Chrome's process arguments
   matched the specified cemfi profile and loopback port 9333.
2. [ ] Real Codex did not complete the shell/edit/result loop. The first
   Generate request failed after paste and before the physical Send attempt.
   The production driver returned `ui_drift`, `submission: not_sent`.
   Codex emitted `turn.failed`; the live command exited 101. No tool call or
   result was produced. The scratch file remained `BEFORE\n`, hexadecimal
   `4245464f52450a`, instead of expected `AFTER\n`, hexadecimal `41465445520a`.
3. [ ] The complete successful-run evidence is unavailable because item 2
   failed. The available sanitized evidence follows. The harness reserved one
   Generate request out of six; zero physical sends occurred. The live test
   reported 4.69 seconds; command wall time was 4,964 ms, within 15 minutes.
   Chrome remained usable: its one original target survived, only one page
   remained, the document was complete, the composer existed, and Chat=true /
   Work=false still held after cleanup.

## Summary

This owner-authorized fresh attempt used merged implementation revision
`23a7d61aac8d72b0dd53df142fe407cc29f52bb1` and `codex-cli 0.164.0`.
The task branch was rebased onto `origin/main` before running; no source,
helper, harness or configuration difference existed against that revision.
Only this required outcome is changed afterward.

One serial live harness used the specified cemfi Chrome attachment, one health
check, one gateway, one Codex process and one scratch repository. The production
Chat-only guard and visible `Medium` effort were preserved. There was no retry,
account switch, sign-in automation, Work selection or owner configuration edit.
The run stopped on its first failure.

Temporary observers under `/tmp` wrapped the existing helper request, page-check
and send methods, forwarding arguments, return values, events and exceptions.
They recorded only timestamps, mode/effort, IDs, terminal errors, ledger counts
and fixture bytes. They did not alter prompts, guards, answers or tool records.
No raw transcript, profile data, credentials or temporary evidence is committed.
Node dependencies were installed with the existing lockfile.

The successful health and post-cleanup browser checks establish that this
attempt did not lose the owner's tab. The failure matches existing open issue
https://github.com/jesusvillota/shunt/issues/69, so no duplicate issue was opened.
Its exact post-paste cause is not established. All observed checks still showed
`Medium`; the post-paste content/enabled-send guard rejected before Send.
A compatible live send path is the missing prerequisite. Fixing the driver is
outside this run-only task. A separate driver-fix task must resolve issue #69
and merge before a newly authorized bounded smoke attempt. The tool protocol
was not reached, so this run provides no basis for changing to MCP/tunnel.
No material protocol decision is requested.

README, engineering docs, translated site pages and generated wiki were
considered. No behavior, configuration or capability changes were made, so
those surfaces need no edit. No task spec, other outcome, decision or goal.yaml
was changed. No merge occurred. The native goal is not marked complete.

## Sanitized live evidence

| Field | Observed value |
| --- | --- |
| Health request/result ID | `live-health` / `live-health` (`health`) |
| Generate request/error ID | `resp_c0ae0d72952a4e7da0aea6e693593492` / same (`error`) |
| Tool call IDs | None; failure preceded any returned tool call |
| Tool result IDs | None; no tool executed |
| Account / effort | cemfi / Medium |
| Guard | Chat=true, Work=false |
| Terminal error | `ui_drift`, `submission: not_sent` |
| Ledger | sends=1, calls=0, results=0, stopped=true |
| Physical sends / retries | 0 / 0 |
| Final file | `BEFORE\n`; expected `AFTER\n` assertion not satisfied |
| Browser after cleanup | Original target preserved; composer responsive to DOM inspection |

## Verification

Commands ran in this session on the verified implementation revision:

```sh
git rev-parse HEAD
codex --version
cargo test --test chatgptweb_codex_smoke demo -- --ignored --exact --nocapture --test-threads=1
SHUNT_CHATGPTWEB_SMOKE_PROFILE="$HOME/.local/share/shunt/chatgptweb-browser/chrome/cemfi" SHUNT_CHATGPTWEB_SMOKE_CDP_URL="http://127.0.0.1:9333" cargo test --test chatgptweb_codex_smoke live -- --ignored --exact --nocapture --test-threads=1
git diff --check
```

Exit codes: revision 0, client version 0, demo 0, live 101, diff check 0.
The live process also inherited the temporary sanitized observer through
`NODE_OPTIONS=--import=/tmp/task-015-fresh-smoke/observe.mjs`.
`cargo fmt --all --check` exited 0. No code changes required a new full workspace
or clippy run. No GitHub Actions were read or used as gates.

Sanitized demo tail:

```text
test demo ... demo: real Codex shell, edit BEFORE→AFTER, all returned results, final completion; malformed output cannot edit; elapsed_ms=711
ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out; finished in 0.71s
```

Sanitized live tail:

```text
stream disconnected before completion: ChatGPT browser failed (ui_drift); request was not replayed
turn.failed
FAILED
failures:
    live
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 4 filtered out; finished in 4.69s
error: test failed, to rerun pass `--test chatgptweb_codex_smoke`
```

The suggested Cargo rerun was not performed. The PR must remain draft with a
`Goal-Blocked:` marker; acceptance items 2 and 3 remain unmet.

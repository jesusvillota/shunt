---
id: TASK-014
title: Integrate Codex tool turns and a self-checking fake demo
milestone: M-2
state: approved
depends_on: [TASK-013]
delivery: pr
---

## Why


SC-3 needs normal Responses tool events and complete follow-up turns.
A deterministic Codex demo must prove the loop before using a live account.

## Scope


No cluster jobs in this task. Implement and verify on fixtures only.
Integrate the merged contract from docs/chatgptweb-provider.md into
src/adapters/chatgptweb/translate.rs and mod.rs. Reuse existing event
helpers where they fit without fabricating browser token usage.
Produce native function/custom call items for the captured tool surface,
correct event order, monotonic sequence numbers, stable item/call IDs,
argument/input deltas and matching final snapshots. Non-streaming JSON
must match streamed completion. Tool results must appear in the next
fresh chat's full transcript. Preserve ordinary text streaming, null
usage, deadlines, account serialization, cancellation and no replay after
failure or uncertain Send. Invalid output must end in failure, never
success or an executable incomplete call. Validate requests before Send.

Update src/discovery.rs to advertise exactly the supported shell/edit
surface and remove text-only instructions that suppress tools. Test all
three model catalog aliases. Keep serial tool execution sufficient for
this milestone; do not advertise parallel tools without coverage.
Extend tests/chatgptweb_adapter.rs using the existing Driver/FakeDriver
seam for multi-turn calls/results, mixed text, custom input if needed,
compressed input, streaming/non-streaming equivalence, malformed frames,
wrong names/IDs, tool-choice enforcement and errors after partial text.
Prove the existing codex pool and other provider catalogs are unchanged.

Extend tests/chatgptweb_codex_smoke.rs with ignored tests named demo and
live. Both run the real installed Codex CLI against an isolated local
shunt endpoint and a temporary repository. Use per-process CLI config
overrides, retaining model_provider=shunt; never edit the owner's Codex
config or use its built-in OpenAI route. Disable unrelated inherited
connectors/hooks in the isolated test configuration. Keep normal client
tool permissions and a workspace-write sandbox, not an approval bypass.
Require Codex to run a harmless shell command, edit a fixture file from
BEFORE to AFTER, return the real tool outputs and give a final answer.
Assert the file bytes and actual tool-execution events, not just prose.

Demo uses a scripted Driver through the existing injection seam, with
deterministic calls for the captured tool declarations and assertions on
every incoming result. It needs no account, browser or external network.
Add an adversarial demo case proving malformed output cannot edit a file.
Include a bounded long-transcript/chunk-boundary fixture and assert the
declared size limits; record elapsed time rather than a machine-specific
timing threshold. This is the small scale check before the live run.

Live must be explicit opt-in and fail, not skip, when its required inputs
are absent. Define SHUNT_CHATGPTWEB_SMOKE_PROFILE and
SHUNT_CHATGPTWEB_SMOKE_CDP_URL for this test only. Use the cemfi account
with its already running owner Chrome. ProcessDriver does not pass
--chrome: create a temporary .mjs launcher selected through the existing
SHUNT_CHATGPTWEB_HELPER override, append --chrome/--cdp-url and import the
production helper CLI. Do not add public config or replace the driver.
Take tests/common/mod.rs's environment guard for the whole test.
Test this launcher with local fixtures, including owner-browser survival.

Build a non-sending health preflight, one-profile lock, a maximum of six
Generate requests, a 15-minute total deadline and per-turn deadlines.
Stop on errors or uncertain submission; never retry a submitted prompt.
Every send must retain the production atomic Chat-only guard. Clean up
test servers, helper children and scratch files while leaving owner Chrome
and its original tabs open. Emit a sanitized summary with revision, CLI
version, account/effort, calls/results, turn count, elapsed time and final
file assertion. Run only capture/demo/fixture paths in this work task.

Update docs/chatgptweb-provider.md with the runnable demo/live commands
and limits. Update README.md and its ko/ja/zh-CN copies, and the provider
site page plus ko/ja/zh-cn copies. Consider the driver protocol/helper
README; change them only where behavior differs. Do not edit wiki.
State that browser reliability remains subject to the separate live gate.

## Acceptance


- Fake HTTP integration proves shell/edit calls, result replay and valid streamed/non-streamed Responses output while preserving text streaming and failure semantics.
- The demo runs real Codex without a browser/account and asserts a shell command, actual file edit, returned tool results and final completion; malformed calls cannot cause edits.
- Catalog aliases expose the tested tools; existing codex behavior remains covered and all affected maintained documentation is updated.
- The live harness is implemented and fixture-tested with explicit inputs, Chat-only health/send guards, six-send/15-minute limits, no retries and owner-browser preservation.

## Verify


codex --version
cargo test --test chatgptweb_codex_smoke capture_tool_surface -- --ignored --exact --nocapture --test-threads=1
cargo test --test chatgptweb_codex_smoke demo -- --ignored --exact --nocapture --test-threads=1
cargo test --test chatgptweb --test chatgptweb_adapter --test codex_multi_account --test inbound_codex_endpoint --test inbound_codex_routes
npm ci --prefix helpers/chatgptweb-browser
npm exec --prefix helpers/chatgptweb-browser -- playwright install chromium
npm test --prefix helpers/chatgptweb-browser
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features --workspace
npm ci --prefix site
npm run build --prefix site
git diff --check

# TASK-014 outcome

Status: delivered for review. PR: https://github.com/jesusvillota/shunt/pull/77.

## Acceptance

1. [x] Fake HTTP integration proves shell/edit calls, result replay, and valid
   streaming/non-streaming Responses output. This session passed all 15
   `chatgptweb_adapter` tests. Coverage includes function `exec_command`, custom
   `apply_patch`, correlated outputs in fresh-chat full transcripts, mixed text,
   compressed requests, monotonic sequence numbers, stable item/call IDs,
   arguments/input events and final snapshots. Streaming/non-streaming snapshots
   match after removing generated IDs and timestamps. Existing immediate text,
   profile serialization, disconnect cleanup, deadlines, failure/no-replay, and
   uncertain submission tests pass. Invalid declarations/history fail before
   Send; malformed records, wrong names/IDs, choice violations and partial-text
   errors fail without releasing executable calls. The 900,000-byte transcript
   passes, the 1 MiB request fails before dispatch, and frames split into single
   characters pass. The test records elapsed time without a timing threshold.
2. [x] The ignored `demo` ran real `codex-cli 0.164.0` against local shunt with
   the scripted Driver. Its four turns proved shell execution, actual patch
   `BEFORE\n` to `AFTER\n`, a readback, all correlated real tool results, and
   final `DEMO_COMPLETE` plus `turn.completed`. It checks command/file-change
   events and final file bytes. A valid edit record followed by malformed output
   caused no edit, failed completion, and exactly one driver request. The final
   demo run passed in 3.11 seconds. No browser, account or external model was
   used. Both capture variants (`gpt-5.5`, `gpt-6.1-sol`) also passed.
3. [x] All three catalog aliases advertise serial shell/freeform edit tools,
   no parallel calls, no search and text input. The 97 targeted tests passed:
   7 driver, 15 adapter, 32 Codex pool, 32 inbound endpoint, 11 inbound route.
   Full workspace verification also covers discovery and other provider catalogs.
   README and its ko/ja/zh-CN copies, the provider guide, all four provider site
   pages, and the configuration example now describe tools and the live gate.
   The site build passed for 213 pages. Driver wire protocol and production
   helper behavior are unchanged, so their documents need no behavior update.
   Generated wiki was not edited.
4. [x] The ignored `live` harness requires explicit profile/CDP inputs and
   fails if absent. Normal fixture tests prove input validation and profile
   exclusion. Its test-only launcher imports the production CLI, attaches to
   owner Chrome, and retains production Chat-only health and atomic Send guards.
   Shared ledger reservations enforce six Generate requests, 120-second turns,
   15-minute work deadline including health, and a permanent error/uncertain-send
   latch. Codex retry counts are zero. HOME/CODEX_HOME, rules and inherited
   connectors are isolated; model_provider remains shunt and workspace-write
   permissions remain active. The exact launcher fixture proves health sends
   nothing, Work and switch-on-paste send nothing, six-send/deadline bounds,
   uncertain submission without retry, helper SIGKILL and creation-race cleanup,
   and preservation of original and concurrently created owner tabs. Cleanup
   uses exact target IDs plus a unique marker before ID persistence. Page
   selection matches that marker, preventing selection of an owner-created tab.
   All 30 Node tests and the Rust browser-conformance test passed this session.

## Summary

The adapter validates complete requests through the existing contract and emits
native function/custom call events only after successful parser and driver
completion. No tool execution moves into shunt. Text continues to stream and
usage stays null. No dependency, public configuration key, credential-writeback,
production helper change, Codex pool change or account-rotation change was added.

The real-Codex offline demo and bounded live harness use the existing Driver
injection and helper override. Live teardown closes explicitly owned targets,
including helpers killed before normal cleanup, while retaining owner Chrome.
Independent review found cleanup and concurrent-tab races; both were fixed and
covered by fixtures. The reviewer reran the final launcher fixture and reported
no further concrete defects. GitHub PR threads, reviews and comments were empty
when checked. No GitHub Actions were inspected or used as delivery gates.

Verified implementation revision: `2c6535eac7ca97d5ab67d80457517ed7eb6900cd`.
Only this required outcome changes after verification. Branch remains
`goal/chatgptweb-provider/task-014`; PR base is `main`. Rebase onto `origin/main`
succeeded without conflicts. All implementation commits were pushed.

## Verification

Environment: `PYO3_PYTHON=/opt/homebrew/bin/python3.13`,
`RUST_TEST_THREADS=1` for the full workspace; existing generated UI bundle.

| Command | Result in this session |
| --- | --- |
| `codex --version` | exit 0; codex-cli 0.164.0 |
| `cargo test --test chatgptweb_codex_smoke capture_tool_surface -- --ignored --exact --nocapture --test-threads=1` | exit 0; both captured surfaces passed |
| `cargo test --test chatgptweb_codex_smoke demo -- --ignored --exact --nocapture --test-threads=1` | exit 0; shell/edit/readback/results/completion and malformed no-edit passed |
| `cargo test --test chatgptweb --test chatgptweb_adapter --test codex_multi_account --test inbound_codex_endpoint --test inbound_codex_routes` | exit 0; 97 passed |
| `cargo test --test chatgptweb_codex_smoke -- --test-threads=1` | exit 0; 2 passed, 3 explicit opt-in tests ignored |
| `npm ci --prefix helpers/chatgptweb-browser` | exit 0 |
| `npm exec --prefix helpers/chatgptweb-browser -- playwright install chromium` | exit 0 |
| `npm test --prefix helpers/chatgptweb-browser` | exit 0; 30 Node tests and 1 Rust browser-conformance test passed |
| `cargo fmt --all --check` | exit 0 |
| `cargo clippy --all-targets --all-features -- -D warnings` | exit 0; rerun after final launcher changes |
| `cargo test --all-features --workspace` | exit 0; 3919 passed, 0 failed, 9 ignored, 52 result groups |
| `npm ci --prefix site` | exit 0 |
| `npm run build --prefix site` | exit 0; 213 pages |
| `git diff --check` | exit 0 |

The workspace suite was run after rebase. Subsequent changes only tighten the
Node launcher/fixture; the final helper suite, clippy and real-Codex demo were
rerun for those changes. Added files are tests, test support or this required
outcome. No task specification, other task, decision or goal.yaml was edited.
No tests were weakened or removed. No merge occurred.

No live account sends or live prefill-checkpoint inference were run. Browser
reliability and real-model framing adherence remain for TASK-015's live gate.
No decision, access or data blocker remains.

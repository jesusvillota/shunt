# TASK-013 outcome

Status: ready for review. All required local Verify commands pass.
PR: https://github.com/jesusvillota/shunt/pull/72.

## Acceptance

1. [x] The contract and sanitized Codex fixtures cover actual shell/edit tools
   and next-turn results. In this session, `codex --version` returned
   `codex-cli 0.164.0`. The ignored `capture_tool_surface` test passed for
   `gpt-5.5` and `gpt-6.1-sol` against numeric loopback. It verified complete
   declarations, direct `exec_command` JSON arguments, freeform `apply_patch`,
   namespaced `functions.exec`, unchanged call IDs, string and ordered text-part
   outputs, the seed read, and the scratch file edit. The normal fixture test
   also passed in the full workspace run. Unsupported direct `tool_search`
   inventories are explicitly rejected; the supported code-mode inventory
   validates without filtering.
2. [x] All 13 protocol unit tests passed in this session. They cover ordered
   transcript/content preservation, strict schema/kind/name/choice validation,
   stable unique IDs and historical collision avoidance, every UTF-8 split and
   single-character chunks, Unicode, immediate text delivery, duplicate JSON
   rejection, poisoned parsers, record/answer/call limits, and terminal handling.
   Calls remain held until driver completion. Instructions and tool results
   remain quoted data.
3. [x] All existing text-only tests passed in this session: 7 driver tests and
   11 adapter tests. No live browser prompt was sent. Comparison with
   `origin/main` was empty for config, discovery, text translation, process
   transport, driver types/FakeDriver, and the Node helper. No provider semantics
   or helper wire v1 changed.

## Summary

The isolated internal contract serializes complete ordered transcripts and
validates declarations, choices, calls, and matching outputs before dispatch.
Its incremental parser streams ordinary text and retains bounded validated
calls until successful driver completion. Shunt never executes tools; Codex
owns tool syntax checks, execution, and permissions. Custom input and grammar
text remain intact. No dependency, public config, or credential behavior changes.

The reproducible loopback capture uses an isolated HOME/CODEX_HOME, a scratch
Git repository, fixed shell/edit calls, and no real provider. Sanitized fixtures
remove private instructions and normalize paths, IDs, and measurements.
`docs/chatgptweb-provider.md` specifies exact framing, examples, transcript
ordering, validation, limits, failure handling, and residual prompt-injection
risk. README, milestone docs, and translated site surfaces were considered;
current setup and supported behavior do not change. Generated wiki is untouched.
TASK-014 owns adapter integration; TASK-015 owns live model adherence.

DEC-007 required a separate fix for issue #73. That fix was delivered in PR #75
and is present in `origin/main`. This session rebased onto
`c491c9ff7e7f01390cee571a266d50ca52c9b5d0` without conflicts and reran every
Verify command. All three formerly failing discovery tests now pass. No
unrelated repair was added to this task. No decision or access blocker remains.

Verified revision: `7dafdba3870f6086b8b1c10411ed516db69b8aec`.
Only this outcome record changes after verification. The implementation commit
is `8c2927b3`. Branch: `goal/chatgptweb-provider/task-013`; PR base: `main`.
An independent read-only review in this session found no issues. GitHub review
threads, reviews, and comments were empty. No task spec, other task, decision,
goal metadata, or browser profile was edited. No merge occurred.

## Verification

Environment: `PYO3_PYTHON=/opt/homebrew/bin/python3.13`,
`RUST_TEST_THREADS=1`, with the existing generated UI bundle.

| Command | Result in this session |
| --- | --- |
| `codex --version` | exit 0; codex-cli 0.164.0 |
| `cargo test --test chatgptweb_codex_smoke capture_tool_surface -- --ignored --exact --nocapture --test-threads=1` | exit 0; 1 passed, both model surfaces verified |
| `cargo test --lib adapters::chatgptweb::tool_protocol` | exit 0; 13 passed |
| `cargo test --test chatgptweb --test chatgptweb_adapter` | exit 0; 7 driver and 11 adapter tests passed |
| `cargo fmt --all --check` | exit 0 |
| `cargo clippy --all-targets --all-features -- -D warnings` | exit 0 |
| `cargo test --all-features --workspace` | exit 0; 3914 passed, 0 failed, 7 ignored across 52 result groups |
| `git diff --check` | exit 0 |

All added files are code, tests, fixtures, or this required outcome. No assertion
was removed or weakened. GitHub Actions were not inspected or used as gates.
Live sends and live prefill checkpoint inference were not run. The contract's
real-model adherence remains for the separate integration and live tasks.

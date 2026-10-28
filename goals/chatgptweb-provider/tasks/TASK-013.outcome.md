# TASK-013 outcome

Status: blocked. Required workspace Verify fails in three unchanged discovery catalog tests.
Draft PR: https://github.com/jesusvillota/shunt/pull/72.
Reported baseline bug: https://github.com/jesusvillota/shunt/issues/73.

## Acceptance

- [x] The contract and sanitized Codex fixtures cover the actual shell/edit tool
  surface and next-turn results. `codex --version` reports `codex-cli 0.164.0`.
  The required ignored `capture_tool_surface` test passed for `gpt-5.5` and
  `gpt-6.1-sol` against numeric loopback. It asserts complete declarations,
  direct `exec_command` JSON arguments, freeform `apply_patch`, namespaced
  `functions.exec`, unchanged call IDs, string versus ordered text-part results,
  seed contents and the actual scratch file edit. Complete direct inventories
  contain unsupported `tool_search`: tests assert rejection before validating
  the supported shell/edit subset. Complete current code-mode inventories
  validate without filtering. Nothing silently filters tools before dispatch.
- [x] All 13 protocol unit tests passed. They prove ordered transcript/content
  preservation, strict schema/kind/name/choice validation, unique stable IDs,
  historical collision avoidance, every UTF-8 split and single-character chunks,
  Unicode, text delivery before completion, duplicate JSON rejection, poisoned
  parsers, record/answer/call bounds, and explicit terminal handling. Calls remain
  held until driver completion. Instructions and results remain quoted data.
- [x] Existing text-only tests pass: 7 driver tests and 11 adapter tests. No live
  browser prompt was sent. `git diff --exit-code origin/main --
  src/adapters/chatgptweb/translate.rs src/adapters/chatgptweb/process.rs
  src/chatgptweb helpers/chatgptweb-browser src/discovery.rs` exited 0.
  Adapter semantics, discovery, config and helper wire v1 are unchanged.

The task is **not delivered**: the required full workspace command exits 101.
The failure is independent of these acceptance tests and must be resolved
before this PR becomes ready. No tests were removed, weakened or skipped.

## Decision request

Should a separate fix restore discovery verification before TASK-013 resumes, or may this task repair the unrelated catalog?

1. Fix https://github.com/jesusvillota/shunt/issues/73 in a separate focused PR, then resume TASK-013 and rerun every
   Verify command. This respects the owner's task boundary and keeps provider
   discovery changes outside the tool-protocol PR. It delays readiness.
2. Authorize a scope exception for TASK-013 to repair catalog ordering and the
   two expected-response fixtures. This can restore verification sooner, but
   mixes an unrelated discovery change into this task and conflicts with the
   standing instruction unless the owner authorizes it.

### Recommendation

Use option 1. Keep this PR draft until the separate catalog fix is available.
Do not waive local verification or weaken assertions. The root cause is known;
no live browser access or additional tool-protocol design decision is needed.

## Summary

Implemented an isolated internal transcript contract and incremental answer
parser beside `translate.rs`. Ordinary text streams immediately. Only bounded,
validated calls are retained, then released as an ordered batch on successful
completion. Shunt never executes tools; Codex owns syntax checks and permissions.
Custom input and grammar declarations are preserved without interpreting Lark,
regex or JavaScript. Unsupported tools, schemas and histories fail before send.

The lasting loopback capture test isolates HOME/CODEX_HOME and credentials,
creates a scratch Git repository, executes only fixed shell/edit calls through
Codex, asserts continuation shapes and deletes scratch data. Sanitized fixtures
remove private instruction messages and normalize paths, generated IDs and
measurements. Fake responses emit calls/messages only; they do not establish
opaque reasoning support or real model adherence.

The existing provider engineering document specifies exact versioned framing,
examples, transcript ordering, choices, namespaces, schema support, byte/count
limits, terminal handling, no-replay behavior and residual prompt-injection risk.
Client permissions remain necessary for model-generated calls. The production
adapter still rejects tools; TASK-014 owns integration and TASK-015 owns live
adherence. README, milestone docs and translated site surfaces were considered;
current setup and supported behavior do not change. Generated wiki is untouched.

Plan followed: offline capture, isolated contract, focused tests, documentation,
independent review and required local checks. Review identified strict-type and
long-turn-ID gaps; regression tests cover both. The reviewer confirmed all fixes
and found no remaining issues. The only deviation is stopping at the unrelated
workspace failure, under the worker Decision request protocol.

Verified implementation revision: `b8b1255b55f1de851d1d90b5de7fff5202bff6f0`. The outcome commit records only
evidence. Branch remains `goal/chatgptweb-provider/task-013`; rebase onto
`origin/main` succeeded without conflicts. No task spec, other task, decision,
`goal.yaml`, credential writeback, public config or helper wire changed.
No merge occurred. GitHub Actions were not read or used as gates.

## Verification

Environment: `PYO3_PYTHON=/opt/homebrew/bin/python3.13` and
`RUST_TEST_THREADS=1`. The required UI bundle was built with
`npm ci --prefix ui` and `npm run build --prefix ui`, both exit 0.

```text
$ codex --version
codex-cli 0.164.0
exit 0
```

```text
$ cargo test --test chatgptweb_codex_smoke capture_tool_surface -- --ignored --exact --nocapture --test-threads=1
     Running tests/chatgptweb_codex_smoke.rs (target/debug/deps/chatgptweb_codex_smoke-70ed86925724992f)

running 1 test
test capture_tool_surface ... codex-cli 0.164.0
gpt-5.5: declarations, shell/edit calls, correlated continuation outputs, scratch edit verified
gpt-6.1-sol: declarations, shell/edit calls, correlated continuation outputs, scratch edit verified
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 3.98s

exit 0
```

```text
$ cargo test --lib adapters::chatgptweb::tool_protocol
test adapters::chatgptweb::tool_protocol::tests::every_unicode_split_streams_text_and_holds_calls_until_driver_done ... ok
test adapters::chatgptweb::tool_protocol::tests::invalid_records_fail_closed_and_poison_parser ... ok
test adapters::chatgptweb::tool_protocol::tests::malformed_item_types_and_overlong_turns_fail_before_dispatch ... ok
test adapters::chatgptweb::tool_protocol::tests::named_custom_choice_accepts_custom_call ... ok
test adapters::chatgptweb::tool_protocol::tests::ordered_transcript_and_outputs_are_quoted_data ... ok
test adapters::chatgptweb::tool_protocol::tests::strict_requests_reject_unsupported_and_malformed_history ... ok
test adapters::chatgptweb::tool_protocol::tests::terminal_duplicate_ids_choices_and_limits ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 3024 filtered out; finished in 0.11s

exit 0
```

```text
$ cargo test --test chatgptweb --test chatgptweb_adapter
test chatgptweb_example_has_one_account_and_disabled_accounts_do_not_send ... ok
test chatgptweb_nonstreaming_and_default_provider_work ... ok
test chatgptweb_partial_failure_never_completes_or_replays ... ok
test chatgptweb_process_client_streams_and_rejects_premature_eof ... ok
test chatgptweb_rejects_tools_images_and_continuation_ids_in_openai_shape ... ok
test chatgptweb_route_can_omit_effort_without_using_the_public_model ... ok
test chatgptweb_stream_delivers_delta_before_completion_and_holds_profile ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.64s

exit 0
```

```text
$ cargo fmt --all --check
(no output)
exit 0
```

```text
$ cargo clippy --all-targets --all-features -- -D warnings
    Checking shunt-gateway v0.51.1 (/Users/jesusvillotamiranda/orca/workspaces/shunt/goal-chatgptweb-provider-task-013)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 14.78s
exit 0
```

```text
$ cargo test --all-features --workspace


failures:
    discovery::tests::builtin_table_is_newest_first_with_unique_ids
    discovery::tests::curated_models_precede_and_override_matching_builtins
    discovery::tests::default_returns_builtin_models_in_api_order

test result: FAILED. 3181 passed; 3 failed; 5 ignored; 0 measured; 0 filtered out; finished in 74.93s

error: test failed, to rerun pass `--lib`
exit 101
```

`git diff --check` also exited 0 after the full-suite failure.

The focused discovery reproduction fails the same three tests. The unchanged
built-in table appends four July 2026 Antigravity rows after August 2025 rows;
two response fixtures omit them. Source comparison against origin/main exits 0.
The complete workspace run stops after its library binary, so subsequent
all-feature integration binaries were not run. Live sends and checkpoint tests
were not run. The native goal remains incomplete.

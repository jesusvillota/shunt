## Acceptance

- [x] Legacy `"ranker": "claude"` still loads and uses the exact TASK-004 argv. `rerank::tests::legacy_strings_run_with_exact_task004_argv` loads preferences from disk and records the command through an injected `Runner` for all four platforms. `models_prefs::tests::ranker_objects_round_trip_and_unselected_strings_stay_strings` preserves strings during unrelated edits.
- [x] Object settings pass model and effort through real CLI flags. `rerank::tests::selected_models_and_efforts_reach_each_cli_opencode_ignores_effort` asserts exact argv for all four platforms with neither field, either field, and both fields. Codex includes `-m gpt-6-sol -c 'model_reasoning_effort="high"'`. **OpenCode ignores effort**: the installed CLI exposes no effort/variant flag. Claude and the existing AGY bridge use `--model` and `--effort`.
- [x] The picker stores the choice and the OpenCode tab shows `platform · model · effort`. `tui::models::tests::ranker_picker_saves_the_catalog_model_and_its_effort_as_an_object` drives platform/model/effort selection, saves the JSON file and loads it again. Cancellation preserves legacy settings. `tui::view::tests::opencode_shows_the_selected_ranker_and_the_picker_key` verifies rendering and the `s` footer hint. `tui::app::tests::rerank_uses_the_current_picker_choice_before_the_disk_write_finishes` verifies immediate dispatch.
- [x] `chatgptweb` remains refused with the unchanged browser-session message. String and object tests cover refusal; the object test also proves that the injected runner never runs.
- [x] Documentation is updated in English, Korean, Japanese and Simplified Chinese: all four READMEs, both affected site pages and their locale copies, plus the English-only `docs/pool-tui.md`. Generated `wiki/` was considered and left for its separate regeneration workflow.
- [x] Required format, clippy and full tests pass. All five Verify commands exited 0 together with `RUST_TEST_THREADS=1`. The workspace suite passed 3,823 tests (5 existing ignored tests); the focused rerank run passed 14 tests (1 existing live-model test ignored). The live OpenCode JSON report returned models with `error: null`.

## Summary

The Models dialog adds an `s` picker for ranker platform, catalog model and effort. Codex retains live per-model effort metadata; Claude uses latest-model aliases. A choice writes an object. Unchanged legacy strings retain their representation and TASK-004 command. The OpenCode tab displays the selection. Ranking takes the current in-memory choice, then reloads preferences after the run so it does not save the snapshot from before the run.

The installed commands were inspected before implementation: Claude Code 2.1.302 (`--model`, `--effort`), codex-cli 0.163.0 (`exec -m`, `-c`), OpenCode v2.0.32 (`run -m`, no effort flag), and AGY 1.4.0 (`--model`, `--effort`). The AGY ranker intentionally retains TASK-004's `claude -p` bridge and `claude-gemini-3.8-flash-medium-via-antigravity` fallback model. OpenCode effort is omitted, documented and visibly marked as ignored. This follows the spec's no-effort-flag rule; no material decision or scoring change was needed.

Plan: preserve preferences compatibility, verify CLI flags, implement catalog selection and display, update translations, and run local checks. Tests use an injected runner and do not call real models. Existing tests were retained in adjacent child modules to keep the production modules focused.

Verified revision: `9c4e046b` (feature implementation), tested in this session. This outcome is a documentation-only follow-up. Site rendering was not run because site dependencies are not installed. No follow-up is required for TASK-005; weighted scoring remains TASK-006's scope.

## Verification

All commands below exited 0 in one sequence, with `RUST_TEST_THREADS=1` exported:

```sh
cargo fmt --all --check
RUSTFLAGS="-D warnings" cargo clippy --all-targets --features tui,bench -- -D warnings
RUSTFLAGS="-D warnings" cargo test --features tui,bench --workspace
cargo test --features tui --lib rerank
cargo run -q -- models opencode --json
```

The successful log ended with:

```text
TASK-005 VERIFY: all five commands exited 0 (RUST_TEST_THREADS=1)
```

Two default parallel workspace runs hit the previously recorded Codex token-estimator timing bug (`pool_rotation_still_seeds_input_token_estimate_after_401`, then `pool_http_dispatch_seeds_message_start_input_token_estimate`). The isolated rotation test passed. Both tests and the entire suite passed with one test thread. No tests were weakened and the unrelated estimator path was not changed. The duplicate bug record already exists at `~/GitHub/config/agent-inbox/bugs/20261014T100821Z-32904-Codex-input-token-estimate-test-can-fail-under-c.md`, so no duplicate note was added. Default-feature commands emit the existing `MAX_TRACKED_CHILD_PINS` unused-import warning; the required warnings-as-errors checks with `tui,bench` pass.

PR: https://github.com/jesusvillota/shunt/pull/27, targeting `main`, labeled `goal:model-filter`. GitHub reported an empty `statusCheckRollup`; there were no remote checks to wait for. The PR remains a draft because GitHub refuses the ready-for-review operation with a GraphQL rate-limit error. No merge is authorized or performed.

## Blocked

GitHub GraphQL rate limit prevents marking PR ready. All six implementation acceptance items and all five local Verify commands passed. Delivery condition (A) remains unsatisfied because PR #27 is still a draft and the required final `gh pr view` cannot run through GraphQL.

Evidence from this session: `gh pr ready 27 --repo jesusvillota/shunt`, the direct `markPullRequestReadyForReview` mutation, and `gh pr view` all returned `API rate limit already exceeded for user ID 145339398`. REST confirms that PR #27 is open, targets `main`, remains a draft, and carries `goal:model-filter`. REST access and git pushes still work. The REST quota endpoint reports available GraphQL points, but GraphQL requests continue to be refused; a reliable retry time is not available from that contradictory response.

The PR body records `Goal-Blocked: GitHub GraphQL rate limit prevents marking PR ready`. `gh pr edit --body-file` was attempted first; because it also requires GraphQL, the blocker was written through the still-working REST API with the same real-newline body. No code or tests need further changes. Resume by marking PR #27 ready and verifying `gh pr view --json url,state,isDraft,baseRefName,body,labels` after GraphQL access returns. This is an access blocker, not a material decision request. The native goal has not been marked complete.

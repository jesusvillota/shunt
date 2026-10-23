# TASK-004 outcome

Status: delivered. Local Verify passes. PR: <https://github.com/jesusvillota/shunt/pull/55>.

## Acceptance

- [x] One `chatgptweb` provider with one account appears in `shunt.toml.example`.
  `chatgptweb_example_has_one_account_and_disabled_accounts_do_not_send` parses
  that example and checks its kind and single account. DEC-002's accepted kind
  is `chatgptweb_browser`; `chatgptweb` is the configured provider instance.
- [x] A Responses request traverses `[server.codex_endpoint]` to TASK-002's fake
  and returns valid streamed Responses events.
  `chatgptweb_codex_endpoint_streams_full_transcript_through_fake` passes with
  plain and zstd requests, a full multi-turn transcript, Unicode deltas,
  monotonically increasing sequence numbers, stable IDs, and final message text.
  `chatgptweb_stream_delivers_delta_before_completion_and_holds_profile` proves
  that text arrives before completion and the profile stays locked.
- [x] Browser models appear in the catalog Codex reads.
  `chatgptweb_catalog_lists_configured_models_on_every_codex_alias` passes for
  `/v1/models?client_version=0.152.0`, `/models`, and
  `/backend-api/codex/models`, checking the public model ID and text-only metadata.
- [x] Existing `codex` tests pass unchanged.
  The full default suite passes. The Codex pool, fallback, gated WebSocket,
  inbound endpoint, routes, and inbound WebSocket binaries pass with the counts
  below. `git diff origin/main -- tests/codex_multi_account.rs
  tests/codex_websocket_fallback.rs tests/codex_websocket_gated.rs
  tests/inbound_codex_endpoint.rs tests/inbound_codex_routes.rs
  tests/inbound_codex_websocket.rs src/adapters/responses` is empty.

## Summary

Added a distinct browser provider kind and text-only Responses adapter. The
adapter sends instructions and the ordered whole transcript through the
versioned Driver interface. It streams Responses events without waiting for
completion, emits failure instead of false completion, rejects tools/images
and continuation IDs, serializes a profile through body completion, honors
account disablement, and never retries or falls back to Codex/Work.

The process client uses private JSON Lines pipes with the TASK-003 CLI contract:
`node cli.mjs --account NAME --profile-dir PATH`. It bounds frames, correlates
IDs, enforces deadlines, and kills/reaps helpers on disconnect or failure.
Cleanup also handles detached browser descendants, verified by
`chatgptweb_disconnect_kills_a_detached_browser_descendant`. A pipe fixture
checks real process framing and premature EOF without a browser. Ten new
adapter tests and all seven existing driver conformance tests pass.

The example, English engineering setup note, all four root READMEs, and a
provider site page in English, Korean, Japanese, and Chinese are updated.
Existing milestone/spec behavior was considered; the new engineering note
records this subsystem. Generated `wiki/` is untouched. No task specs, other
outcomes, decisions, goal.yaml, credential files, or Codex settings changed.

Verified implementation revision: `dbb12369a68c4e79e7f81ebfcc4cc565b0187622`. This outcome commit changes
only acceptance evidence. The branch was rebased onto `origin/main` at
`ab4afdd1` without conflicts, and remains `goal/chatgptweb-provider/task-004`.
The PR base is `main`, with the required marker and goal label. It is marked
ready after this committed outcome is pushed; nothing is merged.

Deviations: none from the text-only task. A Rust process client connects the
adapter to the separately delivered Node helper; no browser automation is
copied into this PR. The configured instance uses `chatgptweb`, while the
accepted provider kind remains `chatgptweb_browser`.

Limits and proposed follow-ups: no real browser, logged-in account, or live
quota check ran here. TASK-003 supplies the helper, TASK-005 supplies login,
and TASK-006 verifies live Chat-mode/quota behavior. Tool calls and account
rotation remain later milestones. The process tests need Node but no browser
or installed helper dependencies. Existing ignored live tests remain ignored.

Earlier concurrent full runs intermittently failed two unchanged input-token
estimate assertions. These are recorded in
<https://github.com/jesusvillota/shunt/issues/56>, after duplicate searches.
The affected fallback test passed alone. Final full-suite runs use
`RUST_TEST_THREADS=1` to reduce scheduling contention; every test and assertion
remains enabled. Both default and all-feature suites pass. No unrelated
estimator change or test weakening is included. GitHub Actions were not read
or used as delivery gates.

## Verification

```text
$ RUST_TEST_THREADS=1 cargo test
running 4 tests
test a_child_without_a_by_type_entry_lands_on_target ... ok
test an_explore_child_lands_on_its_by_type_upstream ... ok
test compaction_and_auxiliary_stay_on_the_parent ... ok
test main_with_an_agent_id_stays_on_the_parent ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

     Running tests/tui_client.rs (target/debug/deps/tui_client-c5558d30fc617895)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests shunt

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

exit 0
Full-suite total: 3728 passed; 0 failed; 3 existing ignored.

$ cargo clippy --all-targets -- -D warnings
    Checking shunt-gateway v0.51.1 (/Users/jesusvillotamiranda/orca/workspaces/shunt/goal-chatgptweb-provider-task-004)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 12.79s
exit 0

$ RUST_TEST_THREADS=1 cargo test --all-features --workspace

running 1 test
test polls_pauses_resumes_and_starts_provisioning ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s

   Doc-tests shunt

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

exit 0
Full-suite total: 3890 passed; 0 failed; 5 existing ignored.

$ cargo clippy --all-targets --all-features -- -D warnings
    Checking shunt-gateway v0.51.1 (/Users/jesusvillotamiranda/orca/workspaces/shunt/goal-chatgptweb-provider-task-004)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 14.03s
exit 0

$ cargo fmt --all --check
exit 0
$ git diff --check
exit 0
$ npm run build --prefix ui
dist/assets/index-ww5dFsSm.js   370.65 kB │ gzip: 118.81 kB
✓ built in 685ms
exit 0
$ npm run build --prefix site
19:23:34 [build] 213 page(s) built in 7.18s
19:23:34 [build] Complete!
exit 0
```

Selected full-suite results:

```text
chatgptweb_adapter: test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.53s
codex_multi_account: test result: ok. 32 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.18s
codex_websocket_fallback: test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.67s
codex_websocket_gated: test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.32s
inbound_codex_endpoint: test result: ok. 32 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.25s
inbound_codex_routes: test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
inbound_codex_websocket: test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.34s
passthrough: test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.66s
```

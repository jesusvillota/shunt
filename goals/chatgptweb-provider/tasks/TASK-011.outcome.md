# TASK-011 outcome

## Acceptance

- [x] `cargo clippy --all-targets -- -D warnings` passes with default features.
  Evidence: session run on revision `b8f34715` exited 0; output ended with
  `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 1m 18s`.
- [x] `cargo clippy --all-targets --features bench -- -D warnings` and `cargo test` still pass.
  Evidence: bench clippy exited 0, finishing in 29.40s. The unchanged full
  `cargo test` rerun exited 0 and completed through `Doc-tests shunt`.
  The first full test run exited 101 on
  `message_start_seeds_tiktoken_estimate_for_streaming_responses_model`.
  That test passed alone, then the full suite passed without code or test changes.
  Existing issue <https://github.com/jesusvillota/shunt/issues/36> documents the
  same cold-token-encoder timing failure; no duplicate issue was opened.

Additional Verify: `cargo fmt --all --check` exited 0 with no output.
The PR contains all four passing command output tails and the initial failure.

## Summary

- What: gate the `MAX_TRACKED_CHILD_PINS` re-export on `feature = "bench"` alone.
- How: split the cap re-exports; retain the existing test/bench gate for
  `MAX_TRACKED_SESSIONS`. Update the adjacent comment to describe the consumers.
- Scope: no warning suppression, test changes, public behavior changes, or new tests.
  The required default and bench clippy commands directly check the affected builds.
- Documentation: considered README sources and translations, docs/, and site
  sources and translations. No user behavior or setup changed, so none need edits.
  wiki/ is generated and was not edited.
- Deviations: none. The unrelated timing failure required one unchanged full rerun.
- Verified revision: `b8f34715` (code), based on `origin/main` at `07d54161`.
  The subsequent outcome commit changes this report only.
- Delivery: <https://github.com/jesusvillota/shunt/pull/53>, targeting `main`.
- Proposed follow-ups: none for TASK-011. Token-estimate timing already has issue #36.

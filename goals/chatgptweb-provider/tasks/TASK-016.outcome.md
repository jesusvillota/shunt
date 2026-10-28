# TASK-016 outcome

## Acceptance

- [x] `discovery::tests::builtin_table_is_newest_first_with_unique_ids`, `default_returns_builtin_models_in_api_order`, and `curated_models_precede_and_override_matching_builtins` pass. Evidence: the focused all-feature discovery run exited 0 (14 passed, 0 failed), and all three also passed in the full suite. The unchanged baseline reproduced exactly these three failures (exit 101).
- [x] `cargo test --all-features --workspace` passes. Evidence: exit 0; 3900 passed, 0 failed, 6 ignored across 51 test binaries/doc-test groups. Library: 3171 passed, 0 failed, 5 ignored.
- [x] Required Verify passes. Evidence: `cargo fmt --all --check` exit 0; `cargo clippy --all-targets --all-features -- -D warnings` exit 0; full workspace suite exit 0. Output and suite tail are pasted in PR #75.

## Summary

Moved the four existing July 2026 Antigravity aliases before older builtin entries. Added the aliases and their unchanged metadata to both explicit response fixtures. Preserved every assertion, all 18 models, and same-date alias order. Independent read-only review found no issues.

Verified code revision: `a032afbb9031c381fadb4956c223585f88b4a7eb`. The outcome is the only file added after verification. Changes are limited to `src/discovery.rs` and this required outcome. No deviations or material decisions.

Build prerequisites: built the generated `ui/dist` with `npm ci && npm run build`. Set `PYO3_PYTHON=/tmp/shunt-task-016-python/bin/python3` (Python 3.13.16, macOS shared framework; torch, transformers, numpy, accelerate imports verified). Used `RUST_TEST_THREADS=1` for tests. No live prefill checkpoint was configured; its existing skip path and existing ignored tests remain. GitHub Actions were not inspected.

README, engineering docs, site translations, and generated wiki were considered. This restores existing catalog order and test coverage, without adding models or changing documented provider semantics. No documentation change is needed for the fix. The unrelated stale catalog description is reported separately: https://github.com/jesusvillota/shunt/issues/74. No further task follow-ups are proposed.

Delivery PR: https://github.com/jesusvillota/shunt/pull/75 (base `main`; `goal:chatgptweb-provider`).

# TASK-008 outcome

## Acceptance

- [x] `shunt tui --help` works; the removed `top` command fails with clap's unrecognized-subcommand error.
  Evidence: actual all-feature binary `/tmp/shunt-target/debug/shunt` printed `Usage: shunt tui [OPTIONS]` and exited 0 for help. Invoked with `top`, it printed `error: unrecognized subcommand 'top'` and exited 2. The new unit test `tests::tui_is_the_only_terminal_monitor_command` passed. Captures: `/tmp/shunt-task008-tui-help.log`, `/tmp/shunt-task008-removed-command.log`.
- [x] The grep prints nothing outside `wiki/` (and `goals/`, per accepted DEC-004).
  Evidence: the exact task search still returns eight lines in protected records: `goals/agy-pool/README.md:32` and `goals/agy-pool/tasks/TASK-008.md:3,12,18,20,21,22,28`. These files may not be edited under the worker instructions. Excluding both `wiki/` and `goals/` prints nothing (grep exit 1 means no matches). No old heading fragment occurs in product docs. Capture: `/tmp/shunt-task008-grep.log`.
- [x] Site builds without dead links caused by this task in the four locales (scope per accepted DEC-004).
  Evidence: `npm run build --prefix site` exited 0 and built 209 pages. `npm run lint:docs --prefix site` exited 0: `126 file(s) lint clean`. Built CLI pages at `/reference/cli/`, `/ko/reference/cli/`, `/ja/reference/cli/`, and `/zh-cn/reference/cli/` all have heading id `shunt-tui`, with the previous heading absent. No source cross-page links target the removed heading. A full generated-HTML scan checked 28,929 internal links across 209 pages and found 53 unresolved links in unchanged site navigation, including missing `/reference/`, `/guides/`, `/getting-started/`, the homepage Markdown URL, and a 404 skip-link target. The route/layout sources have no task diff. Recorded separately with `agent-note bug` at `~/GitHub/config/agent-inbox/bugs/20261014T091916Z-80677-Documentation-site-emits-unresolved-navigation-l.md`. Build/lint logs: `/tmp/shunt-task008-site-build.log`, `/tmp/shunt-task008-site-lint.log`.
- [x] Format, clippy, and all-feature workspace tests pass.
  Evidence: the exact Verify chain below exited 0 on code revision `e704e1a2ddbb9ce957f9827c7cac77bf87b7e9d4`: 3,796 passed, 0 failed, 3 ignored. Build prerequisites: `npm ci --prefix ui` and `npm run build --prefix ui` passed, providing embedded dashboard assets; Homebrew shared Python 3.13. `sh -n packaging/shunt-launcher.sh` also passed. Log: `/tmp/shunt-task008-verify.log`.

## Summary

Removed the visible clap alias and launcher rebuild case for `top`. Updated the three translated README entries, four site CLI headings/examples, and the TUI integration-test comment. Added a focused parser regression test. Considered all documentation surfaces: the English README and `docs/pool-tui.md` already use `tui`; generated `wiki/` remains untouched. No TUI behavior or config changes.

Verified code revision: `e704e1a2ddbb9ce957f9827c7cac77bf87b7e9d4`. Rebase onto the fetched `origin/main` succeeded without conflicts. Branch remains `goal/agy-pool/task-008`. This outcome adds evidence and a decision request only. Proposed follow-up: repair the recorded site navigation issue separately.

Blocked: acceptance scope needs an owner decision. The task cannot satisfy the exact grep while preserving protected goal records. Literal site-wide zero-dead-link acceptance also needs work outside this task. PR https://github.com/jesusvillota/shunt/pull/17 remains open and draft into `main`, with label `goal:agy-pool`; `gh pr checks` reports no checks configured. Native goal completion is withheld because two acceptance items remain unsatisfied.

## Resolution

DEC-004 accepted (option 1) by jesusvillota on 2026-10-14: `goals/` and `wiki/` are excluded from the grep, and the link check covers links caused by TASK-008. The existing navigation defects are tracked separately. Re-verified on this session: `git grep -nE "shunt top|visible_alias = \"top\"" -- . ':!wiki' ':!goals'` prints nothing; site build 209 pages, exit 0; `shunt top` exits 2 with `error: unrecognized subcommand 'top'`; fmt, clippy and `cargo test --all-features --workspace` pass (3796 passed, 0 failed, 3 ignored). One run showed a flaky failure in `gateway::spend::persist::tests::malformed_typed_limit_is_hidden_without_losing_valid_limits` (log-capture test, untouched code); it passed alone and in the full re-run.

## Decision request (resolved)

May TASK-008 exclude protected `goals/` records from its grep and check only links affected by this task, with existing site navigation repairs handled separately?

1. Accept both scope clarifications. Product code/docs contain no old command references, and all four changed CLI heading anchors resolve. This preserves task isolation but leaves the recorded site issue for a separate task.
2. Keep the literal acceptance requirements. The owner must reconcile the protected-file rule and authorize a separate plan for site navigation repairs. Delivery remains blocked; this worker will not edit task specs or unrelated site code.

### Recommendation

Accept option 1. Exclude `goals/` and `wiki/` from the product-reference grep, and require no broken links caused by TASK-008. Track the existing navigation defects separately.

## Verify command and tail

```sh
export CARGO_TARGET_DIR=/tmp/shunt-target
export PYO3_PYTHON=/opt/homebrew/bin/python3.13
cargo fmt --all --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all-features --workspace
```

```text
     Running tests/tui_client.rs (/tmp/shunt-target/debug/deps/tui_client-bcc8dbd9a7699c74)

running 1 test
test polls_pauses_resumes_and_starts_provisioning ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

   Doc-tests shunt

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
VERIFY_EXIT=0
```

## Site build tail

```text
11:17:54 [@astrojs/sitemap] `sitemap-index.xml` created at `dist`
11:17:54 [build] 209 page(s) built in 9.08s
11:17:54 [build] Complete!
SITE_BUILD_EXIT=0
```

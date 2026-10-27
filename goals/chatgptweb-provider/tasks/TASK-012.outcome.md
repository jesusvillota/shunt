# TASK-012 outcome

Status: **blocked**. Installed Google Chrome cannot finish the default Playwright launch. No window became available for manual sign-in. The launch timed out after 180 seconds, before navigation, browser-check inspection or headed health. TASK-012 is not delivered.

## Acceptance

1. [ ] Signed-in `cemfi` health returns `ok`, with live Chat selection.
   Not reached in this session: Chrome failed during `launchPersistentContext`, before a page or driver health result existed. No successful health output is claimed.
2. [ ] Sanitized live fixtures cover Chat, Work, logged out and changed layout; Work sends nothing.
   Live answer markup remains unavailable. Existing synthetic fixtures are unchanged. Their Work rejection test passed locally, but does not satisfy the live-fixture requirement.
3. [x] Conformance runner and `npm test` pass; no cookie, storage or credential access.
   This session passed all 18 Node tests, including the source-access check, and the shared Rust conformance test. The live launcher attempted no cookie, storage, credential, private API or network interception access. No account content was captured.
4. [ ] `upstream_model` follows DEC-004 and docs/translations agree.
   Not implemented. The optional exact visible effort-label behavior still needs the driver, adapter, validation and documentation changes. Current model-label semantics remain in place. The new Chrome launch option is documented in the helper, protocol, provider guide, root README and site provider page, including their maintained translations. Generated wiki files were not edited.
5. [ ] Real Chrome opens the separate profile; owner signs in; headed health is reported.
   `--chrome` now selects Playwright `channel: 'chrome'`. Default flags and headed operation are preserved. The launch used `~/.local/share/shunt/chatgptweb-browser/chrome/cemfi`, but failed before a sign-in window opened. Installed Chrome version: `156.0.8078.48`. This is partial implementation, not acceptance.
6. [ ] Repeat live health with `--headless` and report the result.
   Not run in this session. The required headed step could not start. Missing browser access blocks the requested sequence before further live work.

## Live launch evidence

Date: 2026-10-27. DEC-006 option 2 remains accepted. Its one additional guarded prompt was **not used in this session**. No text was inserted and no physical Send occurred. No Chat/Work control, login flow or browser challenge was automated.

The read-only temporary launcher used the same installed Playwright and launch options as the helper's Chrome path:

```js
await chromium.launchPersistentContext(profile, {
  channel: 'chrome', headless: false,
  permissions: ['clipboard-read', 'clipboard-write'],
});
```

`profile` was the absolute expanded `~/.local/share/shunt/chatgptweb-browser/chrome/cemfi` path. No custom browser flags, stealth settings or challenge bypass were used. The intended next step was to open ChatGPT and allow up to 15 minutes for the owner's hand sign-in. That wait could not begin because launch never returned.

Actual launch output, with the default flag list omitted:

```text
browserType.launchPersistentContext: Timeout 180000ms exceeded.
  - <launching> /Applications/Google Chrome.app/Contents/MacOS/Google Chrome ...
  - <launched> pid=42940
  - [pid=42940][err] Trying to load the allocator multiple times. This is *not* supported.
```

The launcher exited 1. During launch, `orca computer list-windows --app pid:42940 --json` returned `windows: []`. After the timeout, the process was absent. Therefore there is no headed health JSON, live mode selection, completed answer DOM, or headless result from this session. This is a Chrome startup failure; the task did not reach the website's browser check.

## Verification

Verified source revision: `41e56d7e` (source and docs commit). The following commands ran on the same source before that commit; later edits only update this outcome.

`npm test --prefix helpers/chatgptweb-browser` — exit 0:

```text
ℹ tests 18
ℹ pass 18
ℹ fail 0
ℹ cancelled 0
ℹ skipped 0
chatgptweb_browser_uses_shared_fake_conformance_with_local_page ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.48s
```

`cargo fmt --all --check` — exit 0, no output.

`cargo clippy --all-targets -- -D warnings` — exit 0:

```text
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.02s
```

`cargo test` — exit 101:

```text
gateway::spend::persist::tests::malformed_typed_limit_is_hidden_without_losing_valid_limits
panicked at src/gateway/spend/persist/tests.rs:233:9
(empty captured warning log)
test result: FAILED. 3017 passed; 1 failed; 3 ignored
```

The failure matches existing [issue #60](https://github.com/jesusvillota/shunt/issues/60). Duplicate searches and issue inspection confirmed the same parallel log-capture assertion. The exact isolated rerun passed:

```text
cargo test --lib gateway::spend::persist::tests::malformed_typed_limit_is_hidden_without_losing_valid_limits -- --exact
test result: ok. 1 passed; 0 failed; 0 ignored; 3020 filtered out
```

No spend-limit code or tests were changed. The full test suite did not pass or reach later test binaries. `node --check helpers/chatgptweb-browser/cli.mjs` and `git diff --check` passed. GitHub Actions were not inspected. Site pages were not built.

## Summary

What: added `--chrome` and documented its separate profile and direct-launch use. How: two production lines select Playwright's installed Chrome channel while keeping default flags. Deviation: Chrome startup prevented manual sign-in and all subsequent live steps; no selector, fixture or `upstream_model` fix was attempted without the required live evidence.

The temporary launcher is outside the repository. No secrets or profile files are committed. No task spec, accepted decision, other task or goal metadata was edited. PR #58 remains an open draft with `Goal-Task: chatgptweb-provider/TASK-012` and `Goal-Blocked: installed Google Chrome launch times out before manual sign-in`. No review threads were present in the session's GitHub query.

Required next step: restore normal installed-Chrome startup for this separate profile, then resume TASK-012 with Playwright's default flags. Manual sign-in, successful browser-check observation and headed health must precede the DEC-006 prompt. Read the answer DOM; do not use Copy output. The accepted decision is resolved; this blocker requires working browser access, not another send authorization. Do not mark the task complete or merge this draft.

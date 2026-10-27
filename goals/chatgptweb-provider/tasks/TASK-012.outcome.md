# TASK-012 outcome

Status: **blocked**. Live answer markup remains unavailable after the single additional prompt allowed by DEC-005. The 2026-10-27 probe reached the composer with Chat selected and no browser-check error, then clicked Send once under an atomic Chat guard. It observed no assistant turn. Later read-only recovery rendered a blank page. The owner must restore access manually in a normal sign-in. No more automated sends are authorized.

## Acceptance

- [ ] The signed-in `cemfi` driver's health returns `ok`, with Chat selection read from the live page.
  This session read Chat selected (`aria-pressed="true"`) and Work unselected (`aria-pressed="false"`) from the live composer. The unchanged driver's headed health returned `ui_drift`, not success. Exact session output appears below.
- [ ] Sanitized live fixtures cover Chat, Work, logged out and changed layout; Work rejects sends.
  Composer, effort, Send and Stop markup were observed. Assistant, answer-boundary and copy-turn markup could not be captured. No fixture update is committed. Existing synthetic fixtures do not satisfy this requirement.
- [x] The conformance runner and `npm test` pass; no cookie, storage or credential access.
  On 2026-10-27, `npm test --prefix helpers/chatgptweb-browser` passed all 18 Node tests and the shared Rust conformance test. They verify the unchanged implementation, not a delivered fix. All live probes used rendered DOM only. No cookie, storage, credential, network interception or private API access occurred.
- [ ] `upstream_model` follows DEC-004; docs and translations agree.
  Not delivered. DEC-004 still defines it as an optional exact visible effort label, for example `High`. The incomplete prototype and its partial Rust changes were removed. Existing behavior and docs remain unchanged.
- [x] Repeat live health with `--headless` and report the result.
  This session ran the actual helper with `--headless` on `cemfi`. It returned `ui_drift`, with `submission: not_sent`, and exited 0. This error does not establish whether a corrected driver would work headlessly.

## Resume evidence, 2026-10-27

Source revision: `cd5e9e4d`. DEC-005 option 1 was accepted in the owner's resume instruction. Its earlier request is resolved; the access problem and missing completed-turn data remain.

1. A headed read-only probe reached the rendered composer. Its result included:

   ```json
   {"browser_check_failed":false,"account_load_failed":false,"login_visible":false,"challenge_visible":false,"composers":[{"tag":"DIV","role":"textbox","label":"Ask ChatGPT","contenteditable":"true"}],"mode_groups":[{"role":"group","label":"Composer mode","buttons":[{"label":"Chat","pressed":"true"},{"label":"Work","pressed":"false"}]}]}
   ```

2. A second headed probe waited for enabled profile and mode controls. It verified an empty composer and no browser-check or login indicator before inserting `Reply with exactly: SHUNT_TASK012_OK`. Immediately before the single physical Send, the same JavaScript task checked the error page, login/challenge indicators, exactly two enabled mode buttons (Chat true, Work false), exact inserted prompt, unchanged effort control, and one enabled Send button. It then called `send.click()` once. The result was `{"submission":"physical_send"}`. No mode switch was clicked and no retry followed.
3. The probe waited up to 60 seconds for the known assistant marker or browser-check failure. Its final observation was:

   ```json
   {"browser_check_failed":false,"assistant_count":0,"answers":[]}
   ```

   This does not prove server acceptance or that no answer exists under different markup. The page was closed without capturing a conversation URL. Assistant, answer-boundary and copy-turn selectors remain unverified; no live fixture or partial driver fix is committed.
4. Read-only recovery found no matching recent-chat link. A further read-only probe rendered no body text or conversation links:

   ```json
   {"browser_check_failed":false,"chat_links":0,"marker_visible":false,"login_button":false,"challenge_frame":false,"body_text_length":0,"error_phrases":[]}
   ```

   No challenge was automated or bypassed. A blank page is an access failure, not successful readiness.
5. The actual unchanged CLI received a JSONL `health` request on the signed-in `cemfi` profile, first headed, then with `--headless`. Neither request inserted or sent text. Both helper processes exited 0 after EOF:

   ```json
   {"version":1,"id":"task012-resume-headed-health","type":"error","error":{"code":"ui_drift","message":"Browser driver: ui_drift","submission":"not_sent"}}
   {"version":1,"id":"task012-resume-headless-health","type":"error","error":{"code":"ui_drift","message":"Browser driver: ui_drift","submission":"not_sent"}}
   ```

`npm test --prefix helpers/chatgptweb-browser` exited 0 in this session:

```text
ℹ tests 18
ℹ pass 18
ℹ fail 0
ℹ cancelled 0
ℹ skipped 0
test chatgptweb_browser_uses_shared_fake_conformance_with_local_page ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.65s
```

`cargo fmt --all --check` exited 0 with no output. `cargo clippy --all-targets -- -D warnings` exited 0 (`Finished dev profile` in 0.95s).

`cargo test` exited 101 on the unchanged source. Its `passthrough` test binary reported 27 passed and one failed:

```text
thread 'message_start_seeds_tiktoken_estimate_for_streaming_responses_model' panicked at tests/passthrough.rs:1023:5:
expected a nonzero tiktoken estimate in message_start
message_start usage: input_tokens=0, output_tokens=0
message_delta usage: input_tokens=11, output_tokens=2
```

This existing intermittent failure is already reported in [issue #56](https://github.com/jesusvillota/shunt/issues/56), which explicitly includes this test and the same token values. Duplicate search and issue inspection confirmed the match; no duplicate issue was created. An isolated rerun, `cargo test --test passthrough message_start_seeds_tiktoken_estimate_for_streaming_responses_model -- --exact`, also exited 101 with the same assertion (zero passed, one failed). No out-of-scope code or test was changed. This full-suite run did not pass or reach later test binaries.

The earlier evidence below is historical and does not replace this session's results.

## Live evidence

Date: 2026-10-24. Headed Playwright Chromium used the existing `cemfi` profile at `~/.local/share/shunt/chatgptweb-browser/cemfi`. No mode switch was clicked. No account menu or effort menu was opened.

Reduced, sanitized rendered controls:

```html
<div role="group" aria-label="Composer mode">
  <button type="button" aria-pressed="true">Chat</button>
  <button type="button" aria-pressed="false"><span>Work</span></button>
</div>
<div class="ProseMirror" contenteditable="true" role="textbox"
     aria-label="Ask ChatGPT" aria-multiline="true">
  <p data-empty-paragraph="true"><br class="ProseMirror-trailingBreak"></p>
</div>
<button aria-label="Open profile menu"></button>
<button aria-label="Select ChatGPT model" data-selected-reasoning-effort="high">
  <span aria-hidden="true">Thinking effort</span>
  <span aria-hidden="false">High</span>
</button>
<button type="submit" aria-label="Send"></button>
<button type="button" aria-label="Stop"></button>
```

The empty, hydrated composer exposes `Start Voice`, not Send. The initial loading placeholder can expose disabled mode and profile controls. A fixed three-second wait is insufficient for this live page. The effort control includes a hidden `Thinking effort` label and a visible `High` label. Its full `textContent` is not the effort value.

The one approved prompt was `Reply with exactly: SHUNT_TASK012_OK`.

1. Read the rendered group and require exactly two buttons: Chat true, Work false.
2. Insert the short prompt into the visible composer.
3. The initial click guard returned `ui_drift` because Send was still disabled. Nothing was sent then.
4. After Send became enabled, repeat the mode and exact prompt checks in the same page JavaScript task as `send.click()`.
5. The guard returned `sent`. A rendered button with `aria-label="Stop"` appeared. No `data-testid` was present on Send or Stop.
6. The page then displayed `We couldn’t load your account` and `The browser check couldn’t finish. Reload this page to try again.` No assistant message appeared. The URL remained `https://chatgpt.com/`.
7. A read-only reload restored the composer. No second prompt was sent. The available probe did not recover an answer to the approved prompt.

`sent` above records the physical click, not server acceptance. There was no assistant answer or conversation URL to establish acceptance. The failure page displayed `ChatGPT Work` in its header after the click; this was not used as mode evidence. The immediately preceding guard read the actual composer buttons as Chat true and Work false. The task never selected Work.

## Verification

All four requested local commands passed on the unchanged code. The first npm run timed out in `CLI eof cancels an active generation` (17 passed, one cancelled). After closing the live probe processes, the complete npm command passed on repeat. No test assertions were changed.

`npm test --prefix helpers/chatgptweb-browser` — exit 0:

```text
✔ CLI eof cancels an active generation (1377.7085ms)
✔ CLI SIGTERM cancels an active generation (1240.695334ms)
ℹ tests 18
ℹ suites 0
ℹ pass 18
ℹ fail 0
ℹ cancelled 0
ℹ skipped 0
ℹ todo 0
ℹ duration_ms 20012.535084
    Blocking waiting for file lock on build directory
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 11s
     Running tests/chatgptweb_browser.rs (/Users/jesusvillotamiranda/orca/workspaces/shunt/goal-chatgptweb-provider-task-012/target/debug/deps/chatgptweb_browser-7d77f011455832e5)

running 1 test
test chatgptweb_browser_uses_shared_fake_conformance_with_local_page ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.78s

```

`cargo fmt --all --check` — exit 0, no output.

`cargo clippy --all-targets -- -D warnings` — exit 0:

```text
    Checking opentelemetry-otlp v0.32.0
    Checking shunt-gateway v0.51.1 (/Users/jesusvillotamiranda/orca/workspaces/shunt/goal-chatgptweb-provider-task-012)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 00s
```

`cargo test` — exit 0 (tail):

```text
test compaction_and_auxiliary_stay_on_the_parent ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

     Running tests/tui_client.rs (target/debug/deps/tui_client-c5558d30fc617895)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests shunt

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

```

Final live health on the unchanged driver:

```json
{"version":1,"id":"task012-final-health","type":"error","error":{"code":"ui_drift","message":"Browser driver: ui_drift","submission":"not_sent"}}
```

This is a failed health result, not `ok`. The helper's EOF shutdown also exceeded the probe wrapper's 15-second wait; the wrapper exited 1 after reading this JSON. The read-only health request did not type or send anything. These local successes do not satisfy the missing live fixtures or DEC-004 implementation.


## Decision request

Question: How should TASK-012 obtain completed live answer markup after the DEC-005 prompt produced no observable assistant turn?

1. Restore `cemfi` access manually in a normal sign-in and supply sanitized assistant, answer-boundary and copy-turn markup from a completed Chat turn. This avoids another automated send but requires owner action and captured data. Live driver health must still be verified after the fix.
2. After manually restoring access, explicitly authorize another single short guarded Chat prompt. This gives a fresh capture opportunity but consumes another web turn and may fail again. DEC-005's additional prompt has already been used; this option is not currently authorized.
3. Keep the task blocked. This uses no additional turns but leaves TASK-006 blocked on the driver fix.

### Recommendation

Restore access manually in a normal sign-in and supply sanitized completed-turn markup first (option 1). Do not authorize a further automated send until the profile can complete a normal Chat response. Never automate or bypass a challenge.

## Summary

What: resumed under DEC-005, verified the rendered Chat selection, used its single additional guarded Send, and ran actual headed and headless health checks. How: rendered DOM through Playwright, without cookie, storage or credential access. Deviation: no assistant markup was observed and read-only recovery rendered a blank page. This PR records only the blocker and sanitized evidence; README, docs, site translations and generated wiki were each considered and need no edits for this outcome-only change. The required driver and DEC-004 documentation changes remain undelivered.

Verified source revision: `cd5e9e4d`. Source and tests remain unchanged. Follow-up: restore live browser access manually and resolve the new data request above before resuming TASK-012. No task specs, decisions, other outcomes or goal metadata were changed. No open PR review threads were present when checked in this session. Do not mark this task complete or merge this draft.

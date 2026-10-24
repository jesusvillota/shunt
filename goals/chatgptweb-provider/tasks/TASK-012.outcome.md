# TASK-012 outcome

Status: **blocked**. The only approved live prompt reached Send after an atomic Chat check. The page then failed its browser check. No assistant answer or copy-turn markup was available. Another prompt needs renewed permission.

## Acceptance

- [ ] The signed-in `cemfi` driver's health returns `ok`, with Chat selection read from the live page.
  The rendered mode group selected Chat (`aria-pressed="true"`) and deselected Work (`aria-pressed="false"`). The health experiment did not return success. The unchanged driver's final health output is recorded below.
- [ ] Sanitized live fixtures cover Chat, Work, logged out and changed layout; Work rejects sends.
  Composer, effort, Send and Stop markup were observed. Assistant, answer-boundary and copy-turn markup could not be captured. No fixture update is committed. Existing synthetic fixtures do not satisfy this requirement.
- [x] The conformance runner and `npm test` pass; no cookie, storage or credential access.
  Local checks are recorded below. They verify the unchanged implementation, not a delivered fix. All live probes used rendered DOM only. No cookie, storage, credential, network interception or private API access occurred.
- [ ] `upstream_model` follows DEC-004; docs and translations agree.
  Not delivered. DEC-004 still defines it as an optional exact visible effort label, for example `High`. The incomplete prototype and its partial Rust changes were removed. Existing behavior and docs remain unchanged.

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

Question: May TASK-012 send one additional short Chat-mode prompt after the `cemfi` browser check succeeds?

1. Permit one more short prompt, with the same atomic Chat check. This allows capture of live answer and completion markup. It uses one additional web turn and may encounter the same browser check failure.
2. Supply a sanitized, completed live answer fixture instead. This avoids another live send, but needs externally captured data and does not prove this profile can complete a turn.
3. Keep the task blocked. This uses no additional turns but leaves TASK-006 blocked on the driver fix.

### Recommendation

Permit one additional short prompt after the profile can finish its browser check. Keep the atomic Chat-only guard and prohibit retries after a physical Send. If the browser check still fails, restore access manually before resuming. Do not automate the challenge.

## Summary

What: inspected the real composer and used the single approved prompt to probe Send, Stop and completion. How: rendered DOM through Playwright, without cookie, storage or credential access. Deviation: the browser check failed after the guarded Send, so required answer markup was unavailable. All incomplete production and test edits were removed. This PR records only the blocker and sanitized evidence; README, docs, site translations and generated wiki need no edits for this outcome-only change.

Verified revision: `f7c70f9a8db2c4bb7d933a415ca8fc02f952fb03`. The rebase onto `origin/main` changed only goal documents; source and tests match the initial code revision `2738cf0933c1d97dc893680faf172d49871c4b98`. Follow-up: restore live browser access, authorize one additional short prompt, then resume TASK-012. No task specs, decisions, other outcomes or goal metadata were changed. Do not mark this task complete or merge this draft.

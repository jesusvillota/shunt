# TASK-012 outcome

Status: ready for review. The driver reads the live signed-in page and its completed answer. The owner restored access manually. DEC-006's one guarded prompt completed with `OK`; no retry occurred. Chrome remains open.

## Acceptance

1. [x] Signed-in `cemfi` health succeeds and reads live Chat selection.
   The final CLI ran with `--chrome`, the separate `~/.local/share/shunt/chatgptweb-browser/chrome/cemfi` binding, and the default loopback CDP endpoint. Exact stdout:

   ```json
   {"version":1,"id":"task012-cdp-headed-final","type":"health"}
   ```

   Read-only live mode output:

   ```json
   [{"text":"Chat","pressed":"true"},{"text":"Work","pressed":"false"}]
   ```

   The live guard also returned `{"model":"Medium","prompt":"","disabled":true}`. An empty live composer shows Start Voice instead of Send. The driver waits for profile/effort controls to finish rendering.
2. [x] Sanitized live structures cover Chat, Work, logged out and layout drift; Work sends nothing.
   `test/fixture.mjs` projects the observed mode group, pressed buttons, ProseMirror textbox, visible effort span, profile control, submit button, user bubble, assistant renderer, turn wrapper, completed state and Copy/Stop controls. Identifiers and text are synthetic. Work and drift are local mutations of this structure, not live mode switches. Work, missing/unknown/conflicting states, malformed Work state, duplicate Chat labels and a mode change during paste all reject with `not_chat_mode` and zero fixture sends. The logged-out fixture uses the live `data-login-button` attributes. No account name, old chats, secrets or profile files are committed.
3. [x] Browser tests and the shared conformance runner pass; no secret extraction.
   Final `npm test --prefix helpers/chatgptweb-browser` exited 0: 29 Node tests passed, zero failed; shared Rust conformance passed 1/1. The unchanged source-access check rejects cookie, storage, credential and network extraction APIs. Live work read rendered DOM only. No login, browser challenge, Work selection, private API or account-secret access was automated.
4. [x] Optional `upstream_model` means the exact visible effort label (DEC-004).
   Rust preserves omission as `None`; the browser receives no effort constraint in that case. A configured label must match exactly before sending and in the atomic guard. Node tests cover omitted, matching, mismatched and empty labels. Rust tests cover the optional wire field, blank-field rejection, default-provider omission and a routed omission with a public name unrelated to effort. Existing config validation accepts omission and rejects a blank configured value. The config field comment, example, protocol, helper/provider docs, root README and provider site page agree; maintained README/site translations are updated. No public config key was added.
5. [x] Real Chrome access, manual sign-in and headed health are reported.
   As explicitly requested for this resume, `--chrome` now uses `chromium.connectOverCDP(url)` instead of launching Chrome. The endpoint defaults to `http://127.0.0.1:9333`; overrides accept HTTP numeric loopback only, without authentication. The helper adds no browser flags. Shutdown closes only its request tab, including when tab creation is still pending. EOF/SIGTERM tests prove the owner's page remains usable. Final live inspection reported `OWNER_BROWSER_ALIVE true` after CLI shutdown.
6. [x] Headless checks run where possible; the live limitation is reported.
   All browser fixtures and conformance ran in headless Chromium. Live signed-in headless health was not run: the supplied Chrome was already headed, and CDP cannot change its display mode. Restarting it would conflict with the instruction to leave it open; copying its sign-in state would conflict with the no-extraction boundary. `--chrome --headless` is rejected explicitly and tested. This follows the resume instruction's “headless check where possible” qualification, rather than claiming a second live display-mode result.

## Live sequence and single-send evidence

Date: 2026-10-27. The read-only manual-sign-in wait began at 22:37:35 UTC, with a 22:52:35 deadline. It observed logged-out controls until the signed-in profile and Composer mode controls appeared at 22:40:20. No challenge iframe was observed during the successful sequence. Initial health reported `logged_out`, `submission: not_sent`.

After successful headed health, the helper's production guard read Chat=true, Work=false and effort `Medium`. One fresh task tab staged `Reply with exactly OK.` using Playwright's textbox fill. The production `inspect` function checked mode, effort, exact prompt and enabled Send, then clicked Send in one JavaScript task. A temporary exclusive-create marker prevented another attempt. There was exactly one physical Send; no retries or further prompts followed.

The page showed Stop during generation, then Copy and `data-talvt-turn-state="complete"`. Production `ChatPage.answer()` read accepted=true, text=`OK`, invalid=false, done=true, with a stable turn identity. Completion came from rendered DOM, not Copy output. The full clipboard/streaming pipeline is covered by headless fixture tests; no second live generation was used to retest it after learning the markup.

## Verification

Verified source revision: `8c1f05ea`. Subsequent changes only record this outcome.

- `npm test --prefix helpers/chatgptweb-browser`: exit 0; 29 passed, 0 failed; shared Rust conformance 1 passed, 0 failed.
- `cargo fmt --all --check`: exit 0, no output.
- `cargo clippy --all-targets -- -D warnings`: exit 0; `Finished dev profile`.
- `cargo test`: exit 0; 3,729 passed, 0 failed, 4 ignored across 51 result summaries, including the new routed optional-effort test.
- `node --check` for CLI/page and `git diff --check`: passed.
- Independent code review found shutdown-during-tab-creation and malformed-mode gaps. Both were fixed with tests; the reviewer found no remaining important issues. GitHub review-thread query returned an empty list.

GitHub Actions were not inspected. Site pages were not built. The live signed-in headless limitation is stated above. English UI labels remain necessary for some controls; unfamiliar markup fails closed. TASK-006 still owns the quota check.

## Summary

What: attach to the owner's Chrome, read the current Chat-only composer and completed-turn DOM, and carry optional effort checks through the adapter and protocol. How: reuse Playwright CDP, the existing atomic guard, the existing driver protocol and local conformance runner. All affected documentation surfaces were considered; README, docs and translated site pages changed, while generated wiki files were left to their separate workflow.

Deviation from launch wording: the owner's later instruction replaces Playwright Chrome launch with attachment to the already running browser. No automation flags were added. The owner completed normal sign-in manually. The attached headed browser was preserved, so a live headless restart was unavailable. Work fixtures are local mutations because switching live mode is prohibited. No task spec, other task, decision or goal metadata was edited.

PR #58 carries `Goal-Task: chatgptweb-provider/TASK-012` and `goal:chatgptweb-provider`. It is intended for open, non-draft review and must not be merged by this worker. No decision or access blocker remains. Proposed follow-up: resume TASK-006's separate live quota gate.

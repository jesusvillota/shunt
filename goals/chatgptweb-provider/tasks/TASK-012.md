---
id: TASK-012
title: Fix the browser driver for the live ChatGPT page
milestone: M-1
state: approved
depends_on: [DEC-005, DEC-006]
delivery: pr
---

## Why

TASK-006's live health call returned `ui_drift`: the real ChatGPT page differs from the TASK-003 fixtures. DEC-004 chose a separate driver-fix task. Serves SC-1.

## Scope

Fix `helpers/chatgptweb-browser` (`page.mjs`, `driver.mjs`, fixtures, tests) and the shunt adapter for the live page. Facts from TASK-006's probe (`tasks/TASK-006.outcome.md`, branch `goal/chatgptweb-provider/task-006`): the mode switch is `<div role="group" aria-label="Composer mode">` with two `<button aria-pressed>` labelled Chat and Work; the composer is a ProseMirror `contenteditable` `role="textbox"` with `aria-label="Ask ChatGPT"` (no `#prompt-textarea`); the model control is `button[aria-label="Select ChatGPT model"]` showing `Thinking effort High`; the account control is `aria-label="Open profile menu"`.

Learn the send, stop, copy-turn and answer markup from the live page. The owner has approved ONE short Chat-mode test prompt on the `cemfi` profile (`~/.local/share/shunt/chatgptweb-browser/cemfi`), sent by hand or by the driver only after the Chat check passes. Never switch mode, never use Work mode. Prefer selectors that do not depend on the page language (roles, `aria-pressed`, structure) over label text; keep Chat mode fail-closed.

DEC-004 decision on `upstream_model`: the page shows no model name. Drop the model-name check. `upstream_model` is optional and, when set, means the visible effort label (for example `High`); the driver refuses to send if it does not match. Update the protocol doc, config validation, `shunt.toml.example`, docs and the README/site pages (with their translations) that mention `upstream_model`. Record sanitized fixtures of the live markup (no account name, chats or secrets). Out of scope: tool calls, rotation, the quota gate (TASK-006).

## Acceptance

- The driver's `health` call on the signed-in `cemfi` profile returns `ok`, with the Chat selection read from the live page. Paste the output.
- Fixtures built from the live markup cover Chat, Work, logged out and a changed layout; Work mode returns `not_chat_mode` with nothing sent.
- The conformance runner and `npm test` pass; no cookie, storage or credential access.
- `upstream_model` behaves as decided, and docs and translations agree.
- Real Chrome: a `--chrome` option launches installed Google Chrome (Playwright `channel: 'chrome'`) with profile `~/.local/share/shunt/chatgptweb-browser/chrome/cemfi`. Keep Playwright's default flags; no stealth, no challenge bypass. The owner signs in once by hand; then run the headed live `health` call and report it. If Chrome fails the browser check, stop and report.
- Headless: rerun the live `health` call with `--headless` and report the result only. Default stays headed.

## Verify

`npm test --prefix helpers/chatgptweb-browser`, `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`. Paste the outputs and the live health result in the PR.

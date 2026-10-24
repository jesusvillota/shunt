# TASK-006 outcome

Status: **blocked**. The live driver cannot run a turn on the real ChatGPT page. No prompt was sent.

## Acceptance

- [ ] PR body has `goal pool` before and after, and Codex quota did not change.
  Not met: no turn ran. The `codex` section was recorded anyway (see below).
  `personal` 5h moved 0.07 -> 0.08 with no turn from this task. Other agents share that account. So a quota claim needs a quiet window.
- [ ] A transcript shows Codex got an answer from the ChatGPT Web model through shunt. Not met: no turn ran.
- [ ] A live negative check shows `not_chat_mode` or `logged_out` and nothing sent. Not met. The only live call was a `health` request on profile `cemfi` (nothing typed, nothing sent). It returned `ui_drift`, `submission: not_sent`.

## Evidence

Setup: `cargo build`, `npm ci` and `npx playwright install chromium` in `helpers/chatgptweb-browser`. `shunt check --config /tmp/t006/shunt.toml` printed `config ok` (provider `chatgptweb_browser`, account `cemfi`, profile `~/.local/share/shunt/chatgptweb-browser/cemfi`).

Live health call (profile `cemfi`, signed in, Chat selected):

```text
{"version":1,"id":"health-1","type":"error","error":{"code":"ui_drift","message":"Browser driver: ui_drift","submission":"not_sent"}}
```

Why: `page.mjs` `open()` waits for `#prompt-textarea`, a login button or a challenge frame. The live page has none of them. A probe of the live page (headed Chromium, same profile) found:

- Mode switch: `<div role="group" aria-label="Composer mode">` with two `<button aria-pressed="true|false">` labelled Chat and Work. The driver expects `role=tab|radio` with `aria-selected|aria-checked`.
- Composer: `<div class="ProseMirror" contenteditable="true" role="textbox" aria-label="Ask ChatGPT">`. No `#prompt-textarea`.
- Model control: `button[aria-label="Select ChatGPT model"]`, text `Thinking effort High`. No `data-testid="model-switcher-dropdown-button"` and no model name such as `GPT-5.6`.
- Account control: `aria-label="Open profile menu"`, no `accounts-profile-button` test id.
- Send, stop, copy-turn buttons, and the answer markup were not seen (they appear only after typing or sending).
- The page showed "Jesus Villota (cemfi)", Plus. Chat was selected.

Codex pool, before (2026-10-23T20:38Z) and after (2026-10-23T21:07Z):

```text
before:
codex (chatgpt_oauth)
  personal       ok                     5h 0.07 (reset 1792795321) · 7d 0.01 (reset 1793382121) · cooldown —
  cemfi          ok                     5h 0.12 (reset 1792795390) · 7d 0.02 (reset 1793382190) · cooldown —
after:
codex (chatgpt_oauth)
  personal       ok                     5h 0.08 (reset 1792795321) · 7d 0.01 (reset 1793382121) · cooldown —
  cemfi          ok                     5h 0.12 (reset 1792795390) · 7d 0.02 (reset 1793382190) · cooldown —
```

## Decision request

Question: How should TASK-006 proceed, given the real page differs from the TASK-003 fixtures?

Options:

1. Fix the driver in this PR. Update `page.mjs` selectors and the fixtures to the live markup. Cost: edits TASK-003's delivered code and tests. It must learn send/stop/answer markup by sending one real Chat-mode prompt. Risk: this page changes often.
2. New task for the driver fix, then re-run TASK-006. Cost: one more PR and a delay. Keeps this gate clean.
3. Capture the live DOM as a fixture first (a new task), then fix.

Open point inside every option: the page shows no model name, only "Thinking effort High". `upstream_model = "GPT-5.6"` cannot match it. Choose what `upstream_model` means (the effort label, the model switcher menu entry, or no model check).

### Recommendation

Option 2, with the model-label question answered first. Option 1 changes shipped code under a gate whose job is to verify, and the model-label meaning is a product choice.

## Summary

What: set up shunt and the helper, ran one live health call, probed the live DOM, recorded the quota. How: separate shunt config at `/tmp/t006/shunt.toml` on port 3055 (not committed; the running gateway on 3001 was untouched). Deviation: no live turn, no negative check. Verified revision: the base commit `e1967d47`; this PR adds only this outcome. Follow-ups: driver fix task; a quiet-window quota check (other agents use the `codex` accounts).

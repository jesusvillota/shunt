# ChatGPT Web browser helper

This development helper controls one browser profile for one account. It uses
the [version 1 JSON Lines protocol](../../docs/chatgptweb-driver-protocol.md).
It does not register a shunt provider. TASK-004 adds that adapter.

## Start one account

1. Install Node.js 22 or later. From this directory, run `npm ci`.
2. Install the browser with `npx playwright install chromium`.
3. Start the helper with an absolute, normalized profile path:

   ```sh
   node cli.mjs --account personal \
     --profile-dir "$HOME/.local/share/shunt/chatgptweb-browser/personal"
   ```

The browser manages its profile at the exact `--profile-dir` path. Use a
separate directory for each account. Do not share a running browser profile.
Login is an owner step in TASK-005. This helper does not automate login,
challenges, or approvals. It does not import or extract account secrets.

Keep stdin open. Send one protocol request per line and drain its events
before the next request. Stdout contains protocol events only. Closing stdin
or sending SIGTERM closes the browser, including an active turn. The launcher
must still enforce the protocol deadline and terminate the process group if
the helper becomes unresponsive.

Generate opens a fresh page at `https://chatgpt.com/` for every turn. Health
also opens a page but does not insert or send text. The helper accepts only
the exact currently selected model label as `model`; it never substitutes or
changes the model. It pastes through the clipboard and verifies the full text.
It checks selected Chat mode again in the same page task as the Send click.

## Supported page evidence

The helper requires visible Chat and Work choices with explicit `aria-selected`
or `aria-checked` state. Exactly one must select Chat. Unknown or conflicting
state returns `not_chat_mode`. Missing account, model, composer, or Send
controls returns `ui_drift`. Login/challenge indicators return `logged_out`.
It never switches mode or dismisses an approval.

Answer extraction uses one assistant message in one `data-turn-id` wrapper.
Text must only grow. Changed text or answer identity returns `ui_drift`.
Completion requires the turn's copy action and absence of the stop control.
It does not use a translated button label or an inactivity delay.

The fixtures imitate these page structures. They are not captured production
markup. TASK-006 must validate the selected-state selectors and completion
evidence on the live page before operational use. A different layout fails
closed. No live account or quota claim is made by this helper's tests.

## Local verification

1. Run `npm ci` and `npx playwright install chromium` in this directory.
2. Run `npm test`.

The command runs real Chromium against local HTTP fixtures. It then runs
`tests/chatgptweb_browser.rs` through TASK-002's unchanged Rust conformance
runner. The Rust browser test is ignored by default so ordinary shunt tests
need no Node installation. `npm test` explicitly enables it.

Tests cover Chat, Work, logged out, drift, unknown and conflicting mode,
mode changes during paste, Unicode and whitespace, streaming, empty answers,
timeouts, uncertain sends, validation, account isolation, and pipe framing.
A source-access check rejects cookie, storage, credential, and network
extraction APIs in the production helper files.

`--fixture-url http://127.0.0.1:PORT/` and `--headless` support local tests.
The fixture override rejects non-loopback URLs. Default operation is headed
and uses ChatGPT directly. No login state is needed for fixture tests.

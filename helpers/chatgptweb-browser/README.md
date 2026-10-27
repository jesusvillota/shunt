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
Use `--chrome` to attach to an already running Chrome through the Chrome
DevTools Protocol (CDP). Start Chrome yourself with a separate profile:

```sh
'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome' \
  --user-data-dir="$HOME/.local/share/shunt/chatgptweb-browser/chrome/cemfi" \
  --remote-debugging-address=127.0.0.1 --remote-debugging-port=9333
node cli.mjs --chrome --account cemfi \
  --profile-dir "$HOME/.local/share/shunt/chatgptweb-browser/chrome/cemfi"
```

The default CDP endpoint is `http://127.0.0.1:9333`. Override it with
`--cdp-url http://127.0.0.1:PORT` (or HTTP IPv6 loopback). Remote hosts and
authentication in the URL are rejected. The helper adds no browser flags and
does not open or close the attached browser. `--profile-dir` identifies the
account binding; the owner must start Chrome with that same profile.

Without `--chrome`, operation is headed. Add `--headless` to launch Chromium
without a visible window. `--chrome --headless` is rejected: attaching cannot
change the running browser’s display mode.
Login is an owner step in TASK-005. This helper does not automate login,
challenges, or approvals. It does not import or extract account secrets.

Keep stdin open. Send one protocol request per line and drain its events
before the next request. Stdout contains protocol events only. Closing stdin
or sending SIGTERM closes the launched Chromium, including an active turn.
With `--chrome`, it closes only the helper’s request tab and disconnects; the
owner’s browser and tabs remain open. The launcher
must still enforce the protocol deadline and terminate the process group if
the helper becomes unresponsive.

Generate opens a fresh page at `https://chatgpt.com/` for every turn. Health
also opens a page but does not insert or send text. The optional `model` field
must match the visible effort label exactly (for example `Medium`). Omission
leaves effort unchecked. The helper never changes the selected effort. It pastes
through the clipboard and verifies the full text.
It checks selected Chat mode again in the same page task as the Send click.

## Supported page evidence

The helper requires the live `Composer mode` group with Chat and Work buttons
and explicit `aria-pressed` state: Chat must be true and Work must be false.
Unknown or conflicting state returns `not_chat_mode`. Missing account, model, composer, or Send
controls returns `ui_drift`. An empty composer can show Start Voice instead of Send. Login/challenge indicators return `logged_out`.
It never switches mode or dismisses an approval.

Answer extraction uses one `data-markdown-text-style="assistant-message"`
inside a `data-turn-key` wrapper, with a `data-dil-message-id` renderer.
Text must only grow. Changed text or answer identity returns `ui_drift`.
Completion requires `data-talvt-turn-state="complete"`, the turn’s Copy action,
and absence of Stop. Completion uses the observed English Copy and Stop labels.
It never clicks Copy or uses an inactivity delay.

Fixtures retain sanitized structural attributes observed on the live page.
Chat and completion came from one authorized live turn; Work and drift are
local mutations. The logged-out fixture uses the observed login button.
TASK-006 still owns the live quota check. A different layout fails
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

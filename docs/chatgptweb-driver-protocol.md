# ChatGPT Web driver protocol, version 1

TASK-002 implements this contract and an offline fake. DEC-002 selects a
Node/Bun helper per account and the provider name `chatgptweb_browser`.
The real browser helper is in `helpers/chatgptweb-browser` (TASK-003).
See its README for installation, per-account profile paths, and fixture tests.
Provider registration and
Responses translation belong to TASK-004. This document adds no config keys.

## Transport and ownership

Shunt starts one helper for one account and one absolute browser profile path.
The launcher binds that pair for the helper's lifetime. Each request repeats
the pair; it cannot switch the helper's account or profile. Profile paths must
already be expanded and normalized by the launcher. Compare the bound strings
exactly. Never read or write credentials through this protocol.

Use private stdin/stdout pipes, with UTF-8 JSON objects separated by LF
(JSON Lines). Escape newlines inside strings. Flush each event immediately.
Reserve stdout for protocol objects; stderr is for redacted diagnostics.
There is no HTTP listener, bearer token, or unsolicited startup event.
Launch with `node helpers/chatgptweb-browser/cli.mjs --account ACCOUNT
--profile-dir ABSOLUTE_PATH`. The Node helper uses Playwright Chromium and
keeps the browser's persistent profile at that exact path. See the helper README.
Direct helper runs accept `--chrome` to attach to an already running Chrome
at `http://127.0.0.1:9333` through CDP. `--cdp-url` accepts an HTTP loopback
endpoint without authentication. Start Chrome manually with its separate
`~/.local/share/shunt/chatgptweb-browser/chrome/cemfi` profile and the debugging
port. The helper adds no browser flags. Shutdown closes only its request tab,
leaving the owner's browser open. `--headless` applies only to launched
Chromium and cannot be combined with `--chrome`. Shunt does not pass these
flags. See the [helper instructions](../helpers/chatgptweb-browser/README.md).

Shunt serializes requests per helper. Only one request can be active, including
health checks. Every request has a nonblank `id`, unique within that helper's
lifetime. Every event echoes it. The event `version` is always `1`, including
an `unsupported_version` response. Unknown object fields are ignored within
version 1; unknown types, error codes, or submission states are fatal protocol
failures. Incompatible changes require a new version.

The caller drains the current request's stream before reusing the helper.
A terminal event ends that request's Rust stream; it does not close stdout.
Do not wait for process EOF to mark a completed request. Malformed JSON,
missing fields, invalid field types, wrong IDs, unknown events, extra events
after a terminal event, or EOF before a terminal event are transport failures.
Terminate and reap the helper and browser descendants after such a failure.
Do not synthesize `done`. An undecodable request cannot be safely correlated:
the helper reports a redacted diagnostic to stderr and exits without sending.

`timeout_ms` is a positive unsigned 32-bit integer, in milliseconds. Its budget
starts when the helper reads the complete request, and covers all checks,
submission, and generation. At expiry, stop page activity and emit `timeout`.
Shunt also enforces its own deadline, then terminates and reaps an unresponsive
helper and browser descendants. A dropped stream, closed stdin, or shutdown
has the same termination requirement. There is no cancellation request in v1.
After an interrupted or uncertain send, never automatically replay the prompt.

## Requests

Both request types require `version` (unsigned 32-bit integer), `id` (nonblank
string), `account` (nonblank opaque account ID), `profile_dir` (absolute path
string), and `timeout_ms`. Check the version first, validate the common fields,
check the bound account/profile pair, then validate operation fields. Reject
invalid requests before page actions. A well-formed but unsupported version
gets `unsupported_version`. An invalid value gets `invalid_request`.

### Generate

`type` is `generate`. `prompt` is nonblank plain text, preserved exactly,
including whitespace and Unicode. `model` is optional. When present, it is a
nonblank exact visible effort label, such as `Medium` or `High` (DEC-004).
Omission leaves effort unchecked; it does not mean the public route name. A
mismatch gets `invalid_request` before sending. Never change the selected effort.

```json
{"version":1,"id":"turn-1","type":"generate","account":"personal","profile_dir":"/profiles/personal","timeout_ms":120000,"prompt":"Hello\nworld","model":"Medium"}
```

Start a fresh chat for each generate request. Immediately before every physical
Send, verify that the actual selected Chat/Work state is exactly Chat. Work,
missing, conflicting, or unrecognized state gets `not_chat_mode` without Send.
Never infer mode from the model name or switch to Work. Verify login, requested
model, composer, and prompt insertion too. A successful earlier health check
does not replace these checks. Send at most once. No hidden retries.

Return rendered answer text as incremental `text_delta` events, followed by
exactly one `done` or `error`. A successful empty answer is `done` alone.
No Responses events, tool interpretation, local tools, or full-answer buffering
belong in this protocol.

### Health

`type` is `health`; there are no additional fields. Inspect the bound profile's
current login, selected Chat mode, and required page controls without typing
or sending a prompt. Report ready with `health`, or fail with `error`.
This call does not measure quota or establish model entitlement.

```json
{"version":1,"id":"health-1","type":"health","account":"personal","profile_dir":"/profiles/personal","timeout_ms":5000}
```

## Events

All events require `version`, `id`, and `type`. There are four event types.
Only generate permits `text_delta` and `done`; only health permits `health`.
Either request can end with `error`. Exactly one terminal event is required.

### Text delta

`type` is `text_delta`; `text` is a nonempty string appended exactly once to
the current answer. Deltas contain new text only, in page order. Do not resend
the full answer or silently revise already emitted text. If page reconciliation
cannot preserve this rule, return `ui_drift`. Delta boundaries are arbitrary.

```json
{"version":1,"id":"turn-1","type":"text_delta","text":"Hello 🌍\n"}
```

### Done

`type` is `done`; no additional fields. This is terminal success for generate.
Emit it only after positive page completion evidence, never merely inactivity
or timeout. It does not repeat the accumulated text.

```json
{"version":1,"id":"turn-1","type":"done"}
```

### Health

`type` is `health`; no additional fields. This is terminal readiness success.

```json
{"version":1,"id":"health-1","type":"health"}
```

### Error

`type` is `error`. Required `error` fields are `code` (one of the nine codes
below), `message` (nonblank diagnostic string), and `submission` (one of the
three states below). Never include tokens, cookies, profile contents, or prompt
text in diagnostics. Clients branch on `code`, never `message`.

Optional `retry_after_ms` is an unsigned 32-bit delay allowed only with
`rate_limited`. Omit it when the page gives no reliable delay. Zero is allowed.
It is a scheduling hint, not permission to retry an accepted or uncertain send.
Errors are terminal and can follow partial text. Never follow an error with
`done`. A stream with partial text remains failed; it is not a complete answer.

Submission states describe this request's physical Send:

| State | Evidence and consequence |
| --- | --- |
| `not_sent` | Send was never attempted. A later adapter may consider another account, subject to its fail-closed policy. |
| `may_have_been_sent` | Send was attempted but acceptance is unknown. Do not replay automatically, even when no delta arrived. |
| `sent` | The page confirmed acceptance. Do not replay automatically. All errors after a delta must use this state. |

Health errors always use `not_sent`. `not_chat_mode`, `invalid_request`,
`unsupported_version`, and `account_mismatch` always use `not_sent`.
Other codes can use any state supported by actual evidence. Transport failure
after writing a generate request is uncertain unless earlier evidence proves
acceptance. Request IDs correlate events; they are not an idempotency mechanism.

#### `not_chat_mode`

Selected Chat mode cannot be established. Work or unknown state fails closed.

```json
{"version":1,"id":"turn-1","type":"error","error":{"code":"not_chat_mode","message":"Chat mode is not confirmed","submission":"not_sent"}}
```

#### `logged_out`

Login expired or an authentication challenge requires owner action. Do not
refresh OAuth credentials, import cookies, or attempt login through this call.

```json
{"version":1,"id":"health-1","type":"error","error":{"code":"logged_out","message":"Profile needs interactive login","submission":"not_sent"}}
```

#### `ui_drift`

Required page controls, model selection evidence, or answer boundaries changed.
Unknown Chat/Work state specifically uses `not_chat_mode` instead.

```json
{"version":1,"id":"turn-1","type":"error","error":{"code":"ui_drift","message":"Answer boundary changed","submission":"sent"}}
```

#### `rate_limited`

The page explicitly reports a usage or capacity limit. Do not guess a reset.

```json
{"version":1,"id":"turn-1","type":"error","error":{"code":"rate_limited","message":"Page reports a usage limit","submission":"not_sent","retry_after_ms":60000}}
```

#### `timeout`

The request deadline expired, including during checks or after Send.

```json
{"version":1,"id":"turn-1","type":"error","error":{"code":"timeout","message":"Deadline expired after Send attempt","submission":"may_have_been_sent"}}
```

#### `invalid_request`

Decoded request values are invalid, or the requested model is unavailable.
The fake also uses this code when no prompt/model fixture matches.

```json
{"version":1,"id":"turn-1","type":"error","error":{"code":"invalid_request","message":"Model is unavailable","submission":"not_sent"}}
```

#### `unsupported_version`

The decoded version is not 1. This error's event envelope still uses version 1.

```json
{"version":1,"id":"turn-1","type":"error","error":{"code":"unsupported_version","message":"Expected protocol version 1","submission":"not_sent"}}
```

#### `account_mismatch`

The request's account or profile differs from the helper's bound pair.
Never open the requested replacement profile.

```json
{"version":1,"id":"turn-1","type":"error","error":{"code":"account_mismatch","message":"Helper account/profile mismatch","submission":"not_sent"}}
```

#### `internal_error`

An unexpected helper failure prevents completion and no more specific code
applies. A helper crash without a terminal event is a transport failure instead.

```json
{"version":1,"id":"turn-1","type":"error","error":{"code":"internal_error","message":"Browser operation failed","submission":"may_have_been_sent"}}
```

## Rust interface, fake, and conformance

`shunt::chatgptweb::Driver` accepts a typed `DriverRequest` and returns a
`DriverStream` of typed events or `DriverTransportError`. The future process
client implements pipe framing, deadlines, and termination behind this trait.
The real helper can then change without changing the adapter's interface.

`shunt::chatgptweb::fake::FakeDriver` binds an account/profile string pair and
replays `Fixture` answers by exact prompt/model match. Fixtures specify delta
boundaries and an optional terminal error. An optional readiness error simulates
failed pre-send checks for both operations. Fixtures are trusted test inputs;
use nonempty deltas and error states consistent with this contract. No profile
directory is opened. No browser, account, runtime, network, environment variable,
or credential is needed. Fake timeout errors are scripted, without waiting.

`tests/common/chatgptweb_conformance.rs::assert_case` is parameterized over
`Driver`, request, and expected result. It checks JSON round trips, correlation,
version, event ordering, operation-specific events, nonempty deltas, terminal
uniqueness, error metadata, and exact reconstructed text. TASK-003 can reuse
this runner with a real process client and controlled page states. The fake's
tests cover success, health, all error codes, Unicode and newline preservation,
empty answers, partial failures, uncertain sends, repeated requests, and account
isolation. A separate test parses every JSON example in this document.

Run `cargo test chatgptweb` for the browser-free contract tests. Run `npm test`
in `helpers/chatgptweb-browser` for the local browser fixtures and the same
conformance runner against the real helper process. Actual live-page mode and
quota validation remain TASK-006.

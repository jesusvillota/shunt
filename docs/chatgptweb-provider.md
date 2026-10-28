# ChatGPT Web text provider

`kind = "chatgptweb_browser"` serves text-only OpenAI Responses requests through
`[server.codex_endpoint]`. The provider instance can be named `chatgptweb`.
DEC-002 chose the distinct kind name. This provider does not use Codex OAuth.

The separate TASK-003 helper must be installed with its Node dependencies and
browser runtime. Set `SHUNT_CHATGPTWEB_HELPER` to its absolute `cli.mjs` path.
Shunt launches `node "$SHUNT_CHATGPTWEB_HELPER" --account NAME --profile-dir PATH`
through private JSON Lines pipes. The default script path is
`helpers/chatgptweb-browser/cli.mjs`, relative to shunt's working directory.
The helper owns login and checks Chat mode immediately before Send.
Missing helpers, unknown modes, login failures, and page changes fail closed.
TASK-005 handles interactive login; TASK-006 owns the live quota smoke test.

Direct helper runs accept `--chrome` to attach to an already running Chrome
at `http://127.0.0.1:9333` through CDP. `--cdp-url` accepts an HTTP loopback
endpoint without authentication. Start Chrome manually with its separate
`~/.local/share/shunt/chatgptweb-browser/chrome/cemfi` profile and the debugging
port. The helper adds no browser flags. Shutdown closes only its request tab,
leaving the owner's browser open. `--headless` applies only to launched
Chromium and cannot be combined with `--chrome`. Shunt does not pass these
flags. See the [helper instructions](../helpers/chatgptweb-browser/README.md).

Add the following to a legacy provider-table configuration. An ordered
`[[upstreams]]` configuration must declare all providers in that form instead.

```toml
[server.codex_endpoint]
provider = "codex" # preserve the existing default Codex pool

[[server.codex_endpoint.routes]]
model = "chatgptweb-chat"
provider = "chatgptweb"
upstream_model = "Medium" # optional: exact visible effort label

[providers.chatgptweb]
kind = "chatgptweb_browser"
base_url = "http://localhost" # legacy required field, unused by this adapter
auth = "none"

[[providers.chatgptweb.accounts]]
name = "personal"
profile_dir = "~/Library/Application Support/shunt/chatgptweb/personal"
```

`upstream_model` is optional. When set, it must exactly match the visible
effort label, such as `Medium` or `High`. The helper rejects a mismatch and
never changes effort. Omission leaves effort unchecked. No model entitlement or
context size is assumed. Configured browser routes appear in all Codex catalogs:
`/v1/models?client_version=...`, `/models`, and `/backend-api/codex/models`.
Their rows disable shell tools and advertise text input only. With no browser
routes the Codex catalog remains empty, as before.

Each provider requires exactly one named account with an absolute profile path
(after tilde expansion), `auth = "none"`, and no credential fields. Profile
paths must be unique across browser providers. Do not use symlink aliases for
the same profile. Shunt serializes each profile's requests until the response
body completes or disconnects. Multi-account rotation is outside this stage.
The endpoint default can also name this provider; catalog models still need
explicit routes. The Anthropic Messages endpoint does not support this kind.

The adapter sends instructions and the full ordered text transcript to a fresh
chat each turn. Tools, images, function-call items, and `previous_response_id`
return a clear error. Send full history again on each request. Streaming emits
Responses text deltas and a final message snapshot. Non-streaming requests
collect the same events into a JSON response. Token usage is unknown (`null`).

The helper and adapter share the `server.timeouts.upstream_ttfb_ms` budget for
the complete browser turn (default 120 seconds). Each helper frame is limited
to 1 MiB. Disconnect, timeout, malformed output, and shutdown terminate the
helper process group. Driver errors emit `response.failed`, including after
partial text; they never become a successful completion. No request is retried
or sent to another route, even if submission is uncertain. Driver diagnostic
messages are not returned to the client. Existing endpoint authentication
applies before browser dispatch and model discovery.

The offline integration tests install TASK-002's fake through the same `Driver`
interface. They need no browser, account, network service, or helper install.
See [the driver contract](chatgptweb-driver-protocol.md).

## Internal text tool protocol v1

This contract is implemented and tested in `adapters::chatgptweb::tool_protocol`.
It is not connected to the adapter yet. The endpoint and discovery catalog
continue to reject tools. Adapter integration belongs to TASK-014.
Helper wire protocol v1 is unchanged: `Generate.prompt` carries the transcript,
`TextDelta.text` carries answer fragments, and `Done` confirms completion.

### Captured Codex surface

`tests/fixtures/chatgptweb_codex/` records sanitized declarations and successive
requests from `codex-cli 0.164.0`. The ignored test reproduces the capture:

```sh
cargo test --test chatgptweb_codex_smoke capture_tool_surface -- --ignored --exact --nocapture --test-threads=1
```

The test starts a fake Responses endpoint on numeric loopback, with an isolated
Codex home and scratch Git repository. It returns predetermined calls, checks
that Codex reads a seed file and edits a result file, and asserts matching
next-turn results. It uses no browser or real provider. Instruction text,
private paths, generated item IDs and timings are removed or normalized.
The fixtures cover tool shapes, not the model's ability to choose valid calls.

Two model catalogs produce different tool surfaces:

- `gpt-5.5`: `exec_command` and `write_stdin` function declarations, plus
  custom/freeform `apply_patch` with a Lark grammar. Function `arguments` is a
  JSON-encoded object string. Patch `input` is exact text, with real newlines.
  Continuations contain `function_call_output` or `custom_tool_call_output`,
  with the same `call_id` and a string `output`.
- `gpt-6.1-sol`: ordered developer `additional_tools` items declare namespaces,
  including `functions.exec`. A `custom_tool_call` has `namespace: "functions"`,
  `name: "exec"` and JavaScript in its text `input`. Native shell and edit
  signatures remain in its declaration. Codex executes that text. Its matching
  `custom_tool_call_output.output` is an ordered array of `input_text` parts.

The complete direct inventory also includes `tool_search`. The contract
rejects that raw inventory. Fixture tests then remove only `tool_search` to
check the supported shell/edit transcript. The complete code-mode inventory
validates without removing declarations. Integration must reject unsupported
inventories before send; it must not silently filter client tools.

The inventory accepts function, custom and one-level namespace declarations.
Unsupported server tools, images, opaque reasoning items, and deferred tool
search are rejected. Nothing silently turns them into function tools.

### Transcript and validation

`Contract::new(request, turn)` validates before any browser dispatch. `turn`
is a fresh, unpredictable identifier chosen by the future caller, at most
64 bytes long. Other identifiers
contain 1–128 ASCII letters, digits, underscores, hyphens or dots. Namespace
and name are separate fields; the parser never splits a dotted name.

The prompt contains one framing instruction line, then one JSON object with
`version: 1`, `turn`, `instructions`, `tools`, `input`, `tool_choice` and
`parallel_tool_calls`. Instructions and complete tool declarations retain their
content. Input items and content parts retain their original order and fields.
A string input becomes one user message. History contains text messages,
additional tool declarations, calls and matching outputs. Custom input stays
text. Function arguments stay JSON-encoded strings and must decode to objects.

Every historical call must have a unique `call_id`, a declared name and the
right kind. Exactly one output of the matching kind must follow each call.
Outputs may arrive in any order within a parallel batch. A normal message
cannot interrupt outstanding results. Unmatched, repeated or missing outputs
fail validation. `previous_response_id` is unsupported; every turn must include
its retained transcript. Text result arrays are preserved without concatenation.
Quoted results, declarations and instructions are never fed to the answer parser.

Supported choices are `none`, `auto` (default), `required`, and
`{"type":"function"|"custom","name":"...","namespace":"..."}`
(the namespace is optional). Named choices must match a declaration.
`none` forbids calls; `required` and named choices require at least one.
Every call must match a named choice. `parallel_tool_calls: false` permits
at most one call. Unsupported choices and wrong field types fail validation.

Function schemas support types (including type arrays), `properties`,
`required`, `additionalProperties`, `items`, `minItems`, `enum`, `anyOf`, `oneOf` and
`allOf`. Descriptions, titles, defaults, examples, deprecated markers and
`$schema` and Codex's `encrypted` marker are preserved annotations. Unsupported validation keywords fail before send.
Custom formats accept text or declared Lark/regex grammars. Input must be text;
Shunt preserves the grammar but does not interpret it or parse JavaScript.
Codex remains responsible for tool-specific syntax, execution and permissions.

### Incremental answer framing

Ordinary assistant text streams as it arrives. The prefix `<<<SHUNT:` is
reserved everywhere in the current assistant answer, including quoted examples
and Markdown. It cannot be used as literal answer text. A control record is
exactly the following header, one JSON object, and one newline:

```text
<<<SHUNT:1:TURN>>>{"type":"function_call","name":"exec_command","arguments":"{\"cmd\":\"cat seed.txt\"}"}
<<<SHUNT:1:TURN>>>{"type":"custom_tool_call","name":"apply_patch","call_id":"patch_1","input":"*** Begin Patch\n*** Add File: result.txt\n+edited ✓\n*** End Patch\n"}
<<<SHUNT:1:TURN>>>{"type":"custom_tool_call","namespace":"functions","name":"exec","input":"text(await tools.exec_command({cmd: 'pwd'}));"}
<<<SHUNT:1:TURN>>>{"type":"done"}
```

`TURN` must equal this prompt's turn identifier. JSON strings escape embedded
newlines and framing examples. Records accept only the documented fields.
Duplicate JSON keys, unknown names, wrong kinds, malformed arguments, unknown
fields, wrong versions or turns, and duplicate call IDs fail closed.
Explicit IDs are retained. Missing IDs become `call_web_TURN_N`, skipping IDs
already in this history or answer. The assigned ID is returned unchanged to
Codex and must be included unchanged in the next transcript.

`Parser::push` accepts arbitrary UTF-8 string chunks from `TextDelta`. It
returns confirmed text immediately and retains only a possible prefix or one
incomplete record. It validates and holds calls internally. The explicit done
record must occur exactly once, including for a text-only or empty answer.
No bytes, including whitespace, may follow it. Only `Parser::finish`, called
on the driver's successful `Done`, releases validated calls as an ordered batch.
An EOF, driver error, timeout, disconnect or cancellation must discard the
parser. A terminal record alone never authorizes execution before driver Done.

Limits are fixed internally: 1 MiB serialized prompt/request, 4 MiB total
answer, 64 KiB per control record (including header and newline), 128 tools,
32 calls and 32 schema nesting levels. JSON also uses serde's recursion limit.
Ordinary text is not accumulated by the parser. A malformed or over-limit
record poisons the parser and drops pending calls; it cannot resume. Truncated
records or missing terminal records fail at finish. Previously streamed text
may remain visible, but failure must never be reported as successful completion.
No automatic replay is safe after output or uncertain browser submission.

The framing identifier limits accidental matches; it does not prevent prompt
injection. A model can still request destructive commands or unauthorized
edits. Shunt never executes them. Client approval and permission checks remain
necessary for every model-generated tool call.

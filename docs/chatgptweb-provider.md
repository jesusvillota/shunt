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

For direct helper runs, `--chrome` selects installed Google Chrome with
Playwright's default flags. Use the separate profile
`~/.local/share/shunt/chatgptweb-browser/chrome/cemfi`. The default is headed;
`--headless` hides the window. Shunt does not pass these flags. See the
[helper launch example](../helpers/chatgptweb-browser/README.md).

Add the following to a legacy provider-table configuration. An ordered
`[[upstreams]]` configuration must declare all providers in that form instead.

```toml
[server.codex_endpoint]
provider = "codex" # preserve the existing default Codex pool

[[server.codex_endpoint.routes]]
model = "chatgptweb-chat"
provider = "chatgptweb"
upstream_model = "GPT-5.6" # example only: exact selected browser model label

[providers.chatgptweb]
kind = "chatgptweb_browser"
base_url = "http://localhost" # legacy required field, unused by this adapter
auth = "none"

[[providers.chatgptweb.accounts]]
name = "personal"
profile_dir = "~/Library/Application Support/shunt/chatgptweb/personal"
```

Use the exact model label selected in the browser as `upstream_model`. The
helper rejects a mismatch instead of changing models. No model entitlement or
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

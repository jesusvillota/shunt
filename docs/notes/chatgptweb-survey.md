# ChatGPT Web driver and shunt adapter survey

TASK-001 of `chatgptweb-provider`. Survey date: 2026-10-23. This is research,
not a provider implementation or a live smoke test. DEC-001 and DEC-002 retain
the choices of tool protocol, driver runtime, and provider name.

Evidence revisions:

- Shunt: `fa430c0c1c37bece9bfac29148eac46eedb76721`. All shunt `file:line`
  references below refer to this revision. The source was unchanged from the
  initial worktree revision, `3651c1ad24bf7ac06befd650643f833976793084`.
- Upstream: `miuuyy/codex-chatgpt-web`, commit
  `0901b9fb1976b4a984c7e9b4082c2868767bbf7c`, package version `6.1.13`.
  The repository was cloned for source inspection. No installer was run.
- Official OpenAI documentation was fetched on the survey date. No ChatGPT
  prompt was sent. A read-only browser inspection found no Chrome windows.

## Driver mechanics

The upstream bridge receives Responses API requests, then drives a signed-in
Electron page. It translates the page answer into Responses events. Its
[architecture](https://github.com/miuuyy/codex-chatgpt-web/blob/0901b9fb1976b4a984c7e9b4082c2868767bbf7c/docs/architecture.md#L3-L52)
distinguishes `browser-only` from `full`, which adds tools through MCP
(Model Context Protocol, a tool interface).

The launcher owns a persistent login partition and at most five task-bound
tabs. Tabs share login state, but each task owns its exact page. Closing a
running tab cancels its turn. Submission uses logical `data-turn-id` identities
and persistent `data-turn-id-container` wrappers. Display indexes can change
when history is rendered again. Missing or duplicate identities cause errors;
an accepted prompt is never resent to repair its page representation.
See [architecture.md:98-127](https://github.com/miuuyy/codex-chatgpt-web/blob/0901b9fb1976b4a984c7e9b4082c2868767bbf7c/docs/architecture.md#L98-L127).

The browser worker performs these steps:

1. Select the requested model family and effort. Reopen its menu to verify
   persistence. Before Send, require the expected label, unchanged URL, closed
   menu, and editable composer. See
   [browser-worker.ts:2694-2885](https://github.com/miuuyy/codex-chatgpt-web/blob/0901b9fb1976b4a984c7e9b4082c2868767bbf7c/src/adapters/chatgpt-web/browser-worker.ts#L2694-L2885).
2. Insert the complete prompt and read it back exactly. Attach the exact
   connector when tools are enabled. Plain-text DOM insertion avoids corruption
   from live Chrome DevTools Protocol typing in the Lexical editor. See
   [browser-worker.ts:3912-3965](https://github.com/miuuyy/codex-chatgpt-web/blob/0901b9fb1976b4a984c7e9b4082c2868767bbf7c/src/adapters/chatgpt-web/browser-worker.ts#L3912-L3965)
   and [4288-4306](https://github.com/miuuyy/codex-chatgpt-web/blob/0901b9fb1976b4a984c7e9b4082c2868767bbf7c/src/adapters/chatgpt-web/browser-worker.ts#L4288-L4306).
3. Press Enter once and concurrently check that the page accepted the message.
   Recheck the model before physical Send. See
   [browser-worker.ts:4024-4089](https://github.com/miuuyy/codex-chatgpt-web/blob/0901b9fb1976b4a984c7e9b4082c2868767bbf7c/src/adapters/chatgpt-web/browser-worker.ts#L4024-L4089)
   and [5804-5808](https://github.com/miuuyy/codex-chatgpt-web/blob/0901b9fb1976b4a984c7e9b4082c2868767bbf7c/src/adapters/chatgpt-web/browser-worker.ts#L5804-L5808).
4. Read the answer from the page and emit incremental Markdown text. Completion
   requires a present, nonempty response, no running indicator, and completed
   action evidence. Full mode also requires a tool-activity fence: no outstanding
   tool call, and a new final answer after the tool result. See
   [browser-worker.ts:1539-1549](https://github.com/miuuyy/codex-chatgpt-web/blob/0901b9fb1976b4a984c7e9b4082c2868767bbf7c/src/adapters/chatgpt-web/browser-worker.ts#L1539-L1549),
   [6017-6096](https://github.com/miuuyy/codex-chatgpt-web/blob/0901b9fb1976b4a984c7e9b4082c2868767bbf7c/src/adapters/chatgpt-web/browser-worker.ts#L6017-L6096),
   and [security-model.md:32-38](https://github.com/miuuyy/codex-chatgpt-web/blob/0901b9fb1976b4a984c7e9b4082c2868767bbf7c/docs/security-model.md#L32-L38).

Sign-in stays inside the owned Electron partition. Setup requires both a
server-authenticated session and the Temporary Chat composer. A visible editor
alone is insufficient. There is no cookie import or external profile handoff.
Narrow selectors fail explicitly when the page changes. Troubleshooting asks
for observed page evidence instead of guessed selectors. See
[security-model.md:91-104](https://github.com/miuuyy/codex-chatgpt-web/blob/0901b9fb1976b4a984c7e9b4082c2868767bbf7c/docs/security-model.md#L91-L104)
and [TROUBLESHOOTING.md:183-194](https://github.com/miuuyy/codex-chatgpt-web/blob/0901b9fb1976b4a984c7e9b4082c2868767bbf7c/TROUBLESHOOTING.md#L183-L194).

The installer cannot be adopted unchanged. It owns Codex's `openai_base_url`;
external-router composition is unsupported. Shunt must keep its existing
provider route and integrate the driver below it. See
[TROUBLESHOOTING.md:69-87](https://github.com/miuuyy/codex-chatgpt-web/blob/0901b9fb1976b4a984c7e9b4082c2868767bbf7c/TROUBLESHOOTING.md#L69-L87).

## Chat vs Work mode

The page's **Chat/Work switcher** is the mode evidence. Model names, effort,
Think, Temporary Chat, `browser-only`/`full`, and Automatic/Zero Risk are
separate settings. None establishes that Chat is active.

Official evidence: [Use ChatGPT](https://learn.chatgpt.com/codex/use-chatgpt),
section “Choose how you want to work,” says: “Use Chat for a question or
back-and-forth. Turn on Work in the switcher when you want ChatGPT to carry a
larger task through to a reviewable result.” Its feature-availability paragraph
states: “ChatGPT Work and Codex share usage limits.” The fetched
[Markdown page](https://learn.chatgpt.com/codex/use-chatgpt.md) contained these
statements at lines 28-31 and 96-97.

The existing owner skill, `delegate-chatgptweb/SKILL.md:30-36,57` in the config
repository, also requires reading Chat on the toggle before typing. Its local
path is `~/GitHub/config/shared/skills/delegate-chatgptweb/SKILL.md`.
This is policy evidence, not proof of the current page's exact DOM structure.

Proposed driver check, to be validated against the actual signed-in page:

1. Read a fresh accessibility tree or DOM snapshot of the Chat/Work switcher.
2. For a collapsed switcher, read its displayed current value. For separate
   choices, read the selected item's actual selection state. Use the state the
   inspected page exposes, such as `aria-selected`, `aria-checked`, or native
   selection. Merely finding the words Chat and Work in a menu proves nothing.
3. Require exactly **Chat**. Missing, conflicting, unfamiliar, or unrecognized
   localized state means unknown. Reject the request before sending. Do not
   infer the mode from a model label or fall back to Work.
4. Recheck immediately before every physical Send, including prompt parts and
   connector verification. A pre-send gate must also hold for retained chats.

No production selector is established by this survey. A read-only inspection
used `orca computer list-apps --json`, then
`orca computer list-windows --app com.google.Chrome --json`; the latter returned
`"windows": []`. No active page mode was observed or changed. Source search of
the upstream worker, launcher, tests, and docs found no Chat/Work page check.
Its model gate at `browser-worker.ts:2860` and Think `aria-pressed` check at
`:1571` concern other controls. TASK-003 needs a captured page fixture and
negative cases for Work, missing state, and changed labels before live sends.
This is a future driver requirement, not a claim that the upstream is safe to run.

To demonstrate that Codex quota did not move, TASK-006 should do the following:

1. Quiesce all other Codex and Work use for the tested physical account.
   Record a fresh `goal pool --goal chatgptweb-provider --json` snapshot privately.
   Match `providers[].provider == "codex"`, `auth == "chatgpt_oauth"`, and the
   correct account through `account_ref`. Check every pooled account if routing
   can select more than one.
2. Save `utilization_5h`, `reset_5h`, `utilization_7d`, and `reset_7d`, plus account,
   plan, and observation time. Utilization is a fraction from 0 to 1. The human
   display rounds percentages; compare raw JSON instead.
3. Record affirmative Chat mode immediately before one controlled browser turn.
   Afterward, wait for a fresh upstream usage poll, then obtain another snapshot.
   The optional poller has a 60-second minimum interval
   (`src/usage_poll.rs:57-76,91-95`). Prove that a poll succeeded after the turn;
   repeated cached values are insufficient.
4. Require equal raw five-hour and weekly utilization within the same reset
   windows and account/plan. Repeat after usage has settled. A reset, missing
   meter, poll failure, stale data, or concurrent consumption invalidates the
   comparison. Unchanged request counters or reset times alone prove nothing.
5. Report equality at the upstream meter's precision. Percent-based counters
   cannot establish mathematically zero consumption below that precision.

Evidence from this session: `goal pool --goal chatgptweb-provider --json`
succeeded and exposed those fields for two Codex accounts. Anonymous
five-hour/weekly pairs were `(0.09, 0.50)` and `(0.22, 0.50)`. This was one
snapshot, without a browser turn; it proves availability of the meter only.

Field sources: `src/pool/model.rs:30-46`; `src/auth/codex/usage.rs:190-203`
divides upstream `used_percent` by 100. `:217-240` identifies windows by duration,
not primary/secondary ordering. `src/accounts.rs:1653-1661` applies fresh usage
and updates observation times. The private `wham/usage` endpoint can change or
fail (`src/auth/codex/usage.rs:15-24`). `reset_7d_oi` is not a Codex quota window
(`src/auth/codex/usage.rs:176-183`).

The goal tool's `goal-core/src/pool.rs:13-30,202-210` relays `shunt pool --json`.
Its `:84-88` explicitly has no pool provider for browser worker sessions.
Therefore, do **not** compare a `chatgptweb` worker row. These references are in
`~/GitHub/config/tools/goal-rust/goal-core/src/`, inspected during this session.

## Shunt seams

These are extension points and current limits, not code changes authorized by
this report. An adapter converts one provider's protocol to shunt's protocol.
Account failover selects another account; route failover selects another
upstream. Both need deliberate browser failure handling.

| File and line | Existing behavior and required integration |
|---|---|
| `src/config.rs:2152`; `src/routing.rs:14,30` | Add a distinct `ProviderKind`, its `AdapterKind`, and the conversion. A Responses-shaped client does not make browser execution an existing Responses HTTP provider. |
| `src/config/presets.rs:11,25`; `src/config/upstreams.rs:12,232-265` | Presets and ordered upstream normalization carry kind, URL, auth, accounts, and profile settings. A browser preset and valid local transport representation need design; do not point it at the Codex billing backend. |
| `src/config.rs:1993-2002`; `src/config/upstreams.rs:113-117,189-199` | `AccountConfig.profile_dir` already exists, with tilde expansion. Inline account selections preserve it. Its current documented use is an Antigravity usage probe. A browser adapter must read the selected account's directory and require unique, private profiles. No browser account resolver exists yet. |
| `src/config.rs:1876-1891,5427`; `src/config/upstreams.rs:49-51,264` | Provider-level `profile_dir` is a different field, currently for `antigravity_cli`. Its getter returns the provider directory, not the selected account directory. Do not use it to collapse a multi-account browser pool into one login. |
| `src/adapters/mod.rs:43-60,370-416`; `src/proxy/failover.rs:700-739` | Register the new adapter module and dispatch arm. Preserve response bounds and streaming. `AdapterError.failure` distinguishes raw upstream failure from local validation errors. A wrong or unknown mode must fail closed as a local error. |
| `src/accounts.rs:564,855,1725,1804,2161,2335` | Reuse account selection, cooldown, relogin status, health, and admission. Add exclusive per-profile turn ownership: existing pool admission is not a browser-profile lock. Keep browser health and quota independent of Codex billing quota. |
| `src/adapters/responses/pool.rs:39-57`; `src/accounts.rs:3692,3790` | Reference pattern for account iteration and `FailoverAction`. Codex HTTP classification rotates on 429 and refreshes on 401. Browser login, challenge, model entitlement, and UI errors need typed driver classifications; OAuth refresh is not browser sign-in. |
| `src/proxy/failover.rs:700`; `src/proxy/chain_stream.rs:1-16` | Route dispatch is separate from account rotation. The streaming chain currently covers Anthropic and Responses without WebSockets. A new kind needs explicit eligibility and pre-output retry rules. Never replay a prompt after uncertain acceptance or replay side effects after output. Never fall back to a Codex/Work route to satisfy this goal. |
| `src/adapters/antigravity/mod.rs:350-405,944`; `src/adapters/antigravity/child.rs:145-225` | Subprocess precedent: absolute private directory, piped output, concurrent stderr drain, process group, `kill_on_drop`, deadline, kill and reap. Browser helper requires an explicit protocol, cancellation, crash handling, and account ownership. Reuse lifecycle principles, not `agy` command arguments or its credential environment. |
| `src/adapters/antigravity/child.rs:32-65,128-143`; `src/shutdown.rs:89,120-127` | Registry prevents spawns escaping a shutdown sweep. Graceful and forced exits explicitly terminate child groups. A long-lived browser helper needs both hooks and browser descendants included; the current registry is Antigravity-specific. |
| `src/codex_endpoint.rs:272,335-362,385-455`; `src/codex_endpoint/websocket.rs:227` | Native Responses input currently dispatches through Responses paths, including routed requests. Add browser dispatch here, using decoded requests and shared turn handling for HTTP and WebSocket clients. Updating the Anthropic proxy dispatcher alone cannot serve Codex. |
| `src/config.rs:4569,4644-4665`; `src/codex_endpoint.rs:122` | Codex endpoint validation requires Responses kind; routed providers also require a server-side credential. Browser profile auth needs an explicit validation path. Keep gateway-owned errors in OpenAI Responses shape. Do not bypass authentication by pretending browser profiles are OAuth tokens. |
| `src/config.rs:2399`; `src/discovery.rs:202-206,256-296`; `src/server.rs:261-268` | `[[models]]` feeds Anthropic discovery. Codex-negotiated `/v1/models?client_version=...`, `/models`, and `/backend-api/codex/models` currently return `{"models":[]}` deliberately. Add complete Codex `ModelInfo` rows and matching browser routes; merely adding `[[models]]` does not populate Codex's picker. |
| `src/models_catalog.rs:18,151-183` | This module queries CLI catalogs for `shunt models` and ranking. It is not the HTTP catalog writer. `codex debug models` can reflect the completed discovery integration. A new worker platform is unnecessary. |
| `src/pool/mod.rs:87-125`; `src/pool/model.rs:15-55` | These modules are the read-only admin pool client and wire types, not the rotation engine. Browser health/profile account visibility will need the corresponding admin snapshot support. Use these types to read Codex usage during the smoke test. |

The future implementation must keep the existing `codex` pool unchanged. Browser
profile login stays an explicit owner step in TASK-005. This report changes no
public config keys or credential writeback behavior.

## Tool-call approaches

| Property | A: MCP connector + tunnel | B: text protocol |
|---|---|---|
| Found implementation | Upstream `full` mode implements it. ChatGPT calls a custom connector; an outbound OpenAI tunnel reaches a local stdio MCP server. | Proposed for this goal. Upstream browser-only mode explicitly has no local tools; it does not implement B. |
| Account setup | Exact new connector name, Developer Mode, tunnel, runtime key, same OpenAI account/workspace, and Allow all actions. Permission prompts can still appear. | Logged-in isolated browser profile and verified Chat mode. No MCP connector or tunnel is needed for local tool calls. |
| Tool execution | A request-scoped broker exposes only current outer Codex tools. Calls emit Responses tool-call events; native Codex performs execution and returns results. | Prompt includes allowed tool names, schemas, and prior results. Parse a strict block into Responses `function_call` items. Codex executes and returns `function_call_output` next turn. |
| Shunt needs | MCP server, tunnel supervision and credentials, per-account connector config, random per-turn capability, tool inventory, request/result correlation, continuation, cancellation, and completion fencing. Preserve native client approvals and sandbox. | Transcript serialization, explicit framing/version, strict incremental parser, name/schema validation, size limits, unique call IDs, response event synthesis, result replay, and malformed-block errors. Preserve client approvals and sandbox. |
| Rotation | Turn token, connector, browser page, and in-flight tool calls bind to one account. A turn cannot move freely between profiles. | Fresh chat plus full transcript makes a subsequent turn portable. Retry is safe only before accepted submission or executed side effects; self-contained context does not make blind replay safe. |
| Quality and latency | Native connector calls can run multiple tool rounds inside one web answer. More infrastructure and completion races. | More page turns and repeated context; adherence and partial-block behavior need measurement. No measured reliability or latency advantage is claimed. |

A evidence: [architecture.md:41-52](https://github.com/miuuyy/codex-chatgpt-web/blob/0901b9fb1976b4a984c7e9b4082c2868767bbf7c/docs/architecture.md#L41-L52),
[security-model.md:9-47,65-69,123-129](https://github.com/miuuyy/codex-chatgpt-web/blob/0901b9fb1976b4a984c7e9b4082c2868767bbf7c/docs/security-model.md#L9-L47),
and [TROUBLESHOOTING.md:225-239](https://github.com/miuuyy/codex-chatgpt-web/blob/0901b9fb1976b4a984c7e9b4082c2868767bbf7c/TROUBLESHOOTING.md#L225-L239).
The MCP implementation inventories tools at
[mcp-server.ts:785](https://github.com/miuuyy/codex-chatgpt-web/blob/0901b9fb1976b4a984c7e9b4082c2868767bbf7c/src/adapters/chatgpt-web/mcp-server.ts#L785),
invokes exact wire names at `:885`, and starts stdio transport at `:1068`.
[index.ts:243-267](https://github.com/miuuyy/codex-chatgpt-web/blob/0901b9fb1976b4a984c7e9b4082c2868767bbf7c/src/adapters/chatgpt-web/index.ts#L243-L267)
maps results and emits tool calls to the outer client.
Its `:1383-1404,1486-1494` waits for a tool batch and ends the client round
while keeping the browser response alive. On the next client request,
`:1330-1358` matches result IDs and releases the pending MCP calls. Shunt
therefore needs retained tool-loop state across HTTP requests for A. The
tunnel starts before the daemon and shutdown drains browser/tool activity
([architecture.md:227-233,267-279](https://github.com/miuuyy/codex-chatgpt-web/blob/0901b9fb1976b4a984c7e9b4082c2868767bbf7c/docs/architecture.md#L227-L233)).

Shunt is a gateway, not a local tool executor. A cannot simply run a shell
inside shunt: upstream authority recovery depends on native Codex environment
and rollout evidence. That contract needs adaptation to an authenticated gateway
request without trusting prompt text as authority. B naturally uses the client's
normal tool loop, but its framing must distinguish instructions from quoted
repository text. Do not interpret arbitrary prose as an executable call.

Useful B precedents in shunt are `src/adapters/cursor/tool_use_xml.rs:1-36,75`
(incremental text recovery, allowed names, generated IDs) and
`src/model/responses_request.rs:603,629` (`function_call`/`function_call_output`).
These are building blocks, not proof that browser B works. Full Codex tool
schemas may include custom/freeform or tool-search surfaces; translating all
of them is separate from a minimal function-call spike. Claude's Anthropic
tool-use path remains TASK-009's feasibility question.

The goal's B-first proposal has a clear integration advantage: no connector
credential or broker service. A has an existing end-to-end reference. DEC-001
must weigh that evidence against unmeasured B reliability; this report does not
select either approach or create a new decision request.

## Risks

- **Mode safety:** no upstream Chat/Work guard was found. Validate the actual
  switcher's selected state before any send. Official shared Work/Codex limits
  support the risk; only TASK-006 can test this integration's quota behavior.
- **UI drift and acceptance:** page labels and logical turn IDs can change.
  Unknown state must fail clearly. An accepted or uncertain send cannot be
  repeated automatically. The driver must expose that distinction to shunt.
- **Account isolation and concurrency:** upstream supports normal and separate
  DEV profiles, not shunt account rotation. Start with one serialized turn per
  account, a unique profile, bounded queue, and explicit cancellation. Five
  upstream tabs are a cap, not evidence that five turns are safe for this pool.
- **Account restrictions:** automation and ban risk are owner-accepted in the
  goal README. Respect capacity errors and cooldowns; no pacing technique
  guarantees against account restrictions. Shared accounts can lose both web
  and Codex access.
- **Tool authority:** prompt injection applies to both approaches. A adds
  tunnel credentials, cached connector schemas, and completion races. B adds
  parsing ambiguity, missing blocks, duplicate calls, and transcript growth.
  Keep approvals and sandbox enforcement in the client.
- **Local exposure and data:** upstream inference loopback has no independent
  bearer secret, although lifecycle endpoints do. Its Origin guard is not
  authentication. Reuse shunt authentication; keep profiles and tunnel keys
  private. Saved chats are upstream's default and can apply memory/custom
  instructions. Do not copy browser secrets or task transcripts into this PR.
  See [security-model.md:58-89,106-121](https://github.com/miuuyy/codex-chatgpt-web/blob/0901b9fb1976b4a984c7e9b4082c2868767bbf7c/docs/security-model.md#L58-L89).
- **Context and stream semantics:** browser extraction streams rendered text,
  not a stable token API. Context and compaction limits vary by model/account.
  Preserve live text delivery; bound unfinished tool blocks without buffering
  a whole answer. Do not advertise copied context limits as measured guarantees.
- **Reuse and license:** upstream's
  [LICENSE:1-21](https://github.com/miuuyy/codex-chatgpt-web/blob/0901b9fb1976b4a984c7e9b4082c2868767bbf7c/LICENSE#L1-L21)
  is MIT, copyright 2026 codex-chatgpt-web contributors. It permits copying,
  modification, distribution, sublicensing, and commercial reuse. Preserve the
  copyright and permission notice in copies or substantial portions; the code
  has no warranty. Record source revision and modifications when extracting
  worker logic. Review dependency and bundled binary notices separately,
  including Electron and `openai/tunnel-client`; MIT on this repository does
  not relicense dependencies or grant service permissions. Upstream pins
  tunnel-client `v0.0.16` and verifies its SHA256 (`src/tunnel.ts:9-14,131-138`).
  Tunnel-client has a separate
  [Apache-2.0 license](https://github.com/openai/tunnel-client/blob/5f99daabd4aa4a77049e6d81d54a0d8c18335397/LICENSE#L1-L9).
  The upstream `LICENSES/Bun-1.4.0.md:1-9` also covers WebKit/JavaScriptCore
  LGPL notices; `LICENSES/libnotify-0.8.7-LGPL-2.1.md:1-5` covers a Linux
  dependency. Reusing browser source need not mean redistributing these
  runtimes. This PR copies no production implementation or bundled binary.
- **Runtime choice:** a Node/Bun helper can reuse TypeScript browser logic and
  update selectors independently; it adds a runtime and supervised processes.
  Rust keeps one primary runtime but requires porting browser control. Both
  need a shunt-owned versioned protocol. DEC-002 owns that choice.
- **Provider naming:** `goal-core/src/dispatch.rs:26,2399-2400` already reserves
  `chatgptweb` for a goal worker using a GitHub connector. Leave that platform
  unchanged. Options for the shunt provider kind are `chatgpt_browser`
  (clear browser transport, preferred candidate), `chatgpt_web_driver`
  (explicit but longer), or `chatgpt_web` (shorter, easier to confuse in speech).
  Use a separate configured instance name such as `web_chat_pool` and visible
  model suffix `(ChatGPT Web)`. These are DEC-002 options, not implemented keys.

Documentation surfaces considered: this new `docs/notes/` report is the task
deliverable. README and its translations, site and its translations, and
milestone specs need no update because no behavior or setup changes here.
`wiki/` is generated and was not edited. Later implementation tasks must update
their affected public documentation together with the code.

# Claude Code through the ChatGPT Web provider

TASK-009, goal `chatgptweb-provider`, M-4. This is a feasibility report.
It adds no provider support or production code.

**Finding: feasible with limits.** Claude Code can use shunt's existing
Anthropic Messages entry point. A browser adapter can return normal tool
calls for Claude Code to execute. The missing part is a reliable browser
text protocol, shared with the Codex tool path. This is an engineering
assessment, not a successful live Claude Code session.

Source baseline: `142f2434183bcd009d77faf542d61fae96bb3308`.
TASK-001's [survey](chatgptweb-survey.md) and committed outcome were read.
DEC-001 and DEC-002 remain proposed. This report evaluates approach B
conditionally. It does not choose the tool approach, runtime, or provider name.
`chatgptweb` below is the goal's working name.

## Entry point and routing

Claude Code sends Anthropic Messages requests. These already reach
`proxy::post` through `/v1/messages` (`src/server.rs:270`). Shunt reads the
request's `model` and resolves a route chain
(`src/proxy/failover.rs:112`). No new Claude-specific HTTP endpoint is needed.

The existing `[[models]]` shape can map a visible alias to a provider's model:

```toml
# Illustrative model entry; web_chat_pool is a future provider instance.
# This is not a complete, runnable provider configuration.
[[models]]
id = "web-chat-code"
display_name = "ChatGPT Web for Claude Code"
upstream_model = { web_chat_pool = "<validated web model selector>" }
```

`ModelConfig` defines these fields at `src/config.rs:2399`.
`src/routing.rs:296` resolves this map before exact routes at
`src/routing.rs:327`. Use one explicit browser mapping initially. A fallback
to a Codex API provider would violate this goal's Chat-only requirement.
Preserve the visible alias in replies: Claude Code uses it to restore the
model on resume (`src/routing.rs:360`).

The same mapping pattern works for Antigravity, but its transports differ:
native `Antigravity` uses the Gemini adapter (`src/routing.rs:37`);
deprecated `AntigravityCli` runs `agy`. That CLI rejects caller tools
(`src/adapters/antigravity/mod.rs:806`). It runs its own agent and returns no
caller `tool_use` blocks (`src/adapters/antigravity/mod.rs:11`). Do not copy
that tool-execution behavior for Claude Code.

The browser kind still needs registration in `ProviderKind`
(`src/config.rs:2152`), `AdapterKind` and its conversion
(`src/routing.rs:14`, `src/routing.rs:30`), and adapter dispatch
(`src/proxy/failover.rs:700`). M-1 should own the basic browser adapter.
M-4 should extend that adapter's Anthropic tool handling.

Anthropic model discovery and Codex discovery are separate. Configured model
entries can be exposed on the Anthropic side, but `ModelEntry::new` supplies
no context or capability values (`src/discovery.rs:106`). Codex's negotiated
catalog is deliberately empty (`src/discovery.rs:256`,
`src/discovery.rs:295`). Claude Code support does not depend on completing
that Codex catalog.

## Existing translations and their limits

| Existing path | Evidence | Use for M-4 |
|---|---|---|
| Messages history to Responses tools | `src/model/responses_request.rs:602`, `src/model/responses_request.rs:618` preserve `id` / `tool_use_id` as `call_id`. | Preserve the same call/result relationship in the browser transcript. |
| Responses calls to Anthropic blocks | `src/model/responses.rs:582` opens `tool_use`; its finalizer chooses `tool_use` after a call at `src/model/responses.rs:271`. | Reference for block IDs and terminal stop reasons. |
| Cursor text to tool calls | `src/adapters/cursor/tool_use_xml.rs:75`, `src/adapters/cursor/tool_use_xml.rs:185` parse incremental text, check names, and require JSON objects. | A parsing precedent, not a ready browser protocol. |
| Cursor tool pause | `src/adapters/cursor/sse.rs:348` emits the tool block, JSON delta, block close, and `stop_reason: "tool_use"`. | Reference for Claude Code's tool handoff. |
| Native Antigravity / Gemini | `src/routing.rs:37` maps native Antigravity to Gemini. | Evidence that Messages ingress can serve another provider protocol. |

[M16](../m16-codex-inbound-translation.md) describes the opposite ingress:
Responses requests become Anthropic or Chat Completions requests. It explicitly
calls its implementation a translation core without dispatch. Its thinking
codec preserves an existing signature; it does not create Anthropic reasoning.
The Codex route validator still requires `ProviderKind::Responses`
(`src/config.rs:4644`). M-4 should use Messages ingress directly and share
the browser protocol core with M-2. It need not route Claude Code through
M16 or add an internal HTTP round trip.

Cursor's recovery parser needs stricter rules for B. It searches for
`<tool_use` anywhere (`src/adapters/cursor/tool_use_xml.rs:107`). An unfinished
element waits for a close tag, then becomes text on flush
(`src/adapters/cursor/tool_use_xml.rs:133`). Invalid complete elements also
become text (`src/adapters/cursor/tool_use_xml.rs:152`). Its checks do not
validate the full tool schema. Reusing it unchanged would hide malformed
tool output and could confuse quoted examples with commands.

## What approach B needs in Anthropic format

Approach B means ChatGPT writes a framed tool request in its answer.
Shunt parses that request. Claude Code runs the tool through its usual
permission and execution path. Shunt must never execute shell commands or
file edits received in browser text.

The shared M-2 protocol should have one strict, versioned representation.
M-4 needs an Anthropic input/output layer around it:

1. Serialize `system`, ordered message roles, text, prior tool calls, and
   tool results. Include allowed tool names, descriptions, and `input_schema`.
   Preserve `is_error` on results. Escape transcript data so embedded protocol
   markers cannot terminate a history field.
2. Preserve historical call IDs. Mint unique IDs for newly accepted calls,
   then keep them unchanged in `tool_use.id` and later `tool_result.tool_use_id`.
   Reject duplicate or unmatched results. Fresh chats must include the full
   retained transcript, so the next account can understand the continuation.
3. Define explicit text and tool frames, including a terminal frame. Parse
   only the current assistant answer, outside quoted history. Bind frames to
   the current turn with a generated identifier. This reduces accidental
   matches; it does not remove prompt-injection risk.
4. Validate a complete tool batch before exposing calls. Check the allowed
   names, object arguments, schemas, call count, byte limits, and `tool_choice`.
   `none` permits no calls; `any` requires a call; `tool` requires that name.
   `disable_parallel_tool_use` permits at most one call. Reject a violation
   with an Anthropic error, rather than reporting a successful text answer.
5. Stream confirmed text as it arrives. Buffer only bounded incomplete frames
   and the tool batch awaiting validation and the driver's completion signal.
   Never wait for the full text answer before serving its text. Browser output
   can be revised: the driver must identify stable appended text and detect
   rewrites before the parser commits a tool call.
6. Handle text-only tool results first. Reject unsupported images, documents,
   server-side tools, and unknown block types before sending to the browser.
   Define explicit behavior for sampling controls, stop sequences, output
   formats, and `max_tokens`; a web composer cannot promise API-level control.
   Do not silently drop requested behavior.

Deferred tools need special care. Claude Code may send `ToolSearch` and
`tool_reference` results. The existing Responses path maps these references
to loaded schemas (`src/model/responses_request.rs:649`). M-4 needs an
equivalent loaded-tool inventory, or a documented first release with tool
deferral disabled in Claude Code. It must reject an unsupported deferred-tool
request clearly. Tool names alone are insufficient for argument generation.

For example, the semantic round trip is:

```text
Messages request: tools=[Read(path)], user="Read Cargo.toml"
Web answer:       <versioned frame: call Read with {"path":"Cargo.toml"}>
Messages reply:  tool_use(id="toolu_web_1", name="Read", input={...})
                 stop_reason="tool_use"
Claude Code:     checks permission, runs Read
Next request:    assistant tool_use(id="toolu_web_1", ...)
                 user tool_result(tool_use_id="toolu_web_1", content="...")
Next web chat:   receives full history, schemas, and that result
Messages reply:  text answer, stop_reason="end_turn"
```

The example shows meaning, not a second proposed wire syntax. Use M-2's
actual framing after DEC-001. Returned results are data, never new instructions.

For `stream: true`, emit Anthropic server-sent events (SSE), which deliver
small event records over one HTTP response:

```text
message_start
content_block_start(text) -> text_delta* -> content_block_stop   [if text]
content_block_start(tool_use, id, name, input={})
content_block_delta(input_json_delta, partial_json=<validated JSON>)
content_block_stop                                            [per call]
message_delta(stop_reason="tool_use", usage=...)
message_stop
```

A text-only answer ends with `end_turn`. Multiple validated calls need distinct
block indexes and IDs, followed by one terminal message pair. Send `ping`
events during browser waits. `src/model/responses.rs:300` already starts a
message before upstream output to prevent a client stall. For non-streaming
requests, assemble the equivalent Messages JSON. On truncation or malformed
frames, emit an error instead of a clean completion. Do not expose partial
JSON as an executable call.

Only a clearly unsubmitted browser turn is safe for automatic retry.
An uncertain Send, a committed call, or output already sent to Claude Code
must prevent transparent replay. A fresh transcript makes later turns
portable between accounts; it does not make the current turn safe to repeat.

## Thinking blocks

Anthropic thinking includes opaque signatures. Shunt already distinguishes
its own substitutes from Anthropic-issued values
(`src/model/thinking_signature.rs:1`, `src/model/thinking_signature.rs:39`).
The Anthropic adapter removes recognized foreign thinking on a later model
switch (`src/adapters/anthropic/thinking.rs:30`). Responses reasoning is only
opened when thinking was requested (`src/model/responses.rs:427`).

ChatGPT Web does not provide an Anthropic signature or the Responses API's
encrypted reasoning item. A page's progress label is not a thinking block.
The initial M-4 scope should require Claude Code thinking to be disabled.
Reject requests that require extended/adaptive thinking or its token budget.
This does not stop a web model reasoning internally. Do not promise that its
internal reasoning is visible or controlled by Anthropic settings.

For history from another model, exclude opaque `thinking` and
`redacted_thinking` payloads from the browser prompt under an explicit
documented policy. Preserve accompanying text and tool calls; omit a message
left with no usable content. Test model switches with real client fixtures.
Do not invent signatures or send browser reasoning as signed Claude thinking.
If visible reasoning becomes a later requirement, it needs a separate mapping
and round-trip tests, including the existing signature recognition boundary.

## Context, latency, and usage cost

With a fresh chat each turn, the browser receives history again. Let `H` be
retained history tokens, `S` tool-schema tokens, and `F` protocol framing.
Each request costs about `P = H + S + F` input tokens, before web-added text.
This is a size model, not an observed usage measurement.

For `n` turns with initial history `H0` and average history growth `d`, total
input is approximately `n(H0 + S + F) + d*n*(n-1)/2`. For example, `H0=20k`,
`S+F=5k`, `d=2k`, and ten turns imply about 340k total input tokens. The last
turn carries about 43k. These are arithmetic examples, not web-model limits.
Tool results, schema inventories, and repeated repository text can dominate.

Reserve output space and web-system overhead when bounding the prompt.
Do not assume the goal README's quoted context ranges apply to each account.
The browser model and plan need measured limits. A `[1m]` model suffix is only
a client hint that routing strips (`src/routing.rs:115`), not proof of capacity.
Configured discovery entries also omit maximum input/output values
(`src/discovery.rs:113`). Large Claude tasks will need earlier compaction,
which summarizes history. Never trim tool IDs or paired results silently.

`/v1/messages/count_tokens` must be answered locally without opening a chat.
Its dispatcher explicitly lists non-Anthropic adapters
(`src/proxy/failover.rs:603`); the browser kind must join that path.
The current counter includes system text, history, and schemas
(`src/count_tokens.rs:38`) but estimates framing. Browser serialization must
also count its added protocol text. If only an approximation is possible,
document it and keep conservative headroom. The existing fallback is 501,
which tells Claude Code to estimate locally (`src/proxy.rs:211`).

Per-turn latency consists of account queue time, page setup, prompt submission,
model generation, stable-text extraction, and parser completion. Each tool
round adds another page turn and a larger prompt. Validating a complete tool
batch delays tool execution until the batch and browser turn end. Ordinary
text can still stream. One active tab per account serializes concurrent
Claude sessions. No latency figures or tool-adherence rates were measured here.

Measure time to first text, time to first executable call, full turn time,
queue time, and p50/p95 across text and tool-heavy tasks. Record serialized
prompt size and completion failures. Measure actual text separately from
synthetic startup events; the current first-frame metric excludes pings but
can include startup frames (`src/stream_metrics.rs:291`).
Browser token usage, cache hits, dollar cost, and thinking-token counts may
be unavailable. Report estimates as such;
do not invent cache savings or treat Claude Code's `/cost` as a web bill.
ChatGPT Web message limits remain separate from Codex quota.

## Validation and delivery gates for M-4

Local source inspection establishes the integration path. Existing translation
tests cover API tool IDs and event framing, not the web model's obedience.
The following checks should become implementation acceptance gates in TASK-010:

| Layer | Required evidence |
|---|---|
| Fake driver | Chunk-split and Unicode text; one and multiple calls; tool result and `is_error` replay; forced/none choices; unique IDs; invalid names, schemas, or frames; bounded buffers; truncation and cancellation. |
| Messages integration | Exact SSE order and terminal count; equivalent non-streaming JSON; no empty placeholder text on tool-only turns; Anthropic errors; deferred-tool policy; thinking rejection and cross-model history; count probes send no browser message. |
| Shared protocol | Same normalized call/result fixtures through Codex and Claude input/output layers; account rotation between completed turns; no retry after uncertain Send or exposed output. |
| Live Claude Code | Small repository read, approved edit, command with an error result, then a final answer. Show Claude Code executing each call once and sending matching results. Test a denied tool too. |
| Browser operation | Verify Chat mode before every Send, including tool continuations; reject unknown/Work mode; model selection, context overflow, long waits, disconnect, and login expiry. Reuse TASK-006's controlled quota check. |

This session runs existing focused local tests and the report's specified
Verify command; results are recorded in `TASK-009.outcome.md`. No browser or
Claude Code live session is claimed. Live quality and speed remain release
gates, not missing data that prevents this feasibility report.

Documentation surfaces considered: only this `docs/notes/` report is needed
now. README and site translations need no change because no capability or
setup changes. M16 still describes its own unchanged direction. Future
implementation must update setup, supported limits, and provider docs with
their translations. Generated `wiki/` is untouched.

## Verdict

**Feasible with limits. Build a constrained Claude Code path for M-4.**
Use existing `/v1/messages` ingress (`src/server.rs:270`) and model mapping
(`src/routing.rs:296`). Keep tool execution and permission checks in Claude Code.
This assessment supports an implementation spike, not a claim of live support.

TASK-010 should decompose the following work after the M-2 tool protocol exists:

1. Add Anthropic transcript/schema input and Messages/SSE output to the shared
   browser tool protocol. Use the existing call/result ID mapping as a reference
   (`src/model/responses_request.rs:602`, `src/model/responses_request.rs:618`)
   and tool-pause framing (`src/adapters/cursor/sse.rs:348`). Extend browser
   dispatch at `src/proxy/failover.rs:700` through the M-1 adapter.
2. Build strict, bounded tool-frame validation and completion handling. Do not
   copy Cursor's malformed-frame-to-text behavior
   (`src/adapters/cursor/tool_use_xml.rs:133`,
   `src/adapters/cursor/tool_use_xml.rs:152`). Define deferred-tool handling
   using the loaded-schema precedent (`src/model/responses_request.rs:649`).
3. Support text and client tools first, with thinking disabled. Define explicit
   rejection and history rules around signatures
   (`src/model/thinking_signature.rs:39`,
   `src/adapters/anthropic/thinking.rs:30`). Add local token counting
   (`src/proxy/failover.rs:603`) and conservative serialized-prompt bounds
   (`src/count_tokens.rs:38`).
4. Add fake-driver protocol coverage, then the live Claude tool-loop and latency
   checks above. Preserve incremental delivery using the early-start precedent
   (`src/model/responses.rs:300`). Publish support only after these checks pass.

DEC-001 still owns A versus B; DEC-002 owns runtime and naming. If B is not
selected, revisit this protocol design. Stop the Claude path and keep the
provider Codex-only if calls cannot be validated reliably, Chat mode cannot
be enforced, or measured context and latency make the loop unusable. Full
signed Claude thinking and complete Anthropic feature parity are outside this
recommended first build. No new owner decision is needed to deliver this report.

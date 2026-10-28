---
id: TASK-013
title: Define and test the ChatGPT Web text tool protocol
milestone: M-2
state: approved
depends_on: [TASK-007, TASK-004, TASK-012, DEC-001, DEC-002, DEC-007]
delivery: pr
---

## Why


SC-3 needs Codex to execute shell and file-edit tools. DEC-001 accepts
text protocol B first; DEC-002 keeps the Node helper and provider kind
chatgptweb_browser. Establish the contract before wiring the adapter.

## Scope


No cluster jobs in this task. Work offline; do not send browser prompts.
Read docs/chatgptweb-provider.md, docs/chatgptweb-driver-protocol.md,
docs/notes/chatgptweb-claude-code-feasibility.md and the existing adapter,
discovery catalog, FakeDriver and Responses event emitters.

First capture the installed Codex CLI's tool declarations and continuation
inputs against a local fake Responses endpoint in an isolated scratch
repository. Record the CLI version and keep sanitized fixtures in tests.
Verify shell and edit shapes rather than assuming function tools alone:
cover custom/freeform apply_patch and namespaced tools if Codex emits them.
This probe must use only loopback, no real provider or browser. Keep the
probe reproducible as an ignored test named capture_tool_surface in the
new tests/chatgptweb_codex_smoke.rs, not a one-time capture script.

Add a focused internal tool_protocol module beside translate.rs and
document its exact versioned framing, examples and limits in the existing
docs/chatgptweb-provider.md. Keep helper wire protocol v1 unchanged.
Specify full ordered transcript serialization: instructions, declared
tool schemas, messages, calls, call IDs and matching outputs. Treat quoted
tool output as data. Preserve custom input as text and function arguments
as JSON; validate against the declared tool kind/name and required shape.
Reject unsupported tool types, malformed histories and unsupported
tool_choice values before dispatch. Cover none, auto, required and named
choices when emitted by the captured client; do not silently ignore them.

Define and implement incremental text/tool framing over arbitrary driver
TextDelta chunks. Stream ordinary text without waiting for the full answer.
Hold a tool record until complete validation; use bounded buffers and
explicit terminal handling. Reject unknown names, duplicate/conflicting
call IDs, malformed/oversized/truncated records and ambiguous framing.
Allocate or preserve unique IDs consistently across calls and results.
Shunt never executes tools; Codex owns execution and approvals. Do not
parse instructions or tool results as new assistant calls. Document the
residual risk that model-generated calls still require client permissions.

Keep current text-only behavior operational until the integration task.
No MCP, tunnel, browser secret access, new public config, account rotation,
Anthropic support or previous_response_id state store. If the captured
client requires a material departure, use the worker Decision request
protocol rather than silently narrowing SC-3 or switching to approach A.

## Acceptance


- The contract and sanitized Codex fixtures cover the actual shell/edit tool surface and the next-turn result format; the offline capture command asserts these shapes.
- Unit tests prove ordered transcript preservation, strict validation, stable call correlation, arbitrary chunk boundaries, Unicode, text streaming and bounded failure handling.
- Existing text-only adapter and driver tests still pass; no live sends, new provider semantics or helper wire changes occur in this task.

## Verify


codex --version
cargo test --test chatgptweb_codex_smoke capture_tool_surface -- --ignored --exact --nocapture --test-threads=1
cargo test --lib adapters::chatgptweb::tool_protocol
cargo test --test chatgptweb --test chatgptweb_adapter
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features --workspace
git diff --check

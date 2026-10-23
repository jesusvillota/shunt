# TASK-009 outcome

## Acceptance

- [x] `docs/notes/chatgptweb-claude-code-feasibility.md` exists and ends with
  a `Verdict` section. Evidence: the non-empty-file check and specified grep
  command printed `1` and exited 0. The final section begins at line 274;
  no section follows it. Checked at report revision
  `1b243e647d75cc1301bfdd7fcc45018e2d198408`.
- [x] The verdict names what to build for M-4 or the reasons to stop, with
  shunt `file:line` references. Evidence: report lines 276-308 give the
  verdict **feasible with limits**, four concrete build recommendations,
  release checks, and stop conditions. References include Messages ingress
  (`src/server.rs:270`), model mapping (`src/routing.rs:296`), tool IDs
  (`src/model/responses_request.rs:602`), streamed tool handoff
  (`src/adapters/cursor/sse.rs:348`), strict parser gaps
  (`src/adapters/cursor/tool_use_xml.rs:133`), thinking signatures
  (`src/model/thinking_signature.rs:39`), and local token counting
  (`src/proxy/failover.rs:603`). An independent source review approved the
  report's material claims and citations.

## Summary

Delivered a discovery report only. The report checks Claude Code through
shunt's existing Anthropic side. It covers `[[models]]` routing, tool calls
and results, streaming, thinking, deferred tools, repeated context, latency,
and usage estimates. It distinguishes native Antigravity from the deprecated
CLI transport, which cannot return caller tool calls. It also explains why
M16's reverse translation direction is not required for this entry point.

The recommended build is a constrained Anthropic layer over the shared
browser text protocol. Claude Code keeps tool execution and permissions.
The first build should use text and client tools, with thinking disabled.
Strict validation, local token probes, context bounds, and live tool-loop
checks must precede a support claim. No live success is claimed here.

Read TASK-001's report and outcome, the worker protocol, and the existing
DEC-001 and DEC-002 proposals. Neither decision was changed or preempted.
Approach B is assessed conditionally. No new material decision is needed for
this report. TASK-010 remains responsible for M-4 planning; this discovery
task creates no new tasks and dispatches no implementation work.

Source baseline: `142f2434183bcd009d77faf542d61fae96bb3308`.
The branch was rebased onto `origin/main` without conflicts before writing
the report. That base added only a goal dispatch ledger entry; source code
did not change. Verified report revision:
`1b243e647d75cc1301bfdd7fcc45018e2d198408`.
This outcome commit does not change the report or build inputs.

Local verification from this session:

```text
$ test -s docs/notes/chatgptweb-claude-code-feasibility.md && grep -c '^## Verdict' docs/notes/chatgptweb-claude-code-feasibility.md
1
exit 0
$ cargo fmt --all --check
exit 0
$ cargo test --test responses_translate --test antigravity_translate --test passthrough
antigravity_translate: 24 passed; 0 failed
passthrough: 28 passed; 0 failed
responses_translate: 99 passed; 0 failed
exit 0
$ git diff --check
exit 0
```

The 151 tests exercise existing translations, IDs, SSE event order, thinking
round trips, model aliases, and token-count behavior. They do not validate
ChatGPT Web tool adherence. Full all-feature tests and clippy were not run
because this task changes documentation only. No browser, Claude Code live
session, quota experiment, or timing experiment was run. These remain
implementation release checks. GitHub Actions were not inspected or used
as delivery gates.

PR: <https://github.com/jesusvillota/shunt/pull/50>, into `main`, with marker
`Goal-Task: chatgptweb-provider/TASK-009` and label
`goal:chatgptweb-provider`. The report was committed and pushed before the
draft PR opened. This acceptance checklist is committed in the same PR.
The PR is made ready after the outcome is pushed and local Verify passes.
No merge is authorized or performed.

Branch: `goal/chatgptweb-provider/task-009`. Only the report and this outcome
are changed. No task spec, other task, decision, or `goal.yaml` was edited.
No deviations from scope or blockers.

Documentation surfaces considered: the requested English `docs/notes/`
report is updated. README, site, their translations, and existing milestone
specs need no change because this PR changes no supported behavior, config,
or setup. Generated `wiki/` is untouched. Later implementation must publish
its setup and limitations across the maintained documentation surfaces.

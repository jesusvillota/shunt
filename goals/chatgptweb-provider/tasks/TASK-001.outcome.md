# TASK-001 outcome

## Acceptance

- [x] `docs/notes/chatgptweb-survey.md` exists with all five required sections.
  Evidence: TASK-001 Verify printed `5` and exited 0. The sections start at
  lines 18, 70, 151, 180, and 226 of the report at revision
  `eb636ee1344cbd49e2cd28f63fee3b8c1ed2b0de`.
- [x] `Chat vs Work mode` explains how to read the active mode and compare
  Codex quota. Evidence: report lines 70-149 quote the fetched official
  Chat/Work switcher documentation and shared Work/Codex limits. They define
  selected-state checks before every Send, with unknown state rejected.
  A successful live `goal pool --goal chatgptweb-provider --json` read exposed
  the `codex` account fields `utilization_5h`, `reset_5h`, `utilization_7d`,
  and `reset_7d`. The report requires fresh before/after usage, account matching,
  unchanged windows, and no concurrent consumption. It states the meter's
  precision limit. No live mode or unchanged-quota result is claimed.
- [x] `Shunt seams` lists the required file-level extension points.
  Evidence: report lines 151-178 cover `ProviderKind`, adapter dispatch,
  account and provider `profile_dir`, model discovery, child lifecycle,
  shutdown, account rotation, and route failover, with `file:line` references.
  Source inspection also established the Codex endpoint's Responses-only
  validation and deliberately empty Codex model catalog.
- [x] `Tool-call approaches` compares A and B and states what shunt needs.
  Evidence: report lines 180-224 compare setup, transport, client tool
  execution, state, account rotation, and unmeasured quality. Pinned upstream
  source documents A's broker and continuation loop. Local parser and
  Responses translation references identify building blocks for B. B is
  explicitly new work, with no reliability claim or decision made.
- [x] License reuse notes and noncolliding provider-name options are included.
  Evidence: report lines 256-282 cite the upstream MIT license, notice
  requirements, separate tunnel-client Apache-2.0 license, bundled runtime
  notices, and the goal tool's existing `chatgptweb` worker platform.
  Options are `chatgpt_browser`, `chatgpt_web_driver`, and `chatgpt_web`.
  DEC-002 retains the final choice.

## Summary

Delivered the survey report only. No production code, task specs, decisions,
or `goal.yaml` changed. The branch remains
`goal/chatgptweb-provider/task-001`.

The upstream source was inspected at
`0901b9fb1976b4a984c7e9b4082c2868767bbf7c` (version `6.1.13`). Shunt seams were
checked at `fa430c0c1c37bece9bfac29148eac46eedb76721`. The branch was rebased
onto `origin/main` without conflicts before the report commit. No source
files changed between the initial worktree revision and that base.

Verified report revision: `eb636ee1344cbd49e2cd28f63fee3b8c1ed2b0de`.
This outcome commit adds the acceptance evidence; it does not alter the report.
An independent source review found no material upstream factual or link errors.

Local validation:

```text
$ test -s docs/notes/chatgptweb-survey.md && grep -c -E '^## (Driver mechanics|Chat vs Work mode|Shunt seams|Tool-call approaches|Risks)' docs/notes/chatgptweb-survey.md
5
exit 0
$ git diff --check
exit 0
$ cargo fmt --all --check
exit 0
```

Rust tests and clippy were not run: this PR changes documentation only and
does not change Rust code or build inputs. GitHub Actions were not inspected
or used as delivery gates.

PR: <https://github.com/jesusvillota/shunt/pull/49>, base `main`, with marker
`Goal-Task: chatgptweb-provider/TASK-001` and label `goal:chatgptweb-provider`.
The report commit was pushed and a draft PR was opened before this outcome.
The PR is marked ready after local Verify and this committed checklist.

Limits and follow-ups: no Chrome window was available, so no live Chat/Work
selector was observed. No browser request or before/after quota experiment was
run. The official switcher documentation supports the proposed read method;
TASK-003 must validate actual selected-state fixtures and TASK-006 must run
the controlled quota test. These are existing later tasks, not missing inputs
for this survey. DEC-001 and DEC-002 remain responsible for their existing
tool, runtime, and naming decisions. No new decision request is needed.

Documentation surfaces considered: the requested `docs/notes/` report changes.
README, site, their translations, and milestone specs need no update because
this PR introduces no supported capability or setup change. Generated `wiki/`
was not edited. No deviations from task scope.

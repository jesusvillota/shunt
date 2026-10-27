# TASK-007 outcome

## Acceptance

- [x] `proposals/TASK-007.yaml` passes `goal proposal check`.
  From the worktree root, the session ran
  `goal proposal check goals/chatgptweb-provider/proposals/TASK-007.yaml --goal chatgptweb-provider`.
  It exited 0 and printed `Proposal from TASK-007 is valid`.
  The explicit selector is required because this repository has three goals.
  The shorthand without `--goal` exits 1 with `several goals ...; pass --goal SLUG`.
  The successful command is the fully qualified Verify requested for this task.
- [x] Every proposed task has command-based Verify, including a fake demo and live smoke.
  A Ruby YAML inspection checked all three Verify blocks and found 8, 13 and 5 commands.
  It asserted the exact demo/live test invocations, the live task's `kind: run`,
  its integration dependency, and both later milestone links. It exited 0:

  ```text
  tool-protocol: 8 Verify commands
  tool-integration: 13 Verify commands
  tool-live-smoke: 5 Verify commands
  PASS: command-only Verify for every task; fake demo and bounded live smoke; later milestones gated
  ```

## Summary

The proposal decomposes M-2 into three independently reviewable results:

1. Capture the installed Codex tool surface offline; implement and test the text protocol contract.
2. Integrate Responses tool events, catalog support and a real-Codex fake demo; build the guarded live harness.
3. Run the merged harness against one signed-in account and prove shell execution, file editing and tool-result replay.

The contract keeps driver protocol v1. It covers custom edit tools when the
installed Codex emits them, full transcripts, strict parsing and streaming.
Codex keeps tool execution and permissions. The adapter does not execute tools.
The test harness uses the existing helper override to attach to owner Chrome.
It adds no public configuration key. The live run has six-send and 15-minute
limits, a non-sending health check, no automatic retries and browser preservation.

TASK-008 and TASK-010 gain dependencies on the live result. Their existing
dependencies remain. The proposal adds no decision node, cancels no task and
does not depend on cancelled TASK-006. The owner's waiver is recorded in the
existing goal ledger and TASK-006 state. TASK-012 records one live guarded
answer; it does not replace the future end-to-end tool smoke.

Read the task, worker protocol, owner rules, accepted DEC-001/DEC-002,
TASK-004/TASK-009/TASK-012 outcomes, and current code. A read-only research
agent checked the implementation seams and reviewed the final proposal.
It found no material gaps. The plan uses a three-task chain because each
stage needs the preceding contract or executable adapter. There is one real
run, one account and one scratch repository; sharding is unnecessary.
The fake long-transcript checks precede live execution. Login, profile access,
client/runtime availability and exclusive browser use are explicit prerequisites.

Only the proposal and this outcome change. No actual task, decision, goal
metadata, rules or generated graph was edited. No structural goal commands,
proposal apply, dispatch or merge were run. The specific planning-worker rule
reserves application for the orchestrator; proposed refs are not created tasks.
After review, the orchestrator can apply the proposal and dispatch the resulting
approved tasks under the owner's execution rule.

Verified base: `a761ae533899138e01e22074029014d237a78d8d`.
Verified proposal Git blob: `53d2bd65b3a8a9a8f03d12743ec86187fcadd006`.
The branch rebased onto `origin/main` without conflicts and remains
`goal/chatgptweb-provider/task-007`.

Local verification in this session:

```text
$ goal proposal check goals/chatgptweb-provider/proposals/TASK-007.yaml --goal chatgptweb-provider
Proposal from TASK-007 is valid
exit 0
$ cargo fmt --all --check
exit 0
$ git diff --check
exit 0
```

README, engineering docs, site translations and wiki were considered. This PR
changes planning only; implementation tasks carry the affected documentation
updates. No runtime capability is claimed here. Full Rust tests, clippy and
live browser sends were not run for this planning-only change. The proposed
future test targets are deliverables, not tests claimed to pass today.
GitHub Actions were not inspected or used as delivery gates.

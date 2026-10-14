# Fix the Antigravity pool and quota tracking

Goal `agy-pool` · format `goal/v2`. Live status is derived from GitHub and git and is
never written here: run `goal status agy-pool` or `goal tui`.

Make the shunt antigravity pool serve traffic across all configured accounts and track each account's quota.

## Context, scope and non-goals

<what exists today, what is included, what is explicitly out of scope>

## Risks and open questions

<risks, assumptions, and unknowns; material unknowns become discovery tasks or DEC nodes>

## Plan structure

<!-- goal:graph:start -->
```mermaid
flowchart TD
  subgraph M_1["M-1 · Working agy pool with quota"]
    DEC_001{{"DEC-001 Choose the agy quota source"}}
    DEC_002{{"DEC-002 Approve optional per-account profile_dir and upstream-reported-window acceptance with separate captured-weekly and four-window schema tests?"}}
    DEC_003{{"DEC-003 Should TASK-003 include fixes for the known macOS streaming-test failures, or wait for a separate fix?"}}
    TASK_001["TASK-001 Diagnose why the agy pool fails and quota is missing"]
    TASK_002["TASK-002 Read agy quota via the agy /usage command per account"]
    TASK_003["TASK-003 Make the agy pool rotate and fail over across all accounts"]
    TASK_004["TASK-004 Quota-aware account selection and live end-to-end smoke"]
    TASK_005["TASK-005 Fix macOS streaming test failures"]
    TASK_006["TASK-006 Show Antigravity quota buckets in shunt tui and goal pool"]
    TASK_007["TASK-007 Add shunt pool CLI command"]
    TASK_008["TASK-008 Remove the shunt top alias; only shunt tui remains"]
    TASK_009["TASK-009 Switch goal pool to call shunt pool --json"]
    TASK_010["TASK-010 Live failover and ranking test of the agy pool"]
  end
  TASK_001 --> DEC_001
  DEC_001 --> TASK_002
  DEC_002 --> TASK_002
  TASK_001 --> TASK_003
  DEC_003 --> TASK_003
  TASK_002 --> TASK_004
  TASK_003 --> TASK_004
  TASK_005 --> TASK_004
  TASK_006 --> TASK_007
  TASK_007 --> TASK_008
  TASK_007 --> TASK_009
  TASK_007 --> TASK_010
  classDef draft stroke-dasharray: 4 3
  classDef cancelled opacity:0.45
  classDef decision fill:#f3e8ff,stroke:#7c3aed
  class DEC_001 decision
  class DEC_002 decision
  class DEC_003 decision
```
<!-- goal:graph:end -->

## Worker protocol

For an agent executing exactly one task of this goal:

1. Work only on your task, in your own branch and worktree, branched from `origin/main`.
2. Read `tasks/TASK-NNN.md` (the spec), the outcomes of the tasks it depends on, and the
   accepted decisions it depends on. Never edit specs, other tasks, decisions or `goal.yaml`.
3. After your first commit, open a draft PR into `main` whose body contains the line
   `Goal-Task: agy-pool/TASK-NNN`. Write the body with real newlines.
   Use `gh pr create --body-file FILE` or `gh pr edit --body-file FILE`.
4. Do the work and run the spec's Verify commands.
5. Write `tasks/TASK-NNN.outcome.md` with an `## Acceptance` checklist (each item followed by
   its evidence) and a `## Summary` (what, how, deviations, verified revision, proposed
   follow-ups). Commit it in the same PR.
6. Mark the PR ready for review when checks pass or none are configured.
   Failing or pending checks block delivery. Never merge.
7. Blocked by a missing decision, access or data? Explain it in the outcome, add the line
   `Goal-Blocked: <reason>` to the draft PR body, and stop. For a material decision, also
   write `## Decision request` in the outcome with the question, options and trade-offs,
   and your recommendation under `### Recommendation` (omit it if you have none). Add
   `Goal-Decision: <one-line question>` to the draft PR body, commit and push, then stop
   through exit (B). Refresh or `goal sync` creates a decision in the Inbox. Under
   `decide: experts`, the committee advises. Under `orchestrate: agent`, the
   orchestrator accepts or rejects it; otherwise the human does. After acceptance,
   the Resume task action in `:` resumes the task with the decision in its resume note.

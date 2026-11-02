# Per-goal account pinning

Goal `account-pinning` · format `goal/v2`. Live status is derived from GitHub and git and is
never written here: run `goal status --goal account-pinning` or `goal tui --goal account-pinning`.

Let each goal draw only from a configured subset of a platform's accounts (overlapping goals share accounts; goals without an entry use all), carried by a per-goal shunt client token that the goal CLI hands to each session, with the keeper waking on the goal's own subset reset.

## Context, scope and non-goals

**Today.** shunt pools several accounts per platform (for example Claude accounts A, B, C) and every request draws from all of them. The owner's gateway runs without `[server.auth]`, so shunt cannot tell one goal from another. The `goal` CLI sets no gateway token or header on the sessions it starts.

**Design (agreed with the owner on 2026-11-02).**

- *Identity.* Each pinned goal gets its own shunt client token (`[server.auth]`, `SHUNT_CLIENT_TOKENS="<slug>:<token>,…"`). The client name is the goal slug. Claude Code sessions send it as `ANTHROPIC_CUSTOM_HEADERS="x-shunt-token: <token>"`. That header wins over the `apiKeyHelper` Bearer token, so ordinary sessions keep using the owner's token, registered as client `default`. Codex sessions send it through `env_http_headers`. The token lives in the macOS Keychain as `shunt-goal-<slug>`.
- *Config.* `[server.auth.clients.<slug>]` with `accounts.<provider> = [names]`. A goal with no entry uses all accounts. One table covers every pooled platform (Claude, Kimi, Codex/ChatGPT, Antigravity, the inbound Codex endpoint).
- *Selection.* Adapters filter the order from `AccountPool::select_order*` to the subset. Health is keyed per account, so overlapping goals (X and Z both on A) share A's quota and cooldown state.
- *Exhausted subset.* Fail, never spill over: shunt relays the upstream 429 with `Retry-After`, and the goal keeper sleeps until the goal's own subset resets (`shunt pool --json --client <slug>`). An operator-configured `[[routes]]` fallback to another provider still applies.
- *Trust boundary.* The shunt process, its config and its token env var. Clients present only a secret; forged `x-shunt-*` headers are stripped on entry. All agents run as one macOS user and can read the Keychain, so this guards against mistakes, not a hostile agent.

**In scope.** shunt config, validation, selection filter, router side calls, `shunt pool --client`, docs in README/docs/site (4 languages); goal CLI token hand-off and keeper subset reads; owner setup on this Mac.

**Out of scope.** Changes to Claude Code or Codex; spill-over to other accounts; per-OS-user isolation; credential writeback; `wiki/` (generated).

## Risks and open questions

- *Turning on `[server.auth]` makes every credential-injecting request need a token.* Mitigated by registering the `apiKeyHelper` token as `default` (TASK-007) and by fixing goal's agy discovery, which sends `x-api-key: local` (TASK-004).
- *Cold-start skew.* The round-robin start position is computed over all accounts, so before quota headers arrive, a goal on A/B can hit A about twice as often as B. Accepted; headroom ordering takes over once quota signals exist.
- *Token leakage through launch commands.* TASK-004 must resolve the token in the launched shell, never write it into command text, logs or the clipboard.
- *Older shunt binary.* TASK-005 falls back to `shunt pool --json` without `--client` if the flag is unknown.
- *Assumption.* The client name equals the goal slug; Keychain service `shunt-goal-<slug>`.

Model routing (owner's choice): the critical shunt tasks (TASK-001 core feature, TASK-002 security) run on `claude:opus:high`; the others run on `codex:gpt-6.1-sol:xhigh`.

## Plan structure

<!-- goal:graph:start -->
```mermaid
flowchart TD
  subgraph M_1["M-1 · Per-goal account pinning end to end"]
    DEC_001{{"DEC-001 May TASK-005 include inherited rustfmt-only changes outside its four task files?"}}
    TASK_001["TASK-001 Pin clients to account subsets: config, validation and filter on every pooled adapter"]
    TASK_002["TASK-002 Keep the client's account subset on router side calls and block forged scope headers"]
    TASK_003["TASK-003 shunt pool --client: show only a client's accounts"]
    TASK_004["TASK-004 Give each goal session its goal's shunt client token"]
    TASK_005["TASK-005 Keeper reads the goal's own account subset for rate-limit resets"]
    TASK_006["TASK-006 Gate: Codex end-to-end test and run-shunt smoke test per goal"]
    TASK_007["TASK-007 Turn on per-goal pinning on this Mac"]
    TASK_008["TASK-008 shunt reads client tokens from a file on reload"]
  end
  TASK_001 --> TASK_002
  TASK_001 --> TASK_003
  TASK_003 --> TASK_005
  DEC_001 --> TASK_005
  TASK_002 --> TASK_006
  TASK_003 --> TASK_006
  TASK_004 --> TASK_007
  TASK_005 --> TASK_007
  TASK_006 --> TASK_007
  classDef draft stroke-dasharray: 4 3
  classDef cancelled opacity:0.45
  classDef decision fill:#f3e8ff,stroke:#7c3aed
  class DEC_001 decision
```
<!-- goal:graph:end -->

## Worker protocol

For an agent executing exactly one task of this goal:

1. Work only on your task, in your own branch and worktree, branched from `origin/main`.
2. Read `tasks/TASK-NNN.md` (the spec), the outcomes of the tasks it depends on, and the
   accepted decisions it depends on. Never edit specs, other tasks, decisions or `goal.yaml`.
3. After your first commit, open a draft PR into `main` whose body contains the line
   `Goal-Task: account-pinning/TASK-NNN`. Write the body with real newlines.
   Use `gh pr create --body-file FILE` or `gh pr edit --body-file FILE`.
   If the repo has several remotes, resolve `origin` with `git remote get-url origin`.
   For a GitHub origin, use `gh pr create --repo <origin OWNER/NAME> --body-file FILE`.
   Pass the same `--repo <origin OWNER/NAME>` to every other `gh pr` command.
4. Do the work and run the spec's Verify commands.
5. Write `tasks/TASK-NNN.outcome.md` with an `## Acceptance` checklist (each item followed by
   its evidence) and a `## Summary` (what, how, deviations, verified revision, proposed
   follow-ups). Commit it in the same PR.
6. Mark the PR ready for review when the spec's Verify commands pass.
   GitHub checks never gate delivery; local Verify decides. Never merge.
   Planning tasks write `proposals/TASK-NNN.yaml`; see goal-setup `references/format.md`, Plan proposals.
7. Waiting only on submitted jobs or detached runs that start or end without owner action?
   Commit and push, keep the PR a draft, add exactly one line
   `Goal-Waiting: until <UTC time>; <job ids and reason>` to its body, and stop through exit (C).
   Use a UTC time such as `2026-10-30T18:00Z`: the latest expected finish, including queue time.
   Name every job id and its queue or host. Start the final message with `WAITING: <reason>`.
   The task stays running. The orchestrator resumes you after that time to inspect the results.
   On resume, continue the task; remove `Goal-Waiting:` before exit (A), or renew it for another wait.
   Blocked by a missing decision, access or data? Explain it in the outcome, add the line
   `Goal-Blocked: <reason>` to the draft PR body, and stop. For a material decision, also
   write `## Decision request` in the outcome with the question, options and trade-offs,
   and your recommendation under `### Recommendation` (omit it if you have none). Add
   `Goal-Decision: <one-line question>` to the draft PR body, commit and push, then stop
   through exit (B). Refresh or `goal sync` creates a decision in the Inbox. Under
   `decide: experts`, the committee gives opinions. A copilot or orchestrator shell records
   the final call with `goal decide`; the ledger names `experts` as the decider.
   The owner can accept or reject any open decision as the human. After acceptance,
   the Resume task action in `:` resumes the task with the decision in its resume note.
8. A task with `repo: NAME` works in that repository's clone, branched from its base (the
   dispatch condition names it). This README and the spec stay here; read them, never copy
   them. Skip the outcome file: put the step 5 `## Acceptance` checklist and `## Summary`,
   and any step 7 explanation or `## Decision request`, in the PR body. `goal merge` saves
   that body as `tasks/TASK-NNN.outcome.md`.

## Acceptance

- [ ] Unit tests parse a captured `/usage` JSON fixture into four grouped buckets.
  Evidence: the live `agy` 1.3.4 capture below contains two weekly buckets only.
  No captured four-bucket `/usage` fixture was found in the task evidence.
  No parser or test was added while the decision below remains unresolved.
- [ ] A reply with `num_turns > 0` latches the probe off and spends no further calls.
  Evidence: no runner was implemented. The live read-only probe returned
  `num_turns: 0` and all token counts were zero; this does not prove the latch.
- [ ] A live gateway shows 5h and weekly quota for every configured Antigravity account.
  Evidence: `goal pool` exited 0 against the running gateway on
  `http://127.0.0.1:3001`. All six Antigravity rows had unknown 5h and 7d values.
  Authenticated `GET /admin/api/pool` returned HTTP 200, with `has_state: false`
  and no `quota_buckets` for each of the six accounts. The live `/usage` reply
  supplies weekly quota only. See the exact output below.
- [ ] Format, all-feature clippy, and all-feature workspace tests pass.
  Evidence: `cargo fmt --all --check` and
  `cargo clippy --all-targets --all-features -- -D warnings` passed. The full
  chained command exited 101 in `cargo test --all-features --workspace`:
  `streaming_finishes_when_a_descendant_holds_stdout_open` timed out at
  `tests/antigravity_process.rs:324`. Library tests: 3073 passed, 2 ignored;
  CLI unit tests: 43 passed. Tests after this failing integration target were
  not reached. The exact failure tail is below.

## Summary

Status: **blocked before implementation**. DEC-001 is accepted and selects
`agy /usage` as the primary source, with the existing HTTP path as fallback.
That choice is preserved. Two missing prerequisites prevent safe implementation
and the specified live acceptance: an approved account-to-CLI-profile mapping,
and real 5h quota data (including a captured four-bucket fixture).

Only this task's outcome is changed. No task spec, decision, goal configuration,
production code, credential, or live gateway configuration was edited.
The existing gateway was inspected; no scratch gateway was started.

Verified source revision: `f744eef33950729c2e2f19becc219d857cad29ee`.
Session evidence date: 2026-10-14. Branch: `goal/agy-pool/task-002`.
Rebased without conflicts onto `origin/main` at
`622fe71633e8a00c49e575b1042b40aa5a515168`; checks ran at `afbcb36`.
The upstream coordinator has created `DEC-002` from this request; it remains
`proposed`, so this handoff still requires a decision. No upstream goal file
was edited by this worker.

Documentation surfaces considered: README, `docs/`, and site translations need
no implementation update because behavior has not changed. `wiki/` is generated
and was not edited. Implementation must update the surfaces named in TASK-002
after the decision is resolved. No unrelated bug fixes are proposed here.

## Decision request

Question: Approve optional per-account `profile_dir` and verification of only
upstream-reported windows, with separate captured-weekly and four-window schema tests?

### Evidence and boundaries

1. `src/config.rs:1984` defines `AccountConfig` with `deny_unknown_fields` and
   no `profile_dir`. The six live Antigravity account entries contain names only:
   `computingjvm`, `keepuntouser`, `tommyshelby`, `computing-jvm`, `personal`, `cemfi`.
2. `src/config.rs:1882` defines `profile_dir` on `ProviderConfig`, documented
   for `kind = "antigravity_cli"` only. The live provider is native
   `kind = "antigravity"`, `auth = "antigravity_oauth"`, without `profile_dir`.
   One provider-wide CLI home cannot identify six accounts independently.
3. TASK-001's outcome states that only the personal account has a CLI login;
   the other five profiles were not available. Native HTTP credential files
   do not authorize assuming a matching CLI profile or copying their secrets.
4. Root `AGENTS.md` requires approval before changing public config keys or
   documented provider semantics. Adding account `profile_dir` is such a change.
   DEC-001 approves the source choice but does not define profile mapping.
5. The live command below returned only `gemini-weekly` and `3p-weekly`.
   TASK-001 also records that HTTP returns weekly windows only. Synthesizing
   5h values or calling a generated fixture a captured reply would be false evidence.
6. The live config has an empty `[server.pool]`; polling remains disabled.
   A future smoke test can enable `usage_refresh_seconds = 60` in a scratch
   config. That enables observation but cannot create missing upstream 5h data.

### Options and trade-offs

- **A. Approve optional account `profile_dir`; accept the windows actually
  returned.** Use explicit per-account CLI homes when available, gate the CLI
  on version >= 1.1.11, latch it off on `num_turns > 0`, and use HTTP fallback
  when the CLI home is absent or unusable. Test the real two-bucket capture and
  a clearly identified representative four-bucket schema fixture separately.
  Check live grouped weekly quota for all six accounts through the admin API;
  show 5h only when reported. This preserves DEC-001 and honest observations,
  but needs approval for the config extension and acceptance interpretation.
- **B. Keep the current acceptance exactly.** Supply an approved per-account
  profile mapping, authenticated profiles, and an authentic four-window capture
  from an account/plan that reports 5h and weekly quota. The live requirement
  still needs 5h data for every configured account. This preserves the literal
  acceptance but depends on upstream data not currently available.
- **C. Reuse provider-wide `profile_dir` with one provider per account.** This
  avoids adding an account field but changes documented native-provider
  semantics and the deployment topology. It still cannot produce missing 5h
  buckets and adds unnecessary provider configuration for this pool.

### Recommendation

Choose A. Make `profile_dir` optional on account entries and never use an
ambient CLI login for a different managed account. Preserve DEC-001's HTTP
fallback for accounts without CLI profiles. Verify the native grouped buckets
through `GET /admin/api/pool`, since `goal pool` currently renders generic
utilization fields and omits Antigravity `quota_buckets`.

After acceptance, implement the parser, bounded subprocess runner, persistent
latch, poller integration, focused tests, and matching English/translated docs.
Run the full checks and a scratch live gateway with polling enabled.

## Live evidence

`agy --version` exited 0: `1.3.4`.

`agy -p /usage --output-format json --print-timeout 20s` exited 0.
The captured JSON quota and accounting fields were:

```json
{
  "status": "SUCCESS",
  "duration_seconds": 0,
  "num_turns": 0,
  "usage": {
    "input_tokens": 0,
    "output_tokens": 0,
    "thinking_tokens": 0,
    "cache_read_tokens": 0,
    "total_tokens": 0
  },
  "command": {
    "name": "usage",
    "data": {
      "groups": [
        {
          "name": "Gemini Models",
          "buckets": [{
            "id": "gemini-weekly",
            "window": "weekly",
            "remaining_fraction": 0,
            "reset_time": "2026-10-20T17:09:34Z"
          }]
        },
        {
          "name": "Claude and GPT models",
          "buckets": [{
            "id": "3p-weekly",
            "window": "weekly",
            "remaining_fraction": 0.8474119901657104,
            "reset_time": "2026-10-20T23:53:16Z"
          }]
        }
      ]
    }
  }
}
```

Only descriptive text, display names, the empty conversation id, and the
redundant rendered `response` were omitted above. No quota bucket was omitted.

`goal pool` exited 0 with the live gateway running. Its Antigravity output was:

```text
antigravity (antigravity_oauth)
  computingjvm   ok                     5h — (reset —) · 7d — (reset —) · cooldown —
  keepuntouser   ok                     5h — (reset —) · 7d — (reset —) · cooldown —
  tommyshelby    ok                     5h — (reset —) · 7d — (reset —) · cooldown —
  computing-jvm  ok                     5h — (reset —) · 7d — (reset —) · cooldown —
  personal       ok                     5h — (reset —) · 7d — (reset —) · cooldown —
  cemfi          ok                     5h — (reset —) · 7d — (reset —) · cooldown —
```

## Local verification

The first full command failed because `ui/dist` was absent. Built the documented
prerequisite with `npm ci && npm run build` in `ui/` (exit 0), then reran:

```sh
cargo fmt --all --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all-features --workspace
```

Format and clippy passed. The command exited **101**, with this final tail:

```text
---- streaming_finishes_when_a_descendant_holds_stdout_open stdout ----

thread 'streaming_finishes_when_a_descendant_holds_stdout_open' (73378360) panicked at tests/antigravity_process.rs:324:6:
the turn must finish within the guard rather than hang: Elapsed(())
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

failures:
    streaming_finishes_when_a_descendant_holds_stdout_open

test result: FAILED. 11 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 20.03s

error: test failed, to rerun pass `--test antigravity_process`
```

This branch changes only the outcome. No process/streaming test was weakened or
changed. Generated UI assets and dependencies are ignored and are not committed.
GitHub Actions were not inspected or run, per the workspace instructions.

The isolated rerun passed (exit 0):
`cargo test --all-features --test antigravity_process streaming_finishes_when_a_descendant_holds_stdout_open -- --exact`.
Result: 1 passed, 0 failed, 11 filtered out, in 0.38s. This does not change the
failed full-suite result. An `agent-note bug` records the timing-sensitive
failure outside this task's scope, after checking for a duplicate.

Goal-Task: account-pinning/TASK-010

## Summary

Policy now lists live shunt accounts by provider. Space stages a toggle; Enter on **save accounts** applies it through `pin_accounts`. The bracketed mark shows the saved state, arrows show pending changes, and the footer shows the accounts currently used. Opening Policy, `r`, and confirmed refresh reload the pool and saved pins. Reads and writes run in the background. Unavailable shunt disables account editing. All-on removes pins, including stale provider pins; an all-off provider is refused before writing.

Added isolated TUI tests with fake shunt/security executables and a temporary `shunt.toml`. Tests record the selection at the `pin_accounts` boundary, call the real writer, and assert the written config. Policy scrolls account rows into view on short terminals. Updated the TUI README and existing Policy test expectations.

Scope: only `tools/goal-tui`; no task specs, goal metadata, or other tabs changed. No design deviations or proposed follow-ups. Verified revision: `2eae44b`. Rebase onto `origin/main` required no conflicts. Both commits are pushed.

## Acceptance

- [x] **1. A TUI test lists accounts per provider from fake `shunt pool --json`.** `account_tests::policy_accounts_list_and_toggle_c_save_through_pin_accounts` passed. It renders anthropic A/B/C and codex work/personal, plus the initial unpinned footer. The fake shunt rejects arguments other than `pool --json`.
- [x] **2. Toggle C off and save through `pin_accounts`, asserted at the writer boundary.** The same passing test records the exact call for goal `demo`: `anthropic=[A,B]`, `codex=[]` (remove that provider's pin). Its wrapper calls real `pin_accounts`; `current_selection` and the temporary config contain only `accounts.anthropic = ["A", "B"]`. Before saving, config is unchanged and the saved `[on]` mark shows an unsaved off arrow. After saving, C is `[off]` and the footer shows A/B.
- [x] **3. All-on clears the pin; all-off is refused.** `account_tests::policy_accounts_all_on_clears_every_pin_and_all_off_refuses_writer` passed. All-on passes empty removal lists for anthropic, codex, and a retired provider to the real writer, leaves no goal pin, and restores the original config exactly. All-off shows `keep at least one account on for anthropic`, makes no additional writer call, and leaves config unchanged. `policy_accounts_refresh_reopen_and_unreachable_disable_section` also passed: refresh and reopening discover new accounts, a short terminal scrolls them into view, and unreachable shunt shows one error line and disables the section.
- [x] **4. Both crates pass the required local verification.** The exact combined command below exited 0 at `2eae44b`. TUI: 78 passed, 0 failed. The full `goal-rust` workspace, including the TUI and pin writer tests, passed. GitHub checks were not used as a gate.

```sh
cd tools/goal-tui && cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && RUST_TEST_THREADS=2 cargo test
cd ../goal-rust && cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && RUST_TEST_THREADS=2 cargo test --workspace
```

Verification tail:

```text
   Doc-tests goal_core

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests goal_tui

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
exit_code=0
```

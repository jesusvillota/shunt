---
id: TASK-010
title: 'Policy tab: toggle shunt accounts per goal'
milestone: M-1
state: approved
depends_on: [TASK-009]
delivery: pr
repo: goal
route: {platform: codex, model: gpt-6.1-sol, effort: high}
---

## Why

Serves SC-1 and SC-6, and is the owner's goal for this feature: in the goal TUI's Policy tab (key `3`), see the live accounts per platform from shunt and switch each one on or off for this goal.

## Scope

Work in the `goal` repo, `tools/goal-tui` (Policy tab in `src/ui.rs`, keys in `src/keymap.rs`, test helpers in `tests/support/`). Uses TASK-009's `available_accounts`, `current_selection` and `pin_accounts`.

1. In the Policy tab add a "shunt accounts" section. For each provider from `shunt pool --json` it lists the accounts with an on/off mark. Space toggles the highlighted account; the mark shows the saved state.
2. Opening the tab, and a refresh key, re-reads the live account list, so a newly added account appears. If shunt is not reachable, show one line saying so and disable the section.
3. Saving calls `pin_accounts`. With every account of every provider on, it clears the pin (the goal is unpinned). A provider with all accounts off is refused with a one-line message.
4. The footer shows which accounts the goal currently uses, so the owner sees the effect.
5. Tests in the existing TUI test style with a fake shunt: the list renders, toggles save the right selection, all-on clears the pin, unreachable shunt disables the section.
6. Docs: the TUI Policy tab description in the goal repo README or TUI README.

Out of scope: shunt changes; changes to other tabs.

## Acceptance

- A TUI test shows the Policy tab listing accounts per provider from a fake `shunt pool --json`.
- Toggling account C off and saving writes the expected selection through `pin_accounts` (asserted at the writer boundary).
- All accounts on clears the pin; an all-off provider is refused.
- `cd tools/goal-rust && cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace` and the same in `tools/goal-tui` pass.

## Verify

```sh
cd tools/goal-tui && cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && RUST_TEST_THREADS=2 cargo test
cd ../goal-rust && cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && RUST_TEST_THREADS=2 cargo test --workspace
```

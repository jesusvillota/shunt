---
id: TASK-011
title: Redesign the Antigravity section of shunt tui as aligned quota columns
milestone: M-1
state: approved
depends_on: [TASK-008]
delivery: pr
---

## Why

TASK-006 made Antigravity quota visible in `shunt tui`, but as three text lines per account (`Quota remaining ↓`, one line per family). It does not line up with the Anthropic and Codex rows (one row per account, `5h limit` / `7d limit` columns with a usage bar, a percent, a reset time, and a State column), so it is hard to scan. The TUI should give one organised, comparable view per platform.

## Scope

Redesign how `src/tui/view.rs` renders the Antigravity section (and `src/tui/model.rs` if needed). Keep Anthropic and Codex rows unchanged.

Target look, one row per account, same row height as the other providers:

```
▾ antigravity  ON  ranking: balanced · 5 accounts
  # Account        Gemini · weekly              Claude + GPT · weekly        State
  1 computingjvm   ██████████ 100% used · 6d 8h  ░░░░░░░░░░   0% used · 6d 15h  available
  2 keepuntouser   ░░░░░░░░░░   0% used · 6d 15h ░░░░░░░░░░   0% used · 6d 15h  available
```

- The Antigravity section header row replaces the global `5h limit` / `7d limit` column titles with one title per reported quota family (take titles from the bucket labels, shortened: `Gemini · weekly`, `Claude + GPT · weekly`). The two quota columns reuse the existing 5h/7d column x-positions and widths, so all providers stay aligned.
- Each cell reuses the existing bar widget, percent, reset countdown and colour thresholds used by the 7d column. Use the same semantic as the other providers: the bar and percent show USED (`used = 1 - remaining`), colour by used (red when exhausted). Do not mix "left" and "used" in one screen.
- An exhausted family (0% remaining, i.e. 100% used) is clearly red; a missing or unknown reading shows a dim `—`, never 0%.
- Show only windows upstream reports (DEC-002). If an account reports a 5h bucket, show it as an extra labelled cell only if width allows; otherwise skip it. Never invent values.
- State column: keep the existing State text; mark an account that is exhausted in EVERY family as `exhausted` (or the existing near-quota style) so it stands out.
- Narrow terminals: below the width needed for both quota columns, degrade gracefully (shorter titles, then one column) without wrapping or breaking the row alignment, scroll counts or mouse hit targets.
- Remove the old three-line rendering and any code it no longer needs.

Out of scope: other providers, `shunt pool` text output (TASK-007), selection logic, config.

## Acceptance

- View tests (ratatui TestBackend or the existing view test harness) render the section for: all families with quota, one exhausted family, all exhausted, an account with unknown quota, a narrow width. Paste the rendered text frames in the outcome.
- Existing TUI tests still pass; none weakened or removed except those that asserted the removed three-line layout (state each replacement).
- Column alignment: assert that the quota columns start at the same x as the 5h/7d columns of other providers in one shared frame.
- `docs/pool-tui.md` and the pool-account-controls guide (en, ko, ja, zh-cn) describe the new layout. Take any cross-page anchors from the built site.
- `cargo fmt --all --check`, `cargo clippy --all-targets --all-features -- -D warnings` and `cargo test --all-features --workspace` pass.
- The owner reviews the look live: after merge, `cargo build --release --features tui` and `shunt tui`.

## Verify

`cargo fmt --all --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all-features --workspace`

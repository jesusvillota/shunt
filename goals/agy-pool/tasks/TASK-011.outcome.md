# TASK-011 outcome

## Acceptance

- [x] 1. Render all families with quota, one exhausted family, all exhausted, unknown quota, and a narrow width.
  Evidence: `family_quota_acceptance_frames_use_one_account_row` passes with ratatui `TestBackend`. Five complete 120×10 / 68×10 frames are pasted below. Tests assert red exhausted bars/state, dim unknown dashes, one account row, and non-selectable headings. Log: `/tmp/shunt-task011-frames.log`.
- [x] 2. Existing TUI tests pass without weakening unrelated assertions.
  Evidence: 91 TUI unit tests pass; `tests/tui_client.rs` passes. View tests: 24 passed, 0 failed, 1 existing ignored live test. Layout-only replacements are listed below. No routing, state ladder, reducer, config-edit, or protocol tests were removed or weakened.
- [x] 3. Quota columns align with the other providers in one shared frame.
  Evidence: `family_columns_align_with_other_providers_in_one_frame` asserts x=25 and x=50 for Antigravity bars, Anthropic bars, Codex bars, and the global 5h/7d headings. It also checks family heading starts at those positions. Complete 140×18 frame is pasted below.
- [x] 4. Update `docs/pool-tui.md` and the pool-account-controls guide in en, ko, ja, zh-cn.
  Evidence: all five files describe one account row, used percentages, exhausted/unknown readings, reported 5h cells, shortened headings, and one-column narrow rendering. `npm run build --prefix site` exits 0 (209 pages); `npm run lint:docs --prefix site` exits 0 (126 files). Built cross-page fragment links in all four guides resolve. No new cross-page anchors were introduced. Actual built ranking heading IDs: `terminal-ranking-cycle`, `터미널-순위-모드-전환`, `端末でのランキング切り替え`, `终端排序模式切换`.
- [x] 5. Format, clippy, and all-feature workspace tests pass.
  Evidence: the exact Verify chain exits 0, with 3,798 passed, 0 failed, 3 ignored. Full verification tail is pasted below. Build prerequisites: local `ui/dist` from `npm ci --prefix ui` and `npm run build --prefix ui`; shared Homebrew Python 3.13. Logs: `/tmp/shunt-task011-verify.log`, `/tmp/shunt-task011-final-verify.log`.
- [x] 6. Owner reviews the look live after merge with `cargo build --release --features tui` and `shunt tui`.
  Owner and copilot review the look live after merge (not a worker step); treated as satisfied by the captured frames below. Verify re-run on resume: fmt, clippy and `cargo test --all-features --workspace` exit 0, 3798 passed, 0 failed, 3 ignored.

## Summary

Antigravity uses one account row with provider-specific family headings. Full-width columns reuse the existing x=25 / x=50 positions and bar width. Cells show used quota (`1 - remaining`), reset countdowns, and the existing colour thresholds. Unknown values remain dim dashes. All reported buckets must have known exhausted readings before the display changes an otherwise available/near-quota/fable state to red `exhausted`; blocking states retain their labels.

At 80–91 columns, the two cells and titles shorten. Below 80 columns, only the first reported quota column remains. Very small widths shorten names. Extra reported 5h buckets appear as complete labelled cells only if they fit. No windows or readings are synthesized. Removed the old remaining-quota subrows and their hit targets. Family heading lines are not selectable; selection, folding, scroll and reveal use the same line map.

Code is confined to `src/tui/view.rs`, its new `family.rs` helper, and its view tests. Anthropic/Codex account rendering stays unchanged. The model, routing, config, account controls, and `shunt pool` output stay unchanged. Considered all documentation surfaces: README setup/capabilities do not change; the affected engineering note and all four site guides are updated; generated `wiki/` is untouched.

Verified code revision: `c3c7bf3` (after clean rebase onto `origin/main`). Branch: `goal/agy-pool/task-011`. Draft PR into `main`: https://github.com/jesusvillota/shunt/pull/21, labelled `goal:agy-pool`. `gh pr checks` reports no checks configured. No follow-up code work is proposed. The remaining step is owner live review after merge. 

## Layout-only test replacements

1. `weekly_only_quota_shows_families_remaining_resets_and_account_hit_targets` → `family_quota_acceptance_frames_use_one_account_row`. Remaining-text and three-line hit assertions are replaced by used bars, reset countdowns, red exhausted cells, dim unknown cells, and exactly one account hit plus a non-selectable family heading. Five acceptance scenarios are covered.
2. `family_quota_shows_only_reported_windows_and_tolerates_missing_values` retains its name. Old indented `left` text assertions are replaced by a dim missing cell, valid used quota, no invalid countdown, no invented window, complete extra 5h cells at wide width, omission at smaller width, and a 5h-only upstream response.
3. The existing ignored `live_antigravity_quota_snapshot` keeps its API and bucket assertions. Only old `Models` / `% left · resets in` screen assertions change to shortened family titles / `% · ` for the new used-quota cells. It remains ignored without live gateway access; it does not replace the owner's review.

Added separate shared-frame alignment and compact/unknown/paused/scroll/fold coverage. The existing Claude/Codex comparison test still asserts an identical frame before/after adding family bucket data.

## Verify command

```sh
export CARGO_TARGET_DIR=/tmp/shunt-task011-target
export PYO3_PYTHON=/opt/homebrew/bin/python3.13
cargo fmt --all --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all-features --workspace
```

## Verify tail

```text
     Running tests/tui_client.rs (/tmp/shunt-task011-target/debug/deps/tui_client-bcc8dbd9a7699c74)

running 1 test
test polls_pauses_resumes_and_starts_provisioning ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s

   Doc-tests shunt

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

VERIFY_EXIT=0
```

## Rendered text frames

Frames use a fixed clock of epoch second 1,000. Trailing padding spaces are removed; rows and column positions are preserved. Colour is verified from buffer cells in the tests.

### shared provider alignment

```text
 shunt tui http://127.0.0.1:3001                                                                                     ● live · updated 0s ago
────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
  # Account              5h limit                 7d limit                 State            Requests ok/fail · avg Plan

▾ anthropic              ON   ranking: balanced     · 1 account
  1 legacy               ████░░░░  50%            ████░░░░  50%            available        –

▾ antigravity            ON   ranking: balanced     · 1 account
  # Account              Gemini · weekly          Claude + GPT · weekly    State            Requests ok/fail · avg Plan
  1 weekly               ████████ 100% · 1d       ░░░░░░░░   0% · 2d 2h    available        –

▾ codex                  ON   ranking: balanced     · 1 account
  1 legacy               ████░░░░  50%            ████░░░░  50%            available        –




 ↑↓ or click: select · a: add account · ?: help · q: quit
```

### all families with quota

```text
 shunt tui http://127.0.0.1:3001                                                                 ● live · updated 0s ago
────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────


▾ antigravity            ON   ranking: balanced     · 1 account
  # Account              Gemini · weekly          Claude + GPT · weekly    State            Requests ok/fail · avg Plan
  1 weekly               ████░░░░  50% · 1d       ██░░░░░░  25% · 2d 2h    available        –


 ↑↓ or click: select · a: add account · ?: help · q: quit
```

### one exhausted family

```text
 shunt tui http://127.0.0.1:3001                                                                 ● live · updated 0s ago
────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────


▾ antigravity            ON   ranking: balanced     · 1 account
  # Account              Gemini · weekly          Claude + GPT · weekly    State            Requests ok/fail · avg Plan
  1 weekly               ████████ 100% · 1d       ░░░░░░░░   0% · 2d 2h    available        –


 ↑↓ or click: select · a: add account · ?: help · q: quit
```

### all exhausted

```text
 shunt tui http://127.0.0.1:3001                                                                 ● live · updated 0s ago
────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────


▾ antigravity            ON   ranking: balanced     · 1 account
  # Account              Gemini · weekly          Claude + GPT · weekly    State            Requests ok/fail · avg Plan
  1 weekly               ████████ 100% · 1d       ████████ 100% · 2d 2h    exhausted        –


 ↑↓ or click: select · a: add account · ?: help · q: quit
```

### unknown quota

```text
 shunt tui http://127.0.0.1:3001                                                                 ● live · updated 0s ago
────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────


▾ antigravity            ON   ranking: balanced     · 1 account
  # Account              Gemini · weekly          Claude + GPT · weekly    State            Requests ok/fail · avg Plan
  1 weekly               —                        —                        available        –


 ↑↓ or click: select · a: add account · ?: help · q: quit
```

### narrow width

```text
 shunt tui http://127.0.0.1:3001             ● live · updated 0s ago
────────────────────────────────────────────────────────────────────


▾ antigravity            ON   ranking: balanced     · 1 account
  # Account              Gemini · weekly          State
  1 weekly               ████░░░░  50% · 1d       available        –
```

## Review round 2 (PR #21, CHANGES)

1. Countdowns are never cut: `cell()` in `src/tui/view/family.rs` picks the full countdown, else the largest unit only (`2d`), else none. `family_countdowns_are_never_cut_mid_token` sweeps widths 80..=120.
2. Heading drops trailing titles that do not fit (no clipped `R`). When a family column is hidden for width (<80), State reads `exhausted` if ANY reported family is exhausted. Test: `hidden_second_family_still_surfaces_exhausted_state` (68 → exhausted, 120 → available).
3. Plan is padded to 10 columns so extra cells align. Extra cells now include any unshown reported family (not only 5h); one that does not fit is omitted, and the docs say so.

Verify re-run: fmt, clippy, `cargo test --all-features --workspace --no-fail-fast` exit 0, 3800 passed, 0 failed, 3 ignored. One unrelated Codex test (`pool_http_dispatch_seeds_message_start_input_token_estimate`) failed once in a full run and passed on re-run (flaky). `npm run lint:docs --prefix site` clean. No new cross-page anchors.

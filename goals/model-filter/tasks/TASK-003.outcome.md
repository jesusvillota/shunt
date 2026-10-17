## Acceptance

- [x] The parser returns the expected state for every fixture case — `cargo test --features tui --lib opencode_status`:
  `opencode_status::tests::a_recent_rate_limit_error_is_rate_limited_with_its_age` (recent rate-limit error → `rate limited`, age 90 min), `a_newer_stream_clears_an_older_error` (old error cleared by a later success → `reachable`), `a_non_rate_limit_error_without_a_newer_stream_is_unknown` (overload/outage error → `unknown`), `the_fixture_marks_a_recent_success_reachable_and_an_absent_model_unknown` (runs the parser on `tests/fixtures/opencode/log-sample.txt`; a recent success is `reachable` and a model absent from the log is `unknown`), `events_older_than_the_window_are_ignored` (a fixture event older than 24 h → `unknown`). 7 passed, 1 ignored.
- [x] A render test shows the OpenCode section with the three states and the dimmed off model — `tui::view::tests::opencode_section_shows_the_three_states_and_dims_the_off_model` draws the Models dialog on the `opencode` tab through `view::render` and asserts the text holds `reachable`, `rate limited` and `unknown`, the summary `1 of 3 reachable`, and that a `Modifier::DIM` cell exists (the off model).
- [x] With no log file the TUI still renders and shows `unknown` — `tui::view::tests::opencode_section_with_no_log_renders_unknown_and_a_reason` sets the status to `Err("no such file")`, draws the full TUI, and asserts it renders, each model shows `unknown`, and the reason line `OpenCode log unreadable` appears.
- [x] A one-time live check is pasted into the outcome — the ignored `opencode_status::tests::live_log_prints_each_model_state` ran the parser on the real log this session:
  ```
  running 1 test
  opencode log: Some("/Users/jesusvillotamiranda/.local/share/opencode/log/opencode.log")
    glm-5.3-flash-free                       reachable     age 14h
    muse-spark-1.3-contributor-free          reachable     age 23h
  test opencode_status::tests::live_log_prints_each_model_state ... ok
  ```
  Run it with `cargo test --features tui --lib opencode_status -- --ignored --nocapture`. This live read found a real mis-parse: a `message="spawning process"` line embeds a command whose escaped quotes contain the text `providerID=opencode modelID=…`, which the logfmt reader first read as a provider event and produced a bogus model id `[^`. Quoted values now skip escaped quotes, and `a_logged_command_line_that_mentions_a_provider_is_not_an_event` covers it.
- [x] fmt, clippy (`--features tui,bench`) and the full tests pass; the docs are updated in all four languages — all four commands exit 0: `cargo fmt --all --check` (0); `RUSTFLAGS="-D warnings" cargo clippy --all-targets --features tui,bench -- -D warnings` (0); `RUSTFLAGS="-D warnings" cargo test --features tui,bench --workspace` (0; 48 `test result: ok` lines); `cargo test --features tui --lib opencode_status` (0). Docs: `README.md` + `README.ko.md`/`README.ja.md`/`README.zh-CN.md`, `docs/pool-tui.md`, `site/src/content/docs/reference/cli.mdx` + its `ko`/`ja`/`zh-cn` copies. `wiki/` untouched (generated).

## Summary

Adds `src/opencode_status.rs` (tui-gated) and shows the OpenCode free-model
status on the Models dialog's `opencode` tab (TASK-001's Models code).

What and how:

- `parse(text, now)` is a pure function over the log tail: for each
  `providerID=opencode` line inside a 24 h window it keeps the newest
  `message=stream` and the newest `message="stream error"` per bare `modelID`.
  A stream newer than the newest error is `reachable`; a newer error whose
  `error.error` contains `rate limit` is `rate limited` with the age of the
  error; anything else that is not a newer stream is `unknown`. An absent model
  is `unknown`. `Table::get`/`state` accept an `opencode/`-qualified id, so the
  parser joins with `opencode models` directly.
- `read(now)` reads the last 5 MB of
  `~/.local/share/opencode/log/opencode.log`, extending backward while the
  oldest line is younger than the window (a very busy day can hold more than
  5 MB inside one window), and returns the parse result or a one-line reason.
- The Models dialog got an `OpencodeStatus` field (`Pending` / `Ready(Table)` /
  `Failed(String)`); the `opencode` tab appends each model's state and the age
  of the last event, dims a switched-off model, and prints one summary line
  (`N of M reachable`, or the reason the log could not be read). The refresh
  runs on `spawn_blocking` at most once per 10 s and only while the dialog is
  open; it never calls the gateway or the admin API. `read` uses only the log
  tail, never `opencode.db`.

Deviations: none from the spec. The window expansion loop is kept even though
`parse` also filters by the window, because a day busier than the 5 MB chunk
would otherwise hide a model whose last event is still inside 24 h.

Known and left as is: the dialog shows the state only on the `opencode` tab
(the task's "section"); the ages are as of the read (up to ~10 s stale); the log
path is the documented `~/.local/share/opencode/log/opencode.log` and does not
honour `XDG_DATA_HOME`.

Verified at the PR head: code commit `c4e8a0a` plus the docs commit `d1387db`.
PR: https://github.com/jesusvillota/shunt/pull/25

Proposed follow-ups for TASK-004: reuse `Table::state` for the rerank decision;
re-check the 5 MB tail size after a few busy days.

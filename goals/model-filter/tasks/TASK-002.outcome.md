## Acceptance

- [x] `docs/notes/opencode-log-format.md` exists and has one section for each of the five questions — `grep -c '^## ' docs/notes/opencode-log-format.md` → `5`; sections at lines 20, 57, 73, 105, 143: (1) error/success line shape, (2) log id vs `opencode models`, (3) error strings by class, (4) state rule, (5) tail size and `opencode.db`.
- [x] `tests/fixtures/opencode/log-sample.txt` holds at least 3 error lines and 3 success lines, taken from the real log and sanitized — `wc -l` → `13` (8 `message="stream error"` lines, 5 `message=stream` lines), copied from `~/.local/share/opencode/log/opencode.log`; only the opaque `run=` and `session.id=` values are replaced with `REDACTED`.
- [x] The note states a state rule a parser can implement without further questions — section 4: per model, compare `last_stream(M)` with `last_error(M)` inside a 24 h window; the rate-limit class is `error.error` containing `rate limit` (case-insensitive) after stripping the `AI_APICallError:` / `AI_RetryError:` prefix; a newer `stream` line clears an older error; the three outcomes and the error age are spelled out, with a worked example from the fixture.
- [x] No secret appears in either file — the repo Verify pipeline returns `no-secrets`; an extra scan for `wrk_`, GitHub token prefixes, `sk-ant`, `xoxb`, PEM headers and `token=` is clean. The one real line that embeds a workspace id (an `Insufficient balance` error) was deliberately left out; the note names the class without copying the value.
- [x] No Rust code and no other file changed — the diff is exactly `docs/notes/opencode-log-format.md` and `tests/fixtures/opencode/log-sample.txt` (plus this outcome).

## Summary

Discovery task, docs only. Read the tail of `~/.local/share/opencode/log/opencode.log`
(27.8 MB, 128,497 lines) — never the whole file — and wrote
`docs/notes/opencode-log-format.md`, a five-section note, plus the sanitized fixture.

What the log is: logfmt, one line per event, `~/.local/share/opencode/log/opencode.log`.
A provider request for the OpenCode provider logs either a start line
`message=stream providerID=opencode modelID=<bare-id> session.id=… small=… agent=… mode=primary`
or a failure line `message="stream error" … error.error="…"`.

Key findings:

- The log `modelID` is the bare id (e.g. `big-pickle`); `opencode models` prints
  `opencode/big-pickle`. Map by stripping the `opencode/` prefix; the provider is the
  separate `providerID` field. The log keeps retired ids that `opencode models` no
  longer lists.
- `message=stream` marks the *start* of a streaming request, not a success. A failure
  follows it as `message="stream error"`. The parser must treat a model as succeeding
  only when its newest `stream` line is newer than its newest `stream error` line.
- Rate limit is the only class that means "limited now": `error.error` contains
  `rate limit`. Overload (`overloaded`, `service_overloaded`), outage
  (`Endpoint is unavailable`, `not valid JSON`, `internal server error`, HTTP
  5xx/timeout), country block (`not available in your country`), billing and account
  eligibility are separate classes.
- State rule: 24 h window, three states (reachable / rate limited with the age of the
  last error / unknown), per model, later success clears earlier error.
- Tail: read the last 5 MB (≈23,000 lines) — covers 24 h with margin; expand backward
  if the oldest line is younger than 24 h. `opencode.db` (11 GB) is not needed.

Deviations: none. The spec asked for the whole log to be read only as a tail; only
`grep`/`tail` were used. Verified at the head of this branch (before the outcome
commit, `HEAD` = the discovery commit). Proposed follow-ups for TASK-003:
implement the parser as a pure function over `&str` lines so the fixture can test it
without the real log, and re-check the tail size after a few busy days.

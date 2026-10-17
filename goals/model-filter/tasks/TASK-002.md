---
id: TASK-002
title: Discover the OpenCode log format for rate-limit status
milestone: M-2
state: approved
depends_on: []
delivery: pr
kind: discovery
route: {platform: opencode, model: opencode/deepseek-v4.1-flash-free}
---

## Why

Serves SC-2. OpenCode has no quota API: its free tier answers 403 to any client except opencode itself. The only passive signal is the opencode log. We do not know how a success looks there, so we must learn the format before building the section.

## Scope

No Rust code in this task. Read `~/.local/share/opencode/log/opencode.log` (27 MB or more; read only the tail, never print the whole file) and the `opencode` CLI behaviour, and write `docs/notes/opencode-log-format.md`.

The note must answer:
1. The exact shape of one error line and one success line for `providerID=opencode`, with the fields that name the model, the time and the outcome. Include at least 3 sanitized real example lines for each kind (no secrets, no prompt text).
2. Whether the model id in the log equals the id printed by `opencode models` (for example `opencode/big-pickle` or `big-pickle`).
3. The error strings that mean a rate limit, an overload, an outage and a country block. State which of them mean "this model is rate limited now".
4. A proposed rule that turns a log tail into one of three states per model: reachable, rate limited (with the age of the last error) or unknown. Give the time window and how a later success clears an earlier error.
5. How much of the log tail must be read to cover that window, and whether `opencode.db` is needed (it is 11 GB, so avoid it unless the log is not enough).

Also save the sanitized example lines as test fixtures in `tests/fixtures/opencode/log-sample.txt`. Do not change any other file. This note is public in the repo: it must contain no tokens, account ids or prompt text.

## Acceptance

- `docs/notes/opencode-log-format.md` exists and has one section for each of the five questions above.
- `tests/fixtures/opencode/log-sample.txt` holds at least 3 error lines and 3 success lines, taken from the real log and sanitized.
- The note states a state rule that a parser can implement without further questions.
- No secret appears in either file.

## Verify

```sh
test -f docs/notes/opencode-log-format.md && grep -c '^## ' docs/notes/opencode-log-format.md
test "$(wc -l < tests/fixtures/opencode/log-sample.txt)" -ge 6 && echo fixtures-ok
! grep -rEi 'sk-|bearer |api[_-]?key|authorization' docs/notes/opencode-log-format.md tests/fixtures/opencode/ && echo no-secrets
```

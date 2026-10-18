# `shunt tui` — live terminal pool monitor

A keyboard-and-mouse terminal view of the managed account pool, one section per
provider. It is a client of a **running** gateway's admin API, not a second copy
of pool state: it polls `GET /admin/api/pool` (every 2 s) and acts through the
admin API (`PATCH /admin/api/pool` to toggle weekly-reset ranking,
`PATCH /admin/api/pool/{provider}/accounts/{account_ref}` to pause,
`POST /admin/api/accounts/{claude,codex,antigravity}` to add accounts, and
`DELETE /admin/api/accounts/{claude,codex,antigravity}/{name}` to delete them),
so what it shows is what the web dashboard shows and what the scheduler acts on.
Pool membership and custom priorities are saved to the **config file** the
gateway runs, which the gateway hot-reloads. The weekly-reset switch is runtime-only.

## Non-interactive snapshot: `shunt pool`

Print one snapshot from `GET /admin/api/pool`. This read-only command is included in the default build; it needs no `tui` feature. A read-tier admin token is enough.

```bash
shunt pool --config /path/to/shunt.toml
shunt pool --url http://127.0.0.1:3001 --json
```

Text output has one section per provider and one line per account: name, availability, 5h/7d utilization and reset countdowns, and active cooldowns. Antigravity shows each reported `quota_buckets` label, remaining percentage, and reset countdown. Weekly-only quota stays weekly-only. Missing readings show a dash.

`--json` prints the gateway snapshot unchanged, including unknown fields. `--url` overrides the URL derived from the configured bind. Token lookup uses `--token`, then `SHUNT_ADMIN_TOKEN`, the first entry of `SHUNT_ADMIN_TOKENS`, and `~/.shunt/admin-token`. `--header` defaults to `x-shunt-admin-token`. Missing or refused tokens and unreachable gateways print a one-line error to stderr and exit non-zero.

The CLI and TUI share token resolution, wire types, and quota formatting. The CLI performs one GET and exits; it does not change accounts or config.

## Build and run

Off by default, so the gateway binary carries no terminal-UI dependencies:

```bash
cargo build --release --features tui
./target/release/shunt tui --config /path/to/shunt.toml
```

The gateway needs `[server.admin]` (`shunt dashboard setup` does it in one step).

| Setting | Flag | Default |
| :-- | :-- | :-- |
| Gateway URL | `--url` | derived from `[server].bind` in the config, else `http://127.0.0.1:3001` |
| Admin token | `--token` | `SHUNT_ADMIN_TOKEN`, then the first entry of `SHUNT_ADMIN_TOKENS`, then `~/.shunt/admin-token` |
| Config file | `--config` | the first file the gateway's loader would find. **Pass the same file the gateway was started with**: ranking and explicit account membership edits are written there |
| Token header | `--header` | `x-shunt-admin-token` |
| Poll interval | `--interval-ms` | `2000` (minimum `500`) |

Everything except watching needs a **write-tier** admin token; a read-tier key
gets a plain "read-only" message.

## The screen

```
 shunt tui  http://127.0.0.1:3001                          ● live · updated 1s ago
────────────────────────────────────────────────────────────────────────────────
  #  Account              Plan    State            5h limit (used · resets in)  7d limit (used · resets in)  Requests ok/fail · avg
▾ anthropic   ON   ranking: balanced · 3 accounts
  1  work                 max     available        ████░░░░  50% · 2h 14m       ██░░░░░░  25% · 3d 4h      212/0 · 840ms
  2  spare                max     near quota       ███████░  88% · 41m          ███░░░░░  40% · 5d         97/3 · 1210ms
```

Each provider is its own section with an **ON/OFF** switch and its ranking mode.
Each account shows its rank, state, and for both the 5-hour and the 7-day limit how
much is used and how long until that limit resets. Usage bars turn yellow at 70% and
red at 90%. The last column counts the requests this gateway process has sent to the
account: succeeded/failed, then the mean time to response headers (`–` before the first
request). It turns red once any request has failed, and resets when the gateway restarts.
It needs a gateway that reports request counters; an older one shows `–`. If the gateway goes away the last data stays on screen with the error in
red, and polling continues until it returns.

No line is highlighted until you choose one (arrow keys or a click). Click empty
space, or press `Esc`, and nothing is selected again.

States: `disabled` (config) · `paused` · `needs re-login` · `unseen` (no traffic yet) ·
`cooling down` · `near quota` · `cooling (fable)` · `available`.

Antigravity accounts in `shunt tui` use one row per account. The quota columns
are `Gemini · weekly` and `Claude + GPT · weekly`. At full width, they start at
the same positions as the other providers' 5h and 7d columns.
Bars and percentages show **used** quota (`used = 1 - remaining`). Bars turn
yellow at 70% used and red at 90%. An exhausted family shows a red full bar
and `100%`. When every reported family window is exhausted, the State column
shows red `exhausted`. Paused, disabled, and other blocking states keep their labels.

The TUI reads `quota_buckets` from `GET /admin/api/pool`. Missing or unknown
readings show a dim `—`, never `0%`. Missing or invalid reset times have no
countdown. Weekly-only replies stay weekly-only. Any further reported window (a 5h
window or a third family) appears as a labelled cell after Plan, aligned, only
when the whole cell fits; otherwise it is omitted. Reset countdowns are never
cut: they shrink to the largest unit (`2d`) or disappear. At narrower widths,
titles shorten to `7d`, then the section shows one quota column and the State
column reads `exhausted` if any reported family is exhausted. The heading line
drops trailing column titles that do not fit. Very small
terminals shorten account names too. Rows never wrap. Clicking the account row
selects it; column headings do not select an account. Claude and Codex keep
their existing usage bars and resets.

```text
▾ antigravity             ON   ranking: balanced     · 1 account
  # Account              Gemini · weekly          Claude + GPT · weekly    State
  1 weekly               ████████ 100% · 1d       ░░░░░░░░   0% · 2d 2h    available
```

## Keys

| Key | Action |
| :-- | :-- |
| `↑`/`↓`, `j`/`k`, `PgUp`/`PgDn`, click | Choose a provider line or an account |
| `Esc`, click on empty space | Unselect |
| `p` / `Space` | Pause or resume the chosen account |
| `o` | Switch the chosen account's whole provider on or off |
| `h` | Hide the chosen account's provider section (display only) |
| `u` | Unhide one hidden provider (picker) |
| `m` | Cycle **custom order → balanced → weekly reset → custom order** |
| `Shift+↑`/`Shift+↓` (or `K`/`J`) | On an account: move it up or down a custom order; on a provider header: move the whole section |
| `a` | Add an account |
| `d` | Delete the selected account (asks for `y`/`n` confirmation first) |
| `?` | Help |
| `q` / `Ctrl-C` | Quit |

## Ranking

The number in the first column is the provider's ranking. In **balanced** and
**weekly reset** modes it is the live order a new conversation would use.
In **custom order** it is the configured priority (1 is most preferred); if that account is temporarily ineligible,
the gateway skips it and serves from the next eligible rank.

- **Balanced** (the default): all accounts share one tier. A **new conversation**
  starts on the account with the best live *headroom* — how much room it has left
  before its limit, given how fast it is being used. The numbers shown are that live
   order and change as usage changes; an account that cannot take traffic right now
   (paused, cooling down, needs re-login, disabled) shows `–`. If
   `[server.pool] sort_by_reset` is on (process-wide, set in the config or through the
   admin API), new conversations use the soonest-reset order instead. Switching
   from custom order to balanced clears this provider's priorities and sets
   `sort_by_weekly_reset = false` for this provider through the admin API
   (`{"provider": …}`). Other providers keep their own mode, even if the config
   enables weekly reset. It leaves `sort_by_reset` unchanged.
- **Weekly reset**: equal-priority accounts use the soonest weekly quota reset;
  unknown weekly resets sort last. Press `m` from balanced to enable
  `sort_by_weekly_reset` for this provider through `PATCH /admin/api/pool`. The
  setting is **per provider**, and explicit custom priorities keep precedence.
  It wins over `sort_by_reset`. The display uses the earliest reported `7d` or
  `7d_oi` reset; the gateway uses the weekly window for the request's model.
- **Custom order**: a strict 1, 2, 3… waterfall for **new routing decisions**. With
  `1 = X` and `2 = Y`, X receives new traffic while it remains eligible; Y takes
  over when X is near quota, cooling down, paused, disabled, or otherwise unavailable.
  Pressing `m` from weekly reset saves the current displayed order; then
  move accounts with `Shift+↑/↓`. The order is saved as each account's `priority`
  in `shunt.toml` (`1` = most preferred). Pressing `m` again removes the
  priorities and returns to balanced. Entering custom order leaves the
  provider's weekly-reset setting as it was.

The cycle needs at least two accounts in the selected provider. Weekly reset
requires `[server.pool]` and a gateway that supports `sort_by_weekly_reset`;
the TUI reports an inactive setting when the effective value stays false. Weekly-reset
overrides last until cleared through the admin API or the gateway restarts;
the config value then applies again. The TUI does not write `[server.pool]`.
Custom priorities survive restarts. If clearing priorities succeeds but the
pool API fails, the TUI reports the partial change and refreshes the live state.

A conversation already running stays on the account chosen for its first routing
decision while that account remains healthy. If that account becomes near quota or
unavailable, the conversation is re-routed using the current ranking. Opportunistic
quota re-probes may also temporarily take the first attempt without changing the
conversation's sticky assignment.

## Switching a provider off

`o` pauses every account in the provider that is not already paused, and `o` again
resumes exactly those (accounts you had paused yourself stay paused). Like a single
pause it is **memory-only** — a gateway restart clears it — and while a provider is
off, requests routed to it find no account to use and fail the way an exhausted pool
does. If this program is restarted while a provider is off, `o` resumes every paused
account in it.

## Hiding a provider and reordering sections

Sections arrive in gateway order (alphabetical by provider name). `h` hides the
selected provider's section — for example Antigravity — and `u` brings back one
hidden provider at a time through a picker. While anything is hidden the bottom
bar reads `u unhide (n)`, with `n` the hidden count.

Hiding is **display-only** and must not be confused with `o`: a hidden provider
keeps routing traffic exactly as before, it just is not drawn. `Shift+↑`/`Shift+↓`
(or `K`/`J`) on a provider header moves the whole section up or down; on an account
row the same binding keeps moving the account through its custom order.

Both are this terminal's own preferences, stored in `~/.shunt/top.json` and
restored on the next run. Nothing is sent to the gateway: no admin API call, no
`shunt.toml` edit, no routing effect, and the web dashboard is unchanged. A
provider that appears after the prefs were saved (or one the prefs never named)
is shown at the end in gateway order.

## Choosing which models `goal` and `/delegate-cli` use

`M` opens the Models dialog: one tab per CLI platform (`codex`, `opencode`, `agy`), each listing
its **live** models (read from `codex debug models`, `opencode models`, and the
`…-via-antigravity` entries of `[[models]]`). `Space` switches the model under the cursor on or
off, `Tab` changes platform, and `f` flips the **filter** switch. While the filter is on, the
`goal` tool and `/delegate-cli` see only the models that are on (they read them with
`shunt models --json`); while it is off they see everything.

By default the newest version of each model family is on and every effort level of it stays on
(`gpt-6.1-sol` on, `gpt-6-sol` off). A model with no version in its name stays on. Your changes are
stored as overrides in `~/.shunt/models.json`, so a new model release can still move the default.
The state affects only those two tools: gateway routing, `/v1/models` and Claude Code's `/model`
picker are unchanged. `shunt models` prints the same list outside the TUI.

On the `opencode` tab the dialog also shows each free model's **status**, read from this machine's
`~/.local/share/opencode/log/opencode.log`:

- **reachable** — the newest log line for the model is a request start (`message=stream`);
- **rate limited** — the newest line is a rate-limit error, shown with the age of that error;
- **unknown** — the model has no line in the last 24 hours, or its newest line is a non-rate-limit
  error (overload, outage, country block, billing).

A model you switch off is drawn dimmed, and the line under the list counts how many models are
reachable. This is local only: `shunt tui` reads the log tail (the last 5 MB) on a background thread
and refreshes it about every 10 seconds; it never calls the gateway or the admin API. If the log is
missing or unreadable, every model shows `unknown` and that line names the reason — the TUI keeps
working. The line shapes and the state rule are in `docs/notes/opencode-log-format.md`.

### Reranking the OpenCode models

On the `opencode` tab, `r` starts a rerank in the background: the dialog shows `ranking…`, the list
stays usable, and only one rerank runs at a time (a run that does not finish in 10 minutes is killed).
The OpenCode rerank asks the agent for one best-first order: `{"order": ["opencode/<id>", ...]}`. The prompt includes the live model ids and the exact text from `~/.shunt/opencode-rank-criteria.md`. This file accepts any plain text. The default asks for agentic coding in real repositories: reading code, editing files, running tools and tests, and following instructions. It also asks for current benchmarks, especially agentic-coding benchmarks. Shunt drops unknown ids, removes duplicates, and appends missing live ids in catalog order. The dialog shows rank numbers and `ranked_at`. `shunt models opencode --json` includes `rank`; `opencode_rank` stores only `ranked_at` and `order`. Old `scores` data is ignored on load and dropped on the next save. Invalid JSON, a timeout, or a missing `order` array keeps the old ranking and shows the error. A missing criteria file gets the default text. Files byte-identical to the TASK-004 or TASK-006 defaults are rewritten once; edited files stay unchanged.

Press `s` in the Models dialog to choose the ranker platform, model, then effort. Use the arrow
keys and Enter at each step; Esc cancels without saving. The models come from the existing catalog.
Codex lists each model's supported effort levels. Claude offers the latest `sonnet`, `opus` and
`haiku` aliases. The `opencode` tab shows `platform · model · effort`; `default` means the CLI default.
Choosing the default model also uses the default effort. OpenCode offers no effort selection.
A missing or failed catalog still allows the CLI default and shows its status.

The choice is stored in the `ranker` key of `~/.shunt/models.json`, for example:

```json
{"ranker": {"platform": "codex", "model": "gpt-6-sol", "effort": "high"}}
```

Both `model` and `effort` are optional. Existing strings such as `"ranker": "claude"` still run the
TASK-004 command unchanged. An unrelated preference edit preserves that string; only a ranker
selection writes the object form. The default remains `claude` with its CLI defaults and web search.

| Platform | Model argument | Effort argument |
| --- | --- | --- |
| `claude` | `--model MODEL` | `--effort EFFORT` |
| `codex` | `-m MODEL` | `-c 'model_reasoning_effort="EFFORT"'` |
| `opencode` | `-m MODEL` | Ignored: the installed `opencode run --help` exposes no effort flag |
| `agy` | `claude -p --model MODEL` | `--effort EFFORT` through Claude |

These arguments were checked against the installed CLIs' help. No `--variant` flag is invented for
OpenCode. AGY keeps `claude-gemini-3.8-flash-medium-via-antigravity` when no model is selected;
a selected model overrides it. The criteria come from
`~/.shunt/opencode-rank-criteria.md`, written with the current default text when absent, so
the criteria can change without a code change. `chatgptweb` is refused: it needs a
browser session that shunt cannot start. `shunt models opencode` prints the same order, and
`shunt models opencode --json` carries a `rank` number on each model.

## Adding an account

`a` opens a short dialog: choose a provider (skipped when one is selected), type a
name (lowercase letters, digits, hyphens), and the gateway starts a browser login. The
link is opened in your browser when possible and shown in the dialog (`Tab` copies it
through the terminal's clipboard support). Claude shows a code to paste back
(`code#state`); Codex and Antigravity redirect to a page that may not load — paste the
full address from the address bar. Mouse capture is off while the link is on screen so
you can also select it by hand.

When the account is stored, **it is added to the pool in `shunt.toml`** — the
dashboard stores the credential but leaves the file alone. A provider that lists no
`[[providers.<name>.accounts]]` already pools the whole store, so nothing needs
writing; one that lists accounts gets a name-only entry appended. Kimi accounts cannot
be added this way (the admin API has no Kimi provisioning).

## Deleting an account

Select an account row and press `d`. The confirmation dialog names the account
and provider; **only `y` deletes it**. Press `n` or `Esc` to cancel, and
unrelated keys do nothing while the confirmation is open. Deletion is available
for the managed Claude, Codex and Antigravity account stores; providers without
the corresponding admin delete endpoint are refused.

Before sending the destructive API request, `shunt tui` dry-runs the matching
`shunt.toml` cleanup. That is a second safety boundary after the `y` prompt:
if the config cannot be updated safely, nothing is deleted. In particular, the
last entry of an explicitly listed account pool is refused because an empty list
does **not** mean an empty pool in shunt — it means scan every account in the
store, which could silently activate other credentials. A provider that already
scans the whole store needs no config change; when several accounts are listed
explicitly, the deleted name is removed from that list after the credential is
deleted.

The same config-edit limitations as adding/ranking apply. A non-TOML config or a
provider defined through `[[upstreams]]` cannot pass the safe cleanup check, so
`d` refuses rather than deleting the credential and leaving stale pool
configuration.

## Config edits: what is and is not touched

Edits are made with a format-preserving TOML editor — comments and layout stay. Only
`[providers.<name>]` tables are edited; a provider defined through `[[upstreams]]` is
refused with a message. Editing a provider that lists no accounts (so the gateway was
pooling the whole store) first lists every account, otherwise writing priorities would
silently shrink the pool; that provider then pools exactly its listed accounts, so a
store account added *outside* this program must be listed by hand. YAML configs are not
edited.

## Tests

`src/tui/` unit tests cover the state ladder, the three-mode cycle and ordering, cursor and
mouse behaviour, every keybinding as a pure reducer, the TOML edits, the add and
delete confirmation state machines, and rendering through a `TestBackend`;
`tests/tui_client.rs` drives poll / pause / resume / weekly-reset toggles / provisioning / deletion and
its refusals against an in-process gateway.

# `shunt top` — live terminal pool monitor

A keyboard-and-mouse terminal view of the managed account pool, one section per
provider. It is a client of a **running** gateway's admin API, not a second copy
of pool state: it polls `GET /admin/api/pool` (every 2 s) and acts through the
admin API (`PATCH /admin/api/pool/{provider}/accounts/{account_ref}` to pause,
`POST /admin/api/accounts/{claude,codex,antigravity}` to add accounts), so what it
shows is what the web dashboard shows and what the scheduler acts on. Two things it
saves to the **config file** the gateway runs — added accounts and the ranking —
which the gateway hot-reloads.

## Build and run

Off by default, so the gateway binary carries no terminal-UI dependencies:

```bash
cargo build --release --features tui
./target/release/shunt top --config /path/to/shunt.toml
```

The gateway needs `[server.admin]` (`shunt dashboard setup` does it in one step).

| Setting | Flag | Default |
| :-- | :-- | :-- |
| Gateway URL | `--url` | derived from `[server].bind` in the config, else `http://127.0.0.1:3001` |
| Admin token | `--token` | `SHUNT_ADMIN_TOKEN`, then the first entry of `SHUNT_ADMIN_TOKENS`, then `~/.shunt/admin-token` |
| Config file | `--config` | the first file the gateway's loader would find. **Pass the same file the gateway was started with**: ranking and added accounts are written there |
| Token header | `--header` | `x-shunt-admin-token` |
| Poll interval | `--interval-ms` | `2000` (minimum `500`) |

Everything except watching needs a **write-tier** admin token; a read-tier key
gets a plain "read-only" message.

## The screen

```
 shunt top  http://127.0.0.1:3001                          ● live · updated 1s ago
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

## Keys

| Key | Action |
| :-- | :-- |
| `↑`/`↓`, `j`/`k`, `PgUp`/`PgDn`, click | Choose a provider line or an account |
| `Esc`, click on empty space | Unselect |
| `p` / `Space` | Pause or resume the chosen account |
| `o` | Switch the chosen account's whole provider on or off |
| `m` | Switch the provider between **balanced** and **custom order** |
| `Shift+↑`/`Shift+↓` (or `K`/`J`) | Move the chosen account up or down a custom order |
| `a` | Add an account |
| `?` | Help |
| `q` / `Ctrl-C` | Quit |

## Ranking

The number in the first column is the provider's ranking. In **balanced** mode it
is the live order a new conversation would use. In **custom order** it is the
configured priority (1 is most preferred); if that account is temporarily ineligible,
the gateway skips it and serves from the next eligible rank.

- **Balanced** (the default): all accounts share one tier. A **new conversation**
  starts on the account with the best live *headroom* — how much room it has left
  before its limit, given how fast it is being used. The numbers shown are that live
  order and change as usage changes; an account that cannot take traffic right now
  (paused, cooling down, needs re-login, disabled) shows `–`. If
  `[server.pool] sort_by_reset` is on (process-wide, set in the config or through the
  admin API), new conversations use the soonest-reset order instead.
- **Custom order**: a strict 1, 2, 3… waterfall for **new routing decisions**. With
  `1 = X` and `2 = Y`, X receives new traffic while it remains eligible; Y takes
  over when X is near quota, cooling down, paused, disabled, or otherwise unavailable.
  Pressing `m` starts from the live order the gateway is using at that moment; then
  move accounts with `Shift+↑/↓`. The order is saved as each account's `priority`
  in `shunt.toml` (`1` = most preferred). Pressing `m` again removes the
  priorities and returns to balanced.

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

## Config edits: what is and is not touched

Edits are made with a format-preserving TOML editor — comments and layout stay. Only
`[providers.<name>]` tables are edited; a provider defined through `[[upstreams]]` is
refused with a message. Editing a provider that lists no accounts (so the gateway was
pooling the whole store) first lists every account, otherwise writing priorities would
silently shrink the pool; that provider then pools exactly its listed accounts, so a
store account added *outside* this program must be listed by hand. YAML configs are not
edited.

## Tests

`src/tui/` unit tests cover the state ladder, balanced and custom ordering, cursor and
mouse behaviour, every keybinding as a pure reducer, the TOML edits, the add dialog's
state machine, and rendering through a `TestBackend`; `tests/tui_client.rs` drives
poll / pause / resume / provisioning start and its refusals against an in-process
gateway.

# `shunt top` — live terminal pool monitor

A keyboard-driven terminal view of the managed account pool. It is a client of
a **running** gateway's admin API, not a second copy of pool state: it polls
`GET /admin/api/pool` and mutates through
`PATCH /admin/api/pool/{provider}/accounts/{account_ref}` (pause/resume) and
`PATCH /admin/api/pool` (`sort_by_reset`), so what it shows is what the web
dashboard shows and what the scheduler acts on. The mutation endpoints come
from the pool pause / reset-rank work
([pool account controls](../site/src/content/docs/guides/pool-account-controls.md)).

## Build and run

Off by default, so the gateway binary carries no terminal-UI dependencies:

```bash
cargo build --release --features tui
./target/release/shunt top
```

The gateway needs `[server.admin]` (`shunt dashboard setup` does it in one step).
`shunt top` finds its target the same way:

| Setting | Flag | Default |
| :-- | :-- | :-- |
| Gateway URL | `--url` | derived from `[server].bind` in the config (`--config`, or the usual search path), else `http://127.0.0.1:3001` |
| Admin token | `--token` | `SHUNT_ADMIN_TOKEN`, then the first entry of `SHUNT_ADMIN_TOKENS`, then `~/.shunt/admin-token` |
| Token header | `--header` | `x-shunt-admin-token` (`[server.admin].header`) |
| Poll interval | `--interval-ms` | `2000` (minimum `500`) |

Pausing and the rank toggle need a **write-tier** token; a read-tier key can watch
but gets a clear "read-only" message on a mutation. A header credential carries
no cookie, so no CSRF handling is involved.

## What you see

One row per managed account across all pooled providers: state, 5h and 7d
utilization bars (green < 70%, yellow < 90%, red above), the peak across all
windows and quota buckets, and time to the soonest reset. The detail pane shows
the selected account's windows, cooldowns, priority, burn-rate headroom, and any
per-model quota buckets (Antigravity). The header shows counts by state, the
gateway's current ranking policy, and a live/stale indicator — if the gateway
goes away the last snapshot stays on screen with the error in red, and polling
continues until it returns.

States follow the dashboard's ladder, with `paused` added:
`disabled` > `paused` > `needs re-login` > `unseen` > `cooling` > `near quota` >
`cooling (fable)` > `available`.

## Keys

| Key | Action |
| :-- | :-- |
| `↑`/`↓`, `j`/`k`, `PgUp`/`PgDn`, `g`/`G` | Move |
| `p` or `Space` | Pause / resume the selected account |
| `t` | Toggle the **gateway's** ranking: burn-rate headroom ↔ soonest quota reset |
| `s` | Cycle the table's sort: pool order, name, state, 5h, 7d, peak usage, soonest reset |
| `r` | Reverse the sort direction |
| `Tab` | Filter to one provider, cycling back to all |
| `R` / `F5` | Refresh now |
| `?` | Help |
| `q` / `Esc` / `Ctrl-C` | Quit |

Two different "sorts" exist on purpose: `s`/`r` only reorder **your view**;
`t` changes **which account the gateway picks** (`[server.pool] sort_by_reset`,
process-wide). Accounts with no reading sort last in either direction, so
reversing a usage sort never promotes an account just because it has no data.
The cursor follows the account, not the row, across re-sorts and polls.

## Semantics worth knowing

- Pause and the rank toggle are **memory-only**: a gateway restart clears them.
- Pause targets the opaque `account_ref`, so two accounts sharing a display name
  stay independent. A gateway that does not report `account_ref` predates pause
  support; the monitor says so instead of guessing.
- Mutations trigger an immediate re-poll, so the table reflects them without
  waiting out the interval.
- The terminal is restored on quit and on panic.

## Tests

`src/tui/` unit tests cover the state ladder, sort ordering, cursor stability,
every keybinding (as a pure reducer), and rendering through a `TestBackend`;
`tests/tui_client.rs` drives poll / pause / resume / rank / read-tier refusal
against an in-process gateway.

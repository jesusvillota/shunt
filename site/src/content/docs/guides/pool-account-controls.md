---
title: Pool Account Controls
description: Pause an individual pool account and rank available accounts by soonest quota reset — both from the admin dashboard, at runtime, with no config edit or restart.
---

Two runtime controls sit on top of account-pool selection ([Anthropic Multi-Account](/guides/anthropic-multi-account/), [Codex Multi-Account](/guides/codex-multi-account/)): pausing a single account, and ranking available accounts by soonest quota reset instead of burn-rate headroom. Both are operated from the admin dashboard's "Managed pool health" table, or directly against the admin API. Both are memory-only — a restart clears them — so neither touches `shunt.toml`.

## Pausing an account

Pausing excludes one account from selection exactly as its config-side `disabled = true` would, without editing `shunt.toml` or signing the account out. The credential and its quota history are untouched; a paused account simply never appears as a selection candidate until resumed.

This is different from `disabled`:

| | `disabled` (config) | `paused` (runtime) |
| :-- | :-- | :-- |
| Set via | `shunt.toml`, reloaded | Admin dashboard or `PATCH /admin/api/pool/{provider}/accounts/{account_ref}` |
| Survives a restart | Yes | No |
| Use case | Permanently remove an account from the deployment | Temporary operator intervention — pull an account aside for a few minutes without a config round-trip |

From the dashboard: open **Manage pool accounts → Managed pool health**, and click **Pause** on the account's row. Its state shows as `paused`; click **Resume** to bring it back. Both buttons require a write-tier admin session.

Directly against the API (write-tier credential required):

The `account_ref` is the opaque identifier returned on each account object by `GET /admin/api/pool`. Use it rather than the display `name`, so distinct accounts that share a name remain independently addressable.

```bash
curl -X PATCH "$SHUNT_URL/admin/api/pool/anthropic/accounts/$ACCOUNT_REF" \
  -H "x-shunt-admin-token: $ADMIN_TOKEN" \
  -H "content-type: application/json" \
  -d '{"paused": true}'
```

Set `"paused": false` to resume. See [`PATCH /admin/api/pool/{provider}/accounts/{account_ref}`](/reference/endpoints/) for the full endpoint reference.

## Ranking by soonest reset

`[server.pool] sort_by_reset` (default `false`) changes how the *available* tier is ordered: instead of largest projected burn-rate headroom, accounts sort by their earliest known quota reset (ascending — the account that recovers soonest is tried first; an account with no reset signal sorts last). The idea is to drain the account that will replenish earliest, keeping accounts with later resets in reserve as buffers.

`[server.pool]` — and therefore this setting — is process-wide, not per-provider, so toggling it affects every pooled provider at once.

Set it in `shunt.toml`:

```toml
[server.pool]
sort_by_reset = true
```

Or toggle it at runtime from the dashboard's checkbox above the pool table ("Rank available accounts by soonest quota reset instead of burn-rate headroom"), or directly:

```bash
curl -X PATCH "$SHUNT_URL/admin/api/pool" \
  -H "x-shunt-admin-token: $ADMIN_TOKEN" \
  -H "content-type: application/json" \
  -d '{"sort_by_reset": true}'
```

A runtime toggle overrides the config file's value until cleared or the process restarts, at which point the config file's own value applies again. To clear an override explicitly without restarting, send `null`:

```bash
curl -X PATCH "$SHUNT_URL/admin/api/pool" \
  -H "x-shunt-admin-token: $ADMIN_TOKEN" \
  -H "content-type: application/json" \
  -d '{"sort_by_reset": null}'
```

Omitting the field entirely is a no-op — it leaves the current override (or its absence) untouched; only an explicit `null` clears it.

`GET /admin/api/pool` reports the effective value (override or config) as a top-level `sort_by_reset` boolean. This setting has no effect at all while `[server.pool]` itself is absent — the legacy selection path it would otherwise change never runs — so both the runtime toggle and `GET /admin/api/pool`'s reported value are inert until `[server.pool]` exists.

## Ranking by soonest weekly reset

`[server.pool] sort_by_weekly_reset` (default `false`) narrows that ordering to the weekly window alone: instead of the earliest reset across all windows, available accounts sort by their governing weekly reset (`7d`, or `7d_oi` for Fable models) ascending — the account whose weekly limit replenishes soonest is tried first; an account with no weekly reset signal sorts last. The idea is to burn down the accounts closest to their weekly refill first, holding the others back. When both flags are set, the weekly-reset order wins.

Set it in `shunt.toml`:

```toml
[server.pool]
sort_by_weekly_reset = true
```

Or toggle it at runtime, directly:

```bash
curl -X PATCH "$SHUNT_URL/admin/api/pool" \
  -H "x-shunt-admin-token: $ADMIN_TOKEN" \
  -H "content-type: application/json" \
  -d '{"sort_by_weekly_reset": true}'
```

Like `sort_by_reset`, the runtime toggle overrides the config file's value until cleared or the process restarts; send `null` to clear it explicitly, and omitting the field is a no-op. Add `"provider": "<name>"` to the body to set the override for that provider only; it wins over the process-wide value. An unknown provider name returns `404` and changes nothing. `GET /admin/api/pool` reports the effective value as a top-level `sort_by_weekly_reset` boolean (and per provider, on each provider object), and the setting is likewise inert while `[server.pool]` itself is absent.

## Terminal ranking cycle

Antigravity accounts in Antigravity accounts in `shunt tui` use one row per account. The quota columns
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

In `shunt tui` (built with `--features tui`), select a provider header or one of its accounts and press `m`. With at least two accounts, the cycle is **custom order → balanced → weekly reset → custom order**:

- **Custom order → balanced** removes that provider's priorities from `shunt.toml` and sends `{"provider": "<name>", "sort_by_weekly_reset": false}` to the pool API.
- **Balanced → weekly reset** sends `{"provider": "<name>", "sort_by_weekly_reset": true}` without editing the config.
- **Weekly reset → custom order** saves the current displayed order as priorities `1, 2, 3…` for that provider. That provider's weekly-reset flag stays enabled.

The weekly-reset switch affects **only the selected provider**. Custom priorities take precedence, so that provider still shows `custom order`. Returning to balanced disables weekly reset for that provider and leaves `sort_by_reset` unchanged. Both runtime overrides end when cleared or when the gateway restarts; custom priorities survive restarts. The TUI never writes `[server.pool]`. If the config edit succeeds but the API call fails, the TUI reports the partial change and refreshes the live state.

Weekly reset requires `[server.pool]` and a gateway that supports `sort_by_weekly_reset`. The TUI reports an inactive setting if the effective value stays false.

## Choosing which models `goal` and `/delegate-cli` use

In `shunt tui`, press `M` to open the Models dialog. It has one tab each for `codex`, `opencode` and `agy`, and lists the models those CLIs offer right now. `Space` switches the highlighted model on or off, `Tab` changes platform, and `f` turns the filter on or off.

While the filter is on, the `goal` tool and `/delegate-cli` see only the models that are on, and `goal` refuses a route that names a model that is off. By default the newest version of each model family is on: `gpt-6.1-sol` is on and `gpt-6-sol` is off. Your choices are saved in `~/.shunt/models.json`. They change nothing in gateway routing, `/v1/models` or Claude Code's `/model` picker. `shunt models` prints the same list in a shell.

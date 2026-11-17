# shunt-codex

Claude Code subagents that run on the current ChatGPT/Codex model of each family —
**Sol**, **Terra**, **Astra** and **Luna** — routed through the
[shunt](https://github.com/jesusvillota/shunt) gateway. The agent list matches the
models `shunt models codex` switches on: one current model per family.

Unlike a CLI hand-off (which drops persona and preloaded skills), shunt diverts
only *token generation* at the inference layer. The session keeps running inside
Claude Code's harness: same tool loop, same skills, same script-path resolution.
Only the model that generates the tokens changes.

## Agents

| Agent (`@`-mention)   | Model id (`model:`) | Current model |
| --------------------- | ------------------- | ------------- |
| `shunt-codex:sol`     | `codex-sol`         | `gpt-6.1-sol` |
| `shunt-codex:terra`   | `codex-terra`       | `gpt-6.1-terra` |
| `shunt-codex:astra`   | `codex-astra`       | `gpt-6-astra` |
| `shunt-codex:luna`    | `codex-luna`        | `gpt-6-luna` |

Each agent's `model:` frontmatter names a model **family**, not a version. Your
`shunt.toml` maps each family id to the newest model of that family, and
`shunt sync-routes` keeps that mapping current, so a new Codex release needs no
change to these files. Set the reasoning effort per call with the Agent tool's
`effort`. Luna's top level is `max`; Luna does not accept `ultra`, and sending it
is rejected upstream.

## Prerequisites

These agents only work when a shunt gateway is running in front of Claude Code
and is configured to route the model ids above to the Codex provider:

1. **Run shunt** and point Claude Code at it:
   ```bash
   export ANTHROPIC_BASE_URL=http://127.0.0.1:3001   # shunt's default bind address
   ```
2. **Authenticate the ChatGPT/Codex subscription** shunt reuses:
   ```bash
   codex login   # writes ~/.codex/auth.json, which shunt reads + auto-refreshes
   ```
3. **Map the family ids** in your `shunt.toml` to the Codex provider. Each
   alias names the current model through `upstream_model`; `shunt sync-routes`
   (or `shunt tui`, when you switch a model on or off) moves it to the newest
   model that `shunt models codex` has on:
   ```toml
   [[routes]]
   model = "codex-sol"
   provider = "codex"
   upstream_model = "gpt-6.1-sol"

   [[routes]]
   model = "codex-terra"
   provider = "codex"
   upstream_model = "gpt-6.1-terra"

   [[routes]]
   model = "codex-astra"
   provider = "codex"
   upstream_model = "gpt-6-astra"

   [[routes]]
   model = "codex-luna"
   provider = "codex"
   upstream_model = "gpt-6-luna"
   ```

   For the full setup — auth-file handling, effort, context-window sizing, and
   troubleshooting — see the **Codex configuration reference**
   ([site guide](https://shunt.sh/guides/codex/) ·
   [`docs/codex-configuration.md`](https://github.com/pleaseai/shunt/blob/main/docs/codex-configuration.md)).

> The ChatGPT-account backend only accepts the slugs your account is entitled to.
> The canonical catalog is openai/codex's
> [`models.json`](https://github.com/openai/codex/blob/main/codex-rs/models-manager/models.json).

Without a running shunt gateway mapping these ids, Claude Code will send the
`codex-*` model id straight to Anthropic and the request will fail.

## Install

```
/plugin marketplace add jesusvillota/shunt
/plugin install shunt-codex@shunt
```

## Usage

```
@shunt-codex:sol  refactor this module and run the tests
```

Or set every subagent to a Codex model for a session with
`CLAUDE_CODE_SUBAGENT_MODEL=codex-sol`.

Both require a running shunt gateway with the slug routed to the Codex provider —
see [Prerequisites](#prerequisites). Without it the request fails against Anthropic.

## Further reading

- [Codex configuration reference](https://shunt.sh/guides/codex/) — the
  full end-to-end setup ([Markdown source](https://github.com/pleaseai/shunt/blob/main/docs/codex-configuration.md)).
- [Effort & Context](https://shunt.sh/guides/effort-and-context/) — reasoning
  effort, `count_tokens`, and the 372k context window in depth.
- [Model Discovery](https://shunt.sh/guides/model-discovery/) — auto-list
  Codex models in the `/model` picker via a `claude-`-named alias.
- [ChatGPT / Codex auth spec](https://github.com/pleaseai/shunt/blob/main/docs/m2-chatgpt-oauth.md)
  — how shunt reads and refreshes `~/.codex/auth.json`.

## License

MIT OR Apache-2.0, matching the shunt project.

# shunt-codex

Claude Code subagents that run on the current ChatGPT/Codex models — **GPT-6.1 Sol**,
**GPT-6.1 Terra**, **GPT-6 Astra** and **GPT-6 Luna** — routed through the
[shunt](https://github.com/jesusvillota/shunt) gateway. The agent list matches the
models `shunt models codex` switches on: one current model per family.

Unlike a CLI hand-off (which drops persona and preloaded skills), shunt diverts
only *token generation* at the inference layer. The session keeps running inside
Claude Code's harness: same tool loop, same skills, same script-path resolution.
Only the model that generates the tokens changes.

## Agents

| Agent (`@`-mention)          | Model id (`model:`) |
| ---------------------------- | ------------------- |
| `shunt-codex:gpt-6.1-sol`    | `gpt-6.1-sol`       |
| `shunt-codex:gpt-6.1-terra`  | `gpt-6.1-terra`     |
| `shunt-codex:gpt-6-astra`    | `gpt-6-astra`       |
| `shunt-codex:gpt-6-luna`     | `gpt-6-luna`        |

Each agent's `model:` frontmatter pins the request to a Codex slug, so only that
subagent diverts — the main session stays on Claude. Set the reasoning effort per
call with the Agent tool's `effort`. Luna's top level is `max`; Luna does not
accept `ultra`, and sending it is rejected upstream.

When a model is replaced by a newer one, add its agent file, delete the old one,
and update this table.

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
3. **Map the slugs** in your `shunt.toml` to the Codex provider (see
   [`shunt.toml.example`](https://github.com/pleaseai/shunt/blob/main/shunt.toml.example)):
   ```toml
   # [[routes]] is a TOML array-of-tables: one block per model slug.
   [[routes]]
   model = "gpt-6.1-sol"
   provider = "codex"

   [[routes]]
   model = "gpt-6.1-terra"
   provider = "codex"

   [[routes]]
   model = "gpt-6-astra"
   provider = "codex"

   [[routes]]
   model = "gpt-6-luna"
   provider = "codex"
   ```

   For the full setup — auth-file handling, effort, context-window sizing, and
   troubleshooting — see the **Codex configuration reference**
   ([site guide](https://shunt.sh/guides/codex/) ·
   [`docs/codex-configuration.md`](https://github.com/pleaseai/shunt/blob/main/docs/codex-configuration.md)).

> The ChatGPT-account backend only accepts the slugs your account is entitled to.
> The canonical catalog is openai/codex's
> [`models.json`](https://github.com/openai/codex/blob/main/codex-rs/models-manager/models.json).

Without a running shunt gateway mapping these ids, Claude Code will send the
`gpt-6*` model id straight to Anthropic and the request will fail.

## Install

```
/plugin marketplace add jesusvillota/shunt
/plugin install shunt-codex@shunt
```

## Usage

```
@shunt-codex:gpt-6.1-sol  refactor this module and run the tests
```

Or set every subagent to a Codex model for a session with
`CLAUDE_CODE_SUBAGENT_MODEL=gpt-6.1-sol`.

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

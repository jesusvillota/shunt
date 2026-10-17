---
id: TASK-005
title: Select the ranker as platform, model and effort
milestone: M-4
state: approved
depends_on: []
delivery: pr
blocked: null
---

## Why

Serves SC-4. Today `ranker` in `~/.shunt/models.json` is only a platform name (`src/models_prefs.rs`, `src/rerank.rs::command`), so each CLI uses its own default model and effort. The owner wants to choose platform, model and effort for the ranking agent.

## Scope

Build on TASK-004 (`src/rerank.rs`, `src/models_prefs.rs`, `src/tui/models.rs`, `src/tui/view/models.rs`).

- `ranker` becomes `{platform, model, effort}`. `model` and `effort` are optional. The old string form (`"ranker": "claude"`) must still load and means `{platform: "claude"}` with no model and no effort. Write the object form once the owner picks something; never rewrite a string the owner did not change.
- Platforms stay `claude`, `codex`, `opencode`, `agy`; `chatgptweb` stays refused with the same message. `agy` keeps `AGY_MODEL` as its default model but now accepts a model override.
- `rerank::command(ranker, prompt)` adds the model and effort flags for each CLI. Read each CLI's `--help` first and use its real flags (for example `claude --model/--effort`, `codex exec -m` with `-c model_reasoning_effort=…`, `opencode run -m` with `--variant`). Where a CLI has no effort flag, omit it and document that the effort is ignored for that platform. Never invent a flag.
- The Models dialog gets a ranker picker (a key shown in its footer, not `r`): it lists platform, then the models of that platform from the existing catalog (`models_catalog.rs`) with their effort levels, and stores the choice in `~/.shunt/models.json`. The `opencode` tab shows the current ranker as `platform · model · effort`.
- Tests use the injectable `Runner`: the argv for each platform with and without model and effort, the string form still loading, the object form round-tripping, and `chatgptweb` refused. Tests never call a real model.
- Update the docs it affects in the same PR: `docs/pool-tui.md`, `README.md` + `README.ko.md`/`README.ja.md`/`README.zh-CN.md`, and the site pages with their ko/ja/zh-cn copies, per `AGENTS.md`.

Out of scope: the scoring change (TASK-006); any change to gateway routing; the goal tool.

## Acceptance

- `"ranker": "claude"` in `models.json` still loads and runs exactly as before (a test shows the argv equals the TASK-004 argv).
- `"ranker": {"platform": "codex", "model": "gpt-6-sol", "effort": "high"}` produces an argv that carries that model and effort in the CLI's real flags; a test asserts it for all four platforms, and names the platforms where the effort is ignored.
- The picker stores the choice, and the `opencode` tab shows `platform · model · effort`.
- `chatgptweb` is still refused with a clear message.
- Docs are updated in all four languages.
- fmt, clippy (`--features tui,bench`) and the full tests pass.

## Verify

```sh
cargo fmt --all --check
RUSTFLAGS="-D warnings" cargo clippy --all-targets --features tui,bench -- -D warnings
RUSTFLAGS="-D warnings" cargo test --features tui,bench --workspace
cargo test --features tui --lib rerank
cargo run -q -- models opencode --json
```

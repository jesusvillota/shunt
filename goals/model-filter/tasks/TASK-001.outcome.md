## Acceptance
- [x] `shunt models --json` prints `filter` and, per platform, models with `id`, `active` and `reason` — ran `./target/debug/shunt models agy --json`; it returned `filter: true` and the five agy ids with `active: true`.
- [x] With no overrides `gpt-6.1-sol` is on, `gpt-6-sol` is off; effort tiers of the newest version stay on; unversioned names stay on — `shunt models` against the real config showed `[on ] gpt-6.1-sol`, `[off] gpt-6-sol`, `[on ] opencode/big-pickle`; unit tests `models_prefs::tests::*` pass.
- [x] The Models dialog (`M`) lists each platform, `Space` toggles, `f` flips the filter — unit tests `tui::models::tests::*` and the render test `models_dialog_shows_the_filter_switch_tabs_and_each_models_state` pass. The dialog was not driven in a real terminal.
- [x] fmt, clippy and the full tests pass — `cargo fmt --all --check` ok; `cargo clippy --all-targets --features tui,bench -- -D warnings` ok; `cargo test --features tui,bench --workspace`: 3788 passed, 0 failed.
- [x] Docs updated in all four languages — README (+ko/ja/zh-CN), `docs/pool-tui.md`, site `reference/cli.mdx` and `guides/pool-account-controls.md` with ko/ja/zh-cn copies.
- [ ] `--all-features` clippy/test — not run: it needs a built `ui/dist` and a Python with torch.

## Summary
Adds `src/models_prefs.rs`, `src/models_catalog.rs`, the `shunt models` command and the Models dialog in `shunt tui`. State lives in `~/.shunt/models.json` (overrides only). Verified at the PR head of this branch. Deviation from the plan: `shunt models --json` has no `--all` flag; every model is always listed with `active` and the consumer applies `filter`. The docs say `goal` refuses a switched-off model; that ships in the `model-filter-consumer` goal in the config repo. Follow-up: without `--features tui`, clippy on tests already fails on an unused import in `src/routing/stage/store.rs:50` (not touched here).

---
id: TASK-008
title: Remove the shunt top alias; only shunt tui remains
milestone: M-1
state: approved
depends_on: [TASK-007]
delivery: pr
---

## Why

`shunt top` is a deprecated fallback name for `shunt tui`. The owner wants only `shunt tui` to work, so docs and CLI name one command.

## Scope

Remove the `top` alias and rename every user-facing mention to `shunt tui`:

- `src/main.rs:74`: drop `visible_alias = "top"` from the `tui` command.
- `packaging/shunt-launcher.sh:24`: the `tui | top)` case becomes `tui)`.
- Docs: `site/src/content/docs/reference/cli.mdx` (the `## shunt top` heading and example, plus any anchor links to it) and the ko/ja/zh-cn copies; `README.ko.md`, `README.ja.md`, `README.zh-CN.md` and `README.md` if they say `shunt top`; `docs/pool-tui.md`. Take new heading ids from the built `site/dist` as AGENTS.md says, and fix any cross-page fragment links.
- `tests/tui_client.rs:1` doc comment, and any other `shunt top` text found with `git grep`.
- Run `git grep -nE "shunt top|visible_alias = \"top\""` at the end; excluding `wiki/` (generated, never hand-edit) it must print nothing.

Out of scope: any behavior change of the TUI, `wiki/`, other commands.

## Acceptance

- `shunt tui --help` works; `shunt top` exits non-zero with clap's unrecognized-subcommand error (add a test or paste the output).
- The grep above prints nothing outside `wiki/`.
- Site builds without dead links in the four locales (paste the build tail).
- `cargo fmt --all --check`, `cargo clippy --all-targets --all-features -- -D warnings` and `cargo test --all-features --workspace` pass.

## Verify

`cargo fmt --all --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all-features --workspace`

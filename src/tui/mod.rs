//! `shunt top` — a live terminal view of the managed account pool.
//!
//! It is a client of a *running* gateway's admin API (`GET /admin/api/pool`,
//! `PATCH /admin/api/pool[/…]`), not a second implementation of pool state, so
//! what it shows is exactly what the web dashboard shows and what the
//! scheduler acts on. See `docs/pool-tui.md`.

mod app;
pub mod client;
pub mod model;
mod view;

use std::{path::PathBuf, sync::Arc, time::Duration};

use anyhow::{bail, Context};
use ratatui::crossterm::event::{self, Event};
use tokio::sync::{mpsc, Notify};

use app::{App, Effect, Notice};
use client::Client;
use model::Snapshot;

/// Polling floor. The pool snapshot is in-memory and cheap, but a sub-second
/// loop buys nothing the eye can follow and would hammer the admin surface.
const MIN_INTERVAL_MS: u64 = 500;

pub struct Options {
    pub url: Option<String>,
    pub token: Option<String>,
    pub header: String,
    /// Poll interval in milliseconds.
    pub interval_ms: u64,
    pub config: Option<PathBuf>,
}

enum Msg {
    Input(Event),
    Pool(Result<Snapshot, String>),
    Done(Result<String, String>),
    Tick,
}

/// The first usable credential in `name:token` pair text (one per line or
/// comma-separated, `#` comments allowed) — the format `shunt dashboard setup`
/// writes to `~/.shunt/admin-token`.
fn first_token(text: &str) -> Option<String> {
    text.lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .flat_map(|line| line.split(','))
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            entry
                .split_once(':')
                .map_or(entry, |(_, token)| token)
                .trim()
        })
        .find(|token| !token.is_empty())
        .map(str::to_string)
}

fn resolve_token(explicit: Option<String>) -> anyhow::Result<String> {
    let env = |name: &str| std::env::var(name).ok().filter(|v| !v.trim().is_empty());
    if let Some(token) = explicit.filter(|t| !t.trim().is_empty()) {
        return Ok(token);
    }
    if let Some(token) = env("SHUNT_ADMIN_TOKEN") {
        return Ok(token.trim().to_string());
    }
    if let Some(token) = env("SHUNT_ADMIN_TOKENS").as_deref().and_then(first_token) {
        return Ok(token);
    }
    if let Some(path) = crate::config::default_admin_token_file() {
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Some(token) = first_token(&text) {
                return Ok(token);
            }
        }
    }
    bail!(
        "no admin token found. Pass --token, set SHUNT_ADMIN_TOKEN, or run \
         `shunt dashboard setup` (writes ~/.shunt/admin-token)"
    )
}

pub fn run(options: Options) -> anyhow::Result<()> {
    let token = resolve_token(options.token)?;
    let base = options
        .url
        .unwrap_or_else(|| crate::dashboard::gateway_base_url(options.config.as_deref()));
    let client = Client::new(&base, &options.header, &token)?;
    let interval = Duration::from_millis(options.interval_ms.max(MIN_INTERVAL_MS));

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .context("starting async runtime")?;
    // `ratatui::init` installs a panic hook that restores the terminal, so a
    // bug here cannot leave the shell in raw mode.
    let mut terminal = ratatui::init();
    let result = runtime.block_on(event_loop(&mut terminal, client, interval));
    ratatui::restore();
    result
}

async fn event_loop(
    terminal: &mut ratatui::DefaultTerminal,
    client: Client,
    interval: Duration,
) -> anyhow::Result<()> {
    let (tx, mut rx) = mpsc::unbounded_channel::<Msg>();
    let refresh = Arc::new(Notify::new());

    // Input lives on its own OS thread: crossterm's reader blocks, and polling
    // it from the async loop would stall the poller and redraws behind it.
    let input_tx = tx.clone();
    std::thread::spawn(move || {
        while let Ok(ready) = event::poll(Duration::from_millis(250)) {
            if ready {
                match event::read() {
                    Ok(ev) => {
                        if input_tx.send(Msg::Input(ev)).is_err() {
                            return;
                        }
                    }
                    Err(_) => return,
                }
            } else if input_tx.send(Msg::Tick).is_err() {
                return;
            }
        }
    });

    let poller = {
        let (client, tx, refresh) = (client.clone(), tx.clone(), refresh.clone());
        tokio::spawn(async move {
            loop {
                let result = client.fetch_pool().await.map_err(|e| format!("{e:#}"));
                if tx.send(Msg::Pool(result)).is_err() {
                    return;
                }
                // Wake early on an explicit refresh or after a mutation, so the
                // table reflects an action without waiting out the interval.
                let _ = tokio::time::timeout(interval, refresh.notified()).await;
            }
        })
    };

    let mut app = App::new(client.base().to_string(), interval);
    let outcome = loop {
        terminal.draw(|frame| view::render(frame, &app, view::now_secs()))?;
        let Some(msg) = rx.recv().await else {
            break Ok(());
        };
        match msg {
            Msg::Tick => {}
            Msg::Pool(result) => app.on_poll(result),
            Msg::Done(Ok(text)) => app.notify(Notice::Info(text)),
            Msg::Done(Err(text)) => app.notify(Notice::Error(text)),
            Msg::Input(Event::Key(key)) => match app.on_key(key) {
                Effect::None => {}
                Effect::Quit => break Ok(()),
                Effect::Refresh => refresh.notify_one(),
                Effect::SetPaused {
                    provider,
                    account_ref,
                    paused,
                } => {
                    let label = app
                        .selected_row()
                        .map_or_else(String::new, |row| row.account.name.clone());
                    let (client, tx, refresh) = (client.clone(), tx.clone(), refresh.clone());
                    tokio::spawn(async move {
                        let result = client
                            .set_paused(&provider, &account_ref, paused)
                            .await
                            .map(|()| {
                                format!("{} {label}", if paused { "paused" } else { "resumed" })
                            })
                            .map_err(|e| format!("{e:#}"));
                        let _ = tx.send(Msg::Done(result));
                        refresh.notify_one();
                    });
                }
                Effect::SetSortByReset(value) => {
                    let (client, tx, refresh) = (client.clone(), tx.clone(), refresh.clone());
                    tokio::spawn(async move {
                        let result = client
                            .set_sort_by_reset(value)
                            .await
                            .map(|()| {
                                format!(
                                    "gateway now ranks by {}",
                                    if value {
                                        "soonest reset"
                                    } else {
                                        "burn-rate headroom"
                                    }
                                )
                            })
                            .map_err(|e| format!("{e:#}"));
                        let _ = tx.send(Msg::Done(result));
                        refresh.notify_one();
                    });
                }
            },
            Msg::Input(_) => {}
        }
    };
    poller.abort();
    outcome
}

#[cfg(test)]
mod tests {
    use super::first_token;

    #[test]
    fn first_token_reads_the_setup_file_format() {
        assert_eq!(first_token("ops:abc123\n").as_deref(), Some("abc123"));
        assert_eq!(
            first_token("# generated\n\nops:abc, other:def\n").as_deref(),
            Some("abc")
        );
        assert_eq!(first_token("baretoken").as_deref(), Some("baretoken"));
        assert_eq!(first_token("  \n# only a comment\n"), None);
        assert_eq!(first_token("ops:\n"), None);
    }
}

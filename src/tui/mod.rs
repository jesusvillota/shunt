//! `shunt tui` — a live terminal view of the managed account pool.
//!
//! It is a client of a *running* gateway's admin API (`GET /admin/api/pool`,
//! `PATCH /admin/api/pool[/…]`), not a second implementation of pool state, so
//! what it shows is exactly what the web dashboard shows and what the
//! scheduler acts on. See `docs/pool-tui.md`.

mod add;
mod app;
pub mod client;
mod config_edit;
pub mod model;
mod models;
mod prefs;
mod view;

use std::{
    io::{stdout, Write},
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};

use anyhow::Context;
use ratatui::crossterm::{
    event::{
        self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        Event,
    },
    execute,
};
use tokio::sync::{mpsc, Mutex, Notify};

use app::{App, Effect, Notice};
use client::Client;
use model::Snapshot;

/// Polling floor. The pool snapshot is in-memory and cheap, but a sub-second
/// loop buys nothing the eye can follow and would hammer the admin surface.
const MIN_INTERVAL_MS: u64 = 500;
/// The gateway debounces config-file changes for 400 ms before it reloads; a
/// second refresh this long after a write catches the reloaded state.
const RELOAD_SETTLE: Duration = Duration::from_millis(1200);
/// How often the OpenCode log status is reread while the Models dialog is open.
const OPENCODE_REFRESH: Duration = Duration::from_secs(10);

pub struct Options {
    pub url: Option<String>,
    pub token: Option<String>,
    pub header: String,
    /// Poll interval in milliseconds.
    pub interval_ms: u64,
    /// The config file the gateway runs, for the edits `shunt tui` makes
    /// (added accounts, ranking). Defaults to the loader's usual search.
    pub config: Option<PathBuf>,
}

enum Msg {
    Input(Event),
    Pool(Result<Snapshot, String>),
    Notice(Notice),
    WeeklyResetSet(String, bool),
    RanksCleared(String),
    ModelsOpened(crate::models_prefs::ModelPrefs),
    ModelsLoaded(
        String,
        Result<Vec<crate::models_catalog::RankerModel>, String>,
    ),
    OpencodeStatus(Result<crate::opencode_status::Table, String>),
    Ranked(Result<crate::models_prefs::OpenCodeRank, String>),
    AddStarted(Result<String, String>),
    /// `Err`: the gateway refused the code (the dialog stays open to retry).
    /// `Ok`: the account is stored; the notice says whether the config edit worked too.
    AddCompleted(Result<Notice, String>),
    Tick,
}

use crate::pool::resolve_token;

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
    // `ratatui::init` installs a panic hook that restores the terminal; ours
    // additionally turns off the two modes it does not know about.
    let mut terminal = ratatui::init();
    let _ = execute!(stdout(), EnableMouseCapture, EnableBracketedPaste);
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = execute!(stdout(), DisableMouseCapture, DisableBracketedPaste);
        previous(info);
    }));
    let result = runtime.block_on(event_loop(&mut terminal, client, interval, options.config));
    let _ = execute!(stdout(), DisableMouseCapture, DisableBracketedPaste);
    ratatui::restore();
    result
}

/// Ask the OS to open `url` in the default browser. Best effort: over SSH or
/// without a desktop there is nothing to open it, and the dialog shows the URL
/// to copy anyway.
fn open_browser(url: &str) {
    use std::process::{Command, Stdio};
    let mut command = if cfg!(target_os = "macos") {
        let mut c = Command::new("open");
        c.arg(url);
        c
    } else if cfg!(windows) {
        let mut c = Command::new("rundll32");
        c.args(["url.dll,FileProtocolHandler", url]);
        c
    } else {
        let mut c = Command::new("xdg-open");
        c.arg(url);
        c
    };
    let _ = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

/// Copy through the terminal itself (OSC 52), which also works over SSH. Terminals
/// that do not support it ignore the sequence.
fn copy_to_clipboard(text: &str) {
    use base64::Engine as _;
    let encoded = base64::engine::general_purpose::STANDARD.encode(text);
    let _ = write!(stdout(), "\x1b]52;c;{encoded}\x07");
    let _ = stdout().flush();
}

/// Everything a spawned task needs to report back and trigger a refresh.
#[derive(Clone)]
struct Ctx {
    client: Client,
    tx: mpsc::UnboundedSender<Msg>,
    refresh: Arc<Notify>,
    config: Option<PathBuf>,
    /// Config edits are read-modify-write; two in flight would lose one.
    edit_lock: Arc<Mutex<()>>,
}

impl Ctx {
    fn say(&self, notice: Notice) {
        let _ = self.tx.send(Msg::Notice(notice));
    }

    /// Refresh now, and again once the gateway has had time to reload a config
    /// change.
    fn refresh_soon(&self, after_config_write: bool) {
        self.refresh.notify_one();
        if after_config_write {
            let refresh = self.refresh.clone();
            tokio::spawn(async move {
                tokio::time::sleep(RELOAD_SETTLE).await;
                refresh.notify_one();
            });
        }
    }

    /// Edit the config file off the async threads, one edit at a time.
    async fn edit_config(
        &self,
        edit: impl FnOnce(&str) -> anyhow::Result<Option<String>> + Send + 'static,
    ) -> anyhow::Result<PathBuf> {
        let _guard = self.edit_lock.lock().await;
        let path = config_edit::locate(self.config.as_deref())?;
        let target = path.clone();
        tokio::task::spawn_blocking(move || config_edit::apply(&target, edit))
            .await
            .context("config edit task failed")??;
        Ok(path)
    }
}

fn file_name(path: &std::path::Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

fn perform(ctx: &Ctx, effect: Effect) {
    let ctx = ctx.clone();
    match effect {
        Effect::None | Effect::Quit | Effect::MouseCapture(_) => {}
        Effect::CopyToClipboard(text) => {
            copy_to_clipboard(&text);
            ctx.say(Notice::Info(
                "link sent to the clipboard (if your terminal allows it)".into(),
            ));
        }
        Effect::SetPaused {
            provider,
            account_ref,
            label,
            paused,
        } => {
            tokio::spawn(async move {
                let notice = match ctx.client.set_paused(&provider, &account_ref, paused).await {
                    Ok(()) => Notice::Info(format!(
                        "{} {label}",
                        if paused { "paused" } else { "resumed" }
                    )),
                    Err(e) => Notice::Error(format!("{e:#}")),
                };
                ctx.say(notice);
                ctx.refresh_soon(false);
            });
        }
        Effect::SetProvider {
            provider,
            on,
            account_refs,
        } => {
            tokio::spawn(async move {
                let mut done = 0;
                let mut notice = None;
                for account_ref in &account_refs {
                    match ctx.client.set_paused(&provider, account_ref, !on).await {
                        Ok(()) => done += 1,
                        Err(e) => {
                            notice = Some(Notice::Error(format!("{e:#}")));
                            break;
                        }
                    }
                }
                ctx.say(notice.unwrap_or_else(|| {
                    Notice::Info(format!(
                        "{provider} switched {} ({done} account{})",
                        if on { "on" } else { "off" },
                        if done == 1 { "" } else { "s" }
                    ))
                }));
                ctx.refresh_soon(false);
            });
        }
        Effect::SetRanks {
            provider,
            ranks,
            pool_names,
        } => {
            tokio::spawn(async move {
                let target = provider.clone();
                let result = ctx
                    .edit_config(move |text| {
                        config_edit::set_priorities(text, &target, &ranks, &pool_names)
                    })
                    .await;
                ctx.say(match result {
                    Ok(path) => {
                        Notice::Info(format!("{provider}: ranking saved to {}", file_name(&path)))
                    }
                    Err(e) => Notice::Error(format!("{e:#}")),
                });
                ctx.refresh_soon(true);
            });
        }
        Effect::ClearRanks { provider } => {
            tokio::spawn(async move {
                let target = provider.clone();
                let result = ctx
                    .edit_config(move |text| config_edit::clear_priorities(text, &target))
                    .await;
                ctx.say(match result {
                    Ok(path) => match ctx.client.set_sort_by_weekly_reset(Some(&provider), Some(false)).await {
                        Ok(()) => {
                            let _ = ctx.tx.send(Msg::RanksCleared(provider.clone()));
                            Notice::Info(format!(
                                "{provider}: balanced ({} updated); weekly reset off",
                                file_name(&path)
                            ))
                        }
                        Err(e) => Notice::Error(format!(
                            "{provider}: priorities cleared in {}, but weekly reset could not be disabled: {e:#}",
                            file_name(&path)
                        )),
                    },
                    Err(e) => Notice::Error(format!("{e:#}")),
                });
                ctx.refresh_soon(true);
            });
        }
        Effect::SetWeeklyReset {
            provider,
            on: value,
        } => {
            tokio::spawn(async move {
                let notice = match ctx
                    .client
                    .set_sort_by_weekly_reset(Some(&provider), Some(value))
                    .await
                {
                    Ok(()) => match ctx.client.fetch_pool().await {
                        Ok(snapshot) => {
                            let effective =
                                snapshot.provider(&provider).is_some_and(|p| p.weekly_reset);
                            let _ = ctx.tx.send(Msg::Pool(Ok(snapshot)));
                            let _ = ctx
                                .tx
                                .send(Msg::WeeklyResetSet(provider.clone(), effective));
                            if value && !effective {
                                Notice::Info(
                                    "weekly reset is inactive; configure [server.pool] and use a gateway with weekly-reset support".into(),
                                )
                            } else {
                                Notice::Info(format!(
                                    "{provider}: weekly reset {} (until gateway restart)",
                                    if effective { "on" } else { "off" }
                                ))
                            }
                        }
                        Err(e) => Notice::Error(format!(
                            "weekly-reset setting updated, but refreshing the pool failed: {e:#}"
                        )),
                    },
                    Err(e) => Notice::Error(format!("{e:#}")),
                };
                ctx.say(notice);
                ctx.refresh_soon(false);
            });
        }
        Effect::Delete {
            provider,
            kind,
            name,
        } => {
            tokio::spawn(async move {
                // Validate the config transform before deleting the credential.
                // In particular, removing the last explicit account is unsafe:
                // an empty list means "scan every stored account".
                let (guard_provider, guard_name) = (provider.clone(), name.clone());
                if let Err(e) = ctx
                    .edit_config(move |text| {
                        config_edit::remove_account(text, &guard_provider, &guard_name)?;
                        Ok(None)
                    })
                    .await
                {
                    ctx.say(Notice::Error(format!("cannot safely delete {name}: {e:#}")));
                    return;
                }

                if let Err(e) = ctx.client.delete_account(kind, &name).await {
                    ctx.say(Notice::Error(format!("{e:#}")));
                    ctx.refresh_soon(false);
                    return;
                }

                let (target_provider, target_name) = (provider.clone(), name.clone());
                let config_result = ctx
                    .edit_config(move |text| {
                        config_edit::remove_account(text, &target_provider, &target_name)
                    })
                    .await;
                ctx.say(match config_result {
                    Ok(_) => Notice::Info(format!("deleted {name} from {provider}")),
                    Err(e) => Notice::Error(format!(
                        "{name} was deleted, but shunt.toml was not updated: {e:#}"
                    )),
                });
                ctx.refresh_soon(true);
            });
        }
        Effect::OpenModels => {
            let config = ctx.config.clone();
            tokio::spawn(async move {
                let prefs = crate::models_prefs::load();
                let _ = ctx.tx.send(Msg::ModelsOpened(prefs));
                // One blocking fetch per platform, so a slow CLI never holds up the others.
                for platform in crate::models_catalog::RANKER_PLATFORMS {
                    let (tx, config) = (ctx.tx.clone(), config.clone());
                    tokio::task::spawn_blocking(move || {
                        let result =
                            crate::models_catalog::fetch_ranker(platform, config.as_deref());
                        let _ = tx.send(Msg::ModelsLoaded(platform.into(), result));
                    });
                }
            });
        }
        Effect::SaveModels(prefs) => {
            // Queue here, in key order; the blocking task writes the newest snapshot.
            crate::models_prefs::queue(prefs);
            tokio::spawn(async move {
                match tokio::task::spawn_blocking(crate::models_prefs::flush).await {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => ctx.say(Notice::Error(format!("model list not saved: {e:#}"))),
                    Err(e) => ctx.say(Notice::Error(format!("model list not saved: {e}"))),
                }
            });
        }
        Effect::RerankOpencode { ids, ranker } => {
            tokio::spawn(async move {
                let result = tokio::task::spawn_blocking(move || {
                    let path = crate::rerank::default_criteria_path()
                        .ok_or_else(|| "no home directory for the criteria file".to_string())?;
                    let previous = crate::models_prefs::load()
                        .opencode_rank
                        .map(|r| r.order)
                        .unwrap_or_default();
                    let rank = crate::rerank::run(
                        &path,
                        &ranker,
                        &ids,
                        &previous,
                        &crate::rerank::ShellRunner,
                        SystemTime::now(),
                    )?;
                    let mut prefs = crate::models_prefs::load();
                    prefs.opencode_rank = Some(rank.clone());
                    crate::models_prefs::save(&prefs).map_err(|e| format!("{e:#}"))?;
                    Ok::<_, String>(rank)
                })
                .await;
                let result = match result {
                    Ok(result) => result,
                    Err(error) => Err(format!("ranking task failed: {error}")),
                };
                let _ = ctx.tx.send(Msg::Ranked(result));
            });
        }
        Effect::SaveDisplayPrefs { hidden, order } => {
            // Best effort: losing display prefs must never interrupt monitoring.
            tokio::spawn(async move {
                let prefs = prefs::DisplayPrefs { hidden, order };
                if let Err(e) = prefs::save(&prefs) {
                    ctx.say(Notice::Error(format!("display prefs not saved: {e:#}")));
                }
            });
        }
        Effect::Add(add::Effect::Start { target, name }) => {
            tokio::spawn(async move {
                let result = ctx.client.start_account(target.kind, &name).await;
                if let Ok(url) = &result {
                    open_browser(url);
                }
                let _ = ctx
                    .tx
                    .send(Msg::AddStarted(result.map_err(|e| format!("{e:#}"))));
            });
        }
        Effect::Add(add::Effect::Complete { target, name, code }) => {
            tokio::spawn(async move {
                if let Err(e) = ctx.client.complete_account(target.kind, &name, &code).await {
                    let _ = ctx.tx.send(Msg::AddCompleted(Err(format!("{e:#}"))));
                    return;
                }
                // The credentials are stored. Make sure the pool includes the
                // account — a provider that lists accounts explicitly would
                // otherwise ignore it.
                let (provider, account) = (target.provider.clone(), name.clone());
                let notice = match ctx
                    .edit_config(move |text| {
                        config_edit::include_account(text, &provider, &account)
                    })
                    .await
                {
                    Ok(path) => Notice::Info(format!(
                        "added {name} to {} (config: {})",
                        target.provider,
                        file_name(&path)
                    )),
                    Err(e) => Notice::Error(format!(
                        "{name} was stored, but shunt.toml was not updated: {e:#}"
                    )),
                };
                let _ = ctx.tx.send(Msg::AddCompleted(Ok(notice)));
                ctx.refresh_soon(true);
            });
        }
        Effect::Add(_) => {}
    }
}

async fn event_loop(
    terminal: &mut ratatui::DefaultTerminal,
    client: Client,
    interval: Duration,
    config: Option<PathBuf>,
) -> anyhow::Result<()> {
    let (tx, mut rx) = mpsc::unbounded_channel::<Msg>();
    let refresh = Arc::new(Notify::new());
    let ctx = Ctx {
        client: client.clone(),
        tx: tx.clone(),
        refresh: refresh.clone(),
        config,
        edit_lock: Arc::new(Mutex::new(())),
    };

    // Input lives on its own OS thread: crossterm's reader blocks, and polling
    // it from the async loop would stall the poller and redraws behind it.
    let input_tx = tx.clone();
    std::thread::spawn(move || {
        while let Ok(ready) = event::poll(Duration::from_millis(250)) {
            let sent = if ready {
                match event::read() {
                    Ok(ev) => input_tx.send(Msg::Input(ev)),
                    Err(_) => return,
                }
            } else {
                input_tx.send(Msg::Tick)
            };
            if sent.is_err() {
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

    let mut app = App::new(client.base().to_string());
    app.apply_prefs(prefs::load());
    let mut mouse_on = true;
    // The OpenCode log is read only while the Models dialog is open, at most
    // once per `OPENCODE_REFRESH`.
    let mut opencode_at: Option<Instant> = None;
    let outcome = loop {
        terminal.draw(|frame| view::render(frame, &mut app, view::now_secs()))?;
        let Some(msg) = rx.recv().await else {
            break Ok(());
        };
        let effect = match msg {
            Msg::Tick => {
                if app.models.is_some()
                    && opencode_at.is_none_or(|at| at.elapsed() >= OPENCODE_REFRESH)
                {
                    opencode_at = Some(Instant::now());
                    let tx = tx.clone();
                    tokio::task::spawn_blocking(move || {
                        let result = crate::opencode_status::read(SystemTime::now());
                        let _ = tx.send(Msg::OpencodeStatus(result));
                    });
                }
                Effect::None
            }
            Msg::Pool(result) => {
                app.on_poll(result);
                Effect::None
            }
            Msg::Notice(notice) => {
                app.notify(notice);
                Effect::None
            }
            Msg::WeeklyResetSet(provider, value) => {
                app.on_weekly_reset_set(&provider, value);
                Effect::None
            }
            Msg::ModelsOpened(prefs) => {
                app.on_models_opened(prefs);
                opencode_at = None;
                Effect::None
            }
            Msg::ModelsLoaded(platform, result) => {
                app.on_models_loaded(&platform, result);
                Effect::None
            }
            Msg::OpencodeStatus(result) => {
                app.on_opencode_status(result);
                Effect::None
            }
            Msg::Ranked(result) => {
                app.on_ranked(result);
                Effect::None
            }
            Msg::RanksCleared(provider) => {
                app.on_ranks_cleared(&provider);
                Effect::None
            }
            Msg::AddStarted(result) => {
                if let Some(dialog) = &mut app.dialog {
                    dialog.on_started(result);
                }
                app.sync_mouse()
            }
            Msg::AddCompleted(Ok(notice)) => {
                app.notify(notice);
                app.close_dialog()
            }
            Msg::AddCompleted(Err(error)) => {
                if let Some(dialog) = &mut app.dialog {
                    dialog.on_completed(Err(error));
                }
                app.sync_mouse()
            }
            Msg::Input(Event::Key(key)) => app.on_key(key),
            Msg::Input(Event::Mouse(mouse)) => app.on_mouse(mouse),
            Msg::Input(Event::Paste(text)) => {
                app.on_paste(&text);
                Effect::None
            }
            Msg::Input(_) => Effect::None,
        };
        match effect {
            Effect::Quit => break Ok(()),
            // Turning capture off lets the operator select the authorize URL
            // with the mouse; it only toggles when the wanted state changes.
            Effect::MouseCapture(wanted) => {
                if wanted != mouse_on {
                    mouse_on = wanted;
                    let _ = if wanted {
                        execute!(stdout(), EnableMouseCapture)
                    } else {
                        execute!(stdout(), DisableMouseCapture)
                    };
                    let _ = stdout().flush();
                }
            }
            other => perform(&ctx, other),
        }
    };
    poller.abort();
    outcome
}

#[cfg(test)]
mod runtime_tests;

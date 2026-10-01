//! Monitor state and key handling. `App::on_key` is a pure reducer that hands
//! back an [`Effect`] for the runtime to perform, so every keybinding is
//! testable without a terminal or a gateway.

use std::time::{Duration, Instant};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use super::model::{Row, Snapshot, SortKey};

/// How long a status-bar notice stays up.
const NOTICE_TTL: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    None,
    Quit,
    Refresh,
    SetPaused {
        provider: String,
        account_ref: String,
        paused: bool,
    },
    SetSortByReset(bool),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    Info(String),
    Error(String),
}

pub struct App {
    pub snapshot: Snapshot,
    pub base_url: String,
    pub interval: Duration,
    /// Last successful poll. `None` until the first one lands.
    pub last_ok: Option<Instant>,
    /// The most recent poll failure; cleared by the next success.
    pub poll_error: Option<String>,
    pub sort: SortKey,
    pub descending: bool,
    /// Index into `snapshot.providers()`; `None` shows every provider.
    pub filter: Option<usize>,
    pub show_help: bool,
    pub notice: Option<(Notice, Instant)>,
    /// The selection follows the account, not the row index, so a re-sort or a
    /// poll that reorders the table does not move the cursor onto another one.
    selected: Option<(String, String)>,
}

fn key_of(row: &Row) -> (String, String) {
    (
        row.provider.clone(),
        row.account_ref().unwrap_or(&row.account.name).to_string(),
    )
}

impl App {
    pub fn new(base_url: String, interval: Duration) -> Self {
        Self {
            snapshot: Snapshot::default(),
            base_url,
            interval,
            last_ok: None,
            poll_error: None,
            sort: SortKey::Default,
            descending: false,
            filter: None,
            show_help: false,
            notice: None,
            selected: None,
        }
    }

    pub fn filter_name(&self) -> Option<String> {
        self.filter
            .and_then(|at| self.snapshot.providers().into_iter().nth(at))
    }

    pub fn rows(&self) -> Vec<&Row> {
        let filter = self.filter_name();
        self.snapshot
            .view(filter.as_deref(), self.sort, self.descending)
    }

    /// Index of the selected account within [`Self::rows`], falling back to the
    /// first row when the selected account vanished from the pool.
    pub fn selected_index(&self) -> Option<usize> {
        let rows = self.rows();
        if rows.is_empty() {
            return None;
        }
        let at = self
            .selected
            .as_ref()
            .and_then(|key| rows.iter().position(|row| &key_of(row) == key));
        Some(at.unwrap_or(0))
    }

    pub fn selected_row(&self) -> Option<&Row> {
        let at = self.selected_index()?;
        self.rows().get(at).copied()
    }

    fn select(&mut self, at: usize) {
        let key = self.rows().get(at).map(|row| key_of(row));
        if key.is_some() {
            self.selected = key;
        }
    }

    fn move_by(&mut self, delta: isize) {
        let len = self.rows().len();
        let Some(at) = self.selected_index() else {
            return;
        };
        let next = at.saturating_add_signed(delta).min(len - 1);
        self.select(next);
    }

    pub fn on_poll(&mut self, result: Result<Snapshot, String>) {
        match result {
            Ok(snapshot) => {
                // Pin the cursor to its account *before* the table changes
                // underneath it, in case it had defaulted to row 0.
                if self.selected.is_none() {
                    if let Some(at) = self.selected_index() {
                        self.select(at);
                    }
                }
                self.snapshot = snapshot;
                // A provider that disappeared must not leave a dangling filter.
                if self
                    .filter
                    .is_some_and(|at| at >= self.snapshot.providers().len())
                {
                    self.filter = None;
                }
                self.last_ok = Some(Instant::now());
                self.poll_error = None;
            }
            Err(error) => self.poll_error = Some(error),
        }
    }

    pub fn notify(&mut self, notice: Notice) {
        self.notice = Some((notice, Instant::now()));
    }

    /// The live notice, if it has not expired.
    pub fn current_notice(&self) -> Option<&Notice> {
        self.notice
            .as_ref()
            .filter(|(_, at)| at.elapsed() < NOTICE_TTL)
            .map(|(notice, _)| notice)
    }

    pub fn on_key(&mut self, key: KeyEvent) -> Effect {
        if key.kind == KeyEventKind::Release {
            return Effect::None;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Effect::Quit;
        }
        if self.show_help {
            // Any key dismisses the overlay; it must not also act.
            self.show_help = false;
            return Effect::None;
        }
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => Effect::Quit,
            KeyCode::Char('?') | KeyCode::F(1) => {
                self.show_help = true;
                Effect::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.move_by(1);
                Effect::None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.move_by(-1);
                Effect::None
            }
            KeyCode::PageDown => {
                self.move_by(10);
                Effect::None
            }
            KeyCode::PageUp => {
                self.move_by(-10);
                Effect::None
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.select(0);
                Effect::None
            }
            KeyCode::End | KeyCode::Char('G') => {
                let last = self.rows().len().saturating_sub(1);
                self.select(last);
                Effect::None
            }
            KeyCode::Char('s') => {
                self.sort = self.sort.next();
                Effect::None
            }
            KeyCode::Char('r') => {
                self.descending = !self.descending;
                Effect::None
            }
            KeyCode::Tab => {
                self.cycle_filter();
                Effect::None
            }
            KeyCode::Char('R') | KeyCode::F(5) => Effect::Refresh,
            KeyCode::Char('p') | KeyCode::Char(' ') => self.toggle_pause(),
            KeyCode::Char('t') => Effect::SetSortByReset(!self.snapshot.sort_by_reset),
            _ => Effect::None,
        }
    }

    fn cycle_filter(&mut self) {
        let count = self.snapshot.providers().len();
        self.filter = match self.filter {
            None if count > 0 => Some(0),
            Some(at) if at + 1 < count => Some(at + 1),
            _ => None,
        };
    }

    fn toggle_pause(&mut self) -> Effect {
        let Some(row) = self.selected_row() else {
            return Effect::None;
        };
        let (provider, paused) = (row.provider.clone(), row.account.paused);
        let Some(account_ref) = row.account_ref().map(str::to_string) else {
            self.notify(Notice::Error(
                "this gateway does not report account_ref; pause needs a build with pool pause support"
                    .into(),
            ));
            return Effect::None;
        };
        Effect::SetPaused {
            provider,
            account_ref,
            paused: !paused,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::model::{AccountDto, PoolResponse, ProviderDto};

    fn key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    fn account(name: &str, util: f64) -> AccountDto {
        AccountDto {
            name: name.into(),
            account_ref: Some(format!("ref-{name}")),
            has_state: true,
            available: true,
            utilization_5h: Some(util),
            ..AccountDto::default()
        }
    }

    fn app_with(accounts: Vec<AccountDto>) -> App {
        let mut app = App::new("http://x".into(), Duration::from_secs(2));
        app.on_poll(Ok(Snapshot::from_response(PoolResponse {
            providers: vec![ProviderDto {
                provider: "claude".into(),
                accounts,
            }],
            sort_by_reset: false,
        })));
        app
    }

    #[test]
    fn pause_targets_the_selected_account_and_flips_state() {
        let mut app = app_with(vec![account("a", 0.1), account("b", 0.2)]);
        app.on_key(key('j'));
        assert_eq!(
            app.on_key(key('p')),
            Effect::SetPaused {
                provider: "claude".into(),
                account_ref: "ref-b".into(),
                paused: true
            }
        );
        // A paused account asks to resume.
        let mut b = account("b", 0.2);
        b.paused = true;
        app.on_poll(Ok(Snapshot::from_response(PoolResponse {
            providers: vec![ProviderDto {
                provider: "claude".into(),
                accounts: vec![account("a", 0.1), b],
            }],
            sort_by_reset: false,
        })));
        assert!(matches!(
            app.on_key(key('p')),
            Effect::SetPaused { paused: false, .. }
        ));
    }

    #[test]
    fn selection_follows_the_account_across_a_resort() {
        let mut app = app_with(vec![account("a", 0.9), account("b", 0.1)]);
        app.on_key(key('j')); // select "b"
        app.on_key(key('s')); // name
        app.on_key(key('s')); // state
        app.on_key(key('s')); // 5h usage ascending: b first
        assert_eq!(app.selected_row().unwrap().account.name, "b");
        assert_eq!(app.selected_index(), Some(0));
        app.on_key(key('r')); // descending: b moves to the bottom
        assert_eq!(app.selected_row().unwrap().account.name, "b");
        assert_eq!(app.selected_index(), Some(1));
    }

    #[test]
    fn selection_survives_a_poll_that_reorders_the_pool() {
        let mut app = app_with(vec![account("a", 0.1), account("b", 0.2)]);
        app.on_key(key('j'));
        app.on_poll(Ok(Snapshot::from_response(PoolResponse {
            providers: vec![ProviderDto {
                provider: "claude".into(),
                accounts: vec![account("b", 0.2), account("a", 0.1)],
            }],
            sort_by_reset: false,
        })));
        assert_eq!(app.selected_row().unwrap().account.name, "b");
    }

    #[test]
    fn missing_account_ref_blocks_pause_with_a_notice() {
        let mut a = account("a", 0.1);
        a.account_ref = None;
        let mut app = app_with(vec![a]);
        assert_eq!(app.on_key(key('p')), Effect::None);
        assert!(matches!(app.current_notice(), Some(Notice::Error(_))));
    }

    #[test]
    fn t_requests_the_opposite_of_the_current_policy() {
        let mut app = app_with(vec![account("a", 0.1)]);
        assert_eq!(app.on_key(key('t')), Effect::SetSortByReset(true));
        app.snapshot.sort_by_reset = true;
        assert_eq!(app.on_key(key('t')), Effect::SetSortByReset(false));
    }

    #[test]
    fn quit_refresh_and_help_keys() {
        let mut app = app_with(vec![account("a", 0.1)]);
        assert_eq!(app.on_key(key('R')), Effect::Refresh);
        assert_eq!(app.on_key(key('?')), Effect::None);
        assert!(app.show_help);
        // The key that dismisses help must not also act.
        assert_eq!(app.on_key(key('q')), Effect::None);
        assert!(!app.show_help);
        assert_eq!(app.on_key(key('q')), Effect::Quit);
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(app.on_key(ctrl_c), Effect::Quit);
    }

    #[test]
    fn a_failed_poll_keeps_the_last_snapshot_and_records_the_error() {
        let mut app = app_with(vec![account("a", 0.1)]);
        app.on_poll(Err("gateway unreachable".into()));
        assert_eq!(app.rows().len(), 1);
        assert_eq!(app.poll_error.as_deref(), Some("gateway unreachable"));
        app.on_poll(Ok(Snapshot::default()));
        assert!(app.poll_error.is_none());
    }

    #[test]
    fn tab_cycles_provider_filter_and_wraps_to_all() {
        let mut app = App::new("http://x".into(), Duration::from_secs(2));
        app.on_poll(Ok(Snapshot::from_response(PoolResponse {
            providers: vec![
                ProviderDto {
                    provider: "claude".into(),
                    accounts: vec![account("a", 0.1)],
                },
                ProviderDto {
                    provider: "codex".into(),
                    accounts: vec![account("b", 0.1)],
                },
            ],
            sort_by_reset: false,
        })));
        let tab = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
        app.on_key(tab);
        assert_eq!(app.filter_name().as_deref(), Some("claude"));
        app.on_key(tab);
        assert_eq!(app.rows().len(), 1);
        assert_eq!(app.rows()[0].provider, "codex");
        app.on_key(tab);
        assert_eq!(app.filter_name(), None);
        assert_eq!(app.rows().len(), 2);
    }
}

//! Monitor state and input handling. `on_key` / `on_mouse` are reducers that
//! hand back an [`Effect`] for the runtime to perform, so every binding is
//! testable without a terminal or a gateway.

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use ratatui::{
    crossterm::event::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    layout::Rect,
};

use super::{
    add::{self, AddFlow, Target},
    config_edit::DEFAULT_PRIORITY,
    model::{ProviderView, RankMode, Row, Snapshot},
};

/// How long a status-bar notice stays up.
const NOTICE_TTL: Duration = Duration::from_secs(5);
/// How long a just-written ranking is shown ahead of the gateway, which only
/// reports it after its hot reload.
const OVERLAY_TTL: Duration = Duration::from_secs(6);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    None,
    Quit,
    SetPaused {
        provider: String,
        account_ref: String,
        label: String,
        paused: bool,
    },
    /// Pause (`on == false`) or resume every account named in `account_refs`.
    SetProvider {
        provider: String,
        on: bool,
        account_refs: Vec<String>,
    },
    /// Write these priorities to the config file. `pool_names` is every account
    /// the provider currently pools, for a provider that lists none explicitly.
    SetRanks {
        provider: String,
        ranks: Vec<(String, u32)>,
        pool_names: Vec<String>,
    },
    ClearRanks {
        provider: String,
    },
    Delete {
        provider: String,
        kind: &'static str,
        name: String,
    },
    Add(add::Effect),
    /// Put text on the clipboard (via the terminal).
    CopyToClipboard(String),
    /// Mouse capture off while the dialog needs text selection, on otherwise.
    MouseCapture(bool),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    Info(String),
    Error(String),
}

/// What the cursor is on: a provider's header line, or one of its accounts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sel {
    pub provider: String,
    pub account: Option<String>,
}

/// The destructive delete action waiting for the operator to confirm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteConfirm {
    pub provider: String,
    pub kind: &'static str,
    pub name: String,
}

/// One selectable line, in screen order.
pub struct Entry<'a> {
    pub sel: Sel,
    pub provider: &'a ProviderView,
    /// `None` for a provider header; otherwise the account's rank and row.
    pub row: Option<(Option<usize>, &'a Row)>,
}

struct RankOverlay {
    provider: String,
    ranks: Vec<(String, u32)>,
    at: Instant,
}

pub struct App {
    pub snapshot: Snapshot,
    pub base_url: String,
    pub last_ok: Option<Instant>,
    pub poll_error: Option<String>,
    /// `None` until the operator picks a line, and again after they click
    /// outside the list or press Esc.
    pub selected: Option<Sel>,
    pub show_help: bool,
    pub notice: Option<(Notice, Instant)>,
    pub dialog: Option<AddFlow>,
    pub delete_confirm: Option<DeleteConfirm>,
    /// First visible list line.
    pub scroll: usize,
    /// Scroll the selection into view on the next draw (keyboard moves only).
    pub reveal: bool,
    /// Where the list was drawn, and what each drawn line is, for mouse clicks.
    pub list_area: Rect,
    pub hits: Vec<Option<Sel>>,
    /// Accounts a provider switch paused, so switching back on resumes exactly
    /// those and not ones the operator had paused on purpose.
    switched_off: HashMap<String, Vec<String>>,
    overlay: Option<RankOverlay>,
}

impl App {
    pub fn new(base_url: String) -> Self {
        Self {
            snapshot: Snapshot::default(),
            base_url,
            last_ok: None,
            poll_error: None,
            selected: None,
            show_help: false,
            notice: None,
            dialog: None,
            delete_confirm: None,
            scroll: 0,
            reveal: false,
            list_area: Rect::default(),
            hits: Vec::new(),
            switched_off: HashMap::new(),
            overlay: None,
        }
    }

    /// Every selectable line in screen order: each provider's header, then its
    /// accounts in the order the gateway would try them.
    pub fn entries(&self) -> Vec<Entry<'_>> {
        let mut entries = Vec::new();
        for provider in &self.snapshot.providers {
            entries.push(Entry {
                sel: Sel {
                    provider: provider.name.clone(),
                    account: None,
                },
                provider,
                row: None,
            });
            for (rank, row) in provider.ordered(self.snapshot.sort_by_reset) {
                entries.push(Entry {
                    sel: Sel {
                        provider: provider.name.clone(),
                        account: Some(row.key().to_string()),
                    },
                    provider,
                    row: Some((rank, row)),
                });
            }
        }
        entries
    }

    pub fn selected_index(&self) -> Option<usize> {
        let selected = self.selected.as_ref()?;
        self.entries()
            .iter()
            .position(|entry| &entry.sel == selected)
    }

    fn selected_provider(&self) -> Option<&ProviderView> {
        let name = &self.selected.as_ref()?.provider;
        self.snapshot.provider(name)
    }

    fn selected_row(&self) -> Option<(&ProviderView, &Row)> {
        let selected = self.selected.as_ref()?;
        let provider = self.snapshot.provider(&selected.provider)?;
        let key = selected.account.as_deref()?;
        provider
            .rows
            .iter()
            .find(|r| r.key() == key)
            .map(|r| (provider, r))
    }

    fn select_index(&mut self, at: usize) {
        let sel = self.entries().get(at).map(|entry| entry.sel.clone());
        if sel.is_some() {
            self.selected = sel;
            self.reveal = true;
        }
    }

    fn move_by(&mut self, delta: isize) {
        let len = self.entries().len();
        if len == 0 {
            return;
        }
        let next = match self.selected_index() {
            Some(at) => at.saturating_add_signed(delta).min(len - 1),
            None if delta < 0 => len - 1,
            None => 0,
        };
        self.select_index(next);
    }

    pub fn on_poll(&mut self, result: Result<Snapshot, String>) {
        match result {
            Ok(snapshot) => {
                self.snapshot = snapshot;
                self.apply_overlay();
                // A line that is gone (account removed, provider dropped) must
                // not leave an invisible selection that keys still act on.
                if self.selected.is_some() && self.selected_index().is_none() {
                    self.selected = None;
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

    pub fn current_notice(&self) -> Option<&Notice> {
        self.notice
            .as_ref()
            .filter(|(_, at)| at.elapsed() < NOTICE_TTL)
            .map(|(notice, _)| notice)
    }

    /// Show a ranking the moment it is written, then keep showing it until the
    /// gateway's own report catches up (or [`OVERLAY_TTL`] passes). Without this
    /// the list would sit unchanged for the reload delay and a second key press
    /// would be computed from a stale order.
    fn set_overlay(&mut self, provider: &str, ranks: Vec<(String, u32)>) {
        self.overlay = Some(RankOverlay {
            provider: provider.to_string(),
            ranks,
            at: Instant::now(),
        });
        self.apply_overlay();
    }

    fn apply_overlay(&mut self) {
        let Some(overlay) = &self.overlay else { return };
        let caught_up = self
            .snapshot
            .provider(&overlay.provider)
            .is_none_or(|provider| {
                overlay.ranks.iter().all(|(name, rank)| {
                    provider
                        .rows
                        .iter()
                        .find(|row| &row.account.name == name)
                        .is_none_or(|row| row.priority() == *rank)
                })
            });
        if caught_up || overlay.at.elapsed() > OVERLAY_TTL {
            self.overlay = None;
            return;
        }
        let overlay = self.overlay.take().expect("checked above");
        if let Some(provider) = self
            .snapshot
            .providers
            .iter_mut()
            .find(|p| p.name == overlay.provider)
        {
            for row in &mut provider.rows {
                if let Some((_, rank)) = overlay.ranks.iter().find(|(n, _)| *n == row.account.name)
                {
                    row.account.priority = Some(*rank);
                }
            }
        }
        self.overlay = Some(overlay);
    }

    pub fn on_paste(&mut self, text: &str) {
        if let Some(dialog) = &mut self.dialog {
            dialog.on_paste(text);
        }
    }

    pub fn on_mouse(&mut self, mouse: MouseEvent) -> Effect {
        if self.dialog.is_some() || self.delete_confirm.is_some() || self.show_help {
            return Effect::None;
        }
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let area = self.list_area;
                let inside = mouse.column >= area.x
                    && mouse.column < area.x + area.width
                    && mouse.row >= area.y
                    && mouse.row < area.y + area.height;
                // A click on an account or provider line selects it; a click
                // anywhere else — blank space, the headings, outside the list —
                // leaves nothing selected.
                self.selected = if inside {
                    self.hits
                        .get(usize::from(mouse.row - area.y))
                        .cloned()
                        .flatten()
                } else {
                    None
                };
            }
            MouseEventKind::ScrollDown => self.scroll = self.scroll.saturating_add(1),
            MouseEventKind::ScrollUp => self.scroll = self.scroll.saturating_sub(1),
            _ => {}
        }
        Effect::None
    }

    pub fn on_key(&mut self, key: KeyEvent) -> Effect {
        if key.kind == KeyEventKind::Release {
            return Effect::None;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Effect::Quit;
        }
        if self.delete_confirm.is_some() {
            return self.delete_key(key);
        }
        if self.dialog.is_some() {
            return self.dialog_key(key);
        }
        if self.show_help {
            // Any key dismisses the overlay; it must not also act.
            self.show_help = false;
            return Effect::None;
        }
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        match key.code {
            KeyCode::Char('q') => Effect::Quit,
            KeyCode::Esc => {
                self.selected = None;
                Effect::None
            }
            KeyCode::Char('?') => {
                self.show_help = true;
                Effect::None
            }
            KeyCode::Up if shift => self.move_rank(-1),
            KeyCode::Down if shift => self.move_rank(1),
            KeyCode::Char('K') => self.move_rank(-1),
            KeyCode::Char('J') => self.move_rank(1),
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
            KeyCode::Char('p') | KeyCode::Char(' ') => self.toggle_pause(),
            KeyCode::Char('o') => self.toggle_provider(),
            KeyCode::Char('m') => self.toggle_mode(),
            KeyCode::Char('a') => self.open_dialog(),
            KeyCode::Char('d') => self.open_delete_confirm(),
            _ => Effect::None,
        }
    }

    fn need(&mut self, text: &str) -> Effect {
        self.notify(Notice::Info(text.to_string()));
        Effect::None
    }

    fn toggle_pause(&mut self) -> Effect {
        let Some((provider, row)) = self.selected_row() else {
            return self.need(
                "Select an account first (p pauses one account; o switches a whole provider)",
            );
        };
        let Some(account_ref) = row.account_ref().map(str::to_string) else {
            let text = "this gateway does not report account_ref; pause needs pool pause support";
            self.notify(Notice::Error(text.into()));
            return Effect::None;
        };
        Effect::SetPaused {
            provider: provider.name.clone(),
            account_ref,
            label: row.account.name.clone(),
            paused: !row.account.paused,
        }
    }

    fn toggle_provider(&mut self) -> Effect {
        let Some(provider) = self.selected_provider() else {
            return self.need("Select a provider or one of its accounts first");
        };
        let name = provider.name.clone();
        let is_on = provider.is_on();
        let refs = |paused: bool| -> Vec<String> {
            provider
                .rows
                .iter()
                .filter(|r| !r.account.disabled && r.account.paused == paused)
                .filter_map(|r| r.account_ref().map(str::to_string))
                .collect()
        };
        let (live, paused) = (refs(false), refs(true));
        if is_on {
            if live.is_empty() {
                return self.need("This provider has no accounts to switch off");
            }
            self.switched_off.insert(name.clone(), live.clone());
            Effect::SetProvider {
                provider: name,
                on: false,
                account_refs: live,
            }
        } else {
            // Resume what the switch paused; with no memory of it (a restart of
            // this program), resume everything that is paused.
            let remembered = self.switched_off.remove(&name).unwrap_or_default();
            let account_refs = if remembered.is_empty() {
                paused
            } else {
                remembered
            };
            Effect::SetProvider {
                provider: name,
                on: true,
                account_refs,
            }
        }
    }

    fn toggle_mode(&mut self) -> Effect {
        let Some(provider) = self.selected_provider() else {
            return self.need("Select a provider or one of its accounts first");
        };
        if provider.rows.len() < 2 {
            return self.need("Ranking needs at least two accounts in the provider");
        }
        let provider_name = provider.name.clone();
        match provider.mode() {
            RankMode::Custom => {
                let ranks = provider
                    .rows
                    .iter()
                    .map(|r| (r.account.name.clone(), DEFAULT_PRIORITY))
                    .collect();
                self.set_overlay(&provider_name, ranks);
                Effect::ClearRanks {
                    provider: provider_name,
                }
            }
            RankMode::Balanced => {
                // Start from the order the gateway is using right now, so
                // switching does not reshuffle anything.
                let order = self.current_order(&provider_name);
                self.write_order(&provider_name, order)
            }
        }
    }

    fn current_order(&self, provider: &str) -> Vec<String> {
        self.snapshot
            .provider(provider)
            .map(|p| {
                p.ordered(self.snapshot.sort_by_reset)
                    .into_iter()
                    .map(|(_, row)| row.account.name.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn write_order(&mut self, provider: &str, order: Vec<String>) -> Effect {
        let ranks: Vec<(String, u32)> = order
            .into_iter()
            .enumerate()
            .map(|(at, name)| (name, at as u32 + 1))
            .collect();
        let pool_names = self
            .snapshot
            .provider(provider)
            .map(|p| p.rows.iter().map(|r| r.account.name.clone()).collect())
            .unwrap_or_default();
        self.set_overlay(provider, ranks.clone());
        Effect::SetRanks {
            provider: provider.to_string(),
            ranks,
            pool_names,
        }
    }

    fn move_rank(&mut self, delta: isize) -> Effect {
        let Some((provider, row)) = self.selected_row() else {
            return self.need("Select an account to move it up or down the ranking");
        };
        if provider.mode() != RankMode::Custom {
            return self.need(
                "Balanced mode has no fixed order — press m to switch to custom order first",
            );
        }
        let (provider_name, name) = (provider.name.clone(), row.account.name.clone());
        let mut order = self.current_order(&provider_name);
        let Some(at) = order.iter().position(|n| *n == name) else {
            return Effect::None;
        };
        let to = at.saturating_add_signed(delta);
        if to >= order.len() || to == at {
            return Effect::None;
        }
        order.swap(at, to);
        self.write_order(&provider_name, order)
    }

    fn open_delete_confirm(&mut self) -> Effect {
        let Some((provider, row)) = self.selected_row() else {
            return self.need("Select an account first (d deletes one account)");
        };
        let Some(kind) = provider.account_kind() else {
            return self.need("This provider's accounts cannot be deleted from the admin API");
        };
        self.delete_confirm = Some(DeleteConfirm {
            provider: provider.name.clone(),
            kind,
            name: row.account.name.clone(),
        });
        Effect::None
    }

    fn delete_key(&mut self, key: KeyEvent) -> Effect {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let Some(confirm) = self.delete_confirm.take() else {
                    return Effect::None;
                };
                Effect::Delete {
                    provider: confirm.provider,
                    kind: confirm.kind,
                    name: confirm.name,
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.delete_confirm = None;
                Effect::None
            }
            _ => Effect::None,
        }
    }

    fn open_dialog(&mut self) -> Effect {
        let choices: Vec<Target> = self
            .snapshot
            .providers
            .iter()
            .filter_map(|p| {
                p.account_kind().map(|kind| Target {
                    provider: p.name.clone(),
                    kind,
                })
            })
            .collect();
        let preferred = self.selected.as_ref().map(|s| s.provider.clone());
        match AddFlow::new(choices, preferred.as_deref()) {
            Some(flow) => {
                self.dialog = Some(flow);
                Effect::None
            }
            None => self.need("No provider here can add accounts from the admin API"),
        }
    }

    fn dialog_key(&mut self, key: KeyEvent) -> Effect {
        let Some(dialog) = &mut self.dialog else {
            return Effect::None;
        };
        match dialog.on_key(key) {
            add::Effect::Cancel => self.close_dialog(),
            add::Effect::None => self.sync_mouse(),
            add::Effect::CopyUrl(url) => Effect::CopyToClipboard(url),
            other => Effect::Add(other),
        }
    }

    pub fn close_dialog(&mut self) -> Effect {
        self.dialog = None;
        Effect::MouseCapture(true)
    }

    /// Mouse capture follows the dialog's step: off while an authorize URL is on
    /// screen, so it can be selected and copied.
    pub fn sync_mouse(&self) -> Effect {
        match &self.dialog {
            Some(d) => Effect::MouseCapture(!d.wants_text_selection()),
            None => Effect::MouseCapture(true),
        }
    }
}

#[cfg(test)]
mod tests;

//! Rendering. Draws from `&mut App` (it records where the list landed so a
//! mouse click can be mapped back to a line) and a clock reading, so a
//! `TestBackend` can assert on what an operator would see.

use std::time::{SystemTime, UNIX_EPOCH};

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use super::{
    add::{AddFlow, Step},
    app::{App, DeleteConfirm, Entry, Notice, Sel},
    model::{AccountDto, AccountState, ProviderView, RankMode},
};

const BAR_WIDTH: usize = 8;
const NAME_W: usize = 20;
const STATE_W: usize = 16;
const REQUESTS_W: usize = 22;
const WINDOW_W: usize = BAR_WIDTH + 1 + 4 + 3 + 7;

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// "2h 14m" / "3d 4h" / "now" until an epoch-seconds instant.
pub fn until(reset: u64, now: u64) -> String {
    let mins = reset.saturating_sub(now).div_ceil(60);
    if reset <= now || mins == 0 {
        return "now".into();
    }
    let (days, hours, rest) = (mins / 1440, (mins % 1440) / 60, mins % 60);
    match (days, hours, rest) {
        (0, 0, m) => format!("{m}m"),
        (0, h, 0) => format!("{h}h"),
        (0, h, m) => format!("{h}h {m}m"),
        (d, 0, _) => format!("{d}d"),
        (d, h, _) => format!("{d}d {h}h"),
    }
}

fn level_color(fraction: f64) -> Color {
    if fraction >= 0.9 {
        Color::Red
    } else if fraction >= 0.7 {
        Color::Yellow
    } else {
        Color::Green
    }
}

fn fit(text: &str, width: usize) -> String {
    let len = text.chars().count();
    if len <= width {
        format!("{text:<width$}")
    } else {
        let mut cut: String = text.chars().take(width.saturating_sub(1)).collect();
        cut.push('…');
        cut
    }
}

/// `████░░░░  50% · 2h 14m` — usage and the time until that limit resets —
/// padded to a fixed width, or a dash when the account reports no reading.
fn window_cell(utilization: Option<f64>, reset: Option<u64>, now: u64) -> Vec<Span<'static>> {
    let Some(value) = utilization else {
        return vec![Span::styled(
            fit("—", WINDOW_W),
            Style::new().fg(Color::DarkGray),
        )];
    };
    let clamped = value.clamp(0.0, 1.0);
    let filled = (clamped * BAR_WIDTH as f64).round() as usize;
    let tail = reset
        .filter(|r| *r > 0)
        .map_or(String::new(), |r| format!(" · {}", until(r, now)));
    let text = format!(" {:>3.0}%{tail}", value * 100.0);
    vec![
        Span::styled("█".repeat(filled), Style::new().fg(level_color(clamped))),
        Span::styled(
            "░".repeat(BAR_WIDTH - filled),
            Style::new().fg(Color::DarkGray),
        ),
        Span::raw(fit(&text, WINDOW_W - BAR_WIDTH)),
    ]
}

fn state_style(state: AccountState) -> Style {
    match state {
        AccountState::Available => Style::new().fg(Color::Green),
        AccountState::Paused => Style::new().fg(Color::Magenta).bold(),
        AccountState::NearQuota | AccountState::CoolingFable => Style::new().fg(Color::Yellow),
        AccountState::Cooling => Style::new().fg(Color::LightRed),
        AccountState::NeedsRelogin => Style::new().fg(Color::Red).bold(),
        AccountState::Disabled | AccountState::Unseen => Style::new().fg(Color::DarkGray),
    }
}

fn provider_line(entry: &Entry<'_>, collapsed: bool) -> Line<'static> {
    let provider = entry.provider;
    let (switch, switch_style) = if provider.is_on() {
        (
            " ON ",
            Style::new().fg(Color::Black).bg(Color::Green).bold(),
        )
    } else {
        (" OFF", Style::new().fg(Color::White).bg(Color::Red).bold())
    };
    let mode = match provider.mode() {
        RankMode::Balanced => "balanced",
        RankMode::Custom => "custom order",
    };
    let count = provider.rows.len();
    let detail = if count == 0 {
        "no accounts · press a to add one".to_string()
    } else {
        let plural = if count == 1 { "" } else { "s" };
        if collapsed {
            format!("{count} account{plural} · {}", state_summary(provider))
        } else {
            format!("{count} account{plural}")
        }
    };
    let arrow = if collapsed { "▸" } else { "▾" };
    Line::from(vec![
        Span::styled(
            fit(&format!("{arrow} {}", provider.name), NAME_W + 4),
            Style::new().bold(),
        ),
        Span::styled(switch, switch_style),
        Span::styled("  ranking: ", Style::new().fg(Color::Gray)),
        Span::styled(
            fit(mode, 12),
            Style::new().fg(Color::Rgb(255, 165, 0)).bold(),
        ),
        Span::styled(format!(" · {detail}"), Style::new().fg(Color::Gray)),
    ])
    .style(Style::new().bg(Color::Rgb(38, 42, 50)))
}

/// `2 available · 1 paused` — what a folded provider hides.
fn state_summary(provider: &ProviderView) -> String {
    let count = |f: fn(AccountState) -> bool| provider.rows.iter().filter(|r| f(r.state)).count();
    let available = count(|s| s == AccountState::Available);
    let paused = count(|s| s == AccountState::Paused);
    let other = provider.rows.len() - available - paused;
    [
        (available, "available"),
        (paused, "paused"),
        (other, "other"),
    ]
    .iter()
    .filter(|(n, _)| *n > 0)
    .map(|(n, label)| format!("{n} {label}"))
    .collect::<Vec<_>>()
    .join(" · ")
}

fn account_line(entry: &Entry<'_>, now: u64) -> Line<'static> {
    let (rank, row) = entry.row.expect("account entry");
    let a = &row.account;
    let rank = rank.map_or_else(|| "–".to_string(), |n| n.to_string());
    let mut spans = vec![
        Span::styled(format!("{rank:>3} "), Style::new().bold()),
        Span::raw(fit(&a.name, NAME_W + 1)),
    ];
    spans.extend(window_cell(a.utilization_5h, a.reset_5h, now));
    spans.push(Span::raw("  "));
    spans.extend(window_cell(a.utilization_7d, a.reset_7d, now));
    spans.push(Span::raw("  "));
    spans.push(Span::styled(
        fit(row.state.label(), STATE_W + 1),
        state_style(row.state),
    ));
    spans.push(Span::styled(
        fit(&requests_cell(a), REQUESTS_W + 1),
        if a.requests_failed > 0 {
            Style::new().fg(Color::LightRed)
        } else {
            Style::new()
        },
    ));
    spans.push(Span::raw(a.plan.clone().unwrap_or_default()));
    let mut line = Line::from(spans);
    if !entry.provider.is_on() || matches!(row.state, AccountState::Disabled | AccountState::Paused)
    {
        line = line.style(Style::new().add_modifier(Modifier::DIM));
    }
    line
}

/// `ok/failed · mean latency` for the attempts this gateway process has sent
/// to the account; `–` before the first one.
fn requests_cell(a: &AccountDto) -> String {
    if a.requests_attempted == 0 {
        return "–".to_string();
    }
    let latency = a
        .mean_latency_ms
        .map_or_else(String::new, |ms| format!(" · {ms:.0}ms"));
    format!("{}/{}{latency}", a.requests_succeeded, a.requests_failed)
}

fn column_heading() -> Line<'static> {
    let head = format!(
        "{:>3} {:<n$} {:<w$}  {:<w$}  {:<s$} {:<r$} {}",
        "#",
        "Account",
        "5h limit",
        "7d limit",
        "State",
        "Requests ok/fail · avg",
        "Plan",
        n = NAME_W,
        s = STATE_W,
        r = REQUESTS_W,
        w = WINDOW_W,
    );
    Line::styled(head, Style::new().bold().fg(Color::Cyan))
}

pub fn render(frame: &mut Frame, app: &mut App, now: u64) {
    let [header, heading, _gap, list, footer] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(2),
    ])
    .areas(frame.area());

    render_header(frame, app, header);
    render_list(frame, app, heading, list, now);
    render_footer(frame, app, footer);
    if let Some(dialog) = &app.dialog {
        render_dialog(frame, dialog);
    } else if let Some(confirm) = &app.delete_confirm {
        render_delete_confirm(frame, confirm);
    } else if let Some(picker) = &app.unhide {
        render_unhide(frame, app, picker.at);
    } else if app.show_help {
        render_help(frame, frame.area());
    }
}

fn render_header(frame: &mut Frame, app: &App, area: Rect) {
    let freshness = match (&app.poll_error, app.last_ok) {
        (Some(error), _) => Span::styled(format!("● {error}"), Style::new().fg(Color::Red).bold()),
        (None, Some(at)) => Span::styled(
            format!("● live · updated {}s ago", at.elapsed().as_secs()),
            Style::new().fg(Color::Green),
        ),
        (None, None) => Span::styled("● connecting…", Style::new().fg(Color::Yellow)),
    };
    let block = Block::new().borders(Borders::BOTTOM);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let title = Line::from(vec![
        Span::styled(" shunt tui ", Style::new().bold()),
        Span::styled(app.base_url.clone(), Style::new().dark_gray()),
    ]);
    frame.render_widget(Paragraph::new(title), inner);
    frame.render_widget(Paragraph::new(Line::from(freshness).right_aligned()), inner);
}

fn render_list(frame: &mut Frame, app: &mut App, heading: Rect, list: Rect, now: u64) {
    let empty_text = if app.last_ok.is_none() && app.poll_error.is_none() {
        Some("Waiting for the first poll…")
    } else if app.last_ok.is_none() {
        Some("Cannot reach the gateway. Retrying…")
    } else if app.snapshot.providers.is_empty() {
        Some(
            "No pooled providers. Configure a claude_oauth / chatgpt_oauth provider with accounts.",
        )
    } else if app.entries().is_empty() {
        Some("All providers hidden · press u to unhide one")
    } else {
        None
    };
    app.list_area = list;
    app.hits.clear();
    if let Some(text) = empty_text {
        frame.render_widget(
            Paragraph::new(text).wrap(Wrap { trim: true }).dark_gray(),
            list.inner(ratatui::layout::Margin::new(2, 1)),
        );
        return;
    }
    frame.render_widget(Paragraph::new(column_heading()), heading);

    // Build every line with the selection it stands for, then window it.
    let selected = app.selected.clone();
    let mut lines: Vec<(Line<'static>, Option<Sel>)> = Vec::new();
    let mut selected_line = None;
    let mut current_provider: Option<String> = None;
    for entry in app.entries() {
        if entry.row.is_none() && current_provider.is_some() {
            lines.push((Line::default(), None));
        }
        current_provider = Some(entry.sel.provider.clone());
        let mut line = if entry.row.is_none() {
            provider_line(&entry, app.collapsed.contains(&entry.sel.provider))
        } else {
            account_line(&entry, now)
        };
        if selected.as_ref() == Some(&entry.sel) {
            selected_line = Some(lines.len());
            line = line.style(Style::new().add_modifier(Modifier::REVERSED));
        }
        lines.push((line, Some(entry.sel)));
    }

    let height = usize::from(list.height);
    if app.reveal {
        if let Some(at) = selected_line {
            if at < app.scroll {
                app.scroll = at;
            } else if at >= app.scroll + height {
                app.scroll = at + 1 - height;
            }
        }
        app.reveal = false;
    }
    app.scroll = app.scroll.min(lines.len().saturating_sub(height));
    for (line, sel) in lines.into_iter().skip(app.scroll).take(height) {
        let y = list.y + app.hits.len() as u16;
        // A header's background spans the whole row, not just its text.
        let style = if sel.as_ref().is_some_and(|s| s.account.is_none()) {
            line.style
        } else {
            Style::new()
        };
        frame.render_widget(
            Paragraph::new(line).style(style),
            Rect::new(list.x, y, list.width, 1),
        );
        app.hits.push(sel);
    }
}

fn render_footer(frame: &mut Frame, app: &App, area: Rect) {
    // The hidden count lives here, not in the list: `u unhide (2)`.
    let unhide = if app.hidden.is_empty() {
        String::new()
    } else {
        format!(" · u unhide ({})", app.hidden.len())
    };
    let hint = if app.dialog.is_some() {
        "Enter continue · Esc cancel".to_string()
    } else if app.delete_confirm.is_some() {
        "y delete · n/Esc cancel".to_string()
    } else if app.unhide.is_some() {
        "↑↓ choose · Enter unhide · Esc cancel".to_string()
    } else if app.selected.is_none() {
        format!("↑↓ or click: select{unhide} · a: add account · ?: help · q: quit")
    } else if app.header_selected() {
        format!(
            "p pause/resume all · Space fold · m ranking mode · h hide{unhide} · a add · Esc deselect · ? help · q quit"
        )
    } else {
        format!(
            "p pause · h hide{unhide} · m ranking mode · Shift+↑↓ move · a add · d delete · Esc deselect · ? help · q quit"
        )
    };
    // ponytail: notice gets its own line so the hints below never hide.
    let notice = match app.current_notice() {
        Some(Notice::Info(text)) => Line::from(format!(" {text}")).green(),
        Some(Notice::Error(text)) => Line::from(format!(" {text}")).red().bold(),
        None => Line::default(),
    };
    let [notice_area, hint_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(area);
    frame.render_widget(Paragraph::new(notice), notice_area);
    frame.render_widget(
        Paragraph::new(Line::from(format!(" {hint}")).dark_gray()),
        hint_area,
    );
}

fn popup(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    )
}

fn render_delete_confirm(frame: &mut Frame, confirm: &DeleteConfirm) {
    let area = frame.area();
    let lines = vec![
        Line::from(vec![
            Span::raw("Delete account "),
            Span::styled(format!("{:?}", confirm.name), Style::new().bold()),
            Span::raw(format!(" from {}?", confirm.provider)),
        ]),
        Line::default(),
        Line::styled(
            "This permanently removes the managed credential from Shunt's account store.",
            Style::new().fg(Color::Red),
        ),
        Line::default(),
        Line::styled("y: delete · n / Esc: cancel", Style::new().dark_gray()),
    ];
    let rect = popup(area, 78, 9);
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::bordered().title(" Delete account "))
            .wrap(Wrap { trim: false }),
        rect,
    );
}

fn render_unhide(frame: &mut Frame, app: &App, at: usize) {
    let names = app.hidden_names();
    let mut lines = vec![
        Line::from("Which provider should be shown again?"),
        Line::default(),
    ];
    for (i, name) in names.iter().enumerate() {
        let marker = if i == at { "▶ " } else { "  " };
        let style = if i == at {
            Style::new().bold()
        } else {
            Style::new()
        };
        lines.push(Line::styled(format!("{marker}{name}"), style));
    }
    lines.push(Line::default());
    lines.push(Line::styled(
        "↑↓ choose · Enter unhide · Esc cancel",
        Style::new().dark_gray(),
    ));
    let height = lines.len() as u16 + 4;
    let rect = popup(frame.area(), 52, height);
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(lines).block(Block::bordered().title(" Unhide provider ")),
        rect,
    );
}

fn render_dialog(frame: &mut Frame, dialog: &AddFlow) {
    let area = frame.area();
    let width = area.width.saturating_sub(4).min(100);
    let target = dialog.target();
    let mut lines: Vec<Line<'static>> = Vec::new();
    match &dialog.step {
        Step::Provider { at } => {
            lines.push(Line::from("Which provider should the account join?"));
            lines.push(Line::default());
            for (i, choice) in dialog.choices.iter().enumerate() {
                let marker = if i == *at { "▶ " } else { "  " };
                let style = if i == *at {
                    Style::new().bold()
                } else {
                    Style::new()
                };
                lines.push(Line::styled(format!("{marker}{}", choice.provider), style));
            }
            lines.push(Line::default());
            lines.push(Line::styled(
                "↑↓ choose · Enter continue · Esc cancel",
                Style::new().dark_gray(),
            ));
        }
        Step::Name => {
            lines.push(Line::from(format!("Add an account to {}", target.provider)));
            lines.push(Line::default());
            lines.push(Line::from(vec![
                Span::raw("Name: "),
                Span::styled(format!("{}▏", dialog.name), Style::new().bold()),
            ]));
            lines.push(Line::styled(
                "lowercase letters, digits and hyphens, e.g. pool-b",
                Style::new().dark_gray(),
            ));
        }
        Step::Starting => lines.push(Line::from("Contacting the gateway…")),
        Step::Code | Step::Submitting => {
            lines.push(Line::from(format!(
                "Adding {} to {}",
                dialog.name, target.provider
            )));
            lines.push(Line::default());
            lines.push(Line::from(
                "1. Open this link and sign in (your browser was asked to open it):",
            ));
            lines.push(Line::styled(
                dialog.url.clone(),
                Style::new().fg(Color::Cyan),
            ));
            lines.push(Line::default());
            lines.push(Line::from(match target.kind {
                "claude" => "2. Paste the code the page shows you (it looks like  code#state):",
                _ => "2. Paste the full address from the browser's address bar after you sign in\n   (the page itself may fail to load — that is fine):",
            }));
            let code = if dialog.step == Step::Submitting {
                "exchanging the code…".to_string()
            } else {
                format!(
                    "{}▏",
                    tail(&dialog.code, usize::from(width).saturating_sub(8))
                )
            };
            lines.push(Line::styled(code, Style::new().bold()));
            lines.push(Line::default());
            lines.push(Line::styled(
                "Tab: copy the link · Enter: submit · Esc: cancel",
                Style::new().dark_gray(),
            ));
        }
    }
    if let Some(error) = &dialog.error {
        lines.push(Line::default());
        lines.push(Line::styled(
            error.clone(),
            Style::new().fg(Color::Red).bold(),
        ));
    }
    let url_rows = (dialog.url.chars().count() as u16 / width.max(1)) + 1;
    let height = lines.len() as u16 + url_rows + 4;
    let rect = popup(area, width, height);
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(lines)
            // Top and bottom rules only: side borders would end up inside a
            // link copied by dragging the mouse across its wrapped lines.
            .block(
                Block::new()
                    .borders(Borders::TOP | Borders::BOTTOM)
                    .title(" Add account "),
            )
            .wrap(Wrap { trim: false }),
        rect,
    );
}

/// The last `max` characters, so a long paste shows its end where the cursor is.
fn tail(text: &str, max: usize) -> String {
    let skip = text.chars().count().saturating_sub(max);
    text.chars().skip(skip).collect()
}

fn render_help(frame: &mut Frame, area: Rect) {
    let text = [
        "Each provider has its own section. A row is only highlighted once you",
        "pick it (arrow keys or a click); click empty space or press Esc to unpick.",
        "",
        "  ↑↓ / j k        move · PgUp/PgDn jump",
        "  p               pause or resume the selected account; on a provider header, all its accounts",
        "  Space           pause the selected account; on a provider header, fold or unfold it",
        "  h               hide the selected provider (display only — it still routes traffic)",
        "  u               unhide one hidden provider (picker)",
        "  m               ranking mode for the provider:",
        "                    balanced     the gateway spreads load by remaining headroom",
        "                    custom order your own 1, 2, 3… — number 1 is drawn first",
        "  Shift+↑↓ / K J  on an account: move it up/down the custom order;",
        "                  on a provider header: move the whole section",
        "  a               add an account (it is added to the pool in shunt.toml)",
        "  d               delete the selected account (asks y/n before deleting)",
        "  q               quit",
        "",
        "Pause/switch-off last until the gateway restarts. Hiding and section order",
        "are this terminal's own display preferences (~/.shunt/top.json); routing",
        "is untouched. Ranking and explicit pool membership are saved in shunt.toml;",
        "delete also removes the credential.",
        "Open conversations stay on the account they started with.",
        "",
        "Press any key to close.",
    ]
    .join("\n");
    let rect = popup(area, 84, 27);
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(text).block(Block::bordered().title(" Help ")),
        rect,
    );
}

#[cfg(test)]
mod tests;

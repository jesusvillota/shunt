//! Rendering. Pure over `&App` and a clock reading, so a `TestBackend` can
//! assert on what an operator would see.

use std::time::{SystemTime, UNIX_EPOCH};

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row as TableRow, Table, TableState, Wrap},
    Frame,
};

use super::{
    app::{App, Notice},
    model::{AccountState, Row},
};

const BAR_WIDTH: usize = 8;

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

fn secs_short(secs: u64) -> String {
    if secs >= 3600 {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    } else if secs >= 60 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{secs}s")
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

/// `████░░░░ 52%`, or a dash when the account reports no reading.
fn bar(value: Option<f64>) -> Line<'static> {
    let Some(value) = value else {
        return Line::from("—".dark_gray());
    };
    let clamped = value.clamp(0.0, 1.0);
    let filled = (clamped * BAR_WIDTH as f64).round() as usize;
    let color = level_color(clamped);
    Line::from(vec![
        Span::styled("█".repeat(filled), Style::new().fg(color)),
        Span::styled(
            "░".repeat(BAR_WIDTH - filled),
            Style::new().fg(Color::DarkGray),
        ),
        Span::raw(format!(" {:>3.0}%", value * 100.0)),
    ])
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

pub fn render(frame: &mut Frame, app: &App, now: u64) {
    let [header, body, detail, footer] = Layout::vertical([
        Constraint::Length(4),
        Constraint::Min(5),
        Constraint::Length(detail_height(app)),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    render_header(frame, app, header);
    render_table(frame, app, body, now);
    render_detail(frame, app, detail, now);
    render_footer(frame, app, footer);
    if app.show_help {
        render_help(frame, frame.area());
    }
}

fn detail_height(app: &App) -> u16 {
    let buckets = app
        .selected_row()
        .map_or(0, |row| row.account.quota_buckets.len());
    // Border + two fixed lines, plus one per bucket, capped so a provider with
    // many buckets cannot squeeze the table out.
    (3 + buckets.min(6)) as u16
}

fn render_header(frame: &mut Frame, app: &App, area: Rect) {
    let summary = app.snapshot.summary();
    let freshness = match (&app.poll_error, app.last_ok) {
        (Some(error), _) => Span::styled(format!("● {error}"), Style::new().fg(Color::Red).bold()),
        (None, Some(at)) => Span::styled(
            format!(
                "● live · {}s ago · every {:?}",
                at.elapsed().as_secs(),
                app.interval
            ),
            Style::new().fg(Color::Green),
        ),
        (None, None) => Span::styled("● connecting…", Style::new().fg(Color::Yellow)),
    };
    let rank = if app.snapshot.sort_by_reset {
        "soonest reset"
    } else {
        "burn-rate headroom"
    };
    let counts = Line::from(vec![
        Span::raw(format!(
            " {} accounts · {} selectable",
            summary.total, summary.available
        )),
        count_span(summary.paused, "paused", Color::Magenta),
        count_span(summary.cooling, "cooling", Color::LightRed),
        count_span(summary.near_quota, "near quota", Color::Yellow),
        count_span(summary.needs_relogin, "need re-login", Color::Red),
        count_span(summary.disabled, "disabled", Color::DarkGray),
    ]);
    let filter = app.filter_name().unwrap_or_else(|| "all".into());
    let controls = Line::from(vec![
        Span::raw(format!(" gateway picks by: {rank}")),
        Span::styled(" (t)", Style::new().dark_gray()),
        Span::raw(format!(
            "   view: {} {} · provider: {filter}",
            app.sort.label(),
            if app.sort == super::model::SortKey::Default {
                ""
            } else if app.descending {
                "▼"
            } else {
                "▲"
            }
        )),
    ]);
    let block = Block::new().borders(Borders::BOTTOM);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let [title_row, lines_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(inner);
    let title = Line::from(vec![
        Span::styled(" shunt pool ", Style::new().bold()),
        Span::styled(app.base_url.clone(), Style::new().dark_gray()),
    ]);
    frame.render_widget(Paragraph::new(title), title_row);
    frame.render_widget(
        Paragraph::new(Line::from(freshness).right_aligned()),
        title_row,
    );
    frame.render_widget(Paragraph::new(vec![counts, controls]), lines_area);
}

fn count_span(count: usize, label: &str, color: Color) -> Span<'static> {
    if count == 0 {
        Span::raw("")
    } else {
        Span::styled(format!(" · {count} {label}"), Style::new().fg(color))
    }
}

fn render_table(frame: &mut Frame, app: &App, area: Rect, now: u64) {
    let rows = app.rows();
    if rows.is_empty() {
        let text = if app.last_ok.is_none() && app.poll_error.is_none() {
            "Waiting for the first poll…"
        } else if app.last_ok.is_none() {
            "Cannot reach the gateway. Retrying…"
        } else {
            "No managed pool accounts. Add `claude_oauth` / `chatgpt_oauth` providers with accounts \
             (see docs), then they appear here."
        };
        frame.render_widget(
            Paragraph::new(text).wrap(Wrap { trim: true }).dark_gray(),
            area.inner(ratatui::layout::Margin::new(2, 1)),
        );
        return;
    }
    let header = TableRow::new(
        [
            "Provider",
            "Account",
            "Plan",
            "State",
            "5h",
            "7d",
            "Peak",
            "Resets in",
        ]
        .map(|title| Cell::from(title).style(Style::new().bold().fg(Color::Cyan))),
    );
    let table_rows = rows.iter().map(|row| {
        let a = &row.account;
        let reset = row
            .soonest_reset()
            .map_or_else(|| "—".to_string(), |at| until(at, now));
        let dim = matches!(row.state, AccountState::Disabled | AccountState::Paused);
        let base = if dim {
            Style::new().add_modifier(Modifier::DIM)
        } else {
            Style::new()
        };
        TableRow::new(vec![
            Cell::from(row.provider.clone()),
            Cell::from(a.name.clone()),
            Cell::from(a.plan.clone().unwrap_or_default()),
            Cell::from(Span::styled(row.state.label(), state_style(row.state))),
            Cell::from(bar(a.utilization_5h)),
            Cell::from(bar(a.utilization_7d)),
            Cell::from(bar(row.peak_utilization())),
            Cell::from(reset),
        ])
        .style(base)
    });
    let widths = [
        Constraint::Length(10),
        Constraint::Min(14),
        Constraint::Length(8),
        Constraint::Length(15),
        Constraint::Length(BAR_WIDTH as u16 + 5),
        Constraint::Length(BAR_WIDTH as u16 + 5),
        Constraint::Length(BAR_WIDTH as u16 + 5),
        Constraint::Length(10),
    ];
    let table = Table::new(table_rows, widths)
        .header(header)
        .column_spacing(2)
        .row_highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol("▶ ");
    let mut state = TableState::default().with_selected(app.selected_index());
    frame.render_stateful_widget(table, area, &mut state);
}

fn render_detail(frame: &mut Frame, app: &App, area: Rect, now: u64) {
    let block = Block::new().borders(Borders::TOP);
    let Some(row) = app.selected_row() else {
        frame.render_widget(block, area);
        return;
    };
    frame.render_widget(Paragraph::new(detail_lines(row, now)).block(block), area);
}

fn detail_lines(row: &Row, now: u64) -> Vec<Line<'static>> {
    let a = &row.account;
    let window = |label: &str, util: Option<f64>, reset: Option<u64>| -> String {
        match (util, reset) {
            (None, None) => String::new(),
            (u, r) => format!(
                "{label} {}{}   ",
                u.map_or("—".into(), |u| format!("{:.0}%", u * 100.0)),
                r.filter(|r| *r > 0)
                    .map_or(String::new(), |r| format!(" (resets {})", until(r, now)))
            ),
        }
    };
    let mut lines = vec![
        Line::from(vec![
            Span::styled(
                format!(" {} / {} ", row.provider, a.name),
                Style::new().bold(),
            ),
            Span::styled(row.state.label(), state_style(row.state)),
            Span::raw(
                a.priority
                    .map_or(String::new(), |p| format!("   priority {p}")),
            ),
            Span::raw(
                a.headroom_secs
                    .map_or(String::new(), |h| format!("   headroom {}", signed_secs(h))),
            ),
        ]),
        Line::from(format!(
            " {}{}{}",
            window("5h", a.utilization_5h, a.reset_5h),
            window("7d", a.utilization_7d, a.reset_7d),
            window("fable 7d", a.utilization_7d_oi, a.reset_7d_oi),
        )),
    ];
    let mut cooldowns = String::new();
    if let Some(s) = a.cooldown_secs_remaining.filter(|s| *s > 0) {
        cooldowns += &format!(" cooldown {}  ", secs_short(s));
    }
    if let Some(s) = a.cooldown_fable_secs_remaining.filter(|s| *s > 0) {
        cooldowns += &format!(" fable cooldown {}", secs_short(s));
    }
    if !cooldowns.is_empty() {
        lines[1].push_span(Span::styled(cooldowns, Style::new().fg(Color::LightRed)));
    }
    for bucket in a.quota_buckets.iter().take(6) {
        let mut spans = vec![Span::raw(format!(" {:<22}", bucket.label))];
        // `remaining` is the fraction left; the table speaks in usage.
        spans.extend(bar(bucket.remaining.map(|r| 1.0 - r)).spans);
        if let Some(reset) = bucket.reset_time.as_deref() {
            spans.push(Span::raw(format!("   resets {reset}")));
        }
        lines.push(Line::from(spans));
    }
    lines
}

fn signed_secs(secs: i64) -> String {
    let text = secs_short(secs.unsigned_abs());
    if secs < 0 {
        format!("-{text}")
    } else {
        text
    }
}

fn render_footer(frame: &mut Frame, app: &App, area: Rect) {
    let line = match app.current_notice() {
        Some(Notice::Info(text)) => Line::from(format!(" {text}")).green(),
        Some(Notice::Error(text)) => Line::from(format!(" {text}")).red().bold(),
        None => Line::from(" ↑↓ move · p pause/resume · s sort · r reverse · Tab provider · t gateway rank · R refresh · ? help · q quit")
            .dark_gray(),
    };
    frame.render_widget(Paragraph::new(line), area);
}

fn render_help(frame: &mut Frame, area: Rect) {
    let text = [
        "Navigation   ↑/↓ or j/k move · PgUp/PgDn · g/G first/last",
        "",
        "p / Space    Pause or resume the selected account (memory-only; a gateway",
        "             restart clears it). Needs a write-tier admin token.",
        "t            Toggle how the GATEWAY ranks available accounts: burn-rate",
        "             headroom vs soonest quota reset (process-wide, memory-only).",
        "s            Cycle the table's sort column (view only — does not change",
        "             which account the gateway picks).",
        "r            Reverse the sort direction. Accounts with no reading stay last.",
        "Tab          Filter to one provider; cycle back to all.",
        "R / F5       Refresh now.",
        "q / Esc      Quit.",
        "",
        "Press any key to close.",
    ]
    .join("\n");
    let width = 78.min(area.width);
    let height = 16.min(area.height);
    let popup = Rect::new(
        area.x + (area.width.saturating_sub(width)) / 2,
        area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(text)
            .block(Block::bordered().title(" Help "))
            .wrap(Wrap { trim: false }),
        popup,
    );
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use ratatui::{backend::TestBackend, Terminal};

    use super::*;
    use crate::tui::{
        app::App,
        model::{AccountDto, PoolResponse, ProviderDto, Snapshot},
    };

    fn draw(app: &App, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|f| render(f, app, 1_000)).unwrap();
        let buffer = terminal.backend().buffer();
        (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| buffer[(x, y)].symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn app(accounts: Vec<AccountDto>, sort_by_reset: bool) -> App {
        let mut app = App::new("http://127.0.0.1:3001".into(), Duration::from_secs(2));
        app.on_poll(Ok(Snapshot::from_response(PoolResponse {
            providers: vec![ProviderDto {
                provider: "claude".into(),
                accounts,
            }],
            sort_by_reset,
        })));
        app
    }

    #[test]
    fn until_formats_coarsely() {
        assert_eq!(until(1_000, 1_000), "now");
        assert_eq!(until(900, 1_000), "now");
        assert_eq!(until(1_000 + 30, 1_000), "1m");
        assert_eq!(until(1_000 + 3_600, 1_000), "1h");
        assert_eq!(until(1_000 + 3_660, 1_000), "1h 1m");
        assert_eq!(until(1_000 + 86_400 + 7_200, 1_000), "1d 2h");
    }

    #[test]
    fn renders_accounts_state_usage_and_policy() {
        let mut paused = AccountDto {
            name: "work".into(),
            account_ref: Some("r".into()),
            paused: true,
            has_state: true,
            utilization_5h: Some(0.5),
            reset_5h: Some(1_000 + 3_600),
            ..AccountDto::default()
        };
        paused.plan = Some("max".into());
        let screen = draw(&app(vec![paused], true), 110, 20);
        assert!(screen.contains("shunt pool"), "{screen}");
        assert!(screen.contains("1 paused"), "{screen}");
        assert!(screen.contains("soonest reset"), "{screen}");
        assert!(screen.contains("work"), "{screen}");
        assert!(screen.contains("paused"), "{screen}");
        assert!(screen.contains(" 50%"), "{screen}");
        assert!(screen.contains("1h"), "{screen}");
    }

    #[test]
    fn empty_and_unreachable_states_explain_themselves() {
        let waiting = App::new("http://x".into(), Duration::from_secs(2));
        assert!(draw(&waiting, 90, 12).contains("Waiting for the first poll"));
        let mut down = App::new("http://x".into(), Duration::from_secs(2));
        down.on_poll(Err("gateway unreachable".into()));
        let screen = draw(&down, 90, 12);
        assert!(screen.contains("Cannot reach the gateway"), "{screen}");
        assert!(screen.contains("gateway unreachable"), "{screen}");
    }

    #[test]
    fn renders_in_a_tiny_terminal_without_panicking() {
        let a = AccountDto {
            name: "a".into(),
            has_state: true,
            ..AccountDto::default()
        };
        let mut app = app(vec![a], false);
        app.show_help = true;
        draw(&app, 20, 6);
        draw(&app, 1, 1);
    }
}

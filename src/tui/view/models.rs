//! The Models dialog: one tab per CLI platform, each model on or off.

use super::*;
use crate::{
    models_catalog::PLATFORMS,
    models_prefs::Resolved,
    opencode_status::{self, State, Status},
    tui::models::{Catalog, ModelsDialog, OpencodeStatus, Ranking},
};

/// `reachable · 3h`, `rate limited · 12m`, or `unknown` for a model the log
/// does not mention (and for every model while the log is unreadable).
fn status_spans(status: Option<&Status>) -> Vec<Span<'static>> {
    match status {
        Some(s) => vec![
            Span::styled(format!("  {:<12}", s.state.label()), state_style(s.state)),
            Span::styled(
                format!("· {}", opencode_status::age_text(s.age)),
                Style::new().dark_gray(),
            ),
        ],
        None => vec![Span::styled("  unknown", Style::new().dark_gray())],
    }
}

fn state_style(state: State) -> Style {
    match state {
        State::Reachable => Style::new().fg(Color::Green),
        State::RateLimited => Style::new().fg(Color::Yellow).bold(),
        State::Unknown => Style::new().dark_gray(),
    }
}

/// The one summary line under the OpenCode list: how many models are reachable,
/// or why the log could not be read.
fn opencode_summary(dialog: &ModelsDialog, rows: &[Resolved]) -> Line<'static> {
    match &dialog.opencode {
        OpencodeStatus::Pending => {
            Line::styled("reading the OpenCode log…", Style::new().dark_gray())
        }
        OpencodeStatus::Failed(reason) => Line::styled(
            format!("OpenCode log unreadable ({reason}) — every model shows unknown"),
            Style::new().fg(Color::Red),
        ),
        OpencodeStatus::Ready(table) => {
            let reachable = rows
                .iter()
                .filter(|row| table.state(&row.id) == State::Reachable)
                .count();
            Line::styled(
                format!("{reachable} of {} reachable", rows.len()),
                Style::new().dark_gray(),
            )
        }
    }
}

/// The OpenCode ranking line: the run in progress, the last error (the old
/// ranking is kept), or when the stored ranking was made.
fn ranking_line(dialog: &ModelsDialog) -> Line<'static> {
    match &dialog.ranking {
        Ranking::Running => {
            Line::styled("ranking… (one run at a time)", Style::new().fg(Color::Cyan))
        }
        Ranking::Failed(error) => Line::styled(
            format!("rerank failed, old ranking kept: {error}"),
            Style::new().fg(Color::Red),
        ),
        Ranking::Idle => match &dialog.prefs.opencode_rank {
            Some(rank) => Line::styled(
                format!(
                    "ranked_at {} — {} unscored — r to rerank",
                    rank.ranked_at,
                    dialog.rows().iter().filter(|r| r.score.is_none()).count()
                ),
                Style::new().dark_gray(),
            ),
            None => Line::styled("not ranked yet — r to rerank", Style::new().dark_gray()),
        },
    }
}

pub(super) fn render(frame: &mut Frame, dialog: &ModelsDialog) {
    if let Some(picker) = &dialog.picker {
        let title = ["Ranker platform", "Ranker model", "Ranker effort"][picker.step];
        let rows = dialog.picker_rows();
        let room = frame.area().height.saturating_sub(10).max(1) as usize;
        let skip = picker.at.saturating_sub(room - 1);
        let mut lines: Vec<Line> = rows
            .into_iter()
            .enumerate()
            .skip(skip)
            .take(room)
            .map(|(at, row)| {
                Line::from(format!("{} {row}", if at == picker.at { "▶" } else { " " }))
            })
            .collect();
        if picker.step == 1 {
            let platform = crate::models_catalog::RANKER_PLATFORMS[picker.platform];
            match dialog.ranker_catalogs.get(platform) {
                None => lines.push(Line::from("loading the live model list…")),
                Some(Err(error)) => lines.push(Line::from(format!(
                    "cannot list models: {}",
                    error.replace(['\n', '\r'], " ")
                ))),
                _ => {}
            }
        }
        lines.push(Line::from("↑↓ choose · Enter continue/save · Esc cancel"));
        let rect = popup(frame.area(), 92, lines.len() as u16 + 4);
        frame.render_widget(Clear, rect);
        frame.render_widget(
            Paragraph::new(lines).block(Block::bordered().title(title)),
            rect,
        );
        return;
    }
    let filter = if dialog.prefs.filter {
        Span::styled("ON ", Style::new().fg(Color::Green).bold())
    } else {
        Span::styled("OFF", Style::new().fg(Color::Yellow).bold())
    };
    let tabs: Vec<Span> = PLATFORMS
        .iter()
        .enumerate()
        .flat_map(|(i, p)| {
            let style = if i == dialog.tab {
                Style::new().bold().reversed()
            } else {
                Style::new().dark_gray()
            };
            [Span::styled(format!(" {p} "), style), Span::raw(" ")]
        })
        .collect();
    let mut lines = vec![
        Line::from(vec![Span::raw("Model filter "), filter]),
        Line::styled(
            "ON: goal and /delegate-cli see only the models switched on here. OFF: they see all.",
            Style::new().dark_gray(),
        ),
        Line::default(),
        Line::from(tabs),
        Line::default(),
    ];
    if dialog.platform() == "opencode" {
        let ranker = &dialog.prefs.ranker;
        lines.push(Line::from(format!(
            "ranker: {} · {} · {}{}",
            ranker.platform(),
            ranker.model().unwrap_or("default"),
            ranker.effort().unwrap_or("default"),
            if ranker.platform() == "opencode" {
                " (effort ignored)"
            } else {
                ""
            }
        )));
    }
    match &dialog.catalogs[dialog.tab] {
        Catalog::Loading => lines.push(Line::from("loading the live model list…")),
        Catalog::Failed(error) => lines.push(Line::styled(
            format!("cannot list models: {}", error.replace(['\n', '\r'], " ")),
            Style::new().fg(Color::Red),
        )),
        Catalog::Ready(_) => {
            let rows = dialog.rows();
            let opencode = dialog.platform() == "opencode";
            let room = frame.area().height.saturating_sub(16).max(3) as usize;
            let skip = dialog.at.saturating_sub(room - 1);
            for (i, row) in rows.iter().enumerate().skip(skip).take(room) {
                let marker = if i == dialog.at { "▶ " } else { "  " };
                let (state, color) = if row.active {
                    ("on ", Color::Green)
                } else {
                    ("off", Color::DarkGray)
                };
                let note = if row.reason == "override" {
                    "  (set by you)"
                } else {
                    ""
                };
                let mut spans = vec![
                    Span::raw(marker),
                    Span::styled(format!("[{state}] "), Style::new().fg(color)),
                ];
                if let Some(rank) = row.rank {
                    spans.push(Span::styled(
                        format!("#{rank:<3}"),
                        Style::new().dark_gray(),
                    ));
                }
                if opencode {
                    spans.push(Span::styled(
                        row.score
                            .map_or_else(|| "   —  ".into(), |s| format!("{s:5.2} ")),
                        Style::new().dark_gray(),
                    ));
                }
                spans.push(Span::styled(
                    row.id.clone(),
                    if i == dialog.at {
                        Style::new().bold()
                    } else {
                        Style::new()
                    },
                ));
                spans.push(Span::styled(note, Style::new().dark_gray()));
                if opencode {
                    let status = match &dialog.opencode {
                        OpencodeStatus::Ready(table) => table.get(&row.id),
                        _ => None,
                    };
                    spans.extend(status_spans(status));
                }
                let mut line = Line::from(spans);
                if !row.active {
                    line = line.style(Style::new().add_modifier(Modifier::DIM));
                }
                lines.push(line);
            }
            if rows.is_empty() {
                lines.push(Line::from("no models listed"));
            }
            if opencode {
                lines.push(Line::default());
                lines.push(opencode_summary(dialog, &rows));
                lines.push(ranking_line(dialog));
            }
        }
    }
    lines.push(Line::default());
    lines.push(Line::styled(
        "Default: the newest version of each model is on. Gateway routing is not affected.",
        Style::new().dark_gray(),
    ));
    let height = lines.len() as u16 + 4;
    let rect = popup(frame.area(), 92, height);
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(lines).block(Block::bordered().title(" Models ")),
        rect,
    );
}

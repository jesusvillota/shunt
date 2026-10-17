//! The Models dialog: one tab per CLI platform, each model on or off.

use super::*;
use crate::{
    models_catalog::PLATFORMS,
    tui::models::{Catalog, ModelsDialog},
};

pub(super) fn render(frame: &mut Frame, dialog: &ModelsDialog) {
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
    match &dialog.catalogs[dialog.tab] {
        Catalog::Loading => lines.push(Line::from("loading the live model list…")),
        Catalog::Failed(error) => lines.push(Line::styled(
            format!("cannot list models: {}", error.replace(['\n', '\r'], " ")),
            Style::new().fg(Color::Red),
        )),
        Catalog::Ready(_) => {
            let rows = dialog.rows();
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
                lines.push(Line::from(vec![
                    Span::raw(marker),
                    Span::styled(format!("[{state}] "), Style::new().fg(color)),
                    Span::styled(
                        row.id.clone(),
                        if i == dialog.at {
                            Style::new().bold()
                        } else {
                            Style::new()
                        },
                    ),
                    Span::styled(note, Style::new().dark_gray()),
                ]));
            }
            if rows.is_empty() {
                lines.push(Line::from("no models listed"));
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

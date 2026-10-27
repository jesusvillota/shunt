//! Antigravity's upstream family windows, in the shared account columns.

use super::*;
use crate::tui::model::QuotaBucketDto;

pub(super) fn applies(provider: &ProviderView) -> bool {
    provider.auth.as_deref() == Some("antigravity_oauth")
}

struct Columns {
    labels: Vec<String>,
}

impl Columns {
    fn new(provider: &ProviderView) -> Self {
        let mut labels: Vec<String> = provider
            .rows
            .iter()
            .flat_map(|row| &row.account.quota_buckets)
            .map(|bucket| bucket.label.clone())
            .collect();
        // Weekly families first; show 5h here only when no weekly bucket exists.
        labels.sort_by_key(|label| {
            (
                label.ends_with(" · 5h"),
                !label.starts_with("Gemini"),
                label.clone(),
            )
        });
        labels.dedup();
        labels.truncate(2);
        if labels.is_empty() {
            labels.push(String::new());
        }
        Self { labels }
    }

    fn title(&self, label: &str) -> String {
        let title = if label.is_empty() {
            "Quota used".to_string()
        } else {
            label.replace(" Models", "")
        };
        let title = if title.chars().count() > WINDOW_W {
            title.replace(" · weekly", " · 7d")
        } else {
            title
        };
        fit(&title, WINDOW_W)
    }
}

/// The longest countdown form that fits whole: full, largest unit only, none.
fn cell(bucket: Option<&QuotaBucketDto>, now: u64, width: usize) -> Vec<Span<'static>> {
    let used = bucket
        .and_then(|b| b.remaining)
        .filter(|r| r.is_finite())
        .map(|r| 1.0 - r.clamp(0.0, 1.0));
    let full = bucket
        .and_then(QuotaBucketDto::reset_secs)
        .filter(|r| *r > 0)
        .map(|r| until(r, now));
    let room = width.saturating_sub(BAR_WIDTH);
    let countdown = full.as_deref().and_then(|full| {
        let short = full.split(' ').next().unwrap_or(full);
        [full, short].into_iter().find(|c| {
            format!(" {:>3.0}% · {c}", used.unwrap_or(0.0) * 100.0)
                .chars()
                .count()
                <= room
        })
    });
    let mut left = width;
    window_cell_with(used, countdown, width)
        .into_iter()
        .map(|mut span| {
            let text: String = span.content.chars().take(left).collect();
            left = left.saturating_sub(text.chars().count());
            span.content = text.into();
            span
        })
        .collect()
}

pub(super) fn heading(provider: &ProviderView) -> Line<'static> {
    let columns = Columns::new(provider);
    let mut spans = vec![Span::raw(format!(
        "{:>3} {}",
        "#",
        fit("Account", NAME_W + 1)
    ))];
    for (i, label) in columns.labels.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        spans.push(Span::raw(columns.title(label)));
    }
    // Fixed offsets like the shared heading; a narrow terminal clips, never reflows.
    spans.push(Span::raw(format!("  {}", fit("State", STATE_W + 1))));
    spans.push(Span::raw(fit("Requests ok/fail · avg", REQUESTS_W + 1)));
    spans.push(Span::raw("Plan"));
    Line::from(spans).dark_gray()
}

pub(super) fn account_line(entry: &Entry<'_>, now: u64, width: usize) -> Line<'static> {
    let (rank, row) = entry.row.expect("account entry");
    let a = &row.account;
    let columns = Columns::new(entry.provider);
    let rank = rank.map_or_else(|| "–".to_string(), |n| n.to_string());
    let mut spans = vec![
        Span::styled(format!("{rank:>3} "), Style::new().bold()),
        Span::raw(fit(&a.name, NAME_W + 1)),
    ];
    for (i, label) in columns.labels.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        spans.extend(cell(
            a.quota_buckets.iter().find(|b| &b.label == label),
            now,
            WINDOW_W,
        ));
    }
    let spent = |b: &QuotaBucketDto| b.remaining.is_some_and(|r| r.is_finite() && r <= 0.0);
    let exhausted = !a.quota_buckets.is_empty() && a.quota_buckets.iter().all(spent);
    let (state, style) = if exhausted
        && matches!(
            row.state,
            AccountState::Available | AccountState::NearQuota | AccountState::CoolingFable
        ) {
        ("exhausted", Style::new().red().bold())
    } else {
        (row.state.label(), state_style(row.state))
    };
    spans.push(Span::raw("  "));
    spans.push(Span::styled(fit(state, STATE_W + 1), style));
    spans.push(Span::styled(
        fit(&requests_cell(a), REQUESTS_W + 1),
        if a.requests_failed > 0 {
            Style::new().fg(Color::LightRed)
        } else {
            Style::new()
        },
    ));
    spans.push(Span::raw(format!(
        "{:<10}",
        a.plan.clone().unwrap_or_default()
    )));
    let mut line = Line::from(spans);
    // Extra reported windows fit as labelled cells after the usual columns.
    for bucket in &a.quota_buckets {
        if !columns.labels.contains(&bucket.label) {
            let label = format!("  {}: ", bucket.label.replace(" Models", ""));
            if line.width() + label.chars().count() + WINDOW_W <= width {
                line.spans.push(Span::raw(label));
                line.spans.extend(cell(Some(bucket), now, WINDOW_W));
            }
        }
    }
    if !entry.provider.is_on() || matches!(row.state, AccountState::Disabled | AccountState::Paused)
    {
        line = line.style(Style::new().add_modifier(Modifier::DIM));
    }
    line
}

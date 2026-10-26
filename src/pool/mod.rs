//! Read-only, non-interactive client for `GET /admin/api/pool`.
pub mod model;

use anyhow::{bail, Context};
use model::{AccountDto, PoolResponse, QuotaBucketDto};
use std::{fmt::Write as _, path::PathBuf, time::Duration};

pub struct Options {
    pub url: Option<String>,
    pub token: Option<String>,
    pub header: String,
    pub config: Option<PathBuf>,
    pub json: bool,
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

pub(crate) fn resolve_token(explicit: Option<String>) -> anyhow::Result<String> {
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

/// Shared with the TUI: preserve upstream labels, percentages and reset text.
pub(crate) fn quota_parts(bucket: &QuotaBucketDto, now: u64) -> (String, String) {
    let value = bucket
        .remaining
        .filter(|r| r.is_finite())
        .map_or_else(|| "—".into(), |r| format!("{:.0}% left", r * 100.0));
    let reset = bucket
        .reset_secs()
        .map_or_else(String::new, |r| format!(" · resets in {}", until(r, now)));
    (value, reset)
}

pub async fn run(options: Options) -> anyhow::Result<String> {
    let token = resolve_token(options.token)?;
    let base = options
        .url
        .unwrap_or_else(|| crate::dashboard::gateway_base_url(options.config.as_deref()));
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .context("building HTTP client")?;
    let response = http
        .get(format!("{}/admin/api/pool", base.trim_end_matches('/')))
        .header(options.header, token)
        .send()
        .await
        .context("gateway unreachable")?;
    let status = response.status();
    if !status.is_success() {
        match status.as_u16() {
            401 | 403 => bail!("admin token rejected ({})", status.as_u16()),
            404 => bail!("pool endpoint not found (404); enable [server.admin] on the gateway"),
            _ => bail!("gateway answered {status}"),
        }
    }
    let body = response
        .text()
        .await
        .context("could not read pool response")?;
    // Validate JSON without reserializing it: unknown fields and number precision survive.
    let parsed: serde_json::Value =
        serde_json::from_str(&body).context("unexpected pool response")?;
    if options.json {
        return Ok(body);
    }
    let pool: PoolResponse = serde_json::from_value(parsed).context("unexpected pool response")?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    Ok(render(&pool, now))
}

fn availability(account: &AccountDto) -> &'static str {
    if account.disabled {
        "disabled"
    } else if account.paused {
        "paused"
    } else if account.organization_blocked {
        "organization blocked"
    } else if account.needs_relogin {
        "needs relogin"
    } else if account.cooldown_secs_remaining.unwrap_or(0) > 0 {
        "cooling down"
    } else if account.near_quota {
        "near-quota"
    } else {
        "ok"
    }
}

fn window(usage: Option<f64>, reset: Option<u64>, now: u64) -> String {
    let value = usage
        .filter(|u| u.is_finite())
        .map_or_else(|| "—".into(), |u| format!("{:.0}% used", u * 100.0));
    let reset = reset
        .filter(|r| *r > 0)
        .map_or_else(String::new, |r| format!(" · resets in {}", until(r, now)));
    format!("{value}{reset}")
}

pub fn render(pool: &PoolResponse, now: u64) -> String {
    let mut text = String::new();
    for provider in &pool.providers {
        if !text.is_empty() {
            text.push('\n');
        }
        writeln!(text, "{}:", provider.provider).unwrap();
        for account in &provider.accounts {
            write!(text, "  {} | {}", account.name, availability(account)).unwrap();
            if !account.quota_buckets.is_empty()
                && (provider.auth.as_deref() == Some("antigravity_oauth")
                    || provider.provider == "antigravity")
            {
                for bucket in &account.quota_buckets {
                    let (value, reset) = quota_parts(bucket, now);
                    write!(text, " | {}: {value}{reset}", bucket.label).unwrap();
                }
            } else {
                write!(
                    text,
                    " | 5h: {} | 7d: {}",
                    window(account.utilization_5h, account.reset_5h, now),
                    window(account.utilization_7d, account.reset_7d, now)
                )
                .unwrap();
            }
            for (label, seconds) in [
                ("cooldown", account.cooldown_secs_remaining),
                ("fable cooldown", account.cooldown_fable_secs_remaining),
            ] {
                if let Some(seconds) = seconds.filter(|s| *s > 0) {
                    write!(
                        text,
                        " | {label}: {}",
                        until(now.saturating_add(seconds), now)
                    )
                    .unwrap();
                }
            }
            text.push('\n');
        }
    }
    if text.is_empty() {
        text.push_str("No pool providers.\n");
    }
    text
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

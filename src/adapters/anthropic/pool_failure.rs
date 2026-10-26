//! Retain useful failure evidence across Claude OAuth account attempts.

use std::time::{SystemTime, UNIX_EPOCH};

use axum::http::{HeaderValue, StatusCode};

use crate::{
    adapters::{error_body::buffer_error_body, AdapterError, ResponseBounds},
    config::AccountConfig,
    routing::Route,
    server::AppState,
};

pub(super) fn remember_response(
    remembered: &mut Option<(reqwest::Response, String)>,
    response: reqwest::Response,
    account: &str,
) {
    // Keep the last response among equal priorities, as the pool did before.
    // A quota failure must survive a later auth or server failure.
    if remembered.as_ref().is_some_and(|(current, _)| {
        current.status() == StatusCode::TOO_MANY_REQUESTS
            && response.status() != StatusCode::TOO_MANY_REQUESTS
    }) {
        return;
    }
    *remembered = Some((response, account.to_string()));
}

pub(super) async fn inspect_organization_block(
    state: &AppState,
    route: &Route,
    account: &AccountConfig,
    upstream: reqwest::Response,
    bounds: ResponseBounds,
) -> Result<(reqwest::Response, bool), AdapterError> {
    if upstream.status() != StatusCode::FORBIDDEN {
        return Ok((upstream, false));
    }
    // Inspect only small, complete error bodies. Oversized, slow or broken
    // bodies remain unclassified and are replayed without losing any bytes.
    let clock = bounds.idle.map(|idle| (idle, tokio::time::Instant::now()));
    let (upstream, body) = buffer_error_body(upstream, clock, true).await?;
    let blocked = body
        .as_deref()
        .and_then(|body| serde_json::from_slice::<serde_json::Value>(body).ok())
        .is_some_and(|body| {
            [
                "/error",
                "/error/type",
                "/error/code",
                "/error/message",
                "/type",
                "/code",
                "/message",
            ]
            .iter()
            .any(|path| {
                body.pointer(path).and_then(serde_json::Value::as_str)
                    == Some("oauth_not_allowed_for_organization")
            })
        });
    if blocked {
        state.accounts.block_organization(&route.provider, account);
        tracing::warn!(
            provider = %route.provider,
            account = %account.name,
            "Claude OAuth account excluded: oauth_not_allowed_for_organization"
        );
    }
    Ok((upstream, blocked))
}

/// Preserve the upstream's Retry-After. If absent, derive it from the rejected
/// quota windows. All rejected windows must reset before this account can serve.
pub(super) fn quota_retry_after(
    response: &mut reqwest::Response,
    is_fable: bool,
    snapshot: Option<&crate::accounts::AccountSnapshot>,
) {
    if response.status() != StatusCode::TOO_MANY_REQUESTS
        || crate::accounts::retry_after(response.headers()).is_some()
    {
        return;
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let reset = ["5h", "7d", "7d_oi"]
        .into_iter()
        .filter(|window| *window != "7d_oi" || is_fable)
        .filter(|window| {
            response
                .headers()
                .get(format!("anthropic-ratelimit-unified-{window}-status"))
                .is_some_and(|status| status == "rejected")
        })
        .filter_map(|window| {
            response
                .headers()
                .get(format!("anthropic-ratelimit-unified-{window}-reset"))
                .and_then(|value| value.to_str().ok()?.parse::<u64>().ok())
                .or_else(|| {
                    let snapshot = snapshot?;
                    match window {
                        "5h" => snapshot.reset_5h,
                        "7d" => snapshot.reset_7d,
                        "7d_oi" => snapshot.reset_7d_oi,
                        _ => None,
                    }
                })
        })
        .filter(|reset| *reset > now)
        .max();
    if let Some(reset) = reset {
        if let Ok(value) = HeaderValue::from_str(&(reset - now).to_string()) {
            response.headers_mut().insert("retry-after", value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{accounts::AccountPool, config::AccountConfig};
    use axum::http::{HeaderMap, Response};

    #[test]
    fn retry_after_uses_cached_resets_and_preserves_upstream_retry_after() {
        let pool = AccountPool::default();
        let account = AccountConfig {
            name: "quota".into(),
            ..Default::default()
        };
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let mut headers = HeaderMap::new();
        for (window, seconds) in [("5h", 120), ("7d", 240)] {
            headers.insert(
                format!("anthropic-ratelimit-unified-{window}-reset")
                    .parse::<axum::http::HeaderName>()
                    .unwrap(),
                HeaderValue::from_str(&(now + seconds).to_string()).unwrap(),
            );
            headers.insert(
                format!("anthropic-ratelimit-unified-{window}-status")
                    .parse::<axum::http::HeaderName>()
                    .unwrap(),
                HeaderValue::from_static("rejected"),
            );
        }
        pool.note_quota("anthropic", &account, &headers);
        let snapshots = pool.snapshot("anthropic", &[account], None, None);
        // The refusal carries statuses but omits resets that the pool already knows.
        let mut response = reqwest::Response::from(
            Response::builder()
                .status(429)
                .header("anthropic-ratelimit-unified-5h-status", "rejected")
                .header("anthropic-ratelimit-unified-7d-status", "rejected")
                .body("")
                .unwrap(),
        );
        quota_retry_after(&mut response, false, snapshots.first());
        let delay: u64 = response.headers()["retry-after"]
            .to_str()
            .unwrap()
            .parse()
            .unwrap();
        assert!(
            (235..=240).contains(&delay),
            "both rejected windows must reset: {delay}"
        );

        response
            .headers_mut()
            .insert("retry-after", HeaderValue::from_static("17"));
        quota_retry_after(&mut response, false, snapshots.first());
        assert_eq!(response.headers()["retry-after"], "17");
    }
}

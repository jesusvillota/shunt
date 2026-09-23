//! Antigravity pool quota client: Google's Code Assist `retrieveUserQuota` RPC.
//!
//! `POST {base_url}/{API_VERSION}:retrieveUserQuota` with the account's OAuth
//! bearer and a `{"project": <project_id>}` body reports the subscription's
//! per-model quota buckets — each with its own `remainingFraction` (0.0–1.0
//! remaining) and `resetTime`. This is the Antigravity pool account's *only*
//! quota signal: unlike Claude (`anthropic-ratelimit-unified-*` headers) and
//! Codex (`x-codex-*` headers), no reactive per-response header exists for
//! this family, so there is nothing to reconcile — the poller applies these
//! buckets directly via [`crate::accounts::AccountPool::note_antigravity_usage`].
//!
//! Two behaviors here are live-tested observations, not a documented Google API
//! contract (same caveat class as the Codex wham endpoint — see
//! `docs/m10-codex-multi-account.md`): the RPC **requires the Antigravity Hub
//! `User-Agent`** ([`super::version::user_agent`]) — the same token/project
//! gets `403 PERMISSION_DENIED / SUBSCRIPTION_REQUIRED` without it — and the
//! response carries ~28 per-model buckets with independent resets rather than
//! an account-wide window.

use anyhow::Context;

use crate::accounts::QuotaBucketSnapshot;

/// Fetch one Antigravity account's per-model quota buckets. `base_url` is the
/// provider's base (a vetted Code Assist host); `access_token` is a valid
/// refreshable-login bearer and `project_id` the account's Code Assist project.
pub async fn fetch_usage(
    client: &reqwest::Client,
    base_url: &str,
    access_token: &str,
    project_id: &str,
) -> anyhow::Result<Vec<QuotaBucketSnapshot>> {
    let url = format!(
        "{}/{API_VERSION}:retrieveUserQuota",
        base_url.trim_end_matches('/'),
        API_VERSION = super::auth::API_VERSION,
    );
    let response = client
        .post(&url)
        .bearer_auth(access_token)
        .header("User-Agent", super::version::user_agent())
        .json(&serde_json::json!({ "project": project_id }))
        // The shared client carries no default timeout; bound this background
        // poll so a hung connection can never stall the poller task indefinitely.
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await?;
    let status = response.status();
    let text = response
        .text()
        .await
        .context("Antigravity usage response body read failed")?;
    if !status.is_success() {
        let detail: String = text.chars().take(200).collect();
        anyhow::bail!("usage request failed ({status}): {detail}");
    }
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|error| anyhow::anyhow!("invalid usage response: {error}"))?;
    parse_usage(&value)
}

/// Parse the `retrieveUserQuota` JSON into per-model quota buckets. Returns an
/// error when the response carries no recognizable `buckets` array at all, so
/// the poller never trusts an unrelated payload; an empty bucket list is
/// `Ok(vec![])` and left to the poller (which skips applying it, like the
/// Claude/Codex empty-snapshot guards).
fn parse_usage(value: &serde_json::Value) -> anyhow::Result<Vec<QuotaBucketSnapshot>> {
    let buckets = value.get("buckets").and_then(serde_json::Value::as_array);
    let Some(buckets) = buckets else {
        anyhow::bail!("usage response carries no recognizable quota buckets");
    };
    Ok(buckets
        .iter()
        .map(|bucket| QuotaBucketSnapshot {
            label: bucket
                .get("modelId")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown")
                .to_string(),
            remaining: bucket
                .get("remainingFraction")
                .and_then(serde_json::Value::as_f64),
            reset_time: bucket
                .get("resetTime")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_real_quota_response_shape() {
        // Trimmed sample of the live `retrieveUserQuota` response shape: a
        // per-model bucket list with independent remaining fractions and
        // reset times.
        let value = serde_json::json!({
            "buckets": [
                {
                    "modelId": "claude-opus-4-6-thinking",
                    "remainingFraction": 0.92,
                    "remainingAmount": "920",
                    "resetTime": "2026-09-24T00:00:00Z"
                },
                {
                    "modelId": "gemini-3.5-flash",
                    "remainingFraction": 0.41,
                    "resetTime": "2026-09-25T12:00:00Z"
                },
                {
                    "remainingFraction": 0.5
                }
            ]
        });
        let buckets = parse_usage(&value).expect("recognizable buckets parse");
        assert_eq!(buckets.len(), 3);
        assert_eq!(buckets[0].label, "claude-opus-4-6-thinking");
        assert_eq!(buckets[0].remaining, Some(0.92));
        assert_eq!(
            buckets[0].reset_time.as_deref(),
            Some("2026-09-24T00:00:00Z")
        );
        assert_eq!(buckets[1].label, "gemini-3.5-flash");
        assert_eq!(buckets[1].remaining, Some(0.41));
        // A bucket with no model id still parses, labelled unknown.
        assert_eq!(buckets[2].label, "unknown");
        assert_eq!(buckets[2].remaining, Some(0.5));
        assert_eq!(buckets[2].reset_time, None);
    }

    #[test]
    fn rejects_response_without_buckets() {
        let value = serde_json::json!({ "garbage": true });
        assert!(
            parse_usage(&value).is_err(),
            "an unrelated payload must not parse as quota"
        );
    }

    #[tokio::test]
    async fn fetch_usage_sends_hub_user_agent() {
        use wiremock::matchers::{header, method};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        // Regression guard for the live-tested finding: without the Antigravity
        // Hub User-Agent, Google 403s the same token/project.
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(header("user-agent", super::super::version::user_agent()))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "buckets": [
                    { "modelId": "gemini-3.5-flash", "remainingFraction": 0.7 }
                ]
            })))
            .expect(1)
            .mount(&server)
            .await;

        let buckets = fetch_usage(&reqwest::Client::new(), &server.uri(), "token", "project")
            .await
            .expect("usage fetch succeeds");
        assert_eq!(buckets.len(), 1);
        assert_eq!(buckets[0].label, "gemini-3.5-flash");
        assert_eq!(buckets[0].remaining, Some(0.7));
    }

    #[tokio::test]
    async fn fetch_usage_errors_on_non_success() {
        use wiremock::matchers::method;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(403).set_body_string("PERMISSION_DENIED"))
            .mount(&server)
            .await;

        let error = fetch_usage(&reqwest::Client::new(), &server.uri(), "token", "project")
            .await
            .expect_err("a 403 must surface as an error");
        assert!(error.to_string().contains("403"), "got: {error}");
    }
}

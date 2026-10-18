//! Window primer, opt-in via `[server.pool] prime_windows` (runs on the usage
//! poller's tick, so it also needs `usage_refresh_seconds`).
//!
//! A subscription account's 5-hour window only starts on its first request.
//! A backup account that sits unused therefore starts its 5 hours only when
//! the pool fails over to it, and its reset lands later than it would have if
//! the window had already been running. The primer starts every idle window
//! as soon as it is seen: when the pool knows no future 5-hour reset for a
//! `claude_oauth` or ChatGPT-backend `chatgpt_oauth` account, it sends that
//! one account the smallest real request (Haiku / GPT-6 Luna, low effort, a
//! one-word prompt) signed with that account's own credential, so it cannot
//! land on any other account. The response's quota headers are fed to the
//! pool, which then knows the new reset and stops treating the window as idle.
//!
//! Hard limit: at most one primer request per physical account per 5 hours,
//! counted from the attempt, whether it succeeded or not.

use std::{
    collections::HashMap,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde_json::json;

use crate::{
    accounts::{account_key, AccountKey, StoreFamily},
    adapters::responses::request::{CODEX_CLIENT_VERSION, CODEX_USER_AGENT},
    auth::{
        self, claude, claude::auth::CLAUDE_CLI_USER_AGENT, codex, resolve_chatgpt_account,
        resolve_claude_account, Credential,
    },
    config::{AccountConfig, AuthMode},
    server::AppState,
};

const WINDOW: Duration = Duration::from_secs(5 * 60 * 60);
const CLAUDE_MODEL: &str = "claude-haiku-5-5";
const CODEX_MODEL: &str = "gpt-6-luna";
/// Claude subscription OAuth answers a bare 429 to a request that does not
/// identify as Claude Code (checked live 2026-10-18), so the primer sends the
/// same identity system block and headers Claude Code does.
const CLAUDE_CODE_IDENTITY: &str = "You are Claude Code, Anthropic's official CLI for Claude.";

/// Last primer attempt per physical account. The key carries the store family
/// and identity, so two provider aliases of one account share one entry.
#[derive(Default)]
pub(crate) struct WindowPrimer {
    attempted: HashMap<AccountKey, Instant>,
}

impl WindowPrimer {
    /// Prime every idle window once, if `prime_windows` is on in the live config.
    pub(crate) async fn run(&mut self, state: &AppState) {
        let state = state.refreshed();
        let Some(pool) = state
            .config
            .server
            .pool
            .as_ref()
            .filter(|p| p.prime_windows)
        else {
            return;
        };
        for (name, provider) in &state.config.providers {
            let (label, family, dir, scan): (_, _, _, fn() -> _) = match provider.auth {
                AuthMode::ClaudeOauth => (
                    "Claude",
                    StoreFamily::Claude,
                    claude::store::default_accounts_dir(),
                    claude::store::scan_accounts,
                ),
                AuthMode::ChatgptOauth if state.config.is_chatgpt_backend(name) => (
                    "Codex",
                    StoreFamily::Chatgpt,
                    codex::store::default_accounts_dir(),
                    codex::store::scan_accounts,
                ),
                _ => continue,
            };
            let accounts = match auth::shared::resolve_pool_accounts(
                label,
                &provider.accounts,
                &provider.account_scope,
                family,
                dir,
                scan,
            )
            .await
            {
                Ok(accounts) => accounts,
                Err(error) => {
                    tracing::debug!(provider = %name, %error, "window primer: failed to resolve accounts");
                    continue;
                }
            };
            let snapshots = state.accounts.snapshot(name, &accounts, None, Some(pool));
            let now = unix_now();
            for (account, snapshot) in accounts.iter().zip(snapshots) {
                // A future reset means the window is already running. Paused or
                // relogin-pending accounts are left alone.
                if snapshot.reset_5h.is_some_and(|reset| reset > now)
                    || snapshot.paused
                    || snapshot.needs_relogin
                {
                    continue;
                }
                if !self.claim(account_key(name, account), Instant::now()) {
                    continue;
                }
                let base_url = provider.base_url.trim_end_matches('/');
                match family {
                    StoreFamily::Claude => prime_claude(&state, name, base_url, account).await,
                    _ => prime_codex(&state, name, base_url, account).await,
                }
            }
        }
    }

    /// Record an attempt for `key` unless one happened within the last window.
    fn claim(&mut self, key: AccountKey, now: Instant) -> bool {
        if self
            .attempted
            .get(&key)
            .is_some_and(|last| now.duration_since(*last) < WINDOW)
        {
            return false;
        }
        self.attempted.insert(key, now);
        true
    }
}

async fn prime_claude(state: &AppState, provider: &str, base_url: &str, account: &AccountConfig) {
    let access_token = match resolve_claude_account(account, &state.http_client).await {
        Ok(Credential::ClaudeOauth { access_token, .. }) => access_token,
        Ok(_) => return,
        Err(error) => {
            tracing::debug!(provider, account = %account.name, error = %error.message, "window primer: failed to resolve credential");
            return;
        }
    };
    let response = state
        .http_client
        .post(format!("{base_url}/v1/messages"))
        .bearer_auth(access_token)
        .header("anthropic-version", "2023-06-01")
        .header("anthropic-beta", "claude-code-20250219,oauth-2025-04-20")
        .header("user-agent", CLAUDE_CLI_USER_AGENT)
        .header("x-app", "cli")
        .json(&json!({
            "model": CLAUDE_MODEL,
            "max_tokens": 1,
            "system": [{"type": "text", "text": CLAUDE_CODE_IDENTITY}],
            "messages": [{"role": "user", "content": "hi"}],
            "output_config": {"effort": "low"},
        }))
        .timeout(Duration::from_secs(60))
        .send()
        .await;
    match response {
        Ok(response) => {
            state
                .accounts
                .note_quota(provider, account, response.headers());
            log_outcome(provider, account, response).await;
        }
        Err(error) => {
            tracing::warn!(provider, account = %account.name, %error, "window primer: request failed")
        }
    }
}

async fn prime_codex(state: &AppState, provider: &str, base_url: &str, account: &AccountConfig) {
    let (access_token, account_id) = match resolve_chatgpt_account(account, &state.http_client)
        .await
    {
        Ok(Credential::ChatGptOAuth {
            access_token,
            account_id,
        }) => (access_token, account_id),
        Ok(_) => return,
        Err(error) => {
            tracing::debug!(provider, account = %account.name, error = %error.detail, "window primer: failed to resolve credential");
            return;
        }
    };
    // The ChatGPT backend serves only streamed, unstored turns.
    let response = state
        .http_client
        .post(format!("{base_url}/codex/responses"))
        .bearer_auth(access_token)
        .header("chatgpt-account-id", account_id)
        .header("originator", "codex_cli_rs")
        .header("user-agent", CODEX_USER_AGENT)
        .header("version", CODEX_CLIENT_VERSION)
        .header("accept", "text/event-stream")
        .json(&json!({
            "model": CODEX_MODEL,
            "instructions": "Reply with one word.",
            "input": [{"role": "user", "content": [{"type": "input_text", "text": "hi"}]}],
            "reasoning": {"effort": "low"},
            "stream": true,
            "store": false,
        }))
        .timeout(Duration::from_secs(60))
        .send()
        .await;
    match response {
        Ok(response) => {
            state
                .accounts
                .note_codex_quota(provider, account, response.headers());
            log_outcome(provider, account, response).await;
        }
        Err(error) => {
            tracing::warn!(provider, account = %account.name, %error, "window primer: request failed")
        }
    }
}

/// Read the body to the end (so the turn is not cancelled mid-stream) and log
/// the result. The primer never marks an account unhealthy: a failure only
/// means this window stays idle until the next attempt 5 hours later.
async fn log_outcome(provider: &str, account: &AccountConfig, response: reqwest::Response) {
    let status = response.status();
    let body = response.bytes().await.unwrap_or_default();
    if status.is_success() {
        tracing::info!(provider, account = %account.name, "window primer: started 5-hour window");
    } else {
        let detail: String = String::from_utf8_lossy(&body).chars().take(200).collect();
        tracing::warn!(provider, account = %account.name, %status, %detail, "window primer: request rejected");
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        ApiKeyHeader, Config, CountTokens, PoolConfig, ProviderConfig, ProviderKind,
    };
    use axum::http::HeaderMap;
    use wiremock::{
        matchers::{body_partial_json, header, method, path},
        Mock, MockServer, ResponseTemplate,
    };

    fn credential_file(name: &str, token: &str) -> std::path::PathBuf {
        let expires_at_ms = (SystemTime::now() + Duration::from_secs(3_600))
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let path = std::env::temp_dir().join(format!(
            "shunt-window-primer-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let value = json!({"claudeAiOauth": {
            "accessToken": token, "refreshToken": "r", "expiresAt": expires_at_ms
        }});
        std::fs::write(&path, value.to_string()).unwrap();
        path
    }

    fn account(name: &str, credentials: &std::path::Path) -> AccountConfig {
        AccountConfig {
            name: name.to_string(),
            credentials: Some(credentials.to_string_lossy().into_owned()),
            ..Default::default()
        }
    }

    fn state(base_url: String, accounts: Vec<AccountConfig>, prime_windows: bool) -> AppState {
        let mut config = Config::default();
        config.server.pool = Some(PoolConfig {
            prime_windows,
            ..Default::default()
        });
        config.providers.insert(
            "claude-pool".to_string(),
            ProviderConfig {
                kind: ProviderKind::Anthropic,
                base_url,
                auth: AuthMode::ClaudeOauth,
                api_key_env: None,
                api_key_header: ApiKeyHeader::Bearer,
                effort: None,
                service_tier: None,
                classifier_model: None,
                count_tokens: CountTokens::default(),
                accounts,
                account_scope: Vec::new(),
                websocket: false,
                tool_search: None,
                request_compression: true,
                retry: Default::default(),
                workspace_roots: Vec::new(),
                sandbox: true,
                profile_dir: None,
            },
        );
        AppState::new(config, reqwest::Client::new()).unwrap()
    }

    /// Accounts A and B already have a running window; C has none. Only C is
    /// primed, with its own token, and a second run sends nothing more.
    #[tokio::test]
    async fn primes_only_the_idle_account_once() {
        let reset = unix_now() + 3_600;
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .and(header("authorization", "Bearer token-c"))
            .and(body_partial_json(
                json!({"model": CLAUDE_MODEL, "max_tokens": 1}),
            ))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("anthropic-ratelimit-unified-5h-reset", reset.to_string())
                    .insert_header("anthropic-ratelimit-unified-5h-utilization", "0.0")
                    .set_body_json(json!({"type": "message", "content": []})),
            )
            .expect(1)
            .mount(&server)
            .await;
        // Any other request (e.g. to A or B) fails the test.
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(500))
            .with_priority(10)
            .expect(0)
            .mount(&server)
            .await;

        let files: Vec<_> = ["a", "b", "c"]
            .iter()
            .map(|name| credential_file(name, &format!("token-{name}")))
            .collect();
        let accounts: Vec<_> = ["a", "b", "c"]
            .iter()
            .zip(&files)
            .map(|(name, file)| account(name, file))
            .collect();
        let state = state(server.uri(), accounts.clone(), true);
        let mut running = HeaderMap::new();
        running.insert(
            "anthropic-ratelimit-unified-5h-reset",
            reset.to_string().parse().unwrap(),
        );
        state
            .accounts
            .note_quota("claude-pool", &accounts[0], &running);
        state
            .accounts
            .note_quota("claude-pool", &accounts[1], &running);

        let mut primer = WindowPrimer::default();
        primer.run(&state).await;
        primer.run(&state).await;

        let snapshots = state
            .accounts
            .snapshot("claude-pool", &accounts, None, None);
        assert_eq!(
            snapshots[2].reset_5h,
            Some(reset),
            "primer response feeds the pool"
        );
        for file in files {
            let _ = std::fs::remove_file(file);
        }
        // Dropping `server` verifies `.expect(1)`: exactly one request, to C.
    }

    #[tokio::test]
    async fn sends_nothing_when_disabled() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&server)
            .await;
        let file = credential_file("off", "token-off");
        let state = state(server.uri(), vec![account("off", &file)], false);

        WindowPrimer::default().run(&state).await;
        let _ = std::fs::remove_file(file);
    }

    #[test]
    fn claim_allows_one_attempt_per_window() {
        let mut primer = WindowPrimer::default();
        let key = account_key("p", &AccountConfig::default());
        let start = Instant::now();
        assert!(primer.claim(key.clone(), start));
        assert!(!primer.claim(key.clone(), start + WINDOW - Duration::from_secs(1)));
        assert!(primer.claim(key, start + WINDOW));
    }
}

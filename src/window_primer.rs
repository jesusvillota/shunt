//! Window primer, opt-in via `[server.pool] prime_windows` (runs on the usage
//! poller's tick, so it also needs `usage_refresh_seconds`).
//!
//! A subscription account's 5-hour window only starts on its first request.
//! A backup account that sits unused therefore starts its 5 hours only when
//! the pool fails over to it, and its reset lands later than it would have if
//! the window had already been running. The primer starts every idle window
//! as soon as it is seen: when the pool knows no future 5-hour reset for a
//! `claude_oauth` or ChatGPT-backend `chatgpt_oauth` account, the primer asks
//! that account's usage API, and only when it confirms the window is idle does
//! it send that one account the smallest real request (Haiku / GPT-6 Luna, low
//! effort, a one-word prompt) signed with that account's own credential, so it
//! cannot land on any other account. The usage check matters because the pool
//! can lack a reset for a running window: after a restart, or for Codex, whose
//! usage poll does not adopt reset times. A failed check sends nothing. The
//! response's quota headers are fed to the pool.
//!
//! Hard limit: at most one primer request per physical account per 5 hours,
//! counted from the attempt, whether it succeeded or not.

use std::{
    collections::HashMap,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde_json::json;

use crate::{
    accounts::{account_key, AccountKey, StoreFamily, UsageWindow},
    adapters::responses::request::{CODEX_CLIENT_VERSION, CODEX_USER_AGENT},
    auth::{
        self, claude, claude::auth::CLAUDE_CLI_USER_AGENT, codex, resolve_chatgpt_account,
        resolve_claude_account, Credential,
    },
    config::{AccountConfig, AuthMode},
    server::AppState,
};

const WINDOW: u64 = 5 * 60 * 60;
/// A reported reset at least this close to a full window away reads as idle;
/// see [`running_reset`].
const FRESH_WINDOW_SLACK: u64 = 120;
const CLAUDE_MODEL: &str = "claude-haiku-5-5";
const CODEX_MODEL: &str = "gpt-6-luna";
/// Claude subscription OAuth answers a bare 429 to a request that does not
/// identify as Claude Code (checked live 2026-10-18), so the primer sends the
/// same identity system block and headers Claude Code does.
const CLAUDE_CODE_IDENTITY: &str = "You are Claude Code, Anthropic's official CLI for Claude.";

/// Unix time before which an account is not checked again: 5 hours after a
/// primer attempt, or the reset of a window the usage API showed running. The
/// key carries the store family and identity, so two provider aliases of one
/// account share one entry.
#[derive(Default)]
pub(crate) struct WindowPrimer {
    next_check: HashMap<AccountKey, u64>,
}

/// What one account's check did.
enum Outcome {
    /// A primer request was sent (whatever its result).
    Sent,
    /// The usage API showed the window running until this reset.
    Running(u64),
    /// No credential or no usage answer; nothing sent, check again next tick.
    Unknown,
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
                // A reset inside the window means it is already running (a
                // reset a full window away reads as idle, as in
                // [`running_reset`]). Paused or relogin-pending accounts are
                // left alone.
                if snapshot
                    .reset_5h
                    .is_some_and(|reset| is_running(reset, now))
                    || snapshot.organization_blocked
                    || snapshot.paused
                    || snapshot.needs_relogin
                {
                    continue;
                }
                let key = account_key(name, account);
                if self.next_check.get(&key).is_some_and(|at| now < *at) {
                    continue;
                }
                let base_url = provider.base_url.trim_end_matches('/');
                let outcome = match family {
                    StoreFamily::Claude => prime_claude(&state, name, base_url, account).await,
                    _ => prime_codex(&state, name, base_url, account).await,
                };
                self.record(key, outcome, now);
            }
        }
    }

    fn record(&mut self, key: AccountKey, outcome: Outcome, now: u64) {
        match outcome {
            Outcome::Sent => self.next_check.insert(key, now + WINDOW),
            Outcome::Running(reset) => self.next_check.insert(key, reset),
            Outcome::Unknown => None,
        };
    }
}

/// The reset of a running 5-hour window, or `None` when the usage API shows it
/// idle. A reset a full window away also reads as idle: a never-used window
/// may report a reset that moves with the clock, and a window that really
/// started in the last two minutes costs at most one extra tiny request.
fn running_reset(five_hour: Option<&UsageWindow>, now: u64) -> Option<u64> {
    five_hour
        .and_then(|window| window.resets_at)
        .filter(|reset| is_running(*reset, now))
}

fn is_running(reset: u64, now: u64) -> bool {
    reset > now && reset < now + WINDOW - FRESH_WINDOW_SLACK
}

/// Turn a usage-API answer into "send" (`None`) or a non-sending outcome.
fn idle_check(
    provider: &str,
    account: &AccountConfig,
    usage: anyhow::Result<crate::accounts::UsageSnapshot>,
) -> Option<Outcome> {
    match usage {
        Ok(usage) => running_reset(usage.five_hour.as_ref(), unix_now()).map(Outcome::Running),
        Err(error) => {
            tracing::debug!(provider, account = %account.name, %error, "window primer: usage check failed; not priming");
            Some(Outcome::Unknown)
        }
    }
}

async fn prime_claude(
    state: &AppState,
    provider: &str,
    base_url: &str,
    account: &AccountConfig,
) -> Outcome {
    let access_token = match resolve_claude_account(account, &state.http_client).await {
        Ok(Credential::ClaudeOauth { access_token, .. }) => access_token,
        Ok(_) => return Outcome::Unknown,
        Err(error) => {
            tracing::debug!(provider, account = %account.name, error = %error.message, "window primer: failed to resolve credential");
            return Outcome::Unknown;
        }
    };
    let usage = claude::usage::fetch_usage(&state.http_client, base_url, &access_token).await;
    if let Some(outcome) = idle_check(provider, account, usage) {
        return outcome;
    }
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
    Outcome::Sent
}

async fn prime_codex(
    state: &AppState,
    provider: &str,
    base_url: &str,
    account: &AccountConfig,
) -> Outcome {
    let (access_token, account_id) = match resolve_chatgpt_account(account, &state.http_client)
        .await
    {
        Ok(Credential::ChatGptOAuth {
            access_token,
            account_id,
        }) => (access_token, account_id),
        Ok(_) => return Outcome::Unknown,
        Err(error) => {
            tracing::debug!(provider, account = %account.name, error = %error.detail, "window primer: failed to resolve credential");
            return Outcome::Unknown;
        }
    };
    let usage =
        codex::usage::fetch_usage(&state.http_client, base_url, &access_token, &account_id).await;
    if let Some(outcome) = idle_check(provider, account, usage) {
        return outcome;
    }
    // The ChatGPT backend serves only streamed, unstored turns.
    let sent_at = unix_now();
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
            if log_outcome(provider, account, response).await {
                // The window starts with this request.
                state
                    .accounts
                    .note_started_window(provider, account, sent_at + WINDOW);
            }
        }
        Err(error) => {
            tracing::warn!(provider, account = %account.name, %error, "window primer: request failed")
        }
    }
    Outcome::Sent
}

/// Read the body to the end (so the turn is not cancelled mid-stream) and log
/// the result. The primer never marks an account unhealthy: a failure only
/// means this window stays idle until the next attempt 5 hours later.
/// Returns whether the request succeeded.
async fn log_outcome(provider: &str, account: &AccountConfig, response: reqwest::Response) -> bool {
    let status = response.status();
    let body = response.bytes().await.unwrap_or_default();
    if status.is_success() {
        tracing::info!(provider, account = %account.name, "window primer: started 5-hour window");
    } else {
        let detail: String = String::from_utf8_lossy(&body).chars().take(200).collect();
        tracing::warn!(provider, account = %account.name, %status, %detail, "window primer: request rejected");
    }
    status.is_success()
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
        // The built-in `codex` provider would scan the real account store and
        // call chatgpt.com; prime only the mock provider below.
        config.providers.remove("codex");
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
                upstream_ttfb_ms: None,
                retry: Default::default(),
                workspace_roots: Vec::new(),
                sandbox: true,
                profile_dir: None,
            },
        );
        AppState::new(config, reqwest::Client::new()).unwrap()
    }

    async fn mount_usage(server: &MockServer, token: &str, response: ResponseTemplate, times: u64) {
        Mock::given(method("GET"))
            .and(path("/api/oauth/usage"))
            .and(header("authorization", format!("Bearer {token}").as_str()))
            .respond_with(response)
            .expect(times)
            .mount(server)
            .await;
    }

    /// Fails the test on any POST no earlier mock matched.
    async fn forbid_other_posts(server: &MockServer) {
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(500))
            .with_priority(10)
            .expect(0)
            .mount(server)
            .await;
    }

    fn idle_usage() -> ResponseTemplate {
        ResponseTemplate::new(200)
            .set_body_json(json!({"five_hour": {"utilization": 0.0, "resets_at": null}}))
    }

    /// Accounts A and B already have a running window in the pool; C has
    /// none, and its usage API confirms it idle. Only C is primed, with its
    /// own token, and a second run sends nothing more.
    #[tokio::test]
    async fn primes_only_the_idle_account_once() {
        let reset = unix_now() + 3_600;
        let server = MockServer::start().await;
        mount_usage(&server, "token-c", idle_usage(), 1).await;
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
        forbid_other_posts(&server).await;

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
        // Dropping `server` verifies every `.expect`: one request, to C.
    }

    /// A reset a full window away is not a running window (see
    /// [`running_reset`]): the pool check must not skip such an account.
    #[tokio::test]
    async fn primes_an_account_whose_pool_reset_is_a_full_window_away() {
        let server = MockServer::start().await;
        mount_usage(&server, "token-m", idle_usage(), 1).await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .and(header("authorization", "Bearer token-m"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"content": []})))
            .expect(1)
            .mount(&server)
            .await;
        let file = credential_file("m", "token-m");
        let accounts = vec![account("m", &file)];
        let state = state(server.uri(), accounts.clone(), true);
        let mut moving = HeaderMap::new();
        moving.insert(
            "anthropic-ratelimit-unified-5h-reset",
            (unix_now() + WINDOW).to_string().parse().unwrap(),
        );
        state
            .accounts
            .note_quota("claude-pool", &accounts[0], &moving);

        WindowPrimer::default().run(&state).await;
        let _ = std::fs::remove_file(file);
    }

    /// The pool knows no reset (as after a restart), but the usage API shows
    /// the window running: nothing is sent, and the usage API is not asked
    /// again before that reset.
    #[tokio::test]
    async fn skips_a_window_the_usage_api_shows_running() {
        let reset = unix_now() + 3_600;
        let server = MockServer::start().await;
        let running = ResponseTemplate::new(200).set_body_json(json!({
            "five_hour": {"utilization": 12.0, "resets_at": crate::auth::shared::format_iso8601(UNIX_EPOCH + Duration::from_secs(reset))}
        }));
        mount_usage(&server, "token-r", running, 1).await;
        forbid_other_posts(&server).await;
        let file = credential_file("r", "token-r");
        let state = state(server.uri(), vec![account("r", &file)], true);

        let mut primer = WindowPrimer::default();
        primer.run(&state).await;
        primer.run(&state).await;
        let _ = std::fs::remove_file(file);
    }

    /// A failed usage check sends nothing and is retried on the next run.
    #[tokio::test]
    async fn sends_nothing_when_the_usage_check_fails() {
        let server = MockServer::start().await;
        mount_usage(&server, "token-f", ResponseTemplate::new(429), 2).await;
        forbid_other_posts(&server).await;
        let file = credential_file("f", "token-f");
        let state = state(server.uri(), vec![account("f", &file)], true);

        let mut primer = WindowPrimer::default();
        primer.run(&state).await;
        primer.run(&state).await;
        let _ = std::fs::remove_file(file);
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

    /// The reply that starts a Codex window carries no reset header, so the
    /// pool learns the reset from the time the primer request was sent.
    #[tokio::test]
    async fn a_primed_codex_window_gets_a_reset_without_reset_headers() {
        use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(codex::usage::USAGE_PATH))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "rate_limit": {"primary_window": {
                    "used_percent": 0, "limit_window_seconds": 18000, "reset_at": null
                }}
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/codex/responses"))
            .and(body_partial_json(json!({"model": CODEX_MODEL})))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("x-codex-primary-window-minutes", "300")
                    .insert_header("x-codex-primary-used-percent", "0"),
            )
            .expect(1)
            .mount(&server)
            .await;
        forbid_other_posts(&server).await;

        let payload = json!({
            "exp": unix_now() + 3_600,
            "https://api.openai.com/auth": {"chatgpt_account_id": "acct-x"}
        });
        let token = format!("x.{}.y", URL_SAFE_NO_PAD.encode(payload.to_string()));
        let file =
            std::env::temp_dir().join(format!("shunt-window-primer-codex-{}", std::process::id()));
        std::fs::write(
            &file,
            json!({"auth_mode": "ChatGPT", "tokens": {"access_token": token, "account_id": "acct-x"}})
                .to_string(),
        )
        .unwrap();
        // The id the primer resolves from the token keys the pool entry.
        let account = AccountConfig {
            uuid: Some("acct-x".to_string()),
            ..account("x", &file)
        };
        let mut config = Config::default();
        config.server.pool = Some(PoolConfig {
            prime_windows: true,
            ..Default::default()
        });
        let codex = config.providers.get_mut("codex").unwrap();
        codex.base_url = server.uri();
        codex.accounts = vec![account.clone()];
        let state = AppState::new(config, reqwest::Client::new()).unwrap();

        let before = unix_now();
        WindowPrimer::default().run(&state).await;
        let reset = state.accounts.snapshot("codex", &[account], None, None)[0].reset_5h;
        let _ = std::fs::remove_file(file);
        assert!(
            reset.is_some_and(|reset| (before + WINDOW..=unix_now() + WINDOW).contains(&reset)),
            "reset {reset:?} is 5 hours after the primer request"
        );
    }

    #[test]
    fn running_reset_reads_idle_and_running_windows() {
        let now = 1_000_000;
        let window = |resets_at| UsageWindow {
            utilization: 0.0,
            resets_at,
        };
        assert_eq!(running_reset(None, now), None, "no window reported");
        assert_eq!(running_reset(Some(&window(None)), now), None, "no reset");
        assert_eq!(
            running_reset(Some(&window(Some(now))), now),
            None,
            "reset passed"
        );
        assert_eq!(
            running_reset(Some(&window(Some(now + WINDOW))), now),
            None,
            "a reset a full window away moves with the clock"
        );
        assert_eq!(
            running_reset(Some(&window(Some(now + 3_600))), now),
            Some(now + 3_600)
        );
    }

    #[test]
    fn a_sent_primer_blocks_the_account_for_one_window() {
        let mut primer = WindowPrimer::default();
        let key = account_key("p", &AccountConfig::default());
        primer.record(key.clone(), Outcome::Sent, 100);
        assert_eq!(primer.next_check.get(&key), Some(&(100 + WINDOW)));
        primer.record(key.clone(), Outcome::Unknown, 200);
        assert_eq!(primer.next_check.get(&key), Some(&(100 + WINDOW)));
    }
}

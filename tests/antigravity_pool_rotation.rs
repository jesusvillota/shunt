//! End-to-end account rotation and independent Antigravity quota families.

use std::{net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};

use serde_json::{json, Value};
use shunt::{
    accounts::{AccountPool, QuotaBucketSnapshot},
    config::{AccountConfig, Config},
    server,
};
use wiremock::{
    matchers::{body_partial_json, header, method, path},
    Mock, MockServer, ResponseTemplate,
};

mod common;

struct Pool {
    accounts: Arc<AccountPool>,
    resolved_accounts: Vec<AccountConfig>,
    addr: SocketAddr,
    gateway: tokio::task::JoinHandle<()>,
    dir: PathBuf,
    _vars: common::EnvVars,
}

impl Pool {
    async fn start(backend: &MockServer, names: &[&str]) -> Self {
        let mut vars = common::env_lock().await;
        let dir = std::env::temp_dir().join(format!(
            "shunt-agy-rotation-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let accounts_dir = dir.join("accounts");
        std::fs::create_dir_all(&accounts_dir).unwrap();
        for name in names {
            std::fs::write(
                accounts_dir.join(format!("{name}.json")),
                json!({
                    "access_token": format!("token-{name}"),
                    "refresh_token": format!("refresh-{name}"),
                    "expiry_date": 4_000_000_000_000_u64,
                    "project_id": format!("project-{name}"),
                })
                .to_string(),
            )
            .unwrap();
        }
        vars.set("SHUNT_ANTIGRAVITY_ACCOUNTS_DIR", &accounts_dir);
        vars.set("SHUNT_ANTIGRAVITY_AUTH_FILE", dir.join("no-singleton.json"));
        let config_path = dir.join("shunt.toml");
        std::fs::write(
            &config_path,
            format!(
                "[server]\nbind = \"127.0.0.1:0\"\ndefault_provider = \"antigravity\"\n\
                 [providers.antigravity]\nauth = \"antigravity_oauth\"\nbase_url = \"{}\"\n",
                backend.uri()
            ),
        )
        .unwrap();
        let config = Config::load(Some(&config_path)).unwrap();
        let resolved_accounts = config.providers["antigravity"]
            .resolve_pool_accounts()
            .await
            .unwrap();
        let (app, _, state) = server::build_router(config).unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let gateway = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self {
            accounts: state.accounts,
            resolved_accounts,
            addr,
            gateway,
            dir,
            _vars: vars,
        }
    }

    async fn request(&self, model: &str) -> (u16, Option<String>) {
        let response = self.response(model).await;
        let status = response.status().as_u16();
        let account = response
            .headers()
            .get("x-shunt-account")
            .map(|value| value.to_str().unwrap().to_string());
        let body: Value = response.json().await.unwrap();
        if status == 200 {
            assert_eq!(body["content"][0]["text"], "OK");
        }
        (status, account)
    }

    async fn response(&self, model: &str) -> reqwest::Response {
        reqwest::Client::new()
            .post(format!("http://{}/v1/messages", self.addr))
            .json(&json!({
                "model": model,
                "max_tokens": 16,
                "messages": [{"role": "user", "content": "Reply with OK."}]
            }))
            .send()
            .await
            .unwrap()
    }
}

impl Drop for Pool {
    fn drop(&mut self) {
        self.gateway.abort();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

async fn healthy_backend() -> MockServer {
    let backend = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1internal:fetchAvailableModels"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"models": {}})))
        .mount(&backend)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1internal:generateContent"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "response": {"candidates": [{
                "content": {"parts": [{"text": "OK"}]}, "finishReason": "STOP"
            }]}
        })))
        .with_priority(10)
        .mount(&backend)
        .await;
    backend
}

async fn inference(backend: &MockServer) -> Vec<wiremock::Request> {
    backend
        .received_requests()
        .await
        .unwrap()
        .into_iter()
        .filter(|request| request.url.path() == "/v1internal:generateContent")
        .collect()
}

#[tokio::test]
async fn family_quota_selects_the_highest_account_and_fails_over_to_lower() {
    let backend = healthy_backend().await;
    // The highest Gemini account refuses once. The next account must have
    // the next highest Gemini fraction, rather than the best Claude fraction.
    Mock::given(method("POST"))
        .and(path("/v1internal:generateContent"))
        .and(header("authorization", "Bearer token-b"))
        .and(body_partial_json(
            json!({"model": "gemini-3.8-flash-medium"}),
        ))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("Retry-After", "60")
                .set_body_json(json!({"error": {"message": "Quota exhausted."}})),
        )
        .expect(1)
        .mount(&backend)
        .await;
    let pool = Pool::start(&backend, &["a", "b", "c"]).await;
    for (name, gemini, other) in [("a", 0.1, 0.9), ("b", 0.9, 0.1), ("c", 0.5, 0.5)] {
        pool.accounts.note_antigravity_usage(
            "antigravity",
            pool.resolved_accounts
                .iter()
                .find(|account| account.name == name)
                .unwrap(),
            vec![
                QuotaBucketSnapshot {
                    label: "Gemini Models · weekly".into(),
                    remaining: Some(gemini),
                    reset_time: None,
                },
                QuotaBucketSnapshot {
                    label: "Claude + GPT Models · weekly".into(),
                    remaining: Some(other),
                    reset_time: None,
                },
            ],
        );
    }
    assert_eq!(
        pool.request("gemini-3.8-flash").await,
        (200, Some("c".into()))
    );
    assert_eq!(
        pool.request("claude-sonnet-4-6").await,
        (200, Some("a".into()))
    );
    assert_eq!(
        pool.request("gpt-oss-120b-medium").await,
        (200, Some("a".into()))
    );
    assert_eq!(
        pool.request("gemini-3.1-pro").await,
        (200, Some("c".into()))
    );
    let requests = inference(&backend).await;
    assert_eq!(requests.len(), 5);
    for (request, expected) in requests.iter().zip(["b", "c", "a", "a", "c"]) {
        assert_eq!(
            request.headers["authorization"],
            format!("Bearer token-{expected}")
        );
        let body: Value = serde_json::from_slice(&request.body).unwrap();
        assert_eq!(body["project"], format!("project-{expected}"));
    }
}

#[tokio::test]
async fn nine_requests_rotate_across_all_three_accounts() {
    let backend = healthy_backend().await;
    let pool = Pool::start(&backend, &["a", "b", "c"]).await;
    for expected in ["a", "b", "c"].into_iter().cycle().take(9) {
        assert_eq!(
            pool.request("gemini-3.8-flash").await,
            (200, Some(expected.into()))
        );
    }
    let requests = inference(&backend).await;
    assert_eq!(requests.len(), 9);
    for (request, expected) in requests.iter().zip(["a", "b", "c"].into_iter().cycle()) {
        assert_eq!(
            request.headers["authorization"],
            format!("Bearer token-{expected}")
        );
        let body: Value = serde_json::from_slice(&request.body).unwrap();
        assert_eq!(body["project"], format!("project-{expected}"));
    }
}

#[tokio::test]
async fn exhausted_account_fails_over_is_skipped_and_returns_after_reset() {
    let backend = healthy_backend().await;
    Mock::given(method("POST"))
        .and(path("/v1internal:generateContent"))
        .and(header("authorization", "Bearer token-a"))
        .respond_with(ResponseTemplate::new(429).set_body_json(json!({
            "error": {"message": "Individual quota reached. Resets in 2s.",
                      "status": "RESOURCE_EXHAUSTED"}
        })))
        .up_to_n_times(1)
        .expect(1)
        .mount(&backend)
        .await;
    let pool = Pool::start(&backend, &["a", "b"]).await;
    assert_eq!(
        pool.request("gemini-3.8-flash").await,
        (200, Some("b".into()))
    );
    assert_eq!(
        pool.request("gemini-3.1-pro").await,
        (200, Some("b".into()))
    );
    assert_eq!(
        pool.request("gemini-3.8-flash").await,
        (200, Some("b".into()))
    );
    let requests = inference(&backend).await;
    assert_eq!(
        requests.len(),
        4,
        "the family-wide refusal must prevent a repeated attempt"
    );
    assert_eq!(requests[0].headers["authorization"], "Bearer token-a");
    assert!(requests[1..]
        .iter()
        .all(|request| request.headers["authorization"] == "Bearer token-b"));
    tokio::time::sleep(Duration::from_millis(2200)).await;
    // The fourth decision starts at b. The fifth starts at the recovered a.
    assert_eq!(
        pool.request("gemini-3.8-flash").await,
        (200, Some("b".into()))
    );
    assert_eq!(
        pool.request("gemini-3.8-flash").await,
        (200, Some("a".into()))
    );
    assert_eq!(inference(&backend).await.len(), 6);
}

#[tokio::test]
async fn gemini_exhaustion_leaves_claude_and_gpt_available_on_the_same_account() {
    let backend = healthy_backend().await;
    Mock::given(method("POST"))
        .and(path("/v1internal:generateContent"))
        .and(body_partial_json(
            json!({"model": "gemini-3.8-flash-medium"}),
        ))
        .respond_with(ResponseTemplate::new(429).set_body_json(json!({
            "error": {"message": "Individual quota reached. Resets in 161h31m52s.",
                      "status": "RESOURCE_EXHAUSTED"}
        })))
        .expect(1)
        .mount(&backend)
        .await;
    let pool = Pool::start(&backend, &["a"]).await;
    assert_eq!(pool.request("gemini-3.8-flash").await, (429, None));
    for model in ["claude-sonnet-4-6", "gpt-oss-120b-medium"] {
        assert_eq!(pool.request(model).await, (200, Some("a".into())));
    }
    assert_eq!(pool.request("gemini-3.1-pro").await, (429, None));
    let requests = inference(&backend).await;
    assert_eq!(
        requests.len(),
        3,
        "Claude/GPT success must not clear Gemini's cooldown"
    );
    assert!(requests
        .iter()
        .all(|request| request.headers["authorization"] == "Bearer token-a"));
}

#[tokio::test]
async fn claude_exhaustion_also_blocks_gpt_but_leaves_gemini_available() {
    let backend = healthy_backend().await;
    Mock::given(method("POST"))
        .and(path("/v1internal:generateContent"))
        .and(body_partial_json(json!({"model": "claude-sonnet-4-6"})))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("Retry-After", "7200")
                .set_body_json(json!({"error": {"message": "Quota exhausted."}})),
        )
        .expect(1)
        .mount(&backend)
        .await;
    let pool = Pool::start(&backend, &["a"]).await;
    assert_eq!(pool.request("claude-sonnet-4-6").await, (429, None));
    assert_eq!(pool.request("gpt-oss-120b-medium").await, (429, None));
    assert_eq!(
        pool.request("gemini-3.8-flash").await,
        (200, Some("a".into()))
    );
    assert_eq!(pool.request("claude-opus-4-6-thinking").await, (429, None));
    assert_eq!(inference(&backend).await.len(), 2);
}

async fn exhaust_account(backend: &MockServer, name: &str, seconds: u64) {
    Mock::given(method("POST"))
        .and(path("/v1internal:generateContent"))
        .and(header("authorization", format!("Bearer token-{name}")))
        .and(body_partial_json(
            json!({"model": "gemini-3.8-flash-medium"}),
        ))
        .respond_with(ResponseTemplate::new(429).set_body_json(json!({
            "error": {"message": format!("Individual quota reached. Resets in {seconds}s."),
                      "status": "RESOURCE_EXHAUSTED"}
        })))
        .expect(1)
        .mount(backend)
        .await;
}

async fn assert_exhausted(response: reqwest::Response, started: std::time::Instant) {
    assert_eq!(response.status(), 429);
    let seconds: u64 = response.headers()["retry-after"]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    // The earliest reset is 60 seconds from the first refusal, rounded up.
    // Bound elapsed wall time so a slow test machine cannot cause a false failure.
    assert!((60_u64.saturating_sub(started.elapsed().as_secs())..=60).contains(&seconds));
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["type"], "error");
    assert_eq!(body["error"]["type"], "rate_limit_error");
    assert_eq!(
        body["error"]["message"],
        format!("Antigravity quota exhausted. Resets in {seconds}s.")
    );
}

#[tokio::test]
async fn all_cooled_requests_return_earliest_reset_without_upstream_calls() {
    let backend = healthy_backend().await;
    exhaust_account(&backend, "a", 60).await;
    exhaust_account(&backend, "b", 7200).await;
    let pool = Pool::start(&backend, &["a", "b"]).await;
    let started = std::time::Instant::now();
    assert_eq!(pool.request("gemini-3.8-flash").await, (429, None));
    assert_eq!(inference(&backend).await.len(), 2);
    let calls = backend.received_requests().await.unwrap().len();
    for model in ["gemini-3.8-flash", "gemini-3.1-pro", "gemini-3.8-flash"] {
        assert_exhausted(pool.response(model).await, started).await;
    }
    assert_eq!(backend.received_requests().await.unwrap().len(), calls);
    for model in ["claude-sonnet-4-6", "gpt-oss-120b-medium"] {
        assert_eq!(pool.request(model).await.0, 200);
    }
    assert_eq!(inference(&backend).await.len(), 4);
}

#[tokio::test]
async fn all_paused_accounts_report_operator_action_without_upstream_calls() {
    let backend = healthy_backend().await;
    let pool = Pool::start(&backend, &["a", "b"]).await;
    // Even an active quota refusal on a paused account is an operator pause.
    exhaust_account(&backend, "a", 60).await;
    exhaust_account(&backend, "b", 7200).await;
    assert_eq!(pool.request("gemini-3.8-flash").await.0, 429);
    for account in &pool.resolved_accounts {
        pool.accounts.set_paused("antigravity", account, true);
    }
    let calls = backend.received_requests().await.unwrap().len();
    let response = pool.response("gemini-3.8-flash").await;
    assert_eq!(response.status(), 503);
    assert!(!response.headers().contains_key("retry-after"));
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["type"], "error");
    assert_eq!(body["error"]["type"], "api_error");
    assert_eq!(
        body["error"]["message"],
        "all Antigravity accounts are paused or disabled"
    );
    assert_eq!(backend.received_requests().await.unwrap().len(), calls);
}

#[tokio::test]
async fn cooled_and_paused_accounts_use_only_the_unpaused_reset() {
    let backend = healthy_backend().await;
    // The paused account's earlier reset must not determine Retry-After.
    exhaust_account(&backend, "a", 10).await;
    exhaust_account(&backend, "b", 60).await;
    let pool = Pool::start(&backend, &["a", "b"]).await;
    let started = std::time::Instant::now();
    assert_eq!(pool.request("gemini-3.8-flash").await.0, 429);
    pool.accounts
        .set_paused("antigravity", &pool.resolved_accounts[0], true);
    let calls = backend.received_requests().await.unwrap().len();
    assert_exhausted(pool.response("gemini-3.8-flash").await, started).await;
    assert_eq!(backend.received_requests().await.unwrap().len(), calls);
}

async fn assert_no_account_available(response: reqwest::Response) {
    assert_eq!(response.status(), 503);
    assert!(!response.headers().contains_key("retry-after"));
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["error"]["type"], "api_error");
    assert_eq!(
        body["error"]["message"],
        "no Antigravity accounts are available"
    );
}

#[tokio::test]
async fn cooled_account_with_a_token_failure_on_another_is_not_a_429() {
    let backend = healthy_backend().await;
    exhaust_account(&backend, "a", 60).await;
    let pool = Pool::start(&backend, &["a", "b"]).await;
    // The first request tries a (429 -> family cooldown), then b serves it.
    assert_eq!(pool.request("gemini-3.8-flash").await.0, 200);
    // b's token cannot be resolved inside the loop: expired, nothing to refresh.
    std::fs::write(
        pool.dir.join("accounts/b.json"),
        json!({"access_token": "token-b", "expiry_date": 1, "project_id": "project-b"}).to_string(),
    )
    .unwrap();
    let calls = backend.received_requests().await.unwrap().len();
    assert_no_account_available(pool.response("gemini-3.8-flash").await).await;
    assert_eq!(backend.received_requests().await.unwrap().len(), calls);
}

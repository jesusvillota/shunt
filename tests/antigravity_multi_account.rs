//! Antigravity account-pool failover: a 429 on one named account rotates to
//! the next, which serves the turn with its own token and project id. A
//! second test pins the backward-compat singleton path: with no pool
//! configured and no store on disk, the single credential still serves.

use std::{io::ErrorKind, net::SocketAddr, time::Duration};

use serde_json::{json, Value};
use shunt::{config::Config, server};
use wiremock::{
    matchers::{header, method, path},
    Mock, MockServer, ResponseTemplate,
};

mod common;

fn can_bind_loopback() -> bool {
    match std::net::TcpListener::bind("127.0.0.1:0") {
        Ok(listener) => {
            drop(listener);
            true
        }
        Err(error) if error.kind() == ErrorKind::PermissionDenied => {
            eprintln!("skipping network integration test: loopback bind is not permitted");
            false
        }
        Err(error) => panic!("unexpected loopback bind failure: {error}"),
    }
}

fn fresh_dir(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "shunt-antigravity-pool-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

fn future_expiry_ms() -> u64 {
    (std::time::SystemTime::now() + Duration::from_secs(3600))
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn write_account(dir: &std::path::Path, name: &str, token: &str, project: &str) {
    std::fs::write(
        dir.join(format!("{name}.json")),
        serde_json::to_vec(&json!({
            "access_token": token,
            "refresh_token": format!("refresh-{name}"),
            "expiry_date": future_expiry_ms(),
            "project_id": project,
        }))
        .unwrap(),
    )
    .unwrap();
}

async fn mount_backend(backend: &MockServer, exhausted_token: &str) {
    // The matchers are disjoint, so mount order carries no precedence meaning.
    Mock::given(method("POST"))
        .and(path("/v1internal:generateContent"))
        .and(header("authorization", format!("Bearer {exhausted_token}")))
        .respond_with(ResponseTemplate::new(429).set_body_json(json!({
            "error": {"message": "Quota exhausted.", "status": "RESOURCE_EXHAUSTED"}
        })))
        .mount(backend)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1internal:generateContent"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "response": {
                "candidates": [{
                    "content": {"parts": [{"text": "OK"}]},
                    "finishReason": "STOP"
                }]
            }
        })))
        .mount(backend)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1internal:fetchAvailableModels"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"models": {}})))
        .mount(backend)
        .await;
}

async fn serve(config: Config) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind(config.server.bind_addr().unwrap())
        .await
        .unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    let (app, _, _) = server::build_router(config).unwrap();
    let gateway = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (addr, gateway)
}

fn config_with_base(dir: &std::path::Path, base_url: &str) -> Config {
    let config_path = dir.join("shunt.toml");
    std::fs::write(
        &config_path,
        format!(
            "[server]\ndefault_provider = \"antigravity\"\n\n\
             [providers.antigravity]\nauth = \"antigravity_oauth\"\nbase_url = \"{base_url}\"\n",
        ),
    )
    .unwrap();
    let mut config = Config::load(Some(&config_path)).unwrap();
    config.server.bind = "127.0.0.1:0".to_string();
    config
}

async fn post_message(addr: SocketAddr) -> reqwest::Response {
    reqwest::Client::new()
        .post(format!("http://{addr}/v1/messages"))
        .json(&json!({
            "model": "gemini-3.8-flash",
            "max_tokens": 16,
            "messages": [{"role": "user", "content": "Reply with OK."}]
        }))
        .send()
        .await
        .unwrap()
}

#[tokio::test]
async fn quota_exhaustion_on_one_account_fails_over_to_the_next() {
    if !can_bind_loopback() {
        return;
    }

    let backend = MockServer::start().await;
    mount_backend(&backend, "token-a").await;

    let dir = fresh_dir("failover");
    std::fs::create_dir_all(&dir).unwrap();
    let accounts_dir = dir.join("accounts");
    std::fs::create_dir_all(&accounts_dir).unwrap();
    write_account(&accounts_dir, "a", "token-a", "proj-a");
    write_account(&accounts_dir, "b", "token-b", "proj-b");

    let mut vars = common::env_lock().await;
    vars.set("SHUNT_ANTIGRAVITY_ACCOUNTS_DIR", &accounts_dir);
    vars.set("SHUNT_ANTIGRAVITY_AUTH_FILE", dir.join("no-singleton.json"));

    let (addr, gateway) = serve(config_with_base(&dir, &backend.uri())).await;
    let response = post_message(addr).await;
    assert_eq!(response.status(), 200);
    assert_eq!(
        response.headers().get("x-shunt-account").unwrap(),
        "b",
        "the rotation must name the account that served the turn"
    );
    response.bytes().await.unwrap();
    gateway.abort();

    let inference: Vec<_> = backend
        .received_requests()
        .await
        .unwrap()
        .into_iter()
        .filter(|request| request.url.path() == "/v1internal:generateContent")
        .collect();
    assert_eq!(inference.len(), 2, "one attempt per account, no retry loop");
    let first_body: Value = serde_json::from_slice(&inference[0].body).unwrap();
    let second_body: Value = serde_json::from_slice(&inference[1].body).unwrap();
    assert_eq!(first_body["project"], "proj-a");
    assert_eq!(second_body["project"], "proj-b");
    let first_auth = inference[0]
        .headers
        .get("authorization")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let second_auth = inference[1]
        .headers
        .get("authorization")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert_eq!(first_auth, "Bearer token-a");
    assert_eq!(second_auth, "Bearer token-b");

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn singleton_still_serves_when_no_pool_exists() {
    if !can_bind_loopback() {
        return;
    }

    let backend = MockServer::start().await;
    mount_backend(&backend, "token-nobody-holds").await;

    let dir = fresh_dir("singleton");
    std::fs::create_dir_all(&dir).unwrap();
    let credential_path = dir.join("antigravity-auth.json");
    std::fs::write(
        &credential_path,
        serde_json::to_vec(&json!({
            "access_token": "token-solo",
            "refresh_token": "refresh-solo",
            "expiry_date": future_expiry_ms(),
            "project_id": "proj-solo"
        }))
        .unwrap(),
    )
    .unwrap();

    let mut vars = common::env_lock().await;
    vars.set("SHUNT_ANTIGRAVITY_AUTH_FILE", &credential_path);
    vars.set(
        "SHUNT_ANTIGRAVITY_ACCOUNTS_DIR",
        dir.join("no-accounts-dir"),
    );

    let (addr, gateway) = serve(config_with_base(&dir, &backend.uri())).await;
    let response = post_message(addr).await;
    assert_eq!(response.status(), 200, "{:?}", response.text().await);
    gateway.abort();

    let inference: Vec<_> = backend
        .received_requests()
        .await
        .unwrap()
        .into_iter()
        .filter(|request| request.url.path() == "/v1internal:generateContent")
        .collect();
    assert_eq!(inference.len(), 1, "no pool means no second attempt");
    let body: Value = serde_json::from_slice(&inference[0].body).unwrap();
    assert_eq!(body["project"], "proj-solo");

    let _ = std::fs::remove_dir_all(&dir);
}

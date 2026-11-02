//! `[server.auth.clients]` account pinning: a client draws only from its
//! configured subset of a pooled provider's accounts, overlapping clients
//! share accounts, a client without an entry uses every account, and an
//! exhausted subset fails instead of spilling over to other accounts.

use std::{
    collections::{BTreeMap, BTreeSet},
    io::ErrorKind,
    net::SocketAddr,
};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use reqwest::StatusCode;
use shunt::{
    config::{
        AccountConfig, AuthMode, ClientPolicy, CodexEndpointConfig, Config, InboundAuthConfig,
        RouteConfig,
    },
    server,
};
use tokio::task::JoinHandle;
use wiremock::{
    matchers::{header, method, path},
    Mock, MockServer, ResponseTemplate,
};

mod common;

const TOKENS_ENV: &str = "SHUNT_TEST_PIN_CLIENT_TOKENS";
const ACCOUNTS: [&str; 3] = ["a", "b", "c"];

struct TestGateway {
    base_url: String,
    task: JoinHandle<()>,
}

impl Drop for TestGateway {
    fn drop(&mut self) {
        self.task.abort();
    }
}

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

/// The upstream OAuth token of pool account `name`.
fn account_token(name: &str) -> String {
    ["fake-oauth-pin-", name].concat()
}

/// Set the account tokens and the inbound client tokens. Client `goal-w` has
/// no `[server.auth.clients]` entry.
fn set_env(vars: &mut common::EnvVars) {
    for name in ACCOUNTS {
        vars.set(&format!("SHUNT_TEST_PIN_TOKEN_{name}"), account_token(name));
    }
    vars.set(
        TOKENS_ENV,
        "goal-x:tok-x,goal-y:tok-y,goal-z:tok-z,goal-w:tok-w",
    );
}

/// A Claude pool of accounts a, b and c, with goal-x pinned to {a, b},
/// goal-y to {c} and goal-z to {a, c}.
fn pinned_config(upstream: &str) -> Config {
    let mut config = Config::default();
    let provider = config.providers.get_mut("anthropic").unwrap();
    provider.base_url = upstream.to_string();
    provider.auth = AuthMode::ClaudeOauth;
    provider.accounts = ACCOUNTS
        .map(|name| AccountConfig {
            name: name.to_string(),
            token_env: Some(format!("SHUNT_TEST_PIN_TOKEN_{name}")),
            uuid: Some(format!("uuid-{name}")),
            ..Default::default()
        })
        .to_vec();
    config.routes.push(RouteConfig {
        model: "pooled-model".to_string(),
        provider: "anthropic".to_string(),
        upstream_model: None,
        effort: None,
        service_tier: None,
    });
    let clients = [
        ("goal-x", &["a", "b"][..]),
        ("goal-y", &["c"]),
        ("goal-z", &["a", "c"]),
    ]
    .map(|(client, names)| {
        let names = names.iter().map(|name| name.to_string()).collect();
        let policy = ClientPolicy {
            accounts: BTreeMap::from([("anthropic".to_string(), names)]),
        };
        (client.to_string(), policy)
    });
    config.server.auth = Some(InboundAuthConfig {
        jwt: Vec::new(),
        header: "x-shunt-token".to_string(),
        tokens_env: TOKENS_ENV.to_string(),
        clients: clients.into(),
    });
    config
}

async fn start_gateway(mut config: Config) -> TestGateway {
    config.server.bind = "127.0.0.1:0".to_string();
    let config = config.validate().expect("pinned config validates");
    let listener = tokio::net::TcpListener::bind(config.server.bind_addr().unwrap())
        .await
        .unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    let (app, _shared, _state) = server::build_router(config).unwrap();
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    TestGateway {
        base_url: format!("http://{addr}"),
        task,
    }
}

/// One session-less request authenticated as client token `token`.
async fn post_as(gateway: &TestGateway, token: &str) -> reqwest::Response {
    post_with(gateway, token, &[]).await
}

/// [`post_as`] with extra request headers.
async fn post_with(
    gateway: &TestGateway,
    token: &str,
    extra: &[(&str, &str)],
) -> reqwest::Response {
    let mut request = reqwest::Client::new()
        .post(format!("{}/v1/messages", gateway.base_url))
        .header("content-type", "application/json")
        .header("x-shunt-token", token);
    for (name, value) in extra {
        request = request.header(*name, *value);
    }
    request
        .body(r#"{"model":"pooled-model","max_tokens":16,"messages":[{"role":"user","content":"hi"}]}"#)
        .send()
        .await
        .unwrap()
}

/// The accounts whose token the upstream received, from request `from` on.
async fn upstream_accounts(upstream: &MockServer, from: usize) -> Vec<String> {
    upstream.received_requests().await.unwrap()[from..]
        .iter()
        .map(|request| {
            let bearer = request.headers["authorization"].to_str().unwrap();
            ACCOUNTS
                .into_iter()
                .find(|name| bearer == format!("Bearer {}", account_token(name)))
                .unwrap_or_else(|| panic!("unexpected upstream credential {bearer}"))
                .to_string()
        })
        .collect()
}

fn set_of(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|name| name.to_string()).collect()
}

#[tokio::test]
async fn codex_pinning_uses_the_selected_credential_on_both_paths() {
    if !can_bind_loopback() {
        return;
    }
    let mut vars = common::env_lock().await;
    vars.set(TOKENS_ENV, "goal-x:tok-x,goal-y:tok-y");
    let mut config = Config::default();
    let upstream = MockServer::start().await;
    let provider = config.providers.get_mut("codex").unwrap();
    assert_eq!(provider.auth, AuthMode::ChatgptOauth);
    provider.base_url = upstream.uri();
    let tokens: BTreeMap<_, _> = ["a", "b"]
        .map(|name| {
            let payload = serde_json::json!({
                "exp": 4_102_444_800_u64,
                "https://api.openai.com/auth": {"chatgpt_account_id": format!("acct-{name}")}
            });
            let token = format!("x.{}.y", URL_SAFE_NO_PAD.encode(payload.to_string()));
            let token_env = format!("SHUNT_TEST_PIN_CODEX_{name}");
            vars.set(&token_env, &token);
            provider.accounts.push(AccountConfig {
                name: name.to_string(),
                token_env: Some(token_env),
                ..Default::default()
            });
            (name, token)
        })
        .into();
    config.routes.push(RouteConfig {
        model: "pooled-model".to_string(),
        provider: "codex".to_string(),
        upstream_model: None,
        effort: None,
        service_tier: None,
    });
    config.server.codex_endpoint = Some(CodexEndpointConfig::default());
    config.server.auth = Some(InboundAuthConfig {
        jwt: Vec::new(),
        header: "x-shunt-token".to_string(),
        tokens_env: TOKENS_ENV.to_string(),
        clients: [("goal-x", "b"), ("goal-y", "a")]
            .map(|(client, account)| {
                (
                    client.to_string(),
                    ClientPolicy {
                        accounts: BTreeMap::from([(
                            "codex".to_string(),
                            vec![account.to_string()],
                        )]),
                    },
                )
            })
            .into(),
    });
    let sse = "event: response.created\ndata: {\"response\":{\"id\":\"resp_pin\"}}\n\n\
               event: response.output_item.added\ndata: {\"item\":{\"type\":\"message\"}}\n\n\
               event: response.output_text.delta\ndata: {\"delta\":\"pinned codex served\"}\n\n\
               event: response.output_text.done\ndata: {}\n\n\
               event: response.completed\ndata: {\"response\":{\"usage\":{\"input_tokens\":5,\"output_tokens\":4}}}\n\n";
    Mock::given(method("POST"))
        .and(path("/codex/responses"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(sse, "text/event-stream"))
        .expect(24)
        .mount(&upstream)
        .await;
    let gateway = start_gateway(config).await;

    for endpoint in ["/v1/messages", "/responses"] {
        for (client, token, account) in [("goal-x", "tok-x", "b"), ("goal-y", "tok-y", "a")] {
            let from = upstream.received_requests().await.unwrap().len();
            for _ in 0..6 {
                let response = if endpoint == "/v1/messages" {
                    post_as(&gateway, token).await
                } else {
                    reqwest::Client::new()
                        .post(format!("{}{endpoint}", gateway.base_url))
                        .header("x-shunt-token", token)
                        .json(&serde_json::json!({
                            "model": "pooled-model", "input": "hi", "stream": false, "store": false
                        }))
                        .send()
                        .await
                        .unwrap()
                };
                assert_eq!(response.status(), StatusCode::OK, "{endpoint} {client}");
                assert_eq!(response.headers()["x-shunt-account"], account);
                if endpoint == "/v1/messages" {
                    let body: serde_json::Value = response.json().await.unwrap();
                    assert_eq!(body["content"][0]["text"], "pinned codex served");
                } else {
                    assert_eq!(response.text().await.unwrap(), sse);
                }
            }
            let requests = upstream.received_requests().await.unwrap();
            assert_eq!(requests.len() - from, 6, "{endpoint} {client}");
            for request in &requests[from..] {
                assert_eq!(
                    request.headers["authorization"],
                    format!("Bearer {}", tokens[account])
                );
                assert_eq!(
                    request.headers["chatgpt-account-id"],
                    format!("acct-{account}")
                );
            }
            println!(
                "{endpoint} {client}: all 6 upstream requests used account {account}'s credential"
            );
        }
    }
    upstream.verify().await;
}

#[tokio::test]
async fn each_client_draws_only_from_its_pinned_accounts() {
    if !can_bind_loopback() {
        return;
    }
    let mut vars = common::env_lock().await;
    set_env(&mut vars);
    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"ok":true}"#))
        .mount(&upstream)
        .await;
    let gateway = start_gateway(pinned_config(&upstream.uri())).await;

    for (client, token, expected) in [
        ("goal-x", "tok-x", &["a", "b"][..]),
        ("goal-y", "tok-y", &["c"]),
        ("goal-z", "tok-z", &["a", "c"]),
        ("goal-w", "tok-w", &["a", "b", "c"]),
    ] {
        let from = upstream.received_requests().await.unwrap().len();
        let mut served = BTreeSet::new();
        for _ in 0..6 {
            let response = post_as(&gateway, token).await;
            assert_eq!(response.status(), StatusCode::OK, "{client}");
            let account = response.headers()["x-shunt-account"].to_str().unwrap();
            served.insert(account.to_string());
        }
        let received: BTreeSet<_> = upstream_accounts(&upstream, from)
            .await
            .into_iter()
            .collect();
        println!("{client}: upstream received tokens of {received:?}, x-shunt-account {served:?}");
        assert_eq!(received, set_of(expected), "{client} upstream tokens");
        assert_eq!(served, set_of(expected), "{client} x-shunt-account");
    }
}

/// The subset follows the verified token, not a header the caller sends:
/// goal-x forging unpinned goal-w's attribution and scope still reaches only
/// a and b, and the scope header never reaches the upstream.
#[tokio::test]
async fn forged_client_headers_do_not_widen_the_subset() {
    if !can_bind_loopback() {
        return;
    }
    let mut vars = common::env_lock().await;
    set_env(&mut vars);
    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"ok":true}"#))
        .mount(&upstream)
        .await;
    let gateway = start_gateway(pinned_config(&upstream.uri())).await;

    let forged = [
        ("x-shunt-inbound-client", "goal-w"),
        ("x-shunt-account-scope", "goal-w"),
    ];
    for _ in 0..6 {
        let response = post_with(&gateway, "tok-x", &forged).await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    let received: BTreeSet<_> = upstream_accounts(&upstream, 0).await.into_iter().collect();
    println!("goal-x with forged goal-w headers: upstream received tokens of {received:?}");
    assert_eq!(received, set_of(&["a", "b"]), "goal-x upstream tokens");
    for request in upstream.received_requests().await.unwrap() {
        assert_eq!(
            request.headers["x-shunt-inbound-client"], "goal-x",
            "attribution names the verified client"
        );
        assert!(
            !request.headers.contains_key("x-shunt-account-scope"),
            "the account scope never reaches the upstream"
        );
    }
}

#[tokio::test]
async fn an_exhausted_subset_relays_the_429_instead_of_spilling_over() {
    if !can_bind_loopback() {
        return;
    }
    let mut vars = common::env_lock().await;
    set_env(&mut vars);
    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(header(
            "authorization",
            format!("Bearer {}", account_token("c")).as_str(),
        ))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("retry-after", "30")
                // The window header makes it a quota 429, which rotates;
                // the aggregate status alone pauses and retries the account.
                .insert_header("anthropic-ratelimit-unified-status", "rejected")
                .insert_header("anthropic-ratelimit-unified-5h-status", "rejected")
                .set_body_string(r#"{"type":"error","error":{"type":"rate_limit_error","message":"c exhausted"}}"#),
        )
        .mount(&upstream)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"ok":true}"#))
        .mount(&upstream)
        .await;
    let gateway = start_gateway(pinned_config(&upstream.uri())).await;

    let response = post_as(&gateway, "tok-y").await;
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    let retry_after = response
        .headers()
        .get("retry-after")
        .map(|value| value.to_owned());
    let goal_y = upstream_accounts(&upstream, 0).await;
    println!(
        "goal-y: HTTP {}, retry-after {retry_after:?}, upstream received tokens of {goal_y:?}",
        response.status()
    );
    assert!(retry_after.is_some(), "the relayed 429 carries retry-after");
    assert!(!goal_y.is_empty(), "c was tried");
    assert!(
        goal_y.iter().all(|account| account == "c"),
        "goal-y never reaches a or b: {goal_y:?}"
    );

    let from = goal_y.len();
    let response = post_as(&gateway, "tok-w").await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "an unpinned client still succeeds"
    );
    println!(
        "goal-w afterwards: HTTP {}, upstream received tokens of {:?}",
        response.status(),
        upstream_accounts(&upstream, from).await
    );
}

#[tokio::test]
async fn a_pinned_client_with_no_selectable_account_gets_a_503() {
    if !can_bind_loopback() {
        return;
    }
    let mut vars = common::env_lock().await;
    set_env(&mut vars);
    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"ok":true}"#))
        .expect(0)
        .mount(&upstream)
        .await;
    let mut config = pinned_config(&upstream.uri());
    config.providers.get_mut("anthropic").unwrap().accounts[2].disabled = true;
    let gateway = start_gateway(config).await;

    let response = post_as(&gateway, "tok-y").await;

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body: serde_json::Value = response.json().await.unwrap();
    let message = body["error"]["message"].as_str().unwrap();
    for expected in ["'goal-y'", "[c]", "'anthropic'"] {
        assert!(message.contains(expected), "{expected} in {message}");
    }
    upstream.verify().await;
}

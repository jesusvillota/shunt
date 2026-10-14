//! Exercise the default-build CLI, including its stdout and exit status.
use shunt::pool::{model::PoolResponse, render};
use wiremock::{
    matchers::{header, method, path},
    Mock, MockServer, ResponseTemplate,
};

mod common;

fn snapshot() -> &'static str {
    r#"{"future_field": {"precise": 0.1234567890123456789}, "providers": [
      {"provider":"anthropic","auth":"claude_oauth","accounts":[{"name":"work","has_state":true,"utilization_5h":0.5,"reset_5h":4600,"utilization_7d":0.25,"reset_7d":87400}]},
      {"provider":"codex","auth":"chatgpt_oauth","accounts":[{"name":"code","near_quota":true,"utilization_5h":0.95,"reset_5h":2800,"utilization_7d":0.4,"reset_7d":173800,"cooldown_fable_secs_remaining":60}]},
      {"provider":"antigravity","auth":"antigravity_oauth","accounts":[{"name":"google","quota_buckets":[{"label":"Gemini Models · weekly","remaining":0.75,"reset_time":"1970-01-02T00:16:40Z"},{"label":"Claude + GPT Models · weekly","remaining":0.0,"reset_time":"1970-01-03T00:16:40Z"}]}]}
    ]}"#
}

#[test]
fn text_has_anthropic_codex_and_weekly_only_antigravity() {
    let pool: PoolResponse = serde_json::from_str(snapshot()).unwrap();
    assert_eq!(render(&pool, 1000), concat!(
        "anthropic:\n  work | ok | 5h: 50% used · resets in 1h | 7d: 25% used · resets in 1d\n\n",
        "codex:\n  code | near-quota | 5h: 95% used · resets in 30m | 7d: 40% used · resets in 2d | fable cooldown: 1m\n\n",
        "antigravity:\n  google | ok | Gemini Models · weekly: 75% left · resets in 1d | Claude + GPT Models · weekly: 0% left · resets in 2d\n"
    ));
}

#[test]
fn availability_and_missing_readings() {
    for (field, state) in [
        ("disabled", "disabled"),
        ("paused", "paused"),
        ("needs_relogin", "needs relogin"),
        ("near_quota", "near-quota"),
    ] {
        let pool = serde_json::from_value(serde_json::json!({"providers":[{"provider":"anthropic","accounts":[{"name":"a", field: true}]}]})).unwrap();
        assert_eq!(
            render(&pool, 1000),
            format!("anthropic:\n  a | {state} | 5h: — | 7d: —\n")
        );
    }
    let pool = serde_json::from_value(serde_json::json!({"providers":[{"provider":"antigravity","accounts":[{"name":"a","cooldown_secs_remaining":120,"quota_buckets":[{"label":"weekly","reset_time":"invalid"}]}]}]})).unwrap();
    assert_eq!(
        render(&pool, 1000),
        "antigravity:\n  a | cooling down | weekly: — | cooldown: 2m\n"
    );
}

fn command() -> tokio::process::Command {
    let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_shunt"));
    command
        .env_remove("SHUNT_ADMIN_TOKEN")
        .env_remove("SHUNT_ADMIN_TOKENS")
        .env("HOME", "/no/such/shunt-task007-home")
        .env("RUST_BACKTRACE", "1")
        .arg("pool");
    command
}

fn clean_error(output: std::process::Output, expected: &str) {
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert_eq!(stderr.trim(), expected);
}

#[tokio::test]
async fn json_preserves_snapshot_bytes_and_uses_read_request() {
    let _env = common::set_env(&[]).await;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/admin/api/pool"))
        .and(header("x-shunt-admin-token", "read-token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(snapshot(), "application/json"))
        .expect(1)
        .mount(&server)
        .await;
    let output = command()
        .args(["--url", &server.uri(), "--token", "read-token", "--json"])
        .output()
        .await
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, snapshot().as_bytes());
    assert!(output.stderr.is_empty());
}

#[tokio::test]
async fn configured_bind_and_token_pair_environment_work() {
    let _env = common::set_env(&[]).await;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/admin/api/pool"))
        .and(header("x-custom-admin", "read-token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(snapshot(), "application/json"))
        .expect(1)
        .mount(&server)
        .await;
    let file = std::env::temp_dir().join(format!("shunt-pool-cli-{}.toml", std::process::id()));
    std::fs::write(&file, format!("[server]\nbind = '{}'\n", server.address())).unwrap();
    let output = command()
        .env("SHUNT_ADMIN_TOKENS", "reader:read-token,other:ignored")
        .arg("--config")
        .arg(&file)
        .args(["--header", "x-custom-admin"])
        .output()
        .await
        .unwrap();
    std::fs::remove_file(file).unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("work | ok | 5h: 50% used"));
    assert!(text.contains("Gemini Models · weekly: 75% left"));
}

#[tokio::test]
async fn missing_token_is_clean_nonzero() {
    let _env = common::set_env(&[]).await;
    let output = command()
        .args(["--url", "http://127.0.0.1:1"])
        .output()
        .await
        .unwrap();
    clean_error(output, "Error: no admin token found. Pass --token, set SHUNT_ADMIN_TOKEN, or run `shunt dashboard setup` (writes ~/.shunt/admin-token)");
}

#[tokio::test]
async fn unreachable_gateway_is_clean_nonzero() {
    let _env = common::set_env(&[]).await;
    let socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", socket.local_addr().unwrap());
    drop(socket);
    let output = command()
        .args(["--url", &url, "--token", "test-token"])
        .output()
        .await
        .unwrap();
    clean_error(output, "Error: gateway unreachable");
}

#[tokio::test]
async fn refused_tokens_are_clean_nonzero() {
    let _env = common::set_env(&[]).await;
    for status in [401, 403] {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/admin/api/pool"))
            .respond_with(ResponseTemplate::new(status))
            .expect(1)
            .mount(&server)
            .await;
        let output = command()
            .args(["--url", &server.uri(), "--token", "test-token"])
            .output()
            .await
            .unwrap();
        clean_error(output, &format!("Error: admin token rejected ({status})"));
    }
}

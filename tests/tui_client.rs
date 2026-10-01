//! The `shunt top` admin-API client against a real in-process gateway: poll,
//! pause/resume by `account_ref`, account provisioning start/complete, and the
//! read-tier refusal the monitor surfaces to the operator.

#![cfg(feature = "tui")]

use shunt::{
    config::{AccountConfig, AdminConfig, AdminKey, AuthMode, Config, PoolConfig},
    server,
    tui::{client::Client, model::AccountState},
};

const WRITE_KEY: &str = "tui-write-0123456789abcdef0123456789";
const READ_KEY: &str = "tui-read-0123456789abcdef01234567890";

fn config() -> Config {
    let mut config = Config::default();
    let anthropic = config.providers.get_mut("anthropic").unwrap();
    anthropic.auth = AuthMode::ClaudeOauth;
    anthropic.accounts = vec![AccountConfig {
        name: "monitored".to_string(),
        // Never exists, so the test cannot fall back to a real credential store.
        credentials: Some(
            std::env::temp_dir()
                .join(format!(
                    "shunt-tui-test-{}-missing.json",
                    std::process::id()
                ))
                .to_string_lossy()
                .into_owned(),
        ),
        uuid: Some("monitored-uuid".to_string()),
        ..Default::default()
    }];
    config.server.admin = Some(AdminConfig {
        header: "x-shunt-admin-token".to_string(),
        tokens_env: "SHUNT_TEST_ADMIN_TOKENS_TUI".to_string(),
        tokens_file: None,
        write_keys: vec![AdminKey {
            id: "w".to_string(),
            key: WRITE_KEY.into(),
        }],
        read_keys: vec![AdminKey {
            id: "r".to_string(),
            key: READ_KEY.into(),
        }],
        session_ttl_secs: 3600,
        pending_ttl_secs: 600,
        hide_observed: false,
        oidc: None,
    });
    config.server.pool = Some(PoolConfig::default());
    config.server.bind = "127.0.0.1:0".to_string();
    config
}

#[tokio::test]
async fn polls_pauses_resumes_and_starts_provisioning() {
    let config = config();
    let Ok(listener) = tokio::net::TcpListener::bind(config.server.bind_addr().unwrap()).await
    else {
        eprintln!("skipping: loopback bind is not permitted");
        return;
    };
    let base = format!("http://{}", listener.local_addr().unwrap());
    let (app, _shared, _state) = server::build_router(config).unwrap();
    let serve = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    let writer = Client::new(&base, "x-shunt-admin-token", WRITE_KEY).unwrap();
    let snapshot = writer.fetch_pool().await.unwrap();
    let provider = &snapshot.providers[0];
    assert_eq!(provider.auth.as_deref(), Some("claude_oauth"));
    assert_eq!(provider.account_kind(), Some("claude"));
    assert_eq!(provider.rows.len(), 1);
    let row = &provider.rows[0];
    assert_eq!(row.account.name, "monitored");
    let account_ref = row.account_ref().expect("gateway reports account_ref");
    let provider_name = provider.name.clone();

    writer
        .set_paused(&provider_name, account_ref, true)
        .await
        .unwrap();
    let paused = writer.fetch_pool().await.unwrap();
    assert_eq!(paused.providers[0].rows[0].state, AccountState::Paused);
    assert!(!paused.providers[0].is_on(), "its only account is paused");

    writer
        .set_paused(&provider_name, account_ref, false)
        .await
        .unwrap();
    let resumed = writer.fetch_pool().await.unwrap();
    assert_ne!(resumed.providers[0].rows[0].state, AccountState::Paused);
    assert!(resumed.providers[0].is_on());

    // Provisioning: the start call yields an authorize URL (no upstream call is
    // made); completing without a valid code is refused with the gateway's reason.
    let url = writer.start_account("claude", "pool-b").await.unwrap();
    assert!(url.starts_with("https://"), "{url}");
    let refused = writer
        .complete_account("claude", "pool-b", "not-a-code")
        .await;
    let message = format!("{:#}", refused.unwrap_err());
    assert!(message.contains("authorization code"), "{message}");
    let invalid = writer.start_account("claude", "Bad Name").await;
    assert!(format!("{:#}", invalid.unwrap_err()).contains("[a-z0-9-]+"));

    // A read-tier key can poll but not mutate, and says why.
    let reader = Client::new(&base, "x-shunt-admin-token", READ_KEY).unwrap();
    assert_eq!(
        reader.fetch_pool().await.unwrap().providers[0].rows.len(),
        1
    );
    let refused = reader.set_paused(&provider_name, account_ref, true).await;
    assert!(format!("{:#}", refused.unwrap_err()).contains("read-only"));

    // A wrong token is reported as such rather than as an empty pool.
    let stranger = Client::new(&base, "x-shunt-admin-token", "nope").unwrap();
    assert!(format!("{:#}", stranger.fetch_pool().await.unwrap_err()).contains("rejected"));

    // An unknown account surfaces the 404 hint instead of failing silently.
    let missing = writer.set_paused(&provider_name, "no-such-ref", true).await;
    assert!(format!("{:#}", missing.unwrap_err()).contains("404"));
    serve.abort();
}

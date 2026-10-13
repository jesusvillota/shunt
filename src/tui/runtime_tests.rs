//! Verify that mode effects change the gateway and acknowledge only successful operations.

use super::*;
use wiremock::{
    matchers::{body_json, header, method, path},
    Mock, MockServer, ResponseTemplate,
};

fn context(server: &MockServer, config: Option<PathBuf>) -> (Ctx, mpsc::UnboundedReceiver<Msg>) {
    let (tx, rx) = mpsc::unbounded_channel();
    (
        Ctx {
            client: Client::new(&server.uri(), "x-shunt-admin-token", "test-write-key").unwrap(),
            tx,
            refresh: Arc::new(Notify::new()),
            config,
            edit_lock: Arc::new(Mutex::new(())),
        },
        rx,
    )
}

async fn next(rx: &mut mpsc::UnboundedReceiver<Msg>) -> Msg {
    tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("mode effect completed")
        .expect("effect sent a message")
}

fn temp_config() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "shunt-tui-mode-{}-{}",
        std::process::id(),
        rand::random::<u64>()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("shunt.toml")
}

#[tokio::test]
async fn weekly_reset_effect_acknowledges_success_but_not_api_refusal() {
    for status in [200, 403] {
        let server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/admin/api/pool"))
            .and(header("x-shunt-admin-token", "test-write-key"))
            .and(body_json(
                serde_json::json!({"provider": "claude", "sort_by_weekly_reset": true}),
            ))
            .respond_with(ResponseTemplate::new(status))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/admin/api/pool"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"providers": [{"provider": "claude", "sort_by_weekly_reset": true}]})),
            )
            .expect(if status == 200 { 1 } else { 0 })
            .mount(&server)
            .await;
        let (ctx, mut rx) = context(&server, None);
        perform(
            &ctx,
            Effect::SetWeeklyReset {
                provider: "claude".into(),
                on: true,
            },
        );
        if status == 200 {
            assert!(
                matches!(next(&mut rx).await, Msg::Pool(Ok(snapshot)) if snapshot.providers[0].weekly_reset)
            );
            assert!(matches!(next(&mut rx).await, Msg::WeeklyResetSet(_, true)));
            assert!(
                matches!(next(&mut rx).await, Msg::Notice(Notice::Info(text)) if text.contains("claude: weekly reset on"))
            );
        } else {
            assert!(
                matches!(next(&mut rx).await, Msg::Notice(Notice::Error(text)) if text.contains("read-only"))
            );
        }
        assert!(rx.try_recv().is_err());
        server.verify().await;
    }
}

#[tokio::test]
async fn inactive_weekly_reset_uses_the_gateways_effective_mode() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/admin/api/pool"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/admin/api/pool"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"providers": [{"provider": "claude", "sort_by_weekly_reset": false}]})),
        )
        .expect(1)
        .mount(&server)
        .await;
    let (ctx, mut rx) = context(&server, None);
    perform(
        &ctx,
        Effect::SetWeeklyReset {
            provider: "claude".into(),
            on: true,
        },
    );
    assert!(
        matches!(next(&mut rx).await, Msg::Pool(Ok(snapshot)) if !snapshot.providers[0].weekly_reset)
    );
    assert!(matches!(next(&mut rx).await, Msg::WeeklyResetSet(_, false)));
    assert!(
        matches!(next(&mut rx).await, Msg::Notice(Notice::Info(text)) if text.contains("inactive") && text.contains("[server.pool]"))
    );
    server.verify().await;
}

#[tokio::test]
async fn clearing_ranks_disables_weekly_reset_and_reports_partial_failure() {
    for status in [200, 403] {
        let server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/admin/api/pool"))
            .and(body_json(
                serde_json::json!({"provider": "claude", "sort_by_weekly_reset": false}),
            ))
            .respond_with(ResponseTemplate::new(status))
            .expect(1)
            .mount(&server)
            .await;
        let config = temp_config();
        std::fs::write(
            &config,
            "[providers.claude]\naccounts = [{name = 'a', priority = 1}, {name = 'b', priority = 2}]\n",
        )
        .unwrap();
        let (ctx, mut rx) = context(&server, Some(config.clone()));
        perform(
            &ctx,
            Effect::ClearRanks {
                provider: "claude".into(),
            },
        );
        if status == 200 {
            assert!(matches!(next(&mut rx).await, Msg::RanksCleared(name) if name == "claude"));
            assert!(
                matches!(next(&mut rx).await, Msg::Notice(Notice::Info(text)) if text.contains("weekly reset off"))
            );
        } else {
            assert!(
                matches!(next(&mut rx).await, Msg::Notice(Notice::Error(text)) if text.contains("priorities cleared") && text.contains("read-only"))
            );
        }
        assert!(!std::fs::read_to_string(&config)
            .unwrap()
            .contains("priority"));
        assert!(rx.try_recv().is_err());
        server.verify().await;
        std::fs::remove_dir_all(config.parent().unwrap()).unwrap();
    }
}

#[tokio::test]
async fn failed_config_edit_does_not_change_the_global_pool_setting() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
    let config = temp_config();
    std::fs::write(&config, "invalid TOML [").unwrap();
    let (ctx, mut rx) = context(&server, Some(config.clone()));
    perform(
        &ctx,
        Effect::ClearRanks {
            provider: "claude".into(),
        },
    );
    assert!(
        matches!(next(&mut rx).await, Msg::Notice(Notice::Error(text)) if text.contains("not valid TOML"))
    );
    server.verify().await;
    std::fs::remove_dir_all(config.parent().unwrap()).unwrap();
}

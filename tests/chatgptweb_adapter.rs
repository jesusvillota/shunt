//! Browser-free coverage through the actual Codex HTTP router.
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use shunt::{
    chatgptweb::{
        fake::{FakeDriver, Fixture},
        DriverError, ErrorCode, Submission,
    },
    config::{
        AccountConfig, AuthMode, CodexEndpointConfig, CodexRouteConfig, Config, ProviderKind,
    },
    server,
};
use tower::ServiceExt;
mod common;

const PROFILE: &str = "/profiles/task004";
const PROMPT: &str = r#"Continue this conversation. Return only the assistant's next text answer.
[{"role":"system","content":"Be brief"},{"role":"user","content":"First question"},{"role":"assistant","content":"First answer"},{"role":"user","content":"Second question 🌍"}]"#;
fn config(routed: bool) -> Config {
    let mut config = Config::default();
    let mut provider = config.providers["openai"].clone();
    provider.kind = ProviderKind::ChatgptwebBrowser;
    provider.auth = AuthMode::None;
    provider.api_key_env = None;
    provider.base_url = "http://localhost".into();
    provider.accounts = vec![AccountConfig {
        name: "personal".into(),
        profile_dir: Some(PROFILE.into()),
        ..Default::default()
    }];
    config.providers.insert("chatgptweb".into(), provider);
    config.server.codex_endpoint = Some(CodexEndpointConfig {
        provider: if routed { "codex" } else { "chatgptweb" }.into(),
        routes: vec![CodexRouteConfig {
            model: "web-chat".into(),
            provider: "chatgptweb".into(),
            upstream_model: Some("Chat Model".into()),
        }],
    });
    config
}
fn request() -> Value {
    json!({"model":"web-chat", "instructions":"Be brief", "stream":true, "tools":[], "input":[
        {"type":"message","role":"user","content":[{"type":"input_text","text":"First question"}]},
        {"role":"assistant","content":[{"type":"output_text","text":"First answer"}]},
        {"role":"user","content":[{"type":"input_text","text":"Second question 🌍"}]}]})
}
async fn gateway(failure: Option<DriverError>, routed: bool) -> axum::Router {
    let mut config = config(routed);
    if !routed {
        config
            .server
            .codex_endpoint
            .as_mut()
            .unwrap()
            .routes
            .clear();
    }
    let (router, _, state) = server::build_router(config).unwrap();
    state
        .chatgptweb_drivers
        .insert(
            PROFILE.into(),
            Box::new(FakeDriver::new(
                "personal".into(),
                PROFILE.into(),
                vec![Fixture {
                    prompt: PROMPT.into(),
                    model: "Chat Model".into(),
                    deltas: vec!["Hello ".into(), "🌍".into()],
                    error: failure,
                }],
            )),
        )
        .await;
    router
}
async fn post(router: axum::Router, request: Value, compressed: bool) -> axum::response::Response {
    let bytes = serde_json::to_vec(&request).unwrap();
    let mut builder = Request::post("/v1/responses").header("content-type", "application/json");
    let bytes = if compressed {
        builder = builder.header("content-encoding", "zstd");
        zstd::stream::encode_all(bytes.as_slice(), 1).unwrap()
    } else {
        bytes
    };
    router
        .oneshot(builder.body(Body::from(bytes)).unwrap())
        .await
        .unwrap()
}
async fn events(response: axum::response::Response) -> Vec<Value> {
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "text/event-stream");
    assert_eq!(response.headers()["x-shunt-account"], "personal");
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    std::str::from_utf8(&body)
        .unwrap()
        .split("\n\n")
        .filter(|s| !s.is_empty())
        .map(|frame| {
            let mut lines = frame.lines();
            let name = lines.next().unwrap().strip_prefix("event: ").unwrap();
            let value: Value =
                serde_json::from_str(lines.next().unwrap().strip_prefix("data: ").unwrap())
                    .unwrap();
            assert_eq!(value["type"], name);
            value
        })
        .collect()
}
#[tokio::test]
async fn chatgptweb_codex_endpoint_streams_full_transcript_through_fake() {
    let _vars = common::env_lock().await;
    for compressed in [false, true] {
        let response = post(gateway(None, true).await, request(), compressed).await;
        let events = events(response).await;
        let names: Vec<_> = events.iter().map(|e| e["type"].as_str().unwrap()).collect();
        assert_eq!(
            names,
            [
                "response.created",
                "response.in_progress",
                "response.output_item.added",
                "response.content_part.added",
                "response.output_text.delta",
                "response.output_text.delta",
                "response.output_text.done",
                "response.content_part.done",
                "response.output_item.done",
                "response.completed"
            ]
        );
        for (index, event) in events.iter().enumerate() {
            assert_eq!(event["sequence_number"], index);
        }
        assert_eq!(events[4]["delta"], "Hello ");
        assert_eq!(events[5]["delta"], "🌍");
        let response = &events.last().unwrap()["response"];
        assert_eq!(response["object"], "response");
        assert_eq!(response["status"], "completed");
        assert_eq!(response["model"], "web-chat");
        assert_eq!(response["output"][0]["content"][0]["text"], "Hello 🌍");
        assert_eq!(events[0]["response"]["id"], response["id"]);
        assert_eq!(events[4]["item_id"], response["output"][0]["id"]);
        assert!(response["usage"].is_null());
    }
}
#[tokio::test]
async fn chatgptweb_partial_failure_never_completes_or_replays() {
    let _vars = common::env_lock().await;
    let failure = DriverError {
        code: ErrorCode::UiDrift,
        message: "private diagnostic".into(),
        submission: Submission::Sent,
        retry_after_ms: None,
    };
    let events = events(post(gateway(Some(failure), true).await, request(), false).await).await;
    assert_eq!(events.last().unwrap()["type"], "response.failed");
    assert_eq!(
        events.last().unwrap()["response"]["error"]["code"],
        "ui_drift"
    );
    assert!(!events.iter().any(|e| e["type"] == "response.completed"));
    assert!(!serde_json::to_string(&events)
        .unwrap()
        .contains("private diagnostic"));
}
#[tokio::test]
async fn chatgptweb_rejects_tools_images_and_continuation_ids_in_openai_shape() {
    let _vars = common::env_lock().await;
    for (key, value) in [
        ("tools", json!([{"type":"function","name":"shell"}])),
        (
            "input",
            json!([{"role":"user","content":[{"type":"input_image","image_url":"https://invalid/image"}]}]),
        ),
        ("previous_response_id", json!("resp_old")),
    ] {
        let mut request = request();
        request[key] = value;
        let response = post(gateway(None, true).await, request, false).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert_eq!(body["error"]["type"], "invalid_request_error");
        assert!(body.get("type").is_none());
    }
}
#[tokio::test]
async fn chatgptweb_nonstreaming_and_default_provider_work() {
    let _vars = common::env_lock().await;
    let mut request = request();
    request["stream"] = json!(false);
    request["model"] = json!("Chat Model");
    let response = post(gateway(None, false).await, request, true).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(body["status"], "completed");
    assert_eq!(body["output"][0]["content"][0]["text"], "Hello 🌍");
}
#[tokio::test]
async fn chatgptweb_catalog_lists_configured_models_on_every_codex_alias() {
    let _vars = common::env_lock().await;
    let router = gateway(None, true).await;
    for path in [
        "/v1/models?client_version=0.152.0",
        "/models",
        "/backend-api/codex/models",
    ] {
        let response = router
            .clone()
            .oneshot(Request::get(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let catalog: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        let row = &catalog["models"][0];
        assert_eq!(catalog["models"].as_array().unwrap().len(), 1);
        assert_eq!(row["slug"], "web-chat");
        assert_eq!(row["visibility"], "list");
        assert_eq!(row["shell_type"], "disabled");
        assert_eq!(row["input_modalities"], json!(["text"]));
        assert!(row["display_name"]
            .as_str()
            .unwrap()
            .contains("ChatGPT Web"));
    }
}
#[tokio::test]
async fn chatgptweb_config_requires_one_profile_without_credentials() {
    let _vars = common::env_lock().await;
    for case in 0..5 {
        let mut config = config(true);
        let provider = config.providers.get_mut("chatgptweb").unwrap();
        match case {
            0 => provider.accounts.clear(),
            1 => provider.accounts.push(provider.accounts[0].clone()),
            2 => provider.accounts[0].profile_dir = Some("relative".into()),
            3 => provider.accounts[0].token_env = Some("SECRET".into()),
            _ => provider.auth = AuthMode::ChatgptOauth,
        }
        assert!(server::build_router(config).is_err());
    }
}

#[tokio::test]
async fn chatgptweb_process_client_streams_and_rejects_premature_eof() {
    let mut vars = common::env_lock().await;
    let dir = std::env::temp_dir().join(format!("shunt-task004-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    let helper = dir.join("helper.mjs");
    vars.set("SHUNT_CHATGPTWEB_HELPER", &helper);
    for complete in [true, false] {
        std::fs::write(&helper, format!(r#"
import {{ createInterface }} from 'node:readline';
const input = createInterface({{input:process.stdin}});
for await (const line of input) {{
  const r = JSON.parse(line);
  const args = process.argv.slice(2);
  if (args[0] !== '--account' || args[1] !== r.account || args[2] !== '--profile-dir' || args[3] !== r.profile_dir) process.exit(2);
  console.log(JSON.stringify({{version:1,id:r.id,type:'text_delta',text:'pipe answer'}}));
  if ({complete}) console.log(JSON.stringify({{version:1,id:r.id,type:'done'}}));
  process.exit(0);
}}
"#)).unwrap();
        let (router, _, _) = server::build_router(config(true)).unwrap();
        let events = events(post(router, request(), false).await).await;
        assert_eq!(events[4]["delta"], "pipe answer");
        assert_eq!(
            events.last().unwrap()["type"],
            if complete {
                "response.completed"
            } else {
                "response.failed"
            }
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn chatgptweb_stream_delivers_delta_before_completion_and_holds_profile() {
    use futures_util::{stream, StreamExt};
    use shunt::chatgptweb::{
        Driver, DriverEvent, DriverRequest, DriverStream, DriverTransportError, Event,
    };
    struct PausedDriver(Option<tokio::sync::oneshot::Receiver<()>>);
    #[async_trait::async_trait]
    impl Driver for PausedDriver {
        async fn request(
            &mut self,
            request: DriverRequest,
        ) -> Result<DriverStream, DriverTransportError> {
            let release = self.0.take().unwrap();
            let id = request.id;
            let delta = DriverEvent {
                version: 1,
                id: id.clone(),
                event: Event::TextDelta {
                    text: "early".into(),
                },
            };
            Ok(Box::pin(stream::once(async { Ok(delta) }).chain(
                stream::once(async move {
                    let _ = release.await;
                    Ok(DriverEvent {
                        version: 1,
                        id,
                        event: Event::Done,
                    })
                }),
            )))
        }
    }
    let _vars = common::env_lock().await;
    let (router, _, state) = server::build_router(config(true)).unwrap();
    let (release, wait) = tokio::sync::oneshot::channel();
    state
        .chatgptweb_drivers
        .insert(PROFILE.into(), Box::new(PausedDriver(Some(wait))))
        .await;
    let response = post(router.clone(), request(), false).await;
    let mut body = response.into_body().into_data_stream();
    assert!(body
        .next()
        .await
        .unwrap()
        .unwrap()
        .starts_with(b"event: response.created"));
    assert!(std::str::from_utf8(&body.next().await.unwrap().unwrap())
        .unwrap()
        .contains("early"));
    // A second turn cannot enter this single-use driver while the first owns it.
    assert!(tokio::time::timeout(
        std::time::Duration::from_millis(30),
        post(router, request(), false)
    )
    .await
    .is_err());
    release.send(()).unwrap();
    assert!(std::str::from_utf8(&body.next().await.unwrap().unwrap())
        .unwrap()
        .contains("response.completed"));
    assert!(body.next().await.is_none());
}

#[tokio::test]
async fn chatgptweb_example_has_one_account_and_disabled_accounts_do_not_send() {
    let _vars = common::env_lock().await;
    let example = include_str!("../shunt.toml.example");
    let start = example.find("# [server.codex_endpoint]\n").unwrap();
    let end = start + example[start..].find("\n[providers.codex]").unwrap();
    let example: String = example[start..end]
        .lines()
        .filter_map(|line| line.strip_prefix("# "))
        .map(|line| format!("{line}\n"))
        .collect();
    let example: toml::Value = toml::from_str(&example).unwrap();
    assert_eq!(
        example["providers"]["chatgptweb"]["kind"].as_str(),
        Some("chatgptweb_browser")
    );
    assert_eq!(
        example["providers"]["chatgptweb"]["accounts"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let mut config = config(true);
    config.providers.get_mut("chatgptweb").unwrap().accounts[0].disabled = true;
    let (router, _, _) = server::build_router(config).unwrap();
    let response = post(router, request(), false).await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

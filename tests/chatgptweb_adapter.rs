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
            upstream_model: Some("Medium".into()),
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
                    model: "Medium".into(),
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
    if response.status() != StatusCode::OK {
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        panic!("HTTP {status}: {}", String::from_utf8_lossy(&body));
    }
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
    request["model"] = json!("public-name-is-not-an-effort");
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
        assert_eq!(row["shell_type"], "shell_command");
        assert_eq!(row["apply_patch_tool_type"], "freeform");
        assert_eq!(row["supports_parallel_tool_calls"], false);
        assert_eq!(row["supports_search_tool"], false);
        assert_eq!(row["experimental_supported_tools"], json!([]));
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
    let end = start + example[start..].find("\n\n").unwrap();
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

#[cfg(unix)]
#[tokio::test]
async fn chatgptweb_disconnect_kills_a_detached_browser_descendant() {
    use futures_util::StreamExt;
    let mut vars = common::env_lock().await;
    let dir = std::env::temp_dir().join(format!("shunt-task004-cancel-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    let helper = dir.join("helper.mjs");
    let pid_file = dir.join("browser.pid");
    let pid_path = serde_json::to_string(pid_file.to_str().unwrap()).unwrap();
    std::fs::write(&helper, format!(r#"
import {{createInterface}} from 'node:readline';
import {{spawn}} from 'node:child_process';
import {{writeFileSync}} from 'node:fs';
for await (const line of createInterface({{input:process.stdin}})) {{
  const r=JSON.parse(line);
  const browser=spawn(process.execPath,['-e','setInterval(()=>{{}},1000)'],{{detached:true,stdio:'ignore'}});
  writeFileSync({pid_path},String(browser.pid));
  console.log(JSON.stringify({{version:1,id:r.id,type:'text_delta',text:'waiting'}}));
  await new Promise(()=>{{}});
}}
"#)).unwrap();
    vars.set("SHUNT_CHATGPTWEB_HELPER", &helper);
    let (router, _, _) = server::build_router(config(true)).unwrap();
    let response = post(router, request(), false).await;
    let mut body = response.into_body().into_data_stream();
    let _ = body.next().await;
    assert!(std::str::from_utf8(&body.next().await.unwrap().unwrap())
        .unwrap()
        .contains("waiting"));
    let pid: i32 = std::fs::read_to_string(&pid_file).unwrap().parse().unwrap();
    drop(body);
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while unsafe { libc::kill(pid, 0) } == 0 {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("detached browser process must exit on disconnect");
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn chatgptweb_route_can_omit_effort_without_using_the_public_model() {
    let _vars = common::env_lock().await;
    let mut config = config(true);
    config.server.codex_endpoint.as_mut().unwrap().routes[0].upstream_model = None;
    let config = config.validate().unwrap();
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
                    model: "Medium".into(),
                    deltas: vec!["OK".into()],
                    error: None,
                }],
            )),
        )
        .await;
    let result = events(post(router, request(), false).await).await;
    assert_eq!(result.last().unwrap()["type"], "response.completed");
}

#[derive(Clone)]
struct ToolAnswer {
    records: Vec<Value>,
    text: String,
    failure: bool,
}
struct ToolDriver {
    answers: std::collections::VecDeque<ToolAnswer>,
    prompts: std::sync::Arc<std::sync::Mutex<Vec<Value>>>,
}
#[async_trait::async_trait]
impl shunt::chatgptweb::Driver for ToolDriver {
    async fn request(
        &mut self,
        request: shunt::chatgptweb::DriverRequest,
    ) -> Result<shunt::chatgptweb::DriverStream, shunt::chatgptweb::DriverTransportError> {
        use shunt::chatgptweb::{DriverCommand, DriverEvent, Event};
        let DriverCommand::Generate { prompt, .. } = request.command else {
            panic!("generate expected")
        };
        let prompt: Value = serde_json::from_str(prompt.lines().last().unwrap()).unwrap();
        let turn = prompt["turn"].as_str().unwrap();
        let answer = self.answers.pop_front().expect("no replay after failure");
        let mut chunks: Vec<_> = (!answer.text.is_empty())
            .then_some(answer.text)
            .into_iter()
            .collect();
        for record in answer.records {
            let frame = format!("<<<SHUNT:1:{turn}>>>{record}\n");
            chunks.extend(frame.chars().map(|c| c.to_string()));
        }
        self.prompts.lock().unwrap().push(prompt);
        let mut events: Vec<_> = chunks
            .into_iter()
            .map(|text| Event::TextDelta { text })
            .collect();
        events.push(if answer.failure {
            Event::Error {
                error: DriverError {
                    code: ErrorCode::UiDrift,
                    message: "private".into(),
                    submission: Submission::MayHaveBeenSent,
                    retry_after_ms: None,
                },
            }
        } else {
            Event::Done
        });
        Ok(Box::pin(futures_util::stream::iter(
            events.into_iter().map(move |event| {
                Ok(DriverEvent {
                    version: 1,
                    id: request.id.clone(),
                    event,
                })
            }),
        )))
    }
}
fn tool_request() -> Value {
    json!({"model":"web-chat","stream":true,"parallel_tool_calls":false,"tools":[
        {"type":"function","name":"exec_command","parameters":{"type":"object","properties":{"cmd":{"type":"string"}},"required":["cmd"],"additionalProperties":false}},
        {"type":"custom","name":"apply_patch","format":{"type":"text"}}
    ],"input":[{"role":"user","content":"Read and edit"}]})
}
async fn tool_gateway(
    answers: Vec<ToolAnswer>,
) -> (axum::Router, std::sync::Arc<std::sync::Mutex<Vec<Value>>>) {
    let (router, _, state) = server::build_router(config(true)).unwrap();
    let prompts = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    state
        .chatgptweb_drivers
        .insert(
            PROFILE.into(),
            Box::new(ToolDriver {
                answers: answers.into(),
                prompts: prompts.clone(),
            }),
        )
        .await;
    (router, prompts)
}
#[tokio::test]
async fn chatgptweb_tools_roundtrip_shell_edit_and_final_text() {
    let _vars = common::env_lock().await;
    let shell = json!({"type":"function_call","name":"exec_command","call_id":"shell_1","arguments":"{\"cmd\":\"cat seed.txt\"}"});
    let patch = json!({"type":"custom_tool_call","name":"apply_patch","call_id":"edit_1","input":"*** Begin Patch\n*** Update File: seed.txt\n@@\n-BEFORE\n+AFTER\n*** End Patch\n"});
    let (router, prompts) = tool_gateway(vec![
        ToolAnswer {
            text: "Reading. ".into(),
            records: vec![shell, json!({"type":"done"})],
            failure: false,
        },
        ToolAnswer {
            text: String::new(),
            records: vec![patch, json!({"type":"done"})],
            failure: false,
        },
        ToolAnswer {
            text: "Finished 🌍".into(),
            records: vec![json!({"type":"done"})],
            failure: false,
        },
    ])
    .await;
    let mut request = tool_request();
    for (turn, result) in ["BEFORE", "Success"].into_iter().enumerate() {
        let frames = events(post(router.clone(), request.clone(), turn == 0).await).await;
        for (index, frame) in frames.iter().enumerate() {
            assert_eq!(frame["sequence_number"], index);
        }
        let completed = &frames.last().unwrap()["response"];
        assert_eq!(completed["status"], "completed");
        assert!(completed["usage"].is_null());
        let call = completed["output"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["type"] == "function_call" || v["type"] == "custom_tool_call")
            .unwrap();
        let done = frames
            .iter()
            .find(|f| f["type"] == "response.output_item.done" && f["item"]["id"] == call["id"])
            .unwrap();
        assert_eq!(&done["item"], call);
        let field = if turn == 0 { "arguments" } else { "input" };
        let event = if turn == 0 {
            "response.function_call_arguments.delta"
        } else {
            "response.custom_tool_call_input.delta"
        };
        assert_eq!(
            frames.iter().find(|f| f["type"] == event).unwrap()["delta"],
            call[field]
        );
        request["input"]
            .as_array_mut()
            .unwrap()
            .extend(completed["output"].as_array().unwrap().clone());
        request["input"].as_array_mut().unwrap().push(json!({"type":format!("{}_output",call["type"].as_str().unwrap()),"call_id":call["call_id"],"output":result}));
    }
    request["stream"] = json!(false);
    let response = post(router, request, true).await;
    assert_eq!(response.status(), StatusCode::OK);
    let final_response: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(
        final_response["output"][0]["content"][0]["text"],
        "Finished 🌍"
    );
    let prompts = prompts.lock().unwrap();
    assert_eq!(prompts.len(), 3);
    assert!(prompts[2]["input"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["call_id"] == "shell_1" && v["output"] == "BEFORE"));
    assert!(prompts[2]["input"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["call_id"] == "edit_1" && v["output"] == "Success"));
}

#[tokio::test]
async fn chatgptweb_invalid_tool_answers_never_release_calls() {
    let _vars = common::env_lock().await;
    let shell = json!({"type":"function_call","name":"exec_command","call_id":"shell_1","arguments":"{\"cmd\":\"true\"}"});
    for case in 0..8 {
        let mut request = tool_request();
        let mut call = shell.clone();
        let mut records = vec![shell.clone(), json!({"type":"done"})];
        match case {
            0 => {
                call["name"] = json!("undeclared");
                records[0] = call;
            }
            1 => {
                call["arguments"] = json!("{");
                records[0] = call;
            }
            2 => {
                call["call_id"] = json!("bad id");
                records[0] = call;
            }
            3 => records.push(json!({"type":"done"})),
            4 => {
                request["tool_choice"] = json!("none");
            }
            5 => {
                request["tool_choice"] = json!("required");
                records.remove(0);
            }
            6 => {
                records.pop();
            }
            _ => {}
        }
        let (router, prompts) = tool_gateway(vec![ToolAnswer {
            text: "Already visible".into(),
            records,
            failure: case == 7,
        }])
        .await;
        let frames = events(post(router, request, false).await).await;
        assert_eq!(
            frames.last().unwrap()["type"],
            "response.failed",
            "case {case}"
        );
        assert!(frames
            .iter()
            .any(|f| f["type"] == "response.output_text.delta" && f["delta"] == "Already visible"));
        assert!(
            !frames.iter().any(|f| f["type"] == "response.completed"
                || f["type"] == "response.function_call_arguments.delta"
                || f["item"]["type"] == "function_call"),
            "case {case}"
        );
        assert_eq!(prompts.lock().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn chatgptweb_validates_tool_inventory_and_history_before_send() {
    let _vars = common::env_lock().await;
    for case in 0..6 {
        let mut request = tool_request();
        match case {
            0 => request["tools"] = json!({}),
            1 => { request["tools"] = json!([]); request["tool_choice"] = json!("required"); },
            2 => request["tools"].as_array_mut().unwrap().push(json!({"type":"tool_search","name":"tool_search"})),
            3 => request["input"].as_array_mut().unwrap().push(json!({"type":"function_call_output","call_id":"unknown","output":"BEFORE"})),
            4 => request["tool_choice"] = json!({"type":"function","name":"undeclared"}),
            _ => request["input"].as_array_mut().unwrap().push(json!({"type":"function_call","name":"exec_command","call_id":"missing","arguments":"{\"cmd\":\"true\"}"})),
        }
        let (router, prompts) = tool_gateway(vec![]).await;
        assert_eq!(
            post(router, request, false).await.status(),
            StatusCode::BAD_REQUEST,
            "case {case}"
        );
        assert!(prompts.lock().unwrap().is_empty());
    }
}

#[tokio::test]
async fn chatgptweb_tool_snapshots_match_stream_and_nonstream_with_bounded_long_transcript() {
    let _vars = common::env_lock().await;
    let start = std::time::Instant::now();
    let call = json!({"type":"function_call","name":"exec_command","call_id":"stable","arguments":"{\"cmd\":\"true\"}"});
    let answer = ToolAnswer {
        text: "hello 🌍".into(),
        records: vec![call, json!({"type":"done"})],
        failure: false,
    };
    let (router, prompts) = tool_gateway(vec![answer.clone(), answer]).await;
    let mut request = tool_request();
    request["input"][0]["content"] = json!((0..28125)
        .map(|_| uuid::Uuid::new_v4().simple().to_string())
        .collect::<String>());
    let stream = events(post(router.clone(), request.clone(), true).await).await;
    let expected = stream.last().unwrap()["response"].clone();
    assert_eq!(expected["status"], "completed");
    request["stream"] = json!(false);
    let response = post(router.clone(), request.clone(), false).await;
    assert_eq!(response.status(), StatusCode::OK);
    let actual: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    fn normalize(mut value: Value) -> Value {
        value.as_object_mut().unwrap().remove("id");
        value.as_object_mut().unwrap().remove("created_at");
        for item in value["output"].as_array_mut().unwrap() {
            item.as_object_mut().unwrap().remove("id");
        }
        value
    }
    assert_eq!(normalize(expected), normalize(actual));
    request["input"][0]["content"] = json!("a".repeat(1024 * 1024));
    assert_eq!(
        post(router, request, false).await.status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        prompts.lock().unwrap().len(),
        2,
        "over-limit request must not send"
    );
    println!("long transcript: 900000 bytes accepted, 1 MiB rejected, character-split frames; elapsed_ms={}",start.elapsed().as_millis());
}

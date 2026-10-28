//! Offline capture of the installed Codex shell/edit Responses surface.
//! No credentials, user config, browser, or real model endpoint are used.
mod common;
#[path = "common/chatgptweb_smoke.rs"]
mod smoke;

use axum::{extract::State, http::header, routing::post, Json, Router};
use serde_json::{json, Value};
use std::{
    fs,
    path::PathBuf,
    process::Stdio,
    sync::{Arc, Mutex},
    time::Duration,
};

const DIRECT: &str = include_str!("fixtures/chatgptweb_codex/direct.json");
const CODE_MODE: &str = include_str!("fixtures/chatgptweb_codex/code_mode.json");
const PATCH: &str = "*** Begin Patch\n*** Add File: result.txt\n+edited ✓\n*** End Patch\n";

fn calls(code_mode: bool) -> [Value; 2] {
    if code_mode {
        [
            json!({"id":"ct_shell","type":"custom_tool_call","call_id":"call_shell","namespace":"functions","name":"exec","input":"text(await tools.exec_command({cmd: \"cat seed.txt\", login: false}));"}),
            json!({"id":"ct_patch","type":"custom_tool_call","call_id":"call_edit","namespace":"functions","name":"exec","input":"text(await tools.apply_patch(\"*** Begin Patch\\n*** Add File: result.txt\\n+edited ✓\\n*** End Patch\\n\"));"}),
        ]
    } else {
        [
            json!({"id":"fc_shell","type":"function_call","call_id":"call_shell","name":"exec_command","arguments":"{\"cmd\": \"cat seed.txt\", \"login\": false}"}),
            json!({"id":"ct_patch","type":"custom_tool_call","call_id":"call_edit","name":"apply_patch","input":PATCH}),
        ]
    }
}

fn declarations(request: &Value, code_mode: bool) -> Value {
    if code_mode {
        let additional = request["input"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["type"] == "additional_tools")
            .expect("code-mode tools are declared in ordered input");
        let mut additional = additional.clone();
        additional["id"] = json!("at_capture");
        json!({"input":[additional]})
    } else {
        json!({"tools":request["tools"]})
    }
}

fn history(request: &Value) -> Vec<Value> {
    request["input"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|item| {
            matches!(
                item["type"].as_str(),
                Some(
                    "function_call"
                        | "function_call_output"
                        | "custom_tool_call"
                        | "custom_tool_call_output"
                )
            )
        })
        .cloned()
        .collect()
}

fn assert_surface(requests: &[Value], code_mode: bool, fixture: &Value) {
    assert_eq!(
        requests.len(),
        3,
        "initial, shell continuation, edit continuation"
    );
    assert_eq!(
        declarations(&requests[0], code_mode),
        fixture["declarations"]
    );
    let expected = calls(code_mode);
    for (turn, request) in requests.iter().enumerate() {
        assert_eq!(request["tool_choice"], "auto");
        let items = history(request);
        assert_eq!(items.len(), turn * 2);
        for (index, call) in expected.iter().take(turn).enumerate() {
            assert_eq!(items[index * 2], *call);
            let result = &items[index * 2 + 1];
            assert_eq!(result["call_id"], call["call_id"]);
            let result_type = if code_mode || index == 1 {
                "custom_tool_call_output"
            } else {
                "function_call_output"
            };
            assert_eq!(result["type"], result_type);
            if code_mode {
                let output = result["output"]
                    .as_array()
                    .expect("code-mode text content array");
                assert!(!output.is_empty());
                for part in output {
                    assert_eq!(part["type"], "input_text");
                    assert!(part["text"].is_string());
                }
            } else {
                assert!(result["output"].is_string());
            }
            if index == 0 {
                assert!(result["output"].to_string().contains("seed ✓"));
            } else if !code_mode {
                assert!(result["output"].as_str().unwrap().contains("A result.txt"));
            }
        }
    }
}

#[test]
fn sanitized_fixtures_cover_shell_edit_and_continuations() {
    for (source, code_mode) in [(DIRECT, false), (CODE_MODE, true)] {
        let fixture: Value = serde_json::from_str(source).unwrap();
        assert_eq!(fixture["codex_version"], "codex-cli 0.164.0");
        let mut requests = Vec::new();
        for turn in fixture["turns"].as_array().unwrap() {
            let mut request = fixture["declarations"].clone();
            request["tool_choice"] = fixture["tool_choice"].clone();
            let mut input = request["input"].as_array().cloned().unwrap_or_default();
            input.extend(turn["input"].as_array().unwrap().iter().cloned());
            request["input"] = json!(input);
            requests.push(request);
        }
        assert_surface(&requests, code_mode, &fixture);
    }
}

#[derive(Clone)]
struct Probe {
    code_mode: bool,
    requests: Arc<Mutex<Vec<Value>>>,
}

async fn respond(
    State(probe): State<Probe>,
    Json(request): Json<Value>,
) -> ([(header::HeaderName, &'static str); 1], String) {
    let step = {
        let mut requests = probe.requests.lock().unwrap();
        let step = requests.len();
        requests.push(request);
        step
    };
    assert!(step <= 2, "unexpected extra Codex turn");
    let item = calls(probe.code_mode).get(step).cloned().unwrap_or_else(|| json!({"id":"msg_final","type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"Captured.","annotations":[]}]}));
    let events = [
        json!({"type":"response.created","response":{"id":format!("resp_{step}"),"status":"in_progress","output":[]}}),
        json!({"type":"response.output_item.done","output_index":0,"item":item}),
        json!({"type":"response.completed","response":{"id":format!("resp_{step}"),"status":"completed","output":[item],"usage":{"input_tokens":1,"output_tokens":1,"total_tokens":2}}}),
    ];
    let body = events
        .iter()
        .map(|event| {
            format!(
                "event: {}\ndata: {event}\n\n",
                event["type"].as_str().unwrap()
            )
        })
        .collect();
    ([(header::CONTENT_TYPE, "text/event-stream")], body)
}

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Runs only a fixed read and a fixed file creation in an isolated scratch repo.
#[tokio::test]
#[ignore = "requires installed Codex CLI; loopback only, never a live provider"]
async fn capture_tool_surface() {
    let _vars = common::env_lock().await;
    let version = std::process::Command::new("codex")
        .arg("--version")
        .output()
        .unwrap();
    assert!(version.status.success());
    let version = String::from_utf8(version.stdout).unwrap();
    println!("{}", version.trim());
    for (source, code_mode) in [(DIRECT, false), (CODE_MODE, true)] {
        let fixture: Value = serde_json::from_str(source).unwrap();
        let scratch = Scratch(
            std::env::temp_dir().join(format!("shunt-codex-capture-{}", uuid::Uuid::new_v4())),
        );
        let repo = scratch.0.join("repo");
        let home = scratch.0.join("home");
        fs::create_dir_all(&repo).unwrap();
        fs::create_dir_all(&home).unwrap();
        fs::write(repo.join("seed.txt"), "seed ✓\n").unwrap();
        assert!(std::process::Command::new("git")
            .args(["init", "-q"])
            .arg(&repo)
            .status()
            .unwrap()
            .success());
        let probe = Probe {
            code_mode,
            requests: Arc::default(),
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let app = Router::new()
            .route("/v1/responses", post(respond))
            .with_state(probe.clone());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let model = fixture["model"].as_str().unwrap();
        let mut command = tokio::process::Command::new("codex");
        command
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap())
            .env("HOME", &home)
            .env("CODEX_HOME", &home)
            .env("TMPDIR", &scratch.0)
            .args([
                "exec",
                "--ignore-user-config",
                "--ignore-rules",
                "--ephemeral",
                "--skip-git-repo-check",
                "-C",
            ])
            .arg(&repo)
            .args([
                "-s",
                "workspace-write",
                "-m",
                model,
                "-c",
                "model_provider=\"shunt\"",
                "-c",
                "model_providers.shunt.name=\"offline\"",
                "-c",
            ])
            .arg(format!(
                "model_providers.shunt.base_url=\"http://{address}/v1\""
            ))
            .args([
                "-c",
                "model_providers.shunt.wire_api=\"responses\"",
                "-c",
                "model_providers.shunt.requires_openai_auth=false",
                "-c",
                "web_search=\"disabled\"",
                "-c",
                "features.shell_tool=true",
                "-c",
                "features.apply_patch_freeform=true",
                "Read seed.txt and write result.txt.",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let child = command.spawn().unwrap();
        let result = tokio::time::timeout(Duration::from_secs(60), child.wait_with_output()).await;
        server.abort();
        let output = result.expect("offline Codex capture timed out").unwrap();
        assert!(
            output.status.success(),
            "Codex failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            fs::read_to_string(repo.join("result.txt")).unwrap(),
            "edited ✓\n"
        );
        assert_surface(&probe.requests.lock().unwrap(), code_mode, &fixture);
        println!("{model}: declarations, shell/edit calls, correlated continuation outputs, scratch edit verified");
    }
}

/// Executes real Codex against the injected scripted browser driver.
#[tokio::test]
#[ignore = "requires installed Codex CLI; no browser, account or external network"]
async fn demo() {
    let _vars = common::env_lock().await;
    smoke::demo().await;
}

/// Explicit live gate; missing inputs fail rather than silently skip.
#[tokio::test]
#[ignore = "live Chat-only gate: requires explicit profile and owner Chrome CDP URL"]
async fn live() {
    let mut vars = common::env_lock().await;
    smoke::live(&mut vars).await;
}

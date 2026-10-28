//! Test-only Codex runner and explicit, bounded live gate.
use super::{common, Scratch};
use async_trait::async_trait;
use futures_util::stream;
use serde_json::{json, Value};
use shunt::{
    chatgptweb::{
        Driver, DriverCommand, DriverEvent, DriverRequest, DriverStream, DriverTransportError,
        Event,
    },
    config::{
        AccountConfig, AuthMode, CodexEndpointConfig, CodexRouteConfig, Config, ProviderKind,
    },
    server,
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

const MODEL: &str = "gpt-6.1-sol";
const PROMPT: &str = "Read seed.txt with a shell command. Use apply_patch to change result.txt from BEFORE to AFTER. Read result.txt to check the change. Return DEMO_COMPLETE only after tool results confirm the edit.";
const PATCH: &str =
    "*** Begin Patch\n*** Update File: result.txt\n@@\n-BEFORE\n+AFTER\n*** End Patch\n";
const TOTAL: Duration = Duration::from_secs(15 * 60);
const TURN_MS: u32 = 120_000;
const LAUNCHER: &str = include_str!("chatgptweb_smoke_launcher.mjs");

fn scratch() -> Scratch {
    let scratch =
        Scratch(std::env::temp_dir().join(format!("shunt-codex-demo-{}", uuid::Uuid::new_v4())));
    for directory in ["repo", "home"] {
        fs::create_dir_all(scratch.0.join(directory)).unwrap();
    }
    fs::write(scratch.0.join("repo/seed.txt"), "seed ✓\n").unwrap();
    fs::write(scratch.0.join("repo/result.txt"), "BEFORE\n").unwrap();
    assert!(std::process::Command::new("git")
        .args(["init", "-q"])
        .arg(scratch.0.join("repo"))
        .status()
        .unwrap()
        .success());
    scratch
}
fn config(profile: &str) -> Config {
    let mut config = Config::default();
    let mut provider = config.providers["openai"].clone();
    provider.kind = ProviderKind::ChatgptwebBrowser;
    provider.auth = AuthMode::None;
    provider.api_key_env = None;
    provider.base_url = "http://localhost".into();
    provider.accounts = vec![AccountConfig {
        name: "cemfi".into(),
        profile_dir: Some(profile.into()),
        ..Default::default()
    }];
    config.providers.insert("chatgptweb".into(), provider);
    config.server.codex_endpoint = Some(CodexEndpointConfig {
        provider: "chatgptweb".into(),
        routes: vec![CodexRouteConfig {
            model: MODEL.into(),
            provider: "chatgptweb".into(),
            upstream_model: Some("Medium".into()),
        }],
    });
    config.server.timeouts.upstream_ttfb_ms = TURN_MS.into();
    config
}
fn command(scratch: &Scratch, address: std::net::SocketAddr) -> tokio::process::Command {
    let mut command = tokio::process::Command::new("codex");
    command
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap())
        .env("HOME", scratch.0.join("home"))
        .env("CODEX_HOME", scratch.0.join("home"))
        .env("TMPDIR", &scratch.0)
        .args([
            "exec",
            "--ignore-user-config",
            "--ignore-rules",
            "--ephemeral",
            "--skip-git-repo-check",
            "--json",
            "-C",
        ])
        .arg(scratch.0.join("repo"))
        .args([
            "-s",
            "workspace-write",
            "-m",
            MODEL,
            "-c",
            "model_provider=\"shunt\"",
            "-c",
            "model_providers.shunt.name=\"shunt\"",
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
            "model_providers.shunt.request_max_retries=0",
            "-c",
            "model_providers.shunt.stream_max_retries=0",
            "-c",
            "web_search=\"disabled\"",
            "-c",
            "features.shell_tool=true",
            "-c",
            "features.apply_patch_freeform=true",
            PROMPT,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    command
}
struct Server(tokio::task::JoinHandle<()>);
impl Drop for Server {
    fn drop(&mut self) {
        self.0.abort();
    }
}
async fn serve(router: axum::Router) -> (std::net::SocketAddr, Server) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    (
        address,
        Server(tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap()
        })),
    )
}
fn outputs(transcript: &Value) -> Vec<&Value> {
    transcript["input"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|item| {
            matches!(
                item["type"].as_str(),
                Some("function_call_output" | "custom_tool_call_output")
            )
        })
        .collect()
}
struct Script {
    requests: Arc<Mutex<Vec<Value>>>,
    malformed: bool,
}
#[async_trait]
impl Driver for Script {
    async fn request(
        &mut self,
        request: DriverRequest,
    ) -> Result<DriverStream, DriverTransportError> {
        let DriverCommand::Generate { prompt, model } = request.command else {
            panic!("demo only generates")
        };
        assert_eq!(model.as_deref(), Some("Medium"));
        assert!(prompt.len() <= 1024 * 1024, "declared prompt bound");
        let transcript: Value = serde_json::from_str(prompt.lines().last().unwrap()).unwrap();
        let step = {
            let mut requests = self.requests.lock().unwrap();
            let step = requests.len();
            requests.push(transcript.clone());
            step
        };
        assert!(step <= 3, "no replay or unbounded generation");
        let results = outputs(&transcript);
        assert_eq!(
            results.len(),
            step,
            "every real tool result must return in the next full transcript"
        );
        if step >= 1 {
            assert_eq!(results[0]["call_id"], "demo_shell");
            assert!(results[0]["output"].to_string().contains("seed ✓"));
        }
        if step >= 2 {
            assert_eq!(results[1]["call_id"], "demo_edit");
            let parts = results[1]["output"]
                .as_array()
                .expect("code-mode output parts");
            assert!(parts.iter().all(|part| part["type"] == "input_text"));
            assert!(parts[0]["text"]
                .as_str()
                .unwrap()
                .contains("Script completed"));
            assert_eq!(parts[1]["text"], "{}");
        }
        if step >= 3 {
            assert_eq!(results[2]["call_id"], "demo_verify");
            assert!(results[2]["output"].to_string().contains("AFTER"));
        }
        let turn = transcript["turn"].as_str().unwrap();
        let record = if step == 0 && !self.malformed {
            json!({"type":"custom_tool_call","namespace":"functions","name":"exec","call_id":"demo_shell","input":"text(await tools.exec_command({cmd: \"cat seed.txt\", login:false}));"})
        } else if step == 1 || self.malformed {
            json!({"type":"custom_tool_call","namespace":"functions","name":"exec","call_id":"demo_edit","input":format!("text(await tools.apply_patch({}));",serde_json::to_string(PATCH).unwrap())})
        } else {
            json!({"type":"custom_tool_call","namespace":"functions","name":"exec","call_id":"demo_verify","input":"text(await tools.exec_command({cmd: \"cat result.txt\", login:false}));"})
        };
        let answer = if self.malformed {
            format!("<<<SHUNT:1:{turn}>>>{record}\n<<<SHUNT:1:{turn}>>>{{\"type\":\"custom_tool_call\",\"name\":\"missing\",\"input\":\"bad\"}}\n<<<SHUNT:1:{turn}>>>{{\"type\":\"done\"}}\n")
        } else if step < 3 {
            format!("Running step {}.\n<<<SHUNT:1:{turn}>>>{record}\n<<<SHUNT:1:{turn}>>>{{\"type\":\"done\"}}\n",step+1)
        } else {
            format!("DEMO_COMPLETE<<<SHUNT:1:{turn}>>>{{\"type\":\"done\"}}\n")
        };
        assert!(answer.len() <= 4 * 1024 * 1024);
        let mut events: Vec<_> = answer
            .chars()
            .map(|c| {
                Ok(DriverEvent {
                    version: 1,
                    id: request.id.clone(),
                    event: Event::TextDelta {
                        text: c.to_string(),
                    },
                })
            })
            .collect();
        events.push(Ok(DriverEvent {
            version: 1,
            id: request.id,
            event: Event::Done,
        }));
        Ok(Box::pin(stream::iter(events)))
    }
}
fn assert_codex(output: &std::process::Output, scratch: &Scratch) {
    assert!(
        output.status.success(),
        "Codex failed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read(scratch.0.join("repo/result.txt")).unwrap(),
        b"AFTER\n"
    );
    let events: Vec<Value> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert!(
        events.iter().any(|e| e["type"] == "turn.completed"),
        "final turn completion"
    );
    assert!(
        events.iter().any(|e| e["item"]["type"] == "agent_message"
            && e["item"]["text"]
                .as_str()
                .is_some_and(|s| s.contains("DEMO_COMPLETE"))),
        "final answer"
    );
    assert!(
        events
            .iter()
            .any(|e| e["item"]["type"] == "command_execution"),
        "real shell event: {events:?}"
    );
    assert!(
        events.iter().any(|e| e["item"]["type"] == "file_change"),
        "real edit event: {events:?}"
    );
}
pub async fn demo() {
    let start = Instant::now();
    for malformed in [false, true] {
        let scratch = scratch();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let profile = scratch.0.join("profile");
        fs::create_dir(&profile).unwrap();
        let (router, _, state) = server::build_router(config(profile.to_str().unwrap())).unwrap();
        state
            .chatgptweb_drivers
            .insert(
                profile.to_str().unwrap().into(),
                Box::new(Script {
                    requests: requests.clone(),
                    malformed,
                }),
            )
            .await;
        let (address, _server) = serve(router).await;
        let result = tokio::time::timeout(
            Duration::from_secs(90),
            command(&scratch, address)
                .spawn()
                .unwrap()
                .wait_with_output(),
        )
        .await
        .expect("offline demo deadline")
        .unwrap();
        if malformed {
            assert!(
                !result.status.success(),
                "malformed tool output cannot complete"
            );
            assert_eq!(
                fs::read(scratch.0.join("repo/result.txt")).unwrap(),
                b"BEFORE\n"
            );
            assert_eq!(
                requests.lock().unwrap().len(),
                1,
                "failed answer never retries"
            );
        } else {
            assert_codex(&result, &scratch);
            assert_eq!(requests.lock().unwrap().len(), 4);
        }
    }
    println!("demo: real Codex shell, edit BEFORE→AFTER, all returned results, final completion; malformed output cannot edit; elapsed_ms={}",start.elapsed().as_millis());
}

struct ProfileLock(PathBuf);
impl ProfileLock {
    fn take(profile: &Path) -> std::io::Result<Self> {
        use std::hash::{Hash, Hasher};
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        profile.canonicalize()?.hash(&mut hash);
        let path = std::env::temp_dir().join(format!("shunt-codex-live-{:x}.lock", hash.finish()));
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        Ok(Self(path))
    }
}
impl Drop for ProfileLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
fn inputs(profile: Option<String>, cdp: Option<String>) -> Result<(PathBuf, String), &'static str> {
    let profile = PathBuf::from(profile.ok_or("SHUNT_CHATGPTWEB_SMOKE_PROFILE is required")?);
    if !profile.is_absolute() || !profile.is_dir() {
        return Err("profile must be an existing absolute directory");
    }
    let cdp = cdp.ok_or("SHUNT_CHATGPTWEB_SMOKE_CDP_URL is required")?;
    let url = reqwest::Url::parse(&cdp).map_err(|_| "invalid CDP URL")?;
    if url.scheme() != "http"
        || !matches!(url.host_str(), Some("127.0.0.1" | "[::1]"))
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("CDP must be HTTP numeric loopback without authentication");
    }
    Ok((profile, cdp))
}
fn launcher(scratch: &Scratch, cdp: &str) -> PathBuf {
    let cli = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("helpers/chatgptweb-browser/cli.mjs");
    let source = LAUNCHER
        .replace(
            "__CLI__",
            &serde_json::to_string(&cli.to_str().unwrap()).unwrap(),
        )
        .replace("__CDP__", &serde_json::to_string(cdp).unwrap())
        .replace(
            "__LEDGER__",
            &serde_json::to_string(scratch.0.join("ledger.json").to_str().unwrap()).unwrap(),
        );
    let path = scratch.0.join("launcher.mjs");
    fs::write(&path, source).unwrap();
    path
}
struct OwnedTabs(PathBuf);
impl Drop for OwnedTabs {
    fn drop(&mut self) {
        let status = std::process::Command::new("node")
            .arg(&self.0)
            .arg("--cleanup")
            .status();
        if !status.is_ok_and(|status| status.success()) {
            if std::thread::panicking() {
                eprintln!("live owned-tab cleanup failed");
            } else {
                panic!("live owned-tab cleanup failed");
            }
        }
    }
}
async fn health(launcher: &Path, profile: &Path) {
    let mut child = tokio::process::Command::new("node")
        .arg(launcher)
        .args(["--account", "cemfi", "--profile-dir"])
        .arg(profile)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let request = json!({"version":1,"id":"live-health","type":"health","account":"cemfi","profile_dir":profile,"timeout_ms":TURN_MS});
    let mut stdin = child.stdin.take().unwrap();
    stdin
        .write_all(format!("{request}\n").as_bytes())
        .await
        .unwrap();
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let event: Value = serde_json::from_str(
        &tokio::time::timeout(Duration::from_millis(TURN_MS.into()), lines.next_line())
            .await
            .expect("health deadline")
            .unwrap()
            .expect("health EOF"),
    )
    .unwrap();
    drop(stdin);
    let status = child.wait().await.unwrap();
    assert!(status.success());
    assert_eq!(
        event["type"], "health",
        "Chat-only non-sending preflight: {event}"
    );
}
pub async fn live(vars: &mut common::EnvVars) {
    let start = Instant::now();
    let (profile, cdp) = inputs(
        std::env::var("SHUNT_CHATGPTWEB_SMOKE_PROFILE").ok(),
        std::env::var("SHUNT_CHATGPTWEB_SMOKE_CDP_URL").ok(),
    )
    .expect("live requires explicit inputs; never skips");
    let _lock = ProfileLock::take(&profile).expect("another live harness owns this profile");
    let scratch = scratch();
    fs::write(scratch.0.join("ledger.json"),json!({"deadline":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis()+TOTAL.as_millis(),"sends":0,"stopped":false,"calls":0,"results":0}).to_string()).unwrap();
    let launcher = launcher(&scratch, &cdp);
    let _tabs = OwnedTabs(launcher.clone());
    vars.set("SHUNT_CHATGPTWEB_HELPER", &launcher);
    tokio::time::timeout(
        Duration::from_millis(TURN_MS.into()),
        health(&launcher, &profile),
    )
    .await
    .expect("health lifecycle deadline");
    let (router, _, _state) = server::build_router(config(profile.to_str().unwrap())).unwrap();
    let (address, _server) = serve(router).await;
    let remaining = TOTAL
        .checked_sub(start.elapsed())
        .expect("total live deadline");
    let output = tokio::time::timeout(
        remaining,
        command(&scratch, address)
            .spawn()
            .unwrap()
            .wait_with_output(),
    )
    .await
    .expect("15-minute live deadline")
    .unwrap();
    assert_codex(&output, &scratch);
    let ledger: Value =
        serde_json::from_slice(&fs::read(scratch.0.join("ledger.json")).unwrap()).unwrap();
    assert!(ledger["sends"].as_u64().unwrap() <= 6);
    assert!(ledger["calls"].as_u64().unwrap() >= 2);
    assert!(ledger["results"].as_u64().unwrap() >= 2);
    let version = std::process::Command::new("codex")
        .arg("--version")
        .output()
        .unwrap();
    let revision = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    println!("live revision={} cli={} account=cemfi effort=Medium calls={} results={} turns={} elapsed_ms={} final_file=AFTER",String::from_utf8_lossy(&revision.stdout).trim(),String::from_utf8_lossy(&version.stdout).trim(),ledger["calls"],ledger["results"],ledger["sends"],start.elapsed().as_millis());
}

#[test]
fn live_inputs_and_profile_lock_are_explicit() {
    let _vars = common::set_env_blocking(&[]);
    assert!(inputs(None, None).is_err());
    let scratch = scratch();
    for cdp in [
        None,
        Some("https://127.0.0.1:9333".into()),
        Some("http://user:secret@127.0.0.1:9333".into()),
        Some("http://example.com:9333".into()),
    ] {
        assert!(inputs(Some(scratch.0.to_str().unwrap().into()), cdp).is_err());
    }
    assert!(inputs(
        Some(scratch.0.to_str().unwrap().into()),
        Some("http://127.0.0.1:9333".into())
    )
    .is_ok());
    let lock = ProfileLock::take(&scratch.0).unwrap();
    assert!(ProfileLock::take(&scratch.0).is_err());
    drop(lock);
    assert!(ProfileLock::take(&scratch.0).is_ok());
}

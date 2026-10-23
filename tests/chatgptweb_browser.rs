//! Optional browser suite. `npm test` in helpers/chatgptweb-browser installs the
//! runtime boundary around TASK-002's unchanged conformance runner.
mod common;
#[path = "common/chatgptweb_conformance.rs"]
mod conformance;

use std::{path::PathBuf, process::Stdio, sync::Arc, time::Duration};

use async_trait::async_trait;
use conformance::{assert_case, Expected};
use futures_util::stream;
use shunt::chatgptweb::{
    Driver, DriverCommand, DriverEvent, DriverRequest, DriverStream, DriverTransportError,
    ErrorCode, Event, Submission,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    process::{Child, ChildStdin, ChildStdout, Command},
    sync::Mutex,
};

// Test-only pipe bridge, not the adapter/process supervisor from TASK-004.
struct ProcessDriver {
    child: Child,
    input: Option<ChildStdin>,
    output: Arc<Mutex<Lines<BufReader<ChildStdout>>>>,
    profile: PathBuf,
}

impl ProcessDriver {
    fn spawn(root: &std::path::Path, url: &str, scenario: &str) -> Self {
        let profile = std::env::temp_dir().join(format!(
            "shunt-conformance-{}-{scenario}",
            std::process::id()
        ));
        std::fs::create_dir(&profile).unwrap();
        let mut child = Command::new("node")
            .arg(root.join("cli.mjs"))
            .args(["--account", "fixture", "--profile-dir"])
            .arg(&profile)
            .args(["--headless", "--fixture-url"])
            .arg(format!("{url}?state={scenario}"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let input = Some(child.stdin.take().unwrap());
        let output = Arc::new(Mutex::new(
            BufReader::new(child.stdout.take().unwrap()).lines(),
        ));
        Self {
            child,
            input,
            output,
            profile,
        }
    }

    fn request(&self, id: &str, command: DriverCommand) -> DriverRequest {
        DriverRequest {
            version: 1,
            id: id.into(),
            account: "fixture".into(),
            profile_dir: self.profile.to_str().unwrap().into(),
            timeout_ms: 5000,
            command,
        }
    }

    async fn close(mut self) {
        drop(self.input.take());
        let status = tokio::time::timeout(Duration::from_secs(10), self.child.wait())
            .await
            .unwrap()
            .unwrap();
        assert!(status.success());
        tokio::fs::remove_dir_all(self.profile).await.unwrap();
    }
}

#[async_trait]
impl Driver for ProcessDriver {
    async fn request(
        &mut self,
        request: DriverRequest,
    ) -> Result<DriverStream, DriverTransportError> {
        let mut line = serde_json::to_vec(&request).unwrap();
        line.push(b'\n');
        self.input
            .as_mut()
            .unwrap()
            .write_all(&line)
            .await
            .map_err(|_| DriverTransportError("helper input failed".into()))?;
        Ok(Box::pin(stream::unfold(
            (self.output.clone(), false),
            |(output, ended)| async move {
                if ended {
                    return None;
                }
                let next = output.lock().await.next_line().await;
                let event = match next {
                    Ok(Some(line)) => serde_json::from_str::<DriverEvent>(&line)
                        .map_err(|_| DriverTransportError("invalid helper event".into())),
                    _ => Err(DriverTransportError("premature helper EOF".into())),
                };
                let ended = !matches!(
                    &event,
                    Ok(DriverEvent {
                        event: Event::TextDelta { .. },
                        ..
                    })
                );
                Some((event, (output, ended)))
            },
        )))
    }
}

#[tokio::test]
#[ignore = "requires Node, npm ci and Playwright Chromium; run helpers/chatgptweb-browser npm test"]
async fn chatgptweb_browser_uses_shared_fake_conformance_with_local_page() {
    let _env = common::set_env(&[]).await;
    tokio::time::timeout(Duration::from_secs(90), async {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("helpers/chatgptweb-browser");
        let mut server = Command::new("node")
            .arg(root.join("test/fixture.mjs"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let url = BufReader::new(server.stdout.take().unwrap())
            .lines()
            .next_line()
            .await
            .unwrap()
            .unwrap();
        for (scenario, error) in [
            ("chat", None),
            ("empty", None),
            ("work", Some(ErrorCode::NotChatMode)),
            ("logged_out", Some(ErrorCode::LoggedOut)),
            ("drift", Some(ErrorCode::UiDrift)),
        ] {
            let mut driver = ProcessDriver::spawn(&root, &url, scenario);
            let health = driver.request("health", DriverCommand::Health);
            assert_case(
                &mut driver,
                health,
                error.map_or(Expected::Health, |code| {
                    Expected::Error(code, Submission::NotSent, None, "")
                }),
            )
            .await;
            for id in ["turn-1", "turn-2"] {
                let turn = driver.request(
                    id,
                    DriverCommand::Generate {
                        prompt: "exact 🌍\n你好".into(),
                        model: "fixture-model".into(),
                    },
                );
                assert_case(
                    &mut driver,
                    turn,
                    error.map_or(
                        Expected::Answer(if scenario == "empty" {
                            ""
                        } else {
                            "Hello 🌍\n你好"
                        }),
                        |code| Expected::Error(code, Submission::NotSent, None, ""),
                    ),
                )
                .await;
            }
            driver.close().await;
        }
        drop(server.stdin.take());
        assert!(server.wait().await.unwrap().success());
    })
    .await
    .expect("browser fixture conformance deadline");
}

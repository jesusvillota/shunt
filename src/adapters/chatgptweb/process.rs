//! Private JSON Lines pipes. One fresh helper per turn; no HTTP or OAuth path.
use crate::chatgptweb::{
    Driver, DriverEvent, DriverRequest, DriverStream, DriverTransportError, Event,
};
use async_trait::async_trait;
use futures_util::stream;
use std::{
    collections::HashSet,
    process::Stdio,
    sync::{Mutex, OnceLock},
    time::Duration,
};
use tokio::{
    io::{AsyncWriteExt, BufReader},
    process::{Child, Command},
};

static GROUPS: OnceLock<Mutex<(bool, HashSet<u32>)>> = OnceLock::new();
fn groups() -> &'static Mutex<(bool, HashSet<u32>)> {
    GROUPS.get_or_init(Mutex::default)
}

/// Stop new helpers and kill all browser process groups, including on forced exit.
pub fn terminate_all() {
    let mut groups = groups().lock().unwrap_or_else(|e| e.into_inner());
    groups.0 = true;
    for pid in &groups.1 {
        kill_group(*pid);
    }
}
fn kill_group(pid: u32) {
    #[cfg(unix)]
    unsafe {
        libc::kill(-(pid as i32), libc::SIGKILL);
    }
    #[cfg(not(unix))]
    let _ = pid;
}
struct Running {
    child: Option<Child>,
    pid: u32,
}
impl Drop for Running {
    fn drop(&mut self) {
        kill_group(self.pid);
        if let Some(mut child) = self.child.take() {
            let _ = child.start_kill();
            tokio::spawn(async move {
                let _ = child.wait().await;
            });
        }
        groups()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .1
            .remove(&self.pid);
    }
}
pub(super) struct ProcessDriver;
#[async_trait]
impl Driver for ProcessDriver {
    async fn request(
        &mut self,
        request: DriverRequest,
    ) -> Result<DriverStream, DriverTransportError> {
        let helper = std::env::var_os("SHUNT_CHATGPTWEB_HELPER")
            .unwrap_or_else(|| "helpers/chatgptweb-browser/cli.mjs".into());
        let mut command = Command::new("node");
        command
            .arg(helper)
            .arg("--account")
            .arg(&request.account)
            .arg("--profile-dir")
            .arg(&request.profile_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        #[cfg(unix)]
        command.process_group(0);
        let (running, stdout, mut stdin) = {
            // Register before shutdown can sweep; never spawn beyond its latch.
            let mut groups = groups().lock().unwrap_or_else(|e| e.into_inner());
            if groups.0 {
                return Err(DriverTransportError("gateway is stopping".into()));
            }
            let mut child = command
                .spawn()
                .map_err(|_| DriverTransportError("cannot launch browser helper".into()))?;
            let pid = child.id().expect("new process has an id");
            groups.1.insert(pid);
            let stdout = child.stdout.take().unwrap();
            let stdin = child.stdin.take().unwrap();
            (
                Running {
                    child: Some(child),
                    pid,
                },
                stdout,
                stdin,
            )
        };
        let deadline =
            tokio::time::Instant::now() + Duration::from_millis(request.timeout_ms as u64);
        let mut wire = serde_json::to_vec(&request).unwrap();
        wire.push(b'\n');
        tokio::time::timeout_at(deadline, stdin.write_all(&wire))
            .await
            .map_err(|_| DriverTransportError("helper write timeout".into()))?
            .map_err(|_| DriverTransportError("helper pipe closed".into()))?;
        // Keep stdin open through the turn: EOF is cancellation in protocol v1.
        let reader = BufReader::new(stdout);
        Ok(Box::pin(stream::unfold(
            (reader, Some((running, stdin))),
            move |(mut reader, running)| {
                let id = request.id.clone();
                async move {
                    let running = running?;
                    let mut line = Vec::new();
                    // Bound each frame before allocating an unbounded helper line.
                    let read = tokio::time::timeout_at(
                        deadline,
                        tokio::io::AsyncBufReadExt::read_until(
                            &mut tokio::io::AsyncReadExt::take(&mut reader, 1_048_577),
                            b'\n',
                            &mut line,
                        ),
                    )
                    .await;
                    let event = match read {
                        Ok(Ok(n)) if n > 0 && n <= 1_048_576 && line.last() == Some(&b'\n') => {
                            serde_json::from_slice::<DriverEvent>(&line)
                                .map_err(|_| DriverTransportError("malformed helper event".into()))
                        }
                        _ => Err(DriverTransportError(
                            "helper EOF, deadline, or frame limit".into(),
                        )),
                    }
                    .and_then(|event| {
                        if event.id != id || event.version != 1 {
                            Err(DriverTransportError(
                                "helper event correlation failed".into(),
                            ))
                        } else {
                            Ok(event)
                        }
                    });
                    let terminal = !matches!(&event, Ok(DriverEvent { event: Event::TextDelta { text }, .. }) if !text.is_empty());
                    Some((
                        event,
                        (
                            reader,
                            if terminal {
                                drop(running);
                                None
                            } else {
                                Some(running)
                            },
                        ),
                    ))
                }
            },
        )))
    }
}

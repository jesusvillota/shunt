#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Test-only harnesses shared by inline test modules.

use std::{cell::RefCell, io, sync::Once};

thread_local! {
    static LOG_OUTPUT: RefCell<Option<Vec<u8>>> = const { RefCell::new(None) };
}

struct LogWriter;

impl io::Write for LogWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let _ = LOG_OUTPUT.try_with(|output| {
            if let Some(buffer) = output.borrow_mut().as_mut() {
                buffer.extend_from_slice(bytes);
            }
        });
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct LogCapture(Option<Vec<u8>>);

impl Drop for LogCapture {
    fn drop(&mut self) {
        LOG_OUTPUT.with(|output| output.replace(self.0.take()));
    }
}

/// Capture this thread's logs, restoring an outer capture even after a panic.
pub(crate) fn capture_logs<T>(run: impl FnOnce() -> T) -> (T, String) {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        // A scoped subscriber can cache "disabled" when another thread first uses a callsite.
        let subscriber = tracing_subscriber::fmt()
            .with_writer(|| LogWriter)
            .with_ansi(false)
            .without_time()
            .finish();
        tracing::subscriber::set_global_default(subscriber).expect("install test log subscriber");
        tracing::callsite::rebuild_interest_cache();
    });

    let _capture = LogCapture(LOG_OUTPUT.with(|output| output.replace(Some(Vec::new()))));
    let result = run();
    let logs = LOG_OUTPUT.with(|output| output.borrow_mut().take().unwrap());
    (result, String::from_utf8(logs).unwrap())
}

#[test]
fn capture_logs_keeps_parallel_threads_separate() {
    let barrier = std::sync::Barrier::new(2);
    std::thread::scope(|scope| {
        let captures: Vec<_> = ["first thread", "second thread"]
            .into_iter()
            .map(|message| {
                let barrier = &barrier;
                scope.spawn(move || {
                    capture_logs(|| {
                        barrier.wait();
                        tracing::warn!("{message}");
                    })
                    .1
                })
            })
            .collect();
        let logs: Vec<_> = captures.into_iter().map(|c| c.join().unwrap()).collect();
        assert!(logs[0].contains("first thread"), "{}", logs[0]);
        assert!(!logs[0].contains("second thread"), "{}", logs[0]);
        assert!(logs[1].contains("second thread"), "{}", logs[1]);
        assert!(!logs[1].contains("first thread"), "{}", logs[1]);
    });
}

#[test]
fn capture_logs_restores_outer_capture_after_panic() {
    let (_, logs) = capture_logs(|| {
        tracing::warn!("before nested capture");
        let panic = std::panic::catch_unwind(|| {
            capture_logs(|| {
                tracing::warn!("nested warning");
                panic!("test capture cleanup");
            });
        });
        assert!(panic.is_err());
        tracing::warn!("after nested capture");
    });
    assert!(logs.contains("before nested capture"), "{logs}");
    assert!(logs.contains("after nested capture"), "{logs}");
    assert!(!logs.contains("nested warning"), "{logs}");
}

/// A raw one-shot HTTP responder for a terminal error status whose body
/// stalls part-way: the headers (and with them the client's send) resolve
/// immediately, and the declared content-length completes only when the test
/// releases the body. wiremock cannot express the split — its response delay
/// covers headers and body alike — so tests pinning post-header behaviour
/// (deferred body reads, header-arrival latency samples) drive this instead.
pub(crate) struct StalledBody {
    pub(crate) base_url: String,
    release: tokio::sync::oneshot::Sender<()>,
    task: tokio::task::JoinHandle<()>,
}

impl StalledBody {
    /// Serve exactly one request: `status`, then `prefix` bytes of the body,
    /// parked until [`StalledBody::release`] completes the declared length
    /// with `suffix`.
    pub(crate) async fn start(
        status: axum::http::StatusCode,
        prefix: &'static str,
        suffix: &'static str,
    ) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (release, released) = tokio::sync::oneshot::channel::<()>();
        let task = tokio::spawn(async move {
            use tokio::io::AsyncWriteExt;
            let (mut socket, _) = listener.accept().await.unwrap();
            socket
                .write_all(
                    format!(
                        "HTTP/1.1 {}\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n",
                        status.as_str(),
                        prefix.len() + suffix.len(),
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
            socket.write_all(prefix.as_bytes()).await.unwrap();
            let _ = released.await;
            socket.write_all(suffix.as_bytes()).await.unwrap();
        });
        Self {
            base_url: format!("http://{addr}"),
            release,
            task,
        }
    }

    /// Release the stalled body and wait for the response to complete.
    pub(crate) async fn release(self) {
        let _ = self.release.send(());
        let _ = self.task.await;
    }
}

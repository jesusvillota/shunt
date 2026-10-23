//! Shared by fake-driver tests and future real-helper tests. The caller prepares
//! the browser/page state for each case; this runner knows no fake-driver types.

use futures_util::StreamExt;
use shunt::chatgptweb::{
    Driver, DriverCommand, DriverRequest, ErrorCode, Event, Submission, PROTOCOL_VERSION,
};

#[derive(Debug)]
pub enum Expected {
    Answer(&'static str),
    Health,
    Error(ErrorCode, Submission, Option<u32>, &'static str),
}

/// A real driver's harness can call this unchanged with its own `Driver` client
/// and controlled page states. Wrap live invocations in an outer test deadline.
pub async fn assert_case<D: Driver + ?Sized>(
    driver: &mut D,
    request: DriverRequest,
    expected: Expected,
) {
    // Exercise the public JSON representation, not Rust-only request values.
    let request: DriverRequest =
        serde_json::from_slice(&serde_json::to_vec(&request).unwrap()).unwrap();
    let mut events = driver.request(request.clone()).await.unwrap();
    let mut text = String::new();
    let mut terminal = None;
    while let Some(event) = events.next().await {
        let event = event.unwrap();
        let event: shunt::chatgptweb::DriverEvent =
            serde_json::from_slice(&serde_json::to_vec(&event).unwrap()).unwrap();
        assert_eq!(event.version, PROTOCOL_VERSION);
        assert_eq!(event.id, request.id);
        assert!(terminal.is_none(), "event after terminal event");
        match event.event {
            Event::TextDelta { text: delta } => {
                assert!(matches!(request.command, DriverCommand::Generate { .. }));
                assert!(!delta.is_empty(), "empty text delta");
                text.push_str(&delta);
            }
            Event::Done => {
                assert!(matches!(request.command, DriverCommand::Generate { .. }));
                terminal = Some(Event::Done);
            }
            Event::Health => {
                assert_eq!(request.command, DriverCommand::Health);
                terminal = Some(Event::Health);
            }
            Event::Error { error } => {
                assert!(!error.message.trim().is_empty());
                if !text.is_empty() {
                    assert_eq!(error.submission, Submission::Sent);
                }
                if matches!(request.command, DriverCommand::Health) {
                    assert_eq!(error.submission, Submission::NotSent);
                }
                if error.retry_after_ms.is_some() {
                    assert_eq!(error.code, ErrorCode::RateLimited);
                }
                terminal = Some(Event::Error { error });
            }
        }
    }
    let terminal = terminal.expect("stream ended without a terminal event");
    match expected {
        Expected::Answer(answer) => {
            assert_eq!(terminal, Event::Done);
            assert_eq!(text, answer);
        }
        Expected::Health => {
            assert_eq!(terminal, Event::Health);
            assert!(text.is_empty());
        }
        Expected::Error(code, submission, retry_after_ms, partial) => {
            let Event::Error { error } = terminal else {
                panic!("expected an error, got {terminal:?}");
            };
            assert_eq!(error.code, code);
            assert_eq!(error.submission, submission);
            assert_eq!(error.retry_after_ms, retry_after_ms);
            assert_eq!(text, partial);
        }
    }
}

#[path = "common/chatgptweb_conformance.rs"]
mod conformance;

use conformance::{assert_case, Expected};
use futures_util::StreamExt;
use shunt::chatgptweb::{
    fake::{FakeDriver, Fixture},
    Driver, DriverCommand, DriverError, DriverEvent, DriverRequest, ErrorCode, Event, Submission,
    PROTOCOL_VERSION,
};

const ACCOUNT: &str = "fixture-account";
const PROFILE: &str = "/fixtures/chatgptweb-profile";

fn request(command: DriverCommand) -> DriverRequest {
    DriverRequest {
        version: PROTOCOL_VERSION,
        id: "turn-1".into(),
        account: ACCOUNT.into(),
        profile_dir: PROFILE.into(),
        timeout_ms: 30_000,
        command,
    }
}

fn generate(prompt: &str) -> DriverRequest {
    request(DriverCommand::Generate {
        prompt: prompt.into(),
        model: Some("fixture-model".into()),
    })
}

fn error(code: ErrorCode, submission: Submission) -> DriverError {
    DriverError {
        code,
        message: format!("fixture {code:?}"),
        submission,
        retry_after_ms: (code == ErrorCode::RateLimited).then_some(60_000),
    }
}

fn fake(fixtures: Vec<Fixture>) -> FakeDriver {
    FakeDriver::new(ACCOUNT.into(), PROFILE.into(), fixtures)
}

fn fixture(prompt: &str, deltas: &[&str], error: Option<DriverError>) -> Fixture {
    Fixture {
        prompt: prompt.into(),
        model: "fixture-model".into(),
        deltas: deltas.iter().map(|text| (*text).into()).collect(),
        error,
    }
}

#[tokio::test]
async fn chatgptweb_fake_conforms_success_health_and_repeated_turns() {
    let mut driver = fake(vec![
        fixture("hello\nworld", &["Hello", " 🌍\n", "你好"], None),
        fixture("empty", &[], None),
    ]);
    assert_case(
        &mut driver,
        request(DriverCommand::Health),
        Expected::Health,
    )
    .await;
    for id in ["first", "second"] {
        let mut turn = generate("hello\nworld");
        turn.id = id.into();
        assert_case(&mut driver, turn, Expected::Answer("Hello 🌍\n你好")).await;
    }
    assert_case(&mut driver, generate("empty"), Expected::Answer("")).await;
    // Delta boundaries are preserved; the fake does not combine the answer.
    let events: Vec<_> = driver
        .request(generate("hello\nworld"))
        .await
        .unwrap()
        .map(|event| event.unwrap().event)
        .collect()
        .await;
    assert_eq!(
        events,
        vec![
            Event::TextDelta {
                text: "Hello".into()
            },
            Event::TextDelta {
                text: " 🌍\n".into()
            },
            Event::TextDelta {
                text: "你好".into()
            },
            Event::Done,
        ]
    );
}

#[tokio::test]
async fn chatgptweb_fake_conforms_pre_send_failures_for_generate_and_health() {
    for code in [
        ErrorCode::NotChatMode,
        ErrorCode::LoggedOut,
        ErrorCode::UiDrift,
        ErrorCode::RateLimited,
        ErrorCode::Timeout,
        ErrorCode::InternalError,
    ] {
        let failure = error(code, Submission::NotSent);
        let mut driver = fake(vec![]).with_readiness_error(failure.clone());
        for turn in [generate("hello"), request(DriverCommand::Health)] {
            assert_case(
                &mut driver,
                turn,
                Expected::Error(code, Submission::NotSent, failure.retry_after_ms, ""),
            )
            .await;
        }
    }
}

#[tokio::test]
async fn chatgptweb_fake_conforms_partial_and_uncertain_failures() {
    for code in [
        ErrorCode::UiDrift,
        ErrorCode::RateLimited,
        ErrorCode::Timeout,
        ErrorCode::InternalError,
    ] {
        for (submission, deltas, partial) in [
            (Submission::MayHaveBeenSent, vec![], ""),
            (
                Submission::Sent,
                vec!["partial", " answer"],
                "partial answer",
            ),
        ] {
            let failure = error(code, submission);
            let mut driver = fake(vec![fixture("hello", &deltas, Some(failure.clone()))]);
            assert_case(
                &mut driver,
                generate("hello"),
                Expected::Error(code, submission, failure.retry_after_ms, partial),
            )
            .await;
        }
    }
}

#[tokio::test]
async fn chatgptweb_fake_conforms_request_validation_and_account_isolation() {
    let valid = generate("hello");
    let mut cases = Vec::new();
    let mut turn = valid.clone();
    turn.version = 2;
    cases.push((turn, ErrorCode::UnsupportedVersion));
    let mut turn = valid.clone();
    turn.account = "other-account".into();
    cases.push((turn, ErrorCode::AccountMismatch));
    let mut turn = valid.clone();
    turn.profile_dir = "/other-profile".into();
    cases.push((turn, ErrorCode::AccountMismatch));
    for field in [
        "id",
        "account",
        "profile_dir",
        "timeout_ms",
        "prompt",
        "model",
    ] {
        let mut value = serde_json::to_value(&valid).unwrap();
        value[field] = if field == "timeout_ms" {
            0.into()
        } else {
            "".into()
        };
        cases.push((
            serde_json::from_value(value).unwrap(),
            ErrorCode::InvalidRequest,
        ));
    }
    let mut turn = valid;
    turn.profile_dir = "relative/profile".into();
    cases.push((turn, ErrorCode::InvalidRequest));
    let mut driver = fake(vec![fixture("hello", &["ok"], None)]);
    for (turn, code) in cases {
        assert_case(
            &mut driver,
            turn,
            Expected::Error(code, Submission::NotSent, None, ""),
        )
        .await;
    }
    // Rejected requests must not consume the next fixture or poison the helper.
    assert_case(&mut driver, generate("hello"), Expected::Answer("ok")).await;
    assert_case(
        &mut driver,
        generate("unmatched"),
        Expected::Error(ErrorCode::InvalidRequest, Submission::NotSent, None, ""),
    )
    .await;
}

#[test]
fn chatgptweb_documented_json_examples_match_the_wire_types() {
    let doc = include_str!("../docs/chatgptweb-driver-protocol.md");
    let mut request_types = std::collections::BTreeSet::new();
    let mut event_types = std::collections::BTreeSet::new();
    let mut codes = std::collections::BTreeSet::new();
    for block in doc.split("```json\n").skip(1) {
        let json = block.split("```").next().unwrap();
        let value: serde_json::Value = serde_json::from_str(json).unwrap();
        if value.get("account").is_some() {
            let request: DriverRequest = serde_json::from_value(value.clone()).unwrap();
            assert_eq!(serde_json::to_value(request).unwrap(), value);
            request_types.insert(value["type"].as_str().unwrap().to_owned());
        } else {
            let event: DriverEvent = serde_json::from_value(value.clone()).unwrap();
            assert_eq!(serde_json::to_value(event).unwrap(), value);
            event_types.insert(value["type"].as_str().unwrap().to_owned());
            if let Some(code) = value["error"]["code"].as_str() {
                codes.insert(code.to_owned());
            }
        }
    }
    assert_eq!(
        request_types.into_iter().collect::<Vec<_>>(),
        ["generate", "health"]
    );
    assert_eq!(
        event_types.into_iter().collect::<Vec<_>>(),
        ["done", "error", "health", "text_delta"]
    );
    assert_eq!(
        codes.into_iter().collect::<Vec<_>>(),
        [
            "account_mismatch",
            "internal_error",
            "invalid_request",
            "logged_out",
            "not_chat_mode",
            "rate_limited",
            "timeout",
            "ui_drift",
            "unsupported_version",
        ]
    );
}

#[test]
fn chatgptweb_wire_rejects_unknown_or_malformed_messages() {
    let valid = serde_json::to_value(generate("hello")).unwrap();
    for field in [
        "version",
        "id",
        "account",
        "profile_dir",
        "timeout_ms",
        "type",
        "prompt",
    ] {
        let mut value = valid.clone();
        value.as_object_mut().unwrap().remove(field);
        assert!(
            serde_json::from_value::<DriverRequest>(value).is_err(),
            "{field}"
        );
    }
    let mut optional = valid.clone();
    optional.as_object_mut().unwrap().remove("model");
    assert!(matches!(
        serde_json::from_value::<DriverRequest>(optional)
            .unwrap()
            .command,
        DriverCommand::Generate { model: None, .. }
    ));
    for bad in [
        r#"{"version":1,"id":"a","type":"unknown"}"#,
        r#"{"version":1,"id":"a","type":"text_delta"}"#,
        r#"{"version":1,"id":"a","type":"error","error":{"code":"unknown","message":"x","submission":"not_sent"}}"#,
        r#"{"version":1,"id":"a","type":"error","error":{"code":"timeout","message":"x","submission":"unknown"}}"#,
    ] {
        assert!(serde_json::from_str::<DriverEvent>(bad).is_err());
    }
}

#[tokio::test]
async fn chatgptweb_conformance_rejects_broken_driver_streams() {
    use futures_util::FutureExt;
    use shunt::chatgptweb::{DriverStream, DriverTransportError};

    struct BrokenDriver(Vec<DriverEvent>);

    #[async_trait::async_trait]
    impl Driver for BrokenDriver {
        async fn request(
            &mut self,
            _request: DriverRequest,
        ) -> Result<DriverStream, DriverTransportError> {
            Ok(Box::pin(futures_util::stream::iter(
                std::mem::take(&mut self.0).into_iter().map(Ok),
            )))
        }
    }

    let event = |event| DriverEvent {
        version: PROTOCOL_VERSION,
        id: "turn-1".into(),
        event,
    };
    let done = event(Event::Done);
    let delta = event(Event::TextDelta { text: "x".into() });
    let mut wrong_id = done.clone();
    wrong_id.id = "other-turn".into();
    let mut wrong_version = done.clone();
    wrong_version.version = 2;
    let mut invalid_error = error(ErrorCode::Timeout, Submission::NotSent);
    invalid_error.retry_after_ms = Some(1);
    for events in [
        vec![],
        vec![delta.clone()],
        vec![done.clone(), done.clone()],
        vec![done.clone(), delta.clone()],
        vec![wrong_id],
        vec![wrong_version],
        vec![event(Event::Health)],
        vec![event(Event::TextDelta { text: "".into() }), done],
        vec![
            delta,
            event(Event::Error {
                error: error(ErrorCode::Timeout, Submission::NotSent),
            }),
        ],
        vec![event(Event::Error {
            error: invalid_error,
        })],
    ] {
        let mut driver = BrokenDriver(events);
        let result = std::panic::AssertUnwindSafe(assert_case(
            &mut driver,
            generate("hello"),
            Expected::Answer(""),
        ))
        .catch_unwind()
        .await;
        assert!(result.is_err(), "conformance accepted a broken stream");
    }
}

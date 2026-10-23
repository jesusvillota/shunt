//! Deterministic in-memory fixture replay. No files, processes, clocks or sockets.

use async_trait::async_trait;
use futures_util::stream;

use super::{
    Driver, DriverCommand, DriverError, DriverEvent, DriverRequest, DriverStream,
    DriverTransportError, ErrorCode, Event, Submission, PROTOCOL_VERSION,
};

#[derive(Debug, Clone)]
pub struct Fixture {
    pub prompt: String,
    pub model: String,
    pub deltas: Vec<String>,
    /// `None` completes successfully; `Some` fails after the deltas.
    pub error: Option<DriverError>,
}

/// Fixtures match exact prompt/model pairs and can be replayed more than once.
/// `readiness_error` simulates a failed pre-send check on every request,
/// including health. It must describe a `not_sent` failure.
pub struct FakeDriver {
    account: String,
    profile_dir: String,
    fixtures: Vec<Fixture>,
    readiness_error: Option<DriverError>,
}

impl FakeDriver {
    pub fn new(account: String, profile_dir: String, fixtures: Vec<Fixture>) -> Self {
        Self {
            account,
            profile_dir,
            fixtures,
            readiness_error: None,
        }
    }

    pub fn with_readiness_error(mut self, error: DriverError) -> Self {
        assert_eq!(error.submission, Submission::NotSent);
        self.readiness_error = Some(error);
        self
    }

    fn reject(code: ErrorCode, message: &str) -> Vec<Event> {
        vec![Event::Error {
            error: DriverError {
                code,
                message: message.into(),
                submission: Submission::NotSent,
                retry_after_ms: None,
            },
        }]
    }

    fn replay(&self, request: &DriverRequest) -> Vec<Event> {
        if request.version != PROTOCOL_VERSION {
            return Self::reject(ErrorCode::UnsupportedVersion, "expected protocol version 1");
        }
        if request.id.trim().is_empty()
            || request.account.trim().is_empty()
            || !std::path::Path::new(&request.profile_dir).is_absolute()
            || request.timeout_ms == 0
        {
            return Self::reject(ErrorCode::InvalidRequest, "invalid request fields");
        }
        if request.account != self.account || request.profile_dir != self.profile_dir {
            return Self::reject(
                ErrorCode::AccountMismatch,
                "helper account/profile mismatch",
            );
        }
        if let DriverCommand::Generate { prompt, model } = &request.command {
            if prompt.trim().is_empty() || model.trim().is_empty() {
                return Self::reject(ErrorCode::InvalidRequest, "empty prompt or model");
            }
        }
        if let Some(error) = &self.readiness_error {
            return vec![Event::Error {
                error: error.clone(),
            }];
        }
        match &request.command {
            DriverCommand::Health => vec![Event::Health],
            DriverCommand::Generate { prompt, model } => {
                let Some(fixture) = self
                    .fixtures
                    .iter()
                    .find(|fixture| fixture.prompt == *prompt && fixture.model == *model)
                else {
                    return Self::reject(ErrorCode::InvalidRequest, "no matching fixture");
                };
                let mut events: Vec<_> = fixture
                    .deltas
                    .iter()
                    .map(|text| Event::TextDelta { text: text.clone() })
                    .collect();
                events.push(match &fixture.error {
                    Some(error) => Event::Error {
                        error: error.clone(),
                    },
                    None => Event::Done,
                });
                events
            }
        }
    }
}

#[async_trait]
impl Driver for FakeDriver {
    async fn request(
        &mut self,
        request: DriverRequest,
    ) -> Result<DriverStream, DriverTransportError> {
        let events = self.replay(&request).into_iter().map(move |event| {
            Ok(DriverEvent {
                version: PROTOCOL_VERSION,
                id: request.id.clone(),
                event,
            })
        });
        Ok(Box::pin(stream::iter(events)))
    }
}

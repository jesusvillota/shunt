//! Browser-helper contract for the `chatgptweb_browser` provider.
//!
//! See `docs/chatgptweb-driver-protocol.md` and `crate::adapters::chatgptweb`.

pub mod fake;

use std::pin::Pin;

use async_trait::async_trait;
use futures_util::Stream;
use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriverRequest {
    pub version: u32,
    pub id: String,
    pub account: String,
    pub profile_dir: String,
    pub timeout_ms: u32,
    #[serde(flatten)]
    pub command: DriverCommand,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DriverCommand {
    Generate {
        prompt: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        model: Option<String>,
    },
    Health,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriverEvent {
    pub version: u32,
    pub id: String,
    #[serde(flatten)]
    pub event: Event,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    TextDelta { text: String },
    Done,
    Health,
    Error { error: DriverError },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriverError {
    pub code: ErrorCode,
    pub message: String,
    pub submission: Submission,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_after_ms: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    NotChatMode,
    LoggedOut,
    UiDrift,
    RateLimited,
    Timeout,
    InvalidRequest,
    UnsupportedVersion,
    AccountMismatch,
    InternalError,
}

/// Evidence about physical Send, independent of whether text has arrived.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Submission {
    NotSent,
    MayHaveBeenSent,
    Sent,
}

/// A broken pipe, malformed JSON, unknown event/code, or premature EOF is a
/// transport failure, never a fabricated successful completion. Submission is
/// uncertain after writing a generate request unless the helper proves otherwise.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct DriverTransportError(pub String);

pub type DriverStream =
    Pin<Box<dyn Stream<Item = Result<DriverEvent, DriverTransportError>> + Send>>;

/// One helper is bound to one account/profile. Callers serialize requests and
/// drain each stream to its terminal event before reuse. Dropping an unfinished
/// stream must terminate and reap a real helper and its browser descendants.
#[async_trait]
pub trait Driver: Send {
    async fn request(
        &mut self,
        request: DriverRequest,
    ) -> Result<DriverStream, DriverTransportError>;
}

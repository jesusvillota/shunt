//! Browser transport for the inbound Responses endpoint.
mod process;
pub use process::terminate_all;
mod tool_protocol;
mod translate;

use super::{AdapterError, AdapterResult};
use crate::{
    chatgptweb::{Driver, DriverCommand, DriverRequest, Event, PROTOCOL_VERSION},
    error::ShuntError,
    routing::Route,
    server::AppState,
};
use axum::{
    body::{Body, Bytes},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use futures_util::{stream, StreamExt};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::Mutex;

type SharedDriver = Arc<Mutex<Box<dyn Driver>>>;

/// Process-lifetime helper handles. Each account holds its lock until its body
/// completes or is dropped. Tests install the TASK-002 fake at this same seam.
#[derive(Default)]
pub struct Drivers(Mutex<HashMap<String, SharedDriver>>);
impl Drivers {
    pub async fn insert(&self, profile_dir: String, driver: Box<dyn Driver>) {
        self.0
            .lock()
            .await
            .insert(profile_dir, Arc::new(Mutex::new(driver)));
    }
    async fn get(&self, profile_dir: &str) -> SharedDriver {
        self.0
            .lock()
            .await
            .entry(profile_dir.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(Box::new(process::ProcessDriver))))
            .clone()
    }
}

pub(crate) fn error(status: StatusCode, message: &str) -> AdapterError {
    AdapterError {
        message: message.into(),
        response: Box::new(
            ShuntError::new(
                status,
                if status == StatusCode::BAD_REQUEST {
                    "invalid_request_error"
                } else {
                    "api_error"
                },
                message.to_string(),
            )
            .into_response(),
        ),
        // No route or account retries: a browser may already have accepted Send.
        failure: None,
    }
}

pub(crate) async fn forward(state: AppState, route: Route, body: Bytes) -> AdapterResult {
    let request: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|_| error(StatusCode::BAD_REQUEST, "invalid Responses JSON"))?;
    let id = format!("resp_{}", uuid::Uuid::new_v4().simple());
    let contract = tool_protocol::Contract::new(&request, &id)
        .map_err(|message| error(StatusCode::BAD_REQUEST, message))?;
    let has_tools = request
        .get("tools")
        .is_some_and(|v| v.as_array().is_some_and(|a| !a.is_empty()))
        || request["input"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item["type"] == "additional_tools"));
    let contract = has_tools.then_some(contract);
    let prompt = match &contract {
        Some(contract) => contract.prompt().to_string(),
        None => translate::prompt(&request)
            .map_err(|message| error(StatusCode::BAD_REQUEST, message))?,
    };
    let streaming = request
        .get("stream")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let account = &state
        .config
        .provider(&route.provider)
        .expect("validated provider")
        .accounts[0];
    if account.disabled {
        return Err(error(
            StatusCode::SERVICE_UNAVAILABLE,
            "ChatGPT browser account is disabled",
        ));
    }
    let profile = account.profile_dir.as_ref().expect("validated profile");
    let driver = state.chatgptweb_drivers.get(profile).await;
    let guard = driver.lock_owned().await;
    let timeout_ms = state
        .config
        .server
        .timeouts
        .upstream_ttfb_ms
        .clamp(1, u32::MAX as u64) as u32;
    let driver_request = DriverRequest {
        version: PROTOCOL_VERSION,
        id: id.clone(),
        account: account.name.clone(),
        profile_dir: profile.clone(),
        timeout_ms,
        command: DriverCommand::Generate {
            prompt,
            model: state
                .config
                .server
                .codex_endpoint
                .as_ref()
                .and_then(|endpoint| endpoint.route_for(&route.model))
                .and_then(|route| route.upstream_model.clone()),
        },
    };
    let deadline = tokio::time::Instant::now() + Duration::from_millis(timeout_ms as u64);
    let mut guard = guard;
    let events = tokio::time::timeout_at(deadline, guard.request(driver_request))
        .await
        .map_err(|_| {
            error(
                StatusCode::GATEWAY_TIMEOUT,
                "browser helper deadline exceeded",
            )
        })?
        .map_err(|_| error(StatusCode::BAD_GATEWAY, "browser helper could not start"))?;
    let mut output = translate::Output::new(id, route.model, contract, &request);
    let prefix = output.start();
    let body_stream = stream::unfold(
        (events, guard, output, false),
        move |(mut events, guard, mut output, ended)| async move {
            if ended {
                return None;
            }
            let event = tokio::time::timeout_at(deadline, events.next()).await;
            let (frames, terminal) = match event {
                Ok(Some(Ok(event)))
                    if event.version == PROTOCOL_VERSION && event.id == output.id =>
                {
                    match event.event {
                        Event::TextDelta { text } if !text.is_empty() => match output.delta(text) {
                            Ok(frames) => (frames, false),
                            Err(_) => (output.fail("invalid_tool_output"), true),
                        },
                        Event::Done => (
                            output
                                .finish()
                                .unwrap_or_else(|_| output.fail("invalid_tool_output")),
                            true,
                        ),
                        Event::Error { error } => (
                            output
                                .fail(serde_json::to_value(error.code).unwrap().as_str().unwrap()),
                            true,
                        ),
                        _ => (output.fail("invalid_driver_event"), true),
                    }
                }
                Err(_) => (output.fail("timeout"), true),
                _ => (output.fail("driver_transport_error"), true),
            };
            Some((
                Ok::<_, std::io::Error>(Bytes::from(frames)),
                (events, guard, output, terminal),
            ))
        },
    );
    if streaming {
        let stream = stream::once(async move { Ok::<_, std::io::Error>(Bytes::from(prefix)) })
            .chain(body_stream);
        let response = Response::builder()
            .header("content-type", "text/event-stream")
            .header("cache-control", "no-cache")
            .header("x-shunt-account", &account.name)
            .body(Body::from_stream(stream))
            .unwrap();
        Ok((StatusCode::OK, response))
    } else {
        // Reuse the stream's terminal snapshot for non-streaming requests only.
        let mut body_stream = Box::pin(body_stream);
        let mut terminal = None;
        while let Some(chunk) = body_stream.next().await {
            let chunk =
                chunk.map_err(|_| error(StatusCode::BAD_GATEWAY, "browser stream failed"))?;
            for line in std::str::from_utf8(&chunk)
                .unwrap()
                .lines()
                .filter_map(|line| line.strip_prefix("data: "))
            {
                let value: serde_json::Value = serde_json::from_str(line).unwrap();
                if value["type"] == "response.completed" || value["type"] == "response.failed" {
                    terminal = Some(value);
                }
            }
        }
        let terminal = terminal.expect("stream always ends with a terminal event");
        if terminal["type"] == "response.failed" {
            return Err(error(
                StatusCode::BAD_GATEWAY,
                "browser helper failed; request was not replayed",
            ));
        }
        Ok((
            StatusCode::OK,
            axum::Json(terminal["response"].clone()).into_response(),
        ))
    }
}

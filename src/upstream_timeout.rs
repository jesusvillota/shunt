use std::{future::Future, time::Duration};

use axum::{http::StatusCode, response::IntoResponse};

use crate::{adapters::AdapterError, error::ShuntError, retry::RetryableError};

#[derive(Debug)]
pub(crate) enum SendError<E> {
    Transport(E),
    Timeout { model: String, limit_ms: u64 },
}

impl<E: std::fmt::Display> std::fmt::Display for SendError<E> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Transport(error) => error.fmt(formatter),
            Self::Timeout { model, limit_ms } => write!(
                formatter,
                "upstream model {model} timed out before response headers (limit: {limit_ms} ms)"
            ),
        }
    }
}

impl<E: RetryableError> RetryableError for SendError<E> {
    fn log_message(&self) -> String {
        match self {
            Self::Transport(error) => error.log_message(),
            Self::Timeout { .. } => self.to_string(),
        }
    }

    fn is_transient(&self) -> bool {
        match self {
            Self::Transport(error) => error.is_transient(),
            // TTFB timeout is not currently retried. Keeping its distinct type
            // makes any future replay decision explicit rather than accidental.
            Self::Timeout { .. } => false,
        }
    }
}

impl<E: std::fmt::Display> SendError<E> {
    pub(crate) fn into_adapter_error(
        self,
        transport: impl FnOnce(E) -> AdapterError,
    ) -> AdapterError {
        match self {
            Self::Transport(error) => transport(error),
            Self::Timeout { .. } => AdapterError {
                message: self.to_string(),
                response: Box::new(
                    ShuntError::new(
                        StatusCode::GATEWAY_TIMEOUT,
                        "timeout_error",
                        self.to_string(),
                    )
                    .into_response(),
                ),
                failure: None,
            },
        }
    }
}

/// Bound only the future that obtains upstream response headers. `send()`
/// resolves when headers arrive, so reading or streaming the returned body has
/// no wall-clock cap. A zero value disables the bound.
pub(crate) async fn wait<F, T, E>(
    model: &str,
    upstream_ttfb_ms: u64,
    future: F,
) -> Result<T, SendError<E>>
where
    F: Future<Output = Result<T, E>>,
{
    if upstream_ttfb_ms == 0 {
        return future.await.map_err(SendError::Transport);
    }
    match tokio::time::timeout(Duration::from_millis(upstream_ttfb_ms), future).await {
        Ok(result) => result.map_err(SendError::Transport),
        Err(_) => Err(SendError::Timeout {
            model: model.to_owned(),
            limit_ms: upstream_ttfb_ms,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_is_not_a_retryable_transport_failure() {
        let error = SendError::<reqwest::Error>::Timeout {
            model: "slow-model".into(),
            limit_ms: 25,
        };
        assert!(!error.is_transient());
    }

    #[tokio::test(start_paused = true)]
    async fn slow_headers_return_gateway_timeout() {
        let task = tokio::spawn(wait::<_, (), std::convert::Infallible>(
            "slow-model",
            25,
            async {
                tokio::time::sleep(Duration::from_secs(1)).await;
                Ok(())
            },
        ));
        tokio::time::advance(Duration::from_millis(25)).await;
        let error = task
            .await
            .unwrap()
            .unwrap_err()
            .into_adapter_error(|error| match error {});
        assert_eq!(error.response.status(), StatusCode::GATEWAY_TIMEOUT);
        let body = axum::body::to_bytes(error.response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["error"]["type"], "timeout_error");
        let message = body["error"]["message"].as_str().unwrap();
        assert!(message.contains("slow-model"), "{message}");
        assert!(message.contains("25 ms"), "{message}");
    }

    #[tokio::test(start_paused = true)]
    async fn slow_response_body_is_not_subject_to_the_timeout() {
        use axum::body::{to_bytes, Body, Bytes};
        use futures_util::stream;

        let body = wait::<_, _, std::convert::Infallible>("slow-model", 25, async {
            Ok(Body::from_stream(stream::once(async {
                tokio::time::sleep(Duration::from_secs(10)).await;
                Ok::<_, std::convert::Infallible>(Bytes::from_static(b"done"))
            })))
        })
        .await
        .unwrap();
        let read = tokio::spawn(to_bytes(body, usize::MAX));
        tokio::time::advance(Duration::from_secs(10)).await;
        assert_eq!(read.await.unwrap().unwrap(), "done");
    }
}

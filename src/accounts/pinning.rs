//! `[server.auth.clients]` account pinning: a pinned client draws only from
//! its configured subset of a provider's pool.

use axum::response::IntoResponse;
use reqwest::{header::HeaderMap, StatusCode};

use super::ReprobeReservation;
use crate::{
    adapters::{AdapterError, AdapterFailure},
    config::{AccountConfig, Config},
    error::ShuntError,
};

/// The verified inbound client name. `check_inbound_auth` strips any
/// caller-sent copy of this header and `headers_for_route` stamps the
/// authenticated name, so adapters can trust it.
pub(crate) fn inbound_client(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("x-shunt-inbound-client")
        .and_then(|value| value.to_str().ok())
}

/// Drop from `order` every account `client` may not use on `provider`.
///
/// Filters the selected order, not the `accounts` slice, because selection
/// also feeds the provider-wide pool state. Cooling accounts stay in the order,
/// so an exhausted subset still relays the upstream 429. A dropped account's
/// re-probe reservation is dropped too, which cancels it. When the client is
/// pinned and nothing is left, the error is a 503 that never spills over to
/// other accounts; a `[[routes]]` fallback provider is still tried.
pub(crate) fn retain_client_accounts(
    config: &Config,
    client: Option<&str>,
    provider: &str,
    accounts: &[AccountConfig],
    order: &mut Vec<usize>,
    reservation: &mut Option<ReprobeReservation>,
) -> Result<(), AdapterError> {
    let Some((client, allowed)) =
        client.and_then(|client| Some((client, config.client_accounts(client, provider)?)))
    else {
        return Ok(());
    };
    order.retain(|&index| allowed.contains(&accounts[index].name));
    if reservation
        .as_ref()
        .is_some_and(|reservation| !order.contains(&reservation.selected_index()))
    {
        *reservation = None;
    }
    if !order.is_empty() {
        return Ok(());
    }
    let message = format!(
        "client '{client}' is pinned to accounts [{}] of provider '{provider}', and none of them is selectable (disabled, paused, or not in the pool)",
        allowed.join(", ")
    );
    tracing::warn!(client, provider, "no pinned account is selectable");
    Err(AdapterError {
        response: Box::new(
            ShuntError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "api_error",
                message.clone(),
            )
            .into_response(),
        ),
        message,
        // Marked as a 503 so the failover chain tries a `[[routes]]` fallback
        // and, with none left, returns this response.
        failure: Some(AdapterFailure::UpstreamStatus(
            StatusCode::SERVICE_UNAVAILABLE,
        )),
    })
}

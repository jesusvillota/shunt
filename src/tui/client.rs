//! Admin-API client for the monitor. A header credential carries no ambient
//! cookie, so the admin surface exempts it from CSRF — the monitor needs only
//! the token, never a browser session.

use std::time::Duration;

use anyhow::{bail, Context};
use reqwest::StatusCode;
use serde_json::json;

use super::model::{PoolResponse, Snapshot};

#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    base: String,
    header: String,
    token: String,
}

impl Client {
    pub fn new(base: &str, header: &str, token: &str) -> anyhow::Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .context("building HTTP client")?;
        Ok(Self {
            http,
            base: base.trim_end_matches('/').to_string(),
            header: header.to_string(),
            token: token.to_string(),
        })
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    fn url(&self, path: &str) -> String {
        format!("{}/admin/api{path}", self.base)
    }

    pub async fn fetch_pool(&self) -> anyhow::Result<Snapshot> {
        let response = self
            .http
            .get(self.url("/pool"))
            .header(&self.header, &self.token)
            .send()
            .await
            .context("gateway unreachable")?;
        let body: PoolResponse = ok_json(response).await?;
        Ok(Snapshot::from_response(body))
    }

    pub async fn set_paused(
        &self,
        provider: &str,
        account_ref: &str,
        paused: bool,
    ) -> anyhow::Result<()> {
        // `account_ref` is opaque; encode it as a path segment rather than
        // assume it is URL-safe.
        let mut url = reqwest::Url::parse(&self.url("/pool"))?;
        url.path_segments_mut()
            .map_err(|()| anyhow::anyhow!("base URL cannot carry a path"))?
            .extend([provider, "accounts", account_ref]);
        let response = self
            .http
            .patch(url)
            .header(&self.header, &self.token)
            .json(&json!({ "paused": paused }))
            .send()
            .await
            .context("gateway unreachable")?;
        ok_empty(response).await
    }

    pub async fn set_sort_by_reset(&self, value: bool) -> anyhow::Result<()> {
        let response = self
            .http
            .patch(self.url("/pool"))
            .header(&self.header, &self.token)
            .json(&json!({ "sort_by_reset": value }))
            .send()
            .await
            .context("gateway unreachable")?;
        ok_empty(response).await
    }
}

async fn ok_empty(response: reqwest::Response) -> anyhow::Result<()> {
    check(response).await.map(|_| ())
}

async fn ok_json<T: serde::de::DeserializeOwned>(response: reqwest::Response) -> anyhow::Result<T> {
    let response = check(response).await?;
    response.json().await.context("unexpected pool response")
}

async fn check(response: reqwest::Response) -> anyhow::Result<reqwest::Response> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    match status {
        StatusCode::UNAUTHORIZED => bail!("admin token rejected (401)"),
        StatusCode::FORBIDDEN => {
            bail!("this admin token is read-only (403); a write key is needed")
        }
        StatusCode::NOT_FOUND => {
            bail!("not found (404): account is gone, or the gateway predates pause support")
        }
        _ => {
            let text = response.text().await.unwrap_or_default();
            let text = text.chars().take(160).collect::<String>();
            bail!("gateway answered {status}: {text}")
        }
    }
}

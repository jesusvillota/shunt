//! Read-only agy slash-command quota probe. One instance lives for the poller
//! lifetime, including reloads, so a model turn permanently disables CLI probes.

use std::{path::Path, process::Stdio, time::Duration};

use anyhow::{bail, Context};
use serde_json::Value;
use tokio::{io::AsyncReadExt, process::Command};

use crate::accounts::QuotaBucketSnapshot;

const OUTPUT_LIMIT: u64 = 1024 * 1024;
const HARD_TIMEOUT: Duration = Duration::from_secs(25);

pub(crate) struct UsageProbe {
    executable: std::ffi::OsString,
    version_supported: Option<bool>,
    latched_off: bool,
}

impl Default for UsageProbe {
    fn default() -> Self {
        Self {
            executable: "agy".into(),
            version_supported: None,
            latched_off: false,
        }
    }
}

impl UsageProbe {
    /// Never invoke agy without an explicit, existing account profile. Failures
    /// let the caller use HTTP; this code neither creates profiles nor logs in.
    pub(crate) async fn fetch(
        &mut self,
        profile: &str,
    ) -> anyhow::Result<Vec<QuotaBucketSnapshot>> {
        if self.latched_off {
            bail!("agy usage probe latched off after a model turn; restart required");
        }
        if profile.trim().is_empty() {
            bail!("agy usage requires an explicit account profile");
        }
        let profile = tokio::fs::canonicalize(profile)
            .await
            .context("agy account profile is unavailable")?;
        if !profile.is_dir() {
            bail!("agy account profile is not a directory");
        }
        if self.version_supported.is_none() {
            let version = run_bounded(self.command(&profile, &["--version"]), HARD_TIMEOUT)
                .await
                .ok()
                .and_then(|(bytes, success)| {
                    success.then(|| String::from_utf8(bytes).ok()).flatten()
                });
            self.version_supported = Some(version.as_deref().is_some_and(supported_version));
        }
        if self.version_supported != Some(true) {
            bail!("agy /usage requires version >= 1.1.11");
        }
        let (bytes, success) = run_bounded(
            self.command(
                &profile,
                &[
                    "-p",
                    "/usage",
                    "--output-format",
                    "json",
                    "--print-timeout",
                    "20s",
                ],
            ),
            HARD_TIMEOUT,
        )
        .await?;
        let reply: Value = serde_json::from_slice(&bytes).context("invalid agy usage JSON")?;
        // Inspect accounting before status/schema parsing. An error response
        // can still have started a model turn and must disable future probes.
        match reply.get("num_turns").and_then(Value::as_u64) {
            Some(0) => {}
            Some(_) => {
                self.latched_off = true;
                tracing::warn!(
                    "agy /usage started a model turn; CLI quota probes disabled until restart"
                );
                bail!("agy /usage started a model turn");
            }
            None => bail!("agy usage response has no valid num_turns accounting"),
        }
        if !success {
            bail!("agy usage probe exited unsuccessfully");
        }
        parse_reply(&reply)
    }

    fn command(&self, profile: &Path, args: &[&str]) -> Command {
        let mut command = Command::new(&self.executable);
        command
            .args(args)
            .env("HOME", profile)
            .env("USERPROFILE", profile)
            // A profile HOME has no ~/Library/Keychains, so agy's keychain write
            // makes macOS show a "Keychain Not Found" dialog on every token
            // refresh. agy skips the keychain for the token file when it sees an
            // SSH session ("Using file-based token storage because SSH session
            // detected"), which is the storage a profile already uses.
            .env("SSH_CONNECTION", "127.0.0.1 0 127.0.0.1 0")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        for key in [
            "GEMINI_API_KEY",
            "GOOGLE_API_KEY",
            "GOOGLE_APPLICATION_CREDENTIALS",
            "GOOGLE_CLOUD_PROJECT",
            "GOOGLE_CLOUD_LOCATION",
            "GOOGLE_CLOUD_QUOTA_PROJECT",
            "GOOGLE_GENAI_USE_VERTEXAI",
            "GCLOUD_PROJECT",
            "CLOUDSDK_CORE_PROJECT",
        ] {
            command.env_remove(key);
        }
        for (key, _) in std::env::vars_os() {
            let name = key.to_string_lossy();
            if name.starts_with("GOOGLE_")
                || name.starts_with("GEMINI_")
                || name.starts_with("CLOUDSDK_")
                || name == "GCLOUD_PROJECT"
            {
                command.env_remove(key);
            }
        }
        command
    }
}

fn supported_version(version: &str) -> bool {
    let Some(version) = version.split_whitespace().last() else {
        return false;
    };
    let parts = version
        .trim_start_matches('v')
        .split('.')
        .collect::<Vec<_>>();
    if parts.len() != 3 {
        return false;
    }
    let parsed = parts
        .iter()
        .map(|part| part.parse::<u64>())
        .collect::<Result<Vec<_>, _>>();
    parsed.is_ok_and(|parts| (parts[0], parts[1], parts[2]) >= (1, 1, 11))
}

async fn run_bounded(mut command: Command, timeout: Duration) -> anyhow::Result<(Vec<u8>, bool)> {
    let mut child = command.spawn().context("cannot start agy usage probe")?;
    let result = tokio::time::timeout(timeout, async {
        let stdout = child.stdout.take().context("agy stdout unavailable")?;
        let mut bytes = Vec::new();
        stdout
            .take(OUTPUT_LIMIT + 1)
            .read_to_end(&mut bytes)
            .await?;
        if bytes.len() as u64 > OUTPUT_LIMIT {
            bail!("agy usage output exceeds limit");
        }
        let status = child.wait().await?;
        if !status.success() {
            // Preserve JSON accounting even on nonzero exit, so a reported
            // model turn cannot bypass the latch by returning an error code.
            if serde_json::from_slice::<Value>(&bytes).is_err() {
                bail!("agy usage probe failed ({status})");
            }
        }
        Ok((bytes, status.success()))
    })
    .await;
    match result {
        Ok(Ok(output)) => Ok(output),
        other => {
            let _ = child.kill().await;
            match other {
                Ok(Err(error)) => Err(error),
                Err(_) => anyhow::bail!("agy usage probe timed out"),
                Ok(Ok(_)) => unreachable!(),
            }
        }
    }
}

fn parse_reply(reply: &Value) -> anyhow::Result<Vec<QuotaBucketSnapshot>> {
    if reply.get("status").and_then(Value::as_str) != Some("SUCCESS")
        || reply.pointer("/command/name").and_then(Value::as_str) != Some("usage")
    {
        bail!("agy response is not a successful /usage command");
    }
    let summary = reply
        .pointer("/command/data")
        .context("agy usage data missing")?;
    super::usage::parse_usage(serde_json::from_value(summary.clone())?)
}

#[cfg(test)]
#[path = "usage_cli_tests.rs"]
pub(crate) mod tests;

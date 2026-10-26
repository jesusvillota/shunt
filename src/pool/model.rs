//! Wire types for the read-only pool snapshot.
use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PoolResponse {
    #[serde(default)]
    pub providers: Vec<ProviderDto>,
    #[serde(default)]
    pub sort_by_reset: bool,
    #[serde(default)]
    pub sort_by_weekly_reset: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProviderDto {
    pub provider: String,
    /// The provider's auth kind (`claude_oauth`, `chatgpt_oauth`, …).
    #[serde(default)]
    pub auth: Option<String>,
    #[serde(default)]
    pub accounts: Vec<AccountDto>,
    /// This provider's own weekly-reset setting; a gateway that predates it
    /// omits it, and the pool-wide value applies.
    #[serde(default)]
    pub sort_by_weekly_reset: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AccountDto {
    pub name: String,
    pub account_ref: Option<String>,
    pub plan: Option<String>,
    pub disabled: bool,
    pub paused: bool,
    pub needs_relogin: bool,
    pub organization_blocked: bool,
    pub has_state: bool,
    pub near_quota: bool,
    pub priority: Option<u32>,
    pub headroom_secs: Option<i64>,
    pub cooldown_secs_remaining: Option<u64>,
    pub cooldown_fable_secs_remaining: Option<u64>,
    pub utilization_5h: Option<f64>,
    pub reset_5h: Option<u64>,
    pub utilization_7d: Option<f64>,
    pub reset_7d: Option<u64>,
    pub reset_7d_oi: Option<u64>,
    /// Upstream-reported family/window quotas; remaining is a fraction, not usage.
    pub quota_buckets: Vec<QuotaBucketDto>,
    /// Upstream attempts since the gateway started; absent (0) on a gateway
    /// that predates the counters.
    pub requests_attempted: u64,
    pub requests_succeeded: u64,
    pub requests_failed: u64,
    pub mean_latency_ms: Option<f64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct QuotaBucketDto {
    pub label: String,
    pub remaining: Option<f64>,
    pub reset_time: Option<String>,
}

impl QuotaBucketDto {
    pub fn reset_secs(&self) -> Option<u64> {
        self.reset_time
            .as_deref()
            .and_then(crate::auth::claude::usage::parse_rfc3339_to_epoch_secs)
    }
}

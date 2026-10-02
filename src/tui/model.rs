//! The pool snapshot the monitor renders: the wire shape of
//! `GET /admin/api/pool`, grouped by provider, with each provider's accounts
//! in the order the gateway would try them.
//!
//! Everything here is pure so the ordering and state ladder are unit-testable
//! without a terminal or a gateway.

use std::cmp::Ordering;

use serde::Deserialize;

use super::config_edit::DEFAULT_PRIORITY;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PoolResponse {
    #[serde(default)]
    pub providers: Vec<ProviderDto>,
    #[serde(default)]
    pub sort_by_reset: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProviderDto {
    pub provider: String,
    /// The provider's auth kind (`claude_oauth`, `chatgpt_oauth`, …).
    #[serde(default)]
    pub auth: Option<String>,
    #[serde(default)]
    pub accounts: Vec<AccountDto>,
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
    /// Upstream attempts since the gateway started; absent (0) on a gateway
    /// that predates the counters.
    pub requests_attempted: u64,
    pub requests_succeeded: u64,
    pub requests_failed: u64,
    pub mean_latency_ms: Option<f64>,
}

/// The one state an account displays, mirroring the dashboard's ladder
/// (`ui/src/accounts.ts::managedState`) with `Paused` added.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountState {
    Disabled,
    Paused,
    NeedsRelogin,
    Unseen,
    Cooling,
    NearQuota,
    CoolingFable,
    Available,
}

impl AccountState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Paused => "paused",
            Self::NeedsRelogin => "needs re-login",
            Self::Unseen => "unseen",
            Self::Cooling => "cooling down",
            Self::NearQuota => "near quota",
            Self::CoolingFable => "cooling (fable)",
            Self::Available => "available",
        }
    }

    /// Whether the gateway can pick this account for the next request.
    pub fn is_routable(self) -> bool {
        matches!(
            self,
            Self::Available | Self::CoolingFable | Self::NearQuota | Self::Unseen
        )
    }
}

fn state_of(a: &AccountDto) -> AccountState {
    if a.disabled {
        AccountState::Disabled
    } else if a.paused {
        AccountState::Paused
    } else if a.needs_relogin {
        AccountState::NeedsRelogin
    } else if !a.has_state {
        AccountState::Unseen
    } else if a.cooldown_secs_remaining.unwrap_or(0) > 0 {
        AccountState::Cooling
    } else if a.near_quota {
        AccountState::NearQuota
    } else if a.cooldown_fable_secs_remaining.unwrap_or(0) > 0 {
        AccountState::CoolingFable
    } else {
        AccountState::Available
    }
}

#[derive(Debug, Clone)]
pub struct Row {
    pub state: AccountState,
    pub account: AccountDto,
}

impl Row {
    pub fn priority(&self) -> u32 {
        self.account.priority.unwrap_or(DEFAULT_PRIORITY)
    }

    /// Soonest reset among the windows this account reports, epoch seconds.
    fn soonest_reset(&self) -> Option<u64> {
        [
            self.account.reset_5h,
            self.account.reset_7d,
            self.account.reset_7d_oi,
        ]
        .into_iter()
        .flatten()
        .filter(|reset| *reset > 0)
        .min()
    }

    /// The identity a mutation targets. `None` for a gateway too old to send
    /// `account_ref`.
    pub fn account_ref(&self) -> Option<&str> {
        self.account
            .account_ref
            .as_deref()
            .filter(|r| !r.is_empty())
    }

    /// The key an account is tracked by across polls.
    pub fn key(&self) -> &str {
        self.account_ref().unwrap_or(&self.account.name)
    }
}

/// How a provider's accounts are ordered for the next request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RankMode {
    /// Accounts share one priority tier; the gateway balances them by how much
    /// headroom each has left.
    Balanced,
    /// The operator's explicit 1 / 2 / 3 order, kept as account priorities.
    Custom,
}

impl RankMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Balanced => "balanced",
            Self::Custom => "custom order",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProviderView {
    pub name: String,
    pub auth: Option<String>,
    /// Accounts in the order the pool lists them.
    pub rows: Vec<Row>,
}

impl ProviderView {
    /// Balanced when every account shares one priority (or there is nothing to
    /// order); custom as soon as two accounts sit in different tiers.
    pub fn mode(&self) -> RankMode {
        let first = self.rows.first().map(Row::priority);
        if self.rows.iter().all(|row| Some(row.priority()) == first) {
            RankMode::Balanced
        } else {
            RankMode::Custom
        }
    }

    /// A provider is off when it has accounts and every account that could
    /// serve traffic is paused. Config-disabled accounts do not count: pausing
    /// cannot change them.
    pub fn is_on(&self) -> bool {
        let mut serving = self
            .rows
            .iter()
            .filter(|row| !row.account.disabled)
            .peekable();
        serving.peek().is_none() || serving.any(|row| !row.account.paused)
    }

    /// Whether the admin API can add an account to this provider.
    pub fn account_kind(&self) -> Option<&'static str> {
        match self.auth.as_deref() {
            Some("claude_oauth") => Some("claude"),
            Some("chatgpt_oauth") => Some("codex"),
            Some("antigravity_oauth") => Some("antigravity"),
            _ => None,
        }
    }

    /// Rows in the order the gateway would try them, each with its rank
    /// (`None` for an account that cannot take traffic right now).
    ///
    /// The gateway orders available accounts by priority, then (balanced) by the
    /// largest burn-rate headroom — or soonest reset when `sort_by_reset` is on —
    /// with near-quota accounts after the others. An account that is paused,
    /// cooling down, disabled or needing re-login is not tried. In custom mode
    /// every account keeps its place so the operator can still reorder it.
    pub fn ordered(&self, sort_by_reset: bool) -> Vec<(Option<usize>, &Row)> {
        let custom = self.mode() == RankMode::Custom;
        let mut rows: Vec<(usize, &Row)> = self.rows.iter().enumerate().collect();
        // `sort_by` is stable, so pool order is the final tiebreak and the
        // table does not shuffle between polls when nothing changed.
        rows.sort_by(|(_, a), (_, b)| {
            let routable = b.state.is_routable().cmp(&a.state.is_routable());
            if custom {
                a.priority()
                    .cmp(&b.priority())
                    .then_with(|| balanced_cmp(a, b, sort_by_reset))
            } else {
                routable.then_with(|| balanced_cmp(a, b, sort_by_reset))
            }
        });
        rows.into_iter()
            .enumerate()
            .map(|(position, (_, row))| {
                let ranked = custom || row.state.is_routable();
                (ranked.then_some(position + 1), row)
            })
            .collect()
    }
}

/// Best account first: not-near-quota before near-quota, then the most
/// headroom (no reading counts as plenty), or the soonest reset.
fn balanced_cmp(a: &Row, b: &Row, sort_by_reset: bool) -> Ordering {
    a.account
        .near_quota
        .cmp(&b.account.near_quota)
        .then_with(|| {
            if sort_by_reset {
                match (a.soonest_reset(), b.soonest_reset()) {
                    (Some(x), Some(y)) => x.cmp(&y),
                    (Some(_), None) => Ordering::Less,
                    (None, Some(_)) => Ordering::Greater,
                    (None, None) => Ordering::Equal,
                }
            } else {
                let headroom = |row: &Row| {
                    row.account
                        .headroom_secs
                        .map_or(f64::INFINITY, |h| h as f64)
                };
                headroom(b).total_cmp(&headroom(a))
            }
        })
}

#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub providers: Vec<ProviderView>,
    pub sort_by_reset: bool,
}

impl Snapshot {
    pub fn from_response(response: PoolResponse) -> Self {
        let providers = response
            .providers
            .into_iter()
            .map(|provider| ProviderView {
                name: provider.provider,
                auth: provider.auth,
                rows: provider
                    .accounts
                    .into_iter()
                    .map(|account| Row {
                        state: state_of(&account),
                        account,
                    })
                    .collect(),
            })
            .collect();
        Self {
            providers,
            sort_by_reset: response.sort_by_reset,
        }
    }

    pub fn provider(&self, name: &str) -> Option<&ProviderView> {
        self.providers.iter().find(|p| p.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn acct(name: &str) -> AccountDto {
        AccountDto {
            name: name.into(),
            account_ref: Some(format!("ref-{name}")),
            has_state: true,
            ..AccountDto::default()
        }
    }

    fn view(accounts: Vec<AccountDto>) -> ProviderView {
        Snapshot::from_response(PoolResponse {
            providers: vec![ProviderDto {
                provider: "claude".into(),
                auth: Some("claude_oauth".into()),
                accounts,
            }],
            sort_by_reset: false,
        })
        .providers
        .remove(0)
    }

    fn order(view: &ProviderView, by_reset: bool) -> Vec<(Option<usize>, String)> {
        view.ordered(by_reset)
            .into_iter()
            .map(|(rank, row)| (rank, row.account.name.clone()))
            .collect()
    }

    #[test]
    fn parses_the_wire_shape_tolerating_missing_fields() {
        let json = r#"{"sort_by_reset":true,"providers":[{"provider":"claude","auth":"claude_oauth",
            "accounts":[{"name":"a","account_ref":"r1","paused":true,"priority":3,"reset_5h":1700000000},
                        {"name":"b"}]}]}"#;
        let snap = Snapshot::from_response(serde_json::from_str(json).unwrap());
        assert!(snap.sort_by_reset);
        let p = snap.provider("claude").unwrap();
        assert_eq!(p.rows[0].state, AccountState::Paused);
        assert_eq!(p.rows[0].priority(), 3);
        assert_eq!(p.rows[1].state, AccountState::Unseen);
        assert_eq!(p.rows[1].priority(), DEFAULT_PRIORITY);
        assert_eq!(p.account_kind(), Some("claude"));
    }

    #[test]
    fn state_ladder_matches_the_dashboard() {
        let mut a = acct("x");
        assert_eq!(state_of(&a), AccountState::Available);
        a.cooldown_fable_secs_remaining = Some(5);
        assert_eq!(state_of(&a), AccountState::CoolingFable);
        a.near_quota = true;
        assert_eq!(state_of(&a), AccountState::NearQuota);
        a.cooldown_secs_remaining = Some(5);
        assert_eq!(state_of(&a), AccountState::Cooling);
        a.needs_relogin = true;
        assert_eq!(state_of(&a), AccountState::NeedsRelogin);
        a.paused = true;
        assert_eq!(state_of(&a), AccountState::Paused);
        a.disabled = true;
        assert_eq!(state_of(&a), AccountState::Disabled);
    }

    #[test]
    fn balanced_ranks_by_headroom_and_skips_accounts_that_cannot_serve() {
        let mut low = acct("low");
        low.headroom_secs = Some(100);
        let mut high = acct("high");
        high.headroom_secs = Some(9_000);
        let plenty = acct("plenty"); // no pressure → infinite headroom
        let mut paused = acct("paused");
        paused.paused = true;
        let mut near = acct("near");
        near.near_quota = true;
        near.headroom_secs = Some(50_000);
        let v = view(vec![low, paused, near, high, plenty]);
        assert_eq!(v.mode(), RankMode::Balanced);
        assert_eq!(
            order(&v, false),
            [
                (Some(1), "plenty".to_string()),
                (Some(2), "high".to_string()),
                (Some(3), "low".to_string()),
                (Some(4), "near".to_string()),
                (None, "paused".to_string()),
            ]
        );
    }

    #[test]
    fn balanced_can_follow_the_gateways_soonest_reset_setting() {
        let mut later = acct("later");
        later.reset_5h = Some(5_000);
        let mut sooner = acct("sooner");
        sooner.reset_7d = Some(2_000);
        let none = acct("none");
        let v = view(vec![none, later, sooner]);
        let names: Vec<_> = order(&v, true).into_iter().map(|(_, n)| n).collect();
        assert_eq!(names, ["sooner", "later", "none"]);
    }

    #[test]
    fn custom_order_follows_priority_and_keeps_unavailable_accounts_numbered() {
        let mut a = acct("a");
        a.priority = Some(2);
        let mut b = acct("b");
        b.priority = Some(1);
        b.paused = true;
        let mut c = acct("c");
        c.priority = Some(3);
        let v = view(vec![a, b, c]);
        assert_eq!(v.mode(), RankMode::Custom);
        assert_eq!(
            order(&v, false),
            [
                (Some(1), "b".to_string()),
                (Some(2), "a".to_string()),
                (Some(3), "c".to_string()),
            ]
        );
    }

    #[test]
    fn mode_is_balanced_when_priorities_tie_whatever_their_value() {
        let mut a = acct("a");
        a.priority = Some(5);
        let mut b = acct("b");
        b.priority = Some(5);
        assert_eq!(view(vec![a, b]).mode(), RankMode::Balanced);
        assert_eq!(view(vec![]).mode(), RankMode::Balanced);
    }

    #[test]
    fn provider_is_off_only_when_everything_that_could_serve_is_paused() {
        let mut p1 = acct("a");
        p1.paused = true;
        let mut p2 = acct("b");
        p2.paused = true;
        assert!(!view(vec![p1.clone(), p2]).is_on());
        assert!(view(vec![p1.clone(), acct("live")]).is_on());
        // A config-disabled account cannot be paused, so it does not keep the provider on.
        let mut disabled = acct("d");
        disabled.disabled = true;
        assert!(!view(vec![p1, disabled]).is_on());
        assert!(view(vec![]).is_on(), "no accounts: nothing to switch off");
    }

    #[test]
    fn only_oauth_pool_providers_can_add_accounts() {
        let mut p = view(vec![]);
        p.auth = Some("kimi_oauth".into());
        assert_eq!(p.account_kind(), None);
        p.auth = Some("chatgpt_oauth".into());
        assert_eq!(p.account_kind(), Some("codex"));
        p.auth = Some("antigravity_oauth".into());
        assert_eq!(p.account_kind(), Some("antigravity"));
    }
}

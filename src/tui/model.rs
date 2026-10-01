//! The pool snapshot the monitor renders: the wire shape of
//! `GET /admin/api/pool`, flattened into one row per account, with the local
//! sort and provider filter applied on top.
//!
//! Everything here is pure so the ordering and state ladder are unit-testable
//! without a terminal or a gateway.

use std::cmp::Ordering;

use serde::Deserialize;

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
    #[serde(default)]
    pub accounts: Vec<AccountDto>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct QuotaBucketDto {
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub remaining: Option<f64>,
    #[serde(default)]
    pub reset_time: Option<String>,
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
    pub available: bool,
    pub near_quota: bool,
    pub priority: Option<u32>,
    pub headroom_secs: Option<i64>,
    pub cooldown_secs_remaining: Option<u64>,
    pub cooldown_fable_secs_remaining: Option<u64>,
    pub utilization_5h: Option<f64>,
    pub reset_5h: Option<u64>,
    pub utilization_7d: Option<f64>,
    pub reset_7d: Option<u64>,
    pub utilization_7d_oi: Option<f64>,
    pub reset_7d_oi: Option<u64>,
    pub quota_buckets: Vec<QuotaBucketDto>,
}

/// The one state an account displays, mirroring the dashboard's ladder
/// (`ui/src/accounts.ts::managedState`) with `Paused` added. Declaration order
/// is the ladder order *and* the order the "state" sort uses, so the accounts
/// that need an operator sort next to each other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
            Self::Cooling => "cooling",
            Self::NearQuota => "near quota",
            Self::CoolingFable => "cooling (fable)",
            Self::Available => "available",
        }
    }

    /// Whether selection can currently pick this account.
    pub fn is_selectable(self) -> bool {
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
    pub provider: String,
    pub state: AccountState,
    pub account: AccountDto,
}

impl Row {
    /// Soonest reset among the windows this account reports, in epoch seconds.
    /// `None` when it reports none — those sort last, matching how the
    /// gateway's own `sort_by_reset` treats an account with no reset signal.
    pub fn soonest_reset(&self) -> Option<u64> {
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

    /// Highest utilization across the account's windows (fraction, 0..1), or
    /// the fullest quota bucket for providers that report buckets instead.
    pub fn peak_utilization(&self) -> Option<f64> {
        let windows = [
            self.account.utilization_5h,
            self.account.utilization_7d,
            self.account.utilization_7d_oi,
        ];
        let buckets = self
            .account
            .quota_buckets
            .iter()
            .filter_map(|bucket| bucket.remaining.map(|remaining| 1.0 - remaining));
        windows
            .into_iter()
            .flatten()
            .chain(buckets)
            .max_by(|a, b| a.total_cmp(b))
    }

    /// The identity a mutation targets. `None` for a gateway too old to send
    /// `account_ref`, in which case pausing is unavailable.
    pub fn account_ref(&self) -> Option<&str> {
        self.account
            .account_ref
            .as_deref()
            .filter(|r| !r.is_empty())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    /// Gateway order: provider tables as configured, accounts as the pool lists them.
    Default,
    Name,
    State,
    Utilization5h,
    Utilization7d,
    Peak,
    Reset,
}

impl SortKey {
    pub const CYCLE: [SortKey; 7] = [
        Self::Default,
        Self::Name,
        Self::State,
        Self::Utilization5h,
        Self::Utilization7d,
        Self::Peak,
        Self::Reset,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Default => "pool order",
            Self::Name => "name",
            Self::State => "state",
            Self::Utilization5h => "5h usage",
            Self::Utilization7d => "7d usage",
            Self::Peak => "peak usage",
            Self::Reset => "soonest reset",
        }
    }

    pub fn next(self) -> Self {
        let at = Self::CYCLE.iter().position(|k| *k == self).unwrap_or(0);
        Self::CYCLE[(at + 1) % Self::CYCLE.len()]
    }
}

/// `None` sorts last in either direction: an account with no reading is not
/// "least used", so reversing the sort must not float it to the top.
fn directed(ord: Ordering, desc: bool) -> Ordering {
    if desc {
        ord.reverse()
    } else {
        ord
    }
}

fn cmp_opt<T>(
    a: Option<T>,
    b: Option<T>,
    desc: bool,
    cmp: impl Fn(&T, &T) -> Ordering,
) -> Ordering {
    match (a, b) {
        (Some(a), Some(b)) => directed(cmp(&a, &b), desc),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub rows: Vec<Row>,
    pub sort_by_reset: bool,
}

impl Snapshot {
    pub fn from_response(response: PoolResponse) -> Self {
        let rows = response
            .providers
            .into_iter()
            .flat_map(|provider| {
                let name = provider.provider;
                provider.accounts.into_iter().map(move |account| Row {
                    provider: name.clone(),
                    state: state_of(&account),
                    account,
                })
            })
            .collect();
        Self {
            rows,
            sort_by_reset: response.sort_by_reset,
        }
    }

    pub fn providers(&self) -> Vec<String> {
        let mut seen: Vec<String> = Vec::new();
        for row in &self.rows {
            if !seen.contains(&row.provider) {
                seen.push(row.provider.clone());
            }
        }
        seen
    }

    /// Rows after the provider filter and sort. `descending` flips the sort's
    /// natural direction; `Default` ignores it. Direction is applied inside each
    /// key so a missing reading can stay last either way.
    pub fn view(&self, filter: Option<&str>, key: SortKey, descending: bool) -> Vec<&Row> {
        let mut rows: Vec<&Row> = self
            .rows
            .iter()
            .filter(|row| filter.is_none_or(|provider| row.provider == provider))
            .collect();
        // `sort_by` is stable, so ties keep pool order and the table does not
        // shuffle between polls when nothing changed.
        let natural = |a: &Row, b: &Row| match key {
            SortKey::Default => Ordering::Equal,
            SortKey::Name => directed(
                a.account
                    .name
                    .to_lowercase()
                    .cmp(&b.account.name.to_lowercase())
                    .then_with(|| a.provider.cmp(&b.provider)),
                descending,
            ),
            SortKey::State => directed(a.state.cmp(&b.state), descending),
            SortKey::Utilization5h => cmp_opt(
                a.account.utilization_5h,
                b.account.utilization_5h,
                descending,
                f64::total_cmp,
            ),
            SortKey::Utilization7d => cmp_opt(
                a.account.utilization_7d,
                b.account.utilization_7d,
                descending,
                f64::total_cmp,
            ),
            SortKey::Peak => cmp_opt(
                a.peak_utilization(),
                b.peak_utilization(),
                descending,
                f64::total_cmp,
            ),
            SortKey::Reset => cmp_opt(a.soonest_reset(), b.soonest_reset(), descending, u64::cmp),
        };
        rows.sort_by(|a, b| natural(a, b));
        rows
    }

    pub fn summary(&self) -> Summary {
        let mut summary = Summary::default();
        for row in &self.rows {
            summary.total += 1;
            match row.state {
                AccountState::Paused => summary.paused += 1,
                AccountState::Disabled => summary.disabled += 1,
                AccountState::NeedsRelogin => summary.needs_relogin += 1,
                AccountState::Cooling | AccountState::CoolingFable => summary.cooling += 1,
                AccountState::NearQuota => summary.near_quota += 1,
                AccountState::Available | AccountState::Unseen => {}
            }
            if row.account.available && row.state.is_selectable() {
                summary.available += 1;
            }
        }
        summary
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Summary {
    pub total: usize,
    pub available: usize,
    pub paused: usize,
    pub cooling: usize,
    pub near_quota: usize,
    pub needs_relogin: usize,
    pub disabled: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account(name: &str) -> AccountDto {
        AccountDto {
            name: name.into(),
            account_ref: Some(format!("ref-{name}")),
            has_state: true,
            available: true,
            ..AccountDto::default()
        }
    }

    fn snapshot(accounts: Vec<AccountDto>) -> Snapshot {
        Snapshot::from_response(PoolResponse {
            providers: vec![ProviderDto {
                provider: "claude".into(),
                accounts,
            }],
            sort_by_reset: false,
        })
    }

    fn names(rows: &[&Row]) -> Vec<String> {
        rows.iter().map(|r| r.account.name.clone()).collect()
    }

    #[test]
    fn parses_the_pool_wire_shape_tolerating_missing_fields() {
        let json = r#"{"sort_by_reset":true,"providers":[{"provider":"claude","auth":"claude_oauth",
            "accounts":[{"name":"a","account_ref":"r1","paused":true,"utilization_5h":0.5,"reset_5h":1700000000},
                        {"name":"b"}]}]}"#;
        let snap = Snapshot::from_response(serde_json::from_str(json).unwrap());
        assert!(snap.sort_by_reset);
        assert_eq!(snap.rows.len(), 2);
        assert_eq!(snap.rows[0].state, AccountState::Paused);
        assert_eq!(snap.rows[1].state, AccountState::Unseen);
        assert_eq!(snap.rows[1].account_ref(), None);
    }

    #[test]
    fn state_ladder_matches_the_dashboard() {
        let mut a = account("x");
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
    fn utilization_sort_puts_missing_readings_last_in_both_directions() {
        let mut low = account("low");
        low.utilization_5h = Some(0.1);
        let mut high = account("high");
        high.utilization_5h = Some(0.9);
        let none = account("none");
        let snap = snapshot(vec![none, high, low]);

        let asc = snap.view(None, SortKey::Utilization5h, false);
        assert_eq!(names(&asc), ["low", "high", "none"]);
        let desc = snap.view(None, SortKey::Utilization5h, true);
        assert_eq!(names(&desc), ["high", "low", "none"]);
    }

    #[test]
    fn reset_sort_uses_the_soonest_window_and_ignores_zero() {
        let mut soon = account("soon");
        soon.reset_7d = Some(2_000);
        soon.reset_5h = Some(1_500);
        let mut later = account("later");
        later.reset_5h = Some(3_000);
        let mut zero = account("zero");
        zero.reset_5h = Some(0);
        let snap = snapshot(vec![zero, later, soon]);
        let rows = snap.view(None, SortKey::Reset, false);
        assert_eq!(names(&rows), ["soon", "later", "zero"]);
    }

    #[test]
    fn name_and_state_sorts_reverse_and_default_keeps_pool_order() {
        let mut paused = account("b");
        paused.paused = true;
        let snap = snapshot(vec![account("c"), paused, account("a")]);
        assert_eq!(
            names(&snap.view(None, SortKey::Default, true)),
            ["c", "b", "a"]
        );
        assert_eq!(
            names(&snap.view(None, SortKey::Name, false)),
            ["a", "b", "c"]
        );
        assert_eq!(
            names(&snap.view(None, SortKey::Name, true)),
            ["c", "b", "a"]
        );
        // Paused sorts ahead of available on the state key.
        assert_eq!(names(&snap.view(None, SortKey::State, false))[0], "b");
    }

    #[test]
    fn peak_considers_quota_buckets_for_providers_without_windows() {
        let mut a = account("bucketed");
        a.quota_buckets = vec![
            QuotaBucketDto {
                label: "pro".into(),
                remaining: Some(0.8),
                reset_time: None,
            },
            QuotaBucketDto {
                label: "flash".into(),
                remaining: Some(0.25),
                reset_time: None,
            },
        ];
        let snap = snapshot(vec![a]);
        let peak = snap.rows[0].peak_utilization().unwrap();
        assert!((peak - 0.75).abs() < 1e-9);
    }

    #[test]
    fn provider_filter_and_summary() {
        let snap = Snapshot::from_response(PoolResponse {
            providers: vec![
                ProviderDto {
                    provider: "claude".into(),
                    accounts: vec![account("a")],
                },
                ProviderDto {
                    provider: "codex".into(),
                    accounts: vec![{
                        let mut p = account("b");
                        p.paused = true;
                        p.available = false;
                        p
                    }],
                },
            ],
            sort_by_reset: false,
        });
        assert_eq!(snap.providers(), ["claude", "codex"]);
        assert_eq!(snap.view(Some("codex"), SortKey::Default, false).len(), 1);
        let s = snap.summary();
        assert_eq!((s.total, s.available, s.paused), (2, 1, 1));
    }
}

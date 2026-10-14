//! Rank Antigravity's independent model-family budgets within a priority tier.

use std::cmp::Ordering;

use super::QuotaBucketSnapshot;
use crate::auth::{antigravity::pool::ModelFamily, claude::usage::parse_rfc3339_to_epoch_secs};

#[derive(Clone, Copy, Debug)]
pub(super) struct Rank {
    remaining: f64,
    reset: Option<u64>,
}

pub(super) fn rank(buckets: &[QuotaBucketSnapshot], model: Option<&str>, now: u64) -> Option<Rank> {
    let labels = match ModelFamily::for_model(model?)? {
        ModelFamily::Gemini => ["Gemini Models · 5h", "Gemini Models · weekly"],
        ModelFamily::ClaudeGpt => ["Claude + GPT Models · 5h", "Claude + GPT Models · weekly"],
    };
    let mut rank: Option<Rank> = None;
    for bucket in buckets
        .iter()
        .filter(|bucket| labels.contains(&bucket.label.as_str()))
    {
        let Some(remaining) = bucket
            .remaining
            .filter(|value| value.is_finite() && (0.0..=1.0).contains(value))
        else {
            continue;
        };
        let reset = bucket
            .reset_time
            .as_deref()
            .and_then(parse_rfc3339_to_epoch_secs);
        // A reading from a completed window cannot order the new window.
        if reset.is_some_and(|reset| reset <= now) {
            continue;
        }
        rank = Some(match rank {
            None => Rank { remaining, reset },
            Some(previous) => Rank {
                remaining: previous.remaining.min(remaining),
                reset: previous.reset.into_iter().chain(reset).min(),
            },
        });
    }
    rank
}

pub(super) fn compare(left: Option<Rank>, right: Option<Rank>) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => {
            right.remaining.total_cmp(&left.remaining).then_with(|| {
                match (left.reset, right.reset) {
                    (Some(left), Some(right)) => left.cmp(&right),
                    (Some(_), None) => Ordering::Less,
                    (None, Some(_)) => Ordering::Greater,
                    (None, None) => Ordering::Equal,
                }
            })
        }
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        accounts::{AccountPool, StoreFamily},
        config::{AccountConfig, PoolConfig},
    };

    fn account(name: &str) -> AccountConfig {
        AccountConfig {
            name: name.into(),
            store_family: Some(StoreFamily::Antigravity),
            ..Default::default()
        }
    }

    fn bucket(label: &str, remaining: f64, reset: Option<&str>) -> QuotaBucketSnapshot {
        QuotaBucketSnapshot {
            label: label.into(),
            remaining: Some(remaining),
            reset_time: reset.map(str::to_owned),
        }
    }

    #[test]
    fn higher_remaining_fraction_precedes_lower_per_model_family() {
        let pool = AccountPool::new();
        let accounts = [account("a"), account("b")];
        for (account, gemini, other) in [(&accounts[0], 0.2, 0.9), (&accounts[1], 0.8, 0.1)] {
            pool.note_antigravity_usage(
                "custom-google",
                account,
                vec![
                    bucket("Gemini Models · weekly", gemini, None),
                    bucket("Claude + GPT Models · weekly", other, None),
                ],
            );
        }
        // The existing reset sorting flags cannot replace family quota ranking.
        let config = PoolConfig {
            sort_by_reset: true,
            sort_by_weekly_reset: true,
            ..Default::default()
        };
        for _ in 0..4 {
            for (model, expected) in [
                ("gemini-3.1-pro", vec![1, 0]),
                ("claude-sonnet-4-6", vec![0, 1]),
                ("gpt-oss-120b", vec![0, 1]),
            ] {
                assert_eq!(
                    pool.select_order("custom-google", &accounts, None, Some(model), Some(&config)),
                    expected,
                    "{model}"
                );
            }
        }
    }

    #[test]
    fn equal_readings_rotate_and_known_readings_precede_unknown() {
        let pool = AccountPool::new();
        let accounts = [account("a"), account("b"), account("unseen")];
        for account in &accounts[..2] {
            pool.note_antigravity_usage(
                "google",
                account,
                vec![bucket("Gemini Models · weekly", 0.7, None)],
            );
        }
        for expected in [vec![0, 1, 2], vec![1, 0, 2], vec![0, 1, 2]] {
            assert_eq!(
                pool.select_order("google", &accounts, None, Some("gemini-pro"), None),
                expected
            );
        }
        // A different store must ignore grouped Antigravity readings.
        let other_pool = AccountPool::new();
        let other = accounts.map(|mut account| {
            account.store_family = Some(StoreFamily::Claude);
            account
        });
        other_pool.note_antigravity_usage(
            "claude",
            &other[1],
            vec![bucket("Gemini Models · weekly", 0.9, None)],
        );
        assert_eq!(
            other_pool.select_order("claude", &other, None, Some("gemini-pro"), None),
            vec![0, 1, 2]
        );
    }

    #[test]
    fn lowest_active_window_then_earliest_reset_govern_the_family() {
        let buckets = [
            bucket("Gemini Models · 5h", 0.3, Some("2030-01-01T01:00:00Z")),
            bucket("Gemini Models · weekly", 0.9, Some("2030-01-02T00:00:00Z")),
        ];
        let limited = rank(&buckets, Some("gemini-pro"), 0).unwrap();
        assert_eq!(limited.remaining, 0.3);
        let earlier = rank(
            &[bucket(
                "Gemini Models · weekly",
                0.3,
                Some("2030-01-01T00:00:00Z"),
            )],
            Some("gemini-pro"),
            0,
        );
        assert_eq!(compare(earlier, Some(limited)), Ordering::Less);
        // Compare epochs, not strings: these timestamps denote the same instant.
        let offset = rank(
            &[bucket(
                "Gemini Models · weekly",
                0.3,
                Some("2030-01-01T09:00:00+09:00"),
            )],
            Some("gemini-pro"),
            0,
        );
        assert_eq!(compare(earlier, offset), Ordering::Equal);
    }

    #[test]
    fn expired_invalid_missing_and_unknown_windows_do_not_rank() {
        let buckets = [
            bucket("Gemini Models · 5h", 0.0, Some("1970-01-01T00:00:01Z")),
            bucket("Gemini Models · weekly", f64::NAN, None),
            bucket("unknown", 0.8, None),
        ];
        assert!(rank(&buckets, Some("gemini-pro"), 2).is_none());
        assert!(rank(&buckets, Some("claude-sonnet"), 0).is_none());
        assert!(rank(&buckets, Some("unknown"), 0).is_none());
        assert!(rank(&buckets, None, 0).is_none());
        assert_eq!(compare(None, None), Ordering::Equal);
    }

    #[test]
    fn priority_disabled_pause_and_family_cooldown_still_govern_selection() {
        let pool = AccountPool::new();
        let mut accounts = [account("a"), account("b")];
        accounts[0].priority = 1;
        accounts[1].priority = 2;
        for (account, remaining) in [(&accounts[0], 0.1), (&accounts[1], 0.9)] {
            pool.note_antigravity_usage(
                "google",
                account,
                vec![bucket("Gemini Models · weekly", remaining, None)],
            );
        }
        assert_eq!(
            pool.select_order("google", &accounts, None, Some("gemini-pro"), None),
            vec![0, 1]
        );
        accounts[0].disabled = true;
        assert_eq!(
            pool.select_order("google", &accounts, None, Some("gemini-pro"), None),
            vec![1]
        );
        accounts[0].disabled = false;
        pool.set_paused("google", &accounts[0], true);
        assert_eq!(
            pool.select_order("google", &accounts, None, Some("gemini-pro"), None),
            vec![1]
        );
        pool.set_paused("google", &accounts[0], false);
        pool.cooldown_antigravity_family(
            "google",
            &accounts[0],
            ModelFamily::Gemini,
            std::time::Duration::from_secs(60),
        );
        assert_eq!(
            pool.select_order("google", &accounts, None, Some("gemini-pro"), None),
            vec![1]
        );
        assert_eq!(
            pool.select_order("google", &accounts, None, Some("claude-sonnet"), None),
            vec![0, 1]
        );
    }
}

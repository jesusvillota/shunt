//! The two independent subscription pools and quota-reset hints from refusals.

use std::{sync::LazyLock, time::Duration};

#[derive(Debug, Clone, Copy)]
pub(crate) enum ModelFamily {
    Gemini = 0,
    ClaudeGpt = 1,
}

impl ModelFamily {
    pub(crate) fn for_model(model: &str) -> Option<Self> {
        let model = model.to_ascii_lowercase();
        if model.starts_with("gemini-") {
            Some(Self::Gemini)
        } else if model.starts_with("claude-") || model.starts_with("gpt-") {
            Some(Self::ClaudeGpt)
        } else {
            None
        }
    }
}

/// Google reports weekly exhaustion as `Resets in 161h31m52s.` without a
/// Retry-After header. Only parse the error's message, not arbitrary body text.
pub(crate) fn reset_delay(body: &str) -> Option<Duration> {
    static RESET: LazyLock<regex_lite::Regex> = LazyLock::new(|| {
        regex_lite::Regex::new(r"Resets in (?:(\d+)h)?(?:(\d+)m)?(?:(\d+)s)?\.")
            .expect("valid quota-reset pattern")
    });
    let body: serde_json::Value = serde_json::from_str(body).ok()?;
    let message = body.pointer("/error/message")?.as_str()?;
    let captures = RESET.captures(message)?;
    let mut seconds = 0_u64;
    let mut found = false;
    for (index, multiplier) in [(1, 3600), (2, 60), (3, 1)] {
        if let Some(value) = captures.get(index) {
            found = true;
            seconds = seconds.checked_add(
                value
                    .as_str()
                    .parse::<u64>()
                    .ok()?
                    .checked_mul(multiplier)?,
            )?;
        }
    }
    found.then(|| Duration::from_secs(seconds))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_weekly_and_short_reset_delays() {
        for (message, seconds) in [
            ("Individual quota reached. Resets in 161h31m52s.", 581512),
            ("Resets in 2s.", 2),
            ("Resets in 1h.", 3600),
            ("Resets in 0s.", 0),
        ] {
            let body = serde_json::json!({"error": {"message": message}}).to_string();
            assert_eq!(reset_delay(&body), Some(Duration::from_secs(seconds)));
        }
    }

    #[test]
    fn rejects_missing_malformed_and_overflowing_reset_delays() {
        for message in [
            "Quota exhausted.",
            "Resets in .",
            "Resets in -2s.",
            "Resets in 1d.",
            "Resets in 18446744073709551615h.",
        ] {
            let body = serde_json::json!({"error": {"message": message}}).to_string();
            assert_eq!(reset_delay(&body), None, "{message}");
        }
        assert_eq!(reset_delay("Resets in 2s."), None);
    }
}

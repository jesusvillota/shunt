use super::*;

const CAPTURED_WEEKLY: &str =
    include_str!("../../../tests/fixtures/agy-usage-weekly-captured.json");
const FOUR_WINDOWS: &str =
    include_str!("../../../tests/fixtures/agy-usage-four-windows-schema.json");

#[test]
fn captured_weekly_reply_preserves_only_reported_windows() {
    let reply = serde_json::from_str(CAPTURED_WEEKLY).unwrap();
    let buckets = parse_reply(&reply).unwrap();
    assert_eq!(buckets.len(), 2);
    assert_eq!(buckets[0].label, "Gemini Models · weekly");
    assert_eq!(buckets[0].remaining, Some(0.0));
    assert_eq!(buckets[1].label, "Claude + GPT Models · weekly");
    assert!(buckets[1].remaining.unwrap() > 0.0);
    assert!(buckets.iter().all(|bucket| bucket.reset_time.is_some()));
}

#[test]
fn representative_four_window_schema_parses_all_grouped_buckets() {
    let reply = serde_json::from_str(FOUR_WINDOWS).unwrap();
    let buckets = parse_reply(&reply).unwrap();
    assert_eq!(
        buckets.iter().map(|b| b.label.as_str()).collect::<Vec<_>>(),
        [
            "Gemini Models · 5h",
            "Gemini Models · weekly",
            "Claude + GPT Models · 5h",
            "Claude + GPT Models · weekly",
        ]
    );
    assert_eq!(
        buckets.iter().map(|b| b.remaining).collect::<Vec<_>>(),
        [Some(0.7), Some(0.41), Some(0.92), Some(0.63),]
    );
    assert_eq!(
        buckets[0].reset_time.as_deref(),
        Some("2026-10-14T12:00:00Z")
    );
}

#[test]
fn version_gate_rejects_old_and_ambiguous_versions() {
    for version in ["1.1.11", "1.3.4", "agy 2.0.0", "v1.1.11"] {
        assert!(supported_version(version), "{version}");
    }
    for version in ["1.1.10", "0.99.99", "", "1.1", "1.1.11-preview", "garbage"] {
        assert!(!supported_version(version), "{version}");
    }
}

#[test]
fn unrelated_or_empty_command_does_not_replace_quota() {
    for reply in [
        serde_json::json!({"status":"SUCCESS","command":{"name":"chat","data":{}}}),
        serde_json::json!({"status":"ERROR","command":{"name":"usage","data":{}}}),
        serde_json::json!({"status":"SUCCESS","command":{"name":"usage","data":{"groups":[]}}}),
    ] {
        assert!(parse_reply(&reply).is_err());
    }
}

#[cfg(unix)]
pub(crate) struct FakeAgy {
    pub(crate) dir: std::path::PathBuf,
    pub(crate) executable: std::path::PathBuf,
}

#[cfg(unix)]
impl FakeAgy {
    pub(crate) fn new(reply: &str, version: &str) -> Self {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!(
            "shunt-agy-probe-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(dir.join("reply.json"), reply).unwrap();
        std::fs::write(dir.join("version"), version).unwrap();
        let executable = dir.join("agy");
        std::fs::write(
            &executable,
            r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo version >> "$HOME/calls"
  cat "$HOME/version"
else
  echo usage >> "$HOME/calls"
  [ "$USERPROFILE" = "$HOME" ] || exit 12
  [ "$*" = "-p /usage --output-format json --print-timeout 20s" ] || exit 13
  cat "$HOME/reply.json"
  [ ! -f "$HOME/fail-exit" ] || exit 1
fi
"#,
        )
        .unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
        Self { dir, executable }
    }

    pub(crate) fn probe(&self) -> UsageProbe {
        UsageProbe {
            executable: self.executable.clone().into(),
            ..Default::default()
        }
    }

    pub(crate) fn calls(&self) -> String {
        std::fs::read_to_string(self.dir.join("calls")).unwrap_or_default()
    }
}

#[cfg(unix)]
impl Drop for FakeAgy {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[cfg(unix)]
#[tokio::test]
#[allow(clippy::await_holding_lock)] // Current-thread test protects subprocess environment reads.
async fn model_turn_latches_probe_and_spends_no_further_calls_even_for_another_profile() {
    let _guard = crate::config::CONFIG_ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let fake = FakeAgy::new(r#"{"status":"ERROR","num_turns":1}"#, "1.3.4");
    std::fs::write(fake.dir.join("fail-exit"), "").unwrap();
    let other = FakeAgy::new(CAPTURED_WEEKLY, "1.3.4");
    let mut probe = fake.probe();
    assert!(probe.fetch(fake.dir.to_str().unwrap()).await.is_err());
    assert!(probe.latched_off);
    for profile in [&fake.dir, &other.dir, &fake.dir] {
        assert!(probe.fetch(profile.to_str().unwrap()).await.is_err());
    }
    assert_eq!(fake.calls(), "version\nusage\n");
    assert_eq!(other.calls(), "");
}

#[cfg(unix)]
#[tokio::test]
#[allow(clippy::await_holding_lock)] // Current-thread test protects subprocess environment reads.
async fn explicit_profiles_are_isolated_and_old_cli_spends_no_usage_calls() {
    let _guard = crate::config::CONFIG_ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let first = FakeAgy::new(CAPTURED_WEEKLY, "1.3.4");
    let second = FakeAgy::new(FOUR_WINDOWS, "1.3.4");
    let mut probe = first.probe();
    assert_eq!(
        probe
            .fetch(first.dir.to_str().unwrap())
            .await
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        probe
            .fetch(second.dir.to_str().unwrap())
            .await
            .unwrap()
            .len(),
        4
    );
    assert_eq!(first.calls(), "version\nusage\n");
    assert_eq!(second.calls(), "usage\n");
    let old = FakeAgy::new(CAPTURED_WEEKLY, "1.1.10");
    assert!(old.probe().fetch(old.dir.to_str().unwrap()).await.is_err());
    assert_eq!(old.calls(), "version\n");
    assert!(probe.fetch("").await.is_err());
    assert!(probe.fetch("/no/such/shunt-agy-profile").await.is_err());
    assert_eq!(first.calls(), "version\nusage\n");
}

#[cfg(unix)]
#[test]
fn command_removes_ambient_credentials_and_closes_stdin() {
    let _guard = crate::config::CONFIG_ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let probe = UsageProbe::default();
    let command = probe.command(Path::new("/explicit/account"), &["--version"]);
    let env = command.as_std().get_envs().collect::<Vec<_>>();
    assert!(env.contains(&(
        std::ffi::OsStr::new("HOME"),
        Some(std::ffi::OsStr::new("/explicit/account"))
    )));
    for key in [
        "GEMINI_API_KEY",
        "GOOGLE_API_KEY",
        "GOOGLE_APPLICATION_CREDENTIALS",
        "GCLOUD_PROJECT",
    ] {
        assert!(env.contains(&(std::ffi::OsStr::new(key), None)));
    }
    for (key, value) in env {
        let key = key.to_string_lossy();
        if key.starts_with("GOOGLE_") || key.starts_with("GEMINI_") || key.starts_with("CLOUDSDK_")
        {
            assert!(value.is_none());
        }
    }
}

#[cfg(unix)]
#[tokio::test]
#[allow(clippy::await_holding_lock)] // Current-thread test protects subprocess environment reads.
async fn process_deadline_and_output_limit_bound_the_probe() {
    let _guard = crate::config::CONFIG_ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut slow = Command::new("/bin/sleep");
    slow.arg("5").stdout(Stdio::piped()).kill_on_drop(true);
    let start = std::time::Instant::now();
    assert!(run_bounded(slow, Duration::from_millis(30))
        .await
        .unwrap_err()
        .to_string()
        .contains("timed out"));
    assert!(start.elapsed() < Duration::from_secs(2));
    let mut verbose = Command::new("/usr/bin/yes");
    verbose.stdout(Stdio::piped()).kill_on_drop(true);
    assert!(run_bounded(verbose, Duration::from_secs(5))
        .await
        .unwrap_err()
        .to_string()
        .contains("exceeds limit"));
}

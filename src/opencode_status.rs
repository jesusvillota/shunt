//! Passive OpenCode free-model status from the local `opencode` CLI log.
//!
//! OpenCode answers 403 to any client that is not `opencode`, so there is no
//! quota API and no admin endpoint to ask. The one passive signal is the tail of
//! `~/.local/share/opencode/log/opencode.log`; the line shapes and the state
//! rule are in `docs/notes/opencode-log-format.md`.
//!
//! [`parse`] is a pure function over the log text, so the fixture tests it
//! without the real file; [`read`] reads the tail and parses it.

use std::{
    collections::{BTreeSet, HashMap},
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// A rate-limit event older than this says nothing about now: the free tier has
/// reset. Events older than the window are ignored.
const WINDOW_MS: i64 = 24 * 60 * 60 * 1000;
/// Read at least this much of the tail; it covers 24 h with margin on the
/// measured file (~2.6 MB per busy day).
const CHUNK: u64 = 5_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Reachable,
    RateLimited,
    Unknown,
}

impl State {
    pub fn label(self) -> &'static str {
        match self {
            State::Reachable => "reachable",
            State::RateLimited => "rate limited",
            State::Unknown => "unknown",
        }
    }
}

/// One model's state and the age of the event it names (the last error for
/// `RateLimited`, the last stream otherwise).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Status {
    pub state: State,
    pub age: Duration,
}

/// The state of every model seen in one log tail. A model that is absent —
/// including one `opencode models` lists but the log never mentions — is
/// [`State::Unknown`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Table(HashMap<String, Status>);

impl Table {
    /// The state of `model`, whose id may carry the `opencode/` prefix that
    /// `opencode models` prints but the log does not.
    pub fn state(&self, model: &str) -> State {
        self.get(model).map_or(State::Unknown, |s| s.state)
    }

    pub fn get(&self, model: &str) -> Option<&Status> {
        self.0.get(model.strip_prefix("opencode/").unwrap_or(model))
    }

    /// Every model seen in the window, in no particular order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, Status)> + '_ {
        self.0
            .iter()
            .map(|(model, status)| (model.as_str(), *status))
    }
}

/// The state of each model on the lines of `text`, as of `now`.
pub fn parse(text: &str, now: SystemTime) -> Table {
    let now = unix_ms(now);
    let floor = now - WINDOW_MS;
    // Newest stream and newest error per model, inside the window.
    let mut streams: HashMap<&str, i64> = HashMap::new();
    let mut errors: HashMap<&str, (i64, bool)> = HashMap::new();
    for line in text.lines() {
        let Some((model, at, message, rate)) = event(line) else {
            continue;
        };
        if at < floor {
            continue;
        }
        match message {
            "stream" => {
                let newest = streams.entry(model).or_insert(i64::MIN);
                *newest = (*newest).max(at);
            }
            "stream error" => {
                let newest = errors.entry(model).or_insert((i64::MIN, false));
                if at > newest.0 {
                    *newest = (at, rate);
                }
            }
            _ => {}
        }
    }

    let models: BTreeSet<&str> = streams.keys().chain(errors.keys()).copied().collect();
    let mut table = HashMap::new();
    for model in models {
        let stream = streams.get(model).copied();
        let error = errors.get(model).copied();
        // A stream newer than the newest error clears it; anything else that is
        // not a rate limit is unknown.
        let (state, at) = match (stream, error) {
            (Some(s), Some((e, _))) if s > e => (State::Reachable, s),
            (Some(s), None) => (State::Reachable, s),
            (_, Some((e, true))) => (State::RateLimited, e),
            (_, Some((e, false))) => (State::Unknown, e),
            (None, None) => continue,
        };
        table.insert(
            model.to_string(),
            Status {
                state,
                age: Duration::from_millis((now - at).max(0) as u64),
            },
        );
    }
    Table(table)
}

/// One `providerID=opencode` event line: its bare model id, time in ms, the
/// `message` kind, and whether its error is a rate limit.
fn event(line: &str) -> Option<(&str, i64, &str, bool)> {
    let (mut provider, mut model, mut at, mut message) = (None, None, None, None);
    let mut rate = false;
    for (key, value) in pairs(line) {
        match key {
            "providerID" => provider = Some(value),
            "modelID" => model = Some(value),
            "timestamp" => at = parse_time(value),
            "message" => message = Some(value),
            "error.error" => rate = value.to_ascii_lowercase().contains("rate limit"),
            _ => {}
        }
    }
    if provider != Some("opencode") {
        return None;
    }
    Some((model?, at?, message?, rate))
}

/// The `key=value` pairs of one logfmt line. A value wrapped in `"` may hold
/// spaces; a bare token without `=` is skipped.
fn pairs(line: &str) -> Vec<(&str, &str)> {
    let bytes = line.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        while i < bytes.len() && bytes[i] == b' ' {
            i += 1;
        }
        let key_start = i;
        while i < bytes.len() && bytes[i] != b'=' && bytes[i] != b' ' {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] != b'=' {
            continue;
        }
        let key = &line[key_start..i];
        i += 1;
        let value = if i < bytes.len() && bytes[i] == b'"' {
            i += 1;
            let start = i;
            // A quoted value may hold escaped quotes (`\"`), as a logged
            // command line does; do not stop at those.
            while i < bytes.len() && bytes[i] != b'"' {
                if bytes[i] == b'\\' && i + 1 < bytes.len() {
                    i += 1;
                }
                i += 1;
            }
            let value = &line[start..i];
            i += usize::from(i < bytes.len());
            value
        } else {
            let start = i;
            while i < bytes.len() && bytes[i] != b' ' {
                i += 1;
            }
            &line[start..i]
        };
        out.push((key, value));
    }
    out
}

/// Read the log tail that covers the 24 h window and parse it. The reason the
/// file could not be read is returned as text for the TUI.
pub fn read(now: SystemTime) -> Result<Table, String> {
    let Some(path) = log_path() else {
        return Err("no home directory".into());
    };
    read_from(&path, now)
}

/// The `opencode` CLI log this machine writes.
pub fn log_path() -> Option<PathBuf> {
    crate::auth::shared::home_dir().map(|home| home.join(".local/share/opencode/log/opencode.log"))
}

fn read_from(path: &Path, now: SystemTime) -> Result<Table, String> {
    let text = read_window(path, now).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(parse(&text, now))
}

/// The last 5 MB, extended backward while the oldest line is younger than the
/// 24 h window (a very busy day can hold more than 5 MB inside one window).
fn read_window(path: &Path, now: SystemTime) -> std::io::Result<String> {
    let mut file = File::open(path)?;
    let floor = unix_ms(now) - WINDOW_MS;
    let mut start = file.metadata()?.len();
    let mut buf = Vec::new();
    loop {
        let take = CHUNK.min(start);
        start -= take;
        file.seek(SeekFrom::Start(start))?;
        let mut chunk = vec![0u8; take as usize];
        file.read_exact(&mut chunk)?;
        chunk.extend_from_slice(&buf);
        buf = chunk;
        if start == 0 {
            break;
        }
        let oldest = pairs(&String::from_utf8_lossy(&buf))
            .into_iter()
            .find(|(k, _)| *k == "timestamp")
            .and_then(|(_, v)| parse_time(v));
        if oldest.is_none_or(|at| at <= floor) {
            break;
        }
    }
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

fn unix_ms(t: SystemTime) -> i64 {
    t.duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

/// Milliseconds since the Unix epoch for an ISO-8601 UTC timestamp such as
/// `2026-10-13T23:53:00.040Z`, or `None` when it does not parse.
fn parse_time(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    let num = |at: usize, len: usize| s.get(at..at + len)?.parse::<i64>().ok();
    if b.len() < 20 || b[4] != b'-' || b[7] != b'-' || b[13] != b':' || b[16] != b':' {
        return None;
    }
    let (year, month, day) = (num(0, 4)?, num(5, 2)?, num(8, 2)?);
    let (hour, minute, second) = (num(11, 2)?, num(14, 2)?, num(17, 2)?);
    let millis = if b[19] == b'.' {
        num(20, 3).unwrap_or(0)
    } else {
        0
    };
    let secs = days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second;
    Some(secs * 1_000 + millis)
}

/// Days from 1970-01-01 to `y-m-d` (Howard Hinnant's civil-date algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// `12s`, `5m`, `3h`, `2d` — a coarse age for the TUI.
pub fn age_text(age: Duration) -> String {
    let secs = age.as_secs();
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3_600 {
        format!("{}m", secs / 60)
    } else if secs < 86_400 {
        format!("{}h", secs / 3_600)
    } else {
        format!("{}d", secs / 86_400)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../tests/fixtures/opencode/log-sample.txt");

    /// The log fixture lines, shaped like `docs/notes/opencode-log-format.md`.
    fn stream(at: &str, model: &str) -> String {
        format!(
            "timestamp={at} level=INFO run=REDACTED message=stream providerID=opencode \
             modelID={model} session.id=ses_REDACTED small=false agent=build mode=primary"
        )
    }

    fn error(at: &str, model: &str, text: &str) -> String {
        format!(
            "timestamp={at} level=ERROR run=REDACTED message=\"stream error\" \
             providerID=opencode modelID={model} session.id=ses_REDACTED small=false \
             agent=build mode=primary error.error=\"{text}\""
        )
    }

    fn now(at: &str) -> SystemTime {
        UNIX_EPOCH + Duration::from_millis(parse_time(at).expect("valid timestamp") as u64)
    }

    #[test]
    fn a_recent_rate_limit_error_is_rate_limited_with_its_age() {
        let tail = error(
            "2026-10-13T23:53:00.040Z",
            "muse-spark-1.3-contributor-free",
            "AI_APICallError: Rate limit exceeded. Please try again later.",
        );
        let table = parse(&tail, now("2026-10-14T01:23:00.040Z"));
        let status = table.get("muse-spark-1.3-contributor-free").unwrap();
        assert_eq!(status.state, State::RateLimited);
        assert_eq!(status.age, Duration::from_secs(90 * 60));
    }

    #[test]
    fn a_newer_stream_clears_an_older_error() {
        let tail = [
            error(
                "2026-10-13T23:53:00.040Z",
                "big-pickle",
                "AI_APICallError: Rate limit exceeded. Please try again later.",
            ),
            stream("2026-10-13T23:58:00.000Z", "big-pickle"),
        ]
        .join("\n");
        let table = parse(&tail, now("2026-10-14T00:00:00.000Z"));
        let status = table.get("big-pickle").unwrap();
        assert_eq!(status.state, State::Reachable);
        assert_eq!(status.age, Duration::from_secs(120));
    }

    #[test]
    fn a_non_rate_limit_error_without_a_newer_stream_is_unknown() {
        let tail = error(
            "2026-10-13T16:08:27.254Z",
            "ling-3.1-flash-free",
            "AI_APICallError: Upstream request failed: Endpoint is unavailable.",
        );
        let table = parse(&tail, now("2026-10-13T17:00:00.000Z"));
        assert_eq!(table.state("ling-3.1-flash-free"), State::Unknown);
    }

    #[test]
    fn the_fixture_marks_a_recent_success_reachable_and_an_absent_model_unknown() {
        let table = parse(FIXTURE, now("2026-10-16T15:00:00.000Z"));
        // The newest muse-spark line in the window is the 2026-10-16 stream.
        assert_eq!(
            table.state("opencode/muse-spark-1.3-contributor-free"),
            State::Reachable
        );
        assert_eq!(
            table.state("opencode/nemotron-3-ultra-free"),
            State::Unknown
        );
        assert_eq!(table.state("opencode/never-seen-free"), State::Unknown);
    }

    #[test]
    fn events_older_than_the_window_are_ignored() {
        // big-pickle's stream is 2026-10-05; at 2026-10-17 it is outside 24 h.
        let table = parse(FIXTURE, now("2026-10-17T12:00:00.000Z"));
        assert_eq!(table.state("opencode/big-pickle"), State::Unknown);
    }

    #[test]
    fn timestamp_and_age_text_parse() {
        let t = parse_time("2026-10-13T23:53:00.040Z").unwrap();
        assert_eq!(t, 1_791_935_580_040);
        assert_eq!(parse_time("nope"), None);
        assert_eq!(age_text(Duration::from_secs(45)), "45s");
        assert_eq!(age_text(Duration::from_secs(90 * 60)), "1h");
        assert_eq!(age_text(Duration::from_secs(2 * 86_400)), "2d");
    }

    #[test]
    fn a_logged_command_line_that_mentions_a_provider_is_not_an_event() {
        // Real `message="spawning process"` lines embed a command whose escaped
        // quotes contain the text `providerID=opencode modelID=…`; they must not
        // be read as provider events.
        let tail = [
            "timestamp=2026-10-17T13:23:43.168Z level=INFO run=R message=\"spawning process\" command=/bin/zsh args=\"[\\\"-c\\\",\\\"grep 'providerID=opencode modelID=[^ ]+'\\\"]\" cwd=/tmp",
            &stream("2026-10-16T14:00:00.000Z", "fast-free"),
        ]
        .join("\n");
        let table = parse(&tail, now("2026-10-16T15:00:00.000Z"));
        assert_eq!(table.state("fast-free"), State::Reachable);
        assert_eq!(table.iter().count(), 1);
    }

    /// One-time live check: run the parser on the real log and print every
    /// model's state. `cargo test --features tui --lib opencode_status -- --ignored --nocapture`.
    #[test]
    #[ignore = "reads the real opencode log"]
    fn live_log_prints_each_model_state() {
        let now = SystemTime::now();
        let table = read(now).expect("read the real opencode log");
        let mut models: Vec<_> = table.iter().collect();
        models.sort_by_key(|(model, _)| *model);
        println!("opencode log: {:?}", log_path());
        for (model, status) in models {
            println!(
                "  {model:<40} {:<13} age {}",
                status.state.label(),
                age_text(status.age)
            );
        }
    }
}

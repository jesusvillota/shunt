//! `shunt check` runs the routed-Antigravity credential guard, not just
//! `Config::load(...).validate()` (issue #382).
//!
//! The guard is what makes the `kind = "antigravity"` rename non-silent: a
//! migrated config that routes to the native upstream with nobody having run
//! `shunt login antigravity` must be refused. It used to live only in the
//! run/serve path, so `check` reported `config ok` for a config `run` dies on
//! — exactly backwards for the CI and deploy scripts that gate a rollout on
//! `check`.
//!
//! `check` also reads the account stores offline to confirm that every
//! `[server.auth.clients]` pin names an account of its provider.
//!
//! Every case below drives the real binary, so the credential path is set on
//! the *child* process. Writing `SHUNT_ANTIGRAVITY_AUTH_FILE` into this
//! process would instead race — and leak into — every other test sharing it.

use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "shunt-check-cli-{tag}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).expect("create temp directory");
        Self(path)
    }

    /// Write `config` as `shunt.toml` and return the path to hand `--config`.
    fn config(&self, config: &str) -> PathBuf {
        let path = self.0.join("shunt.toml");
        std::fs::write(&path, config).expect("write config");
        path
    }

    /// The credential path the child probes. Nothing is written here unless a
    /// test calls [`TempDir::credential`], so by default it does not exist.
    fn credential_path(&self) -> PathBuf {
        self.0.join("antigravity-auth.json")
    }

    fn credential(&self) -> PathBuf {
        let path = self.credential_path();
        std::fs::write(&path, "{}").expect("write credential");
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn check(config: &Path, credential: &Path) -> Output {
    // `Path` is passed straight through: `Command::arg` takes `AsRef<OsStr>`,
    // so converting to `&str` first would only add a panic on a non-UTF-8
    // temp path without buying anything.
    Command::new(env!("CARGO_BIN_EXE_shunt"))
        .arg("check")
        .arg("--config")
        .arg(config)
        .env("SHUNT_ANTIGRAVITY_AUTH_FILE", credential)
        .env(
            "SHUNT_ANTIGRAVITY_ACCOUNTS_DIR",
            credential.parent().unwrap_or(credential),
        )
        .output()
        .expect("shunt binary should run")
}

fn stdout(output: &Output) -> &str {
    std::str::from_utf8(&output.stdout).expect("stdout should be UTF-8")
}

fn stderr(output: &Output) -> &str {
    std::str::from_utf8(&output.stderr).expect("stderr should be UTF-8")
}

#[test]
fn check_refuses_a_routed_antigravity_provider_without_a_credential() {
    let dir = TempDir::new("routed-no-credential");
    let config = dir.config("[server]\ndefault_provider = \"antigravity\"\n");

    let output = check(&config, &dir.credential_path());

    assert!(
        !output.status.success(),
        "a routed antigravity provider with no credential must fail the check; \
         stdout: {}",
        stdout(&output)
    );
    assert!(
        !stdout(&output).contains("config ok"),
        "the refused config must not also be reported ok: {}",
        stdout(&output)
    );
    // The message has to be actionable on its own: which provider, what is
    // missing, and the command that fixes it.
    let message = stderr(&output);
    for expected in [
        "provider `antigravity`",
        "no credential",
        "shunt login antigravity",
    ] {
        assert!(
            message.contains(expected),
            "the failure must name {expected:?}: {message}"
        );
    }
}

#[test]
fn check_accepts_a_routed_antigravity_provider_with_a_credential() {
    let dir = TempDir::new("routed-with-credential");
    let config = dir.config("[server]\ndefault_provider = \"antigravity\"\n");

    let output = check(&config, &dir.credential());

    assert!(
        output.status.success(),
        "a credential at the probed path must satisfy the guard; stderr: {}",
        stderr(&output)
    );
    assert!(stdout(&output).contains("config ok"), "{}", stdout(&output));
}

#[test]
fn check_accepts_an_empty_credential_because_the_probe_is_presence_only() {
    // Pins a deliberate limit rather than a bug. The guard asks whether the
    // credential file exists, never what is in it, so an empty (or stale, or
    // malformed) file passes `check` and fails later on the request path.
    //
    // This test exists because the docs make that claim in prose
    // (README.md, site guides/providers.mdx). Without it the claim is
    // unfalsifiable, and a future change that started parsing the credential
    // here would silently tighten what `shunt check` accepts relative to
    // `shunt run` — reopening the divergence issue #382 closed.
    let dir = TempDir::new("empty-credential");
    let config = dir.config("[server]\ndefault_provider = \"antigravity\"\n");
    let credential = dir.credential_path();
    std::fs::write(&credential, "").expect("write empty credential");

    let output = check(&config, &credential);

    assert!(
        output.status.success(),
        "presence-only means a zero-byte credential satisfies the guard; stderr: {}",
        stderr(&output)
    );
    assert!(stdout(&output).contains("config ok"), "{}", stdout(&output));
}

#[test]
fn check_accepts_the_built_in_antigravity_provider_when_nothing_routes_to_it() {
    // Every default config seeds a built-in `antigravity` provider, so a guard
    // keyed on the provider merely *existing* would fail this — that is, every
    // config anyone has ever written — rather than on the config being able to
    // send a request to it. Keying on routing is the whole reason the run-path
    // guard is shaped the way it is; `check` inherits that, not a copy of it.
    let dir = TempDir::new("seeded-not-routed");
    let config = dir.config("[server]\ndefault_provider = \"anthropic\"\n");

    let output = check(&config, &dir.credential_path());

    assert!(
        output.status.success(),
        "a config that never routes to antigravity must pass with no credential; \
         stderr: {}",
        stderr(&output)
    );
    assert!(stdout(&output).contains("config ok"), "{}", stdout(&output));
}

#[test]
fn check_accepts_routing_to_the_antigravity_cli_transport_without_a_credential() {
    // `antigravity-cli` is the deprecated `agy` subprocess transport. It
    // authenticates through the local CLI, not through the shunt-owned
    // credential file, so the native upstream's guard must not fire on it —
    // the migration path #372 offers is precisely "route to `antigravity-cli`
    // to stay put", and a guard that refused it would close that path.
    let dir = TempDir::new("cli-transport");
    let config = dir.config("[server]\ndefault_provider = \"antigravity-cli\"\n");

    let output = check(&config, &dir.credential_path());

    assert!(
        output.status.success(),
        "antigravity_cli routing must be unaffected by the native guard; stderr: {}",
        stderr(&output)
    );
    assert!(stdout(&output).contains("config ok"), "{}", stdout(&output));
}

/// Run `shunt check` on a config that appends `clients` to a Claude pool with
/// inline account `a` and a Codex pool read from an account store holding
/// `k`. Every account store points into the temp directory, so the check never
/// reads the real ones.
fn check_clients(tag: &str, clients: &str) -> Output {
    let dir = TempDir::new(tag);
    for family in ["claude", "codex", "kimi", "antigravity"] {
        std::fs::create_dir_all(dir.0.join(family)).expect("create account store");
    }
    std::fs::write(dir.0.join("codex").join("k.json"), "{}").expect("write store account");
    let config = dir.config(&format!(
        r#"[server.auth]
tokens_env = "SHUNT_TEST_CHECK_CLIENT_TOKENS"

[providers.claude]
kind = "anthropic"
base_url = "https://api.anthropic.com"
auth = "claude_oauth"

[[providers.claude.accounts]]
name = "a"
token_env = "SHUNT_TEST_CHECK_CLAUDE_TOKEN"

[providers.codex]
kind = "responses"
base_url = "https://chatgpt.com/backend-api"
auth = "chatgpt_oauth"

{clients}"#
    ));
    Command::new(env!("CARGO_BIN_EXE_shunt"))
        .arg("check")
        .arg("--config")
        .arg(config)
        .env("SHUNT_TEST_CHECK_CLIENT_TOKENS", "goal-x:tok-x")
        .env("SHUNT_CLAUDE_ACCOUNTS_DIR", dir.0.join("claude"))
        .env("SHUNT_CODEX_ACCOUNTS_DIR", dir.0.join("codex"))
        .env("SHUNT_KIMI_ACCOUNTS_DIR", dir.0.join("kimi"))
        .env("SHUNT_ANTIGRAVITY_ACCOUNTS_DIR", dir.0.join("antigravity"))
        .env("SHUNT_ANTIGRAVITY_AUTH_FILE", dir.credential_path())
        .output()
        .expect("shunt binary should run")
}

#[test]
fn check_accepts_clients_pinned_to_inline_and_store_accounts() {
    let output = check_clients(
        "clients-ok",
        "[server.auth.clients.goal-x]\naccounts.claude = [\"a\"]\naccounts.codex = [\"k\"]\n",
    );

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(stdout(&output).contains("config ok"), "{}", stdout(&output));
}

#[test]
fn check_refuses_a_client_entry_that_cannot_select_an_account() {
    for (tag, accounts, expected) in [
        (
            "clients-empty-map",
            "",
            "remove the entry to allow all accounts",
        ),
        (
            "clients-empty-list",
            "accounts.claude = []",
            "accounts.claude is empty; remove it to allow all accounts",
        ),
        (
            "clients-unknown-provider",
            "accounts.nope = [\"a\"]",
            "unknown provider 'nope'",
        ),
        (
            "clients-no-pool",
            "accounts.anthropic = [\"a\"]",
            "provider 'anthropic' has no account pool",
        ),
        (
            "clients-unknown-account",
            "accounts.claude = [\"zz\"]",
            "provider 'claude' has no account 'zz'",
        ),
        (
            "clients-inline-owner",
            "accounts.codex = [\"a\"]",
            "account 'a' belongs to provider 'claude', not 'codex'",
        ),
        (
            "clients-store-owner",
            "accounts.claude = [\"k\"]",
            "account 'k' belongs to provider 'codex', not 'claude'",
        ),
    ] {
        let output = check_clients(tag, &format!("[server.auth.clients.goal-x]\n{accounts}\n"));

        assert!(
            !output.status.success(),
            "{tag} must fail the check; stdout: {}",
            stdout(&output)
        );
        let message = stderr(&output);
        assert!(
            message.contains("[server.auth.clients.goal-x]") && message.contains(expected),
            "{tag} must name the entry and say {expected:?}: {message}"
        );
    }
}

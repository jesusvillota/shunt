//! `shunt client set|clear|show` against a temp `shunt.toml`: the command owns
//! the `[server.auth.clients.<name>]` table and the client's `tokens_file` line.
#![cfg(unix)]

use std::{
    fs,
    io::Write,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Command, Output, Stdio},
};

const TOKEN: &str = "client-cli-secret-0123456789abcdef";

const CONFIG: &str = r#"# shunt gateway
[server]
bind = "127.0.0.1:8080"

# goal tokens
[server.auth]
tokens_file = "TOKENS" # owner-only

[providers.claude]
kind = "anthropic"
base_url = "https://api.anthropic.com"
auth = "claude_oauth"

[[providers.claude.accounts]]
name = "a"
token_env = "SHUNT_TEST_CLIENT_CLAUDE_A"

[[providers.claude.accounts]]
name = "b"
token_env = "SHUNT_TEST_CLIENT_CLAUDE_B"

# codex accounts come from the account store
[providers.codex]
kind = "responses"
base_url = "https://chatgpt.com/backend-api"
auth = "chatgpt_oauth"
"#;

struct Rig {
    dir: PathBuf,
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

impl Rig {
    /// A Claude pool with inline accounts `a` and `b`, and a Codex pool read
    /// from an account store holding `k`. Every store is in the temp directory.
    fn new(tag: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("shunt-client-cli-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        for family in ["claude", "codex", "kimi", "antigravity"] {
            fs::create_dir_all(dir.join(family)).expect("create account store");
        }
        fs::write(dir.join("codex/k.json"), "{}").expect("write store account");
        let rig = Self { dir };
        fs::write(
            rig.path("shunt.toml"),
            CONFIG.replace("TOKENS", rig.path("clients.tokens").to_str().unwrap()),
        )
        .expect("write config");
        fs::set_permissions(rig.path("shunt.toml"), fs::Permissions::from_mode(0o640))
            .expect("chmod config");
        rig
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    fn read(&self, name: &str) -> String {
        fs::read_to_string(self.path(name)).unwrap_or_default()
    }

    fn mode(&self, name: &str) -> u32 {
        fs::metadata(self.path(name)).unwrap().permissions().mode() & 0o777
    }

    fn client(&self, args: &[&str], stdin: Option<&str>) -> Output {
        let mut child = Command::new(env!("CARGO_BIN_EXE_shunt"))
            .arg("client")
            .args(args)
            .arg("--config")
            .arg(self.path("shunt.toml"))
            // The gateway refuses a `[server.auth]` without any token.
            .env("SHUNT_CLIENT_TOKENS", "default:default-token")
            .env("SHUNT_CLAUDE_ACCOUNTS_DIR", self.path("claude"))
            .env("SHUNT_CODEX_ACCOUNTS_DIR", self.path("codex"))
            .env("SHUNT_KIMI_ACCOUNTS_DIR", self.path("kimi"))
            .env("SHUNT_ANTIGRAVITY_ACCOUNTS_DIR", self.path("antigravity"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("shunt binary should run");
        let mut pipe = child.stdin.take().expect("stdin");
        if let Some(text) = stdin {
            pipe.write_all(text.as_bytes()).expect("write stdin");
        }
        drop(pipe);
        child.wait_with_output().expect("shunt binary should exit")
    }
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn assert_no_token(output: &Output) {
    assert!(!stdout(output).contains(TOKEN), "token in stdout");
    assert!(!stderr(output).contains(TOKEN), "token in stderr");
}

#[test]
fn set_registers_the_token_once_and_keeps_comments_order_and_mode() {
    let rig = Rig::new("set");
    let original = rig.read("shunt.toml");
    // The new table goes next to its parent; nothing else moves.
    let with_pins = |pins: &str| {
        original.replace(
            "# owner-only\n",
            &format!("# owner-only\n\n[server.auth.clients.x]\n{pins}"),
        )
    };
    let pinned = with_pins("accounts.claude = [\"a\", \"b\"]\n");

    for _ in 0..2 {
        let output = rig.client(
            &["set", "x", "--accounts", "claude=a,b", "--token-stdin"],
            Some(&format!("{TOKEN}\n")),
        );
        assert!(output.status.success(), "stderr: {}", stderr(&output));
        assert_no_token(&output);
        assert_eq!(rig.read("shunt.toml"), pinned);
        assert_eq!(rig.read("clients.tokens"), format!("x:{TOKEN}\n"));
    }
    assert_eq!(rig.mode("shunt.toml"), 0o640);
    assert_eq!(rig.mode("clients.tokens"), 0o600);

    // A second provider is added; the first one keeps its pin.
    let output = rig.client(&["set", "x", "--accounts", "codex=k"], None);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(
        rig.read("shunt.toml"),
        with_pins("accounts.claude = [\"a\", \"b\"]\naccounts.codex = [\"k\"]\n")
    );

    let shown = rig.client(&["show", "x", "--json"], None);
    assert!(shown.status.success(), "stderr: {}", stderr(&shown));
    assert_no_token(&shown);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&stdout(&shown)).unwrap(),
        serde_json::json!({
            "name": "x",
            "accounts": {"claude": ["a", "b"], "codex": ["k"]},
            "token_registered": true,
        })
    );

    // One provider's pin is removed; the other stays.
    let cleared = rig.client(&["clear", "x", "--provider", "codex"], None);
    assert!(cleared.status.success(), "stderr: {}", stderr(&cleared));
    assert_eq!(rig.read("shunt.toml"), pinned);

    // Clearing restores the file byte for byte and leaves the token in place.
    let cleared = rig.client(&["clear", "x"], None);
    assert!(cleared.status.success(), "stderr: {}", stderr(&cleared));
    assert_eq!(rig.read("shunt.toml"), original);
    assert_eq!(rig.mode("shunt.toml"), 0o640);
    assert_eq!(rig.read("clients.tokens"), format!("x:{TOKEN}\n"));

    let shown = rig.client(&["show", "x"], None);
    assert_eq!(stdout(&shown), "all accounts\ntoken: registered\n");

    // Removing the last pinned provider removes the table too.
    let output = rig.client(&["set", "x", "--accounts", "codex=k"], None);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let cleared = rig.client(&["clear", "x", "--provider", "codex"], None);
    assert!(cleared.status.success(), "stderr: {}", stderr(&cleared));
    assert_eq!(rig.read("shunt.toml"), original);
}

#[test]
fn set_refuses_names_outside_the_pool_without_writing() {
    let rig = Rig::new("pool");
    let original = rig.read("shunt.toml");
    for (accounts, expected) in [
        ("claude=a,zz", "provider 'claude' has no account 'zz'"),
        ("nope=a", "unknown provider 'nope'"),
        (
            "claude=k",
            "account 'k' belongs to provider 'codex', not 'claude'",
        ),
        ("claude=", "name a provider and at least one account"),
        ("claude", "use <provider>=<account,...>"),
    ] {
        let output = rig.client(
            &["set", "x", "--accounts", accounts, "--token-stdin"],
            Some(TOKEN),
        );
        assert_eq!(output.status.code(), Some(1), "{accounts}");
        assert!(stderr(&output).contains(expected), "{}", stderr(&output));
        assert!(output.stdout.is_empty());
        assert_no_token(&output);
    }
    assert_eq!(rig.read("shunt.toml"), original);
    assert!(!rig.path("clients.tokens").exists());
}

#[test]
fn set_refuses_a_token_that_conflicts_with_the_tokens_file() {
    let rig = Rig::new("token");
    let original = rig.read("shunt.toml");
    let tokens = format!("x:another-token\ny:{TOKEN}\n");
    fs::write(rig.path("clients.tokens"), &tokens).unwrap();
    fs::set_permissions(
        rig.path("clients.tokens"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();

    for (name, stdin, expected) in [
        ("x", TOKEN, "registers client x with a different token"),
        ("z", TOKEN, "registers this token for client y"),
        ("z", "", "standard input holds no usable token"),
        ("z", "two words", "standard input holds no usable token"),
    ] {
        let output = rig.client(
            &["set", name, "--accounts", "claude=a", "--token-stdin"],
            Some(stdin),
        );
        assert_eq!(output.status.code(), Some(1), "{name} {expected}");
        assert!(stderr(&output).contains(expected), "{}", stderr(&output));
        assert_no_token(&output);
        assert!(!stderr(&output).contains("another-token"));
    }
    assert_eq!(rig.read("shunt.toml"), original);
    assert_eq!(rig.read("clients.tokens"), tokens);
}

#[test]
fn the_token_is_not_accepted_as_an_argument() {
    let rig = Rig::new("argument");
    let output = rig.client(
        &["set", "x", "--accounts", "claude=a", "--token-stdin", TOKEN],
        None,
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(!rig.path("clients.tokens").exists());
}

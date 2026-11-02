pub const ENV: &[(&str, &str)] = &[
    ("SHUNT_TEST_POOL_CLIENT_TOKENS", "goal-y:inbound-token"),
    (
        "SHUNT_TEST_POOL_ADMIN_TOKEN",
        "pool-read-token-0123456789abcdef0123456789",
    ),
];

pub fn config() -> shunt::config::Config {
    toml::from_str::<shunt::config::Config>(
        r#"
[server]
bind = "127.0.0.1:0"
default_provider = "anthropic"
[server.auth]
tokens_env = "SHUNT_TEST_POOL_CLIENT_TOKENS"
[server.auth.clients.goal-y]
accounts.anthropic = ["c"]

[[server.admin.read_keys]]
id = "pool-reader"
key = "pool-read-token-0123456789abcdef0123456789"

[providers.anthropic]
kind = "anthropic"
base_url = "http://127.0.0.1:1"
auth = "claude_oauth"
accounts = [
    { name = "a", token_env = "SHUNT_TEST_POOL_ACCOUNT_TOKEN" },
    { name = "b", token_env = "SHUNT_TEST_POOL_ACCOUNT_TOKEN" },
    { name = "c", token_env = "SHUNT_TEST_POOL_ACCOUNT_TOKEN" },
]
[providers.codex]
kind = "responses"
base_url = "http://127.0.0.1:1"
auth = "chatgpt_oauth"
accounts = [{ name = "code", token_env = "SHUNT_TEST_POOL_ACCOUNT_TOKEN" }]
"#,
    )
    .unwrap()
    .validate()
    .unwrap()
}

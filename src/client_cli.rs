//! `shunt client` — set, clear and show one client's account pins.
//!
//! This command owns the on-disk format of a pin: the
//! `[server.auth.clients.<name>]` table of `shunt.toml` and the client's
//! `name:token` line in `[server.auth] tokens_file`. Other tools (the `goal`
//! CLI) call it instead of editing either file. See `docs/m4-inbound-auth.md`.
//!
//! Every check runs before the first write, so a refused call changes nothing.
//! The gateway's config hot reload picks the edit up.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, bail, Context};
use toml_edit::{value, Array, DocumentMut, Item, Table};

use crate::config::{ClientPolicy, Config, ConfigFormat, InboundAuthConfig, ProviderConfig};

/// Provider name → the account names a client is pinned to.
pub type Accounts = BTreeMap<String, Vec<String>>;

/// The config file to edit: `--config`, or the first one the gateway's own
/// loader would find. A symlink is resolved, so the edit replaces its target.
fn locate(explicit: Option<&Path>) -> anyhow::Result<PathBuf> {
    let path = explicit
        .map(Path::to_path_buf)
        .or_else(Config::find_config_file)
        .ok_or_else(|| anyhow!("no shunt config file found; pass --config <path>"))?;
    path.canonicalize()
        .map_err(|error| anyhow!("cannot read {}: {error}", path.display()))
}

fn locate_toml(explicit: Option<&Path>) -> anyhow::Result<PathBuf> {
    let path = locate(explicit)?;
    if ConfigFormat::from_path(&path) != ConfigFormat::Toml {
        bail!(
            "{} is not TOML; `shunt client` edits shunt.toml only",
            path.display()
        );
    }
    Ok(path)
}

fn read_document(path: &Path) -> anyhow::Result<(String, DocumentMut)> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| anyhow!("cannot read {}: {error}", path.display()))?;
    let document = text
        .parse::<DocumentMut>()
        .map_err(|error| anyhow!("invalid {}: {error}", path.display()))?;
    Ok((text, document))
}

fn load(path: &Path) -> anyhow::Result<Config> {
    Config::load(Some(path)).map_err(|error| anyhow!("{error}"))
}

/// A client name is a TOML key here and, with a token, the part before `:` on
/// a `tokens_file` line.
fn check_name(name: &str) -> anyhow::Result<()> {
    if name.is_empty()
        || name
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || c == ':' || c == ',')
    {
        bail!("invalid client name: use no spaces, ':' or ','");
    }
    Ok(())
}

/// Parse `--accounts <provider>=<a,b>` items.
fn parse_accounts(items: &[String]) -> anyhow::Result<Accounts> {
    let mut accounts = Accounts::new();
    for item in items {
        let (provider, names) = item
            .split_once('=')
            .ok_or_else(|| anyhow!("--accounts {item}: use <provider>=<account,...>"))?;
        let names: Vec<String> = names.split(',').map(str::to_string).collect();
        if provider.is_empty() || names.iter().any(String::is_empty) {
            bail!("--accounts {item}: name a provider and at least one account; `shunt client clear` removes pins");
        }
        if accounts.insert(provider.to_string(), names).is_some() {
            bail!("--accounts names provider {provider} twice");
        }
    }
    Ok(accounts)
}

fn auth_table(document: &mut DocumentMut) -> Option<&mut Table> {
    document.get_mut("server")?.get_mut("auth")?.as_table_mut()
}

/// Set `providers` in the client's table; providers not listed keep their pins.
fn write_pins(document: &mut DocumentMut, name: &str, providers: &Accounts) -> anyhow::Result<()> {
    let auth = auth_table(document).ok_or_else(|| anyhow!("no [server.auth] table"))?;
    let clients = auth
        .entry("clients")
        .or_insert_with(|| {
            let mut clients = Table::new();
            clients.set_implicit(true);
            Item::Table(clients)
        })
        .as_table_mut()
        .ok_or_else(|| anyhow!("[server.auth] clients is not a table"))?;
    let client = clients
        .entry(name)
        .or_insert_with(|| Item::Table(Table::new()))
        .as_table_mut()
        .ok_or_else(|| anyhow!("[server.auth.clients] {name} is not a table"))?;
    let accounts = client
        .entry("accounts")
        .or_insert_with(|| {
            let mut accounts = Table::new();
            accounts.set_dotted(true);
            Item::Table(accounts)
        })
        .as_table_like_mut()
        .ok_or_else(|| anyhow!("[server.auth.clients.{name}] accounts is not a table"))?;
    for (provider, names) in providers {
        accounts.insert(
            provider,
            value(names.iter().map(String::as_str).collect::<Array>()),
        );
    }
    Ok(())
}

/// Drop the client's pins on `providers`, or all of them when none is named.
/// A client left without pins loses its table, and `clients` goes with its
/// last entry.
fn remove_pins(document: &mut DocumentMut, name: &str, providers: &[String]) {
    let Some(auth) = auth_table(document) else {
        return;
    };
    let Some(clients) = auth.get_mut("clients").and_then(Item::as_table_like_mut) else {
        return;
    };
    let accounts = clients
        .get_mut(name)
        .and_then(|client| client.get_mut("accounts"))
        .and_then(Item::as_table_like_mut);
    let unpinned = match accounts {
        Some(accounts) if !providers.is_empty() => {
            for provider in providers {
                accounts.remove(provider);
            }
            accounts.is_empty()
        }
        _ => providers.is_empty(),
    };
    if unpinned {
        clients.remove(name);
    }
    if clients.is_empty() {
        auth.remove("clients");
    }
}

fn inbound_auth<'a>(config: &'a Config, path: &Path) -> anyhow::Result<&'a InboundAuthConfig> {
    config.server.auth.as_ref().ok_or_else(|| {
        anyhow!(
            "{} has no [server.auth] table; set up inbound auth first",
            path.display()
        )
    })
}

/// The `tokens_file`. The gateway opens it as written, from its own working
/// directory; only an absolute path names the same file for this command.
fn tokens_path(auth: &InboundAuthConfig) -> anyhow::Result<PathBuf> {
    let raw = auth
        .tokens_file
        .as_deref()
        .ok_or_else(|| anyhow!("set [server.auth] tokens_file in shunt.toml first"))?;
    let path = PathBuf::from(raw);
    if !path.is_absolute() {
        bail!("[server.auth] tokens_file must be an absolute path: {raw}");
    }
    Ok(path)
}

fn registered_tokens(path: &Path) -> anyhow::Result<Vec<(String, String)>> {
    crate::config::read_token_file(&path.to_string_lossy()).map_err(|error| anyhow!("{error}"))
}

/// The new `tokens_file` text that registers `name` with `token`, or `None`
/// when that pair is already there. Messages never contain a token.
fn token_file_update(
    auth: &InboundAuthConfig,
    name: &str,
    token: &str,
) -> anyhow::Result<Option<(PathBuf, String)>> {
    if token.is_empty() || !token.chars().all(|c| c.is_ascii_graphic() && c != ',') {
        bail!("standard input holds no usable token (printable ASCII, no spaces or commas)");
    }
    let path = tokens_path(auth)?;
    let registered = registered_tokens(&path)?;
    if let Some((_, existing)) = registered.iter().find(|(client, _)| client == name) {
        if existing == token {
            return Ok(None);
        }
        bail!(
            "{} registers client {name} with a different token",
            path.display()
        );
    }
    if let Some((other, _)) = registered.iter().find(|(_, existing)| existing == token) {
        bail!("{} registers this token for client {other}", path.display());
    }
    let mut contents = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => bail!("cannot read {}: {error}", path.display()),
    };
    if !contents.is_empty() && !contents.ends_with('\n') {
        contents.push('\n');
    }
    contents.push_str(&format!("{name}:{token}\n"));
    Ok(Some((path, contents)))
}

/// Write the edited config back, keeping the file's permission bits. Returns
/// the line to print.
fn save(path: &Path, before: &str, document: &DocumentMut) -> anyhow::Result<String> {
    let after = document.to_string();
    if after == before {
        return Ok(format!("{} is unchanged\n", path.display()));
    }
    crate::atomic_file::write_atomic_keeping_mode(path, after.as_bytes())
        .map_err(|error| anyhow!("cannot write {}: {error}", path.display()))?;
    Ok(format!("saved {}\n", path.display()))
}

/// `shunt client set`: pin `name` to the accounts in `items`
/// (`<provider>=<a,b>`), and register `token` for it when one is given.
pub fn set(
    config: Option<&Path>,
    name: &str,
    items: &[String],
    token: Option<&str>,
) -> anyhow::Result<String> {
    check_name(name)?;
    let providers = parse_accounts(items)?;
    let path = locate_toml(config)?;
    let loaded = load(&path)?;
    let auth = inbound_auth(&loaded, &path)?;

    let mut accounts = auth
        .clients
        .get(name)
        .map(|policy| policy.accounts.clone())
        .unwrap_or_default();
    accounts.extend(providers.clone());
    // Check this client alone: a stale pin of another client must not block it.
    let mut candidate = loaded.clone();
    if let Some(auth) = candidate.server.auth.as_mut() {
        auth.clients = BTreeMap::from([(name.to_string(), ClientPolicy { accounts })]);
    }
    candidate
        .validate_client_accounts(ProviderConfig::pool_account_names)
        .map_err(|error| anyhow!("{error}"))?;

    let (before, mut document) = read_document(&path)?;
    write_pins(&mut document, name, &providers)
        .with_context(|| format!("cannot edit {}", path.display()))?;
    let token_update = match token {
        Some(token) => token_file_update(auth, name, token)?,
        None => None,
    };

    if let Some((tokens, contents)) = token_update {
        crate::atomic_file::write_private_atomic(&tokens, contents.as_bytes())
            .map_err(|error| anyhow!("cannot write {}: {error}", tokens.display()))?;
    }
    save(&path, &before, &document)
}

/// `shunt client clear`: remove the pins of `name` on `providers`, or every pin
/// when none is named. Its token stays registered.
pub fn clear(config: Option<&Path>, name: &str, providers: &[String]) -> anyhow::Result<String> {
    check_name(name)?;
    let path = locate_toml(config)?;
    let (before, mut document) = read_document(&path)?;
    remove_pins(&mut document, name, providers);
    save(&path, &before, &document)
}

/// `shunt client show`: the pins of `name` and whether `tokens_file` holds a
/// token for it. Never prints the token.
pub fn show(config: Option<&Path>, name: &str, json: bool) -> anyhow::Result<String> {
    check_name(name)?;
    let path = locate(config)?;
    let loaded = load(&path)?;
    let auth = loaded.server.auth.as_ref();
    let accounts = auth
        .and_then(|auth| auth.clients.get(name))
        .map(|policy| policy.accounts.clone())
        .unwrap_or_default();
    let token_registered = match auth.and_then(|auth| auth.tokens_file.as_deref()) {
        Some(tokens) => registered_tokens(Path::new(tokens))?
            .iter()
            .any(|(client, _)| client == name),
        None => false,
    };
    if json {
        let body = serde_json::json!({
            "name": name,
            "accounts": accounts,
            "token_registered": token_registered,
        });
        return Ok(format!("{body}\n"));
    }
    let mut text = if accounts.is_empty() {
        "all accounts\n".to_string()
    } else {
        accounts
            .iter()
            .map(|(provider, names)| format!("{provider}={}\n", names.join(",")))
            .collect()
    };
    text.push_str(if token_registered {
        "token: registered\n"
    } else {
        "token: not registered\n"
    });
    Ok(text)
}

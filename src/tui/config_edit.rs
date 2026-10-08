//! Format-preserving edits of `shunt.toml` for the monitor.
//!
//! The monitor changes two things in the file: which accounts a provider's pool
//! contains, and each account's `priority` (the explicit 1/2/3 ranking). Both
//! are existing config keys, so nothing here invents a new one — the gateway's
//! own hot reload picks the edit up.
//!
//! Only the `[providers.<name>]` form is edited. A provider defined through
//! `[[upstreams]]` is refused with a message rather than guessed at.
//!
//! The text transforms are pure (`&str -> String`) so they can be tested
//! without touching the filesystem; [`apply`] wraps them in the read, write
//! and lock.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context};
use toml_edit::{value, ArrayOfTables, DocumentMut, Item, Table, TableLike, Value};

use crate::config::{Config, ConfigFormat};

/// The default selection priority; an account with no `priority` key sits here.
pub const DEFAULT_PRIORITY: u32 = 100;

/// The config file to edit: `--config`, or the first one the gateway's own
/// loader would find. Only TOML is supported.
pub fn locate(explicit: Option<&Path>) -> anyhow::Result<PathBuf> {
    let path = explicit
        .map(Path::to_path_buf)
        .or_else(Config::find_config_file)
        .ok_or_else(|| {
            anyhow!("no shunt config file found; pass --config <path> (the one the gateway runs)")
        })?;
    if ConfigFormat::from_path(&path) != ConfigFormat::Toml {
        bail!(
            "{} is not TOML; editing accounts and ranking from `shunt tui` supports shunt.toml only",
            path.display()
        );
    }
    Ok(path)
}

/// Run `edit` over the file's text and write the result back atomically. A
/// `None` from `edit` means nothing needed to change and the file is not
/// touched. Returns whether the file changed.
pub fn apply(
    path: &Path,
    edit: impl FnOnce(&str) -> anyhow::Result<Option<String>>,
) -> anyhow::Result<bool> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let Some(updated) = edit(&text)? else {
        return Ok(false);
    };
    crate::atomic_file::write_private_atomic(path, updated.as_bytes())
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(true)
}

fn parse(text: &str) -> anyhow::Result<DocumentMut> {
    text.parse::<DocumentMut>()
        .context("the config file is not valid TOML")
}

fn provider_table<'a>(doc: &'a mut DocumentMut, provider: &str) -> anyhow::Result<&'a mut Table> {
    let defined_by_upstreams = doc
        .get("upstreams")
        .and_then(Item::as_array_of_tables)
        .is_some_and(|tables| {
            tables
                .iter()
                .any(|t| t.get("name").and_then(Item::as_str) == Some(provider))
        });
    let table = doc
        .get_mut("providers")
        .and_then(Item::as_table_mut)
        .and_then(|providers| providers.get_mut(provider))
        .and_then(Item::as_table_mut);
    match table {
        Some(table) => Ok(table),
        None if defined_by_upstreams => bail!(
            "provider {provider:?} is defined through [[upstreams]], which `shunt tui` does not edit; \
             change that entry by hand"
        ),
        None => bail!(
            "provider {provider:?} has no [providers.{provider}] table in the config file (it may \
             come from environment overrides), so there is nothing to edit"
        ),
    }
}

fn names_of(accounts: &Item) -> Vec<String> {
    match accounts {
        Item::ArrayOfTables(tables) => tables
            .iter()
            .filter_map(|t| t.get("name").and_then(Item::as_str).map(str::to_string))
            .collect(),
        Item::Value(Value::Array(array)) => array
            .iter()
            .filter_map(Value::as_inline_table)
            .filter_map(|t| t.get("name").and_then(|v| v.as_str()).map(str::to_string))
            .collect(),
        _ => Vec::new(),
    }
}

fn entry_mut<'a>(accounts: &'a mut Item, name: &str) -> Option<&'a mut dyn TableLike> {
    match accounts {
        Item::ArrayOfTables(tables) => tables
            .iter_mut()
            .find(|t| t.get("name").and_then(Item::as_str) == Some(name))
            .map(|t| t as &mut dyn TableLike),
        Item::Value(Value::Array(array)) => array
            .iter_mut()
            .filter_map(Value::as_inline_table_mut)
            .find(|t| t.get("name").and_then(|v| v.as_str()) == Some(name))
            .map(|t| t as &mut dyn TableLike),
        _ => None,
    }
}

/// Append a name-only entry — the form that points at a store account.
fn push_entry(table: &mut Table, name: &str) -> anyhow::Result<()> {
    if !table.contains_key("accounts") {
        table.insert("accounts", Item::ArrayOfTables(ArrayOfTables::new()));
    }
    match table.get_mut("accounts") {
        Some(Item::ArrayOfTables(tables)) => {
            let mut entry = Table::new();
            entry.insert("name", value(name));
            tables.push(entry);
            Ok(())
        }
        Some(Item::Value(Value::Array(array))) => {
            let mut entry = toml_edit::InlineTable::new();
            entry.insert("name", name.into());
            array.push(entry);
            Ok(())
        }
        _ => bail!("`accounts` in this provider is not an array of accounts"),
    }
}

fn accounts_item(table: &mut Table) -> &mut Item {
    table
        .entry("accounts")
        .or_insert(Item::ArrayOfTables(ArrayOfTables::new()))
}

/// Make sure `name` is in the provider's pool. When the provider lists no
/// accounts the gateway pools every account in the store, so a new store
/// account is already included and the file is left alone.
pub fn include_account(text: &str, provider: &str, name: &str) -> anyhow::Result<Option<String>> {
    let mut doc = parse(text)?;
    let table = provider_table(&mut doc, provider)?;
    let listed = table.get("accounts").map(names_of).unwrap_or_default();
    if listed.is_empty() || listed.iter().any(|existing| existing == name) {
        return Ok(None);
    }
    push_entry(table, name)?;
    Ok(Some(doc.to_string()))
}

/// Remove `name` from an explicitly listed provider pool after its managed
/// credential is deleted. A provider with no explicit accounts scans the whole
/// store, so there is nothing to edit. Refuse to turn a one-account explicit
/// list into an empty list: empty means "scan every store account", which could
/// silently widen the pool instead of leaving it empty.
pub fn remove_account(text: &str, provider: &str, name: &str) -> anyhow::Result<Option<String>> {
    let mut doc = parse(text)?;
    let table = provider_table(&mut doc, provider)?;
    let Some(accounts) = table.get_mut("accounts") else {
        return Ok(None);
    };
    let listed = names_of(accounts);
    let matching = listed
        .iter()
        .filter(|existing| existing.as_str() == name)
        .count();
    if matching == 0 {
        return Ok(None);
    }

    let uses_inline_credentials = match accounts {
        Item::ArrayOfTables(tables) => tables.iter().any(|entry| {
            entry.get("name").and_then(Item::as_str) == Some(name)
                && (entry.contains_key("credentials") || entry.contains_key("token_env"))
        }),
        Item::Value(Value::Array(array)) => array.iter().any(|entry| {
            entry.as_inline_table().is_some_and(|table| {
                table.get("name").and_then(Value::as_str) == Some(name)
                    && (table.contains_key("credentials") || table.contains_key("token_env"))
            })
        }),
        _ => false,
    };
    if uses_inline_credentials {
        bail!(
            "account {name:?} in provider {provider:?} uses an inline credential source; \
             d only deletes shunt-managed store accounts"
        );
    }

    if matching == listed.len() {
        bail!(
            "cannot remove the last explicitly listed account {name:?} from provider {provider:?}: \
             an empty account list means scan every stored account; edit the provider by hand if \
             you want different empty-pool behavior"
        );
    }

    match accounts {
        Item::ArrayOfTables(tables) => {
            let remove: Vec<_> = tables
                .iter()
                .enumerate()
                .filter_map(|(index, entry)| {
                    (entry.get("name").and_then(Item::as_str) == Some(name)).then_some(index)
                })
                .collect();
            for index in remove.into_iter().rev() {
                tables.remove(index);
            }
        }
        Item::Value(Value::Array(array)) => {
            let remove: Vec<_> = array
                .iter()
                .enumerate()
                .filter_map(|(index, entry)| {
                    entry
                        .as_inline_table()
                        .and_then(|table| table.get("name"))
                        .and_then(Value::as_str)
                        .is_some_and(|entry_name| entry_name == name)
                        .then_some(index)
                })
                .collect();
            for index in remove.into_iter().rev() {
                array.remove(index);
            }
        }
        _ => bail!("accounts in this provider is not an array of accounts"),
    }

    let updated = doc.to_string();
    Ok((updated != text).then_some(updated))
}

/// Give every account in `ranks` the matching `priority`. If the provider
/// lists no accounts yet, the gateway was pooling the whole store, and an
/// explicit list would silently shrink that to what we write — so every name
/// in `pool_names` is listed first.
pub fn set_priorities(
    text: &str,
    provider: &str,
    ranks: &[(String, u32)],
    pool_names: &[String],
) -> anyhow::Result<Option<String>> {
    let mut doc = parse(text)?;
    let table = provider_table(&mut doc, provider)?;
    if table
        .get("accounts")
        .map(names_of)
        .unwrap_or_default()
        .is_empty()
    {
        for name in pool_names {
            push_entry(table, name)?;
        }
    }
    for (name, rank) in ranks {
        if !table
            .get("accounts")
            .map(names_of)
            .unwrap_or_default()
            .contains(name)
        {
            push_entry(table, name)?;
        }
        let entry = entry_mut(accounts_item(table), name)
            .ok_or_else(|| anyhow!("account {name:?} vanished from the config while editing"))?;
        entry.insert("priority", value(i64::from(*rank)));
    }
    let updated = doc.to_string();
    Ok((updated != text).then_some(updated))
}

/// Back to balanced: drop every `priority` so the accounts share the default
/// tier and the gateway balances them itself.
pub fn clear_priorities(text: &str, provider: &str) -> anyhow::Result<Option<String>> {
    let mut doc = parse(text)?;
    let table = provider_table(&mut doc, provider)?;
    if let Some(accounts) = table.get_mut("accounts") {
        match accounts {
            Item::ArrayOfTables(tables) => {
                for entry in tables.iter_mut() {
                    entry.remove("priority");
                }
            }
            Item::Value(Value::Array(array)) => {
                for entry in array.iter_mut().filter_map(Value::as_inline_table_mut) {
                    entry.remove("priority");
                }
            }
            _ => {}
        }
    }
    let updated = doc.to_string();
    Ok((updated != text).then_some(updated))
}

#[cfg(test)]
mod tests {
    use super::*;

    const AOT: &str = r#"# my config
[server]
bind = "127.0.0.1:3001"

[providers.anthropic]
auth = "claude_oauth" # keep me

[[providers.anthropic.accounts]]
name = "alpha"

[[providers.anthropic.accounts]]
name = "beta"
priority = 7

[providers.codex]
auth = "chatgpt_oauth"
"#;

    fn names(text: &str, provider: &str) -> Vec<String> {
        let doc: DocumentMut = text.parse().unwrap();
        doc["providers"][provider]
            .get("accounts")
            .map(names_of)
            .unwrap_or_default()
    }

    fn priorities(text: &str) -> Vec<(String, Option<i64>)> {
        let value: toml::Value = toml::from_str(text).unwrap();
        value["providers"]["anthropic"]["accounts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| {
                (
                    a["name"].as_str().unwrap().to_string(),
                    a.get("priority").and_then(toml::Value::as_integer),
                )
            })
            .collect()
    }

    #[test]
    fn include_appends_a_name_only_entry_and_keeps_comments() {
        let out = include_account(AOT, "anthropic", "gamma").unwrap().unwrap();
        assert_eq!(names(&out, "anthropic"), ["alpha", "beta", "gamma"]);
        assert!(out.contains("# my config"));
        assert!(out.contains("# keep me"));
        toml::from_str::<toml::Value>(&out).expect("still valid TOML");
    }

    #[test]
    fn include_is_a_no_op_for_a_listed_name_or_a_store_scanned_provider() {
        assert_eq!(include_account(AOT, "anthropic", "alpha").unwrap(), None);
        // `codex` lists no accounts: the gateway already pools the whole store.
        assert_eq!(include_account(AOT, "codex", "new").unwrap(), None);
    }

    #[test]
    fn include_handles_an_inline_accounts_array() {
        let text =
            "[providers.anthropic]\nauth = \"claude_oauth\"\naccounts = [{ name = \"a\" }]\n";
        let out = include_account(text, "anthropic", "b").unwrap().unwrap();
        assert_eq!(names(&out, "anthropic"), ["a", "b"]);
        toml::from_str::<toml::Value>(&out).unwrap();
    }

    #[test]
    fn remove_drops_an_explicit_account_but_never_turns_the_list_into_scan_all() {
        let out = remove_account(AOT, "anthropic", "alpha").unwrap().unwrap();
        assert_eq!(names(&out, "anthropic"), ["beta"]);
        assert!(out.contains("# my config"));
        assert_eq!(remove_account(AOT, "anthropic", "missing").unwrap(), None);
        assert_eq!(
            remove_account(AOT, "codex", "anything").unwrap(),
            None,
            "a store-scanned provider needs no config edit"
        );

        let one =
            "[providers.anthropic]\nauth = \"claude_oauth\"\naccounts = [{ name = \"only\" }]\n";
        let error = remove_account(one, "anthropic", "only")
            .unwrap_err()
            .to_string();
        assert!(error.contains("last explicitly listed account"), "{error}");
        assert!(error.contains("scan every stored account"), "{error}");
    }

    #[test]
    fn remove_refuses_inline_credential_sources() {
        let path = "[providers.anthropic]\nauth = \"claude_oauth\"\naccounts = [{ name = \"custom\", credentials = \"/tmp/custom.json\" }, { name = \"other\" }]\n";
        let error = remove_account(path, "anthropic", "custom")
            .unwrap_err()
            .to_string();
        assert!(error.contains("inline credential source"), "{error}");

        let env = "[providers.anthropic]\nauth = \"claude_oauth\"\n[[providers.anthropic.accounts]]\nname = \"custom\"\ntoken_env = \"TOKEN\"\n[[providers.anthropic.accounts]]\nname = \"other\"\n";
        let error = remove_account(env, "anthropic", "custom")
            .unwrap_err()
            .to_string();
        assert!(error.contains("shunt-managed store accounts"), "{error}");
    }

    #[test]
    fn remove_handles_an_inline_accounts_array_without_reformatting_the_provider() {
        let text = "[providers.anthropic]\nauth = \"claude_oauth\"\naccounts = [{ name = \"a\" }, { name = \"b\", priority = 2 }]\n";
        let out = remove_account(text, "anthropic", "a").unwrap().unwrap();
        assert_eq!(names(&out, "anthropic"), ["b"]);
        assert!(out.contains("priority = 2"));
        toml::from_str::<toml::Value>(&out).unwrap();
    }

    #[test]
    fn set_priorities_writes_ranks_and_leaves_the_rest() {
        let ranks = vec![("beta".to_string(), 1), ("alpha".to_string(), 2)];
        let out = set_priorities(AOT, "anthropic", &ranks, &[])
            .unwrap()
            .unwrap();
        assert_eq!(
            priorities(&out),
            [("alpha".into(), Some(2)), ("beta".into(), Some(1))]
        );
        assert!(out.contains("# keep me"));
    }

    #[test]
    fn set_priorities_lists_the_whole_store_first_when_nothing_was_listed() {
        let pool = vec!["x".to_string(), "y".to_string(), "z".to_string()];
        let ranks = vec![
            ("z".to_string(), 1),
            ("x".to_string(), 2),
            ("y".to_string(), 3),
        ];
        let out = set_priorities(AOT, "codex", &ranks, &pool)
            .unwrap()
            .unwrap();
        let value: toml::Value = toml::from_str(&out).unwrap();
        let accounts = value["providers"]["codex"]["accounts"].as_array().unwrap();
        assert_eq!(accounts.len(), 3, "no account dropped out of the pool");
        let rank_of = |n: &str| {
            accounts
                .iter()
                .find(|a| a["name"].as_str() == Some(n))
                .unwrap()["priority"]
                .as_integer()
        };
        assert_eq!(
            (rank_of("z"), rank_of("x"), rank_of("y")),
            (Some(1), Some(2), Some(3))
        );
    }

    #[test]
    fn set_priorities_is_idempotent() {
        let ranks = vec![("alpha".to_string(), 1), ("beta".to_string(), 2)];
        let once = set_priorities(AOT, "anthropic", &ranks, &[])
            .unwrap()
            .unwrap();
        assert_eq!(
            set_priorities(&once, "anthropic", &ranks, &[]).unwrap(),
            None
        );
    }

    #[test]
    fn clear_priorities_removes_only_the_priority_keys() {
        let out = clear_priorities(AOT, "anthropic").unwrap().unwrap();
        assert_eq!(
            priorities(&out),
            [("alpha".into(), None), ("beta".into(), None)]
        );
        assert_eq!(clear_priorities(&out, "anthropic").unwrap(), None);
    }

    #[test]
    fn refuses_providers_it_cannot_edit_with_a_reason() {
        let upstreams = "[[upstreams]]\nname = \"main\"\nprovider = \"anthropic\"\n";
        let error = include_account(upstreams, "main", "x")
            .unwrap_err()
            .to_string();
        assert!(error.contains("[[upstreams]]"), "{error}");
        let error = include_account(AOT, "missing", "x")
            .unwrap_err()
            .to_string();
        assert!(error.contains("no [providers.missing]"), "{error}");
        assert!(include_account("not toml [", "anthropic", "x").is_err());
    }

    #[test]
    fn apply_writes_atomically_and_skips_unchanged_files() {
        let dir = std::env::temp_dir().join(format!("shunt-tui-edit-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("shunt.toml");
        std::fs::write(&path, AOT).unwrap();
        assert!(apply(&path, |t| include_account(t, "anthropic", "gamma")).unwrap());
        assert!(!apply(&path, |t| include_account(t, "anthropic", "gamma")).unwrap());
        assert!(std::fs::read_to_string(&path).unwrap().contains("gamma"));
        let _ = std::fs::remove_dir_all(dir);
    }
}

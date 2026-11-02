//! `[server.auth.clients]` — pin an inbound client to a subset of each pooled
//! provider's accounts. See `docs/m4-inbound-auth.md`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{Config, ConfigError, ProviderConfig};

/// One `[server.auth.clients.<name>]` entry. The key is the client name: a
/// static token's name from the tokens env var, or a verified JWT email.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ClientPolicy {
    /// Provider name → the account names this client may use on it. A pooled
    /// provider not listed here keeps all of its accounts.
    #[serde(default)]
    pub accounts: BTreeMap<String, Vec<String>>,
}

impl ProviderConfig {
    /// The pool's account names when the config alone fixes them (inline
    /// `accounts` and `account_scope`); `None` when the pool is whatever the
    /// account store holds.
    pub fn configured_account_names(&self) -> Option<Vec<String>> {
        if self.accounts.is_empty() && self.account_scope.is_empty() {
            return None;
        }
        let inline = self.accounts.iter().map(|account| account.name.clone());
        Some(self.account_scope.iter().cloned().chain(inline).collect())
    }

    /// [`Self::configured_account_names`], or else the account store's names
    /// read offline. `None` for a provider without a pool or with an
    /// unreadable store, which the request path reports on its own.
    pub fn pool_account_names(&self) -> Option<Vec<String>> {
        if let Some(names) = self.configured_account_names() {
            return Some(names);
        }
        let (_, _, scan) = self.account_store()?;
        match scan() {
            Ok(accounts) => Some(accounts.into_iter().map(|account| account.name).collect()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Some(Vec::new()),
            Err(_) => None,
        }
    }
}

impl Config {
    /// The account names `client` may use on `provider`, or `None` when the
    /// client is not pinned there and may use every account.
    pub fn client_accounts(&self, client: &str, provider: &str) -> Option<&[String]> {
        self.server
            .auth
            .as_ref()?
            .clients
            .get(client)?
            .accounts
            .get(provider)
            .map(Vec::as_slice)
    }

    /// Reject a `[server.auth.clients]` entry that could never select an
    /// account. `pool_names` lists a pinned provider's account names; a `None`
    /// skips the name check for that provider. Naming the provider that owns a
    /// misplaced account reads the account stores offline, on the error path
    /// only.
    pub fn validate_client_accounts(
        &self,
        pool_names: impl Fn(&ProviderConfig) -> Option<Vec<String>>,
    ) -> Result<(), ConfigError> {
        let Some(auth) = &self.server.auth else {
            return Ok(());
        };
        for (client, policy) in &auth.clients {
            let reject = |message: String| {
                Err(ConfigError::InvalidClientAccounts {
                    client: client.clone(),
                    message,
                })
            };
            if policy.accounts.is_empty() {
                return reject(
                    "`accounts` is empty; pin at least one provider, or remove the entry to allow all accounts"
                        .to_string(),
                );
            }
            for (provider, names) in &policy.accounts {
                if names.is_empty() {
                    return reject(format!(
                        "accounts.{provider} is empty; remove it to allow all accounts"
                    ));
                }
                let Some(config) = self.providers.get(provider) else {
                    return reject(format!("unknown provider '{provider}'"));
                };
                if config.store_family().is_none() {
                    return reject(format!(
                        "provider '{provider}' has no account pool; pinning needs auth = \"claude_oauth\", \"chatgpt_oauth\", \"kimi_oauth\" or \"antigravity_oauth\""
                    ));
                }
                let Some(known) = pool_names(config) else {
                    continue;
                };
                let Some(name) = names.iter().find(|name| !known.contains(name)) else {
                    continue;
                };
                let owner = self.providers.iter().find(|(other, config)| {
                    *other != provider
                        && config
                            .pool_account_names()
                            .is_some_and(|names| names.contains(name))
                });
                return reject(match owner {
                    Some((owner, _)) => {
                        format!("account '{name}' belongs to provider '{owner}', not '{provider}'")
                    }
                    None => format!("provider '{provider}' has no account '{name}'"),
                });
            }
        }
        Ok(())
    }
}

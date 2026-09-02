//! Production OAuth2 authentication: PKCE + OS vault.
//!
//! The lessons of the Phase 0 spikes, in library quality: never a
//! password, refresh token in the Windows Credential Manager, systematic
//! verification of the **granted scopes** (granular consent issues a token
//! even without the mail box ticked), silent reconnection at the next launch.
//!
//! The journey is ONE, whatever the provider; what distinguishes them is
//! described as data in [`provider`].

mod flow;
mod provider;

use oauth2::TokenResponse;
use oauth2::basic::BasicTokenResponse;

pub use flow::AuthError;
pub use provider::{
    ALL as PROVIDERS, ClientSecret, Endpoint, GOOGLE, Identity, MICROSOFT, Provider,
    for_account_kind,
};

const KEYRING_SERVICE: &str = "wind-mail";
/// The service from before the Wind switch (PLAN-WIND E3). Every vault read
/// goes through [`vault_read`], which falls back on it and migrates the
/// entry found — the bridge lives as long as Discovery workstations exist.
const OLD_KEYRING_SERVICE: &str = "discovery-mail";
/// Entry inherited from Phase 2 (a single account) — read as a fallback
/// then migrated to the per-account entry: no re-authentication after the
/// multi-account update.
const KEYRING_REFRESH_LEGACY: &str = "gmail-refresh-token";

/// Authenticated session: enough to open an IMAP XOAUTH2 connection.
/// The access token expires (~1 h): re-authenticate silently when needed.
///
/// The provider travels with the session: it is what carries the servers
/// to reach, plus an application constant.
#[derive(Clone)]
pub struct Authenticated {
    pub provider: &'static Provider,
    pub email: String,
    pub access_token: String,
}

/// E8: never the token in a `{:?}` — a future diagnostic `eprintln!` must
/// not be able to trace it.
impl std::fmt::Debug for Authenticated {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Authenticated")
            .field("provider", &self.provider.account_kind)
            .field("email", &self.email)
            .field("access_token", &"<masked>")
            .finish()
    }
}

/// Credentials of a generic IMAP/SMTP account (server, port, password).
/// The password is in memory only during the session; it is read from the
/// OS vault at startup.
#[derive(Clone)]
pub struct GenericCredentials {
    pub email: String,
    pub username: String,
    pub password: String,
    pub imap_host: String,
    pub imap_port: u16,
    pub smtp_host: String,
    pub smtp_port: u16,
}

/// E8: never the password in a `{:?}`.
impl std::fmt::Debug for GenericCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GenericCredentials")
            .field("email", &self.email)
            .field("username", &self.username)
            .field("password", &"<masked>")
            .field("imap_host", &self.imap_host)
            .field("imap_port", &self.imap_port)
            .field("smtp_host", &self.smtp_host)
            .field("smtp_port", &self.smtp_port)
            .finish()
    }
}

/// Session of a connected account, whatever its authentication method. It
/// is what circulates in the desktop's application state.
#[derive(Debug, Clone)]
pub enum AccountSession {
    /// Account authenticated by OAuth2, whatever the provider.
    OAuth(Authenticated),
    Generic(GenericCredentials),
}

impl AccountSession {
    pub fn email(&self) -> &str {
        match self {
            AccountSession::OAuth(auth) => &auth.email,
            AccountSession::Generic(creds) => &creds.email,
        }
    }
}

/// OAuth2 authenticator of ONE provider.
#[derive(Clone)]
pub struct Authenticator {
    provider: &'static Provider,
    client_id: String,
    /// `None` for a public client (Microsoft): presenting a secret would
    /// get the exchange refused.
    client_secret: Option<String>,
}

/// The resolution order of an OAuth credential (D1, PLAN-RETOURS-9): the
/// RUNTIME variable wins — it is the lever of the dev workstations and of
/// the e2e isolation —, the value embedded at the release build only
/// speaks in its absence. A variable set but empty does not count.
fn resolve_credential(runtime: Option<String>, embedded: Option<&str>) -> Option<String> {
    runtime
        .filter(|v| !v.is_empty())
        .or_else(|| embedded.map(str::to_string))
}

impl Authenticator {
    pub fn new(
        provider: &'static Provider,
        client_id: impl Into<String>,
        client_secret: Option<String>,
    ) -> Self {
        Self {
            provider,
            client_id: client_id.into(),
            client_secret,
        }
    }

    pub fn provider(&self) -> &'static Provider {
        self.provider
    }

    /// Configuration by environment variables `{PREFIX}_CLIENT_ID` and
    /// `{PREFIX}_CLIENT_SECRET`, with a fallback on the credentials
    /// embedded at the release build (D1, PLAN-RETOURS-9). The values never
    /// live in the repository; in a public release the user has nothing to
    /// set, on a dev workstation the variable keeps serving.
    pub fn from_env(provider: &'static Provider) -> Result<Self, AuthError> {
        let id_var = format!("{}_CLIENT_ID", provider.env_prefix);
        let client_id =
            resolve_credential(std::env::var(&id_var).ok(), provider.embedded_client_id)
                .ok_or_else(|| {
                    // The reader may be a tester (binary built outside
                    // make-release.ps1): no more "terminal"-only instruction —
                    // the first remedy stated is an official version (review
                    // 2026-08-23, the PLAN's promise kept).
                    AuthError::Config(format!(
                        "OAuth credentials absent from this binary — install an \
                 official version of Wind; in development, set {id_var}"
                    ))
                })?;
        let client_secret = match provider.client_secret {
            ClientSecret::Required => {
                let secret_var = format!("{}_CLIENT_SECRET", provider.env_prefix);
                Some(
                    resolve_credential(
                        std::env::var(&secret_var).ok(),
                        provider.embedded_client_secret,
                    )
                    .ok_or_else(|| {
                        AuthError::Config(format!(
                            "OAuth credentials incomplete in this binary — install an \
                             official version of Wind; in development, set {secret_var}"
                        ))
                    })?,
                )
            }
            ClientSecret::Forbidden => None,
        };
        Ok(Self::new(provider, client_id, client_secret))
    }

    /// Shortcut of the historical provider — the only one wired to the UI
    /// to this day.
    pub fn google_from_env() -> Result<Self, AuthError> {
        Self::from_env(&GOOGLE)
    }

    /// Reconnection without interaction of ONE account: reads its vault
    /// entry (one per email), with a fallback on the entry inherited from
    /// Phase 2 — migrated to the per-account entry on the way. Fails if
    /// there is no token (→ [`Self::authenticate_interactive`]).
    pub fn authenticate_silent(&self, email: &str) -> Result<Authenticated, AuthError> {
        let (refresh, from_legacy) = match vault_read(&vault_key(self.provider, email)) {
            Ok(token) => (token, false),
            Err(keyring::Error::NoEntry) => {
                let legacy = vault_read(KEYRING_REFRESH_LEGACY).map_err(|err| match err {
                    // §6.8: never the address in an error — it ends up in
                    // wind.log (PLAN-AUDIT-V1 review). The account is
                    // recognized by its id, traced by the caller.
                    keyring::Error::NoEntry => {
                        AuthError::Vault("no token in the vault for this account".to_string())
                    }
                    other => AuthError::Vault(other.to_string()),
                })?;
                (legacy, true)
            }
            Err(other) => return Err(AuthError::Vault(other.to_string())),
        };
        let client = self.client()?;
        let http = flow::http_client()?;
        let tokens = flow::refresh_access_token(&client, &http, refresh.clone())?;
        flow::ensure_mail_scope(self.provider, &tokens)?;
        // E8 (audit S2): Azure AD returns a NEW refresh token at every
        // exchange and expires the old one (90 days by default); Google does
        // it occasionally. Throwing it away was a deferred silent
        // disconnection. If it changes, it replaces the old one in the vault.
        let renewed = tokens
            .refresh_token()
            .map(|token| token.secret().clone())
            .filter(|new| *new != refresh);
        let account = self.finish(&http, &tokens, Some(email), renewed)?;
        if from_legacy {
            // Vault migration: the entry becomes per-account, under the REAL
            // email of the token (the one the provider confirms).
            self.vault(&account.email)?
                .set_password(&refresh)
                .map_err(|err| AuthError::Vault(err.to_string()))?;
            let _ = legacy_vault()?.delete_credential();
        }
        Ok(account)
    }

    /// Phase 2 legacy reconnection: when the database does not yet know any
    /// account, the unkeyed vault entry may reveal one — it is then
    /// migrated. The email comes back from the token itself.
    ///
    /// This path is Google's own: Phase 2 only knew it.
    pub fn authenticate_silent_legacy(&self) -> Result<Authenticated, AuthError> {
        let refresh = vault_read(KEYRING_REFRESH_LEGACY).map_err(|err| match err {
            keyring::Error::NoEntry => AuthError::Vault("no registered account".to_string()),
            other => AuthError::Vault(other.to_string()),
        })?;
        let client = self.client()?;
        let http = flow::http_client()?;
        let tokens = flow::refresh_access_token(&client, &http, refresh.clone())?;
        flow::ensure_mail_scope(self.provider, &tokens)?;
        let account = self.finish(&http, &tokens, None, None)?;
        self.vault(&account.email)?
            .set_password(&refresh)
            .map_err(|err| AuthError::Vault(err.to_string()))?;
        let _ = legacy_vault()?.delete_credential();
        Ok(account)
    }

    /// Full journey: browser → consent → loopback redirect → tokens. The
    /// refresh token is stored in the OS vault.
    ///
    /// `declared_email` is only used by the providers that do not deliver
    /// the account's identity ([`Identity::Declared`]).
    pub fn authenticate_interactive(
        &self,
        declared_email: Option<&str>,
    ) -> Result<Authenticated, AuthError> {
        let client = self.client()?;
        let http = flow::http_client()?;
        let tokens = flow::interactive_tokens(self.provider, client, &http)?;
        flow::ensure_mail_scope(self.provider, &tokens)?;
        let refresh = tokens.refresh_token().map(|token| token.secret().clone());
        self.finish(&http, &tokens, declared_email, refresh)
    }

    /// Forgets ONE account: removes its refresh token from the vault.
    pub fn forget(&self, email: &str) -> Result<(), AuthError> {
        vault_forget(&vault_key(self.provider, email))
            .map_err(|err| AuthError::Vault(err.to_string()))
    }

    fn client(&self) -> Result<flow::OauthClient, AuthError> {
        flow::oauth_client(
            self.provider,
            &self.client_id,
            self.client_secret.as_deref(),
        )
    }

    fn vault(&self, email: &str) -> Result<keyring::Entry, AuthError> {
        keyring::Entry::new(KEYRING_SERVICE, &vault_key(self.provider, email))
            .map_err(|err| AuthError::Vault(err.to_string()))
    }

    /// At a provider that delivers the identity, the email is only known
    /// AFTER the token exchange: the refresh is therefore stored in the
    /// account's entry once the identity is confirmed.
    fn finish(
        &self,
        http: &flow::HttpClient,
        tokens: &BasicTokenResponse,
        declared_email: Option<&str>,
        store_refresh: Option<String>,
    ) -> Result<Authenticated, AuthError> {
        let access_token = tokens.access_token().secret().clone();
        let email = flow::resolve_email(self.provider, http, &access_token, declared_email)?;
        if let Some(refresh) = store_refresh {
            self.vault(&email)?
                .set_password(&refresh)
                .map_err(|err| AuthError::Vault(err.to_string()))?;
        }
        Ok(Authenticated {
            provider: self.provider,
            email,
            access_token,
        })
    }
}

/// Name of the vault entry for an account's refresh token.
///
/// **Never change without a migration.** This name is the only thing that
/// ties the application to the tokens already stored on the user's
/// machine: changing it breaks no test but forces a silent
/// re-authentication of every account. That is why Google's prefix stays
/// `gmail`, inherited from Phase 2.
fn vault_key(provider: &Provider, email: &str) -> String {
    format!("{}-refresh:{email}", provider.vault_prefix)
}

fn legacy_vault() -> Result<keyring::Entry, AuthError> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_REFRESH_LEGACY)
        .map_err(|err| AuthError::Vault(err.to_string()))
}

/// Reads a vault entry under the Wind service, with a fallback on the
/// Discovery service from before the switch (PLAN-WIND E3): the entry
/// found is copied under `wind-mail` then removed from `discovery-mail` —
/// nobody reconnects an account for a rename. Same gesture as the Phase 2
/// migration of [`Authenticator::authenticate_silent`]: migrate on read. A
/// failed copy leaves the old entry in place — the next read will retry.
fn vault_read(key: &str) -> Result<String, keyring::Error> {
    let new = keyring::Entry::new(KEYRING_SERVICE, key)?;
    match new.get_password() {
        Err(keyring::Error::NoEntry) => {
            let old = keyring::Entry::new(OLD_KEYRING_SERVICE, key)?;
            let secret = old.get_password()?;
            new.set_password(&secret)?;
            let _ = old.delete_credential();
            Ok(secret)
        }
        other => other,
    }
}

/// Forgets an entry under BOTH services: a removed secret must not survive
/// under the old name. An absent entry is not an error — forgetting is
/// repeatable.
fn vault_forget(key: &str) -> Result<(), keyring::Error> {
    for service in [KEYRING_SERVICE, OLD_KEYRING_SERVICE] {
        match keyring::Entry::new(service, key)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(err) => return Err(err),
        }
    }
    Ok(())
}

const KEYRING_GENERIC_PASSWORD: &str = "generic-password";

fn generic_vault(email: &str) -> Result<keyring::Entry, AuthError> {
    keyring::Entry::new(
        KEYRING_SERVICE,
        &format!("{KEYRING_GENERIC_PASSWORD}:{email}"),
    )
    .map_err(|err| AuthError::Vault(err.to_string()))
}

/// Stores the password of a generic IMAP/SMTP account in the vault.
pub fn store_generic_password(email: &str, password: &str) -> Result<(), AuthError> {
    generic_vault(email)?
        .set_password(password)
        .map_err(|err| AuthError::Vault(err.to_string()))
}

/// Forgets the secrets of ONE account in the vault, whatever its
/// authentication mode (`account_kind`: `"imap"` for a generic account,
/// otherwise the OAuth provider).
///
/// Requires NO OAuth configuration: the name of the entry only depends on
/// the provider and the address — removing an account must never fail
/// because a `CLIENT_ID` is missing from the environment. An already
/// absent entry is not an error: the removal is repeatable.
pub fn forget_credentials(account_kind: &str, email: &str) -> Result<(), AuthError> {
    let key = match account_kind {
        "imap" => format!("{KEYRING_GENERIC_PASSWORD}:{email}"),
        kind => {
            let provider = provider::for_account_kind(kind)
                .ok_or_else(|| AuthError::Config(format!("unknown provider: {kind}")))?;
            vault_key(provider, email)
        }
    };
    vault_forget(&key).map_err(|err| AuthError::Vault(err.to_string()))
}

/// Fetches the password of a generic IMAP/SMTP account from the vault.
pub fn fetch_generic_password(email: &str) -> Result<String, AuthError> {
    vault_read(&format!("{KEYRING_GENERIC_PASSWORD}:{email}")).map_err(|err| match err {
        keyring::Error::NoEntry => AuthError::Vault(format!("no password for {email}")),
        other => AuthError::Vault(other.to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// D1 (PLAN-RETOURS-9): the runtime variable keeps priority — it is what
    /// serves on a dev workstation and what the e2e harness purges; the
    /// value embedded at the release build only speaks in its absence. With
    /// nothing, no credential: the error is still due.
    #[test]
    fn the_runtime_variable_wins_over_the_embedded_value() {
        assert_eq!(
            resolve_credential(Some("runtime".into()), Some("embedded")),
            Some("runtime".to_string())
        );
        assert_eq!(
            resolve_credential(None, Some("embedded")),
            Some("embedded".to_string())
        );
        assert_eq!(resolve_credential(None, None), None);
        // A variable set but empty does not count: `setx VAR ""` leaves a
        // shell that would mask the embedded value.
        assert_eq!(
            resolve_credential(Some(String::new()), Some("embedded")),
            Some("embedded".to_string())
        );
    }

    /// Characterization test, written BEFORE the generalization per
    /// provider: it freezes the vault entry names.
    ///
    /// No test can fail if they are renamed — the vault is in the OS, not in
    /// the repository. The symptom would be silent and deferred: every
    /// already connected account would ask for consent again at the next
    /// launch. Hence this pin.
    #[test]
    fn vault_entry_names_are_frozen() {
        assert_eq!(
            vault_key(&GOOGLE, "moi@exemple.fr"),
            "gmail-refresh:moi@exemple.fr"
        );
        // The service changed ONCE, with its migration (PLAN-WIND E3,
        // W-D1): `vault_read` falls back on the old service and migrates the
        // entry found. Both names stay pinned together — removing the old
        // one would cut the bridge for the Discovery workstations not yet
        // relaunched.
        assert_eq!(KEYRING_SERVICE, "wind-mail");
        assert_eq!(OLD_KEYRING_SERVICE, "discovery-mail");
        assert_eq!(KEYRING_REFRESH_LEGACY, "gmail-refresh-token");
        assert_eq!(KEYRING_GENERIC_PASSWORD, "generic-password");
    }

    /// The Discovery → Wind bridge against the REAL OS vault — ignored by
    /// default (the suite must not write into the Credential Manager of a
    /// CI runner): `cargo test -p mail-auth -- --ignored` on a Windows
    /// workstation. Unique names per process, cleanup at the end of the run.
    #[test]
    #[ignore]
    fn the_vault_bridge_migrates_discovery_to_wind() {
        let key = format!("wind-test-bridge:{}", std::process::id());
        let old = keyring::Entry::new(OLD_KEYRING_SERVICE, &key).unwrap();
        old.set_password("bridge-secret").unwrap();

        // First read: found under Discovery, copied under Wind, removed from
        // the old service.
        assert_eq!(vault_read(&key).unwrap(), "bridge-secret");
        let new = keyring::Entry::new(KEYRING_SERVICE, &key).unwrap();
        assert_eq!(new.get_password().unwrap(), "bridge-secret");
        assert!(matches!(old.get_password(), Err(keyring::Error::NoEntry)));

        // Re-read: the new path, without fallback.
        assert_eq!(vault_read(&key).unwrap(), "bridge-secret");

        // Forgetting purges both services and stays repeatable.
        vault_forget(&key).unwrap();
        vault_forget(&key).unwrap();
        assert!(matches!(vault_read(&key), Err(keyring::Error::NoEntry)));
    }

    /// Two providers for the same address must never write into the same
    /// entry: the second would overwrite the first's token, and the failure
    /// — a silent disconnection — would come much later.
    #[test]
    fn two_providers_never_share_a_vault_entry() {
        assert_ne!(
            vault_key(&GOOGLE, "moi@exemple.fr"),
            vault_key(&MICROSOFT, "moi@exemple.fr")
        );
    }
}

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

pub use flow::{AuthError, AuthorizationLink, ConsentControl};
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
        self.silent_with_os_vault(Some(email))
    }

    /// Adopt the historical, unkeyed Google account when no account is registered.
    pub fn authenticate_silent_legacy(&self) -> Result<Authenticated, AuthError> {
        self.silent_with_os_vault(None)
    }

    fn silent_with_os_vault(&self, email: Option<&str>) -> Result<Authenticated, AuthError> {
        let client = self.client()?;
        let http = flow::http_client()?;
        self.silent_using(
            email,
            &OsTokenVault,
            |refresh| flow::refresh_access_token(&client, &http, refresh),
            |tokens, declared| {
                flow::resolve_email(
                    self.provider,
                    &http,
                    tokens.access_token().secret(),
                    declared,
                )
            },
        )
    }

    fn silent_using(
        &self,
        email: Option<&str>,
        vault: &dyn TokenVault,
        exchange: impl FnOnce(String) -> Result<BasicTokenResponse, AuthError>,
        resolve: impl FnOnce(&BasicTokenResponse, Option<&str>) -> Result<String, AuthError>,
    ) -> Result<Authenticated, AuthError> {
        let current = email.map(|email| vault.read(&vault_key(self.provider, email)));
        let (refresh, from_legacy) = match current {
            Some(Ok(token)) => (token, false),
            None | Some(Err(keyring::Error::NoEntry))
                if self.provider.account_kind != GOOGLE.account_kind =>
            {
                return Err(AuthError::Vault(
                    "no token in the vault for this account".into(),
                ));
            }
            None | Some(Err(keyring::Error::NoEntry)) => (
                vault
                    .read(KEYRING_REFRESH_LEGACY)
                    .map_err(|err| match err {
                        keyring::Error::NoEntry => {
                            AuthError::Vault("no token in the vault for this account".into())
                        }
                        other => AuthError::Vault(other.to_string()),
                    })?,
                true,
            ),
            Some(Err(other)) => return Err(AuthError::Vault(other.to_string())),
        };
        let tokens = exchange(refresh.clone())?;
        flow::ensure_mail_scope(self.provider, &tokens)?;
        let resolved = resolve(&tokens, email)?;
        if email.is_some_and(|requested| !requested.eq_ignore_ascii_case(&resolved)) {
            return Err(AuthError::OAuth(
                "the token belongs to another account".into(),
            ));
        }
        let key = vault_key(self.provider, email.unwrap_or(&resolved));
        let effective = tokens
            .refresh_token()
            .map_or(refresh.as_str(), |token| token.secret());
        if from_legacy || effective != refresh {
            vault.write(&key, effective)?;
        }
        if from_legacy {
            // Copy the effective token before deleting the recoverable source.
            let _ = vault.forget(KEYRING_REFRESH_LEGACY);
        }
        Ok(Authenticated {
            provider: self.provider,
            email: email.unwrap_or(&resolved).to_string(),
            access_token: tokens.access_token().secret().clone(),
        })
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
        self.authenticate_interactive_controlled(declared_email, None, &ConsentControl::default())
    }

    /// Cancellation is accepted until credential publication starts. A failed
    /// browser launch exposes the same live authorization through `control`.
    pub fn authenticate_interactive_controlled(
        &self,
        declared_email: Option<&str>,
        expected_email: Option<&str>,
        control: &ConsentControl,
    ) -> Result<Authenticated, AuthError> {
        let client = self.client()?;
        let http = flow::http_client()?;
        let tokens = flow::interactive_tokens(self.provider, client, &http, control)?;
        flow::ensure_mail_scope(self.provider, &tokens)?;
        control.check()?;
        let email = flow::resolve_email(
            self.provider,
            &http,
            tokens.access_token().secret(),
            declared_email,
        )?;
        self.publish_interactive(&tokens, email, expected_email, control, &OsTokenVault)
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

    fn publish_interactive(
        &self,
        tokens: &BasicTokenResponse,
        email: String,
        expected_email: Option<&str>,
        control: &ConsentControl,
        vault: &dyn TokenVault,
    ) -> Result<Authenticated, AuthError> {
        if expected_email.is_some_and(|expected| !expected.eq_ignore_ascii_case(&email)) {
            return Err(AuthError::OAuth(
                "consent belongs to another account; pick the account being reconnected".into(),
            ));
        }
        let email = expected_email.unwrap_or(&email).to_string();
        control.begin_publication()?;
        if let Some(refresh) = tokens.refresh_token() {
            vault.write(&vault_key(self.provider, &email), refresh.secret())?;
        }
        Ok(Authenticated {
            provider: self.provider,
            email,
            access_token: tokens.access_token().secret().clone(),
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

trait TokenVault {
    fn read(&self, key: &str) -> Result<String, keyring::Error>;
    fn write(&self, key: &str, token: &str) -> Result<(), AuthError>;
    fn forget(&self, key: &str) -> Result<(), AuthError>;
}

struct OsTokenVault;

impl TokenVault for OsTokenVault {
    fn read(&self, key: &str) -> Result<String, keyring::Error> {
        vault_read(key)
    }
    fn write(&self, key: &str, token: &str) -> Result<(), AuthError> {
        keyring::Entry::new(KEYRING_SERVICE, key)
            .and_then(|entry| entry.set_password(token))
            .map_err(|err| AuthError::Vault(err.to_string()))
    }
    fn forget(&self, key: &str) -> Result<(), AuthError> {
        vault_forget(key).map_err(|err| AuthError::Vault(err.to_string()))
    }
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

fn generic_slot_key(email: &str, slot: Option<&str>) -> Result<String, AuthError> {
    match slot {
        None => Ok(format!("{KEYRING_GENERIC_PASSWORD}:{email}")),
        Some(slot @ ("a" | "b")) => Ok(format!("generic-password-v1:{slot}:{email}")),
        _ => Err(AuthError::Config("invalid credential slot".into())),
    }
}

/// The caller serializes publication for this account. The old vault entry stays
/// referenced until `publish` commits the new slot with the connection settings.
pub fn replace_generic_password<T>(
    email: &str,
    current_slot: Option<&str>,
    password: &str,
    publish: impl FnOnce(&str) -> Result<T, AuthError>,
) -> Result<T, AuthError> {
    replace_generic_using(email, current_slot, password, &OsTokenVault, publish)
}

fn replace_generic_using<T>(
    email: &str,
    current_slot: Option<&str>,
    password: &str,
    vault: &dyn TokenVault,
    publish: impl FnOnce(&str) -> Result<T, AuthError>,
) -> Result<T, AuthError> {
    let previous = generic_slot_key(email, current_slot)?;
    let slot = if current_slot == Some("a") { "b" } else { "a" };
    let candidate = generic_slot_key(email, Some(slot))?;
    vault.write(&candidate, password)?;
    let result = publish(slot);
    if result.is_ok() {
        let _ = vault.forget(&previous);
    } else {
        let _ = vault.forget(&candidate);
    }
    result
}

pub fn fetch_generic_password_slot(email: &str, slot: Option<&str>) -> Result<String, AuthError> {
    vault_read(&generic_slot_key(email, slot)?).map_err(|err| AuthError::Vault(err.to_string()))
}

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
    if account_kind == "imap" {
        for slot in [Some("a"), Some("b"), None] {
            vault_forget(&generic_slot_key(email, slot)?)
                .map_err(|err| AuthError::Vault(err.to_string()))?;
        }
        return Ok(());
    }
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

    #[derive(Default)]
    struct MemoryVault {
        entries: std::cell::RefCell<std::collections::HashMap<String, String>>,
        writes: std::cell::RefCell<Vec<(String, String)>>,
        fail_write: bool,
        fail_delete: bool,
    }
    impl TokenVault for MemoryVault {
        fn read(&self, key: &str) -> Result<String, keyring::Error> {
            self.entries
                .borrow()
                .get(key)
                .cloned()
                .ok_or(keyring::Error::NoEntry)
        }
        fn write(&self, key: &str, token: &str) -> Result<(), AuthError> {
            if self.fail_write {
                return Err(AuthError::Vault("synthetic write failure".into()));
            }
            self.writes.borrow_mut().push((key.into(), token.into()));
            self.entries.borrow_mut().insert(key.into(), token.into());
            Ok(())
        }
        fn forget(&self, key: &str) -> Result<(), AuthError> {
            if self.fail_delete {
                return Err(AuthError::Vault("synthetic delete failure".into()));
            }
            self.entries.borrow_mut().remove(key);
            Ok(())
        }
    }
    #[test]
    fn interactive_wrong_identity_does_not_write_any_vault_entry() {
        let vault = MemoryVault::default();
        let auth = Authenticator::new(&GOOGLE, "fixture", None);
        let result = auth.publish_interactive(
            &refreshed_token(Some("new")),
            "other@example.fr".into(),
            Some("owner@example.fr"),
            &ConsentControl::default(),
            &vault,
        );
        assert!(result.is_err());
        assert!(vault.writes.borrow().is_empty());
    }

    #[test]
    fn cancelled_consent_does_not_publish_and_finished_consent_cannot_be_cancelled() {
        let vault = MemoryVault::default();
        let auth = Authenticator::new(&GOOGLE, "fixture", None);
        let control = ConsentControl::default();
        assert!(control.cancel());
        assert!(
            auth.publish_interactive(
                &refreshed_token(Some("new")),
                "owner@example.fr".into(),
                None,
                &control,
                &vault
            )
            .is_err()
        );
        assert!(vault.writes.borrow().is_empty());
        let control = ConsentControl::default();
        let session = auth
            .publish_interactive(
                &refreshed_token(Some("new")),
                "OWNER@example.fr".into(),
                Some("owner@example.fr"),
                &control,
                &vault,
            )
            .unwrap();
        assert_eq!(session.email, "owner@example.fr");
        assert!(!control.cancel());
    }

    #[test]
    fn generic_password_stays_recoverable_until_its_reference_commits() {
        for current in [None, Some("a"), Some("b")] {
            let vault = MemoryVault::default();
            let old_key = generic_slot_key("owner@example.fr", current).unwrap();
            vault
                .entries
                .borrow_mut()
                .insert(old_key.clone(), "old".into());
            let next = if current == Some("a") { "b" } else { "a" };
            let next_key = generic_slot_key("owner@example.fr", Some(next)).unwrap();
            let result: Result<(), _> =
                replace_generic_using("owner@example.fr", current, "new", &vault, |slot| {
                    assert_eq!(slot, next);
                    assert_eq!(vault.read(&old_key).unwrap(), "old");
                    assert_eq!(vault.read(&next_key).unwrap(), "new");
                    Err(AuthError::Config("synthetic SQL failure".into()))
                });
            assert!(result.is_err());
            assert_eq!(vault.read(&old_key).unwrap(), "old");
            assert!(vault.read(&next_key).is_err());
            let committed =
                replace_generic_using("owner@example.fr", current, "new", &vault, |slot| {
                    Ok(slot.to_string())
                })
                .unwrap();
            assert_eq!(committed, next);
            assert_eq!(vault.read(&next_key).unwrap(), "new");
            assert!(vault.read(&old_key).is_err());
        }
    }

    #[test]
    fn failed_staging_never_changes_the_database_reference() {
        let vault = MemoryVault {
            fail_write: true,
            ..MemoryVault::default()
        };
        let called = std::cell::Cell::new(false);
        assert!(
            replace_generic_using("owner@example.fr", None, "new", &vault, |_| {
                called.set(true);
                Ok(())
            })
            .is_err()
        );
        assert!(!called.get());
    }

    #[test]
    fn failed_slot_cleanup_keeps_the_published_password_and_slots_are_bounded() {
        let vault = MemoryVault {
            fail_delete: true,
            ..MemoryVault::default()
        };
        let mut current = None;
        for _ in 0..5 {
            current = Some(
                replace_generic_using(
                    "owner@example.fr",
                    current.as_deref(),
                    "new",
                    &vault,
                    |slot| Ok(slot.to_string()),
                )
                .unwrap(),
            );
            let key = generic_slot_key("owner@example.fr", current.as_deref()).unwrap();
            assert_eq!(vault.read(&key).unwrap(), "new");
        }
        assert_eq!(vault.entries.borrow().len(), 2);
        assert!(
            replace_generic_using(
                "owner@example.fr",
                Some("broken"),
                "new",
                &vault,
                |_| Ok(())
            )
            .is_err()
        );
    }

    fn legacy_fixture() -> MemoryVault {
        let vault = MemoryVault::default();
        vault
            .entries
            .borrow_mut()
            .insert(KEYRING_REFRESH_LEGACY.into(), "old".into());
        vault
    }
    fn refreshed_token(refresh: Option<&str>) -> BasicTokenResponse {
        serde_json::from_value(serde_json::json!({ "access_token": "synthetic-access",
            "token_type": "Bearer", "refresh_token": refresh }))
        .unwrap()
    }
    fn run_refresh(
        vault: &MemoryVault,
        requested: Option<&str>,
        refresh: Option<&str>,
    ) -> Result<Authenticated, AuthError> {
        Authenticator::new(&GOOGLE, "fixture", None).silent_using(
            requested,
            vault,
            |_| Ok(refreshed_token(refresh)),
            |_, _| Ok("owner@example.fr".into()),
        )
    }
    fn check_legacy_rotation(requested: Option<&str>) {
        let vault = legacy_fixture();
        run_refresh(&vault, requested, Some("rotated")).unwrap();
        assert_eq!(
            vault.read(&vault_key(&GOOGLE, "owner@example.fr")).unwrap(),
            "rotated"
        );
        assert!(matches!(
            vault.read(KEYRING_REFRESH_LEGACY),
            Err(keyring::Error::NoEntry)
        ));
        assert_eq!(
            vault.writes.borrow().len(),
            1,
            "publish only the effective token"
        );
    }
    #[test]
    fn legacy_rotation_is_preserved_for_a_registered_account() {
        check_legacy_rotation(Some("owner@example.fr"));
    }
    #[test]
    fn legacy_rotation_is_preserved_during_initial_adoption() {
        check_legacy_rotation(None);
    }
    #[test]
    fn a_legacy_token_without_rotation_is_still_migrated() {
        let vault = legacy_fixture();
        run_refresh(&vault, None, None).unwrap();
        assert_eq!(
            vault.read(&vault_key(&GOOGLE, "owner@example.fr")).unwrap(),
            "old"
        );
        assert_eq!(vault.writes.borrow().len(), 1);
    }
    #[test]
    fn failed_token_publication_keeps_the_legacy_source() {
        let mut vault = legacy_fixture();
        vault.fail_write = true;
        assert!(run_refresh(&vault, None, Some("rotated")).is_err());
        assert_eq!(vault.read(KEYRING_REFRESH_LEGACY).unwrap(), "old");
        assert_eq!(vault.entries.borrow().len(), 1);
    }
    #[test]
    fn failed_legacy_cleanup_does_not_undo_rotation() {
        let mut vault = legacy_fixture();
        vault.fail_delete = true;
        run_refresh(&vault, None, Some("rotated")).unwrap();
        assert_eq!(
            vault.read(&vault_key(&GOOGLE, "owner@example.fr")).unwrap(),
            "rotated"
        );
        let used = std::cell::RefCell::new(String::new());
        Authenticator::new(&GOOGLE, "fixture", None)
            .silent_using(
                Some("owner@example.fr"),
                &vault,
                |token| {
                    *used.borrow_mut() = token;
                    Ok(refreshed_token(None))
                },
                |_, _| Ok("owner@example.fr".into()),
            )
            .unwrap();
        assert_eq!(*used.borrow(), "rotated");
    }
    #[test]
    fn microsoft_never_tries_the_google_legacy_token() {
        for requested in [Some("owner@example.fr"), None] {
            let vault = legacy_fixture();
            let exchanged = std::cell::Cell::new(false);
            let result = Authenticator::new(&MICROSOFT, "fixture", None).silent_using(
                requested,
                &vault,
                |_| {
                    exchanged.set(true);
                    Ok(refreshed_token(Some("rotated")))
                },
                |_, _| Ok("owner@example.fr".into()),
            );
            assert!(result.is_err());
            assert!(!exchanged.get());
            assert!(vault.writes.borrow().is_empty());
        }
    }
    #[test]
    fn legacy_identity_mismatch_keeps_both_account_entries_untouched() {
        let vault = legacy_fixture();
        assert!(run_refresh(&vault, Some("someone-else@example.fr"), Some("rotated")).is_err());
        assert_eq!(vault.entries.borrow().len(), 1);
        assert_eq!(vault.read(KEYRING_REFRESH_LEGACY).unwrap(), "old");
        assert!(vault.writes.borrow().is_empty());
    }

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

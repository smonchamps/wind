//! What changes from one OAuth2 provider to the next — and nothing else.
//!
//! The rest of the journey (PKCE, loopback listener, CSRF check, OS vault,
//! silent reconnection) is common and lives in [`crate::flow`]. A provider
//! is described here as data; if it asked for code, the seam would be in
//! the wrong place.
//!
//! The Microsoft values are not inferred from the documentation: they come
//! from the [`spikes/microsoft`](../../../spikes/microsoft) spike, played
//! against a real account. The tests of this module pin them.

/// How the provider delivers the identity of the account.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Identity {
    /// JSON endpoint exposing an `email` field, called with the access
    /// token. That is the Google case, measured since Phase 0.
    Userinfo(&'static str),
    /// The provider does not deliver the email within the requested scopes:
    /// the user declares it when adding the account.
    ///
    /// Known lead to do without it on the Microsoft side: request
    /// `openid profile email` and read `https://graph.microsoft.com/oidc/userinfo`.
    /// **Not measured** — the spike never asked for those scopes. Until it
    /// is verified on a real account, we declare.
    Declared,
}

/// A desktop OAuth2 client is sometimes confidential, sometimes public.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientSecret {
    /// Google issues a secret even to installed applications.
    Required,
    /// PUBLIC client: PKCE only. Sending a secret would make Azure AD
    /// **refuse** the exchange.
    Forbidden,
}

/// A provider's mail server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Endpoint {
    pub host: &'static str,
    pub port: u16,
}

#[derive(Debug)]
pub struct Provider {
    /// Name shown to the user, including in error messages.
    pub name: &'static str,
    /// Prefix of the environment variables: `{prefix}_CLIENT_ID`.
    pub env_prefix: &'static str,
    /// Value stored in the `accounts.provider` column. **Never change
    /// without a migration**: the rows already written carry it, and an
    /// account whose key is no longer recognized becomes unconnectable.
    pub account_kind: &'static str,
    /// Prefix of the vault entry. **Never change without a migration**
    /// (see `vault_key`): this name ties the app to the tokens already stored.
    pub vault_prefix: &'static str,
    pub auth_url: &'static str,
    pub token_url: &'static str,
    pub scopes: &'static [&'static str],
    /// Fragment that must appear in a **granted** scope. Both providers
    /// issue a token even on partial consent: only the granted list counts
    /// (Phase 0 lesson, reconfirmed by the Microsoft spike).
    pub granted_scope_marker: &'static str,
    /// Microsoft distinguishes `localhost` from `127.0.0.1`: with the URI
    /// `http://localhost` registered, any port is accepted.
    pub redirect_host: &'static str,
    /// Authorization parameters specific to the provider.
    pub extra_auth_params: &'static [(&'static str, &'static str)],
    pub client_secret: ClientSecret,
    pub identity: Identity,
    pub imap: Endpoint,
    pub smtp: Endpoint,
    /// Credentials frozen at COMPILE time (D1, PLAN-RETOURS-9):
    /// `make-release.ps1` sets `WIND_RELEASE_*` before the two builds — an
    /// end user has no `setx` to do. Any other build (dev, tests, CI)
    /// embeds nothing: the values never live in the repository, and the
    /// e2e isolation — which purges the RUNTIME variables — keeps its
    /// lever. At runtime, the environment variable always wins
    /// (`resolve_credential`).
    pub embedded_client_id: Option<&'static str>,
    pub embedded_client_secret: Option<&'static str>,
}

pub static GOOGLE: Provider = Provider {
    name: "Google",
    env_prefix: "GOOGLE",
    account_kind: "gmail",
    vault_prefix: "gmail",
    auth_url: "https://accounts.google.com/o/oauth2/v2/auth",
    token_url: "https://oauth2.googleapis.com/token",
    scopes: &[
        "https://mail.google.com/",
        "https://www.googleapis.com/auth/userinfo.email",
    ],
    granted_scope_marker: "https://mail.google.com/",
    redirect_host: "127.0.0.1",
    // Without these two parameters, Google issues no refresh token: no
    // silent reconnection at the next launch.
    extra_auth_params: &[("access_type", "offline"), ("prompt", "consent")],
    client_secret: ClientSecret::Required,
    identity: Identity::Userinfo("https://www.googleapis.com/oauth2/v2/userinfo"),
    imap: Endpoint {
        host: "imap.gmail.com",
        port: 993,
    },
    smtp: Endpoint {
        host: "smtp.gmail.com",
        port: 465,
    },
    // The "secret" of a Google installed app is not one (mature clients
    // ship it in their binary); it still never enters the repository — only
    // the release build.
    embedded_client_id: option_env!("WIND_RELEASE_GOOGLE_CLIENT_ID"),
    embedded_client_secret: option_env!("WIND_RELEASE_GOOGLE_CLIENT_SECRET"),
};

pub static MICROSOFT: Provider = Provider {
    name: "Microsoft",
    env_prefix: "MICROSOFT",
    account_kind: "microsoft",
    vault_prefix: "microsoft",
    // "common" endpoint: work AND personal accounts.
    auth_url: "https://login.microsoftonline.com/common/oauth2/v2.0/authorize",
    token_url: "https://login.microsoftonline.com/common/oauth2/v2.0/token",
    // Scopes of the Outlook RESOURCE, never the short Graph names. That is
    // trap #1 of this integration (ADR 0006).
    scopes: &[
        "https://outlook.office.com/IMAP.AccessAsUser.All",
        "https://outlook.office.com/SMTP.Send",
        "offline_access",
    ],
    granted_scope_marker: "IMAP.AccessAsUser",
    redirect_host: "localhost",
    // `offline_access` plays the role of `access_type=offline`.
    extra_auth_params: &[],
    client_secret: ClientSecret::Forbidden,
    identity: Identity::Declared,
    imap: Endpoint {
        host: "outlook.office365.com",
        port: 993,
    },
    // 587 + STARTTLS: Office 365 does not listen on implicit TLS 465.
    smtp: Endpoint {
        host: "smtp.office365.com",
        port: 587,
    },
    embedded_client_id: option_env!("WIND_RELEASE_MICROSOFT_CLIENT_ID"),
    // PUBLIC client: an embedded secret would be refused just like an
    // environment secret — it does not exist, even in release.
    embedded_client_secret: None,
};

/// Every known OAuth2 provider. A generic IMAP/SMTP account is not among
/// them: it has no provider, it has servers.
pub static ALL: &[&Provider] = &[&GOOGLE, &MICROSOFT];

/// Finds an account's provider from the value stored in the database.
/// `None` for `"imap"` (generic account) as for an unknown value — the
/// caller handles both cases distinctly.
pub fn for_account_kind(kind: &str) -> Option<&'static Provider> {
    ALL.iter().copied().find(|p| p.account_kind == kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression: the generalization per provider must change NOTHING to
    /// the Google journey already in production. These values are the ones
    /// that were wired as constants before the extraction.
    #[test]
    fn google_keeps_the_endpoints_it_had_before_extraction() {
        assert_eq!(
            GOOGLE.auth_url,
            "https://accounts.google.com/o/oauth2/v2/auth"
        );
        assert_eq!(GOOGLE.token_url, "https://oauth2.googleapis.com/token");
        assert_eq!(
            GOOGLE.scopes,
            [
                "https://mail.google.com/",
                "https://www.googleapis.com/auth/userinfo.email"
            ]
        );
        assert_eq!(GOOGLE.redirect_host, "127.0.0.1");
        assert_eq!(
            GOOGLE.identity,
            Identity::Userinfo("https://www.googleapis.com/oauth2/v2/userinfo")
        );
        assert_eq!(GOOGLE.client_secret, ClientSecret::Required);
    }

    /// Without `access_type=offline` AND `prompt=consent`, Google returns no
    /// refresh token: silent reconnection disappears, and the user consents
    /// again at every launch. A discreet defect, a visible cost — hence the pin.
    #[test]
    fn google_asks_for_a_refresh_token() {
        assert!(
            GOOGLE
                .extra_auth_params
                .contains(&("access_type", "offline"))
        );
        assert!(GOOGLE.extra_auth_params.contains(&("prompt", "consent")));
    }

    /// The values measured by the spike, not those of the documentation.
    #[test]
    fn microsoft_matches_what_the_spike_measured() {
        assert_eq!(
            MICROSOFT.auth_url,
            "https://login.microsoftonline.com/common/oauth2/v2.0/authorize"
        );
        assert_eq!(
            MICROSOFT.scopes,
            [
                "https://outlook.office.com/IMAP.AccessAsUser.All",
                "https://outlook.office.com/SMTP.Send",
                "offline_access"
            ]
        );
        assert_eq!(MICROSOFT.imap.host, "outlook.office365.com");
        assert_eq!(MICROSOFT.smtp.host, "smtp.office365.com");
    }

    /// The two traps frozen by ADR 0006, each held by an assertion: a public
    /// client must send no secret, and `127.0.0.1` is not `localhost` for
    /// Azure AD.
    #[test]
    fn microsoft_is_a_public_client_redirecting_to_localhost() {
        assert_eq!(MICROSOFT.client_secret, ClientSecret::Forbidden);
        assert_eq!(MICROSOFT.redirect_host, "localhost");
    }

    /// Microsoft's SMTP port is 587/STARTTLS. That is what bug #3 made
    /// unreachable; the datum is now carried by the provider, no longer by
    /// an application constant.
    #[test]
    fn microsoft_submits_mail_on_587_not_465() {
        assert_eq!(MICROSOFT.smtp.port, 587);
        assert_eq!(GOOGLE.smtp.port, 465);
    }

    /// Two providers must never fight over a vault entry: their prefixes are
    /// distinct, and Google's stays `gmail` — renaming it would orphan the
    /// tokens already stored.
    #[test]
    fn vault_prefixes_are_distinct_and_google_keeps_its_historical_one() {
        assert_eq!(GOOGLE.vault_prefix, "gmail");
        assert_ne!(GOOGLE.vault_prefix, MICROSOFT.vault_prefix);
    }

    /// Same class of trap as the vault key, on the database side this time:
    /// the `accounts` rows already written carry `"gmail"`. Renaming would
    /// make the existing accounts unconnectable, without any test noticing
    /// — hence the pin.
    #[test]
    fn account_kinds_are_frozen_and_resolvable() {
        assert_eq!(GOOGLE.account_kind, "gmail");
        assert!(std::ptr::eq(
            for_account_kind("gmail").expect("Google"),
            &GOOGLE
        ));
        assert!(std::ptr::eq(
            for_account_kind("microsoft").expect("Microsoft"),
            &MICROSOFT
        ));
    }

    /// A generic account has no OAuth2 provider: the table must certainly
    /// not invent one for it.
    #[test]
    fn generic_accounts_have_no_oauth_provider() {
        assert!(for_account_kind("imap").is_none());
        assert!(for_account_kind("").is_none());
    }

    /// PLAN-RETOURS-9 D1: a dev or test build embeds NO credential — the
    /// `WIND_RELEASE_*` variables only exist in the run of
    /// `make-release.ps1`. The e2e isolation (purge of the runtime variables
    /// before every launch) relies on that absence; a build poisoned by a
    /// lingering environment must shout here.
    #[test]
    fn dev_builds_embed_no_credentials() {
        for provider in ALL {
            assert!(provider.embedded_client_id.is_none());
            assert!(provider.embedded_client_secret.is_none());
        }
    }

    /// Two providers sharing a key would steal each other's accounts at
    /// startup. The table is small; the invariant must survive the third
    /// provider.
    #[test]
    fn no_two_providers_share_an_account_kind() {
        for (index, provider) in ALL.iter().enumerate() {
            for other in &ALL[index + 1..] {
                assert_ne!(provider.account_kind, other.account_kind);
                assert_ne!(provider.vault_prefix, other.vault_prefix);
                assert_ne!(provider.env_prefix, other.env_prefix);
            }
        }
    }
}

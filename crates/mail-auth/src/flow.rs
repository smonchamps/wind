//! Mechanics of the OAuth2 flow: refresh-token exchange, interactive PKCE
//! journey with loopback redirect, verification of the granted scopes.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

use oauth2::basic::{BasicClient, BasicTokenResponse};
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret as OauthSecret, CsrfToken, EndpointNotSet,
    EndpointSet, PkceCodeChallenge, RedirectUrl, RefreshToken, Scope, TokenResponse, TokenUrl,
};

use crate::provider::{ClientSecret, Identity, Provider};

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("configuration: {0}")]
    Config(String),

    #[error("OS vault: {0}")]
    Vault(String),

    #[error("OAuth exchange: {0}")]
    OAuth(String),

    #[error(
        "the {0} consent does not include mail access (granted: {1:?}) — \
         start again and tick the corresponding box on the authorization screen"
    )]
    MissingMailScope(&'static str, Vec<String>),

    #[error("could not open the browser — open manually: {0}")]
    BrowserFallback(String),

    #[error("local network: {0}")]
    Io(#[from] std::io::Error),
}

pub(crate) type OauthClient =
    BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointSet>;
pub(crate) type HttpClient = oauth2::reqwest::blocking::Client;

/// Builds the provider's OAuth2 client.
///
/// The secret is only set if the provider expects one: Azure AD **refuses**
/// the exchange of a public client that presents one.
pub(crate) fn oauth_client(
    provider: &Provider,
    client_id: &str,
    client_secret: Option<&str>,
) -> Result<OauthClient, AuthError> {
    let client = BasicClient::new(ClientId::new(client_id.to_string()))
        .set_auth_uri(AuthUrl::new(provider.auth_url.to_string()).map_err(config_err)?)
        .set_token_uri(TokenUrl::new(provider.token_url.to_string()).map_err(config_err)?);
    Ok(match (provider.client_secret, client_secret) {
        (ClientSecret::Required, Some(secret)) => {
            client.set_client_secret(OauthSecret::new(secret.to_string()))
        }
        _ => client,
    })
}

pub(crate) fn http_client() -> Result<HttpClient, AuthError> {
    oauth2::reqwest::blocking::ClientBuilder::new()
        .redirect(oauth2::reqwest::redirect::Policy::none())
        .build()
        .map_err(|err| AuthError::Config(err.to_string()))
}

pub(crate) fn refresh_access_token(
    client: &OauthClient,
    http: &HttpClient,
    refresh_token: String,
) -> Result<BasicTokenResponse, AuthError> {
    client
        .exchange_refresh_token(&RefreshToken::new(refresh_token))
        .request(http)
        .map_err(|err| AuthError::OAuth(err.to_string()))
}

/// Interactive journey: loopback listener, browser consent, PKCE exchange.
/// Blocks until the provider's redirect.
pub(crate) fn interactive_tokens(
    provider: &Provider,
    client: OauthClient,
    http: &HttpClient,
) -> Result<BasicTokenResponse, AuthError> {
    // Listening is ALWAYS on the loopback; only the name announced to the
    // provider changes (`localhost` at Microsoft, `127.0.0.1` at Google) —
    // both resolve to the same interface.
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    let client = client.set_redirect_uri(
        RedirectUrl::new(format!("http://{}:{port}", provider.redirect_host))
            .map_err(config_err)?,
    );

    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    let mut request = client
        .authorize_url(CsrfToken::new_random)
        .set_pkce_challenge(pkce_challenge);
    for scope in provider.scopes {
        request = request.add_scope(Scope::new((*scope).to_string()));
    }
    for (key, value) in provider.extra_auth_params {
        request = request.add_extra_param(*key, *value);
    }
    let (auth_url, csrf) = request.url();

    if webbrowser::open(auth_url.as_str()).is_err() {
        return Err(AuthError::BrowserFallback(auth_url.to_string()));
    }

    let (code, state) = wait_for_redirect(&listener, CONSENT_TIMEOUT)?;
    if state != *csrf.secret() {
        return Err(AuthError::OAuth("unexpected CSRF state".to_string()));
    }
    client
        .exchange_code(AuthorizationCode::new(code))
        .set_pkce_verifier(pkce_verifier)
        .request(http)
        .map_err(|err| AuthError::OAuth(err.to_string()))
}

/// Both providers issue a token even on partial consent (boxes unticked at
/// Google, scopes refused at Microsoft): only the list of *granted* scopes
/// counts. A response without a scope field (some refreshes) is accepted:
/// the consent was already validated when the refresh token was stored.
pub(crate) fn ensure_mail_scope(
    provider: &Provider,
    tokens: &BasicTokenResponse,
) -> Result<(), AuthError> {
    match tokens.scopes() {
        Some(scopes) => {
            let granted: Vec<String> = scopes.iter().map(|s| s.as_str().to_string()).collect();
            if granted
                .iter()
                .any(|scope| scope.contains(provider.granted_scope_marker))
            {
                Ok(())
            } else {
                Err(AuthError::MissingMailScope(provider.name, granted))
            }
        }
        None => Ok(()),
    }
}

/// Resolves the account's email according to the provider's identity
/// strategy.
///
/// `declared` is the email given by the user; it is only used if the
/// provider cannot deliver the identity itself.
pub(crate) fn resolve_email(
    provider: &Provider,
    http: &HttpClient,
    access_token: &str,
    declared: Option<&str>,
) -> Result<String, AuthError> {
    match provider.identity {
        Identity::Userinfo(url) => fetch_email(http, url, access_token),
        Identity::Declared => declared.map(str::to_string).ok_or_else(|| {
            AuthError::Config(format!(
                "{} does not deliver the account address: it must be entered",
                provider.name
            ))
        }),
    }
}

fn fetch_email(http: &HttpClient, url: &str, access_token: &str) -> Result<String, AuthError> {
    let body = http
        .get(url)
        .bearer_auth(access_token)
        .send()
        .map_err(network_err)?
        .error_for_status()
        .map_err(network_err)?
        .text()
        .map_err(network_err)?;
    serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|value| {
            value
                .get("email")
                .and_then(|email| email.as_str())
                .map(str::to_string)
        })
        .ok_or_else(|| AuthError::OAuth("email absent from the userinfo response".to_string()))
}

/// How long the browser consent may keep us waiting (PLAN-AUDIT-V1 E8, CE
/// decision D3: 5 minutes). Before: no limit — a closed tab froze the "add
/// account" command forever, and the loopback port stayed bound.
const CONSENT_TIMEOUT: Duration = Duration::from_secs(300);

/// An accepted connection that says nothing (probe, browser pre-opening)
/// must not block the wait either.
const REDIRECT_READ_TIMEOUT: Duration = Duration::from_secs(2);

fn wait_for_redirect(
    listener: &TcpListener,
    timeout: Duration,
) -> Result<(String, String), AuthError> {
    listener.set_nonblocking(true)?;
    let start = Instant::now();
    loop {
        let mut stream = match listener.accept() {
            Ok((stream, _)) => stream,
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                if start.elapsed() >= timeout {
                    return Err(AuthError::OAuth(format!(
                        "consent not received within {} min — restart adding the account",
                        timeout.as_secs() / 60
                    )));
                }
                std::thread::sleep(Duration::from_millis(50));
                continue;
            }
            Err(err) => return Err(err.into()),
        };
        stream.set_nonblocking(false)?;
        stream.set_read_timeout(Some(REDIRECT_READ_TIMEOUT))?;
        let mut request_line = String::new();
        if BufReader::new(&mut stream)
            .read_line(&mut request_line)
            .is_err()
        {
            continue; // mute or cut connection: wait for the real one
        }
        let Some(params) = parse_redirect_query(&request_line) else {
            respond(&mut stream, "Request ignored.")?;
            continue;
        };
        if let Some(error) = params.get("error") {
            respond(&mut stream, "Authorization refused. Close this tab.")?;
            return Err(AuthError::OAuth(format!("authorization refused: {error}")));
        }
        if let (Some(code), Some(state)) = (params.get("code"), params.get("state")) {
            respond(
                &mut stream,
                "Authorization received. Close this tab and return to Wind.",
            )?;
            return Ok((code.clone(), state.clone()));
        }
        respond(&mut stream, "Unexpected parameters.")?;
    }
}

/// Extracts the query parameters of the first HTTP line of the redirect
/// (`GET /?code=…&state=… HTTP/1.1`).
fn parse_redirect_query(request_line: &str) -> Option<HashMap<String, String>> {
    let path = request_line.split_whitespace().nth(1)?;
    let url = url::Url::parse(&format!("http://127.0.0.1{path}")).ok()?;
    Some(
        url.query_pairs()
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect(),
    )
}

fn respond(stream: &mut TcpStream, message: &str) -> Result<(), AuthError> {
    let body = format!("<html><body><p>{message}</p></body></html>");
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes())?;
    Ok(())
}

fn config_err(err: url::ParseError) -> AuthError {
    AuthError::Config(err.to_string())
}

fn network_err(err: oauth2::reqwest::Error) -> AuthError {
    AuthError::OAuth(err.to_string())
}

#[cfg(test)]
mod tests {
    use oauth2::basic::BasicTokenType;
    use oauth2::{AccessToken, EmptyExtraTokenFields, StandardTokenResponse};

    /// E8: a closed tab no longer freezes the command forever — the wait
    /// expires (D3: 5 min in production, 200 ms here).
    #[test]
    fn the_redirect_wait_expires() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let start = std::time::Instant::now();
        let outcome = super::wait_for_redirect(&listener, std::time::Duration::from_millis(200));
        assert!(outcome.is_err(), "without a redirect, the wait must expire");
        assert!(start.elapsed() < std::time::Duration::from_secs(5));
        assert!(
            outcome
                .unwrap_err()
                .to_string()
                .contains("consent not received"),
            "the message says what to do"
        );
    }

    /// E8: an accepted connection that stays silent (probe, browser
    /// pre-opening) does not immobilize the wait — the real redirect that
    /// follows is served.
    #[test]
    fn a_mute_connection_does_not_immobilize_the_wait() {
        use std::io::Write;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let _mute = std::net::TcpStream::connect(address).unwrap();
        let real = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(100));
            let mut real = std::net::TcpStream::connect(address).unwrap();
            real.write_all(b"GET /?code=abc&state=xyz HTTP/1.1\r\n\r\n")
                .unwrap();
            real
        });
        let outcome = super::wait_for_redirect(&listener, std::time::Duration::from_secs(10));
        let _ = real.join();
        assert_eq!(outcome.unwrap(), ("abc".to_string(), "xyz".to_string()));
    }

    /// E8: `Debug` never shows a secret — a future diagnostic `{:?}` will
    /// print neither token nor password.
    #[test]
    fn debug_shows_no_secret() {
        let session = crate::Authenticated {
            provider: &crate::GOOGLE,
            email: "a@x.fr".to_string(),
            access_token: "SECRET-TOKEN".to_string(),
        };
        let text = format!("{session:?}");
        assert!(!text.contains("SECRET-TOKEN"), "{text}");
        assert!(text.contains("a@x.fr"));
        let creds = crate::GenericCredentials {
            email: "a@x.fr".to_string(),
            username: "a".to_string(),
            password: "SECRET-PASSWORD".to_string(),
            imap_host: "imap.x.fr".to_string(),
            imap_port: 993,
            smtp_host: "smtp.x.fr".to_string(),
            smtp_port: 587,
        };
        let text = format!("{creds:?}");
        assert!(!text.contains("SECRET-PASSWORD"), "{text}");
        assert!(text.contains("imap.x.fr"));
    }

    use super::*;
    use crate::provider::{GOOGLE, MICROSOFT};

    fn token_response(scopes: Option<Vec<&str>>) -> BasicTokenResponse {
        let mut response = StandardTokenResponse::new(
            AccessToken::new("test-token".to_string()),
            BasicTokenType::Bearer,
            EmptyExtraTokenFields {},
        );
        response.set_scopes(scopes.map(|list| {
            list.into_iter()
                .map(|s| Scope::new(s.to_string()))
                .collect()
        }));
        response
    }

    #[test]
    fn parses_code_and_state_from_redirect_line() {
        let params =
            parse_redirect_query("GET /?state=xyz&code=abc123 HTTP/1.1").expect("params expected");
        assert_eq!(params.get("code").map(String::as_str), Some("abc123"));
        assert_eq!(params.get("state").map(String::as_str), Some("xyz"));
    }

    #[test]
    fn decodes_percent_encoding_in_redirect() {
        let params =
            parse_redirect_query("GET /?error=access%20denied HTTP/1.1").expect("params expected");
        assert_eq!(
            params.get("error").map(String::as_str),
            Some("access denied")
        );
    }

    #[test]
    fn rejects_garbage_request_line() {
        assert!(parse_redirect_query("").is_none());
        assert!(parse_redirect_query("GET").is_none());
    }

    #[test]
    fn accepts_token_with_mail_scope() {
        let tokens = token_response(Some(vec![
            "https://mail.google.com/",
            "https://www.googleapis.com/auth/userinfo.email",
        ]));
        assert!(ensure_mail_scope(&GOOGLE, &tokens).is_ok());
    }

    #[test]
    fn rejects_token_missing_mail_scope() {
        let tokens = token_response(Some(vec!["https://www.googleapis.com/auth/userinfo.email"]));
        let err = ensure_mail_scope(&GOOGLE, &tokens).expect_err("missing scope expected");
        assert!(matches!(err, AuthError::MissingMailScope(_, _)));
    }

    #[test]
    fn accepts_refresh_response_without_scope_field() {
        let tokens = token_response(None);
        assert!(ensure_mail_scope(&GOOGLE, &tokens).is_ok());
    }

    /// The Microsoft consent is checked on ITS own scopes. The Google marker
    /// never appears there: without a per-provider rule, every Microsoft
    /// account would be refused although perfectly authorized.
    #[test]
    fn accepts_microsoft_token_with_its_own_imap_scope() {
        let tokens = token_response(Some(vec![
            "https://outlook.office.com/IMAP.AccessAsUser.All",
            "https://outlook.office.com/SMTP.Send",
        ]));
        assert!(ensure_mail_scope(&MICROSOFT, &tokens).is_ok());
        assert!(
            ensure_mail_scope(&GOOGLE, &tokens).is_err(),
            "the rules must not be interchangeable"
        );
    }

    /// The really dangerous case: a partial consent where only sending is
    /// granted. The sync would be dead, and the error message must name the
    /// right provider.
    #[test]
    fn rejects_microsoft_token_granted_only_for_sending() {
        let tokens = token_response(Some(vec!["https://outlook.office.com/SMTP.Send"]));
        let err = ensure_mail_scope(&MICROSOFT, &tokens).expect_err("missing IMAP scope expected");
        match err {
            AuthError::MissingMailScope(name, _) => assert_eq!(name, "Microsoft"),
            other => panic!("expected a missing scope, got {other:?}"),
        }
    }
}

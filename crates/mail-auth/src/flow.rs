//! Mechanics of the OAuth2 flow: refresh-token exchange, interactive PKCE
//! journey with loopback redirect, verification of the granted scopes.

use std::collections::HashMap;
use std::io::{Read, Write};
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

    #[error("authorization cancelled")]
    Cancelled,

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
    // rustls with the PLATFORM verifier (Windows certificate store,
    // corporate CAs included) — the ONE TLS stack rule (ADR 0032): without
    // this, reqwest's plain rustls path trusts webpki roots only, and a
    // corporate CA that works in IMAP fails here.
    use rustls_platform_verifier::BuilderVerifierExt;
    // Built ONCE per process (review, wave 3) — the same
    // platform-verifier config mail-imap caches on its side; a change
    // here changes there (ADR 0032's one-stack rule).
    static CONFIG: std::sync::OnceLock<rustls::ClientConfig> = std::sync::OnceLock::new();
    let tls = match CONFIG.get() {
        Some(config) => config.clone(),
        None => {
            let built = rustls::ClientConfig::builder()
                .with_platform_verifier()
                .map_err(|err| AuthError::Config(err.to_string()))?
                .with_no_client_auth();
            CONFIG.get_or_init(|| built).clone()
        }
    };
    oauth2::reqwest::blocking::ClientBuilder::new()
        .use_preconfigured_tls(tls)
        .redirect(oauth2::reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(30))
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

/// A cancellable consent, shared with the UI without exposing PKCE secrets.
#[derive(Clone, Default)]
pub struct ConsentControl(std::sync::Arc<ConsentState>);

#[derive(Default)]
struct ConsentState {
    cancelled: std::sync::atomic::AtomicBool,
    publishing: std::sync::Mutex<bool>,
    link: std::sync::Mutex<Option<AuthorizationLink>>,
}

#[derive(Clone)]
pub struct AuthorizationLink {
    pub url: String,
    pub manual: bool,
}

impl ConsentControl {
    pub fn cancel(&self) -> bool {
        let Ok(publishing) = self.0.publishing.lock() else {
            return false;
        };
        if *publishing {
            return false;
        }
        self.0
            .cancelled
            .store(true, std::sync::atomic::Ordering::Release);
        true
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.cancelled.load(std::sync::atomic::Ordering::Acquire)
    }
    pub fn status(&self) -> Result<Option<AuthorizationLink>, AuthError> {
        self.0
            .link
            .lock()
            .map(|link| link.clone())
            .map_err(|_| AuthError::Config("consent status unavailable".into()))
    }
    fn publish(&self, url: String, manual: bool) -> Result<(), AuthError> {
        *self
            .0
            .link
            .lock()
            .map_err(|_| AuthError::Config("consent status unavailable".into()))? =
            Some(AuthorizationLink { url, manual });
        Ok(())
    }
    pub(crate) fn check(&self) -> Result<(), AuthError> {
        if self.is_cancelled() {
            Err(AuthError::Cancelled)
        } else {
            Ok(())
        }
    }
    pub(crate) fn begin_publication(&self) -> Result<(), AuthError> {
        let mut publishing = self
            .0
            .publishing
            .lock()
            .map_err(|_| AuthError::Config("consent state unavailable".into()))?;
        self.check()?;
        *publishing = true;
        Ok(())
    }
}

/// Interactive journey: loopback listener, browser consent, PKCE exchange.
/// Blocks until the provider's redirect.
pub(crate) fn interactive_tokens(
    provider: &Provider,
    client: OauthClient,
    http: &HttpClient,
    control: &ConsentControl,
) -> Result<BasicTokenResponse, AuthError> {
    interactive_tokens_using(provider, client, http, control, |url| {
        webbrowser::open(url).is_ok()
    })
}

pub(crate) fn interactive_tokens_using(
    provider: &Provider,
    client: OauthClient,
    http: &HttpClient,
    control: &ConsentControl,
    open_browser: impl FnOnce(&str) -> bool,
) -> Result<BasicTokenResponse, AuthError> {
    control.check()?;
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

    let opened = open_browser(auth_url.as_str());
    control.publish(auth_url.to_string(), !opened)?;
    let (code, _) = wait_for_redirect(&listener, CONSENT_TIMEOUT, csrf.secret(), control)?;
    control.check()?;
    let tokens = client
        .exchange_code(AuthorizationCode::new(code))
        .set_pkce_verifier(pkce_verifier)
        .request(http)
        .map_err(|err| AuthError::OAuth(err.to_string()))?;
    control.check()?;
    Ok(tokens)
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

// A browser may pre-open a connection. It must release the listener for another
// client within two seconds, and can never extend the whole consent deadline.
const REDIRECT_CLIENT_TIMEOUT: Duration = Duration::from_secs(2);
const REDIRECT_LINE_MAX: usize = 8192;
const REDIRECT_POLL: Duration = Duration::from_millis(10);

fn wait_for_redirect(
    listener: &TcpListener,
    timeout: Duration,
    expected_state: &str,
    control: &ConsentControl,
) -> Result<(String, String), AuthError> {
    listener.set_nonblocking(true)?;
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        control.check()?;
        let mut stream = match listener.accept() {
            Ok((stream, _)) => stream,
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                pause_redirect(deadline);
                continue;
            }
            Err(err) => return Err(err.into()),
        };
        stream.set_nonblocking(true)?;
        let client_deadline = deadline.min(Instant::now() + REDIRECT_CLIENT_TIMEOUT);
        let line = read_redirect_line(&mut stream, client_deadline, control);
        control.check()?;
        let Some(line) = line else {
            continue;
        };
        let Some(params) = parse_redirect_query(&line) else {
            continue;
        };
        if params.get("state").map(String::as_str) != Some(expected_state) {
            continue;
        }
        match (params.get("code"), params.get("error")) {
            (Some(code), None) if !code.is_empty() => {
                // Browser acknowledgement is best effort; a disconnected tab must
                // not discard an otherwise valid, state-checked authorization.
                respond(
                    &mut stream,
                    "Authorization received. Close this tab and return to Wind.",
                    client_deadline,
                );
                return Ok((code.clone(), expected_state.to_string()));
            }
            (None, Some(error)) if !error.is_empty() => {
                respond(
                    &mut stream,
                    "Authorization refused. Close this tab.",
                    client_deadline,
                );
                return Err(AuthError::OAuth(format!("authorization refused: {error}")));
            }
            _ => {}
        }
    }
    Err(AuthError::OAuth(format!(
        "consent not received within {} min — restart adding the account",
        timeout.as_secs() / 60
    )))
}

fn pause_redirect(deadline: Instant) {
    std::thread::sleep(REDIRECT_POLL.min(deadline.saturating_duration_since(Instant::now())));
}

fn read_redirect_line(
    stream: &mut TcpStream,
    deadline: Instant,
    control: &ConsentControl,
) -> Option<String> {
    let mut line = Vec::new();
    let mut bytes = [0_u8; 1024];
    while Instant::now() < deadline && !control.is_cancelled() {
        match stream.read(&mut bytes) {
            Ok(0) => return None,
            Ok(count) => {
                let end = bytes[..count].iter().position(|byte| *byte == b'\n');
                let used = end.map_or(count, |index| index + 1);
                if line.len() + used > REDIRECT_LINE_MAX {
                    return None;
                }
                line.extend_from_slice(&bytes[..used]);
                if end.is_some() {
                    return String::from_utf8(line).ok();
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => pause_redirect(deadline),
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return None,
        }
    }
    None
}

/// Only the loopback callback's GET / target, with unambiguous query parameters.
fn parse_redirect_query(request_line: &str) -> Option<HashMap<String, String>> {
    let mut parts = request_line.split_ascii_whitespace();
    if parts.next()? != "GET" {
        return None;
    }
    let path = parts.next()?;
    if !matches!(parts.next()?, "HTTP/1.0" | "HTTP/1.1") || parts.next().is_some() {
        return None;
    }
    if !path.starts_with('/') {
        return None;
    }
    let url = url::Url::parse(&format!("http://127.0.0.1{path}")).ok()?;
    if url.path() != "/" || url.fragment().is_some() {
        return None;
    }
    let mut params = HashMap::new();
    for (key, value) in url.query_pairs() {
        if params
            .insert(key.into_owned(), value.into_owned())
            .is_some()
        {
            return None;
        }
    }
    Some(params)
}

fn respond(stream: &mut TcpStream, message: &str, deadline: Instant) {
    let body = format!("<html><body><p>{message}</p></body></html>");
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let mut bytes = response.as_bytes();
    while !bytes.is_empty() && Instant::now() < deadline {
        match stream.write(bytes) {
            Ok(0) => break,
            Ok(count) => bytes = &bytes[count..],
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => pause_redirect(deadline),
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => break,
        }
    }
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
        let outcome = super::wait_for_redirect(
            &listener,
            std::time::Duration::from_millis(200),
            "xyz",
            &ConsentControl::default(),
        );
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
        let outcome = super::wait_for_redirect(
            &listener,
            std::time::Duration::from_secs(10),
            "xyz",
            &ConsentControl::default(),
        );
        let _ = real.join();
        assert_eq!(outcome.unwrap(), ("abc".to_string(), "xyz".to_string()));
    }

    #[test]
    fn a_silent_client_cannot_extend_the_consent_deadline() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let _client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let start = Instant::now();
        assert!(
            wait_for_redirect(
                &listener,
                Duration::from_millis(150),
                "xyz",
                &ConsentControl::default()
            )
            .is_err()
        );
        assert!(
            start.elapsed() < Duration::from_millis(700),
            "{:?}",
            start.elapsed()
        );
    }

    #[test]
    fn redirect_parser_rejects_wrong_targets_and_duplicate_parameters() {
        for line in [
            "POST /?code=abc&state=xyz HTTP/1.1",
            "GET /other?code=abc&state=xyz HTTP/1.1",
            "GET /?code=abc&state=wrong&state=xyz HTTP/1.1",
            "GET /?code=abc&code=other&state=xyz HTTP/1.1",
            "GET /?code=abc&state=xyz#fragment HTTP/1.1",
            "GET /?code=abc&state=xyz HTTP/2.0",
        ] {
            assert!(parse_redirect_query(line).is_none(), "accepted {line}");
        }
    }

    fn queued_request(listener: &TcpListener, request: &str) -> TcpStream {
        let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        client
            .set_write_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        client.write_all(request.as_bytes()).unwrap();
        client
    }

    #[test]
    fn an_oversized_callback_is_ignored_before_a_valid_one() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let _oversized = queued_request(
            &listener,
            &format!(
                "GET /?code=oversized&state=xyz&padding={} HTTP/1.1\r\n",
                "x".repeat(9000)
            ),
        );
        let _valid = queued_request(&listener, "GET /?code=valid&state=xyz HTTP/1.1\r\n");
        assert_eq!(
            wait_for_redirect(
                &listener,
                Duration::from_secs(3),
                "xyz",
                &ConsentControl::default()
            )
            .unwrap(),
            ("valid".into(), "xyz".into())
        );
    }

    #[test]
    fn unrelated_state_cannot_consume_the_real_callback() {
        for query in ["code=wrong&state=other", "error=access_denied&state=other"] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let _unrelated = queued_request(&listener, &format!("GET /?{query} HTTP/1.1\r\n"));
            let _valid = queued_request(&listener, "GET /?code=valid&state=xyz HTTP/1.1\r\n");
            assert_eq!(
                wait_for_redirect(
                    &listener,
                    Duration::from_secs(3),
                    "xyz",
                    &ConsentControl::default()
                )
                .unwrap(),
                ("valid".into(), "xyz".into())
            );
        }
    }

    #[test]
    fn failed_browser_open_keeps_the_same_callback_alive() {
        use std::io::{BufRead, BufReader};
        let token_server = TcpListener::bind("127.0.0.1:0").unwrap();
        token_server.set_nonblocking(true).unwrap();
        let endpoint = format!("http://{}/token", token_server.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            // 10 s / 5 s, not 1 s / 1 s: on a loaded CI runner the token
            // exchange crossed the old deadline and `read_line` panicked
            // mid-request (two occurrences on macOS x86_64, 2026-09-07,
            // PRs #12 and #15 — same test, unrelated bumps). The happy
            // path stays event-driven: green runs pay none of it.
            let deadline = Instant::now() + Duration::from_secs(10);
            while Instant::now() < deadline {
                if let Ok((mut socket, _)) = token_server.accept() {
                    socket
                        .set_read_timeout(Some(Duration::from_secs(5)))
                        .unwrap();
                    socket
                        .set_write_timeout(Some(Duration::from_secs(5)))
                        .unwrap();
                    let mut reader = BufReader::new(&mut socket);
                    let mut request = String::new();
                    let mut length = 0;
                    loop {
                        let mut line = String::new();
                        reader.read_line(&mut line).unwrap();
                        assert!(line.len() < 8192);
                        if line == "\r\n" {
                            break;
                        }
                        if let Some(value) = line.to_lowercase().strip_prefix("content-length:") {
                            length = value.trim().parse::<usize>().unwrap();
                        }
                        request.push_str(&line);
                    }
                    assert!(length < 8192);
                    let mut body = vec![0; length];
                    reader.read_exact(&mut body).unwrap();
                    let json = r#"{"access_token":"synthetic","token_type":"Bearer"}"#;
                    write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{json}", json.len()).unwrap();
                    return Some(String::from_utf8(body).unwrap());
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            None
        });
        let client = oauth_client(&GOOGLE, "fixture", None)
            .unwrap()
            .set_token_uri(TokenUrl::new(endpoint).unwrap());
        let control = ConsentControl::default();
        let result = interactive_tokens_using(
            &GOOGLE,
            client,
            &http_client().unwrap(),
            &control,
            |auth_url| {
                let url = url::Url::parse(auth_url).unwrap();
                let query: HashMap<_, _> = url.query_pairs().into_owned().collect();
                let redirect = url::Url::parse(&query["redirect_uri"]).unwrap();
                let mut callback =
                    TcpStream::connect(("127.0.0.1", redirect.port().unwrap())).unwrap();
                write!(
                    callback,
                    "GET /?code=fixture-code&state={} HTTP/1.1\r\n",
                    query["state"]
                )
                .unwrap();
                false
            },
        );
        let posted = server.join().unwrap();
        assert!(result.is_ok(), "{result:?}");
        let posted = posted.unwrap();
        assert!(posted.contains("code=fixture-code"));
        assert!(posted.contains("code_verifier="));
        assert!(control.status().unwrap().unwrap().manual);
    }

    #[test]
    fn cancelling_consent_interrupts_an_accepted_silent_client() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let _silent = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let control = ConsentControl::default();
        let cancellation = control.clone();
        let worker = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(30));
            cancellation.cancel();
        });
        let start = Instant::now();
        let result = wait_for_redirect(&listener, Duration::from_secs(1), "xyz", &control);
        worker.join().unwrap();
        assert!(matches!(result, Err(AuthError::Cancelled)));
        assert!(start.elapsed() < Duration::from_millis(300));
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

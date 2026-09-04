//! Spike TLS-STACK (PLAN-AUDIT-V3 E1, D2). THROW-AWAY.
//!
//! Proof by compilation + live handshake that the three TLS users of
//! Wind can all run on rustls + rustls-platform-verifier (Windows cert
//! store as verifier — the updater's existing stack).

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Context;
use rustls_platform_verifier::BuilderVerifierExt;

/// ONE rustls ClientConfig, Windows cert store as verifier — the config
/// every user below shares.
fn platform_config() -> anyhow::Result<rustls::ClientConfig> {
    let config = rustls::ClientConfig::builder()
        .with_platform_verifier()?
        .with_no_client_auth();
    Ok(config)
}

/// User 1 — mail-smtp (lettre 0.11.22): NO hook for a caller-supplied
/// rustls::ClientConfig (InnerTlsParameters is pub(crate)); the hook is
/// the cargo feature "rustls-platform-verifier" — with it, build_rustls()
/// on CertificateStore::Default constructs
/// rustls_platform_verifier::Verifier internally (tls.rs:540-548).
fn build_smtp_params() -> anyhow::Result<lettre::transport::smtp::client::TlsParameters> {
    use lettre::transport::smtp::client::{CertificateStore, TlsParameters};
    let params = TlsParameters::builder("smtp.gmail.com".to_string())
        .certificate_store(CertificateStore::Default)
        .build_rustls()?;
    Ok(params)
}

/// User 2 — mail-auth (oauth2 5.0.0 + reqwest 0.12.28 blocking): a
/// preconfigured-TLS reqwest client, accepted by oauth2's executor.
fn build_oauth_client(config: rustls::ClientConfig) -> anyhow::Result<reqwest::blocking::Client> {
    let client = reqwest::blocking::Client::builder()
        .use_preconfigured_tls(config)
        // Same posture as oauth2's docs: never follow redirects on the
        // token endpoint.
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    Ok(client)
}

/// Compile-time proof that oauth2 5.0.0's SyncHttpClient is implemented
/// for the preconfigured reqwest::blocking::Client (the executor the
/// production code hands to request_token).
fn assert_oauth_executor(client: &reqwest::blocking::Client) {
    fn is_sync_http_client<C: oauth2::SyncHttpClient>(_c: &C) {}
    is_sync_http_client(client);
}

/// User 3 — mail-imap (imap 3.0.0-alpha.15): generic Client over any
/// Read+Write stream; a rustls StreamOwned goes in exactly where the
/// native_tls::TlsStream goes today.
fn imap_connect(
    host: &str,
    port: u16,
    config: Arc<rustls::ClientConfig>,
) -> anyhow::Result<(imap::Client<rustls::StreamOwned<rustls::ClientConnection, TcpStream>>, Duration, Duration)> {
    let t0 = Instant::now();
    let tcp = TcpStream::connect((host, port)).context("tcp connect")?;
    tcp.set_read_timeout(Some(Duration::from_secs(20)))?;
    tcp.set_write_timeout(Some(Duration::from_secs(20)))?;
    let tcp_time = t0.elapsed();

    let name = rustls::pki_types::ServerName::try_from(host.to_string())?;
    let conn = rustls::ClientConnection::new(config, name)?;
    let mut stream = rustls::StreamOwned::new(conn, tcp);
    // Force the handshake to complete now (rustls handshakes lazily on
    // first read/write): a flush drives it to completion.
    let t1 = Instant::now();
    stream.flush().context("tls handshake")?;
    while stream.conn.is_handshaking() {
        stream.conn.complete_io(&mut stream.sock).context("tls handshake")?;
    }
    let hs_time = t1.elapsed();

    let mut client = imap::Client::new(stream);
    client
        .read_greeting()
        .map_err(|e| anyhow::anyhow!("greeting: {e}"))?;
    Ok((client, tcp_time, hs_time))
}

/// Live SMTP probe: TCP + TLS handshake on 465 + read the 220 greeting.
/// Done with raw rustls (same config object lettre received) — lettre
/// itself only connects inside a full transport; the point here is the
/// handshake against smtp.gmail.com with the platform verifier.
fn smtp_probe(host: &str, port: u16, config: Arc<rustls::ClientConfig>) -> anyhow::Result<(Duration, String)> {
    let tcp = TcpStream::connect((host, port)).context("tcp connect")?;
    tcp.set_read_timeout(Some(Duration::from_secs(20)))?;
    let name = rustls::pki_types::ServerName::try_from(host.to_string())?;
    let conn = rustls::ClientConnection::new(config, name)?;
    let mut stream = rustls::StreamOwned::new(conn, tcp);
    let t = Instant::now();
    stream.flush()?;
    while stream.conn.is_handshaking() {
        stream.conn.complete_io(&mut stream.sock)?;
    }
    let hs = t.elapsed();
    let mut buf = [0u8; 512];
    let n = stream.read(&mut buf)?;
    let greeting = String::from_utf8_lossy(&buf[..n]).trim_end().to_string();
    Ok((hs, greeting))
}

fn main() -> anyhow::Result<()> {
    let config = Arc::new(platform_config()?);

    // Compilation proofs (constructed, even without network).
    let _smtp_params = build_smtp_params()?;
    println!("[smtp]  lettre build_rustls() + feature rustls-platform-verifier: OK");
    let oauth_client = build_oauth_client((*config).clone())?;
    assert_oauth_executor(&oauth_client);
    println!("[auth]  reqwest::blocking use_preconfigured_tls + oauth2::SyncHttpClient: OK");

    let live = std::env::args().any(|a| a == "--live");
    if !live {
        println!("(compilation proof only; pass --live for the network test)");
        return Ok(());
    }

    // IMAP live: 3 repetitions.
    for i in 1..=3 {
        match imap_connect("imap.gmail.com", 993, config.clone()) {
            Ok((_client, tcp, hs)) => println!(
                "[imap]  run {i}: imap.gmail.com:993 tcp={}ms handshake={}ms greeting=OK",
                tcp.as_millis(),
                hs.as_millis()
            ),
            Err(e) => println!("[imap]  run {i}: FAIL {e:#}"),
        }
    }

    // SMTP live: 3 repetitions.
    for i in 1..=3 {
        match smtp_probe("smtp.gmail.com", 465, config.clone()) {
            Ok((hs, greeting)) => println!(
                "[smtp]  run {i}: smtp.gmail.com:465 handshake={}ms greeting=\"{}\"",
                hs.as_millis(),
                greeting
            ),
            Err(e) => println!("[smtp]  run {i}: FAIL {e:#}"),
        }
    }

    // HTTPS live via the actual reqwest client oauth2 would use.
    for i in 1..=3 {
        let t = Instant::now();
        match oauth_client
            .get("https://accounts.google.com/.well-known/openid-configuration")
            .send()
        {
            Ok(resp) => {
                let status = resp.status();
                let len = resp.bytes().map(|b| b.len()).unwrap_or(0);
                println!(
                    "[auth]  run {i}: GET openid-configuration status={status} bytes={len} total={}ms",
                    t.elapsed().as_millis()
                );
            }
            Err(e) => println!("[auth]  run {i}: FAIL {e:#}"),
        }
    }

    Ok(())
}

//! End-to-end validation: mail-core's `SyncEngine` wired to the real IMAP
//! adapter, against your Gmail account.
//!
//! Prerequisites: `GOOGLE_CLIENT_ID`/`GOOGLE_CLIENT_SECRET` in the environment
//! and an account already connected through the Wind application (the
//! refresh token lives in the Credential Manager, service "wind-mail").
//!
//! ```powershell
//! cargo run -p mail-imap --example sync_gmail --release
//! ```

use std::time::Instant;

use anyhow::Context;
use mail_auth::Authenticator;
use mail_core::{Store, SyncEngine};
use mail_imap::ImapServer;

fn main() -> anyhow::Result<()> {
    let auth = Authenticator::google_from_env().context("OAuth configuration")?;
    let account = match std::env::var("WIND_ACCOUNT") {
        Ok(email) => auth.authenticate_silent(&email),
        Err(_) => auth.authenticate_silent_legacy(),
    }
    .context("connect an account through Wind first (or set WIND_ACCOUNT)")?;

    let timer = Instant::now();
    let mut server =
        ImapServer::connect_xoauth2("imap.gmail.com", 993, &account.email, &account.access_token)?;
    println!("Connected ({}) in {:?}", account.email, timer.elapsed());

    let db_path = std::path::PathBuf::from("target/mail-imap-example.db");
    let mut store = Store::open(&db_path)?;
    let account_id = store.adopt_or_create_account(&account.email, "gmail")?;

    let timer = Instant::now();
    let report = SyncEngine::default().sync(&mut server, &mut store, account_id, "INBOX")?;
    println!(
        "Sync {:?}: {} envelope(s) fetched/updated, {} deleted, in {:?}",
        report.mode,
        report.fetched,
        report.deleted,
        timer.elapsed()
    );
    server.logout();

    let timer = Instant::now();
    let recent = store.recent(account_id, "INBOX", 0, 10)?;
    println!(
        "The 10 most recent (read from SQLite in {:?}):",
        timer.elapsed()
    );
    for envelope in recent {
        let marker = if envelope.seen { " " } else { "●" };
        let date = envelope
            .date
            .map(|d| d.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "????-??-??".to_string());
        println!(
            "{marker} {date}  {:28}  {}",
            truncate(envelope.sender.as_deref().unwrap_or("(unknown)"), 28),
            truncate(envelope.subject.as_deref().unwrap_or("(no subject)"), 58),
        );
    }
    Ok(())
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let cut: String = text.chars().take(max).collect();
        format!("{cut}…")
    }
}

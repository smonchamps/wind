//! End-to-end validation: mail-core's outbox wired to the real SMTP adapter,
//! against your Gmail account — a message sent to yourself.
//!
//! Prerequisites: `GOOGLE_CLIENT_ID`/`GOOGLE_CLIENT_SECRET` in the environment
//! and an account already connected through the Wind application (the
//! refresh token lives in the Credential Manager, service "wind-mail").
//!
//! ```powershell
//! cargo run -p mail-smtp --example send_gmail --release
//! ```

use std::time::Instant;

use anyhow::Context;
use mail_auth::Authenticator;
use mail_core::{OutboxState, Store};
use mail_smtp::SmtpMailer;

fn main() -> anyhow::Result<()> {
    let auth = Authenticator::google_from_env().context("OAuth configuration")?;
    let account = match std::env::var("WIND_ACCOUNT") {
        Ok(email) => auth.authenticate_silent(&email),
        Err(_) => auth.authenticate_silent_legacy(),
    }
    .context("connect an account through Wind first (or set WIND_ACCOUNT)")?;

    // The product's full path: journal first, send next.
    let db_path = std::path::PathBuf::from("target/mail-smtp-example.db");
    let mut store = Store::open(&db_path)?;
    let account_id = store.adopt_or_create_account(&account.email, "gmail")?;
    let draft = mail_core::compose(
        &account.email,
        &account.email,
        "",
        "",
        "Wind — outbox trial",
        "This message went through the persistent outbox.\n\
         If it arrives exactly once, both golden rules hold.",
        None,
    )?;
    store.enqueue_outbox(account_id, &draft)?;
    println!("Journaled: {}", draft.message_id);

    let timer = Instant::now();
    let mut mailer =
        SmtpMailer::connect_xoauth2("smtp.gmail.com", 465, &account.email, &account.access_token)
            .map_err(|err| anyhow::anyhow!("SMTP connection: {err}"))?;
    println!("Connected ({}) in {:?}", account.email, timer.elapsed());

    let timer = Instant::now();
    let report = mail_core::flush_outbox(&mut mailer, &mut store, account_id)?;
    println!(
        "Flush in {:?}: {} sent, {} deferred, {} rejected, {} quarantined",
        timer.elapsed(),
        report.sent,
        report.deferred,
        report.rejected,
        report.quarantined,
    );

    for message in store.outbox_in_state(OutboxState::Queued)? {
        println!("Still queued: {} ({})", message.subject, message.message_id);
    }
    Ok(())
}

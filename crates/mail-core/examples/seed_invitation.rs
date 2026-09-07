//! Synthetic calendar arrivals for the isolated E2E database.
use chrono::Utc;
use mail_core::{Envelope, Store};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let (Some(path), Some(email), Some(source)) = (args.get(1), args.get(2), args.get(3)) else {
        return Err("usage: seed_invitation <db> <email> <ics|inspect> [legacy] [uid]".into());
    };
    let mut store = Store::open(std::path::Path::new(path))?;
    let account = store
        .accounts()?
        .into_iter()
        .find(|a| a.email == *email)
        .ok_or("missing synthetic account")?
        .id;
    if source == "inspect" {
        for send in store.outbox_to_send(account)? {
            println!("{}", send.ics_reply.as_deref().unwrap_or(""));
        }
        return Ok(());
    }
    let mailbox = store
        .sync_state(account, "INBOX")?
        .ok_or("missing synthetic mailbox")?;
    let uid = args
        .get(5)
        .and_then(|s| s.parse().ok())
        .unwrap_or(store.max_uid(mailbox.mailbox_id)? + 1);
    let mut invitation =
        mail_core::extract_invitation(source, email).ok_or("invalid synthetic invitation")?;
    if args.get(4).is_some_and(|s| s == "legacy") {
        invitation.metadata_version = 0;
        invitation.occurrence_key = None;
        invitation.occurrence_property = None;
    }
    store.upsert_envelopes(
        mailbox.mailbox_id,
        &[Envelope {
            uid,
            subject: Some(invitation.title.clone()),
            sender: Some("Calendar owner".into()),
            sender_address: invitation.organizer_address.clone(),
            reply_to: None,
            message_id: Some(format!("<calendar-{uid}@example.fr>")),
            in_reply_to: None,
            date: Some(Utc::now()),
            seen: false,
            flagged: false,
            to_addrs: vec![email.clone()],
            cc_addrs: vec![],
        }],
    )?;
    store.save_body_full(
        mailbox.mailbox_id,
        uid,
        "<p>Cached calendar message available offline.</p>",
        &[],
        Some(&invitation),
    )?;
    println!("{uid}");
    Ok(())
}

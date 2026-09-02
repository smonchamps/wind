//! Diagnostic of draft synchronization.
//!
//! Answers the question the screen cannot show: **did the pull do its
//! job?** The draft banner shows only the subject and the recipient —
//! two successive versions of the same draft therefore look visually
//! identical, and "nothing changed" proves nothing.
//!
//! Same discipline as the other diagnostics: no subject, no recipient,
//! no body. Only technical markers and the SIZE of the text, which is
//! enough to tell two versions apart without revealing either one.
//!
//! ```powershell
//! cargo run -p mail-core --example diag_drafts -- "$env:APPDATA\dev.elements.wind\wind.db"
//! ```

use rusqlite::Connection;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: diag_drafts <path.db>")?;
    let conn = Connection::open(&path)?;
    println!("database: {path}\n");

    let mut stmt = conn.prepare(
        "SELECT d.id, a.email, d.remote_uid, d.updated_epoch, d.pushed_epoch,
                LENGTH(d.body), LENGTH(d.to_raw), LENGTH(d.subject)
         FROM drafts d LEFT JOIN accounts a ON a.id = d.account_id
         ORDER BY d.account_id, d.id",
    )?;
    #[allow(clippy::type_complexity)]
    let rows: Vec<(
        i64,
        Option<String>,
        Option<u32>,
        i64,
        Option<i64>,
        i64,
        i64,
        i64,
    )> = stmt
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
            ))
        })?
        .collect::<Result<_, _>>()?;

    println!("--- drafts ---");
    if rows.is_empty() {
        println!("none");
    }
    for (id, email, remote_uid, updated, pushed, body, to, subject) in rows {
        // "Mirror": a remote copy exists and nothing has been typed here
        // since. It is the only condition under which the pull allows
        // itself to replace it.
        let state = match (remote_uid, pushed) {
            (Some(_), Some(pushed)) if pushed >= updated => "mirror (replaceable)",
            (Some(_), Some(_)) => "EDITED HERE since the push",
            (Some(_), None) => "remote copy without marker",
            (None, _) => "never pushed",
        };
        let uid = remote_uid
            .map(|uid| uid.to_string())
            .unwrap_or_else(|| "—".to_string());
        println!(
            "#{id} [{}] remote uid {uid} — {state}\n    \
             text {body} chars, recipient {to} chars, subject {subject} chars\n    \
             modified {updated}, pushed {}",
            email.unwrap_or_else(|| "(unknown account)".to_string()),
            pushed
                .map(|epoch| epoch.to_string())
                .unwrap_or_else(|| "never".to_string()),
        );
    }

    println!("\n--- remote markers ---");
    let mut stmt = conn.prepare(
        "SELECT a.email, r.uid_validity,
                (SELECT COUNT(*) FROM draft_tombstones t WHERE t.account_id = r.account_id)
         FROM drafts_remote r LEFT JOIN accounts a ON a.id = r.account_id",
    )?;
    let markers: Vec<(Option<String>, i64, i64)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect::<Result<_, _>>()?;
    if markers.is_empty() {
        println!("none — the draft cycle has never completed");
    }
    for (email, validity, tombstones) in markers {
        println!(
            "{} : UIDVALIDITY {validity}, {tombstones} copy(ies) awaiting purge",
            email.unwrap_or_else(|| "(unknown account)".to_string())
        );
    }

    Ok(())
}

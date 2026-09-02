//! Diagnostic of the search index, account by account.
//!
//! Answers a single question: are the messages of a given account
//! present in the FTS5 index? A gap between "envelopes" and "indexed"
//! points at the defect.
//!
//! Shows only **counters**: no subject, no sender, no message content is
//! read or written.
//!
//! ```powershell
//! cargo run -p mail-core --example diag_index -- "$env:APPDATA\dev.elements.wind\wind.db"
//! ```

use rusqlite::Connection;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: diag_index <path.db>")?;
    let conn = Connection::open(&path)?;

    let indexed_exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'search_docs'",
        [],
        |row| row.get(0),
    )?;
    println!("database: {path}");
    println!(
        "search index: {}\n",
        if indexed_exists > 0 {
            "present"
        } else {
            "ABSENT — never created"
        }
    );
    if indexed_exists == 0 {
        return Ok(());
    }

    let mut stmt = conn.prepare(
        "SELECT a.id, a.email, a.provider,
                (SELECT COUNT(*) FROM envelopes e
                   JOIN mailboxes m ON m.id = e.mailbox_id
                  WHERE m.account_id = a.id),
                (SELECT COUNT(*) FROM search_docs d
                   JOIN mailboxes m ON m.id = d.mailbox_id
                  WHERE m.account_id = a.id),
                (SELECT COUNT(*) FROM bodies b
                   JOIN mailboxes m ON m.id = b.mailbox_id
                  WHERE m.account_id = a.id)
         FROM accounts a
         ORDER BY a.id",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, i64>(4)?,
            row.get::<_, i64>(5)?,
        ))
    })?;

    println!(
        "{:<3} {:<32} {:<8} {:>10} {:>10} {:>8}",
        "id", "account", "type", "envelopes", "indexed", "bodies"
    );
    for row in rows {
        let (id, email, provider, envelopes, indexed, bodies) = row?;
        let alert = if indexed < envelopes { "  <-- GAP" } else { "" };
        println!(
            "{id:<3} {email:<32} {provider:<8} {envelopes:>10} {indexed:>10} {bodies:>8}{alert}"
        );
    }

    let orphans: i64 = conn.query_row(
        "SELECT COUNT(*) FROM search_docs d
          WHERE NOT EXISTS (SELECT 1 FROM envelopes e
                             WHERE e.mailbox_id = d.mailbox_id AND e.uid = d.uid)",
        [],
        |row| row.get(0),
    )?;
    println!("\norphaned index entries (message gone): {orphans}");

    // The known mailboxes, to spot an account whose INBOX would carry a
    // different name than the one the unified mailbox queries.
    let mut boxes = conn.prepare(
        "SELECT m.account_id, m.name, COUNT(e.uid)
           FROM mailboxes m
           LEFT JOIN envelopes e ON e.mailbox_id = m.id
          GROUP BY m.id ORDER BY m.account_id, m.name",
    )?;
    println!("\nsynchronized mailboxes:");
    for row in boxes.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })? {
        let (account_id, name, count) = row?;
        println!("  account {account_id}: \"{name}\" — {count} messages");
    }

    if let Some(term) = std::env::args().nth(2) {
        probe_term(&conn, &path, &term)?;
    } else {
        println!("\n(add a word as the 2nd argument to probe search)");
    }
    Ok(())
}

/// Probes a term at TWO levels, to isolate the faulty layer: the raw
/// FTS index on one side, the `Store::search` API on the other. A gap
/// between the two points at the query, not the indexing.
///
/// Shows only counters: no subject is printed.
fn probe_term(conn: &Connection, path: &str, term: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== probing the term \"{term}\" ===");

    // Level 1: the FTS index alone, without any convenience join.
    let expression = format!("\"{}\"*", term.replace('"', ""));
    let mut raw = conn.prepare(
        "SELECT m.account_id, COUNT(*)
           FROM search_fts
           JOIN search_docs d ON d.docid = search_fts.rowid
           JOIN mailboxes m ON m.id = d.mailbox_id
          WHERE search_fts MATCH ?1
          GROUP BY m.account_id ORDER BY m.account_id",
    )?;
    println!("raw FTS index:");
    let mut any = false;
    for row in raw.query_map([&expression], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
    })? {
        let (account_id, count) = row?;
        println!("  account {account_id}: {count} match(es)");
        any = true;
    }
    if !any {
        println!("  no match — the word is not in the indexed fields");
    }

    // Level 2: what the product actually returns.
    let store = mail_core::Store::open(std::path::Path::new(path))?;
    let rows = store.search(term, 50)?;
    println!("Store::search: {} result(s)", rows.len());
    let mut per_account: std::collections::BTreeMap<String, usize> = Default::default();
    for row in &rows {
        *per_account.entry(row.account_email.clone()).or_default() += 1;
    }
    for (email, count) in per_account {
        println!("  {email}: {count}");
    }
    Ok(())
}

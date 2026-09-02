//! Diagnostic of the OPENING cost of the Store — born from gate P1 of the
//! redesign: on the real database (1.3 GB), `Store::open` cost ~500 ms
//! where the bench's synthetic database costs only a handful, and EVERY
//! Tauri command opens its own connection.
//!
//! Three stopwatches separate the layers:
//! 1. raw SQLite open (+ PRAGMA wal + SELECT 1) — the file/OS cost,
//!    outside our code;
//! 2. full `Store::open` — schema and migrations included;
//! 3. a SECOND `Store::open` in the same process — what remains is the
//!    cost paid on EVERY command, not a warm-up.
//!
//! Shows only **durations**: no subject, no sender, no message content
//! is read or written.
//!
//! ```powershell
//! cargo run -p mail-core --example diag_opening --release -- <path.db>
//! ```

use std::time::Instant;

use mail_core::Store;
use rusqlite::Connection;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: diag_opening <path.db>")?;
    println!("database: {path}");

    let start = Instant::now();
    let conn = Connection::open(&path)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    conn.query_row("PRAGMA journal_mode = wal", [], |row| {
        row.get::<_, String>(0)
    })?;
    let _one: i64 = conn.query_row("SELECT 1", [], |row| row.get(0))?;
    println!("raw open (SQLite + WAL + SELECT 1): {:?}", start.elapsed());
    drop(conn);

    let start = Instant::now();
    let store = Store::open(std::path::Path::new(&path))?;
    println!(
        "Store::open — first in the process       : {:?}",
        start.elapsed()
    );
    drop(store);

    let start = Instant::now();
    let store = Store::open(std::path::Path::new(&path))?;
    println!(
        "Store::open — second, same process        : {:?}",
        start.elapsed()
    );
    drop(store);

    // ——— The suspect: the orphan search, replayed on every open. Out
    // of scope, `thread_id` stays NULL forever (ADR 0010 §3): how many
    // rows does the query enumerate before discarding them?
    let conn = Connection::open(&path)?;
    let nulls: i64 = conn.query_row(
        "SELECT COUNT(*) FROM envelopes WHERE thread_id IS NULL",
        [],
        |row| row.get(0),
    )?;
    let in_scope: i64 = conn.query_row(
        "SELECT COUNT(*) FROM envelopes e JOIN mailboxes m ON m.id = e.mailbox_id
         WHERE m.threaded = 1",
        [],
        |row| row.get(0),
    )?;
    println!("\nenvelopes with thread_id NULL: {nulls} · in scope: {in_scope}");

    // CURRENT shape of `orphans()` (reduced projection, same plan).
    let current = "SELECT COUNT(*) FROM envelopes e
         JOIN mailboxes m ON m.id = e.mailbox_id
         WHERE e.thread_id IS NULL AND m.threaded = 1";
    // CANDIDATE shape: the in-scope mailboxes drive the scan —
    // CROSS JOIN pins the join order, the index (mailbox_id, uid)
    // carries the walk; out-of-scope ones are never enumerated.
    let candidate = "SELECT COUNT(*) FROM mailboxes m
         CROSS JOIN envelopes e ON e.mailbox_id = m.id
         WHERE m.threaded = 1 AND e.thread_id IS NULL";
    for (name, sql) in [("current", current), ("candidate", candidate)] {
        let start = Instant::now();
        let n: i64 = conn.query_row(sql, [], |row| row.get(0))?;
        println!("{name} shape: {n} orphan(s) in {:?}", start.elapsed());
        let mut stmt = conn.prepare(&format!("EXPLAIN QUERY PLAN {sql}"))?;
        let plan: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(3))?
            .collect::<Result<_, _>>()?;
        for line in plan {
            println!("  {line}");
        }
    }

    Ok(())
}

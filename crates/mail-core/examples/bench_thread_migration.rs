//! Bench of gate 3: what does adopting legacy messages cost?
//!
//! [`mail_core`] attaches to a thread, **when the database opens**, every
//! message that does not yet have one. It was instant on the 2,800
//! messages of the real mailbox. Gate 3 asks for 200,000, and this
//! adoption is charged entirely against the **startup** budget (< 1 s,
//! [`PLAN.md`] §1): that is the risk named at §8 of the handover.
//!
//! Two regimes are measured separately, because they do not cost the
//! same and do not happen at the same time:
//!
//! | Regime | When | What happens |
//! |---|---|---|
//! | **adoption** | legacy database, never grouped | each message gets attached |
//! | **up to date** | already-grouped database | only a `PRAGMA` is read |
//!
//! The second is the common case — the one the user pays at *every*
//! startup. The first is paid only once, but it is the one that can
//! blow the budget.
//!
//! Unlike the `diagnostic_*` ones, this bench **writes**: it therefore
//! works on a copy it makes itself with `VACUUM INTO`, and never touches
//! the targeted database. (`VACUUM` compacts along the way: the figure
//! is thus a slightly light lower bound, minus real-world fragmentation.)
//!
//! ```powershell
//! cargo run -p mail-core --example bench_thread_migration --release -- "<path.db>"
//! ```

use std::path::PathBuf;
use std::time::Instant;

use mail_core::Store;
use rusqlite::Connection;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: bench_thread_migration <path.db>")?;
    println!("database: {path}\n");

    let copy = working_copy(&path)?;

    // What the database contains, before anything else.
    let source = Connection::open(&copy)?;
    let messages: i64 = source.query_row("SELECT COUNT(*) FROM envelopes", [], |row| row.get(0))?;
    let mailboxes: i64 =
        source.query_row("SELECT COUNT(*) FROM mailboxes", [], |row| row.get(0))?;
    println!("{messages} messages, {mailboxes} mailbox(es)");

    // Rewind: a lagging version marker is enough to make the grouping
    // start over from zero on the next open — exactly the state of a
    // database that has never seen conversations.
    source.execute_batch("PRAGMA user_version = 0;")?;
    drop(source);

    let start = Instant::now();
    let store = Store::open(&copy)?;
    let adoption = start.elapsed();
    drop(store);

    // Second open: the database is now up to date. This is the cost
    // the user pays at EVERY startup, and the only one that matters
    // for the budget under the common regime.
    let start = Instant::now();
    let store = Store::open(&copy)?;
    let up_to_date = start.elapsed();
    drop(store);

    let verif = Connection::open(&copy)?;
    let threads: i64 = verif.query_row("SELECT COUNT(*) FROM threads", [], |row| row.get(0))?;
    let links: i64 = verif.query_row("SELECT COUNT(*) FROM thread_links", [], |row| row.get(0))?;
    let orphans: i64 = verif.query_row(
        "SELECT COUNT(*) FROM envelopes WHERE thread_id IS NULL",
        [],
        |row| row.get(0),
    )?;

    println!("\n--- opening ---");
    println!("adoption (legacy database): {adoption:?}");
    println!("up to date (common case)  : {up_to_date:?}");
    println!("\n--- grouping result ---");
    println!("{threads} thread(s), {links} directory link(s), {orphans} unattached message(s)");
    if orphans > 0 {
        println!("⚠ an unattached message has NO row in the list (ADR 0008 §4)");
    }

    let _ = std::fs::remove_file(&copy);
    Ok(())
}

/// A consistent copy, without touching the targeted database or its WAL.
fn working_copy(path: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let copy = std::env::temp_dir().join("banc-migration-fils.db");
    let _ = std::fs::remove_file(&copy);
    let source = Connection::open(path)?;
    // `VACUUM INTO` reads the database as it is, WAL included, and writes
    // a standalone file. No write to the original.
    source.execute("VACUUM INTO ?1", [copy.to_string_lossy().as_ref()])?;
    Ok(copy)
}

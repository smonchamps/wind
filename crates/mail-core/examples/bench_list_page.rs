//! Bench of gate 3: does the cost of a list page depend on the size
//! of the mailbox?
//!
//! [ADR 0008] §4 bases the whole grouping on a promise: the list starts
//! from the materialized aggregate `threads`, whose index
//! `idx_threads_date` carries **both the sort and the pagination**, so
//! "the cost of a page no longer depends on the size of the mailbox".
//!
//! Gate 3 puts it to the test: 87 ms per page on 160,000 conversations,
//! against 3.8 ms on the 2,727 of the real mailbox.
//!
//! The original index was `threads(mailbox_id, last_epoch DESC,
//! last_uid DESC)`. The **unified mailbox** queries the same mailbox of
//! ALL accounts: it filters on `m.name = 'INBOX'`, not on a
//! `mailbox_id`. An index prefixed by that column could therefore no
//! longer carry the global order, and SQLite fell back to a materialized
//! sort.
//!
//! **Confirmed, then fixed**: `idx_threads_date_globale` carries the
//! same sort without a mailbox prefix. The bench stays — it is what will
//! detect the next regression, and a unit test guards the plan
//! (`la_boite_unifiee_ne_materialise_pas_son_tri`).
//!
//! Deep scrolling, for its part, was fixed at gate P1 of the redesign
//! (2026-08-11): pagination lives in a subquery on `threads` alone, and
//! `OFFSET` no longer runs joins and `EXISTS` on the skipped rows —
//! core measured from 252.6 to 14.6 ms at offset 200,000. The control
//! below reflects this shape.
//!
//! Read-only: no write, no copy.
//!
//! ```powershell
//! cargo run -p mail-core --example bench_list_page --release -- "<path.db>"
//! ```

use std::time::Instant;

use mail_core::Store;
use rusqlite::Connection;

/// The exact shape of the sort and pagination of `Store::unified_recent`.
/// The projection is reduced: it does not change the sort STRATEGY,
/// which is all the plan needs to tell us.
const UNIFIED_PAGE: &str = "SELECT t.last_uid
     FROM (SELECT account_id, last_mailbox_id, last_uid, last_epoch
             FROM threads
            WHERE inbox_size > 0
            ORDER BY last_epoch DESC, last_uid DESC, account_id
            LIMIT 200 OFFSET 0) t
     JOIN envelopes e ON e.mailbox_id = t.last_mailbox_id AND e.uid = t.last_uid
     JOIN mailboxes m ON m.id = e.mailbox_id
     JOIN accounts a ON a.id = t.account_id
     ORDER BY t.last_epoch DESC, t.last_uid DESC, a.id";

/// The same, but bounded to ONE account: the prefixed index becomes usable again.
const ONE_ACCOUNT_PAGE: &str = "SELECT t.id
     FROM threads t
     JOIN envelopes e ON e.mailbox_id = t.last_mailbox_id AND e.uid = t.last_uid
     WHERE t.account_id = ?1
     ORDER BY t.last_epoch DESC, t.last_uid DESC
     LIMIT 200 OFFSET 0";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: bench_list_page <path.db>")?;
    println!("database: {path}\n");

    // Open the Store FIRST: it is the one that applies the schema, hence
    // that creates the indexes. Querying a plan before this open would
    // describe a database the application never uses.
    let start = Instant::now();
    let store = Store::open(std::path::Path::new(&path))?;
    println!("open (schema applied): {:?}", start.elapsed());
    drop(store);

    let conn = Connection::open(&path)?;
    let threads: i64 = conn.query_row("SELECT COUNT(*) FROM threads", [], |row| row.get(0))?;
    let mailboxes: i64 = conn.query_row("SELECT COUNT(*) FROM mailboxes", [], |row| row.get(0))?;
    // The list paginates ONLY threads with at least one received message.
    // Taking the total would measure pages that do not exist — the bench
    // used to return "0 rows" while announcing a duration, which measures
    // nothing.
    //
    // The gap between the two is itself the interesting figure: it is
    // what the partial index excludes (ADR 0009 §4).
    let visible: i64 = conn.query_row(
        "SELECT COUNT(*) FROM threads WHERE inbox_size > 0",
        [],
        |row| row.get(0),
    )?;
    println!("{threads} conversations spread across {mailboxes} mailbox(es)");
    println!("of which {visible} with at least one received message — only those are paginated");

    println!("\n--- plan of the unified mailbox ---");
    plan(&conn, UNIFIED_PAGE, rusqlite::params![])?;

    let one: Option<i64> = conn
        .query_row("SELECT id FROM accounts LIMIT 1", [], |row| row.get(0))
        .ok();
    if let Some(account_id) = one {
        println!("\n--- plan of ONE account (control) ---");
        plan(&conn, ONE_ACCOUNT_PAGE, rusqlite::params![account_id])?;
    }
    drop(conn);

    // The real path, the one the UI takes on every scroll page. Three
    // depths: if the cost follows the OFFSET, pagination is not carried
    // by the index.
    let store = Store::open(std::path::Path::new(&path))?;
    println!("\n--- real cost of a page (Store::unified_recent, 200 rows) ---");
    for offset in [0usize, 20_000, 80_000, 150_000] {
        if offset as i64 >= visible {
            continue;
        }
        // Two rounds: the first warms the SQLite page cache, the second
        // measures the established regime — the one scrolling has.
        let _ = store.unified_recent(offset, 200)?;
        let start = Instant::now();
        let rows = store.unified_recent(offset, 200)?;
        println!(
            "offset {offset:>7}: {:>8.2} ms ({} rows)",
            start.elapsed().as_secs_f64() * 1000.0,
            rows.len()
        );
    }
    Ok(())
}

fn plan(conn: &Connection, sql: &str, params: &[&dyn rusqlite::ToSql]) -> rusqlite::Result<()> {
    let mut stmt = conn.prepare(&format!("EXPLAIN QUERY PLAN {sql}"))?;
    let lines: Vec<String> = stmt
        .query_map(params, |row| row.get::<_, String>(3))?
        .collect::<Result<_, _>>()?;
    for line in lines {
        // The word that decides. "FOR ORDER BY" alone = SQLite sorts
        // EVERYTHING: no index carries the order, the cost follows the
        // size of the mailbox. "FOR LAST TERM OF ORDER BY" only breaks
        // ties on the last criterion — negligible, and not to be
        // confused with the previous one on pain of crying wolf.
        let verdict = if line.contains("TEMP B-TREE FOR ORDER BY") {
            "  ← FULL SORT: the promise of ADR 0008 §4 is broken"
        } else if line.contains("TEMP B-TREE") {
            "  ← partial sort (ties only), no consequence"
        } else {
            ""
        };
        println!("  {line}{verdict}");
    }
    Ok(())
}

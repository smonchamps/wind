//! Bench of Spring cleaning and the Screener (PLAN-AUDIT-V2 E4): what do
//! the unbounded reads of the audit cost — groups, mail of a group,
//! screener waiting, pile, routings — and the verdict on the biggest
//! group, on a given database? Durations and counts only: no subject,
//! no sender printed.
//!
//! ⚠️ MUTATES the database: organized mode set, a cleanup session opened
//! then closed, the biggest group ARCHIVED. Run against a fixture, never
//! against a real database.
//!
//! ```powershell
//! cargo run -p mail-core --example bench_cleanup --release -- <path.db>
//! ```

use std::time::Instant;

use mail_core::Store;

fn timed<T>(
    label: &str,
    f: impl FnOnce() -> Result<T, mail_core::Error>,
) -> Result<T, mail_core::Error> {
    let start = Instant::now();
    let value = f()?;
    println!(
        "{label:<26} {:>9.2} ms",
        start.elapsed().as_secs_f64() * 1000.0
    );
    Ok(value)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: bench_cleanup <path.db>")?;
    let mut store = Store::open(std::path::Path::new(&path))?;
    let now = chrono::Utc::now().timestamp();
    store.set_organized_mode(true, 0)?;
    // The screener waiting list fills up on ARRIVAL (upsert under
    // organized mode); on an already-seeded fixture it is populated by
    // hand: every pending sender — the worst case of the measured read.
    {
        let conn = rusqlite::Connection::open(&path)?;
        conn.execute_batch(
            "INSERT OR IGNORE INTO screener_waiting (address)
             SELECT DISTINCT sender_norm FROM envelopes WHERE sender_norm IS NOT NULL",
        )?;
    }

    let session = timed("cleanup_start", || {
        store.cleanup_start("tout", "dossiersArchives", now)
    })?;
    println!("  {} groups announced", session.total);
    let groups = timed("cleanup_groups", || store.cleanup_groups())?;
    println!("  {} groups returned", groups.len());
    let biggest = groups
        .iter()
        .max_by_key(|group| group.messages)
        .ok_or("no group: is the database empty?")?
        .clone();
    let messages = timed("cleanup_messages (biggest)", || {
        store.cleanup_messages(&biggest.address)
    })?;
    println!("  {} messages in the biggest group", messages.len());
    let waiting = timed("screener_waiting", || store.screener_waiting())?;
    println!("  {} waiting", waiting.len());
    let pile = timed("set_aside_pile", || store.set_aside_pile())?;
    println!("  {} set aside", pile.len());
    let routings = timed("routings", || store.routings())?;
    println!("  {} routings", routings.len());
    let processed = timed("cleanup_verdict (biggest)", || {
        store.cleanup_verdict(&biggest.address, "ecarte", Some("archive"), now)
    })?;
    println!("  {processed} messages archived by the verdict");
    store.cleanup_finish()?;
    Ok(())
}

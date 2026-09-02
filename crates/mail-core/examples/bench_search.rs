//! Bench of gate 3: do search and opening a message hold their budgets
//! at scale?
//!
//! | Budget | Target |
//! |---|---|
//! | Search | < 100 ms |
//! | Opening a message | < 50 ms |
//!
//! Protocol of [ADR 0004]: we measure `search_capped` — WHAT production
//! pays per keystroke (top-100, the production `SEARCH_LIMIT`; COUNT of
//! the total for "N of M"; switches to date sort past
//! `WIDE_QUERY_THRESHOLD`), with **the number of matches shown next to
//! each duration**. Without it a search figure means nothing — FTS5's
//! cost follows the number of matches, since `ORDER BY rank` computes
//! BM25 over all of them. A fast query on a rare term proves nothing.
//!
//! The ADR even names the breaking point: a query matching 69-90% of the
//! corpus goes over budget at 200,000 messages. The bench plays it on
//! purpose, to know where we stand.
//!
//! Read-only.
//!
//! ```powershell
//! cargo run -p mail-core --example bench_search --release -- "<path.db>"
//! ```

use std::time::Instant;

use mail_core::Store;
use rusqlite::Connection;

/// What the user types, in the order they type it.
///
/// **The last term is ALWAYS a prefix**: `parse_query` builds
/// `"term"*` — that is search-as-you-type. The prefix query is therefore
/// not an edge case, it is the normal path, and it is the most
/// expensive. Measuring a whole word without its star would measure
/// nothing of what the product runs.
///
/// Hence a progressive keystroke: each row is a real state of the search
/// field, starting at three characters (the trigger threshold).
const QUERIES: [(&str, &str); 6] = [
    ("rare term (tail)", "ref12345"),
    ("3 chars — the threshold", "fac"), // lang:fr — "fac" is a French prefix, matched against the real (French) corpus
    ("5 chars", "factu"),               // lang:fr — same reason
    ("whole word", "facture"),          // lang:fr — French word matched against the real corpus
    ("two terms", "facture réu"),       // lang:fr — French words matched against the real corpus
    ("very common word", "réunion"),    // lang:fr — French word matched against the real corpus
];

/// The rendering cap, aligned on the `SEARCH_LIMIT` of the
/// `search_messages` command: measuring a different number than what
/// production renders (and the COUNT of the total it pays when capped)
/// would misrepresent the real cost. Set to 100 in the field: 200 went
/// over budget on a very common 3-character prefix (row hydration, not
/// the COUNT).
const SEARCH_LIMIT: usize = 100;

/// The FTS expression that `search` will build — reproduced here so the
/// number of matches lines up with the measured duration.
///
/// Coupling accepted and named: if `parse_query` changes its rule, this
/// bench lies. That is the price of an accurate count without opening
/// the core's API.
fn fts_expression(input: &str) -> String {
    let terms: Vec<&str> = input.split_whitespace().collect();
    let last = terms.len().saturating_sub(1);
    terms
        .iter()
        .enumerate()
        .map(|(i, t)| {
            if i == last {
                format!("\"{t}\"*")
            } else {
                format!("\"{t}\"")
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: bench_search <path.db>")?;
    println!("database: {path}\n");

    let conn = Connection::open(&path)?;
    let messages: i64 = conn.query_row("SELECT COUNT(*) FROM envelopes", [], |row| row.get(0))?;
    let bodies: i64 = conn.query_row("SELECT COUNT(*) FROM bodies", [], |row| row.get(0))?;
    println!("{messages} messages, {bodies} stored bodies");
    drop(conn);

    let store = Store::open(std::path::Path::new(&path))?;

    println!("\n--- search (search_capped: count + sort + render, budget < 100 ms) ---");
    for (label, query) in QUERIES {
        // `search_capped` is WHAT production pays per keystroke: the
        // COUNT of the total, the switch to date sort past the wide
        // query threshold, and the capped render. One blank round
        // (established regime, warm), then the measurement.
        let _ = store.search_capped(query, SEARCH_LIMIT, 0)?;
        let start = Instant::now();
        let (results, total) = store.search_capped(query, SEARCH_LIMIT, 0)?;
        let cost = start.elapsed().as_secs_f64() * 1000.0;
        let date_sort = total > mail_core::WIDE_QUERY_THRESHOLD;
        let verdict = if cost > 100.0 {
            "  ✗ OVER BUDGET"
        } else {
            ""
        };
        println!(
            "{label:<24} \"{query:<12}\" {cost:>7.2} ms — {:>3} rendered out of {total} match(es){}{verdict}",
            results.len(),
            if date_sort { " (date sort)" } else { " (BM25)" },
        );
    }

    // The COUNT alone (PLAN-AUDIT-V2 E2): the share of counting in the
    // keystroke — measured at 1.5 ms out of 57 for "fac" on 200k; a
    // COUNT capped at the threshold (LIMIT 10,001) only gained 1: the
    // count is not the cost, the date-sorted page is. Removed from
    // production, the section stays here.
    println!("\n--- count alone (the share of COUNT in the keystroke) ---");
    for (label, query) in QUERIES {
        let _ = store.search_total(query)?;
        let start = Instant::now();
        let exact = store.search_total(query)?;
        let cost = start.elapsed().as_secs_f64() * 1000.0;
        println!("{label:<24} \"{query:<12}\" {cost:>7.2} ms ({exact} match(es))");
    }

    // The hard point of the "load more" job: does OFFSET hold the budget
    // in depth? The soft bound is ~1000 rows = 10 batches, so pages 1,
    // 5, 10 are measured. If OFFSET degrades, plan B is a cursor
    // (date sort only) — but enumeration should dominate the jump,
    // leaving the cost ~flat.
    println!("\n--- deep OFFSET pagination (load more, budget < 100 ms) ---");
    for (label, query) in QUERIES {
        let mut line = format!("{label:<24} \"{query:<12}\"");
        let mut over = false;
        for page in [0usize, 4, 9] {
            let offset = page * SEARCH_LIMIT;
            let _ = store.search_capped(query, SEARCH_LIMIT, offset)?;
            let start = Instant::now();
            let (rendered, _) = store.search_capped(query, SEARCH_LIMIT, offset)?;
            let cost = start.elapsed().as_secs_f64() * 1000.0;
            over |= cost > 100.0;
            line.push_str(&format!(
                "  p{}={cost:>6.1}ms({})",
                page + 1,
                rendered.len()
            ));
        }
        if over {
            line.push_str("  ✗ OVER BUDGET");
        }
        println!("{line}");
    }

    println!("\n--- opening a message (budget < 50 ms) ---");
    openings(&store, &path)?;

    compare_sorts(&path)?;
    Ok(())
}

/// Relevance against date, at equal matches.
///
/// `ORDER BY rank` computes BM25 over **all** matches: it is the
/// dominant item measured above, ahead of prefix expansion. Sorting by
/// date does not remove it for free — matches still have to be
/// enumerated, and sorted — but it avoids the score computation. What
/// remains is to know what that is worth: hence this comparison.
///
/// The core ALREADY switches to date when the query has no terms (a
/// BM25 without a term makes no sense). The question is therefore
/// whether to switch it there too when there are terms.
fn compare_sorts(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    const BASE: &str = "SELECT e.uid
         FROM search_fts
         JOIN search_docs d ON d.docid = search_fts.rowid
         JOIN envelopes e ON e.mailbox_id = d.mailbox_id AND e.uid = d.uid
         JOIN mailboxes m ON m.id = e.mailbox_id
         JOIN accounts a ON a.id = m.account_id
         WHERE search_fts MATCH ?1
         ORDER BY ";

    let conn = Connection::open(path)?;
    println!("\n--- relevance (BM25) against date, at equal matches ---");
    for (label, input) in QUERIES {
        let expression = fts_expression(input);
        let mut durations = Vec::new();
        for order in [
            "bm25(search_fts, 10.0, 5.0, 3.0, 1.0), e.date_epoch DESC",
            "e.date_epoch DESC, e.uid DESC",
        ] {
            let sql = format!("{BASE}{order} LIMIT 50");
            let mut stmt = conn.prepare(&sql)?;
            // One blank round, then the measurement: same protocol as above.
            let _ = stmt
                .query_map([&expression], |row| row.get::<_, u32>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            let start = Instant::now();
            let rows = stmt
                .query_map([&expression], |row| row.get::<_, u32>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            durations.push((start.elapsed().as_secs_f64() * 1000.0, rows.len()));
        }
        let (bm25, _) = durations[0];
        let (date, _) = durations[1];
        let gain = if date > 0.0 { bm25 / date } else { 0.0 };
        println!(
            "{label:<24} BM25 {bm25:>7.2} ms — date {date:>7.2} ms — ×{gain:.1}{}",
            if date > 100.0 {
                "  ✗ still over budget"
            } else {
                ""
            }
        );
    }
    Ok(())
}

/// Is the body served from cache fast enough? We take messages that
/// HAVE one: measuring an absence measures nothing.
fn openings(store: &Store, path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let conn = Connection::open(path)?;
    let mut stmt = conn.prepare(
        "SELECT m.account_id, m.name, b.uid
         FROM bodies b JOIN mailboxes m ON m.id = b.mailbox_id
         ORDER BY b.uid DESC LIMIT 5",
    )?;
    let targets: Vec<(i64, String, u32)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect::<Result<_, _>>()?;
    drop(stmt);
    drop(conn);

    for (account_id, mailbox, uid) in targets {
        let start = Instant::now();
        let body = store.body(account_id, &mailbox, uid)?;
        let duration = start.elapsed().as_secs_f64() * 1000.0;
        let verdict = if duration > 50.0 {
            "  ✗ OVER BUDGET"
        } else {
            ""
        };
        println!(
            "account {account_id} uid {uid:<6}: {duration:>6.2} ms — {} bytes{verdict}",
            body.map(|html| html.len()).unwrap_or(0)
        );
    }
    Ok(())
}

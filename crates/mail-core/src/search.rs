//! Local full-text search — FTS5, the decision frozen by ADR 0004.
//!
//! Three invariants structure this module:
//! - **the index lives INSIDE the database**: every mutation point of the
//!   messages ([`Store::upsert_envelopes`], deletions, [`Store::save_body`],
//!   UIDVALIDITY reset) maintains the index within ITS OWN transaction —
//!   no second store, no reconciliation after a crash;
//! - **the index is "contentless"** (`content=''`, `contentless_delete`):
//!   no text is duplicated, only the inverted index is stored — the size
//!   vigilance of ADR 0004;
//! - **input is NEVER FTS5 syntax**: every term is neutralized inside
//!   quotes — `AND`, `(`, `*` are words like any other.
//!
//! `envelopes` rowids are unstable (`INSERT OR REPLACE`): the
//! `search_docs` table assigns a stable docid per `(mailbox_id, uid)`.
//! It has no foreign key by design — maintenance goes exclusively
//! through this module's functions, never through a silent CASCADE.

use std::collections::HashMap;
use std::ops::ControlFlow;

use chrono::NaiveDate;
use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension, params, params_from_iter};

use crate::envelope::Uid;
use crate::error::Error;
use crate::store::{AdoptionProgress, SELECT_UNIFIED, Store, UnifiedRow, row_to_unified};

/// Creates the index on the first opening that finds it absent, and
/// rebuilds it from the messages already in the database: a database from
/// earlier phases becomes searchable without resyncing.
///
/// The rebuild is **visible and interruptible** (ADR 0012): it reports
/// its progress through `on_progress` and, on `ControlFlow::Break`,
/// returns [`Error::Interrupted`] — the inner transaction rewinds (the
/// `DROP` of the old index is undone), and the pass replays on the
/// next launch. It is [`Store::pending_adoption`] that makes the screen
/// show up: without it, this rebuild would freeze startup silently
/// (field finding 2026-08-17).
pub(crate) fn migrate_search(
    conn: &Connection,
    on_progress: &mut dyn FnMut(AdoptionProgress) -> ControlFlow<()>,
) -> Result<(), Error> {
    // The current schema carries the `recipients` column and the
    // `prefix='2 3'` option. An earlier database (`search_fts` with three
    // columns, no prefix) is rebuilt: FTS5 cannot add a column, and
    // `prefix=` must exist at creation. Drop + rebuild in ONE
    // transaction — the index is rebuildable from the messages, never
    // a source of truth. The marker is the presence of `recipients`
    // in the `CREATE` recorded by `sqlite_master`.
    let fts_sql: Option<String> = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'search_fts'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if fts_sql
        .as_deref()
        .is_some_and(|sql| sql.contains("recipients"))
    {
        return Ok(());
    }
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch(
        "DROP TABLE IF EXISTS search_fts;
         DROP TABLE IF EXISTS search_docs;
         CREATE TABLE search_docs (
            docid      INTEGER PRIMARY KEY,
            mailbox_id INTEGER NOT NULL,
            uid        INTEGER NOT NULL,
            UNIQUE (mailbox_id, uid)
         );
         CREATE VIRTUAL TABLE search_fts USING fts5(
            subject, sender, recipients, body,
            prefix='2 3',
            content='', contentless_delete=1,
            tokenize='unicode61 remove_diacritics 2'
         );",
    )?;
    rebuild(&tx, on_progress)?;
    tx.commit()?;
    Ok(())
}

/// How many messages between two progress reports: rare enough not to pay
/// a call per message, frequent enough for cancellation to answer within
/// a fraction of a second.
const REBUILD_STEP: u64 = 1000;

fn rebuild(
    conn: &Connection,
    on_progress: &mut dyn FnMut(AdoptionProgress) -> ControlFlow<()>,
) -> Result<(), Error> {
    // The exact denominator of the progress — `pending_adoption` could only
    // give an order of magnitude (a read-only probe).
    let total: u64 = conn.query_row("SELECT COUNT(*) FROM envelopes", [], |row| {
        row.get::<_, i64>(0)
    })? as u64;
    let mut stmt = conn.prepare(
        "SELECT e.mailbox_id, e.uid, e.subject, e.sender, e.sender_address,
                e.to_addrs, e.cc_addrs, b.html
         FROM envelopes e
         LEFT JOIN bodies b ON b.mailbox_id = e.mailbox_id AND b.uid = e.uid",
    )?;
    // Stream, never collect: a `Vec` of all rows would load ALL bodies
    // into memory (~7 GB in the field). Indexing follows the cursor —
    // it reads `envelopes`/`bodies`, the write goes to `search_fts`/
    // `search_docs`, disjoint tables: SQLite serves both on the same
    // connection.
    let mut rows = stmt.query([])?;
    let mut done: u64 = 0;
    while let Some(row) = rows.next()? {
        let subject: Option<String> = row.get(2)?;
        let sender: Option<String> = row.get(3)?;
        let address: Option<String> = row.get(4)?;
        let to: Option<String> = row.get(5)?;
        let cc: Option<String> = row.get(6)?;
        let html: Option<String> = row.get(7)?;
        index_message(
            conn,
            row.get(0)?,
            row.get(1)?,
            Indexed {
                subject: subject.as_deref(),
                sender: sender.as_deref(),
                sender_address: address.as_deref(),
                to_addrs: to.as_deref(),
                cc_addrs: cc.as_deref(),
                body_html: html.as_deref(),
            },
        )?;
        done += 1;
        // Step: report progress and watch for cancellation.
        if done.is_multiple_of(REBUILD_STEP) {
            report(on_progress, done, total)?;
        }
    }
    // "Done" is only said here, the pass complete — and never on a database
    // with no message (nothing to report, no fake banner: the threads' contract).
    if total > 0 {
        report(on_progress, total, total)?;
    }
    Ok(())
}

/// Passes on a progress report, and translates the response: `Break`
/// becomes [`Error::Interrupted`], which the caller's transaction
/// turns into a `ROLLBACK` — the §8 rewind, as for threads.
fn report(
    on_progress: &mut dyn FnMut(AdoptionProgress) -> ControlFlow<()>,
    done: u64,
    total: u64,
) -> Result<(), Error> {
    match on_progress(AdoptionProgress { done, total }) {
        ControlFlow::Continue(()) => Ok(()),
        ControlFlow::Break(()) => Err(Error::Interrupted),
    }
}

/// The fields of a message as they enter the index. Named by design: six
/// positional `Option<&str>` would swap places without the compiler or
/// the tests ever seeing it (`to`/`cc` and `sender`/`sender_address`
/// merge into a single FTS field).
pub(crate) struct Indexed<'a> {
    pub subject: Option<&'a str>,
    pub sender: Option<&'a str>,
    pub sender_address: Option<&'a str>,
    pub to_addrs: Option<&'a str>,
    pub cc_addrs: Option<&'a str>,
    pub body_html: Option<&'a str>,
}

/// Joins the present fields into a single searchable field, separated by
/// a space (unicode61 tokenizes on any whitespace, `\n` included).
fn join_present(a: Option<&str>, b: Option<&str>) -> String {
    [a, b]
        .iter()
        .flatten()
        .copied()
        .collect::<Vec<_>>()
        .join(" ")
}

/// (Re)indexes a message. Call it in the transaction that writes the
/// message itself — the index and the data live or die together.
pub(crate) fn index_message(
    conn: &Connection,
    mailbox_id: i64,
    uid: Uid,
    msg: Indexed<'_>,
) -> Result<(), Error> {
    deindex_message(conn, mailbox_id, uid)?;
    conn.execute(
        "INSERT INTO search_docs (mailbox_id, uid) VALUES (?1, ?2)",
        params![mailbox_id, uid],
    )?;
    let docid = conn.last_insert_rowid();
    let sender_field = join_present(msg.sender, msg.sender_address);
    // `to` and `cc` are a single searchable field: a recipient stays a
    // recipient, whether direct or in copy.
    let recipients_field = join_present(msg.to_addrs, msg.cc_addrs);
    conn.execute(
        "INSERT INTO search_fts (rowid, subject, sender, recipients, body)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            docid,
            msg.subject.unwrap_or(""),
            sender_field,
            recipients_field,
            msg.body_html.map(indexable_text).unwrap_or_default()
        ],
    )?;
    Ok(())
}

pub(crate) fn deindex_message(conn: &Connection, mailbox_id: i64, uid: Uid) -> Result<(), Error> {
    let docid: Option<i64> = conn
        .query_row(
            "SELECT docid FROM search_docs WHERE mailbox_id = ?1 AND uid = ?2",
            params![mailbox_id, uid],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(docid) = docid {
        conn.execute("DELETE FROM search_fts WHERE rowid = ?1", [docid])?;
        conn.execute("DELETE FROM search_docs WHERE docid = ?1", [docid])?;
    }
    Ok(())
}

pub(crate) fn deindex_mailbox(conn: &Connection, mailbox_id: i64) -> Result<(), Error> {
    let docids: Vec<i64> = conn
        .prepare("SELECT docid FROM search_docs WHERE mailbox_id = ?1")?
        .query_map([mailbox_id], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    for docid in docids {
        conn.execute("DELETE FROM search_fts WHERE rowid = ?1", [docid])?;
    }
    conn.execute(
        "DELETE FROM search_docs WHERE mailbox_id = ?1",
        [mailbox_id],
    )?;
    Ok(())
}

/// Beyond this number of matches, [`Store::search_capped`] switches from
/// BM25 ranking to date sort: BM25 over this many matches exceeds the
/// budget (ADR 0004), and for such a broad query relevance ranking does
/// not mean anything anyway. Value calibrated in the field (2026-08-17):
/// queries with ~8,000 matches hold the BM25 budget (~45 ms), one at
/// 36,000 exceeded it (~101 ms). Since the BM25 cost per match is stable,
/// this threshold holds as the corpus grows.
pub const WIDE_QUERY_THRESHOLD: u64 = 10_000;

impl Store {
    /// Search across ALL accounts — the results are rows of the unified
    /// mailbox, sorted by relevance (BM25; a word in the subject weighs
    /// more than a word in the body). The last term is a prefix: "budg"
    /// finds "budgétaire" while typing.
    ///
    /// Filters: `from:`/`de:` (name or address of the sender),
    /// `to:`/`à:` (name or address of a recipient, direct or in copy),
    /// `date:YYYY`, `date:YYYY-MM`, `date:YYYY-MM-DD`. A filter alone,
    /// with no term, lists the matching messages by date.
    pub fn search(&self, input: &str, limit: usize) -> Result<Vec<UnifiedRow>, Error> {
        self.run_search(input, limit, 0, false)
    }

    /// Search sorted by DATE (most recent first), relevance ignored. BM25
    /// ranking of a very broad query (a 3-character prefix, tens of
    /// thousands of matches) does not mean anything, and its cost exceeds
    /// the budget (ADR 0004); date is then the best order.
    /// [`Store::search_capped`] switches to it beyond
    /// [`WIDE_QUERY_THRESHOLD`].
    pub fn search_recent(&self, input: &str, limit: usize) -> Result<Vec<UnifiedRow>, Error> {
        self.run_search(input, limit, 0, true)
    }

    /// Search as the UI consumes it: the rows of the `[offset,
    /// offset+limit)` slice AND the exact total of matches, to say "N of
    /// M" and serve "load more". Switches to date sort beyond
    /// [`WIDE_QUERY_THRESHOLD`] matches — the total COUNT, computed
    /// anyway, informs the switch AND says how many batches remain. The
    /// sort depends only on the total: it is the same from one page to
    /// the next, so the slices chain without gap or duplicate.
    ///
    /// The COUNT per keystroke is NOT the cost (measured at PLAN-AUDIT-V2
    /// E2 on 200k: 1.5 ms out of a total of 57 ms for a three-letter
    /// prefix, the rest is the page sorted by date) — `bench_search`
    /// re-measures it, "count only" section.
    pub fn search_capped(
        &self,
        input: &str,
        limit: usize,
        offset: usize,
    ) -> Result<(Vec<UnifiedRow>, u64), Error> {
        let total = self.search_total(input)?;
        let rows = self.run_search(input, limit, offset, total > WIDE_QUERY_THRESHOLD)?;
        Ok((rows, total))
    }

    /// In TWO steps, so that "load more" holds the budget in depth (field
    /// 2026-08-17: `LIMIT ? OFFSET ?` on the hydrated query degrades to
    /// O(offset) — SQLite hydrates the skipped rows via `SELECT_UNIFIED`
    /// then discards them). Phase 1: the ordered KEYS of the slice, the
    /// OFFSET then only skips keys, cheap. Phase 2: hydrate ONLY the
    /// page's keys, reordered as in phase 1. `force_date` forces the date
    /// sort (the safety valve for too-broad queries).
    fn run_search(
        &self,
        input: &str,
        limit: usize,
        offset: usize,
        force_date: bool,
    ) -> Result<Vec<UnifiedRow>, Error> {
        let keys = self.page_keys(input, limit, offset, force_date)?;
        self.hydrate_in_order(&keys)
    }

    /// Phase 1: the `(mailbox_id, uid)` of the slice, in order, WITHOUT
    /// hydration (no output joins, no attachment subquery, no body) — the
    /// OFFSET only skips these lightweight keys. The order is TOTAL
    /// (`… , e.uid DESC` as the final tiebreaker) so that the slices
    /// chain without gap or duplicate even at equal matches and dates.
    fn page_keys(
        &self,
        input: &str,
        limit: usize,
        offset: usize,
        force_date: bool,
    ) -> Result<Vec<(i64, Uid)>, Error> {
        let (match_expr, has_terms, filters) = build_match(input);
        if match_expr.is_none() && filters.since.is_none() && filters.until.is_none() {
            return Ok(Vec::new());
        }
        let (clauses, date_values) = date_clauses(&filters);
        let mut values: Vec<Value> = Vec::new();
        if let Some(expr) = &match_expr {
            values.push(expr.clone().into());
        }
        values.extend(date_values);
        values.push((limit as i64).into());
        values.push((offset as i64).into());
        let sql = if match_expr.is_some() {
            // Relevance when there are terms AND the query is not too
            // broad; date otherwise — filter alone (BM25 then means
            // nothing), or very broad query (`force_date`, the safety
            // valve).
            let order = if has_terms && !force_date {
                "bm25(search_fts, 10.0, 5.0, 3.0, 1.0), e.date_epoch DESC, e.uid DESC"
            } else {
                "e.date_epoch DESC, e.uid DESC"
            };
            format!(
                "SELECT d.mailbox_id, d.uid
                 FROM search_fts
                 JOIN search_docs d ON d.docid = search_fts.rowid
                 JOIN envelopes e ON e.mailbox_id = d.mailbox_id AND e.uid = d.uid
                 WHERE search_fts MATCH ?{clauses}
                 ORDER BY {order}
                 LIMIT ? OFFSET ?"
            )
        } else {
            format!(
                "SELECT e.mailbox_id, e.uid
                 FROM envelopes e
                 WHERE 1 = 1{clauses}
                 ORDER BY e.date_epoch DESC, e.uid DESC
                 LIMIT ? OFFSET ?"
            )
        };
        let mut stmt = self.conn().prepare(&sql)?;
        let keys = stmt
            .query_map(params_from_iter(values), |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, Uid>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(keys)
    }

    /// Phase 2: hydrates ONLY these keys (never the skipped rows), then
    /// puts them back in phase 1's order — SQL's `IN` does not preserve
    /// it. `mailbox_id` is read back by NAME (`mbid`), so as not to
    /// couple to `SELECT_UNIFIED`'s column count.
    fn hydrate_in_order(&self, keys: &[(i64, Uid)]) -> Result<Vec<UnifiedRow>, Error> {
        if keys.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = vec!["(?,?)"; keys.len()].join(",");
        let sql = format!(
            "{SELECT_UNIFIED}, e.mailbox_id AS mbid
             FROM envelopes e
             JOIN mailboxes m ON m.id = e.mailbox_id
             JOIN accounts a ON a.id = m.account_id
             LEFT JOIN bodies b ON b.mailbox_id = e.mailbox_id AND b.uid = e.uid
             WHERE (e.mailbox_id, e.uid) IN (VALUES {placeholders})"
        );
        let mut values: Vec<Value> = Vec::with_capacity(keys.len() * 2);
        for (mailbox_id, uid) in keys {
            values.push((*mailbox_id).into());
            values.push(i64::from(*uid).into());
        }
        let mut stmt = self.conn().prepare(&sql)?;
        let mut by_key: HashMap<(i64, Uid), UnifiedRow> = stmt
            .query_map(params_from_iter(values), |row| {
                let mbid: i64 = row.get("mbid")?;
                let unified = row_to_unified(row)?;
                Ok(((mbid, unified.envelope.uid), unified))
            })?
            .collect::<Result<_, _>>()?;
        Ok(keys.iter().filter_map(|k| by_key.remove(k)).collect())
    }

    /// The EXACT number of matches for a search — no ranking or
    /// hydration, a plain COUNT on the index (same terms and filters as
    /// [`Store::search`]). Used to display "100 of N" and to decide
    /// [`Store::search_capped`]'s date-sort safety valve, which asks for
    /// it on every keystroke — for 1.5 ms on 200k (see above).
    pub fn search_total(&self, input: &str) -> Result<u64, Error> {
        let (match_expr, _has_terms, filters) = build_match(input);
        if match_expr.is_none() && filters.since.is_none() && filters.until.is_none() {
            return Ok(0);
        }
        let (clauses, date_values) = date_clauses(&filters);
        let mut values: Vec<Value> = Vec::new();
        if let Some(expr) = &match_expr {
            values.push(expr.clone().into());
        }
        values.extend(date_values);
        let inner = if match_expr.is_some() {
            if clauses.is_empty() {
                // Without a date bound, the COUNT needs no join: the
                // index alone carries the answer (the most expensive
                // path, the most frequent — this is where the cap bites).
                "SELECT rowid FROM search_fts WHERE search_fts MATCH ?".to_string()
            } else {
                // A date bound lives in `envelopes`: the join is needed.
                format!(
                    "SELECT search_fts.rowid
                     FROM search_fts
                     JOIN search_docs d ON d.docid = search_fts.rowid
                     JOIN envelopes e ON e.mailbox_id = d.mailbox_id AND e.uid = d.uid
                     WHERE search_fts MATCH ?{clauses}"
                )
            }
        } else {
            format!("SELECT e.rowid FROM envelopes e WHERE 1 = 1{clauses}")
        };
        let sql = format!("SELECT COUNT(*) FROM ({inner})");
        let total: i64 = self
            .conn()
            .query_row(&sql, params_from_iter(values), |row| row.get(0))?;
        Ok(total as u64)
    }
}

/// Translates the input into a joint MATCH expression — terms (last one as
/// a prefix) and `from:`/`to:` filters toward the `sender`/`recipients`
/// columns, which fold case AND accents (`unicode61 remove_diacritics`; a
/// SQL LIKE only folds ASCII and would miss "Étienne"). `has_terms` says
/// whether a BM25 ranking makes sense (otherwise, date sort). Shared by
/// `search` and `search_total`: the same query, counted then rendered.
fn build_match(input: &str) -> (Option<String>, bool, Filters) {
    let (terms_expr, filters) = parse_query(input);
    let from_expr = filters
        .from
        .as_ref()
        .map(|value| format!("sender:\"{value}\"*"));
    let to_expr = filters
        .to
        .as_ref()
        .map(|value| format!("recipients:\"{value}\"*"));
    let has_terms = terms_expr.is_some();
    let parts: Vec<String> = [terms_expr, from_expr, to_expr]
        .into_iter()
        .flatten()
        .collect();
    let match_expr = (!parts.is_empty()).then(|| parts.join(" "));
    (match_expr, has_terms, filters)
}

/// The date bounds as a SQL fragment and their values, in order. Shared
/// by `search` and `search_total`: the date filter must not diverge
/// between the count and the render.
fn date_clauses(filters: &Filters) -> (String, Vec<Value>) {
    let mut clauses = String::new();
    let mut values: Vec<Value> = Vec::new();
    if let Some(since) = filters.since {
        clauses.push_str(" AND e.date_epoch >= ?");
        values.push(since.into());
    }
    if let Some(until) = filters.until {
        clauses.push_str(" AND e.date_epoch < ?");
        values.push(until.into());
    }
    (clauses, values)
}

#[derive(Default)]
struct Filters {
    from: Option<String>,
    to: Option<String>,
    since: Option<i64>,
    until: Option<i64>,
}

/// Splits the input into terms and filters. Each term is neutralized
/// inside FTS5 quotes (the user's own quotes are stripped): the engine's
/// syntax is unreachable from the search field.
fn parse_query(input: &str) -> (Option<String>, Filters) {
    let mut terms: Vec<String> = Vec::new();
    let mut filters = Filters::default();
    for token in input.split_whitespace() {
        let lower = token.to_lowercase();
        if let Some(value) = lower
            .strip_prefix("from:")
            .or_else(|| lower.strip_prefix("de:"))
        {
            // Neutralized like a term: injected into the FTS syntax
            // (`sender:"…"*`), it must not be able to break it.
            let clean: String = value.chars().filter(|c| *c != '"').collect();
            if !clean.is_empty() {
                filters.from = Some(clean);
            }
        } else if let Some(value) = lower
            .strip_prefix("to:")
            .or_else(|| lower.strip_prefix("à:"))
        {
            // Symmetric to `from:`, toward the `recipients` column.
            let clean: String = value.chars().filter(|c| *c != '"').collect();
            if !clean.is_empty() {
                filters.to = Some(clean);
            }
        } else if let Some(value) = lower.strip_prefix("date:") {
            // An unreadable date filter is ignored rather than misapplied:
            // no surprise result.
            if let Some((since, until)) = parse_date_range(value) {
                filters.since = Some(since);
                filters.until = Some(until);
            }
        } else {
            let clean: String = token.chars().filter(|c| *c != '"').collect();
            if !clean.is_empty() {
                terms.push(clean);
            }
        }
    }
    let last = terms.len().saturating_sub(1);
    let match_expr = (!terms.is_empty()).then(|| {
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
    });
    (match_expr, filters)
}

/// `2026` → the year, `2026-07` → the month, `2026-07-18` → the day.
/// UTC bounds, half-open interval `[start, end)`.
fn parse_date_range(value: &str) -> Option<(i64, i64)> {
    let mut parts = value.splitn(3, '-');
    let year: i32 = parts
        .next()?
        .parse()
        .ok()
        .filter(|y| (1970..=9999).contains(y))?;
    let month: Option<u32> = match parts.next() {
        Some(m) => Some(m.parse().ok()?),
        None => None,
    };
    let day: Option<u32> = match parts.next() {
        Some(d) => Some(d.parse().ok()?),
        None => None,
    };
    let (start, end) = match (month, day) {
        (None, _) => (
            NaiveDate::from_ymd_opt(year, 1, 1)?,
            NaiveDate::from_ymd_opt(year + 1, 1, 1)?,
        ),
        (Some(m), None) => {
            let start = NaiveDate::from_ymd_opt(year, m, 1)?;
            let end = if m == 12 {
                NaiveDate::from_ymd_opt(year + 1, 1, 1)?
            } else {
                NaiveDate::from_ymd_opt(year, m + 1, 1)?
            };
            (start, end)
        }
        (Some(m), Some(d)) => {
            let start = NaiveDate::from_ymd_opt(year, m, d)?;
            (start, start.succ_opt()?)
        }
    };
    Some((
        start.and_hms_opt(0, 0, 0)?.and_utc().timestamp(),
        end.and_hms_opt(0, 0, 0)?.and_utc().timestamp(),
    ))
}

/// Reduces an HTML string to indexable words: tags and `<script>` /
/// `<style>` contents disappear, common entities (French accents
/// included) are decoded, whitespace collapses. Deliberately minimal:
/// the index needs words, not formatting — `mail-render` keeps the
/// faithful extraction for quoting.
fn indexable_text(html: &str) -> String {
    // ONE pass, ONE allocation (PLAN-AUDIT-V2 E2): the earlier shape made
    // five full-size copies of a body (lowercase shadow, tag-stripped
    // text, decoded entities, words, join) — ~140 MB allocated for 28 MB.
    // Here tags are recognized without a shadow (case-insensitive ASCII
    // comparison on the fly), entities decode while writing, whitespace
    // collapses while writing.
    let mut out = Output::new(html.len() / 2);
    let bytes = html.as_bytes();
    let mut i = 0;
    while let Some(open) = find(bytes, i, b'<') {
        text_into(&mut out, &html[i..open]);
        out.blank();
        let Some(close) = find(bytes, open, b'>') else {
            // Tag never closed: the rest is markup noise.
            i = html.len();
            break;
        };
        i = close + 1;
        let inner = &bytes[open + 1..close];
        let is_closing = inner.first() == Some(&b'/');
        let inner = if is_closing { &inner[1..] } else { inner };
        let name_end = inner
            .iter()
            .position(|c| !c.is_ascii_alphanumeric())
            .unwrap_or(inner.len());
        let name = &inner[..name_end];
        if !is_closing
            && (name.eq_ignore_ascii_case(b"script") || name.eq_ignore_ascii_case(b"style"))
        {
            i = skip_past_closing_tag(bytes, i, name);
        }
    }
    text_into(&mut out, &html[i..]);
    out.finish()
}

/// The output of `indexable_text`: whitespace collapses here as it is
/// written — never two in a row, never at the start, never at the end.
struct Output {
    text: String,
    pending_blank: bool,
}

impl Output {
    fn new(capacity: usize) -> Self {
        Self {
            text: String::with_capacity(capacity),
            pending_blank: false,
        }
    }

    fn blank(&mut self) {
        self.pending_blank = true;
    }

    fn char(&mut self, c: char) {
        if c.is_whitespace() {
            self.pending_blank = true;
            return;
        }
        if self.pending_blank && !self.text.is_empty() {
            self.text.push(' ');
        }
        self.pending_blank = false;
        self.text.push(c);
    }

    fn finish(self) -> String {
        self.text
    }
}

/// The first `target` byte from `from` onward — a `<` or `>` can only
/// appear at the start of a UTF-8 character, so the index is always a
/// character boundary.
fn find(bytes: &[u8], from: usize, target: u8) -> Option<usize> {
    bytes[from..]
        .iter()
        .position(|c| *c == target)
        .map(|p| from + p)
}

/// Writes `text` while decoding entities and collapsing whitespace.
fn text_into(out: &mut Output, text: &str) {
    let mut rest = text;
    while let Some(pos) = rest.find('&') {
        for c in rest[..pos].chars() {
            out.char(c);
        }
        rest = &rest[pos..];
        // A plausible entity fits in a few characters; beyond that, it's
        // a literal ampersand. We limit by CHARACTERS, not bytes, so as
        // not to cut a multi-byte character (e.g. 'è') in the middle.
        let semi = rest
            .char_indices()
            .take(12)
            .find(|(_, c)| *c == ';')
            .map(|(i, _)| i);
        match semi.and_then(|s| decode_entity(&rest[1..s]).map(|c| (c, s))) {
            Some((decoded, s)) => {
                out.char(decoded);
                rest = &rest[s + 1..];
            }
            None => {
                out.char('&');
                rest = &rest[1..];
            }
        }
    }
    for c in rest.chars() {
        out.char(c);
    }
}

/// Position right after `</name...>`, or the end if the closing tag is
/// missing.
fn skip_past_closing_tag(bytes: &[u8], from: usize, name: &[u8]) -> usize {
    let mut i = from;
    while i + 2 + name.len() <= bytes.len() {
        if bytes[i] == b'<'
            && bytes[i + 1] == b'/'
            && bytes[i + 2..i + 2 + name.len()].eq_ignore_ascii_case(name)
        {
            return match find(bytes, i, b'>') {
                Some(p) => p + 1,
                None => bytes.len(),
            };
        }
        i += 1;
    }
    bytes.len()
}

fn decode_entity(entity: &str) -> Option<char> {
    if let Some(num) = entity.strip_prefix('#') {
        let code = match num.strip_prefix('x').or_else(|| num.strip_prefix('X')) {
            Some(hex) => u32::from_str_radix(hex, 16).ok()?,
            None => num.parse().ok()?,
        };
        return char::from_u32(code);
    }
    Some(match entity {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => ' ',
        "eacute" => 'é',
        "egrave" => 'è',
        "ecirc" => 'ê',
        "euml" => 'ë',
        "agrave" => 'à',
        "acirc" => 'â',
        "ccedil" => 'ç',
        "icirc" => 'î',
        "iuml" => 'ï',
        "ocirc" => 'ô',
        "ouml" => 'ö',
        "ugrave" => 'ù',
        "ucirc" => 'û',
        "uuml" => 'ü',
        "oelig" => 'œ',
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;
    use crate::envelope::Envelope;

    fn envelope(uid: Uid, subject: &str, sender: &str, address: &str, epoch: i64) -> Envelope {
        Envelope {
            reply_to: None,
            uid,
            subject: Some(subject.to_string()),
            sender: Some(sender.to_string()),
            sender_address: Some(address.to_string()),
            message_id: None,
            in_reply_to: None,
            date: Some(Utc.timestamp_opt(epoch, 0).unwrap()),
            seen: false,
            flagged: false,
            to_addrs: Vec::new(),
            cc_addrs: Vec::new(),
        }
    }

    fn envelope_to(
        uid: Uid,
        subject: &str,
        sender: &str,
        address: &str,
        to: &[&str],
        cc: &[&str],
        epoch: i64,
    ) -> Envelope {
        Envelope {
            reply_to: None,
            uid,
            subject: Some(subject.to_string()),
            sender: Some(sender.to_string()),
            sender_address: Some(address.to_string()),
            message_id: None,
            in_reply_to: None,
            date: Some(Utc.timestamp_opt(epoch, 0).unwrap()),
            seen: false,
            flagged: false,
            to_addrs: to.iter().map(|s| s.to_string()).collect(),
            cc_addrs: cc.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// Downgrades the index to the old three-column schema (neither
    /// `recipients` nor `prefix=`): the exact state of a database
    /// installed before this job, the one the rebuild must catch up on.
    fn downgrade_old_schema(store: &Store) {
        store
            .conn()
            .execute_batch(
                "DROP TABLE search_fts;
                 DROP TABLE search_docs;
                 CREATE TABLE search_docs (
                    docid      INTEGER PRIMARY KEY,
                    mailbox_id INTEGER NOT NULL,
                    uid        INTEGER NOT NULL,
                    UNIQUE (mailbox_id, uid)
                 );
                 CREATE VIRTUAL TABLE search_fts USING fts5(
                    subject, sender, body,
                    content='', contentless_delete=1,
                    tokenize='unicode61 remove_diacritics 2'
                 );",
            )
            .unwrap();
    }

    fn store_with_inbox(email: &str) -> (Store, i64) {
        let store = Store::open_in_memory().unwrap();
        let account = store.adopt_or_create_account(email, "gmail").unwrap();
        let mailbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        (store, mailbox)
    }

    fn subjects(rows: &[UnifiedRow]) -> Vec<String> {
        rows.iter()
            .map(|r| r.envelope.subject.clone().unwrap_or_default())
            .collect()
    }

    fn indexed_count(store: &Store) -> i64 {
        store
            .conn()
            .query_row("SELECT COUNT(*) FROM search_docs", [], |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn finds_by_subject_across_accounts() {
        let (mut store, inbox_one) = store_with_inbox("one@example.com");
        let account_two = store
            .adopt_or_create_account("two@example.com", "gmail")
            .unwrap();
        let inbox_two = store.create_mailbox(account_two, "INBOX", 1).unwrap();
        store
            .upsert_envelopes(
                inbox_one,
                &[envelope(1, "Monthly report", "Alice", "alice@ex.fr", 100)],
            )
            .unwrap();
        store
            .upsert_envelopes(
                inbox_two,
                &[envelope(1, "Annual report", "Bob", "bob@ex.fr", 200)],
            )
            .unwrap();

        let rows = store.search("report", 50).unwrap();
        assert_eq!(rows.len(), 2);
        let emails: Vec<&str> = rows.iter().map(|r| r.account_email.as_str()).collect();
        assert!(emails.contains(&"one@example.com"));
        assert!(emails.contains(&"two@example.com"));
    }

    #[test]
    fn accents_fold_in_both_directions() {
        let (mut store, inbox) = store_with_inbox("test@example.com");
        store
            .upsert_envelopes(
                inbox,
                &[envelope(1, "Réunion budgétaire", "Alice", "a@ex.fr", 100)], // lang:fr
            )
            .unwrap();

        assert_eq!(store.search("reunion", 50).unwrap().len(), 1); // lang:fr
        assert_eq!(store.search("réunion", 50).unwrap().len(), 1); // lang:fr
        assert_eq!(store.search("budgetaire", 50).unwrap().len(), 1); // lang:fr
    }

    #[test]
    fn last_term_is_a_prefix_while_typing() {
        let (mut store, inbox) = store_with_inbox("test@example.com");
        store
            .upsert_envelopes(
                inbox,
                &[envelope(1, "Preliminary budget", "Alice", "a@ex.fr", 100)],
            )
            .unwrap();

        assert_eq!(store.search("prelim", 50).unwrap().len(), 1);
        assert_eq!(store.search("budget prelim", 50).unwrap().len(), 1);
        assert_eq!(
            store.search("prelim budget", 50).unwrap().len(),
            0,
            "only the last term is a prefix: the others are whole words"
        );
    }

    #[test]
    fn body_words_are_indexed_markup_is_not() {
        let (mut store, inbox) = store_with_inbox("test@example.com");
        store
            .upsert_envelopes(inbox, &[envelope(1, "No clue", "Alice", "a@ex.fr", 100)])
            .unwrap();
        assert_eq!(store.search("contract", 50).unwrap().len(), 0);

        store
            .save_body(
                inbox,
                1,
                "<div style=\"color:red\">the contract is signed</div>\
                 <style>.x{font-size:12px}</style>\
                 <script>var color = \"blue\";</script>",
                &[],
            )
            .unwrap();

        assert_eq!(store.search("contract", 50).unwrap().len(), 1);
        assert_eq!(
            store.search("signed", 50).unwrap().len(),
            1,
            "word from the body"
        );
        assert_eq!(
            store.search("color", 50).unwrap().len(),
            0,
            "attributes excluded"
        );
        assert_eq!(store.search("div", 50).unwrap().len(), 0, "tags excluded");
        assert_eq!(
            store.search("blue", 50).unwrap().len(),
            0,
            "scripts excluded"
        );
    }

    #[test]
    fn html_entities_decode_for_french_words() {
        let (mut store, inbox) = store_with_inbox("test@example.com");
        store
            .upsert_envelopes(inbox, &[envelope(1, "Invitation", "Alice", "a@ex.fr", 100)])
            .unwrap();
        store
            .save_body(
                inbox,
                1,
                "<p>r&eacute;union &amp; caf&eacute; &#233;quipe</p>", // lang:fr
                &[],
            )
            .unwrap();

        assert_eq!(store.search("reunion", 50).unwrap().len(), 1); // lang:fr
        assert_eq!(store.search("cafe", 50).unwrap().len(), 1); // lang:fr
        assert_eq!(store.search("equipe", 50).unwrap().len(), 1); // lang:fr
    }

    #[test]
    fn subject_hit_outranks_body_hit() {
        let (mut store, inbox) = store_with_inbox("test@example.com");
        store
            .upsert_envelopes(
                inbox,
                &[
                    envelope(1, "Miscellaneous", "Alice", "a@ex.fr", 300),
                    envelope(2, "Monthly invoice", "Alice", "a@ex.fr", 100),
                ],
            )
            .unwrap();
        store
            .save_body(inbox, 1, "<p>the invoice is attached</p>", &[])
            .unwrap();

        let rows = store.search("invoice", 50).unwrap();
        assert_eq!(
            subjects(&rows),
            vec!["Monthly invoice", "Miscellaneous"],
            "the subject weighs more than the body, despite an older date"
        );
    }

    #[test]
    fn reupsert_replaces_the_index_entry() {
        let (mut store, inbox) = store_with_inbox("test@example.com");
        store
            .upsert_envelopes(inbox, &[envelope(1, "before", "Alice", "a@ex.fr", 100)])
            .unwrap();
        store
            .upsert_envelopes(inbox, &[envelope(1, "after", "Alice", "a@ex.fr", 100)])
            .unwrap();

        assert_eq!(store.search("before", 50).unwrap().len(), 0);
        assert_eq!(store.search("after", 50).unwrap().len(), 1);
        assert_eq!(indexed_count(&store), 1, "a single index entry per message");
    }

    #[test]
    fn reupsert_keeps_the_indexed_body() {
        let (mut store, inbox) = store_with_inbox("test@example.com");
        store
            .upsert_envelopes(inbox, &[envelope(1, "Subject", "Alice", "a@ex.fr", 100)])
            .unwrap();
        store
            .save_body(inbox, 1, "<p>the contract</p>", &[])
            .unwrap();
        // A resync passes over the envelope again (the read flag, for example).
        store
            .upsert_envelopes(inbox, &[envelope(1, "Subject", "Alice", "a@ex.fr", 100)])
            .unwrap();

        assert_eq!(
            store.search("contract", 50).unwrap().len(),
            1,
            "the body already cached stays indexed after the envelope is rewritten"
        );
    }

    /// PLAN-AUDIT-V2 E2: a resync (read flag, CONDSTORE delta) used to
    /// pass EVERY envelope back through `index_message` — the body reread
    /// and re-tokenized under the write lock, for nothing. The index
    /// entry is stable (same docid) as long as the subject, sender and
    /// recipients have not changed.
    #[test]
    fn a_resynced_envelope_without_change_keeps_its_docid() {
        let (mut store, inbox) = store_with_inbox("test@example.com");
        let docid = |store: &Store| -> i64 {
            store
                .conn()
                .query_row(
                    "SELECT docid FROM search_docs WHERE mailbox_id = ?1 AND uid = 1",
                    [inbox],
                    |row| row.get(0),
                )
                .unwrap()
        };
        // Two messages: SQLite reuses the last deleted rowid, a single
        // message re-indexed would keep its number by accident. With a
        // second docid behind it, any re-indexing takes a fresh one.
        store
            .upsert_envelopes(
                inbox,
                &[
                    envelope(1, "Subject", "Alice", "a@ex.fr", 100),
                    envelope(2, "Second", "Bob", "b@ex.fr", 200),
                ],
            )
            .unwrap();
        let before = docid(&store);

        // Same envelope, flag changed: the sync passes over it again.
        let mut reread = envelope(1, "Subject", "Alice", "a@ex.fr", 100);
        reread.seen = true;
        store.upsert_envelopes(inbox, &[reread]).unwrap();
        assert_eq!(docid(&store), before, "re-indexed for no reason");

        // The subject changes: this time, the index must follow.
        store
            .upsert_envelopes(inbox, &[envelope(1, "Other", "Alice", "a@ex.fr", 100)])
            .unwrap();
        assert_ne!(docid(&store), before, "a changed subject re-indexes");
        assert_eq!(store.search("other", 50).unwrap().len(), 1);
    }

    #[test]
    fn local_removal_cleans_the_index() {
        let (mut store, inbox) = store_with_inbox("test@example.com");
        store
            .upsert_envelopes(inbox, &[envelope(1, "Ephemeral", "Alice", "a@ex.fr", 100)])
            .unwrap();
        store.remove_local(inbox, 1).unwrap();

        assert_eq!(store.search("ephemeral", 50).unwrap().len(), 0);
        assert_eq!(indexed_count(&store), 0);
    }

    #[test]
    fn absent_removal_cleans_the_index() {
        let (mut store, inbox) = store_with_inbox("test@example.com");
        store
            .upsert_envelopes(
                inbox,
                &[
                    envelope(1, "Gone from the server", "Alice", "a@ex.fr", 100),
                    envelope(2, "Still there", "Alice", "a@ex.fr", 200),
                ],
            )
            .unwrap();
        store
            .remove_absent(inbox, &std::collections::HashSet::from([2]))
            .unwrap();

        assert_eq!(store.search("gone", 50).unwrap().len(), 0);
        assert_eq!(store.search("still", 50).unwrap().len(), 1);
    }

    #[test]
    fn uidvalidity_reset_clears_only_that_mailbox() {
        let (mut store, inbox_one) = store_with_inbox("one@example.com");
        let account_two = store
            .adopt_or_create_account("two@example.com", "gmail")
            .unwrap();
        let inbox_two = store.create_mailbox(account_two, "INBOX", 1).unwrap();
        store
            .upsert_envelopes(inbox_one, &[envelope(1, "Report one", "A", "a@ex.fr", 100)])
            .unwrap();
        store
            .upsert_envelopes(inbox_two, &[envelope(1, "Report two", "B", "b@ex.fr", 200)])
            .unwrap();

        store.reset_mailbox(inbox_one, 2).unwrap();

        assert_eq!(
            subjects(&store.search("report", 50).unwrap()),
            vec!["Report two"]
        );
        assert_eq!(indexed_count(&store), 1);
    }

    #[test]
    fn hostile_input_is_literal_never_fts_syntax() {
        let (mut store, inbox) = store_with_inbox("test@example.com");
        store
            .upsert_envelopes(
                inbox,
                &[envelope(
                    1,
                    "budget (Q3) \"special\"",
                    "Alice",
                    "a@ex.fr",
                    100,
                )],
            )
            .unwrap();

        for hostile in [
            "budget AND",
            "AND",
            "OR NOT",
            "(",
            ")",
            "*",
            "\"",
            "\" OR \"",
            "NEAR(",
            "bud*get",
            "subject:x",
            "-budget",
        ] {
            assert!(
                store.search(hostile, 50).is_ok(),
                "input \"{hostile}\" must never be FTS5 syntax"
            );
        }
        // Operators are words: "AND" alone matches nothing here.
        assert_eq!(store.search("AND", 50).unwrap().len(), 0);
    }

    #[test]
    fn from_filter_narrows_by_name_or_address() {
        let (mut store, inbox) = store_with_inbox("test@example.com");
        store
            .upsert_envelopes(
                inbox,
                &[
                    envelope(1, "Sales report", "Alice Martin", "alice@ex.fr", 100),
                    envelope(2, "Purchases report", "Bob Durand", "bob@ex.fr", 200),
                ],
            )
            .unwrap();

        assert_eq!(
            subjects(&store.search("report from:alice", 50).unwrap()),
            vec!["Sales report"]
        );
        assert_eq!(
            subjects(&store.search("report de:durand", 50).unwrap()),
            vec!["Purchases report"],
            "the filter matches the display name as well as the address"
        );
    }

    /// Regression (bug #3): the from: filter only folded ASCII (SQL LIKE)
    /// and missed a name with an accented capital. It now goes through
    /// the `sender` column of the FTS index, which folds case AND
    /// accents (`unicode61 remove_diacritics`).
    #[test]
    fn from_filter_folds_case_and_accents() {
        let (mut store, inbox) = store_with_inbox("test@example.com");
        store
            .upsert_envelopes(
                inbox,
                &[envelope(
                    1,
                    "Report",
                    "Étienne Bernard", // lang:fr
                    "e.bernard@ex.fr",
                    100,
                )],
            )
            .unwrap();

        // The "É" capital must be found, with or without the accent typed. // lang:fr
        assert_eq!(store.search("from:etienne", 50).unwrap().len(), 1); // lang:fr
        assert_eq!(store.search("from:étienne", 50).unwrap().len(), 1); // lang:fr
        assert_eq!(store.search("from:ETIENNE", 50).unwrap().len(), 1); // lang:fr
        assert_eq!(store.search("report from:etienne", 50).unwrap().len(), 1); // lang:fr
        // A sender that does not match stays excluded.
        assert_eq!(store.search("from:durand", 50).unwrap().len(), 0);
    }

    #[test]
    fn to_filter_narrows_by_recipient_name_or_address() {
        let (mut store, inbox) = store_with_inbox("me@example.com");
        store
            .upsert_envelopes(
                inbox,
                &[
                    envelope_to(
                        1,
                        "Minutes",
                        "Me",
                        "me@ex.fr",
                        &["Alice Martin <alice@ex.fr>"],
                        &[],
                        100,
                    ),
                    envelope_to(
                        2,
                        "Quote",
                        "Me",
                        "me@ex.fr",
                        &["Bob Durand <bob@ex.fr>"],
                        &[],
                        200,
                    ),
                ],
            )
            .unwrap();

        assert_eq!(
            subjects(&store.search("to:alice", 50).unwrap()),
            vec!["Minutes"]
        );
        assert_eq!(
            subjects(&store.search("à:durand", 50).unwrap()), // lang:fr
            vec!["Quote"],
            "the recipient filter also answers to \"à:\""
        );
        assert_eq!(
            subjects(&store.search("to:bob@ex.fr", 50).unwrap()),
            vec!["Quote"],
            "the filter matches the address as well as the display name"
        );
    }

    #[test]
    fn cc_recipients_fold_case_and_accents() {
        let (mut store, inbox) = store_with_inbox("me@example.com");
        store
            .upsert_envelopes(
                inbox,
                &[envelope_to(
                    1,
                    "Meeting",
                    "Me",
                    "me@ex.fr",
                    &["alice@ex.fr"],
                    &["Étienne Bernard <e.bernard@ex.fr>"], // lang:fr
                    100,
                )],
            )
            .unwrap();

        // An address in copy is a recipient; "É" folds. // lang:fr
        assert_eq!(store.search("to:etienne", 50).unwrap().len(), 1); // lang:fr
        assert_eq!(store.search("to:étienne", 50).unwrap().len(), 1); // lang:fr
    }

    #[test]
    fn bare_term_finds_a_message_by_its_recipient() {
        // "alice" finds a mail I sent HER, not only the ones received from
        // her: the recipient is a searchable field.
        let (mut store, inbox) = store_with_inbox("me@example.com");
        store
            .upsert_envelopes(
                inbox,
                &[envelope_to(
                    1,
                    "Without her name elsewhere",
                    "Me",
                    "me@ex.fr",
                    &["Alice Martin <alice@ex.fr>"],
                    &[],
                    100,
                )],
            )
            .unwrap();

        assert_eq!(store.search("alice", 50).unwrap().len(), 1);
    }

    #[test]
    fn to_filter_alone_lists_by_date() {
        let (mut store, inbox) = store_with_inbox("me@example.com");
        store
            .upsert_envelopes(
                inbox,
                &[
                    envelope_to(1, "First", "Me", "me@ex.fr", &["alice@ex.fr"], &[], 100),
                    envelope_to(2, "To Bob", "Me", "me@ex.fr", &["bob@ex.fr"], &[], 200),
                    envelope_to(3, "Second", "Me", "me@ex.fr", &["alice@ex.fr"], &[], 300),
                ],
            )
            .unwrap();

        assert_eq!(
            subjects(&store.search("to:alice", 50).unwrap()),
            vec!["Second", "First"],
            "a recipient filter alone lists by date, most recent first"
        );
    }

    #[test]
    fn date_filter_bounds_by_year_month_or_day() {
        let (mut store, inbox) = store_with_inbox("test@example.com");
        let in_2025 = Utc
            .with_ymd_and_hms(2025, 6, 15, 12, 0, 0)
            .unwrap()
            .timestamp();
        let in_2026 = Utc
            .with_ymd_and_hms(2026, 7, 1, 9, 0, 0)
            .unwrap()
            .timestamp();
        store
            .upsert_envelopes(
                inbox,
                &[
                    envelope(1, "Old report", "Alice", "a@ex.fr", in_2025),
                    envelope(2, "Recent report", "Alice", "a@ex.fr", in_2026),
                ],
            )
            .unwrap();

        assert_eq!(
            subjects(&store.search("report date:2026", 50).unwrap()),
            vec!["Recent report"]
        );
        assert_eq!(
            subjects(&store.search("report date:2026-07", 50).unwrap()),
            vec!["Recent report"]
        );
        assert_eq!(
            subjects(&store.search("report date:2025-06-15", 50).unwrap()),
            vec!["Old report"]
        );
        assert_eq!(store.search("report date:2024", 50).unwrap().len(), 0);
        assert_eq!(
            store.search("report date:whatever", 50).unwrap().len(),
            2,
            "an unreadable date filter is ignored, not misapplied"
        );
    }

    #[test]
    fn filter_alone_lists_by_date_without_terms() {
        let (mut store, inbox) = store_with_inbox("test@example.com");
        store
            .upsert_envelopes(
                inbox,
                &[
                    envelope(1, "First from Alice", "Alice", "alice@ex.fr", 100),
                    envelope(2, "From Bob", "Bob", "bob@ex.fr", 200),
                    envelope(3, "Second from Alice", "Alice", "alice@ex.fr", 300),
                ],
            )
            .unwrap();

        assert_eq!(
            subjects(&store.search("from:alice", 50).unwrap()),
            vec!["Second from Alice", "First from Alice"],
            "a filter alone lists by date, most recent first"
        );
    }

    /// Migration: a database from before this job carries a `search_fts`
    /// with three columns (neither `recipients` nor `prefix=`). On
    /// opening, `migrate_search` rebuilds it from the messages — without
    /// resyncing — and recipients become searchable.
    #[test]
    fn migration_rebuilds_an_old_index_with_recipients() {
        let (mut store, inbox) = store_with_inbox("me@example.com");
        store
            .upsert_envelopes(
                inbox,
                &[envelope_to(
                    1,
                    "Quote",
                    "Me",
                    "me@ex.fr",
                    &["Alice Martin <alice@ex.fr>"],
                    &[],
                    100,
                )],
            )
            .unwrap();
        assert_eq!(store.search("to:alice", 50).unwrap().len(), 1);

        downgrade_old_schema(&store);

        // The rebuild is triggered by detecting the old schema (no
        // `recipients` column).
        migrate_search(store.conn(), &mut |_| ControlFlow::Continue(())).unwrap();

        assert_eq!(
            store.search("quote", 50).unwrap().len(),
            1,
            "the message stays searchable after the rebuild"
        );
        assert_eq!(
            store.search("to:alice", 50).unwrap().len(),
            1,
            "the rebuild re-indexes the recipients from the envelopes"
        );
        assert_eq!(indexed_count(&store), 1, "a single index entry");

        // The performance lever (D3) must survive the rebuild: without
        // this guard, a removal of `prefix='2 3'` would pass unnoticed.
        let schema: String = store
            .conn()
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'search_fts'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(
            schema.contains("prefix='2 3'"),
            "the rebuilt schema keeps the prefix='2 3' option"
        );
    }

    #[test]
    fn migration_reports_progress_and_indexes_every_message() {
        let (mut store, inbox) = store_with_inbox("me@example.com");
        // Several messages, with and without a body: the stream (no
        // collecting all bodies in memory) must re-index all of them.
        let envelopes: Vec<Envelope> = (1..=5u32)
            .map(|uid| {
                envelope_to(
                    uid,
                    &format!("Subject {uid}"),
                    "Me",
                    "me@ex.fr",
                    &[&format!("dest{uid}@ex.fr")],
                    &[],
                    uid as i64 * 100,
                )
            })
            .collect();
        store.upsert_envelopes(inbox, &envelopes).unwrap();
        store
            .save_body(inbox, 3, "<p>the special contract</p>", &[])
            .unwrap();

        downgrade_old_schema(&store);

        let mut reports = Vec::new();
        migrate_search(store.conn(), &mut |p| {
            reports.push((p.done, p.total));
            ControlFlow::Continue(())
        })
        .unwrap();

        // "Done" announced on the exact total (5 envelopes).
        assert_eq!(reports.last(), Some(&(5, 5)));
        // Every message re-indexed, recipient and body included: the
        // proof that the stream skips none of them.
        assert_eq!(store.search("to:dest4", 50).unwrap().len(), 1);
        assert_eq!(store.search("contract", 50).unwrap().len(), 1);
        assert_eq!(indexed_count(&store), 5);
    }

    #[test]
    fn migration_cancel_rolls_back_and_reruns() {
        let (mut store, inbox) = store_with_inbox("me@example.com");
        store
            .upsert_envelopes(
                inbox,
                &[envelope_to(
                    1,
                    "Quote",
                    "Me",
                    "me@ex.fr",
                    &["alice@ex.fr"],
                    &[],
                    100,
                )],
            )
            .unwrap();

        downgrade_old_schema(&store);

        // The user cancels: Break → Interrupted.
        let result = migrate_search(store.conn(), &mut |_| ControlFlow::Break(()));
        assert!(matches!(result, Err(Error::Interrupted)));

        // Rewound: the schema stayed the old one (no `recipients`), so
        // the pass will replay on the next launch instead of leaving a
        // half-done index.
        let schema: String = store
            .conn()
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'search_fts'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(
            !schema.contains("recipients"),
            "the cancellation rewound the DROP/CREATE — nothing partial persisted"
        );

        // Replayed without cancellation: it succeeds.
        migrate_search(store.conn(), &mut |_| ControlFlow::Continue(())).unwrap();
        assert_eq!(store.search("to:alice", 50).unwrap().len(), 1);
    }

    #[test]
    fn search_total_counts_all_matches_beyond_the_limit() {
        let (mut store, inbox) = store_with_inbox("test@example.com");
        let envelopes: Vec<Envelope> = (1..=10)
            .map(|uid| envelope(uid, "Report", "Alice", "a@ex.fr", uid as i64))
            .collect();
        store.upsert_envelopes(inbox, &envelopes).unwrap();

        // `search` caps at 3; `search_total` sees all 10.
        assert_eq!(store.search("report", 3).unwrap().len(), 3);
        assert_eq!(store.search_total("report").unwrap(), 10);

        // Same filters as the search.
        assert_eq!(store.search_total("report from:alice").unwrap(), 10);
        assert_eq!(store.search_total("report from:bob").unwrap(), 0);

        // Empty query: nothing to count.
        assert_eq!(store.search_total("").unwrap(), 0);
        assert_eq!(store.search_total("   ").unwrap(), 0);
    }

    #[test]
    fn search_total_respects_the_date_filter() {
        let (mut store, inbox) = store_with_inbox("test@example.com");
        let in_2025 = Utc
            .with_ymd_and_hms(2025, 6, 15, 12, 0, 0)
            .unwrap()
            .timestamp();
        let in_2026 = Utc
            .with_ymd_and_hms(2026, 7, 1, 9, 0, 0)
            .unwrap()
            .timestamp();
        store
            .upsert_envelopes(
                inbox,
                &[
                    envelope(1, "Old report", "Alice", "a@ex.fr", in_2025),
                    envelope(2, "Recent report", "Alice", "a@ex.fr", in_2026),
                ],
            )
            .unwrap();

        assert_eq!(store.search_total("report").unwrap(), 2);
        assert_eq!(
            store.search_total("report date:2026").unwrap(),
            1,
            "the total follows the date filter (the joined path)"
        );
        // Date filter ALONE (no term): counted by the envelopes branch.
        assert_eq!(store.search_total("date:2025").unwrap(), 1);
    }

    #[test]
    fn search_recent_orders_by_date_ignoring_relevance() {
        let (mut store, inbox) = store_with_inbox("test@example.com");
        store
            .upsert_envelopes(
                inbox,
                &[
                    // The term in the SUBJECT, but OLD.
                    envelope(1, "Monthly invoice", "Alice", "a@ex.fr", 100),
                    // The term in the BODY only, but RECENT.
                    envelope(2, "Miscellaneous", "Alice", "a@ex.fr", 300),
                ],
            )
            .unwrap();
        store
            .save_body(inbox, 2, "<p>the invoice is attached</p>", &[])
            .unwrap();

        // BM25: the subject weighs more → "Monthly invoice" first.
        assert_eq!(
            subjects(&store.search("invoice", 50).unwrap()),
            vec!["Monthly invoice", "Miscellaneous"]
        );
        // Date: most recent first, relevance ignored → "Miscellaneous".
        assert_eq!(
            subjects(&store.search_recent("invoice", 50).unwrap()),
            vec!["Miscellaneous", "Monthly invoice"],
            "the safety valve for broad queries sorts by date, not by BM25"
        );
    }

    #[test]
    fn search_capped_returns_rows_and_exact_total() {
        let (mut store, inbox) = store_with_inbox("test@example.com");
        let envelopes: Vec<Envelope> = (1..=10)
            .map(|uid| envelope(uid, "Report", "Alice", "a@ex.fr", uid as i64))
            .collect();
        store.upsert_envelopes(inbox, &envelopes).unwrap();

        let (rows, total) = store.search_capped("report", 3, 0).unwrap();
        assert_eq!(rows.len(), 3, "rendered capped at the limit");
        assert_eq!(total, 10, "exact total, beyond the cap");
        // 10 matches, well under WIDE_QUERY_THRESHOLD: BM25 ranking
        // kept (the subject wins, proven by the dedicated tests).
    }

    #[test]
    fn search_capped_pages_without_gap_or_overlap() {
        let (mut store, inbox) = store_with_inbox("test@example.com");
        // Ten messages, sorted by date (increasing uid = increasing epoch):
        // the search renders them from most recent (uid 10) to oldest (uid 1).
        let envelopes: Vec<Envelope> = (1..=10)
            .map(|uid| envelope(uid, "Report", "Alice", "a@ex.fr", uid as i64))
            .collect();
        store.upsert_envelopes(inbox, &envelopes).unwrap();

        // Three pages of 4: [10..7], [6..3], [2..1]. Total constant at 10.
        let uids = |rows: &[UnifiedRow]| rows.iter().map(|r| r.envelope.uid).collect::<Vec<_>>();
        let (p1, total) = store.search_capped("report", 4, 0).unwrap();
        let (p2, _) = store.search_capped("report", 4, 4).unwrap();
        let (p3, _) = store.search_capped("report", 4, 8).unwrap();

        assert_eq!(total, 10);
        assert_eq!(uids(&p1), vec![10, 9, 8, 7]);
        assert_eq!(
            uids(&p2),
            vec![6, 5, 4, 3],
            "page 2 chains without gap or duplicate"
        );
        assert_eq!(uids(&p3), vec![2, 1], "the last page renders the rest");
        // Beyond the total: nothing left.
        assert!(store.search_capped("report", 4, 12).unwrap().0.is_empty());
    }

    #[test]
    fn blank_query_returns_nothing() {
        let (mut store, inbox) = store_with_inbox("test@example.com");
        store
            .upsert_envelopes(inbox, &[envelope(1, "Subject", "Alice", "a@ex.fr", 100)])
            .unwrap();

        assert!(store.search("", 50).unwrap().is_empty());
        assert!(store.search("   ", 50).unwrap().is_empty());
        assert!(store.search("\"\"", 50).unwrap().is_empty());
    }

    #[test]
    fn limit_caps_the_result_set() {
        let (mut store, inbox) = store_with_inbox("test@example.com");
        let envelopes: Vec<Envelope> = (1..=10)
            .map(|uid| envelope(uid, "Report", "Alice", "a@ex.fr", uid as i64))
            .collect();
        store.upsert_envelopes(inbox, &envelopes).unwrap();

        assert_eq!(store.search("report", 3).unwrap().len(), 3);
    }

    #[test]
    fn strips_markup_and_collapses_whitespace() {
        assert_eq!(
            indexable_text("<p>one\n  <b>two</b></p>   three"),
            "one two three"
        );
        assert_eq!(
            indexable_text("before <img src=x"),
            "before",
            "tag never closed"
        );
        assert_eq!(
            indexable_text("caf&eacute; &amp; th&eacute; &inconnu; &#x41;"), // lang:fr
            "café & thé &inconnu; A"                                         // lang:fr
        );
    }

    /// Regression: an ampersand followed by a multi-byte character right
    /// after the 12-character window must not cut the character in the
    /// middle.
    #[test]
    fn ampersand_before_multibyte_char_does_not_panic() {
        assert_eq!(
            indexable_text("&quot; (modèle avec médecins)"), // lang:fr
            "\" (modèle avec médecins)"                      // lang:fr
        );
        // Ampersand not followed by an entity, with a multi-byte character
        // that overflows the 12-character limit.
        assert_eq!(
            indexable_text("modèle & clinique de médecins"), // lang:fr
            "modèle & clinique de médecins"                  // lang:fr
        );
    }
}

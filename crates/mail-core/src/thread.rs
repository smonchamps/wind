//! Grouping messages into conversations — the algorithm, pure.
//!
//! This module knows neither SQLite nor the network. It answers a single
//! question: “which thread does this message belong to?”, from the
//! RFC 5322 identifiers it carries and from what is already known.
//!
//! The grouping is a **union-find**: every `Message-ID` encountered — the
//! message's own, AND those of its ancestors, *even absent from the
//! mailbox* — is entered into a directory that points to a thread. A
//! message citing two identifiers attached to two different threads
//! **merges** them.
//!
//! This merge is not an exotic case, it is what makes the grouping
//! **convergent**. Two reasons why a thread regularly comes to life in
//! pieces:
//!
//! - messages do not arrive in order (a reply can be synced before the
//!   message it cites);
//! - headers do not arrive together — `In-Reply-To` comes from the
//!   ENVELOPE, for free, while `References` needs a separate pass over
//!   the full headers.
//!
//! The pieces knit back together as soon as the missing link appears,
//! without any acquired information being lost along the way. That is
//! the property that allows delivering the acquisition in two stages.

use std::collections::{BTreeSet, HashMap};
use std::ops::ControlFlow;

use rusqlite::{Connection, OptionalExtension, params};

use crate::envelope::Uid;
use crate::error::Error;
use crate::store::AdoptionProgress;

/// Internal identifier of a thread.
///
/// An integer, not the root's `Message-ID`: the root can arrive after its
/// replies, or never arrive at all (it is in “Sent”, or it was deleted).
/// A thread must not be able to change identity along the way.
pub(crate) type ThreadId = i64;

/// Maximum number of ancestors kept in `References`.
///
/// The header is cumulative: a long discussion, or a faulty piece of
/// software, can pile up thousands. We keep the two ends — the root,
/// which attaches the whole thread, and the immediate ancestors, which
/// attach the neighborhood. The middle is redundant: those messages, if
/// they are in the mailbox, carry their own links.
const MAX_REFERENCES: usize = 32;

/// Share of the limit reserved for the start of `References` (the root).
const KEPT_AT_ROOT: usize = 8;

/// Splits an identifier header (`Message-ID`, `In-Reply-To`,
/// `References`) into canonical identifiers.
///
/// Canonical form = the content of the angle brackets, without them.
/// RFC 5322 makes them mandatory; real life omits them. Comparing the two
/// forms without normalizing them would make two threads where there is
/// only one.
fn canonical_ids(raw: &str) -> Vec<String> {
    let mut ids = Vec::new();
    let mut bracketed = false;
    let mut rest = raw;
    while let Some(open) = rest.find('<') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('>') else { break };
        bracketed = true;
        let id = after[..close].trim();
        if is_message_id(id) {
            ids.push(id.to_string());
        }
        rest = &after[close + 1..];
    }
    if !bracketed {
        // No angle brackets: out of spec, but common enough that ignoring
        // these messages would amount to not grouping them at all.
        //
        // The fallback is decided on the PRESENCE of angle brackets,
        // never on whether something was extracted from them: a
        // `Message-ID: <>` — a faulty piece of software produces one —
        // would otherwise fall through to here.
        ids = raw
            .split_whitespace()
            .filter(|token| is_message_id(token))
            .map(str::to_string)
            .collect();
    }
    ids
}

/// Is this token a plausible `Message-ID`?
///
/// RFC 5322 §3.6.4: `msg-id = "<" id-left "@" id-right ">"`. **The at sign
/// is mandatory**, and it is what separates an identifier from a word.
///
/// This is not purism, it is the guard rail that was missing. Without
/// it, a header written in prose — the RFC 822 form
/// `In-Reply-To: Your message of January 3rd`, which some autoresponders
/// still produce — manufactures as many fake identifiers as words. Every
/// word becomes an anchor, every message carrying the same phrase latches
/// onto it, and the union-find *correctly* reunites them into a thread
/// that makes no sense.
///
/// Measured on a real mailbox before the fix: **43 unrelated messages in
/// a single conversation**, latched onto 3-to-11-character tokens without
/// an at sign that nobody actually carried.
///
/// Accepted consequence: an out-of-spec `Message-ID` (`<1234567890>`) is
/// ignored. The message then forms its own thread and the replies it
/// receives do not attach to it. That is a local, silent loss, against a
/// massive and visible merge — the trade is very favorable.
fn is_message_id(token: &str) -> bool {
    token.contains('@') && !token.chars().any(char::is_whitespace)
}

/// All the identifiers that attach a message to its thread: its own
/// first, then its ancestors, from oldest to nearest.
pub(crate) fn linking_ids(
    message_id: Option<&str>,
    in_reply_to: Option<&str>,
    references: Option<&str>,
    addresses: &[String],
) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    let mut push = |candidates: Vec<String>| {
        for id in candidates {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
    };
    push(message_id.map(canonical_ids).unwrap_or_default());
    push(cap_references(
        references.map(canonical_ids).unwrap_or_default(),
    ));
    push(in_reply_to.map(canonical_ids).unwrap_or_default());
    // An envelope ADDRESS between angle brackets (`In-Reply-To:
    // <alice@x.fr>`, some autoresponders do this) has the shape of an
    // identifier — it is not one (PLAN-AUDIT-V2 E5: it was merging
    // unrelated threads, ADR 0008 to the letter but not in spirit).
    ids.retain(|id| {
        !addresses
            .iter()
            .any(|address| address.trim().eq_ignore_ascii_case(id))
    });
    ids
}

/// Applies [`MAX_REFERENCES`] while keeping both ends.
fn cap_references(mut refs: Vec<String>) -> Vec<String> {
    if refs.len() <= MAX_REFERENCES {
        return refs;
    }
    let tail = refs.split_off(refs.len() - (MAX_REFERENCES - KEPT_AT_ROOT));
    refs.truncate(KEPT_AT_ROOT);
    refs.extend(tail);
    refs
}

/// What must be done to attach a message, once the directory has been
/// consulted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ThreadPlan {
    /// The host thread. `None`: no identifier known, a new thread must be
    /// created.
    pub keep: Option<ThreadId>,
    /// The threads that `keep` absorbs. Empty outside of a merge.
    ///
    /// Repointing their identifiers to `keep` is the caller's job: it is
    /// a write, and this module performs none.
    pub absorb: Vec<ThreadId>,
    /// The identifiers still absent from the directory, to be entered
    /// into it.
    ///
    /// Including those of ancestors **absent from the mailbox**: that is
    /// precisely what lets a message arriving later join the right
    /// thread.
    pub register: Vec<String>,
}

/// Consults the directory and decides on the attachment.
///
/// `known` only needs to contain the identifiers from `ids` — it is the
/// caller's job to make the single query that looks them up.
pub(crate) fn plan(ids: &[String], known: &HashMap<String, ThreadId>) -> ThreadPlan {
    let mut threads: Vec<ThreadId> = Vec::new();
    let mut register: Vec<String> = Vec::new();
    for id in ids {
        match known.get(id) {
            Some(thread) => {
                if !threads.contains(thread) {
                    threads.push(*thread);
                }
            }
            None => {
                if !register.contains(id) {
                    register.push(id.clone());
                }
            }
        }
    }
    // The oldest thread wins — its identifier is the smallest. This
    // tie-break must be the SAME regardless of the order in which
    // messages arrive: otherwise two syncs of the same mailbox would not
    // yield the same grouping, and the thread would “jump” before the
    // user's eyes.
    threads.sort_unstable();
    let mut threads = threads.into_iter();
    ThreadPlan {
        keep: threads.next(),
        absorb: threads.collect(),
        register,
    }
}

// ---------------------------------------------------------------------------
// Persistence — the algorithm above, applied to the database.
//
// All these functions take a `&Connection` and are called WITHIN the
// transaction that writes the message, like the search index (ADR 0004):
// a half-attached thread would be worse than an unattached message.
// ---------------------------------------------------------------------------

/// The two thread tables, with clearly distinct roles.
///
/// `threads` is a **materialized aggregate**: the list must be able to
/// display a page of conversations without aggregating 200,000 envelopes
/// on every scroll. Same reasoning as the search index — the aggregate
/// lives in the database and is maintained within the same transaction
/// as the message.
///
/// `thread_links` is the **directory**: it also retains the identifiers
/// of ancestors that the mailbox does not contain. That memory is what
/// lets two halves of a thread recognize each other later.
/// The mailbox whose messages are “received”.
///
/// A hardcoded name, and that is deliberate: `inbox_size` serves the list
/// filter, which shows only one mailbox — the one for incoming mail. The
/// day the list would show several, this counter would lose its meaning
/// before losing its value, and it would need rethinking rather than
/// parameterizing.
pub(crate) const RECEIVED_MAILBOX: &str = "INBOX";

pub(crate) const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS threads (
    id         INTEGER PRIMARY KEY,
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    -- The last message can live in INBOX just as well as in “Sent”:
    -- its UID alone identifies nothing (invariant “identity = account+UID”).
    last_mailbox_id INTEGER,
    last_uid   INTEGER NOT NULL DEFAULT 0,
    last_epoch INTEGER,
    size       INTEGER NOT NULL DEFAULT 0,
    unseen     INTEGER NOT NULL DEFAULT 0,
    -- How many messages were RECEIVED. A purely outgoing thread — I write,
    -- nobody replies — is worth 0 and has no row in the list
    -- (ADR 0009 §2).
    inbox_size INTEGER NOT NULL DEFAULT 0,
    -- Is the thread OUTSIDE the ORGANIZED Inbox (Organized mode E2):
    -- it carries a message from a sender routed ELSEWHERE (it lives in
    -- its own view — mirror of thread_route_sql), or ALL its messages
    -- come from unknowns pending at the Screener (a mixed thread STAYS —
    -- golden rule). Maintained by `refresh`, like size/unseen — verdict
    -- S2-bis: any form computed at query time collapses at deep offset
    -- (299 ms), the flag + partial index matches the control.
    organise_hors INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_threads_date
    ON threads(account_id, last_epoch DESC, last_uid DESC);
-- The same sort, WITHOUT a mailbox prefix: this is what the unified
-- mailbox needs. It covers the same mailbox across ALL accounts, so it
-- fixes no `mailbox_id` — and an index starting with that column can no
-- longer carry the ordering. SQLite fell back to a materialized sort of
-- all conversations, on EVERY scroll page: 987 ms measured on 160,000
-- conversations at gate 3, against 0.66 ms with this index. The prefixed
-- index remains useful for queries scoped to one mailbox.
-- PARTIAL: the “at least one received message” filter enters the index
-- INSTEAD of being evaluated after it. Without the WHERE clause, SQLite
-- would scan then discard every purely outgoing thread, and the
-- materialized sort that gate 3 just removed would come back through
-- another door (ADR 0009 §4).
CREATE INDEX IF NOT EXISTS idx_threads_date_globale
    ON threads(last_epoch DESC, last_uid DESC, account_id)
    WHERE inbox_size > 0;
-- The MIRROR of the previous one for the ORGANIZED Inbox: the retention
-- filter enters the index (S2-bis) and, since E4, the key carries the
-- SECTIONS — unread first (verdict S1/A2: without this expression index,
-- the section sort materializes the whole mailbox, 548 ms per page; with
-- it, the control's profile). The expression is for ORDERING only, never
-- for a join (E2 trap). On a legacy database, the column already exists:
-- `migrate()` adds it BEFORE this schema, and REBUILDS the E2 index whose
-- key lacked the sections.
CREATE INDEX IF NOT EXISTS idx_threads_date_organise
    ON threads((unseen > 0) DESC, last_epoch DESC, last_uid DESC, account_id)
    WHERE inbox_size > 0 AND organise_hors = 0;
-- The same key PREFIXED by account — the “Mailboxes” nav of the
-- Organized Inbox (the pattern of idx_threads_date for the classic one).
CREATE INDEX IF NOT EXISTS idx_threads_date_organise_compte
    ON threads(account_id, (unseen > 0) DESC, last_epoch DESC, last_uid DESC)
    WHERE inbox_size > 0 AND organise_hors = 0;
CREATE TABLE IF NOT EXISTS thread_links (
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    message_id TEXT NOT NULL,
    thread_id  INTEGER NOT NULL REFERENCES threads(id) ON DELETE CASCADE,
    PRIMARY KEY (account_id, message_id)
);
CREATE INDEX IF NOT EXISTS idx_thread_links_thread ON thread_links(thread_id);
";

/// Attaches a message to its thread and returns it.
///
/// Does NOT write `envelopes.thread_id`: the caller does that, because
/// only it knows whether the envelope is already written. It must then
/// call [`refresh`] on the returned thread.
pub(crate) fn attach(
    conn: &Connection,
    account_id: i64,
    message_id: Option<&str>,
    in_reply_to: Option<&str>,
    references: Option<&str>,
    addresses: &[String],
) -> Result<ThreadId, Error> {
    let ids = linking_ids(message_id, in_reply_to, references, addresses);
    let decision = plan(&ids, &lookup(conn, account_id, &ids)?);

    let thread = match decision.keep {
        Some(thread) => thread,
        None => {
            conn.prepare_cached("INSERT INTO threads (account_id) VALUES (?1)")?
                .execute([account_id])?;
            conn.last_insert_rowid()
        }
    };
    for absorbed in decision.absorb {
        // Order matters: repoint BEFORE deleting, otherwise the foreign
        // key on `thread_links` refuses the deletion — and refusing is
        // the right reaction, it signals we were about to lose links.
        conn.execute(
            "UPDATE thread_links SET thread_id = ?2 WHERE thread_id = ?1",
            params![absorbed, thread],
        )?;
        conn.execute(
            "UPDATE envelopes SET thread_id = ?2 WHERE thread_id = ?1",
            params![absorbed, thread],
        )?;
        conn.execute("DELETE FROM threads WHERE id = ?1", [absorbed])?;
    }
    for id in decision.register {
        conn.prepare_cached(
            "INSERT OR IGNORE INTO thread_links (account_id, message_id, thread_id)
             VALUES (?1, ?2, ?3)",
        )?
        .execute(params![account_id, id, thread])?;
    }
    Ok(thread)
}

/// The threads already known for these identifiers — a single query.
fn lookup(
    conn: &Connection,
    account_id: i64,
    ids: &[String],
) -> Result<HashMap<String, ThreadId>, Error> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let placeholders = (2..=ids.len() + 1)
        .map(|n| format!("?{n}"))
        .collect::<Vec<_>>()
        .join(", ");
    let mut values: Vec<rusqlite::types::Value> = Vec::with_capacity(ids.len() + 1);
    values.push(account_id.into());
    values.extend(ids.iter().map(|id| id.clone().into()));

    // `prepare_cached`: the cache is indexed by the SQL text, and there
    // are only a handful of shapes (one per number of identifiers cited).
    // Without it, every message re-parses and re-plans its query — this
    // is the dominant cost of adopting a legacy database.
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT message_id, thread_id FROM thread_links
         WHERE account_id = ?1 AND message_id IN ({placeholders})"
    ))?;
    let known = stmt
        .query_map(rusqlite::params_from_iter(values), |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?
        .collect::<Result<_, _>>()?;
    Ok(known)
}

/// Recomputes a thread's aggregate from its messages — and deletes it if
/// it no longer has any.
///
/// **Recompute, never increment.** A counter maintained by additions and
/// subtractions drifts at the first forgotten path (merge, UIDVALIDITY,
/// replayed action), and a drift shows on screen forever: “4 messages” on
/// a thread that shows 3. The recompute is bounded by the thread's size
/// and goes through the index.
pub(crate) fn refresh(conn: &Connection, thread: ThreadId) -> Result<(), Error> {
    let aggregate = conn
        .prepare_cached(
            "SELECT e.mailbox_id, e.uid, e.date_epoch,
                    (SELECT COUNT(*) FROM envelopes WHERE thread_id = ?1),
                    (SELECT COUNT(*) FROM envelopes WHERE thread_id = ?1 AND seen = 0),
                    (SELECT COUNT(*) FROM envelopes x
                       JOIN mailboxes m ON m.id = x.mailbox_id
                      WHERE x.thread_id = ?1 AND m.name = ?2)
             FROM envelopes e
             WHERE e.thread_id = ?1
             ORDER BY e.date_epoch DESC, e.uid DESC
             LIMIT 1",
        )?
        .query_row(params![thread, RECEIVED_MAILBOX], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Uid>(1)?,
                row.get::<_, Option<i64>>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
            ))
        })
        .optional()?;

    match aggregate {
        Some((last_mailbox, last_uid, last_epoch, size, unseen, inbox_size)) => {
            // `organise_hors` is recomputed WITH the aggregate — same
            // rule as size/unseen: never increment, a drift shows
            // forever. The rule lives in ONE shared fragment
            // (`store::organized_off_sql` — golden rule included: a
            // mixed thread stays); key-based probes, ~0 when the mode
            // has never been used (empty tables, O(1) guard at the head
            // of the CASE). The SQL text is stable: `prepare_cached`
            // holds.
            conn.prepare_cached(&format!(
                "UPDATE threads SET last_mailbox_id = ?2, last_uid = ?3, last_epoch = ?4,
                                    size = ?5, unseen = ?6, inbox_size = ?7,
                                    organise_hors = {}
                 WHERE id = ?1",
                crate::store::organized_off_sql("?1")
            ))?
            .execute(params![
                thread,
                last_mailbox,
                last_uid,
                last_epoch,
                size,
                unseen,
                inbox_size
            ])?;
        }
        None => {
            // The thread emptied out: it disappears along with its
            // directory.
            //
            // Accepted consequence: if a reply arrives later, it opens a
            // NEW thread. That is honest — the mailbox no longer holds
            // anything of this conversation. Keeping the directory would
            // resurrect empty threads that the list would then have to
            // filter, at the cost of the index that keeps it fast.
            conn.execute("DELETE FROM thread_links WHERE thread_id = ?1", [thread])?;
            conn.execute("DELETE FROM threads WHERE id = ?1", [thread])?;
        }
    }
    Ok(())
}

/// The thread of a message, to be refreshed AFTER removing it from the
/// mailbox.
pub(crate) fn thread_of(
    conn: &Connection,
    mailbox_id: i64,
    uid: Uid,
) -> Result<Option<ThreadId>, Error> {
    let thread = conn
        .query_row(
            "SELECT thread_id FROM envelopes WHERE mailbox_id = ?1 AND uid = ?2",
            params![mailbox_id, uid],
            |row| row.get::<_, Option<ThreadId>>(0),
        )
        .optional()?
        .flatten();
    Ok(thread)
}

/// Rebuilds the threads of ONE account — called when one of its mailboxes
/// is reset (UIDVALIDITY changed: nothing in it means anything anymore).
///
/// **Why the whole account, and not just the mailbox.** Since
/// [ADR 0009] a thread reunites messages from several mailboxes. Erasing
/// only those of the reset mailbox would leave the others pointing at
/// vanished messages — and the directory does not say which mailbox
/// entered which identifier, by construction: it is the account that
/// carries it. The recompute is bounded by the account's size, and the
/// event is rare.
pub(crate) fn rebuild_account(conn: &Connection, account_id: i64) -> Result<(), Error> {
    conn.execute(
        "DELETE FROM thread_links WHERE account_id = ?1",
        [account_id],
    )?;
    conn.execute("DELETE FROM threads WHERE account_id = ?1", [account_id])?;
    conn.execute(
        "UPDATE envelopes SET thread_id = NULL
         WHERE mailbox_id IN (SELECT id FROM mailboxes WHERE account_id = ?1)",
        [account_id],
    )?;
    // Re-adopt RIGHT AWAY, not at the next opening: the list starts from
    // `threads`, so a message with `thread_id` NULL has no row. Deferring
    // would make the mailbox disappear from the screen in the meantime —
    // the trap of a feature that does not adopt its own data.
    let orphans = orphans(conn, Some(account_id))?;
    adopt(conn, orphans)
}

/// Attaches messages already in the database — those from before threads
/// existed.
///
/// Without this pass, every legacy message would keep `thread_id` NULL
/// and **disappear** from a list grouped by thread. That is exactly the
/// attachment trap, where metadata was only ever written by the new
/// path: a feature that does not adopt old data is wrong from the first
/// opening, and forever.
///
/// These messages only have their `Message-ID`: they will mostly form
/// single-message threads, which will regroup as headers get acquired
/// (that is the convergence property, at the top of the module).
/// Version of the grouping rule recorded in the database
/// (`PRAGMA user_version`, free to use and free to read).
///
/// **1** — identifiers are filtered by [`is_message_id`].
///
/// Older databases were grouped by a rule that took the words of a
/// header written in prose for identifiers: their threads are WRONG, and
/// no code fix repairs them on its own. They must be redone.
///
/// **2** — a thread's scope is the ACCOUNT, not the mailbox
/// ([ADR 0009](../../../docs/adr/0009-portee-des-fils-au-compte.md)).
///
/// Both tables change key, and SQLite cannot modify a primary key in
/// place: they are **dropped then recreated**, where version 1 was
/// content to just empty them.
pub(crate) const THREADING_VERSION: i64 = 2;

/// Drops the thread tables when the rule that produced them has changed —
/// **to call BEFORE applying [`SCHEMA`]**.
///
/// `CREATE TABLE IF NOT EXISTS` does not touch a table that exists: on an
/// older database, `threads` would therefore keep its columns. But the
/// partial index does not exist yet — SQLite actually creates it, and
/// fails on `inbox_size`, a column absent from the old table. **The
/// whole opening was refused, and the application no longer started.**
///
/// Defect found in the field: no test could see it, all of them creating
/// a fresh database, hence already at the current schema. The fixture
/// that reproduces it is
/// `une_base_au_schema_des_fils_precedent_s_ouvre_et_se_migre`.
///
/// The version marker is NOT advanced here: [`migrate_threads_with`]
/// does that, once the tables are recreated, the envelopes detached AND
/// the adoption finished — all within the same transaction, owned by
/// `Store::init`. Advancing it earlier would make cancellation partial:
/// the rewind (handover §8) requires that `ROLLBACK` leave
/// `user_version` unchanged.
pub(crate) fn drop_if_outdated(conn: &Connection) -> Result<(), Error> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version >= THREADING_VERSION {
        return Ok(());
    }
    conn.execute_batch(
        "DROP TABLE IF EXISTS thread_links;
         DROP TABLE IF EXISTS threads;",
    )?;
    Ok(())
}

/// Forwards a progress reading, and translates the response: `Break`
/// becomes [`Error::Interrupted`], which the caller's transaction
/// converts to `ROLLBACK` — the rewind of §8.
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

/// A message not yet attached: its account, its mailbox, its UID, then
/// the three grouping headers.
///
/// The account comes from the query rather than a per-message
/// resolution: over 200,000 orphans, one join done once beats 200,000
/// round trips.
type Orphan = (
    i64,
    i64,
    Uid,
    Option<String>,
    Option<String>,
    Option<String>,
);

/// The threadless messages — across all storage, or for a single
/// account.
fn orphans(conn: &Connection, account: Option<i64>) -> Result<Vec<Orphan>, Error> {
    // `m.threaded`: out of scope, `thread_id` stays NULL **forever**
    // (ADR 0010 §3). Without this filter, adoption would pick them back
    // up on every opening without ever closing them out — on a path
    // already measured at 3.7 s for 200,000 messages, and that a full
    // sync lengthens.
    //
    // And it is the MAILBOXES IN SCOPE that drive the scan (`CROSS
    // JOIN`: the join order is fixed, the index (mailbox_id, …) carries
    // the traversal). Starting from the envelopes, the plan started from
    // `idx_envelopes_thread (thread_id=NULL)` and enumerated the
    // everlasting NULLs of the WHOLE database to discard them after the
    // join — 247,835 rows, 398 ms, on EVERY `Store::open`, hence on every
    // command (measured at gate P1 of the redesign, `diagnostic_ouverture`).
    // Driven by scope: 3,229 rows, 23 ms — the cost follows what
    // adoption may have to do, plus the size of the database.
    const BASE: &str = "SELECT m.account_id, e.mailbox_id, e.uid,
                e.message_id, e.in_reply_to, e.refs
         FROM mailboxes m CROSS JOIN envelopes e ON e.mailbox_id = m.id
         WHERE m.threaded = 1 AND e.thread_id IS NULL";
    let read = |row: &rusqlite::Row<'_>| {
        Ok((
            row.get(0)?,
            row.get(1)?,
            row.get(2)?,
            row.get(3)?,
            row.get(4)?,
            row.get(5)?,
        ))
    };
    let rows = match account {
        Some(account_id) => conn
            .prepare(&format!(
                "{BASE} AND m.account_id = ?1 ORDER BY e.mailbox_id, e.uid"
            ))?
            .query_map([account_id], read)?
            .collect::<Result<Vec<_>, _>>()?,
        None => conn
            .prepare(&format!("{BASE} ORDER BY e.mailbox_id, e.uid"))?
            .query_map([], read)?
            .collect::<Result<Vec<_>, _>>()?,
    };
    Ok(rows)
}

/// The unit of adoption for legacy messages, WITHOUT a transaction: it is
/// the caller that owns it — `Store::init` extends it from the
/// conditional DROP through to `user_version`, so that cancellation
/// rewinds everything (§8).
///
/// Returns the total announced to `on_progress` when a pass took place:
/// the caller will report `(total, total)` again once the transaction is
/// COMMITTED — “done” is never said before it is true.
pub(crate) fn migrate_threads_with(
    conn: &Connection,
    on_progress: &mut dyn FnMut(AdoptionProgress) -> ControlFlow<()>,
) -> Result<Option<u64>, Error> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let outdated = version < THREADING_VERSION;
    if outdated {
        // The tables were already dropped by `drop_if_outdated` then
        // recreated empty by `SCHEMA`: what remains is detaching the
        // envelopes so that the adoption right below redoes the threads
        // — a single reconstruction path, the one already tested. Purely
        // local: the raw headers are intact in the database, only their
        // interpretation was faulty. Nothing to ask the server for
        // again.
        conn.execute_batch("UPDATE envelopes SET thread_id = NULL")?;
    }
    let orphans = orphans(conn, None)?;
    let mut announced = None;
    if !orphans.is_empty() {
        // The total is an UPPER BOUND declared up front: attach each
        // orphan, then consolidate AT MOST that many threads. It never
        // moves afterwards — a bar that goes backward is worse than an
        // imprecise bar.
        let total = orphans.len() as u64 * 2;
        announced = Some(total);
        report(on_progress, 0, total)?;
        adopt_with_progress(conn, orphans, total, on_progress)?;
    }
    if outdated {
        // The version is recorded WITHIN the same transaction as the
        // adoption: cancelling leaves `user_version` unchanged, and the
        // whole pass replays at the next launch. Never a partially
        // persisted adoption — the list starts from `threads`, a
        // half-adopted database would be a half-empty mailbox.
        conn.execute_batch(&format!("PRAGMA user_version = {THREADING_VERSION}"))?;
    }
    Ok(announced)
}

/// The same path, silent and transactional — for direct test calls,
/// which have neither an interface to feed nor an open transaction.
/// Production goes through `Store::init`, which owns the transaction.
///
/// One transaction, not one per message: on an already-full mailbox, an
/// fsync per envelope would turn the application's opening into minutes
/// of waiting — the “startup < 1 s” budget forbids this path.
#[cfg(test)]
pub(crate) fn migrate_threads(conn: &Connection) -> Result<(), Error> {
    conn.execute_batch("BEGIN")?;
    match migrate_threads_with(conn, &mut |_| ControlFlow::Continue(())) {
        Ok(_) => {
            conn.execute_batch("COMMIT")?;
            Ok(())
        }
        Err(err) => {
            // A failed rollback would teach nothing more than the
            // original error, which is the one that must be reported.
            let _ = conn.execute_batch("ROLLBACK");
            Err(err)
        }
    }
}

/// Report step: ~1,000 messages take ~18 ms at the rate measured by
/// `banc_migration_fils` — the callback's cost is invisible, and the
/// cancellation latency stays below perception.
const REPORT_STEP: u64 = 1_000;

fn adopt(conn: &Connection, orphans: Vec<Orphan>) -> Result<(), Error> {
    // Silent path (UIDVALIDITY invalidated, targeted reconstruction):
    // same steps, without an observer or cancellation — the event is
    // rare and bounded by the account's size.
    let total = orphans.len() as u64 * 2;
    adopt_with_progress(conn, orphans, total, &mut |_| ControlFlow::Continue(()))
}

fn adopt_with_progress(
    conn: &Connection,
    orphans: Vec<Orphan>,
    total: u64,
    on_progress: &mut dyn FnMut(AdoptionProgress) -> ControlFlow<()>,
) -> Result<(), Error> {
    // A SET, not a list. `Vec::contains` is linear: on a legacy database
    // where almost every message opens its own thread, “have I already
    // seen this thread?” became quadratic — 160,000 threads make
    // ~1.3×10¹⁰ comparisons. Measured: 11.1 s of adoption over 200,000
    // messages, against a one-second startup budget. Invisible on the
    // 2,800 messages of a real mailbox, crushing at gate 3 scale. The
    // tree also keeps a deterministic order, with no sort needed.
    let mut touched: BTreeSet<ThreadId> = BTreeSet::new();
    let mut done: u64 = 0;
    for (account_id, mailbox_id, uid, message_id, in_reply_to, references) in orphans {
        let thread = attach(
            conn,
            account_id,
            message_id.as_deref(),
            in_reply_to.as_deref(),
            references.as_deref(),
            // Adopting a legacy database has no addresses on hand: the
            // address guard rail only plays during sync.
            &[],
        )?;
        conn.prepare_cached(
            "UPDATE envelopes SET thread_id = ?3 WHERE mailbox_id = ?1 AND uid = ?2",
        )?
        .execute(params![mailbox_id, uid, thread])?;
        touched.insert(thread);
        done += 1;
        if done.is_multiple_of(REPORT_STEP) {
            report(on_progress, done, total)?;
        }
    }
    for thread in touched {
        // A thread from `touched` may have been absorbed in the
        // meantime; `refresh` notices and does nothing.
        refresh(conn, thread)?;
        done += 1;
        if done.is_multiple_of(REPORT_STEP) {
            report(on_progress, done, total)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known(pairs: &[(&str, ThreadId)]) -> HashMap<String, ThreadId> {
        pairs
            .iter()
            .map(|(id, thread)| ((*id).to_string(), *thread))
            .collect()
    }

    fn ids(raw: &[&str]) -> Vec<String> {
        raw.iter().map(|id| (*id).to_string()).collect()
    }

    /// A message with neither a `Message-ID` nor an ancestor has NO
    /// identifier at all. It must therefore register none: two silent
    /// messages must remain two distinct threads, not join up on
    /// “nothing”.
    #[test]
    fn a_message_with_no_identifier_attaches_to_nothing() {
        assert!(linking_ids(None, None, None, &[]).is_empty());

        let plan = plan(&[], &known(&[]));
        assert_eq!(plan.keep, None, "it needs a new thread");
        assert!(
            plan.register.is_empty(),
            "nothing to register: otherwise the next message, also \
             silent, would fall into the same thread"
        );
    }

    #[test]
    fn a_reply_joins_its_parent_s_thread() {
        let links = linking_ids(Some("<r@b>"), Some("<a@b>"), None, &[]);
        let plan = plan(&links, &known(&[("a@b", 7)]));

        assert_eq!(plan.keep, Some(7));
        assert!(plan.absorb.is_empty());
        assert_eq!(
            plan.register,
            ids(&["r@b"]),
            "only the new identifier is registered"
        );
    }

    /// Disorder is the rule, not the exception: sync fetches by UID, and
    /// a reply can precede what it cites. The absent ancestor is
    /// therefore registered too, as a reservation.
    #[test]
    fn an_absent_ancestor_is_registered_so_its_arrival_joins_the_thread() {
        // The reply arrives first: nothing is known yet.
        let reply = linking_ids(Some("<r@b>"), Some("<a@b>"), None, &[]);
        let first = plan(&reply, &known(&[]));
        assert_eq!(first.keep, None);
        assert_eq!(
            first.register,
            ids(&["r@b", "a@b"]),
            "the still-absent ancestor is reserved"
        );

        // Thread 3 is created and carries both reservations. The parent
        // arrives next: it recognizes itself.
        let parent = linking_ids(Some("<a@b>"), None, None, &[]);
        let next = plan(&parent, &known(&[("r@b", 3), ("a@b", 3)]));
        assert_eq!(next.keep, Some(3));
        assert!(next.register.is_empty());
    }

    /// The case that makes everything work: in an inbox, the
    /// intermediate message of an exchange is the one that was SENT — it
    /// is not there. It is `References`, which also carries the root,
    /// that knits the two halves back together.
    #[test]
    fn the_message_that_links_two_threads_merges_them() {
        let links = linking_ids(Some("<c@b>"), Some("<b@b>"), Some("<a@b> <b@b>"), &[]);
        let plan = plan(&links, &known(&[("a@b", 4), ("b@b", 9)]));

        assert_eq!(plan.keep, Some(4));
        assert_eq!(plan.absorb, vec![9]);
        assert_eq!(plan.register, ids(&["c@b"]));
    }

    /// The tie-break must not depend on the order of identifiers in the
    /// header, otherwise the same message classified twice gives two
    /// different results.
    #[test]
    fn the_merge_always_keeps_the_oldest_thread() {
        let directory = known(&[("a@b", 4), ("b@b", 9), ("c@b", 6)]);

        let forward = plan(&ids(&["a@b", "b@b", "c@b"]), &directory);
        let reversed = plan(&ids(&["b@b", "c@b", "a@b"]), &directory);

        assert_eq!(forward.keep, Some(4));
        assert_eq!(forward.absorb, vec![6, 9]);
        assert_eq!(forward, reversed, "the result does not depend on order");
    }

    /// Some software copies the message's own `Message-ID` into its own
    /// `References`. Citing itself must neither duplicate nor trigger a
    /// thread merging with itself.
    #[test]
    fn a_message_that_cites_itself_does_not_create_a_second_thread() {
        let links = linking_ids(Some("<r@b>"), Some("<r@b>"), Some("<r@b>"), &[]);
        assert_eq!(links, ids(&["r@b"]), "only one identifier retained");

        let plan = plan(&links, &known(&[("r@b", 2)]));
        assert_eq!(plan.keep, Some(2));
        assert!(plan.absorb.is_empty(), "a thread does not absorb itself");
    }

    /// THE field defect, in one assertion.
    ///
    /// `In-Reply-To: Your message of January 3rd` — the RFC 822 form,
    /// which some autoresponders still produce. The old rule extracted
    /// five identifiers from it: “Your”, “message”, “of”, “January”,
    /// “3rd”. Every message carrying this phrase latched onto the same
    /// anchors and ended up in a single thread. Measured on a real
    /// mailbox: 43 unrelated messages reunited.
    /// PLAN-AUDIT-V2 E5: `In-Reply-To: <alice@x.fr>` — an autoresponder
    /// that puts the recipient's ADDRESS between angle brackets — passed
    /// the guard rail (an at sign, no space) and merged unrelated threads
    /// (ADR 0008 to the letter, not in spirit). Envelope addresses are
    /// never identifiers.
    #[test]
    fn an_address_between_angle_brackets_is_not_a_message_id() {
        let addresses = vec!["Alice@x.fr".to_string(), "bob@y.fr".to_string()];
        let links = linking_ids(Some("<r@b>"), Some("<alice@x.fr>"), None, &addresses);
        assert_eq!(links, vec!["r@b".to_string()]);
        let links = linking_ids(None, None, Some("<a@b> <bob@y.fr> <c@d>"), &addresses);
        assert_eq!(links, vec!["a@b".to_string(), "c@d".to_string()]);
    }

    #[test]
    fn a_header_written_in_prose_produces_no_identifier() {
        assert!(canonical_ids("Votre message du 3 janvier").is_empty()); // lang:fr
        assert!(canonical_ids("Your message of Mon, 01 Jan 2024").is_empty());
        assert!(linking_ids(None, Some("Votre message du 3 janvier"), None, &[]).is_empty()); // lang:fr
    }

    /// RFC 5322 §3.6.4: the at sign is not decorative, it is what
    /// distinguishes an identifier from a word.
    #[test]
    fn a_token_without_an_at_sign_is_not_an_identifier() {
        assert!(canonical_ids("NIL").is_empty());
        assert!(canonical_ids("0").is_empty());
        assert!(
            canonical_ids("<1234567890>").is_empty(),
            "even between angle brackets: out of spec, and short so prone to collision"
        );
        assert_eq!(canonical_ids("<a@b>"), ids(&["a@b"]));
    }

    /// The fallback without angle brackets remains useful — many pieces
    /// of software omit them — but it now only retains what IS an
    /// identifier.
    #[test]
    fn the_fallback_without_angle_brackets_keeps_only_real_identifiers() {
        assert_eq!(canonical_ids("a@b Votre message c@d"), ids(&["a@b", "c@d"])); // lang:fr
    }

    /// An identifier does not contain whitespace: without this rule, a
    /// prose header between angle brackets would sneak back through the
    /// window.
    #[test]
    fn a_token_containing_whitespace_is_rejected() {
        assert!(canonical_ids("<Votre message du 3 janvier@relais>").is_empty()); // lang:fr
    }

    #[test]
    fn missing_angle_brackets_give_the_same_identifier() {
        assert_eq!(canonical_ids("<a@b>"), ids(&["a@b"]));
        assert_eq!(canonical_ids("  a@b  "), ids(&["a@b"]));
        assert_eq!(canonical_ids("< a@b >"), ids(&["a@b"]));
        assert_eq!(canonical_ids("a@b c@d"), ids(&["a@b", "c@d"]));
    }

    /// The giant `References` of the neighboring test must remain made
    /// of real identifiers, otherwise the limit would prove nothing
    /// anymore.
    #[test]
    fn the_limit_applies_after_filtering() {
        let raw: String = (0..40).map(|n| format!("<m{n}@b> mot ")).collect();
        let links = linking_ids(None, None, Some(&raw), &[]);
        assert_eq!(links.len(), MAX_REFERENCES);
        assert!(links.iter().all(|id| id.contains('@')));
    }

    /// `References` reads folded over several lines; the whitespace
    /// separating the angle brackets does not belong to the identifiers.
    #[test]
    fn a_references_folded_over_several_lines_reads_in_full() {
        assert_eq!(
            canonical_ids("<a@b>\r\n\t<c@d>\r\n <e@f>"),
            ids(&["a@b", "c@d", "e@f"])
        );
    }

    #[test]
    fn an_empty_or_truncated_header_produces_no_identifier() {
        assert!(canonical_ids("").is_empty());
        assert!(canonical_ids("   ").is_empty());
        assert!(canonical_ids("<>").is_empty(), "empty angle brackets");
    }

    /// The limit protects the directory query: without it, a pathological
    /// header would look up thousands of identifiers for a single
    /// message.
    #[test]
    fn a_giant_references_keeps_the_root_and_the_immediate_ancestors() {
        let raw: String = (0..500).map(|n| format!("<m{n}@b> ")).collect();
        let links = linking_ids(Some("<moi@b>"), None, Some(&raw), &[]);

        assert_eq!(links.len(), MAX_REFERENCES + 1, "its own, plus the limit");
        assert_eq!(links[0], "moi@b");
        assert_eq!(links[1], "m0@b", "the root attaches the whole thread");
        assert_eq!(
            links[KEPT_AT_ROOT + 1],
            "m476@b",
            "then the jump to the immediate ancestors"
        );
        assert_eq!(
            links[MAX_REFERENCES], "m499@b",
            "and the nearest one closes the list"
        );
    }

    /// The message's own identifier precedes its ancestors: it is the
    /// one future replies will cite.
    #[test]
    fn the_message_s_own_identifier_comes_first() {
        let links = linking_ids(Some("<moi@b>"), Some("<parent@b>"), Some("<racine@b>"), &[]);
        assert_eq!(links, ids(&["moi@b", "racine@b", "parent@b"]));
    }

    /// Two ancestors already attached to the same thread only count
    /// once: `absorb` must not contain the kept thread.
    #[test]
    fn two_ancestors_of_the_same_thread_do_not_trigger_a_merge() {
        let plan = plan(&ids(&["a@b", "c@b"]), &known(&[("a@b", 5), ("c@b", 5)]));
        assert_eq!(plan.keep, Some(5));
        assert!(plan.absorb.is_empty());
    }
}

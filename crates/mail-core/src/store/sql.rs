use super::*;

/// Columns of the unified SELECT, shared by [`Store::unified_recent`]
/// and [`Store::search`] — the order is that of [`row_to_unified`].
/// The last column is an EXISTS on `attachments`: the list must be
/// able to show the paperclip without one query per row. The primary
/// key (mailbox_id, uid, idx) makes this test indexed.
// Requires the aliases `e` (envelopes), `m` (mailboxes), `a`
// (accounts) AND the join `LEFT JOIN bodies b` — the list preview
// comes from there, NULL until the body is fetched. The attachment
// COUNT replaces the old EXISTS: the prototype's chip says "2 files",
// not "some files". Both only run on the rows KEPT by pagination
// (gate P1).
pub(crate) const SELECT_UNIFIED: &str = "SELECT a.id, a.email, e.uid, e.subject, e.sender, e.sender_address, e.message_id, e.date_epoch, e.seen, e.flagged, (SELECT COUNT(*) FROM attachments att WHERE att.mailbox_id = e.mailbox_id AND att.uid = e.uid), e.thread_id, e.in_reply_to, m.name, b.preview, e.to_addrs, e.cc_addrs";

/// The SELECT for the grouped list: the columns above, plus the thread
/// aggregate. It requires the join on `threads` (alias `t`), which
/// search does not have — a search result is ONE message, not a
/// conversation. Comes AFTER `to_addrs`/`cc_addrs` of
/// [`SELECT_UNIFIED`]: `t.size`/`t.unseen` are therefore at indices
/// 17/18.
pub(crate) const THREAD_AGGREGATE: &str = ", t.size, t.unseen";

/// PINNED threads (R4, PLAN-RETOURS-7) — the subquery shared by the
/// page (exclusion, D5), the count, and the standalone service.
/// Materialized ONCE per query (LIST SUBQUERY), small by construction
/// (a handful of pins at most) — but ONLY IF `pins` is the outer
/// table: without `ANALYZE` (never run here), SQLite picks `envelopes`
/// as the outer table and pays a FULL scan of the widest table on the
/// hottest path (review 2026-08-21, measured on the bench: ~24 ms per
/// page at 200k). The `CROSS JOIN` is SQLite's join-order directive:
/// `pins` is scanned, `envelopes` is probed by its primary key. The
/// plan guard `la_boite_unifiee_ne_materialise_pas_son_tri` proves it.
pub(crate) const PINNED_THREADS: &str = "SELECT pe.thread_id FROM pins p CROSS JOIN envelopes pe ON pe.mailbox_id = p.mailbox_id AND pe.uid = p.uid WHERE pe.thread_id IS NOT NULL";

/// SET-ASIDE threads (E5) — the twin of [`PINNED_THREADS`], same
/// reasons: list materialized once, small by construction, and a
/// directive `CROSS JOIN` (without ANALYZE, SQLite would pick
/// `envelopes` as the outer table — a full scan on the hottest path).
pub(crate) const SET_ASIDE_THREADS: &str = "SELECT ce.thread_id FROM mis_de_cote c CROSS JOIN envelopes ce ON ce.mailbox_id = c.mailbox_id AND ce.uid = c.uid WHERE ce.thread_id IS NOT NULL";

/// The tail of the unified list — joins and final sort — shared by the
/// page ([`unified_page_sql`]) and the pinned section
/// ([`Store::pinned_unified_scoped`]): ONE place to write it, the two
/// queries can no longer drift apart (review 2026-08-21 — copying the
/// skeleton would have shifted the columns on the first addition).
pub(crate) const UNIFIED_JOINS: &str = "
         JOIN envelopes e ON e.mailbox_id = t.last_mailbox_id AND e.uid = t.last_uid
         JOIN mailboxes m ON m.id = e.mailbox_id
         JOIN accounts a ON a.id = t.account_id
         LEFT JOIN bodies b ON b.mailbox_id = e.mailbox_id AND b.uid = e.uid";

/// The predicate "this message still awaits its body", shared by the
/// ACCOUNT ([`Store::bodies_pending_count`]) and the working LIST
/// ([`Store::bodies_to_backfill`]).
///
/// ONE piece of writing: the two can no longer diverge — and it is
/// this piece of writing, never a copy, that the plan guard queries
/// (same reason as [`unified_page_sql`], and the same lesson paid
/// for).
///
/// **It reads NO column of `bodies`, and that is the whole point.**
/// The row's existence is decided from the auto-index of the primary
/// key `(mailbox_id, uid)` — so without ever recalling the row,
/// which weighs 56 KB on average in the field. Reading even a single
/// bit cost 251k random reads across 11.4 GB: **20,839 ms cold
/// versus 396 ms without** (measured 2026-08-26 on the field database).
///
/// This predicate used to carry `AND b.scanned = 1` — the trace of
/// bodies fetched BEFORE attachments existed, whose MIME had never
/// been inspected. **Removed 2026-08-26 (PLAN-DEMARRAGE, decision
/// D8)** on three measured facts: production NEVER writes
/// `scanned = 0` ([`Store::save_body_full`] hardcodes a `1`), both
/// fleet workstations carry **zero** rows at `scanned = 0`, and the
/// criterion cost an 8,870 ms startup freeze to protect zero rows. The
/// column survives, vestigial: removing it would require rewriting
/// 11.4 GB — it will leave with whatever job touches `bodies` anyway
/// (the preview, a debt).
///
/// **Requires the alias `e`** for `envelopes` wherever it is used —
/// as [`SELECT_UNIFIED`] requires its own. The fragment is a string:
/// a different alias compiles and fails at `prepare`, on a path where
/// the UI shows nothing (the backfill's `catch` is a
/// `console.error`).
pub(crate) const BODY_ABSENT: &str = "NOT EXISTS (
                   SELECT 1 FROM bodies b
                    WHERE b.mailbox_id = e.mailbox_id AND b.uid = e.uid
               )";

/// The COUNT of missing bodies for a mailbox: `?1` the account, `?2`
/// the mailbox, `?3` the horizon.
pub(crate) fn bodies_pending_count_sql() -> String {
    format!(
        "SELECT COUNT(*)
             FROM envelopes e
             JOIN mailboxes m ON m.id = e.mailbox_id
             WHERE m.account_id = ?1 AND m.name = ?2
               AND (e.date_epoch IS NULL OR e.date_epoch >= ?3)
               AND {BODY_ABSENT}"
    )
}

/// The working LIST of the backfill — same parameters, plus `?4`, the
/// batch bound.
pub(crate) fn bodies_to_backfill_sql() -> String {
    format!(
        "SELECT e.uid
             FROM envelopes e
             JOIN mailboxes m ON m.id = e.mailbox_id
             WHERE m.account_id = ?1 AND m.name = ?2
               AND (e.date_epoch IS NULL OR e.date_epoch >= ?3)
               AND {BODY_ABSENT}
             ORDER BY e.date_epoch DESC, e.uid DESC
             LIMIT ?4"
    )
}

/// The query for a page of the unified mailbox.
///
/// Isolated so a test can query **its own** execution plan, and not a
/// copy that would diverge the day one of the two changes. The cost
/// of this query is the hottest path of the product.
/// `organized` (E2): the ORGANIZED Inbox — the SAME skeleton plus the
/// retention flag, in the EXACT shape of the partial index
/// `idx_threads_date_organise` which then carries sort, filter and
/// pagination (S2-bis: the offset skips index entries, never probed
/// rows). ONE piece of writing for both modes — the E1 review had
/// isolated this query precisely so that no copy would diverge.
pub(crate) fn unified_page_sql(by_account: bool, unread_only: bool, organized: bool) -> String {
    // Pagination (`LIMIT`/`OFFSET`) applies in a subquery on
    // `threads` ALONE, not on the join: `OFFSET` produces then
    // discards each skipped row, so everything computed per row — the
    // triple join and the correlated `EXISTS` on `attachments` from
    // SELECT_UNIFIED — was being paid for the 200,000 rows of a deep
    // jump. Measured (rewrite gate P1, 205,050 conversations):
    // 252.6 ms at offset 200,000, linear growth. With the skeleton in
    // a subquery, the jump only walks the partial index
    // `idx_threads_date_globale` — which carries the COMPLETE sort
    // key (last_epoch DESC, last_uid DESC, account_id) and the filter
    // `inbox_size > 0` — and the joins only run on the `limit`
    // retained rows.
    //
    // The outer ORDER BY re-sorts the retained rows with the same
    // key: it guarantees the final order whatever the join strategy,
    // for the price of sorting `limit` rows.
    // `by_account` adds the `account_id = ?3` filter of nav v2
    // ("Mailboxes" of screen 02): same skeleton, the prefixed index
    // `idx_threads_date (account_id, …)` then carries sort and
    // pagination.
    // `unread_only` is the "Unread" tab of the prototype — filtered
    // HERE, not on the client side: 331 conversations out of 2,929 in
    // the field, a page must only carry what it displays.
    let filter = if by_account {
        " AND account_id = ?3"
    } else {
        ""
    };
    let unread_only_clause = if unread_only { " AND unseen > 0" } else { "" };
    // E5: in organized mode, SET-ASIDE threads leave the flow — they
    // live in the pile (shared exclusion, pins pattern). The classic
    // mode excludes nothing.
    let exclusion = if organized {
        organized_exclusion()
    } else {
        String::new()
    };
    // E4: the INTERNAL order (the one the partial index carries)
    // follows the sections in organized mode — same key as the join
    // tail.
    let sort_clause = if organized {
        "ORDER BY (unseen > 0) DESC, last_epoch DESC, last_uid DESC, account_id"
    } else {
        "ORDER BY last_epoch DESC, last_uid DESC, account_id"
    };
    let tail = unified_join_tail(organized);
    // R4 (PLAN-RETOURS-7, D5): PINNED conversations leave the
    // paginated flow — they are served SEPARATELY, at the top of page
    // 0 (`pinned_unified_scoped`); the list never shows the same
    // message twice. `NOT IN` on the pins subquery: a list
    // materialized once, tiny by construction.
    format!(
        "{SELECT_UNIFIED}{THREAD_AGGREGATE}
         FROM (SELECT account_id, last_mailbox_id, last_uid, last_epoch, size, unseen
                 FROM threads
                WHERE inbox_size > 0{exclusion} AND id NOT IN ({PINNED_THREADS}){filter}{unread_only_clause}
                {sort_clause}
                LIMIT ?1 OFFSET ?2) t{tail}"
    )
}

/// The senders index (sender, date, mailbox) — named ONCE: Cleanup
/// queries require it via `INDEXED BY` (review: four copies of the
/// name, a rename would have silently missed one).
pub(crate) const SENDERS_INDEX: &str = "idx_envelopes_sender";

/// The fields of an envelope that live in the search index — as
/// reread from the database, to know whether a resync has changed
/// them (subject, sender, address, recipients, cc).
pub(super) type IndexedFields = (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

/// Recipients stored on one row — one per `\n`, NULL when empty (R4).
/// `join`/`split` are reciprocal; an address never contains a line
/// break (it is `mailbox@host`).
/// The addresses an envelope carries (sender, To, Cc) — never thread
/// identifiers, even in angle brackets (PLAN-AUDIT-V2 E5).
pub(super) fn addresses_from(envelope: &Envelope) -> Vec<String> {
    let mut addresses: Vec<String> = Vec::new();
    addresses.extend(envelope.sender_address.clone());
    addresses.extend(envelope.to_addrs.iter().cloned());
    addresses.extend(envelope.cc_addrs.iter().cloned());
    addresses
}

pub(super) fn join_addrs(addrs: &[String]) -> Option<String> {
    if addrs.is_empty() {
        None
    } else {
        Some(addrs.join("\n"))
    }
}

pub(super) fn split_addrs(raw: Option<String>) -> Vec<String> {
    raw.map(|s| {
        s.split('\n')
            .filter(|part| !part.is_empty())
            .map(str::to_string)
            .collect()
    })
    .unwrap_or_default()
}

/// Mapping shared by every envelope read — the column order is that
/// of the SELECTs above (`to_addrs`/`cc_addrs` at the tail, index
/// 9/10).
/// THE SINGLE authority for normalizing an address for the image
/// memory (R1, PLAN-RETOURS-11): Unicode lowercase on the Rust side —
/// writing (`allow_images_sender_of`, `revoke_images_sender`) and
/// reading (`images_allowed`) all go through here.
pub(super) fn images_address(adresse: Option<String>) -> Option<String> {
    adresse
        .map(|a| a.trim().to_lowercase())
        .filter(|a| !a.is_empty())
}

/// Purges the Screener's ranks that no longer rest on ANY mail (E2):
/// the pending state is DERIVED — a recycled UID inherits no decision
/// (A43/A89). Shared by account removal and mailbox reset.
/// THE list of "per message" tables, for the three purges
/// (`remove_local`, `remove_absent`, `reset_mailbox`) —
/// PLAN-AUDIT-V1 E4. Before: three diverging copies, `remove_absent`
/// was missing five. Pending actions are NOT in the list: depending
/// on the purge, they carry the gesture (`remove_local`) or are
/// unrealizable (`remove_absent`, `reset_mailbox` — which removes
/// them separately).
pub(crate) const TABLES_PER_MESSAGE: [&str; 7] = [
    "bodies",
    "invitations",
    "attachments",
    "images_messages",
    "mis_de_cote",
    "kiosque_lus",
    "envelopes",
];

/// Purges ONE message from all its tables and returns its thread,
/// READ BEFORE the deletion (after, the link is lost) — without
/// refreshing it: it is the caller who refreshes, ONCE per affected
/// thread (review PLAN-AUDIT-V1: a refresh per message cost ~500× on
/// a thread of 500 vanished messages).
pub(crate) fn purge_message(
    conn: &Connection,
    mailbox_id: i64,
    uid: Uid,
) -> Result<Option<thread::ThreadId>, Error> {
    let thread = thread::thread_of(conn, mailbox_id, uid)?;
    search::deindex_message(conn, mailbox_id, uid)?;
    for table in TABLES_PER_MESSAGE {
        conn.execute(
            &format!("DELETE FROM {table} WHERE mailbox_id = ?1 AND uid = ?2"),
            params![mailbox_id, uid],
        )?;
    }
    Ok(thread)
}

/// The refused actions of a message (quarantine E3): a fresh gesture
/// from the user replaces them.
pub(super) fn forget_refused(conn: &Connection, mailbox_id: i64, uid: Uid) -> Result<(), Error> {
    conn.execute(
        "DELETE FROM pending_actions WHERE mailbox_id = ?1 AND uid = ?2 AND refusee = 1",
        params![mailbox_id, uid],
    )?;
    Ok(())
}

pub(super) fn row_to_envelope(row: &rusqlite::Row<'_>) -> rusqlite::Result<Envelope> {
    Ok(Envelope {
        reply_to: None,
        uid: row.get(0)?,
        subject: row.get(1)?,
        sender: row.get(2)?,
        sender_address: row.get(3)?,
        message_id: row.get(4)?,
        date: row
            .get::<_, Option<i64>>(5)?
            .and_then(|epoch| DateTime::from_timestamp(epoch, 0)),
        seen: row.get(6)?,
        flagged: row.get(7)?,
        in_reply_to: row.get(8)?,
        to_addrs: split_addrs(row.get(9)?),
        cc_addrs: split_addrs(row.get(10)?),
    })
}

/// Mapping shared by reads of the unified mailbox — the column order
/// is that of [`SELECT_UNIFIED`].
pub(crate) fn row_to_unified(row: &rusqlite::Row<'_>) -> rusqlite::Result<UnifiedRow> {
    let attachment_count = row.get::<_, i64>(10)?.max(0) as u32;
    Ok(UnifiedRow {
        account_id: row.get(0)?,
        account_email: row.get(1)?,
        envelope: Envelope {
            reply_to: None,
            uid: row.get(2)?,
            subject: row.get(3)?,
            sender: row.get(4)?,
            sender_address: row.get(5)?,
            message_id: row.get(6)?,
            date: row
                .get::<_, Option<i64>>(7)?
                .and_then(|epoch| DateTime::from_timestamp(epoch, 0)),
            seen: row.get(8)?,
            flagged: row.get(9)?,
            in_reply_to: row.get(12)?,
            to_addrs: split_addrs(row.get(15)?),
            cc_addrs: split_addrs(row.get(16)?),
        },
        mailbox: row.get(13)?,
        has_attachment: attachment_count > 0,
        attachment_count,
        preview: row.get(14)?,
        thread_id: row.get(11)?,
        // Values for a message seen ALONE — this is the case for
        // search, which does not join `threads`. The grouped list
        // overwrites them with the real aggregate via
        // [`row_to_threaded`].
        thread_size: 1,
        thread_unseen: u32::from(!row.get::<_, bool>(8)?),
        // Set by the PAGE pass (`enrichir_lignes`), never here.
        invitation: None,
    })
}

/// Mapping for the grouped list: the unified columns, then the thread
/// aggregate added by [`THREAD_AGGREGATE`].
pub(crate) fn row_to_threaded(row: &rusqlite::Row<'_>) -> rusqlite::Result<UnifiedRow> {
    Ok(UnifiedRow {
        // `to_addrs`/`cc_addrs` pushed the aggregate to indexes 17/18.
        thread_size: row.get(17)?,
        thread_unseen: row.get(18)?,
        ..row_to_unified(row)?
    })
}

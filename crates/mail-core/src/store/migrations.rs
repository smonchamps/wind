use super::*;

pub(super) fn migrate(
    conn: &Connection,
    on_progress: &mut dyn FnMut(AdoptionProgress) -> ControlFlow<()>,
) -> Result<(), Error> {
    migrate_multi_account(conn)?;
    add_missing_columns(
        conn,
        "drafts",
        &[
            ("account_id", "INTEGER NOT NULL DEFAULT 1"),
            ("reply_to_mailbox", "TEXT"),
        ],
    )?;
    // ADR 0010: the scope of grouping becomes explicit. The mailboxes
    // already in the database are INBOX and "Sent" — both included,
    // hence the default of 1. A legacy database therefore keeps
    // exactly the threads it had: the migration changes nothing about
    // what is displayed.
    add_missing_columns(
        conn,
        "mailboxes",
        &[("threaded", "INTEGER NOT NULL DEFAULT 1")],
    )?;
    add_missing_columns(conn, "accounts", &[("sent_mailbox", "TEXT")])?;
    add_missing_columns(conn, "folders", &[("special_use", "TEXT")])?;
    add_missing_columns(conn, "mailboxes", &[("relevee_epoch", "INTEGER")])?;
    add_missing_columns(
        conn,
        "mailboxes",
        &[("remote_total", "INTEGER NOT NULL DEFAULT 0")],
    )?;
    // ADR 0017: the UIDNEXT seen at the last poll — NULL as long as
    // no completed poll has taken place, so a legacy database polls
    // everything on its first cycle (conservative), then becomes
    // frugal.
    add_missing_columns(conn, "mailboxes", &[("remote_uidnext", "INTEGER")])?;
    // PLAN-AUDIT-V1 E3: the quarantine of refused actions.
    add_missing_columns(
        conn,
        "pending_actions",
        &[
            ("attempts", "INTEGER NOT NULL DEFAULT 0"),
            ("refusee", "INTEGER NOT NULL DEFAULT 0"),
            ("last_error", "TEXT"),
        ],
    )?;
    // PLAN-AUDIT-V1 E2: the initialization flag. On a legacy
    // database, ONCE, when the column is added: any mailbox that
    // already has a marker is deemed initialized — rows at 0 keep the
    // previous behavior (first pass = initial).
    if !table_columns(conn, "mailboxes")?.contains("initialisee") {
        add_missing_columns(
            conn,
            "mailboxes",
            &[("initialisee", "INTEGER NOT NULL DEFAULT 0")],
        )?;
        conn.execute(
            "UPDATE mailboxes SET initialisee = 1 WHERE last_uid > 0",
            [],
        )?;
    }
    add_missing_columns(
        conn,
        "outbox",
        &[
            ("account_id", "INTEGER NOT NULL DEFAULT 1"),
            ("refs", "TEXT"),
            ("reply_to", "TEXT"),
        ],
    )?;
    add_missing_columns(
        conn,
        "envelopes",
        &[
            ("sender_address", "TEXT"),
            ("message_id", "TEXT"),
            ("flagged", "INTEGER NOT NULL DEFAULT 0"),
            ("in_reply_to", "TEXT"),
            ("refs", "TEXT"),
            // NULL = "not yet attached". This is what
            // `thread::migrate_threads` looks for, further down.
            ("thread_id", "INTEGER"),
            // R4: recipients arrive NULL on existing rows — the send
            // backfill (D2) populates them, sync now writes them on
            // every new message.
            ("to_addrs", "TEXT"),
            ("cc_addrs", "TEXT"),
            // PLAN-AUDIT-V2 E5: the envelope's Reply-To. Field STOP 2
            // (2026-09-02): the column lived only in the CREATE
            // TABLE — "no column named reply_to" on every watcher
            // pass over a database from before wave 2. NULL on
            // existing rows: the poll writes it on every new or
            // resynced message.
            ("reply_to", "TEXT"),
        ],
    )?;
    add_missing_columns(
        conn,
        "drafts",
        &[
            ("remote_uid", "INTEGER"),
            ("pushed_epoch", "INTEGER"),
            // Cc/Bcc of a draft — empty on existing rows
            // (PLAN-RETOURS-2).
            ("cc_raw", "TEXT NOT NULL DEFAULT ''"),
            ("bcc_raw", "TEXT NOT NULL DEFAULT ''"),
            // Rich body — NULL on existing rows, plain-text path
            // intact (PLAN-COMPOSITION-HTML).
            ("body_html", "TEXT"),
        ],
    )?;
    // Cc/Bcc of the send log — empty on existing rows
    // (PLAN-RETOURS-2).
    add_missing_columns(
        conn,
        "outbox",
        &[
            ("cc_addrs", "TEXT NOT NULL DEFAULT ''"),
            ("bcc_addrs", "TEXT NOT NULL DEFAULT ''"),
            // Rich body — NULL on existing rows
            // (PLAN-COMPOSITION-HTML).
            ("body_html", "TEXT"),
        ],
    )?;
    // Bodies already in the database are worth 0: they predate
    // attachments, and the backfill will need to reread them once.
    add_missing_columns(conn, "bodies", &[("scanned", "INTEGER NOT NULL DEFAULT 0")])?;
    // Echo recipients — NULL on existing rows (PLAN-RETOURS-5).
    add_missing_columns(conn, "echos", &[("to_addrs", "TEXT")])?;
    // "Important" and delayed sending (PLAN-RETOURS-6): existing rows
    // are neither flagged nor scheduled.
    add_missing_columns(
        conn,
        "drafts",
        &[("important", "INTEGER NOT NULL DEFAULT 0")],
    )?;
    add_missing_columns(
        conn,
        "outbox",
        &[
            ("important", "INTEGER NOT NULL DEFAULT 0"),
            ("send_at_epoch", "INTEGER"),
        ],
    )?;
    // iTIP reply (PLAN-INVITATIONS) — NULL on existing rows,
    // historical send path unchanged.
    add_missing_columns(conn, "outbox", &[("ics_reply", "TEXT")])?;
    // The cross-cancellation link (field R6) — databases born during
    // the job have the table without the column.
    add_missing_columns(
        conn,
        "invitations",
        &[("annule", "INTEGER NOT NULL DEFAULT 0")],
    )?;
    // The list preview (rewrite screen 02) is computed at the WRITE
    // of the body; earlier bodies backfill it IN BATCHES
    // (`preview_catchup`, called by the shell as polling proceeds) —
    // never on the opening path nor while scrolling. The partial
    // index makes the "any stragglers?" probe free once the pass is
    // closed out.
    add_missing_columns(conn, "bodies", &[("preview", "TEXT")])?;
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_bodies_apercu_manquant
             ON bodies(mailbox_id, uid) WHERE preview IS NULL;",
    )?;
    // The envelopes date index gains `uid` (see the SCHEMA comment).
    // `CREATE INDEX IF NOT EXISTS` is NOT enough: on an existing
    // database the index already carries this name, the creation is
    // a silent no-op and the defect would survive. So its DEFINITION
    // is read and it is rebuilt if it lacks the column — same pattern
    // as the `recipients` probe of the search index.
    //
    // No freeze: the rebuild only reads `envelopes` (47 MB in the
    // field), never the bodies — 0.332 s measured on the CE's
    // database, versus the 18 s an index on `bodies` would have cost.
    // That is the whole difference between an acceptable silent
    // migration and the 2026-08-17 freeze.
    //
    // The reread and the rebuild live in ONE transaction, and this is
    // not caution for its own sake (fresh-eyes review of 2026-08-26):
    // `connect_accounts` calls `Store::open` DIRECTLY, outside the
    // commands' global lock (commands.rs), so two `migrate()` calls
    // really do run in parallel at startup. Without a transaction,
    // both would read the two-column index before either writes, and
    // rebuild it each in turn: ~3.5 s of freeze instead of 1.77 s.
    // `BEGIN IMMEDIATE` takes the write lock as soon as it reads —
    // the second one to arrive rereads AFTER the first, finds `uid`,
    // and does nothing.
    // DOUBLE CHECK, and the first check matters as much as the
    // second: `migrate()` runs on EVERY `Store::open`, so dozens of
    // times per startup. A bare read of `sqlite_master` takes no
    // lock; opening a write transaction just to check would cost the
    // write lock on every command.
    rebuild_index_if_old(
        conn,
        "idx_envelopes_date",
        "uid",
        "CREATE INDEX idx_envelopes_date ON envelopes(mailbox_id, date_epoch DESC, uid);",
    )?;
    // The full-messages exclusion probe (nav, Archive category on
    // Gmail) looks up by message_id: without this index, every row
    // of "All messages" would pay for a table scan.
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_envelopes_message
             ON envelopes(message_id) WHERE message_id IS NOT NULL;",
    )?;
    // Repair of previews extracted by the first decoder, which let
    // numeric entities (&#233;) and named ones (&eacute;, &zwnj;…)
    // slip through — a defect seen in the field. Setting back to NULL
    // is enough: the batch backfill recomputes them with the full
    // decoder, off the opening path. The criterion is THE decoder's
    // own scanner (not an approximate SQL pattern). ONE single pass,
    // held by a marker: a double-encoded body ("&amp;gt;")
    // legitimately produces "&gt;" in the new preview — without the
    // marker, the repair would reset it to NULL on every open, for
    // nothing.
    conn.execute_batch("CREATE TABLE IF NOT EXISTS reparations (nom TEXT PRIMARY KEY);")?;
    let already_done: bool = conn
        .prepare("SELECT 1 FROM reparations WHERE nom = 'apercus-entites'")?
        .exists([])?;
    if !already_done {
        let mut stmt = conn.prepare(
            "SELECT mailbox_id, uid, preview FROM bodies
                 WHERE preview IS NOT NULL AND preview LIKE '%&%'",
        )?;
        let polluted: Vec<(i64, u32)> = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, u32>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .filter_map(Result::ok)
            .filter(|(_, _, p)| crate::body::contains_residual_entity(p))
            .map(|(m, u, _)| (m, u))
            .collect();
        drop(stmt);
        for (mailbox_id, uid) in polluted {
            conn.execute(
                "UPDATE bodies SET preview = NULL WHERE mailbox_id = ?1 AND uid = ?2",
                params![mailbox_id, uid],
            )?;
        }
        conn.execute_batch("INSERT INTO reparations (nom) VALUES ('apercus-entites');")?;
    }
    // Repair of bodies mangled during decoding — a defect seen in the
    // field (25 bodies in the measurement database). Two causes,
    // fixed on the mail-imap side: multi-byte charsets (gb2312…)
    // required the `full_encoding` feature of mail-parser, and a
    // missing charset fell back to UTF-8 with replacement instead of
    // the actual windows-1252. Deleting the row is enough: the
    // backfill (`bodies_to_backfill`) redownloads any message without
    // a body, and `save_body` redoes the preview, the search index
    // and the attachments along the way. Genuine U+FFFD characters
    // (sent as such) will come back identical — that's a pointless
    // redownload, but only ONCE, held by the marker.
    let already_done: bool = conn
        .prepare("SELECT 1 FROM reparations WHERE nom = 'corps-fffd'")?
        .exists([])?;
    if !already_done {
        conn.execute_batch(
            "DELETE FROM bodies WHERE html LIKE '%' || char(65533) || '%';
             INSERT INTO reparations (nom) VALUES ('corps-fffd');",
        )?;
    }
    // Repair of messages with a calendar part scanned BEFORE
    // PLAN-INVITATIONS. Two reasons, one remedy: (1) the
    // `est_calendrier_inline` filter (mail-imap) changed the
    // numbering of parts — the stored `idx` values counted the
    // calendar part, rereading the bytes no longer counts it:
    // clicking an attachment would silently serve the WRONG file;
    // (2) these messages have no `invitations` row — their card must
    // be born (adoption, invariant §6.7). Deleting both the body AND
    // the attachment rows is enough: the backfill
    // (`bodies_to_backfill`) rereads the message, and
    // `save_body_full` redoes attachments (fresh indices), preview,
    // search index and invitation all at once. ONCE, held by the
    // marker.
    //
    // D-30 (docs/DEBT.md): the `attachments` criterion above misses a
    // LEGACY message whose calendar part mail-parser never classified
    // as an attachment (an exotic `inline` disposition) — it has no
    // matching row there. Widened with a SECOND, independent
    // criterion: the stored BODY itself still carries the raw
    // `BEGIN:VCALENDAR` marker (SQLite's `LIKE` already folds ASCII
    // case, so this reads both `BEGIN:VCALENDAR` and any
    // lowercase/mixed-case variant a server might send). One bounded
    // pass, at this SAME adoption moment — never a per-poll cost.
    let already_done: bool = conn
        .prepare("SELECT 1 FROM reparations WHERE nom = 'pieces-calendrier'")?
        .exists([])?;
    if !already_done {
        conn.execute_batch(
            "CREATE TEMP TABLE reparation_calendrier AS
                 SELECT DISTINCT mailbox_id, uid FROM attachments
                 WHERE mime IN ('text/calendar', 'application/ics')
                    OR LOWER(name) LIKE '%.ics'
             UNION
                 SELECT DISTINCT mailbox_id, uid FROM bodies
                 WHERE html LIKE '%BEGIN:VCALENDAR%';
             DELETE FROM bodies WHERE (mailbox_id, uid) IN
                 (SELECT mailbox_id, uid FROM reparation_calendrier);
             DELETE FROM attachments WHERE (mailbox_id, uid) IN
                 (SELECT mailbox_id, uid FROM reparation_calendrier);
             DROP TABLE reparation_calendrier;
             INSERT INTO reparations (nom) VALUES ('pieces-calendrier');",
        )?;
    }
    // R2 (PLAN-RETOURS-MAIL): envelopes synced BEFORE the fix carry
    // the backslash-escapes of IMAP `quoted-string`s that
    // `imap-proto` leaves in the content (subject `Test \"Envoyés\"`,
    // sender name, address). The new decoding strips them at sync
    // time, but existing rows stay tainted: repaired ONCE. The
    // stored content is already RFC 2047-decoded; only the IMAP
    // escape layer remains, so un-escaping the stored value is
    // equivalent to the new decoding (an encoded-word carries neither
    // `"` nor `\`). The FTS index does not need to move: its
    // tokenizer already discards the backslash, search gave the same
    // results. char(92) = `\`.
    let already_done: bool = conn
        .prepare("SELECT 1 FROM reparations WHERE nom = 'objets-escapes'")?
        .exists([])?;
    if !already_done {
        let mut stmt = conn.prepare(
            "SELECT mailbox_id, uid, subject, sender, sender_address FROM envelopes
                 WHERE instr(subject, char(92)) > 0
                    OR instr(sender, char(92)) > 0
                    OR instr(sender_address, char(92)) > 0",
        )?;
        #[allow(clippy::type_complexity)]
        let tainted: Vec<(i64, u32, Option<String>, Option<String>, Option<String>)> = stmt
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })?
            .filter_map(Result::ok)
            .collect();
        drop(stmt);
        for (mailbox_id, uid, subject, sender, sender_address) in tainted {
            let clean =
                |v: Option<String>| v.map(|s| crate::unescape_imap_quoted_str(&s).into_owned());
            conn.execute(
                "UPDATE envelopes SET subject = ?3, sender = ?4, sender_address = ?5
                     WHERE mailbox_id = ?1 AND uid = ?2",
                params![
                    mailbox_id,
                    uid,
                    clean(subject),
                    clean(sender),
                    clean(sender_address),
                ],
            )?;
        }
        conn.execute_batch("INSERT INTO reparations (nom) VALUES ('objets-escapes');")?;
    }
    add_missing_columns(
        conn,
        "accounts",
        &[
            ("imap_host", "TEXT"),
            ("imap_port", "INTEGER"),
            ("smtp_host", "TEXT"),
            ("smtp_port", "INTEGER"),
            ("username", "TEXT"),
        ],
    )?;
    search::migrate_search(conn, on_progress)?;
    // The index comes AFTER `add_missing_columns`, not in `SCHEMA`:
    // on a legacy database, `CREATE TABLE IF NOT EXISTS envelopes`
    // does nothing and the `thread_id` column does not yet exist at
    // the moment the schema runs. Two migration tests proved it.
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_envelopes_thread
             ON envelopes(thread_id, date_epoch DESC);",
    )?;
    // The NORMALIZED sender address, as a generated column (Organized
    // mode E2, spike S2-bis): SQLite only uses an EXPRESSION index
    // against a literal — in a join (`= r.address`), it scans (2.3 s
    // measured at 200k). The VIRTUAL column stores nothing (ALTER
    // 14 ms); the real index (188 ms at 200k, once) makes SEARCH out
    // of every sender probe of routing and the Screener. Same
    // expression as `fil_route_sql` — known divergence with
    // `images_address` (Rust) on non-ASCII, assumed E1 limit: a real
    // address is ASCII.
    add_missing_columns(
        conn,
        "envelopes",
        &[(
            "sender_norm",
            "TEXT GENERATED ALWAYS AS (lower(trim(sender_address))) VIRTUAL",
        )],
    )?;
    // Three columns (PLAN-AUDIT-V2 E4): the Cleanup aggregate is
    // COVERED — sender, date, mailbox — without reading a single
    // table row; sender probes (Screener, storing a verdict) are
    // still served by its prefix. One fleet database carried the
    // two-column index: rebuilt, same pattern as the date index.
    let creation =
        format!("CREATE INDEX {SENDERS_INDEX} ON envelopes(sender_norm, date_epoch, mailbox_id);");
    conn.execute_batch(&creation.replace("CREATE INDEX", "CREATE INDEX IF NOT EXISTS"))?;
    rebuild_index_if_old(conn, SENDERS_INDEX, "mailbox_id", &creation)?;
    // The thread retention flag (E2, S2-bis verdict: V4 — maintained
    // by `thread::refresh` like `size`/`unseen`, served by the mirror
    // partial index). On a legacy database, `threads` already exists
    // without the column — and its partial index, created by
    // `thread::SCHEMA` AFTER this point, would fail without it: this
    // is the documented `drop_if_outdated` trap. A fresh database
    // does not have the table yet: the thread schema creates it
    // complete.
    // E4: the Organized Inbox index gains the SECTIONS in its key —
    // an E2 index (without the `unseen` expression) would no longer
    // carry the sort and every page would pay for a materialized sort
    // (S1: 548 ms). Same pattern as the idx_envelopes_date rebuild:
    // the name is not enough, the DEFINITION is read. The thread
    // schema (applied afterward) recreates the new shape.
    let organized_sql: Option<String> = conn
        .query_row(
            "SELECT sql FROM sqlite_master
              WHERE type = 'index' AND name = 'idx_threads_date_organise'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if organized_sql.is_some_and(|sql| !sql.contains("unseen")) {
        conn.execute_batch("DROP INDEX idx_threads_date_organise;")?;
    }
    let thread_columns = table_columns(conn, "threads")?;
    if thread_columns.contains("id") && !thread_columns.contains("organise_hors") {
        add_missing_columns(
            conn,
            "threads",
            &[("organise_hors", "INTEGER NOT NULL DEFAULT 0")],
        )?;
        // ONE-TIME backfill for a database from BEFORE E2 where the
        // mode has already been used (E1 field finding: the epoch
        // may have been recorded and unknowns may have arrived
        // BEFORE this update — without a backfill they would pass
        // the desk forever, silently). First the pending state (the
        // definition of arrival, replayed on the stock: 21 ms
        // measured at 200k), then the flags of affected threads,
        // through THE shared fragment — never a copy of the rule.
        let epoch: Option<i64> = conn
            .query_row(
                "SELECT value FROM prefs WHERE key = 'mode_organise_epoch'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .and_then(|v| v.parse().ok());
        if let Some(epoch) = epoch {
            conn.execute(
                "INSERT OR IGNORE INTO portier_attente (address)
                 SELECT e.sender_norm FROM envelopes e
                   JOIN mailboxes m ON m.id = e.mailbox_id AND m.name = ?2
                  WHERE (e.date_epoch > ?1 OR e.date_epoch IS NULL)
                    AND e.sender_norm IS NOT NULL
                  GROUP BY e.sender_norm
                 HAVING NOT EXISTS (SELECT 1 FROM routage_expediteurs r
                                     WHERE r.address = e.sender_norm)
                    AND NOT EXISTS (SELECT 1 FROM envelopes v
                                     WHERE v.sender_norm = e.sender_norm
                                       AND v.date_epoch <= ?1)
                    AND NOT EXISTS (SELECT 1 FROM accounts a
                                     WHERE lower(trim(a.email)) = e.sender_norm)",
                params![epoch, thread::RECEIVED_MAILBOX],
            )?;
        }
        conn.execute(
            &format!(
                "UPDATE threads SET organise_hors = {}
                  WHERE id IN (
                    SELECT DISTINCT te.thread_id FROM envelopes te
                     WHERE te.thread_id IS NOT NULL
                       AND (EXISTS (SELECT 1 FROM routage_expediteurs r
                                     WHERE r.address = te.sender_norm)
                            OR EXISTS (SELECT 1 FROM portier_attente pa
                                        WHERE pa.address = te.sender_norm)))",
                organized_off_sql("threads.id")
            ),
            [],
        )?;
    }
    // Thread adoption does NOT live here: it belongs to the
    // transactional unit of `init_with`, to be rewindable (§8). It
    // comes after this module — the column and the index must exist
    // before adopting legacy messages.
    Ok(())
}

/// Phase 2 → 3 switchover: the constraints of three tables change
/// (UNIQUE and per-account keys) — SQLite requires a rebuild.
/// Existing data is adopted by a "pending" account (empty email) that
/// the first connection will claim: in practice, the same Gmail
/// account as before the update. Zero loss, proven by test.
fn migrate_multi_account(conn: &Connection) -> Result<(), Error> {
    if table_columns(conn, "mailboxes")?.contains("account_id") {
        return Ok(());
    }
    conn.execute_batch(
        "PRAGMA foreign_keys = OFF;
         BEGIN;
         INSERT INTO accounts (id, email, provider) VALUES (1, '', 'gmail');

         CREATE TABLE mailboxes_v3 (
             id             INTEGER PRIMARY KEY,
             account_id     INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
             name           TEXT NOT NULL,
             uid_validity   INTEGER NOT NULL,
             last_uid       INTEGER NOT NULL DEFAULT 0,
             highest_modseq INTEGER,
             UNIQUE (account_id, name)
         );
         INSERT INTO mailboxes_v3 (id, account_id, name, uid_validity, last_uid, highest_modseq)
             SELECT id, 1, name, uid_validity, last_uid, highest_modseq FROM mailboxes;
         DROP TABLE mailboxes;
         ALTER TABLE mailboxes_v3 RENAME TO mailboxes;

         CREATE TABLE drafts_remote_v3 (
             account_id   INTEGER PRIMARY KEY,
             uid_validity INTEGER NOT NULL
         );
         INSERT INTO drafts_remote_v3 (account_id, uid_validity)
             SELECT 1, uid_validity FROM drafts_remote;
         DROP TABLE drafts_remote;
         ALTER TABLE drafts_remote_v3 RENAME TO drafts_remote;

         CREATE TABLE draft_tombstones_v3 (
             account_id INTEGER NOT NULL,
             remote_uid INTEGER NOT NULL,
             PRIMARY KEY (account_id, remote_uid)
         );
         INSERT INTO draft_tombstones_v3 (account_id, remote_uid)
             SELECT 1, remote_uid FROM draft_tombstones;
         DROP TABLE draft_tombstones;
         ALTER TABLE draft_tombstones_v3 RENAME TO draft_tombstones;

         COMMIT;
         PRAGMA foreign_keys = ON;",
    )?;
    Ok(())
}

/// The path of a connection to a FILE — `None` for an in-memory
/// database (SQLite answers an empty name), which never registers
/// itself.
fn file_key(conn: &Connection) -> Option<std::path::PathBuf> {
    conn.path()
        .filter(|path| !path.is_empty())
        .map(std::path::PathBuf::from)
}

/// The registry of paths whose full initialization has SUCCEEDED in
/// this process (PLAN-AUDIT-V2 E1). A poisoned lock is recovered:
/// losing the registry would replay the migrations, never skip them.
struct InitializedRegistry(std::sync::Mutex<HashSet<std::path::PathBuf>>);

impl InitializedRegistry {
    fn contains(&self, key: &std::path::Path) -> bool {
        self.lock().contains(key)
    }

    fn insert(&self, key: std::path::PathBuf) {
        self.lock().insert(key);
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashSet<std::path::PathBuf>> {
        match self.0.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

fn initialized_registry() -> &'static InitializedRegistry {
    static REGISTRY: std::sync::OnceLock<InitializedRegistry> = std::sync::OnceLock::new();
    REGISTRY.get_or_init(|| InitializedRegistry(std::sync::Mutex::new(HashSet::new())))
}

/// Rebuilds an index whose definition in the database does not yet
/// carry `marker` (a column added after the fact). DOUBLE CHECK, and
/// the first check matters as much as the second: a bare read of
/// `sqlite_master` takes no lock; then, under `BEGIN IMMEDIATE`, a
/// reread — two `migrate()` calls can run in parallel at startup
/// (`connect_accounts` opens outside the commands' lock): the second
/// one to arrive rereads AFTER the first, finds the marker, and does
/// nothing.
fn rebuild_index_if_old(
    conn: &Connection,
    name: &str,
    marker: &str,
    creation: &str,
) -> Result<(), Error> {
    let definition = |conn: &Connection| -> Result<Option<String>, Error> {
        Ok(conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'index' AND name = ?1",
                [name],
                |row| row.get(0),
            )
            .optional()?)
    };
    let outdated = |sql: Option<String>| sql.is_some_and(|sql| !sql.contains(marker));
    if !outdated(definition(conn)?) {
        return Ok(());
    }
    conn.execute_batch("BEGIN IMMEDIATE")?;
    let work = (|| -> Result<(), Error> {
        if outdated(definition(conn)?) {
            conn.execute_batch(&format!("DROP INDEX {name}; {creation}"))?;
        }
        Ok(())
    })();
    match work {
        Ok(()) => conn.execute_batch("COMMIT")?,
        Err(err) => {
            // A rollback failure would teach nothing more than the
            // original error — same choice as in the thread unit.
            let _ = conn.execute_batch("ROLLBACK");
            return Err(err);
        }
    }
    Ok(())
}

pub(super) fn table_columns(conn: &Connection, table: &str) -> Result<HashSet<String>, Error> {
    // `table_xinfo`, not `table_info`: the latter HIDES generated
    // columns (`sender_norm`) — the existence probe would recreate
    // them on every reopen, "duplicate column name" (proven red at
    // E2).
    let mut stmt = conn.prepare(&format!("PRAGMA table_xinfo({table})"))?;
    let columns = stmt
        .query_map([], |row| row.get(1))?
        .collect::<Result<_, _>>()?;
    Ok(columns)
}

fn add_missing_columns(
    conn: &Connection,
    table: &str,
    columns: &[(&str, &str)],
) -> Result<(), Error> {
    let existing = table_columns(conn, table)?;
    for (column, ddl) in columns {
        if !existing.contains(*column) {
            conn.execute(
                &format!("ALTER TABLE {table} ADD COLUMN {column} {ddl}"),
                [],
            )?;
        }
    }
    Ok(())
}

impl Store {
    /// Is a legacy database adoption waiting here? Probed in
    /// **read-only** mode: nothing is triggered, nothing is created —
    /// this is what lets the desktop show the migration screen BEFORE
    /// the first real opening, the one that will pay for the pass.
    ///
    /// Returns the number of messages concerned (`None` = nothing to
    /// do). It is an order of magnitude for the waiting screen, not the
    /// denominator of progress: that one comes from
    /// [`Store::open_with_progress`], the only one that knows the exact
    /// scope.
    pub fn pending_adoption(path: &Path) -> Result<Option<u64>, Error> {
        if !path.exists() {
            // First install: nothing legacy, and opening would create
            // the file — a probe leaves no trace.
            return Ok(None);
        }
        let conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        // Two distinct passes may claim the screen, independently:
        // thread adoption (a database from before ADR 0008) AND
        // rebuilding the search index (FTS schema from before the
        // `recipients` column). The second touches databases that are
        // ALREADY up to date on the thread side — without this
        // detection, it would freeze startup silently, outside any
        // screen (field finding 2026-08-17).
        let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        let threads_pending = version < thread::THREADING_VERSION;
        let search_pending = {
            let fts_sql: Option<String> = conn
                .query_row(
                    "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'search_fts'",
                    [],
                    |row| row.get(0),
                )
                .optional()?;
            fts_sql
                .as_deref()
                .is_some_and(|sql| !sql.contains("recipients"))
        };
        if !threads_pending && !search_pending {
            return Ok(None);
        }
        // A database from before threads may not have the table: the
        // direct COUNT would fail, and the probe must answer, not
        // explain.
        let has_envelopes: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'envelopes'",
            [],
            |row| row.get(0),
        )?;
        if has_envelopes == 0 {
            return Ok(None);
        }
        // Rebuilding the index scans ALL envelopes; thread adoption,
        // only the grouping scope (ADR 0010: INBOX + Sent, well below
        // the total — "256,312" for a pass that reattaches 7,500 would
        // not name what it says). The widest pending pass is announced;
        // it is only an order of magnitude, the real denominator comes
        // from `open_with_progress`.
        let messages: i64 = if search_pending {
            conn.query_row("SELECT COUNT(*) FROM envelopes", [], |row| row.get(0))?
        } else if table_columns(&conn, "mailboxes")?.contains("threaded") {
            conn.query_row(
                "SELECT COUNT(*) FROM envelopes e
                 JOIN mailboxes m ON m.id = e.mailbox_id
                 WHERE m.threaded = 1",
                [],
                |row| row.get(0),
            )?
        } else {
            conn.query_row("SELECT COUNT(*) FROM envelopes", [], |row| row.get(0))?
        };
        if messages == 0 {
            Ok(None)
        } else {
            Ok(Some(messages as u64))
        }
    }

    pub(super) fn init(conn: Connection) -> Result<Self, Error> {
        Self::init_with(conn, &mut |_| ControlFlow::Continue(()))
    }

    /// Forgets initialization for ONE path — for tests that REWIND a
    /// database by hand between two openings (the fixture of a
    /// pre-existing database), which the single-instance rule forbids
    /// in production. One path, never the whole registry: tests run in
    /// parallel, and clearing the registry out from under another test
    /// would make it replay a schema it is precisely proving it does
    /// not replay.
    #[cfg(test)]
    pub(crate) fn forget_initialization(path: &Path) {
        // The SAME key as the registry: the one SQLite gives the file.
        if let Some(key) = Connection::open(path).ok().and_then(|conn| file_key(&conn)) {
            initialized_registry().lock().remove(&key);
        }
    }

    pub(super) fn init_with(
        conn: Connection,
        on_progress: &mut dyn FnMut(AdoptionProgress) -> ControlFlow<()>,
    ) -> Result<Self, Error> {
        // Several commands each open their own connection: wait rather
        // than fail with SQLITE_BUSY on a concurrent write. 30 s and
        // not 5 (field finding 2026-08-15): under heavy machine load, a
        // sync write batch can hold the lock beyond 5 s — a UI gesture
        // (`delete_draft` on an emptied draft) would then die with BUSY
        // and its failure, silenced by the UI of that era, left a ghost
        // in the folder. In WAL, reads never wait; only a write behind
        // a write waits — late beats dead.
        conn.busy_timeout(std::time::Duration::from_secs(30))?;
        // WAL (ADR 0011): a read no longer ever blocks a write, nor the
        // reverse. Rollback mode held up while writes lasted a few
        // seconds; full sync (ADR 0010) stretches them into minutes,
        // and the FIRST field trial produced "database is locked" —
        // the progress probe and the list, by reading, made the header
        // pass's busy_timeout expire.
        //
        // `query_row` and not `pragma_update`: this PRAGMA answers with
        // one row (the effective mode). An in-memory database answers
        // "memory" — that is not a failure, tests live in it just fine
        // without WAL. The mode is PERSISTENT: written once in the file
        // header, reread on every open, legacy databases included.
        conn.query_row("PRAGMA journal_mode = wal", [], |row| {
            row.get::<_, String>(0)
        })?;
        // PLAN-AUDIT-V2 E1 — the fast door: each shell command opens
        // ITS OWN connection (103 call sites); replaying the schema
        // here, some twenty `table_xinfo` calls and the migrations,
        // cost 36 ms on 200k envelopes ON EVERY COMMAND. Once
        // initialization has SUCCEEDED once on a path in this process,
        // subsequent opens only do the two settings above. Safe because
        // single-instance (PLAN-AUDIT-V1 E1) guarantees no other
        // process migrates the database in the meantime, and
        // registration only happens after the adoption's COMMIT (a
        // cancellation, a failure: nothing registered, the whole pass
        // replays). An in-memory database has no path: never
        // registered.
        // Foreign keys are a PER-CONNECTION setting: `SCHEMA` turns
        // them on up front, and the fast door does not replay `SCHEMA`.
        // The wave-2 review found lost cascades there; the test meant
        // to prove it stayed GREEN without this line — rusqlite's
        // `bundled` compiles SQLite with `SQLITE_DEFAULT_FOREIGN_KEYS=1`.
        // The line stays, ahead of the fast door: a belt that does not
        // depend on a compile flag (the test keeps it honest).
        conn.execute_batch("PRAGMA foreign_keys = ON")?;
        conn.execute_batch("PRAGMA foreign_keys = ON")?;
        let key = file_key(&conn);
        if let Some(key) = &key
            && initialized_registry().contains(key)
        {
            return Ok(Self(conn));
        }
        conn.execute_batch(SCHEMA)?;
        // Light migrations first: columns, indexes. Rebuilding the
        // search index lives HERE but is NOT light on a database that
        // already has data (rereading the bodies): it is therefore
        // visible and interruptible via `on_progress`, and
        // `pending_adoption` has it preceded by a screen (otherwise, a
        // silent startup freeze — field finding 2026-08-17). Thread
        // adoption, just below, needs the columns these migrations add
        // (`thread_id`, `in_reply_to`, `refs`).
        migrate(&conn, on_progress)?;
        // ——— The unity of threads, as one piece (handover §8). ———
        // From the conditional DROP to `user_version`, everything lives
        // in ONE transaction: cancelling during adoption rewinds
        // EVERYTHING — a partial adoption persisted would be a
        // half-empty mailbox, the list starting from `threads`. The
        // BEGIN is DEFERRED: on an up-to-date database nothing writes,
        // the transaction stays a reader and never meets the writer of
        // a long sync (ADR 0011).
        conn.execute_batch("BEGIN")?;
        let unit = (|| {
            // BEFORE the thread schema, never after: if the grouping
            // rule has changed, both tables must DISAPPEAR so that the
            // `CREATE TABLE IF NOT EXISTS` just below recreates them in
            // their new shape. Without this, opening fails — see
            // `thread::drop_if_outdated`.
            thread::drop_if_outdated(&conn)?;
            conn.execute_batch(thread::SCHEMA)?;
            thread::migrate_threads_with(&conn, on_progress)
        })();
        let announced = match unit {
            Ok(announced) => {
                conn.execute_batch("COMMIT")?;
                announced
            }
            Err(err) => {
                // A rollback failure would teach nothing more than the
                // original error, which is the one that must be
                // surfaced — including a deliberate cancellation.
                let _ = conn.execute_batch("ROLLBACK");
                return Err(err);
            }
        };
        if let Some(total) = announced {
            // "Done" is only said once the pass is COMMITTED — never
            // before (a signal must be observable, handover §9). Too
            // late to cancel: the answer is ignored.
            let _ = on_progress(AdoptionProgress { done: total, total });
        }
        let store = Self(conn);
        // The contacts directory backfills ONCE from existing data
        // (PLAN-RETOURS-5): set-based, marked in `prefs` — on an
        // up-to-date database, one SELECT and nothing else.
        store.backfill_contacts()?;
        if let Some(key) = key {
            initialized_registry().insert(key);
        }
        Ok(store)
    }
}

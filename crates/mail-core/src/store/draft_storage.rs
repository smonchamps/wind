use super::*;

pub(super) fn migrate_files(
    conn: &Connection,
    on_progress: &mut dyn FnMut(AdoptionProgress) -> ControlFlow<()>,
) -> Result<(), Error> {
    if !migrations::table_columns(conn, "draft_attachments")?.contains("bytes") {
        return ensure_edit_schema(conn);
    }
    let tx = rusqlite::Transaction::new_unchecked(conn, rusqlite::TransactionBehavior::Immediate)?;
    if !migrations::table_columns(&tx, "draft_attachments")?.contains("bytes") {
        ensure_edit_schema(&tx)?;
        tx.commit()?;
        return Ok(());
    }
    let files: u64 = tx.query_row("SELECT COUNT(*) FROM draft_attachments", [], |r| {
        r.get::<_, i64>(0).map(crate::sql_read_u64)
    })?;
    let total = files + 1;
    if files > 0 && on_progress(AdoptionProgress { done: 0, total }).is_break() {
        return Err(Error::Interrupted);
    }
    tx.execute_batch(
        "CREATE TABLE draft_blobs (id INTEGER PRIMARY KEY AUTOINCREMENT, bytes BLOB NOT NULL);
         CREATE TABLE draft_files_new (
           id INTEGER PRIMARY KEY AUTOINCREMENT,
           draft_id INTEGER NOT NULL REFERENCES drafts(id) ON DELETE CASCADE,
           name TEXT NOT NULL, mime TEXT NOT NULL, size INTEGER NOT NULL,
           blob_id INTEGER NOT NULL REFERENCES draft_blobs(id));",
    )?;
    // One payload at a time: cancellation is checked between files and no BLOB
    // crosses into Rust memory. The original table survives until commit.
    let mut previous: Option<i64> = None;
    for done in 0..files {
        let id: i64 = tx.query_row(
            "SELECT id FROM draft_attachments WHERE ?1 IS NULL OR id > ?1 ORDER BY id LIMIT 1",
            [previous],
            |r| r.get(0),
        )?;
        tx.execute("INSERT INTO draft_blobs (id, bytes) SELECT id, bytes FROM draft_attachments WHERE id = ?1", [id])?;
        tx.execute("INSERT INTO draft_files_new SELECT id, draft_id, name, mime, size, id FROM draft_attachments WHERE id = ?1", [id])?;
        previous = Some(id);
        if on_progress(AdoptionProgress {
            done: done + 1,
            total,
        })
        .is_break()
        {
            return Err(Error::Interrupted);
        }
    }
    if !migrations::table_columns(conn, "drafts")?.contains("incarnation") {
        tx.execute_batch(
            "ALTER TABLE drafts ADD COLUMN incarnation TEXT NOT NULL DEFAULT '';
                          UPDATE drafts SET incarnation = lower(hex(randomblob(16)));",
        )?;
    }
    tx.execute_batch(
        "DROP TABLE draft_attachments;
         ALTER TABLE draft_files_new RENAME TO draft_attachments;
         CREATE INDEX idx_draft_attachments_draft ON draft_attachments(draft_id);
         CREATE INDEX idx_draft_attachments_blob ON draft_attachments(blob_id);
         CREATE UNIQUE INDEX IF NOT EXISTS idx_draft_incarnation ON drafts(incarnation) WHERE incarnation != '';
         CREATE TRIGGER IF NOT EXISTS draft_incarnation AFTER INSERT ON drafts WHEN NEW.incarnation = ''
         BEGIN UPDATE drafts SET incarnation = lower(hex(randomblob(16))) WHERE id = NEW.id; END;
         CREATE TABLE draft_edit (
           slot INTEGER PRIMARY KEY CHECK(slot = 1), token TEXT NOT NULL UNIQUE,
           account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
           source_id INTEGER, incarnation TEXT, base_epoch INTEGER,
           dirty INTEGER NOT NULL DEFAULT 0, files_dirty INTEGER NOT NULL DEFAULT 0,
           to_raw TEXT NOT NULL DEFAULT '', cc_raw TEXT NOT NULL DEFAULT '', bcc_raw TEXT NOT NULL DEFAULT '',
           subject TEXT NOT NULL DEFAULT '', body TEXT NOT NULL DEFAULT '', body_html TEXT,
           reply_to_uid INTEGER, reply_to_mailbox TEXT, important INTEGER NOT NULL DEFAULT 0);
         CREATE TABLE draft_edit_files (
           id INTEGER PRIMARY KEY AUTOINCREMENT,
           slot INTEGER NOT NULL REFERENCES draft_edit(slot) ON DELETE CASCADE,
           name TEXT NOT NULL, mime TEXT NOT NULL, size INTEGER NOT NULL,
           blob_id INTEGER NOT NULL REFERENCES draft_blobs(id));
         CREATE INDEX idx_draft_edit_files_blob ON draft_edit_files(blob_id);
         CREATE TRIGGER draft_blobs_immutable BEFORE UPDATE ON draft_blobs
         BEGIN SELECT RAISE(ABORT, 'attachment payloads are immutable'); END;
         CREATE TRIGGER draft_files_collect AFTER DELETE ON draft_attachments
         BEGIN DELETE FROM draft_blobs WHERE id = OLD.blob_id
           AND NOT EXISTS(SELECT 1 FROM draft_attachments WHERE blob_id = OLD.blob_id)
           AND NOT EXISTS(SELECT 1 FROM draft_edit_files WHERE blob_id = OLD.blob_id); END;
         CREATE TRIGGER draft_edit_files_collect AFTER DELETE ON draft_edit_files
         BEGIN DELETE FROM draft_blobs WHERE id = OLD.blob_id
           AND NOT EXISTS(SELECT 1 FROM draft_attachments WHERE blob_id = OLD.blob_id)
           AND NOT EXISTS(SELECT 1 FROM draft_edit_files WHERE blob_id = OLD.blob_id); END;"
    )?;
    ensure_edit_schema(&tx)?;
    tx.commit()?;
    if files > 0 {
        let _ = on_progress(AdoptionProgress { done: total, total });
    }
    Ok(())
}

fn ensure_edit_schema(conn: &Connection) -> Result<(), Error> {
    migrations::add_missing_columns(
        conn,
        "draft_edit",
        &[
            ("reply_in_reply_to", "TEXT"),
            ("reply_references", "TEXT"),
            ("reply_mailbox_id", "INTEGER"),
            ("reply_uid_validity", "INTEGER"),
            ("reply_account_id", "INTEGER"),
        ],
    )?;
    migrations::add_missing_columns(conn, "outbox", &[("edit_token", "TEXT")])?;
    conn.execute_batch("CREATE UNIQUE INDEX IF NOT EXISTS idx_outbox_edit_token ON outbox(edit_token) WHERE edit_token IS NOT NULL;")?;
    Ok(())
}

impl Store {
    /// Conservative free-space estimate for copying and journaling legacy files.
    /// Read-only, so the shell can refuse before starting a migration.
    pub fn draft_file_migration_headroom(path: &Path) -> Result<Option<u64>, Error> {
        if !path.exists() {
            return Ok(None);
        }
        let conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        if !migrations::table_columns(&conn, "draft_attachments")?.contains("bytes") {
            return Ok(None);
        }
        let (files, bytes): (u64, u64) = conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(length(bytes)), 0) FROM draft_attachments",
            [],
            |r| {
                Ok((
                    crate::sql_read_u64(r.get(0)?),
                    crate::sql_read_u64(r.get(1)?),
                ))
            },
        )?;
        Ok((files > 0).then(|| {
            bytes
                .saturating_add(files.saturating_mul(8192))
                .saturating_mul(3)
                .saturating_add(8 * 1024 * 1024)
        }))
    }

    /// Reconstructs legacy attachment rows before a binary downgrade. Consumes
    /// this Store because its current attachment API no longer matches the schema.
    pub fn revert_draft_file_storage(
        self,
        mut on_progress: impl FnMut(AdoptionProgress) -> ControlFlow<()>,
    ) -> Result<(), Error> {
        self.recover_draft_edit()?;
        let tx = self.conn().unchecked_transaction()?;
        let total: u64 = tx.query_row("SELECT COUNT(*) + 1 FROM draft_attachments", [], |r| {
            r.get::<_, i64>(0).map(crate::sql_read_u64)
        })?;
        if on_progress(AdoptionProgress { done: 0, total }).is_break() {
            return Err(Error::Interrupted);
        }
        tx.execute_batch("CREATE TABLE draft_files_legacy (id INTEGER PRIMARY KEY, draft_id INTEGER NOT NULL REFERENCES drafts(id) ON DELETE CASCADE, name TEXT NOT NULL, mime TEXT NOT NULL, size INTEGER NOT NULL, bytes BLOB NOT NULL);")?;
        let mut previous: Option<i64> = None;
        for done in 1..total {
            let id: i64 = tx.query_row(
                "SELECT id FROM draft_attachments WHERE ?1 IS NULL OR id > ?1 ORDER BY id LIMIT 1",
                [previous],
                |r| r.get(0),
            )?;
            tx.execute("INSERT INTO draft_files_legacy SELECT a.id, a.draft_id, a.name, a.mime, a.size, b.bytes FROM draft_attachments a JOIN draft_blobs b ON b.id = a.blob_id WHERE a.id = ?1", [id])?;
            previous = Some(id);
            if on_progress(AdoptionProgress { done, total }).is_break() {
                return Err(Error::Interrupted);
            }
        }
        tx.execute_batch("DROP TRIGGER draft_files_collect; DROP TRIGGER draft_edit_files_collect;
            DROP TABLE draft_edit_files; DROP TABLE draft_edit; DROP TABLE draft_attachments; DROP TABLE draft_blobs;
            ALTER TABLE draft_files_legacy RENAME TO draft_attachments;
            CREATE INDEX idx_draft_attachments_draft ON draft_attachments(draft_id);")?;
        tx.commit()?;
        migrations::invalidate_initialization(self.conn());
        let _ = on_progress(AdoptionProgress { done: total, total });
        Ok(())
    }
}

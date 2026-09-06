//! Durable ownership of the version currently being edited. Files are immutable
//! payload references, independent of a remote mirror's lifetime.
use rusqlite::{OptionalExtension, params};

use crate::{
    DraftAttachmentMeta, DraftContent, DraftSaved, Error, MAX_ATTACHMENTS_BYTES, SavedDraft, Store,
};

pub struct DraftEdit {
    pub draft: Option<SavedDraft>,
    pub attachments: Vec<DraftAttachmentMeta>,
}

struct Snapshot {
    source: Option<i64>,
    dirty: bool,
    files_dirty: bool,
    reply_account: Option<i64>,
    draft: SavedDraft,
}

impl SavedDraft {
    pub fn content(&self) -> DraftContent<'_> {
        DraftContent {
            to_raw: &self.to_raw,
            cc_raw: &self.cc_raw,
            bcc_raw: &self.bcc_raw,
            subject: &self.subject,
            body: &self.body,
            body_html: self.body_html.as_deref(),
            reply_to_uid: self.reply_to_uid,
            reply_to_mailbox: self.reply_to_mailbox.as_deref(),
            important: self.important,
        }
    }
}

impl Store {
    /// Owner of the active snapshot, including one without a persisted source draft.
    pub fn active_draft_edit_account(&self) -> Result<Option<i64>, Error> {
        Ok(self
            .conn()
            .query_row("SELECT account_id FROM draft_edit LIMIT 1", [], |row| {
                row.get(0)
            })
            .optional()?)
    }

    /// Resolve an enqueue acknowledgement before saving the consumed editing session.
    /// Only a still-owned snapshot permits a new attempt when no receipt exists.
    pub fn queued_draft_edit(&self, token: &str, account_id: i64) -> Result<Option<i64>, Error> {
        let queued = self
            .conn()
            .query_row(
                "SELECT id FROM outbox WHERE edit_token = ?1 AND account_id = ?2",
                params![token, account_id],
                |row| row.get(0),
            )
            .optional()?;
        if queued.is_none() && self.edit_snapshot(token)?.draft.account_id != account_id {
            return Err(Error::StaleDraft);
        }
        Ok(queued)
    }

    /// Freeze reply headers while the displayed source identity still matches.
    pub fn set_draft_edit_reply(
        &self,
        token: &str,
        identity: &crate::MailboxIdentity,
        uid: crate::Uid,
    ) -> Result<(), Error> {
        let tx = self.conn().unchecked_transaction()?;
        self.edit_snapshot(token)?;
        self.verify_mailbox_identity(identity)?;
        let envelope = self
            .envelope(identity.account_id, &identity.mailbox, uid)?
            .ok_or(Error::StaleMailbox)?;
        let references = self.references_of(identity.account_id, &identity.mailbox, uid)?;
        tx.execute("UPDATE draft_edit SET reply_in_reply_to = ?1, reply_references = ?2, reply_account_id = ?3, reply_to_mailbox = ?5, reply_to_uid = ?6, reply_mailbox_id = ?7, reply_uid_validity = ?8 WHERE token = ?4", params![envelope.message_id, references, identity.account_id, token, identity.mailbox, uid, identity.mailbox_id, identity.uid_validity])?;
        tx.commit()?;
        Ok(())
    }

    /// Queue the requested content and snapshot files, then consume the exact
    /// edited version in the same transaction. A repeated IPC token is idempotent.
    pub fn enqueue_draft_edit(
        &self,
        token: &str,
        account_id: i64,
        draft: &crate::Draft,
        due: Option<i64>,
    ) -> Result<i64, Error> {
        let tx = self.conn().unchecked_transaction()?;
        if let Some(id) = tx
            .query_row(
                "SELECT id FROM outbox WHERE edit_token = ?1 AND account_id = ?2",
                params![token, account_id],
                |r| r.get::<_, i64>(0),
            )
            .optional()?
        {
            return Ok(id);
        }
        let snapshot = self.edit_snapshot(token)?;
        if snapshot.draft.account_id != account_id {
            return Err(Error::StaleDraft);
        }
        let mut draft = draft.clone();
        draft.in_reply_to = snapshot.draft.thread_headers.in_reply_to.clone();
        draft.references = snapshot.draft.thread_headers.references.clone();
        let id = self.enqueue_outbox(account_id, &draft)?;
        tx.execute("INSERT INTO outbox_attachments (outbox_id, name, mime, size, bytes) SELECT ?1, a.name, a.mime, a.size, b.bytes FROM draft_edit_files a JOIN draft_blobs b ON b.id = a.blob_id WHERE a.slot = 1 ORDER BY a.id", [id])?;
        tx.execute(
            "UPDATE outbox SET send_at_epoch = ?1, edit_token = ?2 WHERE id = ?3",
            params![due, token, id],
        )?;
        tx.execute("INSERT OR IGNORE INTO draft_tombstones (account_id, remote_uid) SELECT account_id, remote_uid FROM drafts WHERE id = ?1 AND incarnation = ?2 AND updated_epoch = ?3 AND remote_uid IS NOT NULL", params![snapshot.source, snapshot.draft.incarnation, snapshot.draft.updated_epoch])?;
        tx.execute(
            "DELETE FROM drafts WHERE id = ?1 AND incarnation = ?2 AND updated_epoch = ?3",
            params![
                snapshot.source,
                snapshot.draft.incarnation,
                snapshot.draft.updated_epoch
            ],
        )?;
        tx.execute("DELETE FROM draft_edit WHERE token = ?1", [token])?;
        tx.commit()?;
        Ok(id)
    }
    pub fn draft_edit(&self, token: &str) -> Result<DraftEdit, Error> {
        let snapshot = self.edit_snapshot(token)?;
        Ok(DraftEdit {
            draft: snapshot.source.map(|_| snapshot.draft),
            attachments: self.draft_edit_attachments(token)?,
        })
    }
    pub fn begin_draft_edit(
        &self,
        token: &str,
        account_id: i64,
        id: Option<i64>,
        incarnation: Option<&str>,
        base_epoch: Option<i64>,
    ) -> Result<DraftEdit, Error> {
        if token.is_empty() || token.len() > 128 {
            return Err(Error::StaleDraft);
        }
        let tx = self.conn().unchecked_transaction()?;
        let active: Option<String> = tx
            .query_row("SELECT token FROM draft_edit", [], |r| r.get(0))
            .optional()?;
        if let Some(active) = active {
            if active != token {
                return Err(Error::DraftAlreadyOpen);
            }
            let snapshot = self.edit_snapshot(token)?;
            if snapshot.draft.account_id != account_id {
                return Err(Error::StaleDraft);
            }
            let attachments = self.draft_edit_attachments(token)?;
            tx.commit()?;
            return Ok(DraftEdit {
                draft: snapshot.source.map(|_| snapshot.draft),
                attachments,
            });
        }
        let draft = match id {
            Some(id) => {
                let draft = self.draft(id)?.ok_or(Error::StaleDraft)?;
                if draft.account_id != account_id
                    || Some(draft.incarnation.as_str()) != incarnation
                    || Some(draft.updated_epoch) != base_epoch
                {
                    return Err(Error::StaleDraft);
                }
                Some(draft)
            }
            None => None,
        };
        tx.execute(
            "INSERT INTO draft_edit (slot, token, account_id) VALUES (1, ?1, ?2)",
            params![token, account_id],
        )?;
        if let Some(draft) = &draft {
            self.write_edit_content(token, account_id, draft.content())?;
            tx.execute("UPDATE draft_edit SET reply_in_reply_to = ?1, reply_references = ?2, reply_account_id = ?3, reply_mailbox_id = ?5, reply_uid_validity = ?6 WHERE token = ?4", params![draft.thread_headers.in_reply_to, draft.thread_headers.references, draft.reply_to_uid.map(|_| account_id), token, draft.reply_identity.as_ref().map(|i| i.mailbox_id), draft.reply_identity.as_ref().map(|i| i.uid_validity)])?;
            tx.execute("UPDATE draft_edit SET source_id = ?1, incarnation = ?2, base_epoch = ?3 WHERE slot = 1", params![draft.id, draft.incarnation, draft.updated_epoch])?;
            tx.execute("INSERT INTO draft_edit_files (slot, name, mime, size, blob_id) SELECT 1, name, mime, size, blob_id FROM draft_attachments WHERE draft_id = ?1 ORDER BY id", [draft.id])?;
        }
        let attachments = self.draft_edit_attachments(token)?;
        tx.commit()?;
        Ok(DraftEdit { draft, attachments })
    }

    pub fn draft_edit_attachments(&self, token: &str) -> Result<Vec<DraftAttachmentMeta>, Error> {
        let snapshot = self.edit_snapshot(token)?;
        Ok(self
            .conn()
            .prepare(
                "SELECT id, name, mime, size FROM draft_edit_files WHERE slot = 1 ORDER BY id",
            )?
            .query_map([], |r| {
                Ok(DraftAttachmentMeta {
                    id: r.get(0)?,
                    draft_id: snapshot.source.unwrap_or(0),
                    name: r.get(1)?,
                    mime: r.get(2)?,
                    size: r.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub fn save_draft_edit(
        &self,
        token: &str,
        account_id: i64,
        content: DraftContent<'_>,
    ) -> Result<Option<DraftSaved>, Error> {
        let tx = self.conn().unchecked_transaction()?;
        let saved = self.save_edit_inner(token, account_id, content)?;
        tx.commit()?;
        Ok(saved)
    }

    fn save_edit_inner(
        &self,
        token: &str,
        account_id: i64,
        content: DraftContent<'_>,
    ) -> Result<Option<DraftSaved>, Error> {
        let snapshot = self.edit_snapshot(token)?;
        let content_changed = content != snapshot.draft.content();
        let reply_current = match snapshot
            .draft
            .reply_identity
            .as_ref()
            .map(|identity| self.verify_mailbox_identity(identity))
        {
            Some(Ok(())) => true,
            Some(Err(Error::StaleMailbox)) | None => false,
            Some(Err(err)) => return Err(err),
        };
        let content = if snapshot
            .reply_account
            .is_some_and(|account| account != account_id || !reply_current)
        {
            DraftContent {
                reply_to_uid: None,
                reply_to_mailbox: None,
                ..content
            }
        } else {
            content
        };
        let reply_identity = content
            .reply_to_uid
            .and(snapshot.draft.reply_identity.as_ref());
        let current = snapshot
            .source
            .map(|id| self.draft(id))
            .transpose()?
            .flatten();
        let same = current.as_ref().is_some_and(|d| {
            d.incarnation == snapshot.draft.incarnation
                && d.updated_epoch == snapshot.draft.updated_epoch
                && d.account_id == snapshot.draft.account_id
        });
        let headers_changed = current
            .as_ref()
            .is_some_and(|d| d.thread_headers != snapshot.draft.thread_headers);
        let changed = content_changed
            || account_id != snapshot.draft.account_id
            || snapshot.files_dirty
            || headers_changed;
        if !same && snapshot.source.is_some() && !changed && !snapshot.dirty {
            return Ok(None); // An untouched obsolete mirror needs no new remote copy.
        }
        let mut saved = self.save_draft_in_transaction(
            account_id,
            if same { snapshot.source } else { None },
            if same {
                Some(snapshot.draft.updated_epoch)
            } else {
                None
            },
            content,
        )?;
        saved.forked = !same && snapshot.source.is_some();
        if same && account_id != snapshot.draft.account_id {
            self.conn().execute("INSERT OR IGNORE INTO draft_tombstones (account_id, remote_uid) SELECT account_id, remote_uid FROM drafts WHERE id = ?1 AND remote_uid IS NOT NULL", [saved.id])?;
            self.conn().execute("UPDATE drafts SET account_id = ?1, remote_uid = NULL, pushed_epoch = NULL, updated_epoch = MAX(updated_epoch, ?2 + 1), reply_to_uid = NULL, reply_to_mailbox = NULL WHERE id = ?3", params![account_id, snapshot.draft.updated_epoch, saved.id])?;
            saved.updated_epoch = saved.updated_epoch.max(snapshot.draft.updated_epoch + 1);
        }
        if !same || headers_changed {
            self.conn().execute(
                "UPDATE drafts SET reply_in_reply_to = ?1, reply_references = ?2 WHERE id = ?3",
                params![
                    snapshot.draft.thread_headers.in_reply_to,
                    snapshot.draft.thread_headers.references,
                    saved.id
                ],
            )?;
            if same && saved.updated_epoch == snapshot.draft.updated_epoch {
                self.conn().execute(
                    "UPDATE drafts SET updated_epoch = updated_epoch + 1 WHERE id = ?1",
                    [saved.id],
                )?;
                saved.updated_epoch += 1;
            }
        }
        if !same || snapshot.files_dirty {
            self.conn().execute(
                "DELETE FROM draft_attachments WHERE draft_id = ?1",
                [saved.id],
            )?;
            self.conn().execute("INSERT INTO draft_attachments (draft_id, name, mime, size, blob_id) SELECT ?1, name, mime, size, blob_id FROM draft_edit_files WHERE slot = 1 ORDER BY id", [saved.id])?;
            if same && saved.updated_epoch == snapshot.draft.updated_epoch {
                self.conn().execute(
                    "UPDATE drafts SET updated_epoch = updated_epoch + 1 WHERE id = ?1",
                    [saved.id],
                )?;
                saved.updated_epoch += 1;
            }
        }
        self.conn().execute(
            "UPDATE drafts SET reply_mailbox_id = ?1, reply_uid_validity = ?2 WHERE id = ?3",
            params![
                reply_identity.map(|i| i.mailbox_id),
                reply_identity.map(|i| i.uid_validity),
                saved.id
            ],
        )?;
        self.write_edit_content(token, account_id, content)?;
        let incarnation: String = self.conn().query_row(
            "SELECT incarnation FROM drafts WHERE id = ?1",
            [saved.id],
            |r| r.get(0),
        )?;
        self.conn().execute("UPDATE draft_edit SET source_id = ?1, incarnation = ?2, base_epoch = ?3, dirty = dirty OR ?4, files_dirty = 0 WHERE token = ?5", params![saved.id, incarnation, saved.updated_epoch, changed, token])?;
        saved.attachments = self.draft_edit_attachments(token)?;
        Ok(Some(saved))
    }

    pub fn add_draft_edit_source_attachment(
        &self,
        token: &str,
        account_id: i64,
        identity: &crate::MailboxIdentity,
        file: &crate::DraftAttachmentFull,
    ) -> Result<DraftSaved, Error> {
        let tx = self.conn().unchecked_transaction()?;
        self.verify_mailbox_identity(identity)?;
        let saved =
            self.add_edit_file_inner(token, account_id, &file.name, &file.mime, &file.bytes)?;
        tx.commit()?;
        Ok(saved)
    }

    pub fn add_draft_edit_attachment(
        &self,
        token: &str,
        account_id: i64,
        name: &str,
        mime: &str,
        bytes: &[u8],
    ) -> Result<DraftSaved, Error> {
        let tx = self.conn().unchecked_transaction()?;
        let saved = self.add_edit_file_inner(token, account_id, name, mime, bytes)?;
        tx.commit()?;
        Ok(saved)
    }

    fn add_edit_file_inner(
        &self,
        token: &str,
        account_id: i64,
        name: &str,
        mime: &str,
        bytes: &[u8],
    ) -> Result<DraftSaved, Error> {
        let snapshot = self.edit_snapshot(token)?;
        let used: u64 = self.conn().query_row(
            "SELECT COALESCE(SUM(size), 0) FROM draft_edit_files WHERE slot = 1",
            [],
            |r| r.get(0),
        )?;
        let remaining = MAX_ATTACHMENTS_BYTES.saturating_sub(used);
        if bytes.len() as u64 > remaining {
            return Err(Error::AttachmentOverBudget {
                name: name.into(),
                size: bytes.len() as u64,
                remaining,
            });
        }
        self.conn()
            .execute("INSERT INTO draft_blobs (bytes) VALUES (?1)", [bytes])?;
        let blob = self.conn().last_insert_rowid();
        self.conn().execute("INSERT INTO draft_edit_files (slot, name, mime, size, blob_id) VALUES (1, ?1, ?2, ?3, ?4)", params![name, mime, bytes.len() as i64, blob])?;
        self.conn().execute(
            "UPDATE draft_edit SET files_dirty = 1 WHERE token = ?1",
            [token],
        )?;
        let saved = self
            .save_edit_inner(token, account_id, snapshot.draft.content())?
            .ok_or(Error::StaleDraft)?;
        Ok(saved)
    }

    pub fn remove_draft_edit_attachment(
        &self,
        token: &str,
        account_id: i64,
        attachment_id: i64,
    ) -> Result<Option<DraftSaved>, Error> {
        let tx = self.conn().unchecked_transaction()?;
        let snapshot = self.edit_snapshot(token)?;
        let removed = tx.execute(
            "DELETE FROM draft_edit_files WHERE slot = 1 AND id = ?1",
            [attachment_id],
        )?;
        if removed > 0 {
            tx.execute(
                "UPDATE draft_edit SET files_dirty = 1 WHERE token = ?1",
                [token],
            )?;
        }
        let saved = self.save_edit_inner(token, account_id, snapshot.draft.content())?;
        tx.commit()?;
        Ok(saved)
    }

    /// Successful close materializes any committed edit whose mirror vanished.
    /// Discard only targets the exact version owned by this session.
    pub fn finish_draft_edit(
        &self,
        token: &str,
        discard: bool,
    ) -> Result<Option<DraftSaved>, Error> {
        let tx = self.conn().unchecked_transaction()?;
        let active: Option<String> = tx
            .query_row("SELECT token FROM draft_edit", [], |r| r.get(0))
            .optional()?;
        if active.is_none() {
            return Ok(None);
        }
        let snapshot = self.edit_snapshot(token)?;
        let mut recovered = None;
        if discard {
            tx.execute("INSERT OR IGNORE INTO draft_tombstones (account_id, remote_uid) SELECT account_id, remote_uid FROM drafts WHERE id = ?1 AND incarnation = ?2 AND updated_epoch = ?3 AND remote_uid IS NOT NULL", params![snapshot.source, snapshot.draft.incarnation, snapshot.draft.updated_epoch])?;
            tx.execute(
                "DELETE FROM drafts WHERE id = ?1 AND incarnation = ?2 AND updated_epoch = ?3",
                params![
                    snapshot.source,
                    snapshot.draft.incarnation,
                    snapshot.draft.updated_epoch
                ],
            )?;
        } else if snapshot.dirty || snapshot.files_dirty {
            recovered =
                self.save_edit_inner(token, snapshot.draft.account_id, snapshot.draft.content())?;
        }
        tx.execute("DELETE FROM draft_edit WHERE token = ?1", [token])?;
        tx.commit()?;
        Ok(recovered)
    }

    /// Call once at process startup, never on each connection opening.
    pub fn recover_draft_edit(&self) -> Result<(), Error> {
        let token: Option<String> = self
            .conn()
            .query_row("SELECT token FROM draft_edit", [], |r| r.get(0))
            .optional()?;
        if let Some(token) = token {
            self.finish_draft_edit(&token, false)?;
        }
        Ok(())
    }

    fn write_edit_content(
        &self,
        token: &str,
        account_id: i64,
        c: DraftContent<'_>,
    ) -> Result<(), Error> {
        self.conn().execute("UPDATE draft_edit SET account_id = ?1, to_raw = ?2, cc_raw = ?3, bcc_raw = ?4, subject = ?5, body = ?6, body_html = ?7, reply_to_uid = ?8, reply_to_mailbox = ?9, important = ?10 WHERE token = ?11",
            params![account_id, c.to_raw, c.cc_raw, c.bcc_raw, c.subject, c.body, c.body_html, c.reply_to_uid, c.reply_to_mailbox, c.important, token])?;
        Ok(())
    }

    fn edit_snapshot(&self, token: &str) -> Result<Snapshot, Error> {
        self.conn().query_row("SELECT source_id, account_id, incarnation, base_epoch, dirty, files_dirty, to_raw, cc_raw, bcc_raw, subject, body, body_html, reply_to_uid, reply_to_mailbox, important, reply_in_reply_to, reply_references, reply_account_id, reply_mailbox_id, reply_uid_validity FROM draft_edit WHERE token = ?1", [token], |r| {
            let source: Option<i64> = r.get(0)?;
            let reply_account: Option<i64> = r.get(17)?;
            let reply_identity = reply_account.zip(r.get::<_, Option<String>>(13)?).zip(r.get::<_, Option<i64>>(18)?).zip(r.get::<_, Option<u32>>(19)?).map(|(((account_id, mailbox), mailbox_id), uid_validity)| crate::MailboxIdentity { account_id, mailbox, mailbox_id, uid_validity });
            Ok(Snapshot { source, reply_account: r.get(17)?, dirty: r.get(4)?, files_dirty: r.get(5)?, draft: SavedDraft {
                id: source.unwrap_or(0), account_id: r.get(1)?, incarnation: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                updated_epoch: r.get::<_, Option<i64>>(3)?.unwrap_or(0), to_raw: r.get(6)?, cc_raw: r.get(7)?, bcc_raw: r.get(8)?,
                subject: r.get(9)?, body: r.get(10)?, body_html: r.get(11)?, reply_to_uid: r.get(12)?, reply_to_mailbox: r.get(13)?,
                important: r.get(14)?, reply_identity, thread_headers: crate::ThreadHeaders { in_reply_to: r.get(15)?, references: r.get(16)? }, thread_id: None, remote_uid: None, pushed_epoch: None,
            }})
        }).optional()?.ok_or(Error::StaleDraft)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enqueue_receipt_survives_consumption_and_delivery_but_never_authorizes_another_session() {
        let (store, draft) = fixture();
        begin(&store, &draft);
        assert_eq!(
            store.queued_draft_edit("test", draft.account_id).unwrap(),
            None
        );
        assert!(matches!(
            store.queued_draft_edit("missing", draft.account_id),
            Err(Error::StaleDraft)
        ));
        assert!(matches!(
            store.queued_draft_edit("test", draft.account_id + 1),
            Err(Error::StaleDraft)
        ));
        let message = crate::compose(
            "edit@example.com",
            "to@example.com",
            "",
            "",
            "receipt",
            "body",
            None,
        )
        .unwrap();
        let id = store
            .enqueue_draft_edit("test", draft.account_id, &message, None)
            .unwrap();
        assert_eq!(
            store.queued_draft_edit("test", draft.account_id).unwrap(),
            Some(id)
        );
        store
            .set_outbox_state(id, crate::OutboxState::Sent)
            .unwrap();
        assert_eq!(
            store.queued_draft_edit("test", draft.account_id).unwrap(),
            Some(id)
        );
        assert!(matches!(
            store.queued_draft_edit("test", draft.account_id + 1),
            Err(Error::StaleDraft)
        ));
        assert!(matches!(
            store.queued_draft_edit("other", draft.account_id),
            Err(Error::StaleDraft)
        ));
    }

    fn fixture() -> (Store, SavedDraft) {
        let store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("edit@example.com", "gmail")
            .unwrap();
        let saved = store
            .save_draft(
                account,
                None,
                None,
                DraftContent {
                    body: "original",
                    ..Default::default()
                },
            )
            .unwrap();
        store
            .add_draft_attachment(saved.id, "original.txt", "text/plain", b"original bytes")
            .unwrap();
        let draft = store.draft(saved.id).unwrap().unwrap();
        store
            .record_draft_pushed(draft.id, Some(100), draft.updated_epoch)
            .unwrap();
        (store, draft)
    }
    fn begin(store: &Store, draft: &SavedDraft) -> DraftEdit {
        store
            .begin_draft_edit(
                "test",
                draft.account_id,
                Some(draft.id),
                Some(&draft.incarnation),
                Some(draft.updated_epoch),
            )
            .unwrap()
    }
    fn blobs(store: &Store) -> i64 {
        store
            .conn()
            .query_row("SELECT COUNT(*) FROM draft_blobs", [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn draft_edit_reply_identity_survives_autosave_and_rejects_a_reset_source() {
        let mut server = crate::test_support::FakeServer::new(false);
        server.add_with_body(1, "parent", "body");
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("reply@example.com", "gmail")
            .unwrap();
        crate::SyncEngine::default()
            .sync(&mut server, &mut store, account, "INBOX")
            .unwrap();
        let identity = store.mailbox_identity(account, "INBOX").unwrap().unwrap();
        store
            .begin_draft_edit("reply", account, None, None, None)
            .unwrap();
        store.set_draft_edit_reply("reply", &identity, 1).unwrap();
        let content = DraftContent {
            body: "My reply",
            reply_to_uid: Some(1),
            reply_to_mailbox: Some("INBOX"),
            ..Default::default()
        };
        let saved = store
            .save_draft_edit("reply", account, content)
            .unwrap()
            .unwrap();
        assert_eq!(
            store
                .draft_edit("reply")
                .unwrap()
                .draft
                .unwrap()
                .reply_identity,
            Some(identity.clone())
        );
        server.uid_validity += 1;
        crate::SyncEngine::default()
            .sync(&mut server, &mut store, account, "INBOX")
            .unwrap();
        assert_eq!(store.draft(saved.id).unwrap().unwrap().thread_id, None);
        assert!(matches!(
            store.set_draft_edit_reply("reply", &identity, 1),
            Err(Error::StaleMailbox)
        ));
        let file = crate::DraftAttachmentFull {
            name: "wrong.bin".into(),
            mime: "application/octet-stream".into(),
            bytes: vec![42],
        };
        assert!(matches!(
            store.add_draft_edit_source_attachment("reply", account, &identity, &file),
            Err(Error::StaleMailbox)
        ));
        let saved = store
            .save_draft_edit("reply", account, content)
            .unwrap()
            .unwrap();
        assert_eq!(store.draft(saved.id).unwrap().unwrap().reply_to_uid, None);
        assert!(store.draft_edit_attachments("reply").unwrap().is_empty());
    }

    #[test]
    fn draft_edit_late_reply_context_updates_an_already_saved_draft() {
        let (store, draft) = fixture();
        begin(&store, &draft);
        store.conn().execute("UPDATE draft_edit SET reply_in_reply_to = '<parent@example.com>', reply_references = '<root@example.com> <parent@example.com>' WHERE token = 'test'", []).unwrap();
        let saved = store
            .save_draft_edit("test", draft.account_id, draft.content())
            .unwrap()
            .unwrap();
        let frozen = store.draft(saved.id).unwrap().unwrap();
        assert_eq!(
            frozen.thread_headers.in_reply_to.as_deref(),
            Some("<parent@example.com>")
        );
        assert!(saved.updated_epoch > draft.updated_epoch);
        let message = crate::compose(
            "from@example.com",
            "to@example.com",
            "",
            "",
            "Reply",
            "body",
            Some("<wrong@example.com>"),
        )
        .unwrap();
        let id = store
            .enqueue_draft_edit("test", draft.account_id, &message, None)
            .unwrap();
        assert_eq!(
            store
                .conn()
                .query_row("SELECT in_reply_to FROM outbox WHERE id = ?1", [id], |r| {
                    r.get::<_, String>(0)
                })
                .unwrap(),
            "<parent@example.com>"
        );
    }

    #[test]
    fn draft_edit_sender_change_moves_the_version_and_tombstones_the_old_account() {
        let (store, draft) = fixture();
        let other = store
            .adopt_or_create_account("other@example.com", "gmail")
            .unwrap();
        begin(&store, &draft);
        let moved = store
            .save_draft_edit("test", other, draft.content())
            .unwrap()
            .unwrap();
        assert_eq!(moved.id, draft.id);
        assert!(!moved.forked);
        assert!(moved.updated_epoch > draft.updated_epoch);
        assert!(store.drafts_of(draft.account_id).unwrap().is_empty());
        assert_eq!(store.draft_tombstones(draft.account_id).unwrap(), vec![100]);
        assert_eq!(store.drafts_to_push(other).unwrap().len(), 1);
        assert_eq!(store.draft(moved.id).unwrap().unwrap().remote_uid, None);
        assert_eq!(
            store.draft_attachments_full(moved.id).unwrap()[0].bytes,
            b"original bytes"
        );
        store.finish_draft_edit("test", false).unwrap();
        assert_eq!(blobs(&store), 1);
    }

    #[test]
    fn draft_edit_open_close_and_identical_save_do_not_repush_or_copy_payloads() {
        let (store, draft) = fixture();
        assert_eq!(begin(&store, &draft).attachments.len(), 1);
        begin(&store, &draft); // Retry after a lost begin response.
        let saved = store
            .save_draft_edit("test", draft.account_id, draft.content())
            .unwrap()
            .unwrap();
        assert_eq!(saved.updated_epoch, draft.updated_epoch);
        assert_eq!(blobs(&store), 1);
        store.finish_draft_edit("test", false).unwrap();
        store.finish_draft_edit("test", false).unwrap();
        assert!(store.drafts_to_push(draft.account_id).unwrap().is_empty());
        assert_eq!(blobs(&store), 1);
    }

    #[test]
    fn draft_edit_rejects_stale_open_and_reused_id_at_the_same_epoch() {
        let (store, draft) = fixture();
        store.drop_stale_draft(draft.id).unwrap();
        let replacement = store
            .save_draft(draft.account_id, Some(draft.id), None, draft.content())
            .unwrap();
        store
            .conn()
            .execute(
                "UPDATE drafts SET updated_epoch = ?1 WHERE id = ?2",
                params![draft.updated_epoch, replacement.id],
            )
            .unwrap();
        assert_ne!(
            store.draft(draft.id).unwrap().unwrap().incarnation,
            draft.incarnation
        );
        assert!(matches!(
            store.begin_draft_edit(
                "test",
                draft.account_id,
                Some(draft.id),
                Some(&draft.incarnation),
                Some(draft.updated_epoch)
            ),
            Err(Error::StaleDraft)
        ));
    }

    #[test]
    fn draft_edit_survives_forty_remote_replacements_without_history_growth() {
        let (store, draft) = fixture();
        begin(&store, &draft);
        let mut id = draft.id;
        for _ in 0..40 {
            store.drop_stale_draft(id).unwrap();
            id = store
                .save_draft(draft.account_id, None, None, draft.content())
                .unwrap()
                .id;
            store
                .add_draft_attachment(id, "remote.txt", "text/plain", b"remote bytes")
                .unwrap();
            assert_eq!(blobs(&store), 2);
        }
        assert!(
            store
                .save_draft_edit("test", draft.account_id, draft.content())
                .unwrap()
                .is_none()
        );
        store.finish_draft_edit("test", false).unwrap();
        assert_eq!(store.drafts().unwrap().len(), 1);
        assert_eq!(blobs(&store), 1);
    }

    #[test]
    fn draft_edit_recovers_a_committed_version_deleted_after_autosave() {
        let (store, draft) = fixture();
        begin(&store, &draft);
        let saved = store
            .save_draft_edit(
                "test",
                draft.account_id,
                DraftContent {
                    body: "saved edit",
                    ..draft.content()
                },
            )
            .unwrap()
            .unwrap();
        store.drop_stale_draft(saved.id).unwrap();
        store.recover_draft_edit().unwrap();
        store.recover_draft_edit().unwrap();
        let recovered = store.drafts().unwrap();
        assert_eq!(recovered.len(), 1);
        assert_eq!(recovered[0].body, "saved edit");
        assert_eq!(
            store.draft_attachments_full(recovered[0].id).unwrap()[0].bytes,
            b"original bytes"
        );
        assert_eq!(blobs(&store), 1);
    }

    #[test]
    fn draft_edit_attachment_only_changes_fork_the_edited_file_set() {
        let (store, draft) = fixture();
        let opened = begin(&store, &draft);
        store.drop_stale_draft(draft.id).unwrap();
        let added = store
            .add_draft_edit_attachment(
                "test",
                draft.account_id,
                "added.txt",
                "text/plain",
                b"added bytes",
            )
            .unwrap();
        assert!(added.forked);
        assert_eq!(added.attachments.len(), 2);
        let removed = store
            .remove_draft_edit_attachment("test", draft.account_id, opened.attachments[0].id)
            .unwrap()
            .unwrap();
        assert_eq!(removed.attachments.len(), 1);
        assert_eq!(
            store.draft_attachments_full(removed.id).unwrap()[0].bytes,
            b"added bytes"
        );
        store.finish_draft_edit("test", false).unwrap();
        assert_eq!(blobs(&store), 1);
    }

    #[test]
    fn draft_edit_fork_failure_rolls_back_without_releasing_the_files() {
        let (store, draft) = fixture();
        begin(&store, &draft);
        store.drop_stale_draft(draft.id).unwrap();
        store.conn().execute_batch("CREATE TRIGGER fail_fork BEFORE INSERT ON draft_attachments BEGIN SELECT RAISE(ABORT, 'synthetic failure'); END;").unwrap();
        let content = DraftContent {
            body: "my edit",
            ..draft.content()
        };
        assert!(
            store
                .save_draft_edit("test", draft.account_id, content)
                .is_err()
        );
        assert!(store.drafts().unwrap().is_empty());
        assert_eq!(blobs(&store), 1);
        assert_eq!(store.draft_edit_attachments("test").unwrap().len(), 1);
        store
            .conn()
            .execute_batch("DROP TRIGGER fail_fork")
            .unwrap();
        assert!(
            store
                .save_draft_edit("test", draft.account_id, content)
                .unwrap()
                .unwrap()
                .forked
        );
    }

    #[test]
    fn draft_edit_enqueue_is_atomic_and_replayed_once() {
        let (store, draft) = fixture();
        begin(&store, &draft);
        let message = crate::compose(
            "edit@example.com",
            "recipient@example.com",
            "",
            "",
            "subject",
            "edited body",
            None,
        )
        .unwrap();
        store.conn().execute_batch("CREATE TRIGGER fail_consume BEFORE DELETE ON draft_edit BEGIN SELECT RAISE(ABORT, 'synthetic consume failure'); END;").unwrap();
        assert!(
            store
                .enqueue_draft_edit("test", draft.account_id, &message, None)
                .is_err()
        );
        assert_eq!(
            store
                .conn()
                .query_row("SELECT COUNT(*) FROM outbox", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert_eq!(store.draft_edit_attachments("test").unwrap().len(), 1);
        store
            .conn()
            .execute_batch("DROP TRIGGER fail_consume")
            .unwrap();
        let id = store
            .enqueue_draft_edit("test", draft.account_id, &message, Some(2_000_000_000))
            .unwrap();
        assert_eq!(
            store
                .enqueue_draft_edit("test", draft.account_id, &message, None)
                .unwrap(),
            id
        );
        assert_eq!(
            store
                .conn()
                .query_row(
                    "SELECT bytes FROM outbox_attachments WHERE outbox_id = ?1",
                    [id],
                    |r| r.get::<_, Vec<u8>>(0)
                )
                .unwrap(),
            b"original bytes"
        );
        assert_eq!(
            store
                .conn()
                .query_row("SELECT COUNT(*) FROM outbox", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert!(store.drafts().unwrap().is_empty());
        assert_eq!(blobs(&store), 0);
        store.recover_draft_edit().unwrap();
        assert!(store.drafts().unwrap().is_empty());
    }

    #[test]
    fn draft_edit_tokens_and_account_removal_scope_all_files() {
        let (mut store, draft) = fixture();
        begin(&store, &draft);
        assert!(
            store
                .save_draft_edit("other", draft.account_id, draft.content())
                .is_err()
        );
        assert!(
            store
                .remove_draft_edit_attachment("other", draft.account_id, 1)
                .is_err()
        );
        assert!(store.finish_draft_edit("other", true).is_err());
        store.delete_account(draft.account_id).unwrap();
        assert_eq!(blobs(&store), 0);
        assert!(store.draft_edit_attachments("test").is_err());
    }
}

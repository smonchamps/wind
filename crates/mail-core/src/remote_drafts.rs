//! Complete remote imports; obsolete mirrors survive until every fetch succeeds.
use crate::{Error, RemoteDraft, SavedDraft, Store, Uid};
use rusqlite::{OptionalExtension, params};

impl Store {
    pub fn import_remote_draft_complete(
        &self,
        account: i64,
        generation: u32,
        uid: Uid,
        draft: &RemoteDraft,
    ) -> Result<i64, Error> {
        let tx = self.conn().unchecked_transaction()?;
        self.verify_drafts_generation(account, generation)?;
        let discarded: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM draft_tombstones WHERE account_id = ?1 AND remote_uid = ?2)", params![account, uid], |r| r.get(0))?;
        if discarded {
            return Err(Error::StaleDraft);
        }
        if let Some(id) = tx
            .query_row(
                "SELECT id FROM drafts WHERE account_id = ?1 AND remote_uid = ?2",
                params![account, uid],
                |r| r.get(0),
            )
            .optional()?
        {
            return Ok(id);
        }
        let now = chrono::Utc::now().timestamp_millis();
        tx.execute("INSERT INTO drafts (account_id, remote_uid, updated_epoch, pushed_epoch, to_raw, cc_raw, bcc_raw, subject, body, body_html, important, reply_in_reply_to, reply_references) VALUES (?1, ?2, ?3, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)", params![account, uid, now, draft.to_raw, draft.cc_raw, draft.bcc_raw, draft.subject, draft.text.as_deref().unwrap_or_default(), draft.html, draft.important, draft.thread_headers.in_reply_to, draft.thread_headers.references])?;
        let id = tx.last_insert_rowid();
        let mut remaining = crate::MAX_ATTACHMENTS_BYTES;
        for file in &draft.attachments {
            let size = file.bytes.len() as u64;
            if size > remaining {
                return Err(Error::AttachmentOverBudget {
                    name: file.name.clone(),
                    size,
                    remaining,
                });
            }
            remaining -= size;
            tx.execute("INSERT INTO draft_blobs (bytes) VALUES (?1)", [&file.bytes])?;
            let blob = tx.last_insert_rowid();
            tx.execute("INSERT INTO draft_attachments (draft_id, name, mime, size, blob_id) VALUES (?1, ?2, ?3, ?4, ?5)", params![id, file.name, file.mime, crate::sql_u64(size), blob])?;
        }
        tx.commit()?;
        Ok(id)
    }

    pub fn finish_remote_draft_pull(
        &self,
        account: i64,
        generation: u32,
        stale: &[SavedDraft],
    ) -> Result<(), Error> {
        let tx = self.conn().unchecked_transaction()?;
        self.verify_drafts_generation(account, generation)?;
        for draft in stale {
            tx.execute("DELETE FROM drafts WHERE id = ?1 AND account_id = ?2 AND incarnation = ?3 AND updated_epoch = ?4 AND remote_uid = ?5 AND pushed_epoch >= updated_epoch", params![draft.id, account, draft.incarnation, draft.updated_epoch, draft.remote_uid])?;
        }
        tx.commit()?;
        Ok(())
    }

    fn verify_drafts_generation(&self, account: i64, generation: u32) -> Result<(), Error> {
        let known = self
            .conn()
            .query_row(
                "SELECT uid_validity FROM drafts_remote WHERE account_id = ?1",
                [account],
                |r| r.get::<_, u32>(0),
            )
            .optional()?;
        if known != Some(generation) {
            return Err(Error::StaleMailbox);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DraftAttachmentFull, DraftContent};

    fn fixture() -> (Store, i64, RemoteDraft) {
        let store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("remote@example.com", "gmail")
            .unwrap();
        store.align_drafts_uidvalidity(account, 7).unwrap();
        let draft = RemoteDraft {
            to_raw: "to@example.com".into(),
            cc_raw: "cc@example.com".into(),
            bcc_raw: "hidden@example.com".into(),
            important: true,
            subject: "remote".into(),
            text: Some("body".into()),
            html: Some("<b>body</b>".into()),
            attachments: vec![DraftAttachmentFull {
                name: "binary.bin".into(),
                mime: "application/octet-stream".into(),
                bytes: vec![0, 255, 42],
            }],
            thread_headers: crate::ThreadHeaders {
                in_reply_to: Some("<parent@example.com>".into()),
                references: Some("<root@example.com> <parent@example.com>".into()),
            },
        };
        (store, account, draft)
    }

    #[test]
    fn remote_import_does_not_resurrect_a_draft_discarded_during_fetch() {
        let (store, account, remote) = fixture();
        store
            .conn()
            .execute(
                "INSERT INTO draft_tombstones (account_id, remote_uid) VALUES (?1, 42)",
                [account],
            )
            .unwrap();
        assert!(matches!(
            store.import_remote_draft_complete(account, 7, 42, &remote),
            Err(Error::StaleDraft)
        ));
        assert!(store.drafts_of(account).unwrap().is_empty());
    }

    #[test]
    fn remote_import_is_complete_clean_and_idempotent() {
        let (store, account, remote) = fixture();
        let id = store
            .import_remote_draft_complete(account, 7, 42, &remote)
            .unwrap();
        let draft = store.draft(id).unwrap().unwrap();
        assert_eq!(draft.cc_raw, remote.cc_raw);
        assert_eq!(draft.bcc_raw, remote.bcc_raw);
        assert!(draft.important);
        assert_eq!(draft.thread_headers, remote.thread_headers);
        assert_eq!(
            store.draft_attachments_full(id).unwrap(),
            remote.attachments
        );
        assert!(store.drafts_to_push(account).unwrap().is_empty());
        assert_eq!(
            store
                .import_remote_draft_complete(account, 7, 42, &remote)
                .unwrap(),
            id
        );
        assert_eq!(store.drafts_of(account).unwrap().len(), 1);
    }

    #[test]
    fn remote_reply_headers_survive_edit_and_mirror_replacement() {
        let (store, account, remote) = fixture();
        let id = store
            .import_remote_draft_complete(account, 7, 42, &remote)
            .unwrap();
        let draft = store.draft(id).unwrap().unwrap();
        let edit = store
            .begin_draft_edit(
                "reply",
                account,
                Some(id),
                Some(&draft.incarnation),
                Some(draft.updated_epoch),
            )
            .unwrap();
        assert_eq!(edit.draft.unwrap().thread_headers, remote.thread_headers);
        store.drop_stale_draft(id).unwrap();
        let recovered = store
            .save_draft_edit(
                "reply",
                account,
                DraftContent {
                    body: "edited reply",
                    ..draft.content()
                },
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            store.draft(recovered.id).unwrap().unwrap().thread_headers,
            remote.thread_headers
        );
        store.finish_draft_edit("reply", false).unwrap();
    }

    #[test]
    fn remote_import_failure_leaves_no_partial_draft_or_payload() {
        let (store, account, remote) = fixture();
        store.conn().execute_batch("CREATE TRIGGER fail_import BEFORE INSERT ON draft_attachments BEGIN SELECT RAISE(ABORT, 'disk failure'); END;").unwrap();
        assert!(
            store
                .import_remote_draft_complete(account, 7, 42, &remote)
                .is_err()
        );
        assert!(store.drafts_of(account).unwrap().is_empty());
        assert_eq!(
            store
                .conn()
                .query_row("SELECT COUNT(*) FROM draft_blobs", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }

    #[test]
    fn remote_pull_rejects_changed_generation_and_keeps_locally_edited_mirrors() {
        let (store, account, remote) = fixture();
        let id = store
            .import_remote_draft_complete(account, 7, 42, &remote)
            .unwrap();
        let snapshot = store.draft(id).unwrap().unwrap();
        store
            .save_draft(
                account,
                Some(id),
                Some(snapshot.updated_epoch),
                DraftContent {
                    body: "edited locally",
                    ..snapshot.content()
                },
            )
            .unwrap();
        store
            .finish_remote_draft_pull(account, 7, &[snapshot])
            .unwrap();
        assert_eq!(store.draft(id).unwrap().unwrap().body, "edited locally");
        store.align_drafts_uidvalidity(account, 8).unwrap();
        assert!(matches!(
            store.import_remote_draft_complete(account, 7, 43, &remote),
            Err(Error::StaleMailbox)
        ));
        assert!(matches!(
            store.finish_remote_draft_pull(account, 7, &[]),
            Err(Error::StaleMailbox)
        ));
    }
}

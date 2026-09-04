//! Local drafts: never lose text again.
//!
//! A draft is RAW text, not yet validated — that is its whole point: a
//! half-typed address is kept exactly as it is. Strict validation
//! ([`crate::compose`]) only happens at send time. Same philosophy as the
//! outbox: log first, the user decides afterwards (resume, send, or
//! discard).
//!
//! Synchronization to Gmail (push only, v1): every local draft is
//! mirrored into the server's Drafts folder. Invariants:
//! - we only delete remotely UIDs that WE registered; UIDVALIDITY
//!   changed → we abandon the markers (a duplicate draft is acceptable,
//!   deleting the wrong message never is);
//! - the "clean" marker is a timestamp snapshot: an edit that happened
//!   DURING the push leaves the draft still to push.
//!
//! **Pull** (Phase 3): a draft created elsewhere — webmail, phone — is
//! brought back to be edited here. The reverse direction reopens a
//! question that push-only avoided: what to do when both sides have
//! moved? The answer follows the golden rule already in force — **a
//! duplicate is acceptable, lost text never is** — and is spelled out in
//! [`plan_draft_pull`].

use chrono::Utc;
use rusqlite::{OptionalExtension, params};

use crate::envelope::Uid;
use crate::error::Error;
use crate::store::Store;

/// The content of a draft, as the editor holds it.
///
/// Grouped rather than spread across parameters: these four fields always
/// travel together, and a signature that separates them invites swapping
/// two of them — they are all strings.
///
/// Nothing is validated here: a half-typed address must be kept exactly
/// as it is. Strict validation only happens at send time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DraftContent<'a> {
    pub to_raw: &'a str,
    /// Raw, unvalidated Cc and Bcc — like `to_raw`, strict validation
    /// only happens at send time. Empty = no Cc/Bcc.
    pub cc_raw: &'a str,
    pub bcc_raw: &'a str,
    pub subject: &'a str,
    pub body: &'a str,
    /// Rich body (PLAN-COMPOSITION-HTML) — `None` = plain-text draft.
    /// `body` stays ALWAYS populated (text derived on the app side): it
    /// is the one previews and the text/plain fallback read.
    pub body_html: Option<&'a str>,
    pub reply_to_uid: Option<Uid>,
    /// The mailbox that gives `reply_to_uid` its meaning: UIDs restart
    /// from 1 in each mailbox (ADR 0009), a bare UID names nothing. It
    /// is what lets the draft be linked back to its conversation
    /// (PLAN-BROUILLONS, B-D2).
    pub reply_to_mailbox: Option<&'a str>,
    /// Marked "important" (R3, PLAN-RETOURS-6): the state follows the
    /// draft — resuming it finds it again, sending carries it along.
    pub important: bool,
}

/// What a save actually did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftSaved {
    pub id: i64,
    /// To pass back as `base_epoch` on the next save.
    pub updated_epoch: i64,
    /// The version in the database had changed under the editor's
    /// fingers: its text was kept **apart** instead of overwriting the
    /// other one.
    pub forked: bool,
}

/// A draft as the user left it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedDraft {
    pub id: i64,
    /// The account that will send it (and whose Drafts folder mirrors it).
    pub account_id: i64,
    /// Raw, unvalidated "To" field (can be empty or incomplete).
    pub to_raw: String,
    /// Raw, unvalidated Cc and Bcc (empty if the draft has none, or if
    /// it predates these columns).
    pub cc_raw: String,
    pub bcc_raw: String,
    pub subject: String,
    pub body: String,
    /// Rich body — `None` for a plain-text draft (from before the
    /// column existed, or pulled from the server); the editor converts
    /// it on open.
    pub body_html: Option<String>,
    /// UID of the message this draft replies to, if any.
    pub reply_to_uid: Option<Uid>,
    /// The mailbox of the targeted message — `None` for a free
    /// composition or a draft from before the column (they keep their
    /// safety net: the Drafts folder, with no mention in the list).
    pub reply_to_mailbox: Option<String>,
    /// The thread of the conversation this draft replies to — resolved
    /// at READ time (mailbox + UID → envelope), never stored: a
    /// recomputed thread cannot leave a stale marker here.
    /// Marked "important" (R3) — `false` on a draft from before the
    /// column.
    pub important: bool,
    /// `None`: free composition, mailbox gone, message purged, or draft
    /// from before the column.
    pub thread_id: Option<i64>,
    /// Milliseconds — the "most recent first" ordering must stay true
    /// between two saves close together in time.
    pub updated_epoch: i64,
    /// UID of the last copy pushed to the Gmail Drafts folder.
    pub remote_uid: Option<Uid>,
    /// Snapshot of `updated_epoch` at the moment of the last successful
    /// push.
    pub pushed_epoch: Option<i64>,
}

/// Cap on the attachments of ONE message: 25 MB of decoded sizes — the
/// Gmail limit, the most common one (PJ-D3). Refused at the gesture,
/// never at send time: the attachment that goes over never enters the
/// database. Base64 encoding (+33%) can still trip up a stricter
/// server: that refusal is a 5xx classed `Permanent`, carried by the
/// existing notice slot.
pub const MAX_ATTACHMENTS_BYTES: u64 = 25 * 1024 * 1024;

/// An attachment of a draft, WITHOUT its bytes: the chip list draws
/// itself without ever loading the blobs — they are only read when
/// building the MIME.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftAttachmentMeta {
    pub id: i64,
    pub draft_id: i64,
    pub name: String,
    pub mime: String,
    /// DECODED bytes — the size the user recognizes.
    pub size: u64,
}

/// An attachment with its bytes — the shape the MIME builder of the
/// IMAP mirror consumes (PJ-D6). Never for a list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftAttachmentFull {
    pub name: String,
    pub mime: String,
    pub bytes: Vec<u8>,
}

/// Outcome of a gesture on the attachments (add or remove).
///
/// `updated_epoch` is to be taken as `base_epoch` by the editor: the
/// gesture modified the draft (it will re-push on the next cycle), and
/// a save that kept the old marker would be accused of a conflict that
/// does not exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftAttachmentSaved {
    pub attachment: DraftAttachmentMeta,
    pub updated_epoch: i64,
}

impl Store {
    /// Saves (`id: None`) or updates a draft.
    ///
    /// A stale id (draft deleted in the meantime by another view)
    /// re-inserts instead of silently losing the text — this is a
    /// safety net, it must never have a missing mesh.
    ///
    /// `base_epoch` is the `updated_epoch` the editor believes it is
    /// modifying. It is an **assertion**: "I am modifying the row I
    /// read." It is contradicted in two ways, and both count:
    ///
    /// 1. the timestamp in the database has changed — someone rewrote
    ///    the row;
    /// 2. **the row has disappeared** — the pull *replaced* it, because
    ///    it does not update: it removes the stale mirror and imports
    ///    the fresh version under a new identifier
    ///    ([`plan_draft_pull`]). This is the only one of the two cases
    ///    the field actually produces, and it was the one that went
    ///    unnoticed: comparing only timestamps, the detection fell
    ///    silent as soon as there was only one left.
    ///
    /// In both cases, overwriting — or re-inserting silently — leaves
    /// the user with two texts whose existence they do not know about.
    /// So we keep BOTH **and say so**: the module's golden rule applied
    /// to concurrent editing.
    ///
    /// `None` disables the detection — for callers that do not hold an
    /// in-memory copy, and so have nothing to overwrite.
    pub fn save_draft(
        &self,
        account_id: i64,
        id: Option<i64>,
        base_epoch: Option<i64>,
        content: DraftContent<'_>,
    ) -> Result<DraftSaved, Error> {
        let DraftContent {
            to_raw,
            cc_raw,
            bcc_raw,
            subject,
            body,
            body_html,
            reply_to_uid,
            reply_to_mailbox,
            important,
        } = content;
        let now = Utc::now().timestamp_millis();
        match id {
            Some(id) => {
                let stored: Option<i64> = self
                    .conn()
                    .query_row(
                        "SELECT updated_epoch FROM drafts WHERE id = ?1",
                        [id],
                        |row| row.get(0),
                    )
                    .optional()?;
                let conflict = match (stored, base_epoch) {
                    // The row was rewritten out from under the composer.
                    (Some(stored), Some(base)) => stored != base,
                    // It disappeared out from under it: replaced by the
                    // pull, or discarded from another view. The safety
                    // net re-inserts it regardless — but silently, the
                    // two texts became indistinguishable.
                    (None, Some(_)) => true,
                    // No in-memory copy: nothing to overwrite.
                    (_, None) => false,
                };
                if conflict {
                    let forked = self.insert_draft(
                        account_id,
                        to_raw,
                        cc_raw,
                        bcc_raw,
                        subject,
                        body,
                        body_html,
                        reply_to_uid,
                        reply_to_mailbox,
                        important,
                        now,
                    )?;
                    return Ok(DraftSaved {
                        id: forked,
                        updated_epoch: now,
                        forked: true,
                    });
                }
                // MAX(…, +1): the timestamp advances STRICTLY on every
                // real modification — an edit in the same millisecond as
                // a push's snapshot would otherwise stay invisible (a
                // net mesh, caught by a test). And the WHERE: a save
                // with IDENTICAL content touches nothing, otherwise
                // every close would re-push an identical copy to Gmail
                // (churn observed in field validation).
                self.conn().execute(
                    "INSERT INTO drafts (id, account_id, to_raw, cc_raw, bcc_raw, subject, body, body_html, reply_to_uid, reply_to_mailbox, important, updated_epoch)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
                     ON CONFLICT(id) DO UPDATE SET
                       to_raw = excluded.to_raw,
                       cc_raw = excluded.cc_raw,
                       bcc_raw = excluded.bcc_raw,
                       subject = excluded.subject,
                       body = excluded.body,
                       body_html = excluded.body_html,
                       reply_to_uid = excluded.reply_to_uid,
                       reply_to_mailbox = excluded.reply_to_mailbox,
                       important = excluded.important,
                       updated_epoch = MAX(excluded.updated_epoch, drafts.updated_epoch + 1)
                     WHERE drafts.to_raw IS NOT excluded.to_raw
                        OR drafts.cc_raw IS NOT excluded.cc_raw
                        OR drafts.bcc_raw IS NOT excluded.bcc_raw
                        OR drafts.subject IS NOT excluded.subject
                        OR drafts.body IS NOT excluded.body
                        OR drafts.body_html IS NOT excluded.body_html
                        OR drafts.reply_to_uid IS NOT excluded.reply_to_uid
                        OR drafts.reply_to_mailbox IS NOT excluded.reply_to_mailbox
                        OR drafts.important IS NOT excluded.important",
                    params![id, account_id, to_raw, cc_raw, bcc_raw, subject, body, body_html, reply_to_uid, reply_to_mailbox, important, now],
                )?;
                // Re-read, not assumed: the `WHERE` above may have left
                // the timestamp untouched (identical save), and
                // returning `now` would make detection fail on the next
                // round over a conflict that does not exist.
                let updated_epoch = self.conn().query_row(
                    "SELECT updated_epoch FROM drafts WHERE id = ?1",
                    [id],
                    |row| row.get(0),
                )?;
                Ok(DraftSaved {
                    id,
                    updated_epoch,
                    forked: false,
                })
            }
            None => Ok(DraftSaved {
                id: self.insert_draft(
                    account_id,
                    to_raw,
                    cc_raw,
                    bcc_raw,
                    subject,
                    body,
                    body_html,
                    reply_to_uid,
                    reply_to_mailbox,
                    important,
                    now,
                )?,
                updated_epoch: now,
                forked: false,
            }),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn insert_draft(
        &self,
        account_id: i64,
        to_raw: &str,
        cc_raw: &str,
        bcc_raw: &str,
        subject: &str,
        body: &str,
        body_html: Option<&str>,
        reply_to_uid: Option<Uid>,
        reply_to_mailbox: Option<&str>,
        important: bool,
        now: i64,
    ) -> Result<i64, Error> {
        self.conn().execute(
            "INSERT INTO drafts (account_id, to_raw, cc_raw, bcc_raw, subject, body, body_html, reply_to_uid, reply_to_mailbox, important, updated_epoch)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![account_id, to_raw, cc_raw, bcc_raw, subject, body, body_html, reply_to_uid, reply_to_mailbox, important, now],
        )?;
        Ok(self.conn().last_insert_rowid())
    }

    /// A cheap revision of the drafts table — `(count, latest
    /// updated_epoch, largest id)`: any add, delete or edit moves at
    /// least one of the three. It is what lets the resting probe say
    /// "nothing changed" without shipping the whole list, bodies
    /// included, every ten seconds (PLAN-AUDIT-V3 E5, D-52 item 3).
    pub fn drafts_revision(&self) -> Result<(i64, i64, i64), Error> {
        Ok(self.conn().query_row(
            "SELECT COUNT(*), COALESCE(MAX(updated_epoch), 0), COALESCE(MAX(id), 0) FROM drafts",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?)
    }

    /// The drafts, most recent first.
    pub fn drafts(&self) -> Result<Vec<SavedDraft>, Error> {
        let mut stmt = self.conn().prepare(&format!(
            "{DRAFT_SELECT} ORDER BY d.updated_epoch DESC, d.id DESC"
        ))?;
        let rows = stmt
            .query_map([], row_to_draft)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// All the drafts of ONE account — what the pull compares to the
    /// remote list.
    pub fn drafts_of(&self, account_id: i64) -> Result<Vec<SavedDraft>, Error> {
        let mut stmt = self.conn().prepare(&format!(
            "{DRAFT_SELECT} WHERE d.account_id = ?1 ORDER BY d.id"
        ))?;
        let rows = stmt
            .query_map([account_id], row_to_draft)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// The drafts of ONE account whose Drafts folder does not (or no
    /// longer) have the latest version, in creation order.
    pub fn drafts_to_push(&self, account_id: i64) -> Result<Vec<SavedDraft>, Error> {
        let mut stmt = self.conn().prepare(&format!(
            "{DRAFT_SELECT}
             WHERE d.account_id = ?1
               AND (d.pushed_epoch IS NULL OR d.pushed_epoch < d.updated_epoch)
             ORDER BY d.id"
        ))?;
        let rows = stmt
            .query_map([account_id], row_to_draft)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Records a successful push: the old remote copy (if different)
    /// becomes a tombstone, the timestamp snapshot becomes the "clean"
    /// marker. An edit that happened during the push keeps the draft
    /// still to push — the net never skips a mesh.
    pub fn record_draft_pushed(
        &self,
        id: i64,
        remote_uid: Option<Uid>,
        pushed_epoch: i64,
    ) -> Result<(), Error> {
        let tx = self.conn().unchecked_transaction()?;
        tx.execute(
            "INSERT OR IGNORE INTO draft_tombstones (account_id, remote_uid)
             SELECT account_id, remote_uid FROM drafts
             WHERE id = ?1 AND remote_uid IS NOT NULL AND remote_uid IS NOT ?2",
            params![id, remote_uid],
        )?;
        tx.execute(
            "UPDATE drafts SET remote_uid = ?2, pushed_epoch = ?3 WHERE id = ?1",
            params![id, remote_uid, pushed_epoch],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Discards a draft — explicit user decision (or a draft that
    /// became a send: it did its job). Its possible remote copy
    /// becomes a tombstone, purged on the next cycle.
    pub fn delete_draft(&self, id: i64) -> Result<(), Error> {
        let tx = self.conn().unchecked_transaction()?;
        tx.execute(
            "INSERT OR IGNORE INTO draft_tombstones (account_id, remote_uid)
             SELECT account_id, remote_uid FROM drafts
             WHERE id = ?1 AND remote_uid IS NOT NULL",
            [id],
        )?;
        tx.execute("DELETE FROM drafts WHERE id = ?1", [id])?;
        tx.commit()?;
        Ok(())
    }

    /// Attaches an attachment to the draft: the bytes are copied into
    /// the database AT THE GESTURE (PJ-D1) — never a bare path, a file
    /// moved afterwards breaks nothing. Refuses past the cap (PJ-D3)
    /// without attaching anything: the attachments already acquired
    /// stay.
    ///
    /// The draft is marked modified in the same transaction: it is the
    /// gesture that carries the modification, not the autosave
    /// (PJ-D6).
    pub fn add_draft_attachment(
        &self,
        draft_id: i64,
        name: &str,
        mime: &str,
        bytes: &[u8],
    ) -> Result<DraftAttachmentSaved, Error> {
        let size = bytes.len() as u64;
        let tx = self.conn().unchecked_transaction()?;
        let used: u64 = tx.query_row(
            "SELECT COALESCE(SUM(size), 0) FROM draft_attachments WHERE draft_id = ?1",
            [draft_id],
            |row| row.get(0),
        )?;
        let remaining = MAX_ATTACHMENTS_BYTES.saturating_sub(used);
        if size > remaining {
            return Err(Error::AttachmentOverBudget {
                name: name.to_string(),
                size,
                remaining,
            });
        }
        tx.execute(
            "INSERT INTO draft_attachments (draft_id, name, mime, size, bytes)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![draft_id, name, mime, size, bytes],
        )?;
        let attachment_id = tx.last_insert_rowid();
        let updated_epoch = touch_draft(&tx, draft_id)?;
        tx.commit()?;
        Ok(DraftAttachmentSaved {
            attachment: DraftAttachmentMeta {
                id: attachment_id,
                draft_id,
                name: name.to_string(),
                mime: mime.to_string(),
                size,
            },
            updated_epoch,
        })
    }

    /// Removes an attachment from a draft. Returns the draft's new
    /// `updated_epoch` (the removal is a modification), or `None` if
    /// the attachment no longer existed — a double-click on the removal
    /// modifies nothing.
    pub fn remove_draft_attachment(&self, attachment_id: i64) -> Result<Option<i64>, Error> {
        let tx = self.conn().unchecked_transaction()?;
        let draft_id: Option<i64> = tx
            .query_row(
                "SELECT draft_id FROM draft_attachments WHERE id = ?1",
                [attachment_id],
                |row| row.get(0),
            )
            .optional()?;
        let Some(draft_id) = draft_id else {
            return Ok(None);
        };
        tx.execute(
            "DELETE FROM draft_attachments WHERE id = ?1",
            [attachment_id],
        )?;
        let updated_epoch = touch_draft(&tx, draft_id)?;
        tx.commit()?;
        Ok(Some(updated_epoch))
    }

    /// The attachments of a draft WITH their bytes — reserved for
    /// building a message (IMAP mirror, PJ-D6). Lists go through
    /// [`Store::draft_attachments_meta`].
    pub fn draft_attachments_full(&self, draft_id: i64) -> Result<Vec<DraftAttachmentFull>, Error> {
        let mut stmt = self.conn().prepare(
            "SELECT name, mime, bytes FROM draft_attachments
             WHERE draft_id = ?1 ORDER BY id",
        )?;
        let rows = stmt
            .query_map([draft_id], |row| {
                Ok(DraftAttachmentFull {
                    name: row.get(0)?,
                    mime: row.get(1)?,
                    bytes: row.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// The attachments of a draft, metadata only, in gesture order —
    /// the blobs only leave the database when building the MIME.
    pub fn draft_attachments_meta(&self, draft_id: i64) -> Result<Vec<DraftAttachmentMeta>, Error> {
        let mut stmt = self.conn().prepare(
            "SELECT id, draft_id, name, mime, size FROM draft_attachments
             WHERE draft_id = ?1 ORDER BY id",
        )?;
        let rows = stmt
            .query_map([draft_id], |row| {
                Ok(DraftAttachmentMeta {
                    id: row.get(0)?,
                    draft_id: row.get(1)?,
                    name: row.get(2)?,
                    mime: row.get(3)?,
                    size: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Records a draft pulled back from the server.
    ///
    /// It is born **clean**: `pushed_epoch` equals `updated_epoch`, so
    /// the next cycle will not re-push it. Pushing it back as is would
    /// create a second remote copy of a message we just read — a
    /// round trip that would double on every pass.
    /// `body_html` (PLAN-COMPOSITION-HTML): without it, a RICH draft
    /// pushed then pulled back (UIDVALIDITY changed, webmail edit)
    /// came back as plain text — the formatting silently destroyed by
    /// the very job that had created it.
    #[allow(clippy::too_many_arguments)]
    pub fn import_remote_draft(
        &self,
        account_id: i64,
        remote_uid: Uid,
        to_raw: &str,
        subject: &str,
        body: &str,
        body_html: Option<&str>,
    ) -> Result<i64, Error> {
        let now = Utc::now().timestamp_millis();
        self.conn().execute(
            "INSERT INTO drafts
             (account_id, to_raw, subject, body, body_html, reply_to_uid, updated_epoch,
              remote_uid, pushed_epoch)
             VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, ?7, ?6)",
            params![
                account_id, to_raw, subject, body, body_html, now, remote_uid
            ],
        )?;
        Ok(self.conn().last_insert_rowid())
    }

    /// Removes a mirror that has become stale — its remote copy no
    /// longer exists.
    ///
    /// **Without a tombstone**, unlike [`Store::delete_draft`]: there is
    /// nothing left to delete server-side, and planting a tombstone on
    /// a freed UID would purge the message that reclaims it.
    pub fn drop_stale_draft(&self, id: i64) -> Result<(), Error> {
        self.conn()
            .execute("DELETE FROM drafts WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Remote copies of ONE account to purge (deleted or replaced) —
    /// each tombstone is purged via the connection of ITS OWN account.
    pub fn draft_tombstones(&self, account_id: i64) -> Result<Vec<Uid>, Error> {
        let mut stmt = self.conn().prepare(
            "SELECT remote_uid FROM draft_tombstones
             WHERE account_id = ?1 ORDER BY remote_uid",
        )?;
        let rows = stmt
            .query_map([account_id], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn clear_draft_tombstone(&self, account_id: i64, remote_uid: Uid) -> Result<(), Error> {
        self.conn().execute(
            "DELETE FROM draft_tombstones WHERE account_id = ?1 AND remote_uid = ?2",
            params![account_id, remote_uid],
        )?;
        Ok(())
    }

    /// Aligns the remote state of ONE account on the observed
    /// UIDVALIDITY of its Drafts folder. If it has changed, THIS
    /// account's markers are abandoned: we will re-push (possible
    /// duplicate — acceptable; deleting the wrong UID, never). Returns
    /// `true` if a reset happened. Other accounts are not touched.
    pub fn align_drafts_uidvalidity(
        &self,
        account_id: i64,
        uid_validity: u32,
    ) -> Result<bool, Error> {
        let known: Option<u32> = self
            .conn()
            .query_row(
                "SELECT uid_validity FROM drafts_remote WHERE account_id = ?1",
                [account_id],
                |row| row.get(0),
            )
            .optional()?;
        if known == Some(uid_validity) {
            return Ok(false);
        }
        let tx = self.conn().unchecked_transaction()?;
        let reset = known.is_some();
        if reset {
            tx.execute(
                "UPDATE drafts SET remote_uid = NULL, pushed_epoch = NULL
                 WHERE account_id = ?1",
                [account_id],
            )?;
            tx.execute(
                "DELETE FROM draft_tombstones WHERE account_id = ?1",
                [account_id],
            )?;
        }
        tx.execute(
            "INSERT INTO drafts_remote (account_id, uid_validity) VALUES (?1, ?2)
             ON CONFLICT(account_id) DO UPDATE SET uid_validity = excluded.uid_validity",
            params![account_id, uid_validity],
        )?;
        tx.commit()?;
        Ok(reset)
    }
}

/// What to do with the remote Drafts folder, once its UIDs are known.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DraftPull {
    /// Remote UIDs we don't know about: to pull.
    pub fetch: Vec<Uid>,
    /// Local drafts that are ONLY the mirror of a remote copy that has
    /// disappeared: to remove.
    ///
    /// Never a draft edited here — that one carries text the server
    /// does not have.
    pub stale: Vec<i64>,
}

/// Decides the pull: what to bring back, which stale mirrors to remove.
///
/// Pure and I/O-free, like thread grouping: the decision is tested
/// against field scenarios, execution stays with the caller.
///
/// Three rules, in order of importance:
///
/// 1. **We do not pull back what we already have.** Our own pushed
///    copies (`remote_uid`) and those awaiting purge (tombstones) are
///    ignored, otherwise every cycle would duplicate the mailbox.
/// 2. **We only remove a mirror.** A draft whose remote copy has
///    disappeared is removed *only* if it has not been edited here
///    since its last push. Otherwise it carries text the server has
///    never seen: it stays, and the push will put it back in place.
/// 3. **An empty remote list removes nothing.** That is exactly the
///    shape of a partial failure — wrong folder selected, truncated
///    response — and the cost of an error here is erased text. If the
///    user really deleted everything elsewhere, their copies survive
///    locally: a duplicate, not a loss.
///
/// Rule 2 is what makes editing a draft on your phone **replace** the
/// local copy instead of doubling it: the server replaces the message
/// (old UID expunged, new one created), so the same pass removes the
/// stale mirror and pulls the fresh version.
pub fn plan_draft_pull(local: &[SavedDraft], remote: &[Uid], tombstones: &[Uid]) -> DraftPull {
    let mirrored: Vec<Uid> = local.iter().filter_map(|draft| draft.remote_uid).collect();
    let fetch = remote
        .iter()
        .copied()
        .filter(|uid| !mirrored.contains(uid) && !tombstones.contains(uid))
        .collect();

    if remote.is_empty() {
        return DraftPull {
            fetch,
            stale: Vec::new(),
        };
    }
    let stale = local
        .iter()
        .filter(|draft| draft.is_clean_mirror() && !remote.contains(&draft.remote_uid.unwrap_or(0)))
        .map(|draft| draft.id)
        .collect();
    DraftPull { fetch, stale }
}

impl SavedDraft {
    /// Is the draft only the reflection of a remote copy?
    ///
    /// True when a copy has been pushed (or pulled) and nothing has
    /// been typed here since. It is the only condition under which
    /// removing it cannot erase any text.
    fn is_clean_mirror(&self) -> bool {
        match (self.remote_uid, self.pushed_epoch) {
            (Some(_), Some(pushed)) => pushed >= self.updated_epoch,
            _ => false,
        }
    }
}

/// Marks a draft modified by a gesture on its attachments, and returns
/// the new timestamp. MAX(…, +1): the same mechanics as `save_draft` —
/// the timestamp advances STRICTLY, otherwise a gesture within the
/// millisecond of a push's snapshot would stay invisible on the next
/// cycle.
fn touch_draft(conn: &rusqlite::Connection, draft_id: i64) -> Result<i64, Error> {
    let now = Utc::now().timestamp_millis();
    conn.execute(
        "UPDATE drafts SET updated_epoch = MAX(?2, updated_epoch + 1) WHERE id = ?1",
        params![draft_id, now],
    )?;
    Ok(conn.query_row(
        "SELECT updated_epoch FROM drafts WHERE id = ?1",
        [draft_id],
        |row| row.get(0),
    )?)
}

// The thread is resolved at read time — LEFT JOIN: a draft whose
// target has disappeared (mailbox renamed, message purged) stays a
// draft, simply without a thread. `(mailbox_id, uid)` is the primary
// key of envelopes: the join cannot multiply rows, and drafts number
// in the dozens — the cost is nil.
const DRAFT_SELECT: &str = "SELECT d.id, d.account_id, d.to_raw, d.subject, d.body,
        d.reply_to_uid, d.reply_to_mailbox, re.thread_id,
        d.updated_epoch, d.remote_uid, d.pushed_epoch, d.cc_raw, d.bcc_raw,
        d.body_html, d.important
 FROM drafts d
 LEFT JOIN mailboxes rm ON rm.account_id = d.account_id AND rm.name = d.reply_to_mailbox
 LEFT JOIN envelopes re ON re.mailbox_id = rm.id AND re.uid = d.reply_to_uid";

fn row_to_draft(row: &rusqlite::Row<'_>) -> rusqlite::Result<SavedDraft> {
    Ok(SavedDraft {
        id: row.get(0)?,
        account_id: row.get(1)?,
        to_raw: row.get(2)?,
        subject: row.get(3)?,
        body: row.get(4)?,
        reply_to_uid: row.get(5)?,
        reply_to_mailbox: row.get(6)?,
        thread_id: row.get(7)?,
        updated_epoch: row.get(8)?,
        remote_uid: row.get(9)?,
        pushed_epoch: row.get(10)?,
        cc_raw: row.get(11)?,
        bcc_raw: row.get(12)?,
        body_html: row.get(13)?,
        important: row.get(14)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (Store, i64) {
        let store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("test@example.com", "gmail")
            .unwrap();
        (store, account)
    }

    /// R3 (PLAN-RETOURS-6): the "important" mark follows the draft — a
    /// resume finds it again — and toggling it ALONE is a real
    /// modification (the timestamp advances, the mirror will follow;
    /// otherwise the anti-churn guard would swallow the gesture).
    #[test]
    fn save_draft_roundtrips_important_and_toggling_counts() {
        let (store, account) = store();
        let content = |important: bool| DraftContent {
            to_raw: "a@b.fr",
            cc_raw: "",
            bcc_raw: "",
            subject: "Subject",
            body: "body",
            body_html: None,
            reply_to_uid: None,
            reply_to_mailbox: None,
            important,
        };
        let saved = store
            .save_draft(account, None, None, content(true))
            .unwrap();
        assert!(
            store.drafts().unwrap()[0].important,
            "the resume finds the mark again"
        );

        let again = store
            .save_draft(
                account,
                Some(saved.id),
                Some(saved.updated_epoch),
                content(false),
            )
            .unwrap();
        assert!(
            again.updated_epoch > saved.updated_epoch,
            "toggling the mark alone advances the timestamp"
        );
        assert!(!store.drafts().unwrap()[0].important);
    }

    #[test]
    fn saves_raw_unvalidated_content_and_roundtrips() {
        let (store, account) = store();
        let id = store
            .save_draft(
                account,
                None,
                None,
                DraftContent {
                    to_raw: "incomplete-addr",
                    cc_raw: "",
                    bcc_raw: "",
                    body_html: None,
                    subject: "Subject",
                    body: "body\non two lines",
                    reply_to_uid: Some(42),
                    reply_to_mailbox: None,
                    important: false,
                },
            )
            .unwrap()
            .id;

        let drafts = store.drafts().unwrap();
        assert_eq!(drafts.len(), 1);
        let draft = &drafts[0];
        assert_eq!(draft.id, id);
        assert_eq!(
            draft.to_raw, "incomplete-addr",
            "the raw value is kept as is"
        );
        assert_eq!(draft.subject, "Subject");
        assert_eq!(draft.body, "body\non two lines");
        assert_eq!(draft.reply_to_uid, Some(42));
    }

    /// PLAN-COMPOSITION-HTML: the rich body survives the save and the
    /// re-read; a draft without HTML reads back `None` — never a
    /// pseudo-content (it is `None` that says "text path").
    #[test]
    fn save_draft_roundtrips_body_html() {
        let (rich_store, rich_account) = store();
        rich_store
            .save_draft(
                rich_account,
                None,
                None,
                DraftContent {
                    to_raw: "a@b.fr",
                    cc_raw: "",
                    bcc_raw: "",
                    subject: "Subject",
                    body: "bold",
                    body_html: Some("<b>bold</b>"),
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                    important: false,
                },
            )
            .unwrap();
        let rich = &rich_store.drafts().unwrap()[0];
        assert_eq!(rich.body_html.as_deref(), Some("<b>bold</b>"));
        assert_eq!(rich.body, "bold", "the derived text stays populated");

        let (plain_store, account) = store();
        plain_store
            .save_draft(
                account,
                None,
                None,
                DraftContent {
                    to_raw: "a@b.fr",
                    cc_raw: "",
                    bcc_raw: "",
                    subject: "Subject",
                    body: "text",
                    body_html: None,
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                    important: false,
                },
            )
            .unwrap();
        assert_eq!(plain_store.drafts().unwrap()[0].body_html, None);
    }

    /// The pull preserves formatting: a rich draft pushed then pulled
    /// back comes back WITH its HTML — and is born clean (not to
    /// re-push), like any import.
    #[test]
    fn import_remote_draft_keeps_the_rich_body() {
        let (store, account) = store();
        store
            .import_remote_draft(account, 42, "a@b.fr", "s", "bold", Some("<b>bold</b>"))
            .unwrap();
        let draft = &store.drafts().unwrap()[0];
        assert_eq!(draft.body_html.as_deref(), Some("<b>bold</b>"));
        assert_eq!(draft.body, "bold");
        assert!(
            store.drafts_to_push(account).unwrap().is_empty(),
            "an import is born clean"
        );
    }

    /// The rich body counts in the "identical content" detection: a
    /// formatting change ALONE (same derived text) must re-push the
    /// draft — otherwise the Gmail mirror would keep the previous
    /// version.
    #[test]
    fn body_html_change_marks_the_draft_dirty() {
        let (store, account) = store();
        let content = |html: Option<&'static str>| DraftContent {
            to_raw: "a@b.fr",
            cc_raw: "",
            bcc_raw: "",
            subject: "s",
            body: "text",
            body_html: html,
            reply_to_uid: None,
            reply_to_mailbox: None,
            important: false,
        };
        let id = store
            .save_draft(account, None, None, content(Some("text")))
            .unwrap()
            .id;
        let epoch = store.drafts_to_push(account).unwrap()[0].updated_epoch;
        store.record_draft_pushed(id, Some(101), epoch).unwrap();

        store
            .save_draft(account, Some(id), None, content(Some("<b>text</b>")))
            .unwrap();
        assert_eq!(
            store.drafts_to_push(account).unwrap().len(),
            1,
            "formatting alone must re-push"
        );
    }

    /// A54: Cc/Bcc survive the save and the re-read — the layer that
    /// prevents their loss when a draft is resumed. A draft with no
    /// copy reads back EMPTY strings, never a pseudo-content.
    #[test]
    fn save_draft_roundtrips_cc_and_bcc() {
        let (with_store, account) = store();
        with_store
            .save_draft(
                account,
                None,
                None,
                DraftContent {
                    to_raw: "a@b.fr",
                    cc_raw: "c@b.fr, d@b.fr",
                    bcc_raw: "secret@b.fr",
                    body_html: None,
                    subject: "Subject",
                    body: "body",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                    important: false,
                },
            )
            .unwrap();
        let with = &with_store.drafts().unwrap()[0];
        assert_eq!(with.cc_raw, "c@b.fr, d@b.fr");
        assert_eq!(with.bcc_raw, "secret@b.fr");

        let (without_store, account) = store();
        without_store
            .save_draft(
                account,
                None,
                None,
                DraftContent {
                    to_raw: "a@b.fr",
                    cc_raw: "",
                    bcc_raw: "",
                    body_html: None,
                    subject: "Subject",
                    body: "body",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                    important: false,
                },
            )
            .unwrap();
        let without = &without_store.drafts().unwrap()[0];
        assert_eq!(without.cc_raw, "");
        assert_eq!(without.bcc_raw, "");
    }

    #[test]
    fn save_with_id_updates_in_place() {
        let (store, account) = store();
        let id = store
            .save_draft(
                account,
                None,
                None,
                DraftContent {
                    to_raw: "",
                    cc_raw: "",
                    bcc_raw: "",
                    body_html: None,
                    subject: "v1",
                    body: "text",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                    important: false,
                },
            )
            .unwrap()
            .id;
        let same = store
            .save_draft(
                account,
                Some(id),
                None,
                DraftContent {
                    to_raw: "a@b.fr",
                    cc_raw: "",
                    bcc_raw: "",
                    body_html: None,
                    subject: "v2",
                    body: "enriched text",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                    important: false,
                },
            )
            .unwrap()
            .id;

        assert_eq!(same, id);
        let drafts = store.drafts().unwrap();
        assert_eq!(drafts.len(), 1, "update, not duplication");
        assert_eq!(drafts[0].subject, "v2");
        assert_eq!(drafts[0].to_raw, "a@b.fr");
    }

    /// The safety net must never have a missing mesh: a stale id (draft
    /// deleted in the meantime) re-inserts instead of losing the text.
    #[test]
    fn save_with_stale_id_still_persists_the_text() {
        let (store, account) = store();
        let id = store
            .save_draft(
                account,
                None,
                None,
                DraftContent {
                    to_raw: "",
                    cc_raw: "",
                    bcc_raw: "",
                    body_html: None,
                    subject: "s",
                    body: "precious",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                    important: false,
                },
            )
            .unwrap()
            .id;
        store.delete_draft(id).unwrap();

        store
            .save_draft(
                account,
                Some(id),
                None,
                DraftContent {
                    to_raw: "",
                    cc_raw: "",
                    bcc_raw: "",
                    body_html: None,
                    subject: "s",
                    body: "precious",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                    important: false,
                },
            )
            .unwrap();

        let drafts = store.drafts().unwrap();
        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].body, "precious");
    }

    #[test]
    fn drafts_lists_most_recent_first() {
        let (store, account) = store();
        store
            .save_draft(
                account,
                None,
                None,
                DraftContent {
                    to_raw: "",
                    cc_raw: "",
                    bcc_raw: "",
                    body_html: None,
                    subject: "first",
                    body: "a",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                    important: false,
                },
            )
            .unwrap();
        store
            .save_draft(
                account,
                None,
                None,
                DraftContent {
                    to_raw: "",
                    cc_raw: "",
                    bcc_raw: "",
                    body_html: None,
                    subject: "second",
                    body: "b",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                    important: false,
                },
            )
            .unwrap();

        let drafts = store.drafts().unwrap();
        let subjects: Vec<&str> = drafts.iter().map(|draft| draft.subject.as_str()).collect();
        assert_eq!(subjects, vec!["second", "first"]);
    }

    #[test]
    fn delete_draft_removes_it() {
        let (store, account) = store();
        let id = store
            .save_draft(
                account,
                None,
                None,
                DraftContent {
                    to_raw: "",
                    cc_raw: "",
                    bcc_raw: "",
                    body_html: None,
                    subject: "s",
                    body: "b",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                    important: false,
                },
            )
            .unwrap()
            .id;
        store.delete_draft(id).unwrap();
        assert!(store.drafts().unwrap().is_empty());
    }

    #[test]
    fn fresh_and_edited_drafts_are_to_push_until_recorded() {
        let (store, account) = store();
        let id = store
            .save_draft(
                account,
                None,
                None,
                DraftContent {
                    to_raw: "",
                    cc_raw: "",
                    bcc_raw: "",
                    body_html: None,
                    subject: "s",
                    body: "v1",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                    important: false,
                },
            )
            .unwrap()
            .id;
        assert_eq!(
            store.drafts_to_push(account).unwrap().len(),
            1,
            "fresh = to push"
        );

        let draft = &store.drafts_to_push(account).unwrap()[0];
        store
            .record_draft_pushed(id, Some(101), draft.updated_epoch)
            .unwrap();
        assert!(
            store.drafts_to_push(account).unwrap().is_empty(),
            "pushed = clean"
        );

        store
            .save_draft(
                account,
                Some(id),
                None,
                DraftContent {
                    to_raw: "",
                    cc_raw: "",
                    bcc_raw: "",
                    body_html: None,
                    subject: "s",
                    body: "v2",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                    important: false,
                },
            )
            .unwrap();
        assert_eq!(
            store.drafts_to_push(account).unwrap().len(),
            1,
            "edited = to push again"
        );
    }

    /// A save with identical content marks nothing to push: otherwise
    /// every close of a composition would re-push a byte-for-byte
    /// identical copy to Gmail (churn observed in the field).
    #[test]
    fn identical_resave_does_not_mark_dirty_again() {
        let (store, account) = store();
        let id = store
            .save_draft(
                account,
                None,
                None,
                DraftContent {
                    to_raw: "a@b.fr",
                    cc_raw: "",
                    bcc_raw: "",
                    body_html: None,
                    subject: "s",
                    body: "text",
                    reply_to_uid: Some(1),
                    reply_to_mailbox: None,
                    important: false,
                },
            )
            .unwrap()
            .id;
        let epoch = store.drafts_to_push(account).unwrap()[0].updated_epoch;
        store.record_draft_pushed(id, Some(101), epoch).unwrap();

        store
            .save_draft(
                account,
                Some(id),
                None,
                DraftContent {
                    to_raw: "a@b.fr",
                    cc_raw: "",
                    bcc_raw: "",
                    body_html: None,
                    subject: "s",
                    body: "text",
                    reply_to_uid: Some(1),
                    reply_to_mailbox: None,
                    important: false,
                },
            )
            .unwrap();

        assert!(
            store.drafts_to_push(account).unwrap().is_empty(),
            "identical content: nothing to re-push"
        );
    }

    /// The anti-loss invariant: an edit DURING the push leaves the
    /// draft still to push — the marker is a snapshot, not a flag.
    #[test]
    fn edit_during_push_stays_dirty() {
        let (store, account) = store();
        let id = store
            .save_draft(
                account,
                None,
                None,
                DraftContent {
                    to_raw: "",
                    cc_raw: "",
                    bcc_raw: "",
                    body_html: None,
                    subject: "s",
                    body: "v1",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                    important: false,
                },
            )
            .unwrap()
            .id;
        let snapshot = store.drafts_to_push(account).unwrap()[0].updated_epoch;

        // The user edits while the push is in flight — even within the
        // same millisecond, the strictly increasing timestamp makes the
        // edit detectable…
        store
            .save_draft(
                account,
                Some(id),
                None,
                DraftContent {
                    to_raw: "",
                    cc_raw: "",
                    bcc_raw: "",
                    body_html: None,
                    subject: "s",
                    body: "v2 edited in flight",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                    important: false,
                },
            )
            .unwrap();
        // …then the push (of v1) succeeds and records with ITS snapshot.
        store.record_draft_pushed(id, Some(101), snapshot).unwrap();

        let to_push = store.drafts_to_push(account).unwrap();
        assert_eq!(to_push.len(), 1, "v2 must go out again on the next cycle");
        assert_eq!(to_push[0].body, "v2 edited in flight");
    }

    #[test]
    fn replacement_tombstones_the_previous_remote_copy() {
        let (store, account) = store();
        let id = store
            .save_draft(
                account,
                None,
                None,
                DraftContent {
                    to_raw: "",
                    cc_raw: "",
                    bcc_raw: "",
                    body_html: None,
                    subject: "s",
                    body: "v1",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                    important: false,
                },
            )
            .unwrap()
            .id;
        store.record_draft_pushed(id, Some(101), 1).unwrap();

        store.record_draft_pushed(id, Some(202), 2).unwrap();

        assert_eq!(store.draft_tombstones(account).unwrap(), vec![101]);
        store.clear_draft_tombstone(account, 101).unwrap();
        assert!(store.draft_tombstones(account).unwrap().is_empty());
    }

    #[test]
    fn delete_tombstones_the_remote_copy_but_only_if_pushed() {
        let (store, account) = store();
        let pushed = store
            .save_draft(
                account,
                None,
                None,
                DraftContent {
                    to_raw: "",
                    cc_raw: "",
                    bcc_raw: "",
                    body_html: None,
                    subject: "pushed",
                    body: "b",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                    important: false,
                },
            )
            .unwrap()
            .id;
        store.record_draft_pushed(pushed, Some(303), 1).unwrap();
        let local_only = store
            .save_draft(
                account,
                None,
                None,
                DraftContent {
                    to_raw: "",
                    cc_raw: "",
                    bcc_raw: "",
                    body_html: None,
                    subject: "local",
                    body: "b",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                    important: false,
                },
            )
            .unwrap()
            .id;

        store.delete_draft(pushed).unwrap();
        store.delete_draft(local_only).unwrap();

        assert_eq!(
            store.draft_tombstones(account).unwrap(),
            vec![303],
            "never a tombstone without a registered remote copy"
        );
    }

    /// The UIDVALIDITY guard is PER ACCOUNT: resetting A's markers
    /// touches neither B's markers nor B's tombstones.
    #[test]
    fn align_resets_only_the_given_account() {
        let (store, account) = store();
        let other = store
            .adopt_or_create_account("other@example.com", "gmail")
            .unwrap();
        let draft_a = store
            .save_draft(
                account,
                None,
                None,
                DraftContent {
                    to_raw: "",
                    cc_raw: "",
                    bcc_raw: "",
                    body_html: None,
                    subject: "a",
                    body: "x",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                    important: false,
                },
            )
            .unwrap()
            .id;
        let draft_b = store
            .save_draft(
                other,
                None,
                None,
                DraftContent {
                    to_raw: "",
                    cc_raw: "",
                    bcc_raw: "",
                    body_html: None,
                    subject: "b",
                    body: "y",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                    important: false,
                },
            )
            .unwrap()
            .id;
        let epoch_a = store.drafts_to_push(account).unwrap()[0].updated_epoch;
        store
            .record_draft_pushed(draft_a, Some(11), epoch_a)
            .unwrap();
        let epoch_b = store.drafts_to_push(other).unwrap()[0].updated_epoch;
        store
            .record_draft_pushed(draft_b, Some(22), epoch_b)
            .unwrap();
        store.align_drafts_uidvalidity(account, 5).unwrap();
        store.align_drafts_uidvalidity(other, 7).unwrap();

        assert!(
            store.align_drafts_uidvalidity(account, 6).unwrap(),
            "A reset"
        );

        assert_eq!(
            store.drafts_to_push(account).unwrap().len(),
            1,
            "A must re-push everything"
        );
        assert!(
            store.drafts_to_push(other).unwrap().is_empty(),
            "B is not affected"
        );
        let drafts = store.drafts().unwrap();
        let of_b = drafts.iter().find(|draft| draft.id == draft_b).unwrap();
        assert_eq!(of_b.remote_uid, Some(22), "B's markers survive");
    }

    /// UIDVALIDITY changed: we abandon all markers — a duplicate is
    /// acceptable, deleting the wrong UID never is.
    #[test]
    fn uidvalidity_change_resets_remote_state() {
        let (store, account) = store();
        assert!(
            !store.align_drafts_uidvalidity(account, 7).unwrap(),
            "first view"
        );
        let id = store
            .save_draft(
                account,
                None,
                None,
                DraftContent {
                    to_raw: "",
                    cc_raw: "",
                    bcc_raw: "",
                    body_html: None,
                    subject: "s",
                    body: "b",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                    important: false,
                },
            )
            .unwrap()
            .id;
        store.record_draft_pushed(id, Some(404), 1).unwrap();
        store.record_draft_pushed(id, Some(505), 2).unwrap(); // 404 becomes a tombstone

        assert!(
            !store.align_drafts_uidvalidity(account, 7).unwrap(),
            "unchanged"
        );
        assert!(
            store.align_drafts_uidvalidity(account, 8).unwrap(),
            "changed: reset"
        );

        assert!(store.draft_tombstones(account).unwrap().is_empty());
        let drafts = store.drafts().unwrap();
        assert_eq!(drafts[0].remote_uid, None);
        assert_eq!(
            store.drafts_to_push(account).unwrap().len(),
            1,
            "everything is to re-push"
        );
    }
}

/// Concurrent editing: two writers on the same draft.
#[cfg(test)]
mod tests_concurrency {
    use super::*;

    fn store() -> (Store, i64) {
        let store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("test@example.com", "gmail")
            .unwrap();
        (store, account)
    }

    /// THE field defect: the composer holds an in-memory copy, the pull
    /// replaces the draft out from under it, and the next save used to
    /// overwrite the version that came from elsewhere.
    ///
    /// Both texts must survive. This is the module's golden rule — a
    /// duplicate is acceptable, lost text never is — applied to
    /// concurrent editing.
    #[test]
    fn a_concurrent_edit_keeps_both_texts() {
        let (store, account) = store();
        let open = store
            .save_draft(
                account,
                None,
                None,
                DraftContent {
                    to_raw: "a@b.fr",
                    cc_raw: "",
                    bcc_raw: "",
                    body_html: None,
                    subject: "Quote",
                    body: "composer version",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                    important: false,
                },
            )
            .unwrap();

        // Someone else writes: the pull, in practice.
        store
            .save_draft(
                account,
                Some(open.id),
                None,
                DraftContent {
                    to_raw: "a@b.fr",
                    cc_raw: "",
                    bcc_raw: "",
                    body_html: None,
                    subject: "Quote",
                    body: "version from elsewhere",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                    important: false,
                },
            )
            .unwrap();

        // The composer saves, believing it is modifying what it read.
        let outcome = store
            .save_draft(
                account,
                Some(open.id),
                Some(open.updated_epoch),
                DraftContent {
                    to_raw: "a@b.fr",
                    cc_raw: "",
                    bcc_raw: "",
                    body_html: None,
                    subject: "Quote",
                    body: "composer version",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                    important: false,
                },
            )
            .unwrap();

        assert!(
            outcome.forked,
            "the text from the other side is not overwritten"
        );
        assert_ne!(outcome.id, open.id, "it is kept apart");
        let texts: Vec<String> = store
            .drafts()
            .unwrap()
            .into_iter()
            .map(|draft| draft.body)
            .collect();
        assert_eq!(texts.len(), 2);
        assert!(texts.contains(&"composer version".to_string()));
        assert!(texts.contains(&"version from elsewhere".to_string()));
    }

    /// The round trip: the returned timestamp must allow chaining saves
    /// without triggering a false conflict. The trap is real — a save
    /// with identical content does NOT touch the timestamp, so
    /// returning "now" would make the editor diverge from the database.
    #[test]
    fn the_returned_timestamp_allows_chaining_saves() {
        let (store, account) = store();
        let mut outcome = store
            .save_draft(
                account,
                None,
                None,
                DraftContent {
                    to_raw: "a@b.fr",
                    cc_raw: "",
                    bcc_raw: "",
                    body_html: None,
                    subject: "Quote",
                    body: "one",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                    important: false,
                },
            )
            .unwrap();

        for text in ["two", "two", "three"] {
            outcome = store
                .save_draft(
                    account,
                    Some(outcome.id),
                    Some(outcome.updated_epoch),
                    DraftContent {
                        to_raw: "a@b.fr",
                        cc_raw: "",
                        bcc_raw: "",
                        body_html: None,
                        subject: "Quote",
                        body: text,
                        reply_to_uid: None,
                        reply_to_mailbox: None,
                        important: false,
                    },
                )
                .unwrap();
            assert!(!outcome.forked, "no conflict: it is the same editor");
        }
        assert_eq!(store.drafts().unwrap().len(), 1);
    }

    /// Without `base_epoch`, nothing changes: callers that hold no
    /// in-memory copy have nothing to overwrite.
    #[test]
    fn without_a_reference_timestamp_the_save_updates_in_place() {
        let (store, account) = store();
        let first = store
            .save_draft(
                account,
                None,
                None,
                DraftContent {
                    to_raw: "a@b.fr",
                    cc_raw: "",
                    bcc_raw: "",
                    body_html: None,
                    subject: "Quote",
                    body: "one",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                    important: false,
                },
            )
            .unwrap();
        let second = store
            .save_draft(
                account,
                Some(first.id),
                None,
                DraftContent {
                    to_raw: "a@b.fr",
                    cc_raw: "",
                    bcc_raw: "",
                    body_html: None,
                    subject: "Quote",
                    body: "two",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                    important: false,
                },
            )
            .unwrap();

        assert!(!second.forked);
        assert_eq!(second.id, first.id);
        assert_eq!(store.drafts().unwrap().len(), 1);
    }

    /// The pull does NOT update the draft: it **replaces** it
    /// ([`plan_draft_pull`]) — the stale mirror is removed, the fresh
    /// version arrives under a new identifier. The composer, meanwhile,
    /// still holds the old one: the row it believes it is modifying no
    /// longer exists.
    ///
    /// This is a conflict just as much as an in-place rewrite, and the
    /// only one the field actually produces. Detection did not see it:
    /// it compares two timestamps, and there is now only one left.
    /// Reported symptom: "the red message doesn't show up."
    #[test]
    fn a_draft_replaced_by_the_pull_is_also_a_conflict() {
        let (store, account) = store();
        let open = store
            .save_draft(
                account,
                None,
                None,
                DraftContent {
                    to_raw: "a@b.fr",
                    cc_raw: "",
                    bcc_raw: "",
                    body_html: None,
                    subject: "Quote",
                    body: "composer version",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                    important: false,
                },
            )
            .unwrap();
        // A MORE RECENT draft exists, and that is what makes the defect
        // visible. SQLite assigns `max(rowid) + 1`: if the edited draft
        // were the last one, the import would reclaim the identifier it
        // just freed, the row would reappear under the composer, and
        // detection would land on its feet **by accident**. A single
        // younger draft is enough to remove that coincidence — hence a
        // defect that only shows up half the time, exactly what the
        // field reported.
        store
            .save_draft(
                account,
                None,
                None,
                DraftContent {
                    to_raw: "x@y.fr",
                    cc_raw: "",
                    bcc_raw: "",
                    body_html: None,
                    subject: "Other",
                    body: "z",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                    important: false,
                },
            )
            .unwrap();
        store
            .record_draft_pushed(open.id, Some(7), open.updated_epoch)
            .unwrap();

        // Edited again in the webmail: the server expunges 7 and creates 8.
        let local = store.drafts_of(account).unwrap();
        let plan = plan_draft_pull(&local, &[8], &[]);
        assert_eq!(plan.stale, vec![open.id], "the stale mirror leaves");
        for id in plan.stale {
            store.drop_stale_draft(id).unwrap();
        }
        for uid in plan.fetch {
            store
                .import_remote_draft(
                    account,
                    uid,
                    "a@b.fr",
                    "Quote",
                    "version from elsewhere",
                    None,
                )
                .unwrap();
        }

        // The composer closes and saves what it was holding.
        let outcome = store
            .save_draft(
                account,
                Some(open.id),
                Some(open.updated_epoch),
                DraftContent {
                    to_raw: "a@b.fr",
                    cc_raw: "",
                    bcc_raw: "",
                    body_html: None,
                    subject: "Quote",
                    body: "composer version",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                    important: false,
                },
            )
            .unwrap();

        assert!(
            outcome.forked,
            "the targeted row had disappeared under the composer: staying \
             silent leaves the user with two texts without knowing it"
        );
        let texts: Vec<String> = store
            .drafts()
            .unwrap()
            .into_iter()
            .map(|draft| draft.body)
            .collect();
        assert!(texts.contains(&"composer version".to_string()));
        assert!(texts.contains(&"version from elsewhere".to_string()));
    }
}

/// The pull has its own scenarios: they share neither fixtures nor
/// invariants with those of the push, above.
#[cfg(test)]
mod tests_pull {
    use super::*;

    fn draft(id: i64, remote_uid: Option<Uid>, updated: i64, pushed: Option<i64>) -> SavedDraft {
        SavedDraft {
            id,
            account_id: 1,
            to_raw: "alice@example.com".to_string(),
            cc_raw: String::new(),
            bcc_raw: String::new(),
            body_html: None,
            subject: "Quote".to_string(),
            body: "Hello".to_string(),
            reply_to_uid: None,
            reply_to_mailbox: None,
            important: false,
            thread_id: None,
            updated_epoch: updated,
            remote_uid,
            pushed_epoch: pushed,
        }
    }

    /// The draft written in the webmail: nobody knows it here.
    #[test]
    fn an_unknown_remote_draft_is_pulled_back() {
        let plan = plan_draft_pull(&[], &[7], &[]);
        assert_eq!(plan.fetch, vec![7]);
        assert!(plan.stale.is_empty());
    }

    /// Our own pushed copy must not come back: without this guard,
    /// every cycle would duplicate the drafts mailbox.
    #[test]
    fn our_own_pushed_copy_is_not_pulled_back() {
        let plan = plan_draft_pull(&[draft(1, Some(7), 100, Some(100))], &[7], &[]);
        assert!(plan.fetch.is_empty());
        assert!(plan.stale.is_empty(), "the mirror is up to date");
    }

    /// A copy we asked to delete but haven't purged yet is still there:
    /// pulling it back would resurrect a discarded draft.
    #[test]
    fn a_copy_awaiting_purge_is_not_pulled_back() {
        let plan = plan_draft_pull(&[], &[7], &[7]);
        assert!(plan.fetch.is_empty());
    }

    /// Editing a draft elsewhere: the server expunges the old message
    /// and creates a new one. The same pass must therefore remove the
    /// stale mirror AND pull the fresh version — replace, not double.
    #[test]
    fn editing_elsewhere_replaces_the_mirror_instead_of_doubling_it() {
        let plan = plan_draft_pull(&[draft(1, Some(7), 100, Some(100))], &[8], &[]);
        assert_eq!(plan.fetch, vec![8]);
        assert_eq!(plan.stale, vec![1]);
    }

    /// THE module's rule: a draft edited here carries text the server
    /// has never seen. It cannot be "stale."
    #[test]
    fn a_draft_edited_here_is_never_removed() {
        // Pushed at 100, edited again at 150: the remote copy is behind.
        let plan = plan_draft_pull(&[draft(1, Some(7), 150, Some(100))], &[8], &[]);
        assert!(
            plan.stale.is_empty(),
            "removing it would erase the local edit"
        );
    }

    /// A draft never pushed has no mirror: nothing to compare.
    #[test]
    fn a_draft_never_pushed_is_never_removed() {
        let plan = plan_draft_pull(&[draft(1, None, 100, None)], &[8], &[]);
        assert!(plan.stale.is_empty());
    }

    /// The safeguard. An empty list has exactly the shape of a partial
    /// failure, and getting it wrong here costs text. If the user
    /// really deleted everything elsewhere, their copies survive
    /// locally: a duplicate, not a loss.
    #[test]
    fn an_empty_remote_list_removes_nothing() {
        let local = [
            draft(1, Some(7), 100, Some(100)),
            draft(2, Some(8), 100, Some(100)),
        ];
        let plan = plan_draft_pull(&local, &[], &[]);
        assert!(plan.stale.is_empty(), "an empty folder proves nothing");
        assert!(plan.fetch.is_empty());
    }

    /// Several accounts, several drafts: the plan stays stable and
    /// mixes nothing up. The caller already filters by account.
    #[test]
    fn the_plan_handles_several_drafts_without_mixing_them_up() {
        let local = [
            draft(1, Some(7), 100, Some(100)), // up to date
            draft(2, Some(8), 100, Some(100)), // gone from the server
            draft(3, Some(9), 200, Some(100)), // edited here
            draft(4, None, 100, None),         // never pushed
        ];
        let plan = plan_draft_pull(&local, &[7, 42], &[]);
        assert_eq!(plan.fetch, vec![42]);
        assert_eq!(plan.stale, vec![2]);
    }
}

/// The draft -> conversation link (PLAN-BROUILLONS, B-D2): resolved at
/// read time, never stored — and never guessed (ADR 0009: a UID
/// without its mailbox names nothing).
#[cfg(test)]
mod tests_thread {
    use chrono::TimeZone;

    use super::*;
    use crate::envelope::Envelope;

    fn store() -> (Store, i64) {
        let store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("test@example.com", "gmail")
            .unwrap();
        (store, account)
    }

    fn message(uid: Uid, subject: &str) -> Envelope {
        Envelope {
            reply_to: None,
            uid,
            subject: Some(subject.to_string()),
            sender: Some("Marie Dubois".to_string()),
            sender_address: Some("marie@example.com".to_string()),
            message_id: Some(format!("<m{uid}@example.com>")),
            in_reply_to: None,
            date: Some(chrono::Utc.timestamp_opt(1_700_000_000, 0).unwrap()),
            seen: true,
            flagged: false,
            to_addrs: Vec::new(),
            cc_addrs: Vec::new(),
        }
    }

    fn reply<'a>(uid: Option<Uid>, mailbox: Option<&'a str>) -> DraftContent<'a> {
        DraftContent {
            to_raw: "marie@example.com",
            cc_raw: "",
            bcc_raw: "",
            body_html: None,
            subject: "Re: Quote",
            body: "Hello Marie,",
            reply_to_uid: uid,
            reply_to_mailbox: mailbox,
            important: false,
        }
    }

    #[test]
    fn a_reply_draft_links_to_its_thread() {
        let (mut store, account) = store();
        let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        store
            .upsert_envelopes(inbox, &[message(42, "Quote")])
            .unwrap();
        store
            .save_draft(account, None, None, reply(Some(42), Some("INBOX")))
            .unwrap();

        let thread = store.unified_recent(0, 10).unwrap()[0].thread_id;
        assert!(thread.is_some(), "the fixture must carry a thread");
        let drafts = store.drafts().unwrap();
        assert_eq!(
            drafts[0].thread_id, thread,
            "same thread as the targeted message"
        );
        assert_eq!(drafts[0].reply_to_mailbox.as_deref(), Some("INBOX"));
    }

    #[test]
    fn a_free_composition_stays_threadless() {
        let (store, account) = store();
        store
            .save_draft(account, None, None, reply(None, None))
            .unwrap();
        assert_eq!(store.drafts().unwrap()[0].thread_id, None);
    }

    /// The target can be missing in two ways — mailbox never seen
    /// (renamed, account reorganized) or message purged — and neither
    /// must make the draft disappear: it stays, simply without a thread.
    #[test]
    fn an_unknown_mailbox_or_a_purged_message_leave_it_threadless() {
        let (mut store, account) = store();
        let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        store
            .upsert_envelopes(inbox, &[message(42, "Quote")])
            .unwrap();
        store
            .save_draft(account, None, None, reply(Some(42), Some("Elsewhere")))
            .unwrap();
        store
            .save_draft(account, None, None, reply(Some(99), Some("INBOX")))
            .unwrap();

        let drafts = store.drafts().unwrap();
        assert_eq!(drafts.len(), 2, "the drafts survive the target");
        assert!(drafts.iter().all(|draft| draft.thread_id.is_none()));
    }

    /// Drafts from before the column: `reply_to_uid` without a mailbox.
    /// They NEVER link up — a bare UID could point at the wrong message
    /// (ADR 0009); their safety net stays the folder.
    #[test]
    fn a_draft_from_before_the_column_stays_threadless() {
        let (mut store, account) = store();
        let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        store
            .upsert_envelopes(inbox, &[message(42, "Quote")])
            .unwrap();
        store
            .conn()
            .execute(
                "INSERT INTO drafts (account_id, to_raw, subject, body, reply_to_uid, updated_epoch)
                 VALUES (?1, '', 'Re: Quote', 'b', 42, 1)",
                [account],
            )
            .unwrap();
        assert_eq!(store.drafts().unwrap()[0].thread_id, None);
    }

    /// The anti-churn WHERE covers the new column: fixing ONLY the
    /// targeted mailbox must mark the draft to push again.
    #[test]
    fn changing_the_targeted_mailbox_marks_the_draft_to_push() {
        let (store, account) = store();
        let saved = store
            .save_draft(account, None, None, reply(Some(42), Some("INBOX")))
            .unwrap();
        store
            .record_draft_pushed(saved.id, Some(7), saved.updated_epoch)
            .unwrap();
        assert!(store.drafts_to_push(account).unwrap().is_empty());

        store
            .save_draft(
                account,
                Some(saved.id),
                None,
                reply(Some(42), Some("Archives")),
            )
            .unwrap();
        assert_eq!(store.drafts_to_push(account).unwrap().len(), 1);
    }
}

#[cfg(test)]
mod tests_attachments {
    use super::*;

    const MB: u64 = 1024 * 1024;

    fn store_with_draft() -> (Store, i64) {
        let store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("test@example.com", "gmail")
            .unwrap();
        let id = store
            .save_draft(
                account,
                None,
                None,
                DraftContent {
                    to_raw: "vous@example.com",
                    cc_raw: "",
                    bcc_raw: "",
                    body_html: None,
                    subject: "Photos",
                    body: "body",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                    important: false,
                },
            )
            .unwrap()
            .id;
        (store, id)
    }

    fn table_rows(store: &Store, draft_id: i64) -> i64 {
        store
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM draft_attachments WHERE draft_id = ?1",
                [draft_id],
                |row| row.get(0),
            )
            .unwrap()
    }

    #[test]
    fn attach_stores_bytes_and_meta_lists_them_in_gesture_order() {
        let (store, draft) = store_with_draft();
        store
            .add_draft_attachment(draft, "facade.jpg", "image/jpeg", &[1, 2, 3])
            .unwrap();
        store
            .add_draft_attachment(draft, "devis.pdf", "application/pdf", &[4, 5])
            .unwrap();

        let meta = store.draft_attachments_meta(draft).unwrap();
        assert_eq!(meta.len(), 2);
        assert_eq!(meta[0].name, "facade.jpg");
        assert_eq!(meta[0].mime, "image/jpeg");
        assert_eq!(meta[0].size, 3);
        assert_eq!(meta[1].name, "devis.pdf");
        assert_eq!(meta[1].size, 2);
    }

    /// PJ-D3: the refusal happens at the gesture — the attachment that
    /// goes over never enters, the attachments already acquired stay,
    /// and the error states the remaining room.
    #[test]
    fn over_budget_attachment_is_refused_and_earlier_pieces_stay() {
        let (store, draft) = store_with_draft();
        store
            .add_draft_attachment(
                draft,
                "a.zip",
                "application/zip",
                &vec![0u8; (13 * MB) as usize],
            )
            .unwrap();

        let refused = store.add_draft_attachment(
            draft,
            "b.zip",
            "application/zip",
            &vec![0u8; (13 * MB) as usize],
        );

        match refused {
            Err(Error::AttachmentOverBudget {
                name,
                size,
                remaining,
            }) => {
                assert_eq!(name, "b.zip");
                assert_eq!(size, 13 * MB);
                assert_eq!(remaining, MAX_ATTACHMENTS_BYTES - 13 * MB);
            }
            other => panic!("expected a refusal at the cap, got {other:?}"),
        }
        let meta = store.draft_attachments_meta(draft).unwrap();
        assert_eq!(meta.len(), 1, "the acquired one is not punished");
        assert_eq!(meta[0].name, "a.zip");
    }

    /// The bound is INCLUSIVE: exactly 25 MB passes, one byte more does not.
    #[test]
    fn budget_boundary_is_inclusive() {
        let (store, draft) = store_with_draft();
        store
            .add_draft_attachment(
                draft,
                "stack.bin",
                "application/octet-stream",
                &vec![0u8; MAX_ATTACHMENTS_BYTES as usize],
            )
            .unwrap();
        let refused = store.add_draft_attachment(draft, "drop.txt", "text/plain", &[0u8]);
        assert!(matches!(
            refused,
            Err(Error::AttachmentOverBudget { remaining: 0, .. })
        ));
    }

    /// PJ-D1: discarding the draft carries its bytes away (cascade).
    #[test]
    fn deleting_the_draft_cascades_to_its_attachments() {
        let (store, draft) = store_with_draft();
        store
            .add_draft_attachment(draft, "f.pdf", "application/pdf", &[1])
            .unwrap();
        assert_eq!(table_rows(&store, draft), 1);

        store.delete_draft(draft).unwrap();

        assert_eq!(table_rows(&store, draft), 0, "no orphaned blob");
    }

    /// The gesture marks the draft modified (PJ-D6) and returns the new
    /// timestamp — the editor takes it as `base_epoch`, otherwise its
    /// next save would be accused of a phantom conflict.
    #[test]
    fn attach_and_detach_advance_the_draft_epoch_strictly() {
        let (store, draft) = store_with_draft();
        let before = store.drafts().unwrap()[0].updated_epoch;

        let saved = store
            .add_draft_attachment(draft, "f.pdf", "application/pdf", &[1])
            .unwrap();
        assert!(saved.updated_epoch > before, "adding is a modification");
        let stored = store.drafts().unwrap()[0].updated_epoch;
        assert_eq!(
            saved.updated_epoch, stored,
            "the outcome states what is in the database"
        );

        let after_removal = store
            .remove_draft_attachment(saved.attachment.id)
            .unwrap()
            .expect("the attachment existed");
        assert!(after_removal > saved.updated_epoch);
        assert!(store.draft_attachments_meta(draft).unwrap().is_empty());
    }

    /// A double-click on the removal changes nothing the second time.
    #[test]
    fn removing_a_gone_attachment_is_a_silent_noop() {
        let (store, draft) = store_with_draft();
        let saved = store
            .add_draft_attachment(draft, "f.pdf", "application/pdf", &[1])
            .unwrap();
        let epoch = store
            .remove_draft_attachment(saved.attachment.id)
            .unwrap()
            .unwrap();

        assert_eq!(
            store.remove_draft_attachment(saved.attachment.id).unwrap(),
            None
        );
        assert_eq!(
            store.drafts().unwrap()[0].updated_epoch,
            epoch,
            "no phantom modification"
        );
    }
}

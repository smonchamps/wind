//! The local echo (PLAN-REACTIVITE E3, verdict R-D1 "< 1 s"): the
//! destination of a gesture shows from the local database, without
//! waiting for the server — offline included.
//!
//! Three non-negotiable safeguards:
//! - **never a forged key**: the echo lives in ITS OWN table, served in
//!   the list by a UNION (`nav.rs`) — never a UID invented in
//!   `envelopes`;
//! - **never without intent**: an echo reflects a logged action
//!   (deletion, archiving) or a send that reached `sent` — a send echo
//!   is NEVER born before SMTP acceptance ("never a phantom send");
//! - **never against the server**: the echo dies at reconciliation (the
//!   real row arrives — same `message_id` in the destination) or at the
//!   sweep (intent settled, destination polled with no copy: we do not
//!   show what the server denies).

use std::collections::{BTreeMap, BTreeSet};

use rusqlite::{OptionalExtension, params};

use crate::action::Action;
use crate::envelope::Uid;
use crate::error::Error;
use crate::store::Store;

/// The categories that carry echoes — the destinations of the three
/// covered gestures. A move to a free folder has no list to show up in
/// (the nav only serves the canonical ones): no echo.
pub const ECHO_DESTINATIONS: &[&str] = &["envoyes", "archives", "corbeille"];

/// The text of a send rendered as minimal HTML: escaped, line breaks
/// preserved. This is OUR text (the send log) — escaping is the only
/// requirement; reading sanitization runs behind it as for any body
/// (S1).
pub fn text_as_html(text: &str) -> String {
    let escaped = text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    format!("<div>{}</div>", escaped.replace('\n', "<br>"))
}

impl Store {
    /// Does the message already carry an envelope in the destination?
    /// (Gmail: archiving leaves the copy in "All Mail" — the exclusion
    /// clause unmasks it when removed from INBOX, the echo would be a
    /// duplicate.) An unresolved or never-synced destination answers
    /// "no": the echo is then the only truth available.
    fn present_at_destination(
        &self,
        account_id: i64,
        destination: &str,
        message_id: &str,
    ) -> Result<bool, Error> {
        let folders = self.canonical_folders(account_id)?;
        let Some(name) = folders.mailbox(destination) else {
            return Ok(false);
        };
        let Some(state) = self.sync_state(account_id, &name)? else {
            return Ok(false);
        };
        let present: bool = self.conn().query_row(
            "SELECT EXISTS(SELECT 1 FROM envelopes
              WHERE mailbox_id = ?1 AND message_id = ?2)",
            params![state.mailbox_id, message_id],
            |row| row.get(0),
        )?;
        Ok(present)
    }

    /// The gesture that moves (deletion, archiving, move) — in ONE
    /// transaction: the action is logged, the material of the message
    /// (envelope, body, preview, attachment count) is POURED into the
    /// destination echo, then the source empties. A crash in between
    /// loses nothing and fabricates nothing: all or nothing.
    ///
    /// `destination = None` (move to a free folder) or a message with no
    /// `message_id` (the echo would be unreconcilable): the action and
    /// the local disappearance happen, without an echo — the pre-E3
    /// behavior, intact.
    pub fn gesture_with_echo(
        &self,
        mailbox_id: i64,
        uid: Uid,
        action: Action,
        destination: Option<&str>,
    ) -> Result<(), Error> {
        let tx = self.conn().unchecked_transaction()?;
        self.gesture_under(&tx, mailbox_id, uid, action, destination)?;
        tx.commit()?;
        Ok(())
    }

    /// The gesture INSIDE a transaction opened by the caller — the batch
    /// of E6 (PLAN-AUDIT-V2) chains N messages there, all or nothing.
    fn gesture_under(
        &self,
        tx: &rusqlite::Connection,
        mailbox_id: i64,
        uid: Uid,
        action: Action,
        destination: Option<&str>,
    ) -> Result<(), Error> {
        let account_id: i64 = self.conn().query_row(
            "SELECT account_id FROM mailboxes WHERE id = ?1",
            [mailbox_id],
            |row| row.get(0),
        )?;
        // A fresh gesture replaces the refused ones for the message (E3 review).
        tx.execute(
            "DELETE FROM pending_actions WHERE mailbox_id = ?1 AND uid = ?2 AND refusee = 1",
            params![mailbox_id, uid],
        )?;
        tx.execute(
            "INSERT INTO pending_actions (mailbox_id, uid, kind) VALUES (?1, ?2, ?3)",
            params![mailbox_id, uid, action.to_kind()],
        )?;
        let action_id = tx.last_insert_rowid();
        if let Some(destination) = destination {
            // The echo's material is read BEFORE the source empties.
            type Material = (
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<i64>,
                Option<String>,
            );
            let envelope: Option<Material> = tx
                .query_row(
                    "SELECT subject, sender, sender_address, message_id, date_epoch, to_addrs
                     FROM envelopes WHERE mailbox_id = ?1 AND uid = ?2",
                    params![mailbox_id, uid],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                            row.get(5)?,
                        ))
                    },
                )
                .optional()?;
            if let Some((subject, sender, sender_address, Some(message_id), date_epoch, to_addrs)) =
                envelope
                && !self.present_at_destination(account_id, destination, &message_id)?
            {
                let body: Option<(Option<String>, Option<String>)> = tx
                    .query_row(
                        "SELECT html, preview FROM bodies
                         WHERE mailbox_id = ?1 AND uid = ?2",
                        params![mailbox_id, uid],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .optional()?;
                let (html, preview) = body.unwrap_or((None, None));
                let attachment_count: i64 = tx.query_row(
                    "SELECT COUNT(*) FROM attachments WHERE mailbox_id = ?1 AND uid = ?2",
                    params![mailbox_id, uid],
                    |row| row.get(0),
                )?;
                tx.execute(
                    "INSERT INTO echos (account_id, destination, message_id, sender,
                        sender_address, subject, date_epoch, preview, html,
                        attachment_count, to_addrs, origin_action_id, created_epoch)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, unixepoch())",
                    params![
                        account_id,
                        destination,
                        message_id,
                        sender,
                        sender_address,
                        subject,
                        date_epoch,
                        preview,
                        html,
                        attachment_count,
                        to_addrs,
                        action_id
                    ],
                )?;
            }
        }
        // The source disappearing — the same work as `remove_local`,
        // INSIDE the transaction (same connection).
        self.remove_local(mailbox_id, uid)?;
        Ok(())
    }

    /// The echo of a send — called when the outbox flush transitions to
    /// `sent`, and ONLY there: the query refuses any other state, by
    /// construction AND by guard. Returns `true` if an echo was born.
    pub fn send_echo(&self, outbox_id: i64) -> Result<bool, Error> {
        type SendRow = (
            i64,
            String,
            String,
            String,
            String,
            Option<String>,
            i64,
            String,
        );
        let row: Option<SendRow> = self
            .conn()
            .query_row(
                "SELECT account_id, message_id, sender, subject, body_text, body_html,
                        queued_epoch, recipients
                 FROM outbox WHERE id = ?1 AND state = 'sent'",
                [outbox_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                    ))
                },
            )
            .optional()?;
        let Some((
            account_id,
            message_id,
            sender,
            subject,
            body_text,
            body_html,
            queued_epoch,
            recipients,
        )) = row
        else {
            return Ok(false);
        };
        if self.present_at_destination(account_id, "envoyes", &message_id)? {
            return Ok(false);
        }
        let attachment_count: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM outbox_attachments WHERE outbox_id = ?1",
            [outbox_id],
            |row| row.get(0),
        )?;
        // A rich send shows ITS OWN HTML (PLAN-COMPOSITION-HTML) —
        // re-escaping it would show the tags; a plain-text send keeps
        // the historical escaped rendering. Reading re-sanitizes either
        // way (S1).
        let html = body_html.unwrap_or_else(|| text_as_html(&body_text));
        let preview = crate::body::extract_preview(&html);
        // `outbox.recipients` is already joined by '\n' (TO_SEPARATOR) —
        // the exact format of `envelopes.to_addrs`: copied as is.
        self.conn().execute(
            "INSERT INTO echos (account_id, destination, message_id, sender,
                sender_address, subject, date_epoch, preview, html,
                attachment_count, to_addrs, origin_outbox_id, created_epoch)
             VALUES (?1, 'envoyes', ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, unixepoch())",
            params![
                account_id,
                message_id,
                sender,
                sender,
                subject,
                queued_epoch,
                preview,
                html,
                attachment_count,
                recipients,
                outbox_id
            ],
        )?;
        Ok(true)
    }

    /// The attachments of a send echo, as METADATA only (name, mime,
    /// size — the bytes are purged at `sent`, PJ-D7): enough to show
    /// honest chips during the reconciliation window, never an
    /// "Attachments" title with nothing under it. A gesture echo
    /// (`origin_outbox_id` NULL) has none: empty list.
    pub fn echo_attachments(&self, echo_id: i64) -> Result<Vec<crate::OutboxAttachment>, Error> {
        let rows = self
            .conn()
            .prepare(
                "SELECT oa.name, oa.mime, oa.size
                 FROM echos ec
                 JOIN outbox_attachments oa ON oa.outbox_id = ec.origin_outbox_id
                 WHERE ec.id = ?1
                 ORDER BY oa.id",
            )?
            .query_map([echo_id], |row| {
                Ok(crate::OutboxAttachment {
                    name: row.get(0)?,
                    mime: row.get(1)?,
                    size: row.get::<_, i64>(2)? as u64,
                    bytes: None,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(rows)
    }

    /// How many echoes a category carries — the counterpart of the nav
    /// counters and the pagination totals ("never two truths").
    pub fn count_echos(&self, destination: &str, account_id: Option<i64>) -> Result<u64, Error> {
        let count: i64 = match account_id {
            Some(id) => self.conn().query_row(
                "SELECT COUNT(*) FROM echos WHERE destination = ?1 AND account_id = ?2",
                params![destination, id],
                |row| row.get(0),
            )?,
            None => self.conn().query_row(
                "SELECT COUNT(*) FROM echos WHERE destination = ?1",
                params![destination],
                |row| row.get(0),
            )?,
        };
        Ok(count as u64)
    }

    /// The body of an echo for Reading: HTML (that of the source
    /// message, or the rendered send text) and attachment count. `None`
    /// if the echo has already been reconciled — the real row took its
    /// place.
    pub fn echo_view(&self, echo_id: i64) -> Result<Option<(String, usize)>, Error> {
        let row: Option<(Option<String>, i64)> = self
            .conn()
            .query_row(
                "SELECT html, attachment_count FROM echos WHERE id = ?1",
                [echo_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        Ok(row
            .map(|(html, attachment_count)| (html.unwrap_or_default(), attachment_count as usize)))
    }

    /// Reconciliation: the echo dies when the real row arrives — same
    /// `message_id` in a mailbox of its destination. Called after any
    /// poll that may have served a destination (cycle, after-gesture
    /// pass). Returns the number of echoes removed.
    pub fn reconcile_echos(&self, account_id: i64) -> Result<usize, Error> {
        let echos: Vec<(i64, String, String)> = self
            .conn()
            .prepare("SELECT id, destination, message_id FROM echos WHERE account_id = ?1")?
            .query_map([account_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<Result<_, _>>()?;
        let mut removed = 0usize;
        for (id, destination, message_id) in echos {
            if self.present_at_destination(account_id, &destination, &message_id)? {
                self.conn()
                    .execute("DELETE FROM echos WHERE id = ?1", [id])?;
                removed += 1;
            }
        }
        Ok(removed)
    }

    /// The safety sweep: an echo whose INTENT is settled (action replayed
    /// and removed from the queue, send gone out) but whose destination,
    /// polled, still shows nothing — we do not show what the server
    /// denies. Only call this after a CLEAN pass (polls without error)
    /// and its retries: an echo whose action is still pending (offline,
    /// backoff) LIVES — it reflects the intent. Returns one incident per
    /// echo removed.
    ///
    /// A SEND echo has no origin action: its intent is settled by
    /// construction. It is only swept if Sent has been POLLED AFTER it
    /// (`mailboxes.relevee_epoch`) — otherwise the copy has simply not
    /// been seen yet (PLAN-AUDIT-V2 E5: it used to leave on the very
    /// first pass, the sent message vanished from the screen until the
    /// next poll). An account with no announced sent folder keeps the
    /// echo: it is the only trace of the message gone out.
    pub fn sweep_echos(&self, account_id: i64) -> Result<Vec<String>, Error> {
        let expired: Vec<(i64, String)> = self
            .conn()
            .prepare(
                "SELECT id, destination FROM echos
                 WHERE account_id = ?1
                   AND ((origin_action_id IS NULL
                         AND EXISTS (SELECT 1 FROM mailboxes m
                                       JOIN accounts a ON a.id = m.account_id
                                      WHERE m.account_id = echos.account_id
                                        AND m.name = a.sent_mailbox
                                        AND m.relevee_epoch > echos.created_epoch))
                        OR (origin_action_id IS NOT NULL
                            AND NOT EXISTS (SELECT 1 FROM pending_actions p
                                             WHERE p.id = origin_action_id
                                               AND p.refusee = 0)))",
            )?
            .query_map([account_id], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<_, _>>()?;
        let mut incidents = Vec::new();
        for (id, destination) in expired {
            self.conn()
                .execute("DELETE FROM echos WHERE id = ?1", [id])?;
            incidents.push(format!(
                "copy expected in \u{201c}{destination}\u{201d} never seen from the server — echo removed"
            ));
        }
        Ok(incidents)
    }

    /// Are any echoes still waiting for their reconciliation? This is
    /// the retry signal of the after-gesture pass.
    pub fn pending_echos(&self, account_id: i64) -> Result<u64, Error> {
        let count: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM echos WHERE account_id = ?1",
            [account_id],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }

    /// The mailboxes of an account that carry pending actions — the
    /// "intentions" phase of the after-gesture pass: polling them
    /// replays the log NOW, instead of waiting for the cycle.
    pub fn mailboxes_with_actions(&self, account_id: i64) -> Result<Vec<String>, Error> {
        let rows = self
            .conn()
            .prepare(
                "SELECT DISTINCT m.name FROM pending_actions p
                 JOIN mailboxes m ON m.id = p.mailbox_id
                 WHERE m.account_id = ?1 AND p.refusee = 0",
            )?
            .query_map([account_id], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        Ok(rows)
    }

    /// The accounts that have after-gesture work — pending actions or
    /// echoes to reconcile. The back-online trigger (R-D3) uses this:
    /// nothing to do = no connection opened.
    pub fn accounts_with_work(&self) -> Result<Vec<i64>, Error> {
        let rows = self
            .conn()
            .prepare(
                "SELECT DISTINCT m.account_id FROM pending_actions p
                 JOIN mailboxes m ON m.id = p.mailbox_id
                 WHERE p.refusee = 0
                 UNION
                 SELECT DISTINCT account_id FROM echos",
            )?
            .query_map([], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        Ok(rows)
    }
}

/// A conversation targeted by a bulk gesture — as the UI names it
/// (account, mailbox, UID of the row, and its thread if it has one).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GestureTarget {
    pub account_id: i64,
    pub mailbox: String,
    pub uid: Uid,
    pub thread_id: Option<i64>,
}

/// The gestures the selection bar knows how to do in bulk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupGesture {
    Archive,
    Delete,
    Spam,
    NotSpam,
    Seen(bool),
}

impl Store {
    /// The BULK gesture (PLAN-AUDIT-V2 E6, D6: all or nothing): each
    /// checked row is a CONVERSATION — the whole thread goes (D6 of
    /// PLAN-RETOURS-10) — and the whole batch lives in ONE transaction: a
    /// failure halfway leaves nothing half done, the UI says so. Before,
    /// the UI replayed N × k unit commands in series (250 + 50 IPC for 50
    /// conversations), each with its own transaction, the bar frozen.
    ///
    /// Returns the number of conversations processed. `Spam` with no
    /// junk folder on a targeted account is an outright refusal, BEFORE
    /// any write; an unknown mailbox in the database refuses the whole
    /// batch (nothing done).
    pub fn act_on_group(
        &self,
        targets: &[GestureTarget],
        gesture: &GroupGesture,
    ) -> Result<usize, Error> {
        let mut messages: Vec<(i64, String, Uid)> = Vec::new();
        let mut already: BTreeSet<(i64, String, Uid)> = BTreeSet::new();
        for target in targets {
            let alone = vec![(target.account_id, target.mailbox.clone(), target.uid)];
            let of_thread = match target.thread_id {
                Some(thread_id) => {
                    let in_thread = self.messages_of_thread(thread_id)?;
                    if in_thread.is_empty() {
                        alone
                    } else {
                        in_thread
                    }
                }
                None => alone,
            };
            for message in of_thread {
                if already.insert(message.clone()) {
                    messages.push(message);
                }
            }
        }
        // The junk folder of EACH account, resolved BEFORE the
        // transaction (same rule as the E3 arrival and Cleanup).
        let mut junk_folders: BTreeMap<i64, String> = BTreeMap::new();
        if *gesture == GroupGesture::Spam {
            let accounts: BTreeSet<i64> = targets.iter().map(|target| target.account_id).collect();
            for account in accounts {
                let folder = self.canonical_folders(account)?.junk.ok_or_else(|| {
                    Error::Refusal("no junk folder recognized on this account".to_string())
                })?;
                junk_folders.insert(account, folder);
            }
        }
        // The mailbox of each (account, name) resolved ONCE — a batch of
        // 250 messages lives in one or two mailboxes (review).
        let mut mailboxes: BTreeMap<(i64, String), Option<i64>> = BTreeMap::new();
        let tx = self.conn().unchecked_transaction()?;
        for (account_id, mailbox, uid) in &messages {
            let key = (*account_id, mailbox.clone());
            let resolved = match mailboxes.get(&key) {
                Some(id) => *id,
                None => {
                    let id = self.sync_state(*account_id, mailbox)?.map(|s| s.mailbox_id);
                    mailboxes.insert(key, id);
                    id
                }
            };
            // All or nothing (D6): a mailbox the database does not know
            // (renamed, gone mid-gesture) refuses the WHOLE batch —
            // before, the message was silently skipped and the summary
            // said "N done" (review).
            let Some(mailbox_id) = resolved else {
                return Err(Error::Refusal(format!(
                    "mailbox unknown in database for account {account_id}: the batch is refused"
                )));
            };
            match gesture {
                GroupGesture::Archive => {
                    self.gesture_under(&tx, mailbox_id, *uid, Action::Archive, Some("archives"))?;
                }
                GroupGesture::Delete => {
                    self.gesture_under(&tx, mailbox_id, *uid, Action::Delete, Some("corbeille"))?;
                }
                GroupGesture::Spam => {
                    let spam = &junk_folders[account_id];
                    if spam != mailbox {
                        self.gesture_under(
                            &tx,
                            mailbox_id,
                            *uid,
                            Action::MoveTo(spam.clone()),
                            None,
                        )?;
                    }
                }
                GroupGesture::NotSpam => {
                    self.gesture_under(
                        &tx,
                        mailbox_id,
                        *uid,
                        Action::MoveTo(crate::thread::RECEIVED_MAILBOX.to_string()),
                        None,
                    )?;
                }
                GroupGesture::Seen(seen) => {
                    if self.set_seen_local(mailbox_id, *uid, *seen)? {
                        let action = if *seen {
                            Action::MarkSeen
                        } else {
                            Action::MarkUnseen
                        };
                        self.enqueue_action(mailbox_id, *uid, action)?;
                    }
                }
            }
        }
        tx.commit()?;
        Ok(targets.len())
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;
    use crate::envelope::Envelope;

    fn envelope(uid: Uid, subject: &str, epoch: i64) -> Envelope {
        Envelope {
            reply_to: None,
            uid,
            subject: Some(subject.to_string()),
            sender: Some("Alice Martin".to_string()),
            sender_address: Some("alice@example.com".to_string()),
            message_id: Some(format!("<m{uid}@example.com>")),
            in_reply_to: None,
            date: Some(Utc.timestamp_opt(epoch, 0).unwrap()),
            seen: true,
            flagged: false,
            to_addrs: Vec::new(),
            cc_addrs: Vec::new(),
        }
    }

    fn store_with_trash() -> (Store, i64, i64, i64) {
        let store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("t@exemple.fr", "gmail")
            .unwrap();
        let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        let trash = store.create_mailbox(account, "Trash", 1).unwrap();
        store
            .replace_folders(
                account,
                &[
                    crate::Folder {
                        wire: "INBOX".into(),
                        display: "INBOX".into(),
                        selectable: true,
                        special_use: None,
                    },
                    crate::Folder {
                        wire: "Trash".into(),
                        display: "Trash".into(),
                        selectable: true,
                        special_use: None,
                    },
                ],
            )
            .unwrap();
        (store, account, inbox, trash)
    }

    /// The gesture empties the source, logs the action AND sets the
    /// echo — with the message's material (preview, body, attachments):
    /// the destination shows without the server.
    #[test]
    fn the_gesture_pours_material_into_the_echo() {
        let (mut store, account, inbox, _) = store_with_trash();
        store
            .upsert_envelopes(inbox, &[envelope(1, "to discard", 100)])
            .unwrap();
        store.save_body(inbox, 1, "<p>body</p>", &[]).unwrap();

        store
            .gesture_with_echo(inbox, 1, Action::Delete, Some("corbeille"))
            .unwrap();

        // The source is empty, the action logged.
        assert!(store.recent(account, "INBOX", 0, 10).unwrap().is_empty());
        assert_eq!(store.pending_actions(inbox).unwrap().len(), 1);
        // The echo carries everything.
        assert_eq!(store.count_echos("corbeille", Some(account)).unwrap(), 1);
        let (id, preview): (i64, Option<String>) = store
            .conn()
            .query_row("SELECT id, preview FROM echos", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .unwrap();
        assert_eq!(preview.as_deref(), Some("body"));
        let (html, attachment_count) = store.echo_view(id).unwrap().unwrap();
        assert_eq!(html, "<p>body</p>");
        assert_eq!(attachment_count, 0);
    }

    /// Without a `message_id`, the echo would be unreconcilable: the
    /// gesture goes through without an echo — the pre-E3 behavior,
    /// intact.
    #[test]
    fn without_message_id_no_echo() {
        let (mut store, account, inbox, _) = store_with_trash();
        let mut without_id = envelope(1, "anonymous", 100);
        without_id.message_id = None;
        store.upsert_envelopes(inbox, &[without_id]).unwrap();

        store
            .gesture_with_echo(inbox, 1, Action::Delete, Some("corbeille"))
            .unwrap();

        assert_eq!(store.count_echos("corbeille", Some(account)).unwrap(), 0);
        assert!(store.recent(account, "INBOX", 0, 10).unwrap().is_empty());
        assert_eq!(store.pending_actions(inbox).unwrap().len(), 1);
    }

    /// Already present at the destination (Gmail: "All Mail" already
    /// carries the copy an archiving unmasks): no duplicate.
    #[test]
    fn already_present_at_destination_no_duplicate() {
        let (mut store, account, inbox, trash) = store_with_trash();
        store
            .upsert_envelopes(inbox, &[envelope(1, "already there", 100)])
            .unwrap();
        store
            .upsert_envelopes(trash, &[envelope(1, "already there", 100)])
            .unwrap();

        store
            .gesture_with_echo(inbox, 1, Action::Delete, Some("corbeille"))
            .unwrap();

        assert_eq!(store.count_echos("corbeille", Some(account)).unwrap(), 0);
    }

    /// Reconciliation: the real row arrives (same `message_id` in the
    /// destination) → the echo dies, the list does not move visibly.
    #[test]
    fn the_real_row_kills_the_echo() {
        let (mut store, account, inbox, trash) = store_with_trash();
        store
            .upsert_envelopes(inbox, &[envelope(1, "to discard", 100)])
            .unwrap();
        store
            .gesture_with_echo(inbox, 1, Action::Delete, Some("corbeille"))
            .unwrap();
        assert_eq!(
            store.reconcile_echos(account).unwrap(),
            0,
            "nothing arrived: nothing to do"
        );

        // The copy arrives in Trash (poll) — fresh UID, same message_id.
        store
            .upsert_envelopes(trash, &[envelope(1, "to discard", 100)])
            .unwrap();

        assert_eq!(store.reconcile_echos(account).unwrap(), 1);
        assert_eq!(store.count_echos("corbeille", Some(account)).unwrap(), 0);
    }

    /// The sweep: the action replayed (removed from the queue) and still
    /// no copy → the echo is removed, the incident is recorded. An
    /// action still queued protects its echo — offline, the echo LIVES.
    #[test]
    fn the_sweep_respects_a_pending_intention() {
        let (mut store, account, inbox, _) = store_with_trash();
        store
            .upsert_envelopes(inbox, &[envelope(1, "to discard", 100)])
            .unwrap();
        store
            .gesture_with_echo(inbox, 1, Action::Delete, Some("corbeille"))
            .unwrap();

        // The action is still pending: the sweep touches nothing.
        assert!(store.sweep_echos(account).unwrap().is_empty());
        assert_eq!(store.pending_echos(account).unwrap(), 1);

        // The action is replayed (the queue empties) — the polled
        // destination shows nothing: the echo leaves, the incident is
        // recorded.
        let action = store.pending_actions(inbox).unwrap().remove(0);
        store.remove_action(action.id).unwrap();
        let incidents = store.sweep_echos(account).unwrap();
        assert_eq!(incidents.len(), 1);
        assert!(incidents[0].contains("corbeille"), "{incidents:?}");
        assert_eq!(store.pending_echos(account).unwrap(), 0);
    }

    /// The send echo is only born at `sent` — never for an entry still
    /// queued, failed, or quarantined ("never a phantom send").
    #[test]
    fn the_send_echo_is_born_only_at_sent() {
        let store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("t@exemple.fr", "gmail")
            .unwrap();
        let draft = crate::compose(
            "t@exemple.fr",
            "a@b.fr",
            "",
            "",
            "subject",
            "body\nline 2",
            None,
        )
        .unwrap();
        store.enqueue_outbox(account, &draft).unwrap();
        let id = store.outbox_to_send(account).unwrap()[0].id;

        assert!(!store.send_echo(id).unwrap(), "still queued: no echo");
        assert_eq!(store.count_echos("envoyes", Some(account)).unwrap(), 0);

        store
            .set_outbox_state(id, crate::OutboxState::Sent)
            .unwrap();
        assert!(store.send_echo(id).unwrap());
        assert_eq!(store.count_echos("envoyes", Some(account)).unwrap(), 1);
        // The body is the log text, escaped and readable.
        let echo_id: i64 = store
            .conn()
            .query_row("SELECT id FROM echos", [], |row| row.get(0))
            .unwrap();
        let (html, _) = store.echo_view(echo_id).unwrap().unwrap();
        assert!(html.contains("body<br>line 2"), "{html}");
    }

    /// PLAN-AUDIT-V2 E6 (D6, all or nothing): fifty conversations
    /// archived in ONE gesture and ONE transaction — a failure at the
    /// thirtieth removal leaves nothing half done; without a failure,
    /// all fifty go. Before: N × k unit commands, each its own
    /// transaction.
    #[test]
    fn fifty_conversations_archived_in_one_transaction() {
        use crate::{GestureTarget, GroupGesture};
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("t@exemple.fr", "gmail")
            .unwrap();
        let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        let fixture: Vec<crate::Envelope> = (1..=50)
            .map(|uid| crate::Envelope {
                uid,
                subject: Some(format!("message {uid}")),
                sender: Some("Alice".to_string()),
                sender_address: Some("alice@exemple.fr".to_string()),
                to_addrs: Vec::new(),
                cc_addrs: Vec::new(),
                reply_to: None,
                message_id: Some(format!("<m{uid}@exemple.fr>")),
                in_reply_to: None,
                date: None,
                seen: false,
                flagged: false,
            })
            .collect();
        store.upsert_envelopes(inbox, &fixture).unwrap();
        let targets: Vec<GestureTarget> = (1..=50)
            .map(|uid| GestureTarget {
                account_id: account,
                mailbox: "INBOX".to_string(),
                uid,
                thread_id: None,
            })
            .collect();
        let count = |table: &str| -> i64 {
            store
                .conn()
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .unwrap()
        };

        // Failure at the thirtieth removal: nothing must be left half done.
        store
            .conn()
            .execute_batch(
                "CREATE TRIGGER panne BEFORE DELETE ON envelopes WHEN OLD.uid = 30
                 BEGIN SELECT RAISE(ABORT, 'simulated failure'); END;",
            )
            .unwrap();
        assert!(
            store
                .act_on_group(&targets, &GroupGesture::Archive)
                .is_err()
        );
        assert_eq!(count("envelopes"), 50, "all or nothing: nothing left");
        assert_eq!(count("pending_actions"), 0);
        assert_eq!(count("echos"), 0);

        store.conn().execute_batch("DROP TRIGGER panne").unwrap();

        // A target on an unknown mailbox: the batch is refused, nothing
        // leaves — the summary never says "50 done" for 49 (review).
        let mut with_unknown = targets.clone();
        with_unknown.push(GestureTarget {
            account_id: account,
            mailbox: "Disparue".to_string(),
            uid: 1,
            thread_id: None,
        });
        assert!(
            store
                .act_on_group(&with_unknown, &GroupGesture::Archive)
                .is_err()
        );
        assert_eq!(count("envelopes"), 50);
        assert_eq!(count("pending_actions"), 0);

        assert_eq!(
            store
                .act_on_group(&targets, &GroupGesture::Archive)
                .unwrap(),
            50
        );
        assert_eq!(count("envelopes"), 0);
        assert_eq!(count("pending_actions"), 50);
        assert_eq!(count("echos"), 50, "one archive echo per message");
    }

    /// PLAN-AUDIT-V2 E5 (audit 2.1 "SEND echo swept on the very first
    /// pass"): a send echo has no origin action; the
    /// `origin_action_id IS NULL` sweep took it for a settled intent and
    /// removed it BEFORE any poll of Sent — the message that had gone
    /// out vanished from the screen until the next poll.
    #[test]
    fn the_send_echo_survives_the_sweep_without_a_sent_poll() {
        let store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("t@exemple.fr", "gmail")
            .unwrap();
        let draft =
            crate::compose("t@exemple.fr", "a@b.fr", "", "", "subject", "body", None).unwrap();
        store.enqueue_outbox(account, &draft).unwrap();
        let id = store.outbox_to_send(account).unwrap()[0].id;
        store
            .set_outbox_state(id, crate::OutboxState::Sent)
            .unwrap();
        assert!(store.send_echo(id).unwrap());

        let incidents = store.sweep_echos(account).unwrap();
        assert!(incidents.is_empty(), "wrongly swept: {incidents:?}");
        assert_eq!(store.count_echos("envoyes", Some(account)).unwrap(), 1);

        // Sent polled AFTER the send, without the copy: there, the echo
        // is denied by the server and leaves.
        let envoyes = store.create_mailbox(account, "Envoyes", 1).unwrap();
        store
            .conn()
            .execute_batch(&format!(
                "UPDATE accounts SET sent_mailbox = 'Envoyes' WHERE id = {account};
                 UPDATE mailboxes SET relevee_epoch = (SELECT MAX(created_epoch) + 1 FROM echos)
                  WHERE id = {envoyes};"
            ))
            .unwrap();
        assert_eq!(store.sweep_echos(account).unwrap().len(), 1);
        assert_eq!(store.count_echos("envoyes", Some(account)).unwrap(), 0);
    }

    /// PLAN-COMPOSITION-HTML: the echo of a RICH send carries the
    /// composed HTML as is — never the re-escaped text (the formatting
    /// would show as tags in Sent). Reading sanitization runs behind it
    /// as for any body (S1).
    #[test]
    fn a_rich_send_echo_carries_the_composed_html() {
        let store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("t@exemple.fr", "gmail")
            .unwrap();
        let mut draft =
            crate::compose("t@exemple.fr", "a@b.fr", "", "", "subject", "body", None).unwrap();
        draft.body_html = Some("<div><b>body</b></div>".to_string());
        store.enqueue_outbox(account, &draft).unwrap();
        let id = store.outbox_to_send(account).unwrap()[0].id;
        store
            .set_outbox_state(id, crate::OutboxState::Sent)
            .unwrap();
        assert!(store.send_echo(id).unwrap());

        let echo_id: i64 = store
            .conn()
            .query_row("SELECT id FROM echos", [], |row| row.get(0))
            .unwrap();
        let (html, _) = store.echo_view(echo_id).unwrap().unwrap();
        assert!(html.contains("<b>body</b>"), "{html}");
        assert!(
            !html.contains("&lt;b&gt;"),
            "the HTML must not be re-escaped: {html}"
        );
    }

    /// PLAN-RETOURS-5 (field 2026-08-21: "To: envoyes" during the
    /// reconciliation window): the send echo carries the REAL
    /// recipients, copied from the send log in the envelopes' format
    /// (`\n`).
    #[test]
    fn the_send_echo_carries_the_recipients() {
        let store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("t@exemple.fr", "gmail")
            .unwrap();
        let draft = crate::compose(
            "t@exemple.fr",
            "a@b.fr, c@d.fr",
            "",
            "",
            "subject",
            "body",
            None,
        )
        .unwrap();
        store.enqueue_outbox(account, &draft).unwrap();
        let id = store.outbox_to_send(account).unwrap()[0].id;
        store
            .set_outbox_state(id, crate::OutboxState::Sent)
            .unwrap();
        assert!(store.send_echo(id).unwrap());

        let to: Option<String> = store
            .conn()
            .query_row("SELECT to_addrs FROM echos", [], |row| row.get(0))
            .unwrap();
        assert_eq!(to.as_deref(), Some("a@b.fr\nc@d.fr"));
    }

    /// The gesture also pours the recipients of the moved message — the
    /// envelopes column is already in the right format, copied as is.
    #[test]
    fn the_gesture_pours_recipients_into_the_echo() {
        let (mut store, _account, inbox, _) = store_with_trash();
        let mut env = envelope(1, "to discard", 100);
        env.to_addrs = vec!["x@y.fr".to_string(), "z@w.fr".to_string()];
        store.upsert_envelopes(inbox, &[env]).unwrap();

        store
            .gesture_with_echo(inbox, 1, Action::Delete, Some("corbeille"))
            .unwrap();

        let to: Option<String> = store
            .conn()
            .query_row("SELECT to_addrs FROM echos", [], |row| row.get(0))
            .unwrap();
        assert_eq!(to.as_deref(), Some("x@y.fr\nz@w.fr"));
    }

    /// The attachments of a send echo read as METADATA (name, mime,
    /// size) from the send log — the bytes are purged at `sent`
    /// (PJ-D7), never an "Attachments" title with nothing under it. A
    /// gesture echo has none: empty list.
    #[test]
    fn send_echo_attachments_read_as_metadata_only() {
        let store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("t@exemple.fr", "gmail")
            .unwrap();
        let draft =
            crate::compose("t@exemple.fr", "a@b.fr", "", "", "subject", "body", None).unwrap();
        let draft_id = store
            .save_draft(
                account,
                None,
                None,
                crate::DraftContent {
                    to_raw: "a@b.fr",
                    cc_raw: "",
                    bcc_raw: "",
                    body_html: None,
                    subject: "subject",
                    body: "body",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                    important: false,
                },
            )
            .unwrap()
            .id;
        store
            .add_draft_attachment(draft_id, "rapport.pdf", "application/pdf", &[1, 2, 3])
            .unwrap();
        let id = store
            .enqueue_outbox_from_draft(account, &draft, draft_id)
            .unwrap();
        store
            .set_outbox_state(id, crate::OutboxState::Sent)
            .unwrap();
        store.purge_sent_attachment_bytes(id).unwrap();
        assert!(store.send_echo(id).unwrap());
        let echo_id: i64 = store
            .conn()
            .query_row("SELECT id FROM echos", [], |row| row.get(0))
            .unwrap();

        let attachments = store.echo_attachments(echo_id).unwrap();
        assert_eq!(attachments.len(), 1);
        assert_eq!(attachments[0].name, "rapport.pdf");
        assert_eq!(attachments[0].mime, "application/pdf");
        assert_eq!(attachments[0].size, 3);
        assert!(attachments[0].bytes.is_none(), "metadata only");
    }

    /// Accounts with work: pending actions OR echoes — the back-online
    /// trigger only wakes them.
    #[test]
    fn accounts_with_work_combines_actions_and_echoes() {
        let (mut store, account, inbox, _) = store_with_trash();
        assert!(store.accounts_with_work().unwrap().is_empty());
        store
            .upsert_envelopes(inbox, &[envelope(1, "x", 100)])
            .unwrap();
        store
            .gesture_with_echo(inbox, 1, Action::Delete, Some("corbeille"))
            .unwrap();
        assert_eq!(store.accounts_with_work().unwrap(), vec![account]);
        assert_eq!(
            store.mailboxes_with_actions(account).unwrap(),
            vec!["INBOX".to_string()]
        );
    }

    /// The rendered send text: escaped (never HTML interpreted from a
    /// plain text), line breaks preserved.
    #[test]
    fn the_send_text_is_escaped() {
        assert_eq!(
            text_as_html("a <b> & c\nd"),
            "<div>a &lt;b&gt; &amp; c<br>d</div>"
        );
    }
}

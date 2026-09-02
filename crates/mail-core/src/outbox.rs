//! The persistent send outbox — the summit of Phase 2.
//!
//! Two golden rules (PLAN.md §1 and §4), proven by tests:
//! - **never a lost send**: the send intent is journaled in SQLite
//!   BEFORE any network attempt; on a cut connection or a crash, it
//!   survives and departs at the next flush;
//! - **never a phantom send**: a send interrupted in flight (a crash
//!   between delivery to the server and the local acknowledgment) is
//!   NEVER resent automatically — it is quarantined until the user's
//!   explicit decision. A silent duplicate is worse than a delay: a
//!   delay catches up, a duplicate is already at the recipient's door.

use chrono::Utc;
use rusqlite::params;

use crate::compose::Draft;
use crate::error::Error;
use crate::store::Store;
use crate::transport::{MailTransport, SendError};

/// Recipient separator in storage: safe by construction, since
/// [`crate::EmailAddress`] refuses any whitespace character.
const TO_SEPARATOR: char = '\n';

/// Rebuilds a stored address list (Cc, Bcc): the EMPTY string means an
/// empty list — otherwise `"".split('\n')` would yield a phantom `[""]`
/// (the "To" field, for its part, is never empty and needs no such guard).
fn split_recipients(stored: &str) -> Vec<String> {
    if stored.is_empty() {
        Vec::new()
    } else {
        stored.split(TO_SEPARATOR).map(str::to_string).collect()
    }
}

/// Life cycle of a send. Strict state machine:
///
/// ```text
/// queued ──→ sending ──→ sent
///    ↑          │
///    │          ├─ transient failure ──→ queued (automatic retry)
///    │          ├─ permanent refusal ──→ rejected (user decision)
///    │          └─ crash in flight ────→ interrupted (quarantine)
///    └────────── requeue: the user's explicit decision
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutboxState {
    /// Waiting — will be picked up by the next flush.
    Queued,
    /// Delivery to the server in progress. Found in this state at the
    /// start of a flush, the message comes from a crash: off to quarantine.
    Sending,
    /// Accepted by the sending server.
    Sent,
    /// Interrupted in flight: may have gone out, may not have.
    /// NEVER resent without the user's confirmation.
    Interrupted,
    /// Definitively refused by the server.
    Rejected,
}

impl OutboxState {
    pub fn as_str(self) -> &'static str {
        match self {
            OutboxState::Queued => "queued",
            OutboxState::Sending => "sending",
            OutboxState::Sent => "sent",
            OutboxState::Interrupted => "interrupted",
            OutboxState::Rejected => "rejected",
        }
    }

    pub(crate) fn parse(kind: &str) -> Option<Self> {
        match kind {
            "queued" => Some(OutboxState::Queued),
            "sending" => Some(OutboxState::Sending),
            "sent" => Some(OutboxState::Sent),
            "interrupted" => Some(OutboxState::Interrupted),
            "rejected" => Some(OutboxState::Rejected),
            _ => None,
        }
    }
}

/// An attachment in the send journal.
///
/// `bytes` is `None` once the message has gone out (PJ-D7 purge): the
/// history keeps the name and the weight, never the bytes. As long as
/// the message can still go out — queued, quarantined, rejected — the
/// bytes are there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboxAttachment {
    pub name: String,
    pub mime: String,
    /// DECODED bytes — the size the user recognizes.
    pub size: u64,
    pub bytes: Option<Vec<u8>>,
}

/// A message journaled in the outbox.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboxMessage {
    pub id: i64,
    /// The sending account — each flush goes through ITS OWN SMTP connection.
    pub account_id: i64,
    /// RFC 5322 Message-ID generated at composition — the stable identity
    /// that ties this journal entry to the message that actually went out.
    pub message_id: String,
    pub from: String,
    pub to: Vec<String>,
    /// Carbon copy — appears in the `Cc:` header of the sent message.
    pub cc: Vec<String>,
    /// Blind carbon copy — NEVER in the headers of the delivered message;
    /// the send carries it only in the SMTP envelope (mail-smtp).
    pub bcc: Vec<String>,
    pub subject: String,
    pub body_text: String,
    /// Rich body (PLAN-COMPOSITION-HTML) — the text/html part of the
    /// multipart/alternative; `None` = text-only send (historical path).
    pub body_html: Option<String>,
    pub in_reply_to: Option<String>,
    /// E7: the complete `References` chain (parent + its own References),
    /// as composed; `None` = the parent alone.
    pub references: Option<String>,
    /// Flagged "important" at composition (R3): delivery will set the
    /// priority headers.
    pub important: bool,
    /// Deferred send (R2): the epoch (seconds) before which the flush
    /// will not pick up this message. `None` = right away.
    pub send_at_epoch: Option<i64>,
    /// The iTIP reply (PLAN-INVITATIONS) — delivery carries it in a
    /// `text/calendar; method=REPLY` part. `None` = an ordinary send.
    pub ics_reply: Option<String>,
    /// The attachments, in gesture order (PJ-D2).
    pub attachments: Vec<OutboxAttachment>,
    pub state: OutboxState,
    pub attempts: u32,
    pub last_error: Option<String>,
    pub queued_epoch: i64,
}

const OUTBOX_SELECT: &str = "SELECT id, account_id, message_id, sender, recipients, subject,
        body_text, in_reply_to, state, attempts, last_error, queued_epoch, cc_addrs, bcc_addrs,
        body_html, important, send_at_epoch, ics_reply, refs
 FROM outbox";

impl Store {
    /// Journals the send intent — BEFORE any network attempt. This
    /// write is what founds "never a lost send".
    pub fn enqueue_outbox(&self, account_id: i64, draft: &Draft) -> Result<i64, Error> {
        let sep = TO_SEPARATOR.to_string();
        self.conn().execute(
            "INSERT INTO outbox
             (account_id, message_id, sender, recipients, cc_addrs, bcc_addrs, subject, body_text,
              body_html, in_reply_to, important, ics_reply, state, queued_epoch, refs)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                account_id,
                draft.message_id,
                draft.from,
                draft.to.join(&sep),
                draft.cc.join(&sep),
                draft.bcc.join(&sep),
                draft.subject,
                draft.body_text,
                draft.body_html,
                draft.in_reply_to,
                draft.important,
                draft.ics_reply,
                OutboxState::Queued.as_str(),
                Utc::now().timestamp(),
                draft.references,
            ],
        )?;
        let outbox_id = self.conn().last_insert_rowid();
        // PLAN-RETOURS-5 (D4): an address we write is an address we
        // know — the directory learns it the moment it is queued,
        // without waiting for it to come back through the Sent sync.
        let now = Utc::now().timestamp();
        for address in draft.to.iter().chain(&draft.cc).chain(&draft.bcc) {
            crate::contacts::note(self.conn(), address, None, now)?;
        }
        Ok(outbox_id)
    }

    /// Journals the send intent AND copies the draft's attachments in
    /// the SAME transaction (PJ-D2): "never a lost send" covers the
    /// bytes too — the draft can then disappear (it has served its
    /// purpose), the journal is self-sufficient.
    pub fn enqueue_outbox_from_draft(
        &self,
        account_id: i64,
        draft: &Draft,
        draft_id: i64,
    ) -> Result<i64, Error> {
        self.enqueue_outbox_full(account_id, draft, Some(draft_id), None)
    }

    /// The COMPLETE path of queuing a send (R2, PLAN-RETOURS-6): an
    /// optional anchor draft, an optional deadline — all in ONE
    /// transaction. "Never a lost send" also covers the chosen time: a
    /// crash never leaves a scheduled send amputated of its deadline
    /// (it would go out right away, against the intent).
    pub fn enqueue_outbox_full(
        &self,
        account_id: i64,
        draft: &Draft,
        draft_id: Option<i64>,
        send_at_epoch: Option<i64>,
    ) -> Result<i64, Error> {
        let tx = self.conn().unchecked_transaction()?;
        let outbox_id = self.enqueue_outbox(account_id, draft)?;
        if let Some(draft_id) = draft_id {
            tx.execute(
                "INSERT INTO outbox_attachments (outbox_id, name, mime, size, bytes)
                 SELECT ?1, name, mime, size, bytes FROM draft_attachments
                 WHERE draft_id = ?2 ORDER BY id",
                params![outbox_id, draft_id],
            )?;
        }
        if let Some(send_at) = send_at_epoch {
            tx.execute(
                "UPDATE outbox SET send_at_epoch = ?2 WHERE id = ?1",
                params![outbox_id, send_at],
            )?;
        }
        tx.commit()?;
        Ok(outbox_id)
    }

    /// Journals the iTIP reply email AND records the reply on the card
    /// — in ONE transaction (PLAN-INVITATIONS, D6). If the invitation
    /// row no longer exists (message purged, mailbox reset between the
    /// display and the click), NOTHING goes out: `None` is better than
    /// a queued email in front of a card that still says "no reply
    /// yet" — the user would click again, and the organizer would
    /// receive two REPLYs.
    pub fn enqueue_invitation_reply(
        &self,
        account_id: i64,
        draft: &Draft,
        mailbox: &str,
        uid: crate::envelope::Uid,
        reply: &str,
        epoch: i64,
    ) -> Result<Option<i64>, Error> {
        let tx = self.conn().unchecked_transaction()?;
        let touched = tx.execute(
            "UPDATE invitations SET reponse = ?4, reponse_epoch = ?5
             WHERE uid = ?3 AND mailbox_id IN
                   (SELECT id FROM mailboxes WHERE account_id = ?1 AND name = ?2)",
            params![account_id, mailbox, uid, reply, epoch],
        )?;
        if touched == 0 {
            // The transaction rewinds on drop: nothing gets journaled.
            return Ok(None);
        }
        let outbox_id = self.enqueue_outbox(account_id, draft)?;
        tx.commit()?;
        Ok(Some(outbox_id))
    }

    /// Cancels a scheduled send (R2, CE decision D2): the entry leaves
    /// the journal and a COMPLETE draft is reborn — recipients, body,
    /// flag, attachments with their bytes. Nothing is lost, the gesture
    /// is reversible. `None` if the entry is no longer queued (the
    /// flush picked it up in the meantime: too late, the message is
    /// going out) — the caller says so honestly rather than promising a
    /// draft that does not exist.
    ///
    /// Only targets entries that are SCHEDULED and not yet due: a due
    /// entry may be in the middle of delivery by a concurrent flush
    /// (outside the serialized queue) — cancelling it here would
    /// recreate a draft for a message that may have already gone out
    /// (a duplicate). Abandoning an ordinary send stays `delete_outbox`.
    pub fn cancel_scheduled_send(&self, id: i64) -> Result<Option<i64>, Error> {
        let tx = self.conn().unchecked_transaction()?;
        let mut stmt = self.conn().prepare(&format!(
            "{OUTBOX_SELECT} WHERE id = ?1 AND state = 'queued'
               AND send_at_epoch IS NOT NULL AND send_at_epoch > ?2"
        ))?;
        let Some(message) = stmt
            .query_map(params![id, Utc::now().timestamp()], row_to_outbox)?
            .next()
            .transpose()?
        else {
            drop(stmt);
            tx.commit()?;
            return Ok(None);
        };
        drop(stmt);
        // The draft is reborn in the composer's format: addresses
        // joined by ", " (the field as it is typed), body and flag as
        // the journal carries them.
        let now = Utc::now().timestamp_millis();
        self.conn().execute(
            "INSERT INTO drafts (account_id, to_raw, cc_raw, bcc_raw, subject, body, body_html, important, updated_epoch)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                message.account_id,
                message.to.join(", "),
                message.cc.join(", "),
                message.bcc.join(", "),
                message.subject,
                message.body_text,
                message.body_html,
                message.important,
                now,
            ],
        )?;
        let draft_id = self.conn().last_insert_rowid();
        // The bytes live in the journal as long as the send has not
        // gone out (PJ-D7): the copy goes back to the draft whole.
        self.conn().execute(
            "INSERT INTO draft_attachments (draft_id, name, mime, size, bytes)
             SELECT ?1, name, mime, size, bytes FROM outbox_attachments
             WHERE outbox_id = ?2 AND bytes IS NOT NULL ORDER BY id",
            params![draft_id, id],
        )?;
        self.conn()
            .execute("DELETE FROM outbox WHERE id = ?1", [id])?;
        tx.commit()?;
        Ok(Some(draft_id))
    }

    /// The send queue of ONE account, in emission order — each flush
    /// goes through its account's SMTP connection. A scheduled send
    /// (R2) only appears once its deadline has passed: the filter lives
    /// HERE, the single gate of the flush — no caller can make a
    /// scheduled send go out early.
    pub fn outbox_to_send(&self, account_id: i64) -> Result<Vec<OutboxMessage>, Error> {
        let mut stmt = self.conn().prepare(&format!(
            "{OUTBOX_SELECT} WHERE account_id = ?1 AND state = 'queued'
               AND (send_at_epoch IS NULL OR send_at_epoch <= ?2) ORDER BY id"
        ))?;
        let rows = stmt
            .query_map(params![account_id, Utc::now().timestamp()], row_to_outbox)?
            .collect::<Result<Vec<_>, _>>()?;
        self.load_outbox_attachments(rows)
    }

    /// Is there anything to flush for this account? A COUNT — the flush
    /// used to ask by rereading the whole queue, attachment bytes
    /// included (PLAN-AUDIT-V2 E7).
    pub fn outbox_pending_count(&self, account_id: i64) -> Result<u64, Error> {
        let count: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM outbox WHERE account_id = ?1 AND state = 'queued'
               AND (send_at_epoch IS NULL OR send_at_epoch <= ?2)",
            params![account_id, Utc::now().timestamp()],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }

    /// The whole outbox WITHOUT the attachment bytes — for the status
    /// (every 10 s) and any listing: an attachment's `.len()` is not
    /// worth 25 MB reread (PLAN-AUDIT-V2 E7).
    pub fn outbox_metadata(&self) -> Result<Vec<OutboxMessage>, Error> {
        self.outbox_with(false)
    }

    /// The whole outbox, in emission order, attachments included.
    pub fn outbox(&self) -> Result<Vec<OutboxMessage>, Error> {
        self.outbox_with(true)
    }

    fn outbox_with(&self, with_bytes: bool) -> Result<Vec<OutboxMessage>, Error> {
        let mut stmt = self
            .conn()
            .prepare(&format!("{OUTBOX_SELECT} ORDER BY id"))?;
        let rows = stmt
            .query_map([], row_to_outbox)?
            .collect::<Result<Vec<_>, _>>()?;
        self.load_attachments(rows, with_bytes)
    }

    /// The messages in a given state, in emission order.
    pub fn outbox_in_state(&self, state: OutboxState) -> Result<Vec<OutboxMessage>, Error> {
        let mut stmt = self
            .conn()
            .prepare(&format!("{OUTBOX_SELECT} WHERE state = ?1 ORDER BY id"))?;
        let rows = stmt
            .query_map([state.as_str()], row_to_outbox)?
            .collect::<Result<Vec<_>, _>>()?;
        self.load_outbox_attachments(rows)
    }

    /// Attaches their attachments to reread messages — bytes included
    /// for the flush, metadata only for the status. The read path stays
    /// unique for all four callers.
    fn load_outbox_attachments(
        &self,
        messages: Vec<OutboxMessage>,
    ) -> Result<Vec<OutboxMessage>, Error> {
        self.load_attachments(messages, true)
    }

    fn load_attachments(
        &self,
        mut messages: Vec<OutboxMessage>,
        with_bytes: bool,
    ) -> Result<Vec<OutboxMessage>, Error> {
        let sql = if with_bytes {
            "SELECT name, mime, size, bytes FROM outbox_attachments
             WHERE outbox_id = ?1 ORDER BY id"
        } else {
            "SELECT name, mime, size, NULL FROM outbox_attachments
             WHERE outbox_id = ?1 ORDER BY id"
        };
        let mut stmt = self.conn().prepare(sql)?;
        for message in &mut messages {
            message.attachments = stmt
                .query_map([message.id], |row| {
                    Ok(OutboxAttachment {
                        name: row.get(0)?,
                        mime: row.get(1)?,
                        size: row.get(2)?,
                        bytes: row.get(3)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
        }
        Ok(messages)
    }

    /// PJ-D7: the message has gone out, its bytes leave the journal —
    /// the metadata stays (the history can still be read). Only `sent`
    /// purges: quarantine and refusal keep everything, a resend must
    /// stay whole.
    pub(crate) fn purge_sent_attachment_bytes(&self, id: i64) -> Result<(), Error> {
        self.conn().execute(
            "UPDATE outbox_attachments SET bytes = NULL WHERE outbox_id = ?1",
            [id],
        )?;
        Ok(())
    }

    pub(crate) fn set_outbox_state(&self, id: i64, state: OutboxState) -> Result<(), Error> {
        self.conn().execute(
            "UPDATE outbox SET state = ?2 WHERE id = ?1",
            params![id, state.as_str()],
        )?;
        Ok(())
    }

    /// Transient failure: back to the queue, reason and counter kept.
    pub(crate) fn record_transient_failure(&self, id: i64, reason: &str) -> Result<(), Error> {
        self.conn().execute(
            "UPDATE outbox
             SET state = 'queued', attempts = attempts + 1, last_error = ?2
             WHERE id = ?1",
            params![id, reason],
        )?;
        Ok(())
    }

    /// Permanent refusal: the send leaves the queue, the user will decide.
    pub(crate) fn record_rejection(&self, id: i64, reason: &str) -> Result<(), Error> {
        self.conn().execute(
            "UPDATE outbox
             SET state = 'rejected', attempts = attempts + 1, last_error = ?2
             WHERE id = ?1",
            params![id, reason],
        )?;
        Ok(())
    }

    /// Quarantines the sends found "in flight": only a crash during
    /// delivery leaves this state behind. Maybe gone out, maybe not —
    /// nothing is resent without the user.
    ///
    /// [`flush_outbox`] calls it at the head of the flush; public so
    /// the host can notice an earlier crash even offline, without
    /// opening a connection.
    pub fn quarantine_inflight(&self) -> Result<usize, Error> {
        let quarantined = self.conn().execute(
            "UPDATE outbox SET state = 'interrupted' WHERE state = 'sending'",
            [],
        )?;
        Ok(quarantined)
    }

    /// Requeues a quarantined or refused send — THE explicit user
    /// decision that "never a phantom send" requires.
    pub fn requeue_outbox(&self, id: i64) -> Result<(), Error> {
        self.conn().execute(
            "UPDATE outbox SET state = 'queued'
             WHERE id = ?1 AND state IN ('interrupted', 'rejected')",
            [id],
        )?;
        Ok(())
    }

    /// Abandons a send (user decision). `sent` sends are preserved:
    /// they are the outbox's provable history.
    pub fn delete_outbox(&self, id: i64) -> Result<(), Error> {
        self.conn()
            .execute("DELETE FROM outbox WHERE id = ?1 AND state != 'sent'", [id])?;
        Ok(())
    }
}

fn row_to_outbox(row: &rusqlite::Row<'_>) -> rusqlite::Result<OutboxMessage> {
    let state_raw: String = row.get(8)?;
    let state = OutboxState::parse(&state_raw).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            8,
            rusqlite::types::Type::Text,
            format!("unknown outbox state: {state_raw}").into(),
        )
    })?;
    let recipients: String = row.get(4)?;
    let cc_addrs: String = row.get(12)?;
    let bcc_addrs: String = row.get(13)?;
    Ok(OutboxMessage {
        id: row.get(0)?,
        account_id: row.get(1)?,
        message_id: row.get(2)?,
        from: row.get(3)?,
        to: recipients.split(TO_SEPARATOR).map(str::to_string).collect(),
        cc: split_recipients(&cc_addrs),
        bcc: split_recipients(&bcc_addrs),
        subject: row.get(5)?,
        body_text: row.get(6)?,
        body_html: row.get(14)?,
        in_reply_to: row.get(7)?,
        references: row.get(18)?,
        important: row.get(15)?,
        send_at_epoch: row.get(16)?,
        ics_reply: row.get(17)?,
        // Loaded by `load_outbox_attachments`, never here: a row does
        // not know its attachments.
        attachments: Vec::new(),
        state,
        attempts: row.get(9)?,
        last_error: row.get(10)?,
        queued_epoch: row.get(11)?,
    })
}

/// The outcome of one outbox flush.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct OutboxReport {
    /// Accepted by the sending server.
    pub sent: usize,
    /// Deferred on a transient failure — still queued, retried later.
    pub deferred: usize,
    /// Definitively refused — out of the queue, a user decision.
    pub rejected: usize,
    /// Sends found "in flight" from an earlier crash, quarantined.
    pub quarantined: usize,
}

/// Flushes the outbox to the server, in emission order.
///
/// Quarantine passes FIRST: a send interrupted by a crash never goes
/// out again on its own. Then each queued message is marked "in
/// flight" (persisted) before delivery, then "sent" after the server's
/// acknowledgment — the window of ambiguity is narrowed to the
/// delivery itself. On the first transient failure the pump stops: the
/// network is down, no point insisting, the queue survives as is.
/// How many CONSECUTIVE transient failures before a send is refused
/// (PLAN-AUDIT-V2 E7, CE decision D5): THE quarantine threshold for
/// actions, a single value — the review had found two of them. Before,
/// `attempts` was counted but never read: a poisoned message held the
/// account's queue hostage forever.
pub const SEND_THRESHOLD: u32 = Store::QUARANTINE_THRESHOLD as u32;

pub fn flush_outbox(
    transport: &mut dyn MailTransport,
    store: &mut Store,
    account_id: i64,
) -> Result<OutboxReport, Error> {
    let mut report = OutboxReport {
        quarantined: store.quarantine_inflight()?,
        ..OutboxReport::default()
    };

    for message in store.outbox_to_send(account_id)? {
        store.set_outbox_state(message.id, OutboxState::Sending)?;
        match transport.send(&message) {
            Ok(()) => {
                store.set_outbox_state(message.id, OutboxState::Sent)?;
                store.purge_sent_attachment_bytes(message.id)?;
                // E3 (PLAN-REACTIVITE): the Sent copy shows up RIGHT
                // AWAY — the local echo is born at the transition to
                // `sent`, never before ("never a phantom send"). Best
                // effort: the message HAS gone out, an echo failure
                // must not make it look lost.
                let _ = store.send_echo(message.id);
                report.sent += 1;
            }
            Err(SendError::Transient(reason)) => {
                if message.attempts + 1 >= SEND_THRESHOLD {
                    // The poison leaves the queue (D5): refused, reason
                    // stated, the user will decide — and the next one
                    // gets its turn.
                    store.record_rejection(
                        message.id,
                        &format!("{SEND_THRESHOLD} attempts: {reason}"),
                    )?;
                    report.rejected += 1;
                    continue;
                }
                store.record_transient_failure(message.id, &reason)?;
                report.deferred += 1;
                break;
            }
            Err(SendError::Permanent(reason)) => {
                // The refusal of ONE message must not block the others.
                store.record_rejection(message.id, &reason)?;
                report.rejected += 1;
            }
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compose::compose;

    /// Simulated transport: accepts, cuts the network, or refuses by subject.
    #[derive(Default)]
    struct FakeTransport {
        accepted: Vec<String>,
        calls: usize,
        network_down: bool,
        reject_subjects: Vec<String>,
    }

    impl MailTransport for FakeTransport {
        fn send(&mut self, message: &OutboxMessage) -> Result<(), SendError> {
            self.calls += 1;
            if self.network_down {
                return Err(SendError::Transient("simulated network cut".to_string()));
            }
            if self.reject_subjects.contains(&message.subject) {
                return Err(SendError::Permanent("550 simulated refusal".to_string()));
            }
            self.accepted.push(message.message_id.clone());
            Ok(())
        }
    }

    fn draft(subject: &str) -> Draft {
        compose(
            "moi@exemple.fr",
            "vous@exemple.fr",
            "",
            "",
            subject,
            "body",
            None,
        )
        .unwrap()
    }

    fn store() -> (Store, i64) {
        let store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("test@exemple.fr", "gmail")
            .unwrap();
        (store, account)
    }

    /// E7: the `References` chain of a reply = the parent's `References`
    /// plus its `Message-ID` (RFC 5322 §3.6.4), read from storage — the
    /// core is what knows it, the adapter only copies it out.
    #[test]
    fn references_carries_the_whole_chain() {
        let (mut store, account) = store();
        let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        let mut parent = crate::test_support::FakeServer::simple_envelope(1, "subject");
        parent.message_id = Some("<c@x>".to_string());
        store.upsert_envelopes(inbox, &[parent]).unwrap();
        assert_eq!(
            store.references_of(account, "INBOX", 1).unwrap().as_deref(),
            Some("<c@x>"),
            "with no known References, the parent's Message-ID alone"
        );
        store
            .conn()
            .execute(
                "UPDATE envelopes SET refs = '<a@x> <b@x>' WHERE mailbox_id = ?1 AND uid = 1",
                [inbox],
            )
            .unwrap();
        assert_eq!(
            store.references_of(account, "INBOX", 1).unwrap().as_deref(),
            Some("<a@x> <b@x> <c@x>")
        );
        assert_eq!(store.references_of(account, "INBOX", 9).unwrap(), None);
    }

    #[test]
    fn enqueue_journals_everything_before_any_network() {
        let (store, account) = store();
        let composed = compose(
            "moi@exemple.fr",
            "a@exemple.fr, b@exemple.fr",
            "",
            "",
            "Subject",
            "Body\non two lines",
            Some("<origine@exemple.fr>"),
        )
        .unwrap();
        let id = store.enqueue_outbox(account, &composed).unwrap();

        let queued = store.outbox_in_state(OutboxState::Queued).unwrap();
        assert_eq!(queued.len(), 1);
        let message = &queued[0];
        assert_eq!(message.id, id);
        assert_eq!(message.message_id, composed.message_id);
        assert_eq!(message.from, "moi@exemple.fr");
        assert_eq!(message.to, vec!["a@exemple.fr", "b@exemple.fr"]);
        assert_eq!(message.subject, "Subject");
        assert_eq!(message.body_text, "Body\non two lines");
        assert_eq!(message.in_reply_to.as_deref(), Some("<origine@exemple.fr>"));
        assert_eq!(message.attempts, 0);
        assert_eq!(message.last_error, None);
    }

    /// PLAN-COMPOSITION-HTML: the rich body survives the enqueue and
    /// the reread — it is what the flush will hand to mail-smtp for the
    /// text/html part. A text-only send reads back `None`, the
    /// historical path.
    #[test]
    fn enqueue_roundtrips_body_html() {
        let (store, account) = store();
        let mut rich = draft("Subject");
        rich.body_html = Some("<b>body</b>".to_string());
        store.enqueue_outbox(account, &rich).unwrap();
        let plain = draft("Subject 2");
        store.enqueue_outbox(account, &plain).unwrap();

        let queued = store.outbox_to_send(account).unwrap();
        assert_eq!(queued[0].body_html.as_deref(), Some("<b>body</b>"));
        assert_eq!(queued[0].body_text, "body", "the text stays the fallback");
        assert_eq!(queued[1].body_html, None);
    }

    /// R3 (PLAN-RETOURS-6): the "important" flag survives the enqueue
    /// and the reread — it is the journal entry that the flush hands to
    /// mail-smtp, the priority headers depend on it.
    #[test]
    fn enqueue_roundtrips_important() {
        let (store, account) = store();
        let mut urgent = draft("urgent");
        urgent.important = true;
        store.enqueue_outbox(account, &urgent).unwrap();
        store.enqueue_outbox(account, &draft("ordinary")).unwrap();

        let queued = store.outbox_to_send(account).unwrap();
        assert!(queued[0].important, "the journal carries the flag");
        assert!(!queued[1].important, "the ordinary send stays ordinary");
    }

    /// PLAN-INVITATIONS: the journal's iTIP reply survives the enqueue
    /// and the reread — it is what mail-smtp carries in a
    /// `text/calendar; method=REPLY` part; an ordinary send stays NULL.
    #[test]
    fn enqueue_roundtrips_ics_reply() {
        let (store, account) = store();
        let mut reply = draft("reply");
        reply.ics_reply = Some("BEGIN:VCALENDAR\r\nMETHOD:REPLY\r\nEND:VCALENDAR\r\n".into());
        store.enqueue_outbox(account, &reply).unwrap();
        store.enqueue_outbox(account, &draft("ordinary")).unwrap();

        let queued = store.outbox_to_send(account).unwrap();
        assert!(
            queued[0]
                .ics_reply
                .as_deref()
                .is_some_and(|ics| ics.contains("METHOD:REPLY"))
        );
        assert_eq!(queued[1].ics_reply, None);
    }

    /// A54: the journal's Cc/Bcc survive the enqueue and the reread; a
    /// send without a copy reads them back EMPTY, never a phantom
    /// `[""]` (the guard in `split_recipients`).
    #[test]
    fn enqueue_roundtrips_cc_and_bcc() {
        let (with_store, account) = store();
        let with_copies = compose(
            "moi@exemple.fr",
            "a@exemple.fr",
            "b@exemple.fr, c@exemple.fr",
            "secret@exemple.fr",
            "Subject",
            "body",
            None,
        )
        .unwrap();
        with_store.enqueue_outbox(account, &with_copies).unwrap();
        let queued = with_store.outbox_to_send(account).unwrap();
        assert_eq!(queued[0].cc, vec!["b@exemple.fr", "c@exemple.fr"]);
        assert_eq!(queued[0].bcc, vec!["secret@exemple.fr"]);

        let (plain_store, account) = store();
        plain_store
            .enqueue_outbox(account, &draft("plain"))
            .unwrap();
        let plain = plain_store.outbox_to_send(account).unwrap();
        assert!(plain[0].cc.is_empty(), "no phantom Cc recipient");
        assert!(plain[0].bcc.is_empty(), "no phantom Bcc recipient");
    }

    /// R2 (PLAN-RETOURS-6): a scheduled send waits for its hour — the
    /// flush ignores it until the deadline has passed, then picks it up
    /// like any other queued send. The deadline reads back (it
    /// survives, golden rule n°1 extended to the chosen time).
    #[test]
    fn scheduled_send_waits_for_its_hour() {
        let (mut store, account) = store();
        let future = Utc::now().timestamp() + 3600;
        let scheduled = store
            .enqueue_outbox_full(account, &draft("later"), None, Some(future))
            .unwrap();
        store
            .enqueue_outbox_full(
                account,
                &draft("due"),
                None,
                Some(Utc::now().timestamp() - 60),
            )
            .unwrap();
        let mut transport = FakeTransport::default();

        let report = flush_outbox(&mut transport, &mut store, account).unwrap();

        assert_eq!(report.sent, 1, "only the due one goes out");
        let sent = store.outbox_in_state(OutboxState::Sent).unwrap();
        assert_eq!(sent[0].subject, "due");
        let queued = store.outbox_in_state(OutboxState::Queued).unwrap();
        assert_eq!(queued.len(), 1, "the scheduled one still waits");
        assert_eq!(queued[0].id, scheduled);
        assert_eq!(
            queued[0].send_at_epoch,
            Some(future),
            "the deadline reads back"
        );
    }

    /// Golden rule n°1: the send intent survives the process stopping.
    #[test]
    fn queued_send_survives_process_restart() {
        let path = std::env::temp_dir().join(format!("wind-test-outbox-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let store = Store::open(&path).unwrap();
            let account = store
                .adopt_or_create_account("test@exemple.fr", "gmail")
                .unwrap();
            store.enqueue_outbox(account, &draft("survivor")).unwrap();
        } // "crash": the process stops before any send.

        let reopened = Store::open(&path).unwrap();
        let queued = reopened.outbox_in_state(OutboxState::Queued).unwrap();
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].subject, "survivor");

        drop(reopened);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn flush_sends_in_emission_order_and_marks_sent() {
        let (mut store, account) = store();
        let first = draft("first");
        let second = draft("second");
        store.enqueue_outbox(account, &first).unwrap();
        store.enqueue_outbox(account, &second).unwrap();
        let mut transport = FakeTransport::default();

        let report = flush_outbox(&mut transport, &mut store, account).unwrap();

        assert_eq!(report.sent, 2);
        assert_eq!(
            transport.accepted,
            vec![first.message_id, second.message_id],
            "the emission order must be preserved"
        );
        assert!(
            store
                .outbox_in_state(OutboxState::Queued)
                .unwrap()
                .is_empty()
        );
        assert_eq!(store.outbox_in_state(OutboxState::Sent).unwrap().len(), 2);
        // E3: every send that went out has its Sent echo — the copy
        // shows up without waiting for the server's poll.
        assert_eq!(store.count_echos("envoyes", Some(account)).unwrap(), 2);
    }

    /// Golden rule n°1: a network cut loses nothing — the queue
    /// survives and goes out at the next flush.
    #[test]
    fn network_cut_keeps_message_queued_then_next_flush_sends_it() {
        let (mut store, account) = store();
        store.enqueue_outbox(account, &draft("to retry")).unwrap();

        let mut down = FakeTransport {
            network_down: true,
            ..FakeTransport::default()
        };
        let cut = flush_outbox(&mut down, &mut store, account).unwrap();
        assert_eq!((cut.sent, cut.deferred), (0, 1));
        let queued = store.outbox_in_state(OutboxState::Queued).unwrap();
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].attempts, 1);
        assert_eq!(
            queued[0].last_error.as_deref(),
            Some("simulated network cut")
        );

        let mut up = FakeTransport::default();
        let recovered = flush_outbox(&mut up, &mut store, account).unwrap();
        assert_eq!(recovered.sent, 1);
        assert_eq!(store.outbox_in_state(OutboxState::Sent).unwrap().len(), 1);
    }

    /// Network down: no point hammering the server for each message.
    #[test]
    fn transient_failure_stops_the_pump_after_one_attempt() {
        let (mut store, account) = store();
        store.enqueue_outbox(account, &draft("a")).unwrap();
        store.enqueue_outbox(account, &draft("b")).unwrap();
        let mut down = FakeTransport {
            network_down: true,
            ..FakeTransport::default()
        };

        flush_outbox(&mut down, &mut store, account).unwrap();

        assert_eq!(
            down.calls, 1,
            "a single attempt is enough to notice the cut"
        );
        assert_eq!(store.outbox_in_state(OutboxState::Queued).unwrap().len(), 2);
    }

    /// PLAN-AUDIT-V2 E7 (D5): a poisoned message — transient on every
    /// cycle — held the account's queue hostage forever (`attempts`
    /// counted, never read). On the fifth failure it is REFUSED, the
    /// user will decide, and the next one gets its turn.
    #[test]
    fn five_transient_failures_reject_the_message_and_free_the_queue() {
        let (mut store, account) = store();
        store.enqueue_outbox(account, &draft("a")).unwrap();
        store.enqueue_outbox(account, &draft("b")).unwrap();
        let mut down = FakeTransport {
            network_down: true,
            ..FakeTransport::default()
        };
        for _ in 0..SEND_THRESHOLD {
            flush_outbox(&mut down, &mut store, account).unwrap();
        }
        let rejected = store.outbox_in_state(OutboxState::Rejected).unwrap();
        assert_eq!(rejected.len(), 1, "\"a\" is refused on the fifth failure");
        assert!(
            rejected[0]
                .last_error
                .as_deref()
                .unwrap_or("")
                .contains("5 attempts"),
            "{:?}",
            rejected[0].last_error
        );
        let queued = store.outbox_in_state(OutboxState::Queued).unwrap();
        assert_eq!(queued.len(), 1);
        assert_eq!(
            queued[0].attempts, 1,
            "\"b\" got its turn on the fifth cycle"
        );
    }

    /// PLAN-AUDIT-V2 E7: the status (every 10 s) and the flush's
    /// emptiness guard used to load the BYTES of every attachment —
    /// 25 MB × N reread for a `.len()`. The status reads the metadata.
    #[test]
    fn the_status_loads_no_attachment_bytes() {
        let (store, account) = store();
        store.enqueue_outbox(account, &draft("a")).unwrap();
        let id = store.outbox().unwrap()[0].id;
        store
            .conn()
            .execute(
                "INSERT INTO outbox_attachments (outbox_id, name, mime, size, bytes)
                 VALUES (?1, 'a.pdf', 'application/pdf', 3, X'010203')",
                [id],
            )
            .unwrap();
        let full = store.outbox().unwrap();
        assert!(
            full[0].attachments[0].bytes.is_some(),
            "the flush reads the bytes"
        );
        let light = store.outbox_metadata().unwrap();
        assert_eq!(light[0].attachments.len(), 1);
        assert_eq!(light[0].attachments[0].size, 3);
        assert!(
            light[0].attachments[0].bytes.is_none(),
            "the status does not read them"
        );
        assert_eq!(store.outbox_pending_count(account).unwrap(), 1);
    }

    #[test]
    fn permanent_rejection_steps_aside_and_the_rest_still_goes() {
        let (mut store, account) = store();
        store.enqueue_outbox(account, &draft("bad")).unwrap();
        store.enqueue_outbox(account, &draft("good")).unwrap();
        let mut transport = FakeTransport {
            reject_subjects: vec!["bad".to_string()],
            ..FakeTransport::default()
        };

        let report = flush_outbox(&mut transport, &mut store, account).unwrap();

        assert_eq!((report.sent, report.rejected), (1, 1));
        let rejected = store.outbox_in_state(OutboxState::Rejected).unwrap();
        assert_eq!(rejected.len(), 1);
        assert_eq!(
            rejected[0].last_error.as_deref(),
            Some("550 simulated refusal")
        );

        // The refusal is final: the next flush does not retry it.
        let mut second = FakeTransport::default();
        let idle = flush_outbox(&mut second, &mut store, account).unwrap();
        assert_eq!(second.calls, 0);
        assert_eq!(idle, OutboxReport::default());
    }

    /// Golden rule n°2: a send interrupted in flight (a crash during
    /// delivery) is NEVER resent automatically — quarantine.
    #[test]
    fn inflight_message_is_quarantined_never_resent() {
        let (mut store, account) = store();
        let id = store.enqueue_outbox(account, &draft("ambiguous")).unwrap();
        // Simulated crash: the "sending" state persists, the
        // acknowledgment never came back. Maybe gone out, maybe not.
        store.set_outbox_state(id, OutboxState::Sending).unwrap();

        let mut transport = FakeTransport::default();
        let report = flush_outbox(&mut transport, &mut store, account).unwrap();

        assert_eq!(report.quarantined, 1);
        assert_eq!(transport.calls, 0, "nothing must go out on its own");
        let interrupted = store.outbox_in_state(OutboxState::Interrupted).unwrap();
        assert_eq!(interrupted.len(), 1);
        assert_eq!(interrupted[0].id, id);
    }

    /// Coming out of quarantine is a user decision — and only then does
    /// the send go out.
    #[test]
    fn user_requeue_is_the_only_way_out_of_quarantine() {
        let (mut store, account) = store();
        let id = store.enqueue_outbox(account, &draft("confirmed")).unwrap();
        store.set_outbox_state(id, OutboxState::Sending).unwrap();
        let mut transport = FakeTransport::default();
        flush_outbox(&mut transport, &mut store, account).unwrap();
        assert!(transport.accepted.is_empty());

        store.requeue_outbox(id).unwrap();
        let report = flush_outbox(&mut transport, &mut store, account).unwrap();

        assert_eq!(report.sent, 1);
        assert_eq!(store.outbox_in_state(OutboxState::Sent).unwrap().len(), 1);
    }

    #[test]
    fn requeue_ignores_states_that_are_not_user_decisions() {
        let (mut store, account) = store();
        let id = store
            .enqueue_outbox(account, &draft("already gone"))
            .unwrap();
        let mut transport = FakeTransport::default();
        flush_outbox(&mut transport, &mut store, account).unwrap();

        store.requeue_outbox(id).unwrap();

        assert_eq!(
            store.outbox_in_state(OutboxState::Sent).unwrap().len(),
            1,
            "an accepted send never becomes a send candidate again"
        );
    }

    #[test]
    fn delete_abandons_pending_but_preserves_sent_history() {
        let (mut store, account) = store();
        let kept = store.enqueue_outbox(account, &draft("gone out")).unwrap();
        let mut transport = FakeTransport::default();
        flush_outbox(&mut transport, &mut store, account).unwrap();
        let abandoned = store.enqueue_outbox(account, &draft("abandoned")).unwrap();

        store.delete_outbox(abandoned).unwrap();
        store.delete_outbox(kept).unwrap();

        let all = store.outbox().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].state, OutboxState::Sent);
    }

    /// Each account flushes ITS OWN queue through ITS OWN SMTP
    /// connection: one account's flush never touches another's queue.
    #[test]
    fn flush_only_sends_the_given_accounts_queue() {
        let (mut store, account) = store();
        let other = store
            .adopt_or_create_account("autre@exemple.fr", "gmail")
            .unwrap();
        store
            .enqueue_outbox(account, &draft("from account A"))
            .unwrap();
        store
            .enqueue_outbox(other, &draft("from account B"))
            .unwrap();
        let mut transport = FakeTransport::default();

        let report = flush_outbox(&mut transport, &mut store, account).unwrap();

        assert_eq!(report.sent, 1);
        assert_eq!(
            store.outbox_to_send(other).unwrap().len(),
            1,
            "B's queue is waiting for ITS OWN connection"
        );
    }

    #[test]
    fn outbox_state_labels_roundtrip() {
        for state in [
            OutboxState::Queued,
            OutboxState::Sending,
            OutboxState::Sent,
            OutboxState::Interrupted,
            OutboxState::Rejected,
        ] {
            assert_eq!(OutboxState::parse(state.as_str()), Some(state));
        }
        assert_eq!(OutboxState::parse("unknown"), None);
    }
}

#[cfg(test)]
mod tests_pieces {
    use super::*;
    use crate::compose::compose;
    use crate::drafts::DraftContent;

    /// Simulated transport, reduced to what this module checks: the
    /// attachments seen at delivery — what the transport receives is
    /// what goes out.
    #[derive(Default)]
    struct FakeTransport {
        attachments_seen: Vec<(String, bool)>,
    }

    impl MailTransport for FakeTransport {
        fn send(&mut self, message: &OutboxMessage) -> Result<(), SendError> {
            for attachment in &message.attachments {
                self.attachments_seen
                    .push((attachment.name.clone(), attachment.bytes.is_some()));
            }
            Ok(())
        }
    }

    fn store() -> (Store, i64) {
        let store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("test@exemple.fr", "gmail")
            .unwrap();
        (store, account)
    }

    fn draft_with_attachments(store: &Store, account: i64) -> i64 {
        let id = store
            .save_draft(
                account,
                None,
                None,
                DraftContent {
                    to_raw: "vous@exemple.fr",
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
        store
            .add_draft_attachment(id, "facade.jpg", "image/jpeg", &[1, 2, 3])
            .unwrap();
        store
            .add_draft_attachment(id, "devis.pdf", "application/pdf", &[4, 5])
            .unwrap();
        id
    }

    fn composed() -> Draft {
        compose(
            "moi@exemple.fr",
            "vous@exemple.fr",
            "",
            "",
            "Photos",
            "body",
            None,
        )
        .unwrap()
    }

    /// PJ-D2: the gesture copies the attachments to the journal — the
    /// draft can then disappear (send = it has served its purpose), the
    /// journal is self-sufficient.
    #[test]
    fn enqueue_copies_pieces_and_survives_draft_deletion() {
        let (store, account) = store();
        let draft_id = draft_with_attachments(&store, account);
        store
            .enqueue_outbox_from_draft(account, &composed(), draft_id)
            .unwrap();

        store.delete_draft(draft_id).unwrap();

        let queued = store.outbox_in_state(OutboxState::Queued).unwrap();
        assert_eq!(queued.len(), 1);
        let attachments = &queued[0].attachments;
        assert_eq!(attachments.len(), 2);
        assert_eq!(attachments[0].name, "facade.jpg");
        assert_eq!(attachments[0].mime, "image/jpeg");
        assert_eq!(attachments[0].size, 3);
        assert_eq!(attachments[0].bytes.as_deref(), Some(&[1u8, 2, 3][..]));
        assert_eq!(attachments[1].name, "devis.pdf");
        assert_eq!(attachments[1].bytes.as_deref(), Some(&[4u8, 5][..]));
    }

    /// Golden rule n°1, extended: a crash between the gesture and the
    /// flush loses no bytes — the attachments survive the process
    /// stopping.
    #[test]
    fn queued_pieces_survive_process_restart() {
        let path =
            std::env::temp_dir().join(format!("wind-test-outbox-pieces-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let store = Store::open(&path).unwrap();
            let account = store
                .adopt_or_create_account("test@exemple.fr", "gmail")
                .unwrap();
            let draft_id = draft_with_attachments(&store, account);
            store
                .enqueue_outbox_from_draft(account, &composed(), draft_id)
                .unwrap();
        } // "crash": the process stops before any flush.

        let reopened = Store::open(&path).unwrap();
        let queued = reopened.outbox_in_state(OutboxState::Queued).unwrap();
        assert_eq!(queued[0].attachments.len(), 2);
        assert!(queued[0].attachments.iter().all(|p| p.bytes.is_some()));

        drop(reopened);
        let _ = std::fs::remove_file(&path);
    }

    /// Delivery receives the bytes; PJ-D7: the moment it goes out, the
    /// journal purges them — the metadata stays, the history can still
    /// be read.
    #[test]
    fn sent_pieces_are_purged_to_metadata_only() {
        let (mut store, account) = store();
        let draft_id = draft_with_attachments(&store, account);
        store
            .enqueue_outbox_from_draft(account, &composed(), draft_id)
            .unwrap();
        let mut transport = FakeTransport::default();

        let report = flush_outbox(&mut transport, &mut store, account).unwrap();

        assert_eq!(report.sent, 1);
        assert_eq!(
            transport.attachments_seen,
            vec![
                ("facade.jpg".to_string(), true),
                ("devis.pdf".to_string(), true)
            ],
            "delivery goes out with the bytes"
        );
        let sent = store.outbox_in_state(OutboxState::Sent).unwrap();
        let attachments = &sent[0].attachments;
        assert_eq!(attachments.len(), 2, "the metadata stays");
        assert_eq!(attachments[0].name, "facade.jpg");
        assert_eq!(attachments[0].size, 3);
        assert!(
            attachments.iter().all(|p| p.bytes.is_none()),
            "the bytes have left the journal"
        );
    }

    /// PJ-D7, the other half: quarantine KEEPS its bytes — a resend on
    /// the user's decision must stay whole.
    #[test]
    fn quarantined_pieces_keep_their_bytes_and_requeue_sends_them() {
        let (mut store, account) = store();
        let draft_id = draft_with_attachments(&store, account);
        let id = store
            .enqueue_outbox_from_draft(account, &composed(), draft_id)
            .unwrap();
        // Simulated crash during delivery: the "sending" state persists.
        store.set_outbox_state(id, OutboxState::Sending).unwrap();

        let mut transport = FakeTransport::default();
        flush_outbox(&mut transport, &mut store, account).unwrap();
        let interrupted = store.outbox_in_state(OutboxState::Interrupted).unwrap();
        assert!(
            interrupted[0].attachments.iter().all(|p| p.bytes.is_some()),
            "quarantine keeps everything"
        );

        store.requeue_outbox(id).unwrap();
        let report = flush_outbox(&mut transport, &mut store, account).unwrap();
        assert_eq!(report.sent, 1);
        assert!(
            transport.attachments_seen.iter().all(|(_, bytes)| *bytes),
            "the resend goes out whole"
        );
    }

    /// Abandoning a queued send carries away its blobs (cascade) — no
    /// orphaned bytes left in the journal.
    #[test]
    fn deleting_a_pending_send_cascades_to_its_pieces() {
        let (store, account) = store();
        let draft_id = draft_with_attachments(&store, account);
        let id = store
            .enqueue_outbox_from_draft(account, &composed(), draft_id)
            .unwrap();

        store.delete_outbox(id).unwrap();

        let orphans: i64 = store
            .conn()
            .query_row("SELECT COUNT(*) FROM outbox_attachments", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(orphans, 0);
    }

    /// R2, CE decision D2: cancelling a scheduled send recreates the
    /// draft WHOLE — recipients, body, "important" flag, attachments
    /// with their bytes — and the entry leaves the journal. Nothing is
    /// lost, the gesture is reversible.
    #[test]
    fn cancelling_a_scheduled_send_recreates_the_draft() {
        let (store, account) = store();
        let draft_id = draft_with_attachments(&store, account);
        let mut urgent = composed();
        urgent.important = true;
        let id = store
            .enqueue_outbox_full(
                account,
                &urgent,
                Some(draft_id),
                Some(chrono::Utc::now().timestamp() + 3600),
            )
            .unwrap();
        // The real flow deletes the draft the moment the send is journaled.
        store.delete_draft(draft_id).unwrap();

        let recreated = store
            .cancel_scheduled_send(id)
            .unwrap()
            .expect("a recreated draft");

        assert!(
            store.outbox().unwrap().is_empty(),
            "the entry leaves the journal"
        );
        let drafts = store.drafts().unwrap();
        assert_eq!(drafts.len(), 1);
        let draft = &drafts[0];
        assert_eq!(draft.id, recreated);
        assert_eq!(draft.to_raw, "vous@exemple.fr");
        assert_eq!(draft.subject, "Photos");
        assert_eq!(draft.body, "body");
        assert!(draft.important, "the flag comes back");
        let attachments = store.draft_attachments_full(recreated).unwrap();
        assert_eq!(attachments.len(), 2);
        assert_eq!(attachments[0].name, "facade.jpg");
        assert_eq!(
            attachments[0].bytes,
            vec![1, 2, 3],
            "the bytes come back whole"
        );
    }

    /// D2, the other half: an entry that has already GONE OUT (the
    /// flush picked it up before the gesture) does not cancel — `None`,
    /// and the history stays. Nor does an ordinary entry (no deadline):
    /// it may be in the middle of delivery by a concurrent flush —
    /// abandoning an ordinary send stays `delete_outbox`.
    #[test]
    fn cancelling_a_send_that_is_gone_does_nothing() {
        let (mut store, account) = store();
        let draft_id = draft_with_attachments(&store, account);
        let id = store
            .enqueue_outbox_from_draft(account, &composed(), draft_id)
            .unwrap();
        assert_eq!(
            store.cancel_scheduled_send(id).unwrap(),
            None,
            "an entry WITHOUT a deadline does not go through this path"
        );
        let mut transport = FakeTransport::default();
        flush_outbox(&mut transport, &mut store, account).unwrap();

        assert_eq!(store.cancel_scheduled_send(id).unwrap(), None);
        assert_eq!(
            store.outbox_in_state(OutboxState::Sent).unwrap().len(),
            1,
            "the send history does not move"
        );
    }

    /// A send without a draft (a composition never saved) stays
    /// possible: the historical path requires no attachment.
    #[test]
    fn plain_enqueue_still_carries_no_pieces() {
        let (store, account) = store();
        store.enqueue_outbox(account, &composed()).unwrap();
        let queued = store.outbox_in_state(OutboxState::Queued).unwrap();
        assert!(queued[0].attachments.is_empty());
    }
}

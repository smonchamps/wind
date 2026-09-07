use crate::{Error, MailboxIdentity, Store, Uid};
use rusqlite::{OptionalExtension, params};

#[derive(Clone, Copy)]
pub(crate) enum Kind {
    Bodies,
    Recipients,
    Headers,
}

impl Kind {
    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::Bodies => "bodies",
            Self::Recipients => "recipients",
            Self::Headers => "headers",
        }
    }

    fn eligible(self) -> String {
        match self {
            Self::Bodies => format!(
                "{} AND NOT EXISTS (SELECT 1 FROM body_download_refusals r WHERE r.mailbox_id = e.mailbox_id AND r.uid = e.uid AND r.limit_bytes >= {})",
                crate::store::sql::BODY_ABSENT,
                crate::REMOTE_MESSAGE_BYTES
            ),
            Self::Recipients => "e.to_addrs IS NULL".into(),
            Self::Headers => "e.refs IS NULL".into(),
        }
    }
}

pub(crate) struct Window {
    pub uids: Vec<Uid>,
    pub scanned: usize,
    pub at_end: bool,
}

impl Store {
    pub fn backfill_order(
        &self,
        scope: &str,
        mailboxes: &mut [MailboxIdentity],
    ) -> Result<(), Error> {
        let key = format!("backfill_turn.{scope}");
        if let Some(last) = self.text_pref(&key)?.and_then(|s| s.parse::<i64>().ok())
            && let Some(index) = mailboxes.iter().position(|m| m.mailbox_id == last)
        {
            let next = (index + 1) % mailboxes.len();
            mailboxes.rotate_left(next);
        }
        Ok(())
    }

    pub fn mark_backfill_turn(&self, scope: &str, mailbox_id: i64) -> Result<(), Error> {
        self.set_text_pref(&format!("backfill_turn.{scope}"), &mailbox_id.to_string())
    }

    /// Claim attempted positions, not completed downloads. Failed work returns after wrap.
    ///
    /// Arrivals first: every UID above the cursor's head (the highest UID it
    /// has ever examined) is scanned before the historical sweep resumes, so
    /// a burst of new messages never waits for a full wrap of the mailbox.
    pub(crate) fn claim_backfill_window(
        &self,
        identity: &MailboxIdentity,
        kind: Kind,
        since: i64,
        limit: usize,
    ) -> Result<Window, Error> {
        if limit == 0 {
            return Ok(Window {
                uids: Vec::new(),
                scanned: 0,
                at_end: false,
            });
        }
        let tx = self.conn().unchecked_transaction()?;
        self.verify_mailbox_identity(identity)?;
        self.conn().execute("INSERT INTO backfill_cursors (mailbox_id, kind, uid_validity) VALUES (?1, ?2, ?3) ON CONFLICT(mailbox_id, kind) DO UPDATE SET uid_validity = excluded.uid_validity, epoch = NULL, uid = NULL, head_uid = NULL WHERE backfill_cursors.uid_validity != excluded.uid_validity", params![identity.mailbox_id, kind.key(), identity.uid_validity])?;
        let (mut date, uid, head): (Option<i64>, Option<Uid>, Option<Uid>) =
            self.conn().query_row(
                "SELECT epoch, uid, head_uid FROM backfill_cursors WHERE mailbox_id = ?1 AND kind = ?2",
                params![identity.mailbox_id, kind.key()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?;
        let mut before_uid = uid;
        let mut need_head = uid.is_none();
        if date.is_some_and(|date| date < since) {
            date = None;
            before_uid = None;
            need_head = false;
        }
        let mut scanned = 0;
        let mut uids = Vec::new();
        let mut at_end = false;
        let predicate = kind.eligible();
        let mut head = match head {
            // First claim: the sweep itself starts at the newest message.
            None => self.conn().query_row(
                "SELECT MAX(uid) FROM envelopes WHERE mailbox_id = ?1",
                [identity.mailbox_id],
                |row| row.get::<_, Option<Uid>>(0),
            )?,
            Some(head) => {
                let sql = format!(
                    "SELECT e.uid, ({predicate}) FROM envelopes e WHERE e.mailbox_id = ?1 AND e.uid > ?2 AND (e.date_epoch IS NULL OR e.date_epoch >= ?3) ORDER BY e.uid ASC LIMIT ?4"
                );
                let rows: Vec<(Uid, bool)> = self
                    .conn()
                    .prepare_cached(&sql)?
                    .query_map(
                        params![identity.mailbox_id, head, since, limit as i64],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )?
                    .collect::<Result<_, _>>()?;
                scanned += rows.len();
                uids.extend(rows.iter().filter(|(_, ok)| *ok).map(|(uid, _)| *uid));
                Some(rows.last().map_or(head, |(uid, _)| *uid))
            }
        };
        if let Some(newest) = self.conn().query_row(
            "SELECT MAX(uid) FROM envelopes WHERE mailbox_id = ?1",
            [identity.mailbox_id],
            |row| row.get::<_, Option<Uid>>(0),
        )? && head.is_some_and(|head| head < newest)
            && scanned < limit
        {
            // Out-of-horizon arrivals were skipped by the query above, not scanned.
            head = Some(newest);
        }
        while scanned < limit {
            if need_head {
                date = match date {
                    Some(older) => self.conn().query_row("SELECT date_epoch FROM envelopes WHERE mailbox_id = ?1 AND date_epoch >= ?2 AND date_epoch < ?3 ORDER BY date_epoch DESC LIMIT 1", params![identity.mailbox_id, since, older], |r| r.get(0)).optional()?,
                    None => self.conn().query_row("SELECT date_epoch FROM envelopes WHERE mailbox_id = ?1 AND date_epoch >= ?2 ORDER BY date_epoch DESC LIMIT 1", params![identity.mailbox_id, since], |r| r.get(0)).optional()?,
                };
                before_uid = None;
            }
            let uid_bound = if before_uid.is_some() {
                "AND e.uid < ?4"
            } else {
                ""
            };
            let sql = format!(
                "SELECT e.uid, ({predicate}) FROM envelopes e INDEXED BY idx_envelopes_date WHERE e.mailbox_id = ?1 AND e.date_epoch IS ?2 {uid_bound} ORDER BY e.uid DESC LIMIT ?3"
            );
            let mut statement = self.conn().prepare_cached(&sql)?;
            let map = |r: &rusqlite::Row<'_>| Ok((r.get::<_, Uid>(0)?, r.get::<_, bool>(1)?));
            let rows: Vec<_> = if let Some(before) = before_uid {
                statement
                    .query_map(
                        params![identity.mailbox_id, date, (limit - scanned) as i64, before],
                        map,
                    )?
                    .collect::<Result<_, _>>()?
            } else {
                statement
                    .query_map(
                        params![identity.mailbox_id, date, (limit - scanned) as i64],
                        map,
                    )?
                    .collect::<Result<_, _>>()?
            };
            scanned += rows.len();
            if let Some((last, _)) = rows.last() {
                before_uid = Some(*last);
            }
            uids.extend(
                rows.iter()
                    .filter(|(_, eligible)| *eligible)
                    .map(|(uid, _)| *uid),
            );
            if scanned == limit {
                break;
            }
            if date.is_none() {
                at_end = true;
                break;
            }
            need_head = true;
        }
        if at_end {
            date = None;
            before_uid = None;
        }
        self.conn().execute(
            "UPDATE backfill_cursors SET epoch = ?3, uid = ?4, head_uid = ?5 WHERE mailbox_id = ?1 AND kind = ?2",
            params![identity.mailbox_id, kind.key(), date, before_uid, head],
        )?;
        tx.commit()?;
        Ok(Window {
            uids,
            scanned,
            at_end,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_and_folder_rotation_survive_reopen_and_reset_with_the_namespace() {
        let path = std::env::temp_dir().join(format!(
            "wind-cursor-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let identity;
        let other;
        {
            let mut store = Store::open(&path).unwrap();
            let account = store
                .adopt_or_create_account("cursor@example.test", "imap")
                .unwrap();
            let mailbox = store.create_mailbox(account, "INBOX", 1).unwrap();
            store.create_mailbox(account, "Archive", 1).unwrap();
            let envelopes: Vec<_> = (1..=6)
                .map(|uid| crate::test_support::FakeServer::simple_envelope(uid, "cursor"))
                .collect();
            store.upsert_envelopes(mailbox, &envelopes).unwrap();
            identity = store.mailbox_identity(account, "INBOX").unwrap().unwrap();
            other = store.mailbox_identity(account, "Archive").unwrap().unwrap();
            assert_eq!(
                store
                    .claim_backfill_window(&identity, Kind::Bodies, i64::MIN, 2)
                    .unwrap()
                    .uids,
                [6, 5]
            );
            store.mark_backfill_turn("bodies", mailbox).unwrap();
        }
        {
            let store = Store::open(&path).unwrap();
            let mut order = [identity.clone(), other.clone()];
            store.backfill_order("bodies", &mut order).unwrap();
            assert_eq!(order[0], other);
            assert_eq!(
                store
                    .claim_backfill_window(&identity, Kind::Bodies, i64::MIN, 2)
                    .unwrap()
                    .uids,
                [4, 3]
            );
            store.conn().execute_batch("CREATE TEMP TRIGGER fail_cursor BEFORE UPDATE ON backfill_cursors BEGIN SELECT RAISE(ABORT, 'synthetic'); END;").unwrap();
            assert!(
                store
                    .claim_backfill_window(&identity, Kind::Bodies, i64::MIN, 2)
                    .is_err()
            );
            store
                .conn()
                .execute_batch("DROP TRIGGER fail_cursor")
                .unwrap();
            assert_eq!(
                store
                    .claim_backfill_window(&identity, Kind::Bodies, i64::MIN, 2)
                    .unwrap()
                    .uids,
                [2, 1]
            );
            store.reset_mailbox(identity.mailbox_id, 2).unwrap();
            assert!(
                store
                    .claim_backfill_window(&identity, Kind::Bodies, i64::MIN, 2)
                    .is_err()
            );
            let fresh = store
                .mailbox_identity(identity.account_id, "INBOX")
                .unwrap()
                .unwrap();
            assert!(
                store
                    .claim_backfill_window(&fresh, Kind::Bodies, i64::MIN, 2)
                    .unwrap()
                    .at_end
            );
        }
        std::fs::remove_file(path).unwrap();
    }
}

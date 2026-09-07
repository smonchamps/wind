use crate::{Error, FlagState, MailboxIdentity, Store, Uid};
use rusqlite::params;
use std::collections::HashSet;

pub(crate) struct FlagWindow {
    identity: MailboxIdentity,
    pub(crate) uids: Vec<Uid>,
    before: Option<Uid>,
    next: Option<Uid>,
}

impl Store {
    pub(crate) fn flag_window(
        &self,
        identity: &MailboxIdentity,
        bound: usize,
    ) -> Result<FlagWindow, Error> {
        let tx = self.conn().unchecked_transaction()?;
        self.verify_mailbox_identity(identity)?;
        let before: Option<Uid> = tx.query_row(
            "SELECT flag_before_uid FROM mailboxes WHERE id = ?1",
            [identity.mailbox_id],
            |row| row.get(0),
        )?;
        let recent_count = if bound > 1 { (bound / 5).max(1) } else { 0 };
        let mut uids = self.recent_uids(identity.mailbox_id, recent_count)?;
        let upper = uids
            .last()
            .map_or(i64::from(u32::MAX) + 1, |uid| i64::from(*uid));
        let rotating_count = bound.saturating_sub(uids.len());
        let mut query = tx.prepare_cached(
            "SELECT uid FROM envelopes WHERE mailbox_id = ?1 AND uid < ?2
             ORDER BY uid DESC LIMIT ?3",
        )?;
        let mut read = |boundary: i64| -> Result<Vec<Uid>, Error> {
            Ok(query
                .query_map(
                    params![identity.mailbox_id, boundary, rotating_count as i64],
                    |row| row.get(0),
                )?
                .collect::<Result<_, _>>()?)
        };
        // One descending keyset bound freezes the sweep despite new arrivals.
        let mut rotating = read(before.map_or(upper, |uid| i64::from(uid).min(upper)))?;
        if rotating.is_empty() && before.is_some() {
            rotating = read(upper)?;
        }
        let next = rotating.last().copied();
        uids.extend(rotating);
        drop(query);
        tx.commit()?;
        Ok(FlagWindow {
            identity: identity.clone(),
            uids,
            before,
            next,
        })
    }

    pub(crate) fn settle_flag_window(
        &self,
        window: &FlagWindow,
        flags: &[FlagState],
    ) -> Result<usize, Error> {
        let requested: HashSet<_> = window.uids.iter().copied().collect();
        let mut returned = HashSet::new();
        if flags
            .iter()
            .any(|flag| !requested.contains(&flag.uid) || !returned.insert(flag.uid))
        {
            return Err(Error::Server(
                "unexpected or duplicate UID in flag response".into(),
            ));
        }
        let tx = self.conn().unchecked_transaction()?;
        self.verify_mailbox_identity(&window.identity)?;
        let current: Option<Uid> = tx.query_row(
            "SELECT flag_before_uid FROM mailboxes WHERE id = ?1",
            [window.identity.mailbox_id],
            |row| row.get(0),
        )?;
        if current != window.before {
            return Err(Error::StaleMailbox);
        }
        let changed = self.apply_flags_rows(window.identity.mailbox_id, flags)?;
        tx.execute(
            "UPDATE mailboxes SET flag_before_uid = ?2 WHERE id = ?1",
            params![window.identity.mailbox_id, window.next],
        )?;
        tx.commit()?;
        Ok(changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SyncEngine, test_support::FakeServer};

    fn populate(store: &mut Store) -> (i64, MailboxIdentity, FakeServer) {
        let account = store
            .adopt_or_create_account("flags@example.fr", "imap.example.fr")
            .unwrap();
        let mut server = FakeServer::new(false);
        for uid in [1, 3, 8, u32::MAX] {
            server.add(uid, "mail");
        }
        SyncEngine::default()
            .sync(&mut server, store, account, "INBOX")
            .unwrap();
        let identity = store.mailbox_identity(account, "INBOX").unwrap().unwrap();
        (account, identity, server)
    }

    #[test]
    fn flags_cursor_survives_reopen_wraps_and_resets_with_namespace() {
        let path = std::env::temp_dir().join(format!(
            "wind-flags-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let identity = {
            let mut store = Store::open(&path).unwrap();
            let (_, identity, _) = populate(&mut store);
            let window = store.flag_window(&identity, 2).unwrap();
            assert_eq!(window.uids, [u32::MAX, 8]);
            store.settle_flag_window(&window, &[]).unwrap();
            identity
        };
        {
            let store = Store::open(&path).unwrap();
            let window = store.flag_window(&identity, 2).unwrap();
            assert_eq!(window.uids, [u32::MAX, 3]);
            store.settle_flag_window(&window, &[]).unwrap();
            store.remove_local(identity.mailbox_id, 1).unwrap();
            let wrapped = store.flag_window(&identity, 2).unwrap();
            assert_eq!(wrapped.uids, [u32::MAX, 8]);
            store.reset_mailbox(identity.mailbox_id, 2).unwrap();
            assert!(matches!(
                store.settle_flag_window(&wrapped, &[]),
                Err(Error::StaleMailbox)
            ));
            let cursor: Option<u32> = store
                .conn()
                .query_row(
                    "SELECT flag_before_uid FROM mailboxes WHERE id=?1",
                    [identity.mailbox_id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(cursor, None);
        }
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
        }
    }

    #[test]
    fn flags_and_cursor_rollback_together_and_stale_windows_cannot_rewind() {
        let mut store = Store::open_in_memory().unwrap();
        let (account, identity, _) = populate(&mut store);
        let window = store.flag_window(&identity, 1).unwrap();
        let flag = FlagState {
            uid: u32::MAX,
            seen: true,
            flagged: true,
        };
        store.conn().execute_batch("CREATE TRIGGER fail_cursor BEFORE UPDATE OF flag_before_uid ON mailboxes BEGIN SELECT RAISE(ABORT, 'cursor failure'); END;").unwrap();
        assert!(store.settle_flag_window(&window, &[flag]).is_err());
        assert!(
            !store
                .recent(account, "INBOX", 0, 4)
                .unwrap()
                .iter()
                .any(|row| row.seen)
        );
        assert_eq!(store.flag_window(&identity, 1).unwrap().uids, window.uids);
        store
            .conn()
            .execute_batch("DROP TRIGGER fail_cursor")
            .unwrap();
        store.settle_flag_window(&window, &[flag]).unwrap();
        assert!(matches!(
            store.settle_flag_window(&window, &[flag]),
            Err(Error::StaleMailbox)
        ));
        assert_eq!(store.flag_window(&identity, 1).unwrap().uids, [8]);
    }

    #[test]
    fn flags_failures_and_remote_reset_do_not_consume_progress() {
        let mut store = Store::open_in_memory().unwrap();
        let (account, identity, mut server) = populate(&mut store);
        let engine = SyncEngine::default().with_flag_window(1);
        server.flag_error = true;
        assert!(
            engine
                .flags_pass(&mut server, &mut store, account, "INBOX")
                .is_err()
        );
        server.flag_error = false;
        server.reset_during_flag_fetch = true;
        assert!(matches!(
            engine.flags_pass(&mut server, &mut store, account, "INBOX"),
            Err(Error::StaleMailbox)
        ));
        assert_eq!(store.flag_window(&identity, 1).unwrap().uids, [u32::MAX]);
        assert_eq!(server.flag_batches, vec![vec![u32::MAX], vec![u32::MAX]]);
    }

    #[test]
    fn flags_reject_unrequested_and_duplicate_uids_without_advancing() {
        let mut store = Store::open_in_memory().unwrap();
        let (_, identity, _) = populate(&mut store);
        assert!(store.flag_window(&identity, 0).unwrap().uids.is_empty());
        let window = store.flag_window(&identity, 1).unwrap();
        let unexpected = FlagState {
            uid: 1,
            seen: true,
            flagged: true,
        };
        assert!(store.settle_flag_window(&window, &[unexpected]).is_err());
        let requested = FlagState {
            uid: u32::MAX,
            ..unexpected
        };
        assert!(
            store
                .settle_flag_window(&window, &[requested, requested])
                .is_err()
        );
        assert_eq!(store.flag_window(&identity, 1).unwrap().uids, window.uids);
        store.settle_flag_window(&window, &[]).unwrap();
        assert_eq!(store.flag_window(&identity, 1).unwrap().uids, [8]);
    }
}

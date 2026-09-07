use crate::{Error, Store};

pub const BACKGROUND_DISK_RESERVE: u64 = 64 * 1024 * 1024;

impl Store {
    /// Admission for BACKGROUND work only, taken once per window or pass before
    /// the network: envelope batches, body/header/recipient windows, remote
    /// draft pulls. The user's own reads and writes (a click, a draft) never
    /// pay the reserve; SQLite still arbitrates the actual write.
    pub fn admit_background_write(&self, input_bytes: u64) -> Result<(), Error> {
        #[cfg(test)]
        if let Some(available) = AVAILABLE.get() {
            return admit(input_bytes, available);
        }
        let Some(path) = self.conn().path().filter(|path| !path.is_empty()) else {
            return Ok(());
        };
        let path = std::path::Path::new(path);
        if let Ok(available) = fs4::available_space(path.parent().unwrap_or(path)) {
            admit(input_bytes, available)?;
        }
        Ok(())
    }
}

fn admit(input: u64, available: u64) -> Result<(), Error> {
    // Estimate payload, indexes and journal headroom; SQLite still arbitrates actual writes.
    let required = BACKGROUND_DISK_RESERVE
        .saturating_add(input.saturating_mul(4))
        .saturating_add(256 * 1024);
    if available < required {
        return Err(Error::InsufficientDisk {
            required,
            available,
        });
    }
    Ok(())
}

#[cfg(test)]
thread_local! { static AVAILABLE: std::cell::Cell<Option<u64>> = const { std::cell::Cell::new(None) }; }

#[cfg(test)]
pub(crate) fn with_available<T>(bytes: u64, work: impl FnOnce() -> T) -> T {
    struct Reset(Option<u64>);
    impl Drop for Reset {
        fn drop(&mut self) {
            AVAILABLE.set(self.0);
        }
    }
    let _reset = Reset(AVAILABLE.replace(Some(bytes)));
    work()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SyncEngine, test_support::FakeServer};

    #[test]
    fn missing_bodies_stop_before_network_and_resume_after_space_is_freed() {
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("disk@example.test", "imap")
            .unwrap();
        let mut server = FakeServer::new(false);
        server.add_with_body(1, "stored envelope", "<p>content</p>");
        SyncEngine::default()
            .sync(&mut server, &mut store, account, "INBOX")
            .unwrap();
        with_available(BACKGROUND_DISK_RESERVE - 1, || {
            assert!(
                crate::backfill_bodies(&mut server, &mut store, account, "INBOX", 0, 1).is_err()
            );
        });
        assert_eq!(server.body_fetches, 0);
        assert!(store.body(account, "INBOX", 1).unwrap().is_none());
        store.retry_operations(account).unwrap();
        assert_eq!(
            crate::backfill_bodies(&mut server, &mut store, account, "INBOX", 0, 1)
                .unwrap()
                .fetched,
            1
        );
    }

    #[test]
    fn a_click_reads_a_message_without_paying_the_background_reserve() {
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("disk@example.test", "imap")
            .unwrap();
        let mut server = FakeServer::new(false);
        server.add_with_body(1, "clicked", "<p>read me</p>");
        SyncEngine::default()
            .sync(&mut server, &mut store, account, "INBOX")
            .unwrap();
        let identity = store.mailbox_identity(account, "INBOX").unwrap().unwrap();
        with_available(0, || {
            assert!(
                crate::backfill_bodies(&mut server, &mut store, account, "INBOX", 0, 1).is_err(),
                "the pump waits for space"
            );
            assert_eq!(
                crate::load_body_version(&mut server, &mut store, &identity, 1)
                    .unwrap()
                    .as_deref(),
                Some("<p>read me</p>"),
                "the click reads whenever the server delivers"
            );
        });
    }

    #[test]
    fn a_full_disk_does_not_spend_the_space_reserved_for_local_drafts() {
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("disk@example.test", "imap")
            .unwrap();
        let mut server = FakeServer::new(false);
        server.add_with_body(1, "incoming", "body");
        with_available(0, || {
            assert!(
                SyncEngine::default()
                    .sync(&mut server, &mut store, account, "INBOX")
                    .is_err()
            );
            store
                .save_draft(
                    account,
                    None,
                    None,
                    crate::DraftContent {
                        to_raw: "",
                        cc_raw: "",
                        bcc_raw: "",
                        subject: "local intention",
                        body: "keep my text",
                        body_html: None,
                        reply_to_uid: None,
                        reply_to_mailbox: None,
                        important: false,
                    },
                )
                .unwrap();
        });
        assert!(server.fetch_batches.is_empty());
        assert_eq!(store.drafts().unwrap()[0].body, "keep my text");
    }
}

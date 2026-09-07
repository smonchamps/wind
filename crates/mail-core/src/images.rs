use crate::{Error, MailboxIdentity, Store};
use rusqlite::params;

impl Store {
    pub fn images_allowed_message(&self, mailbox_id: i64, uid: u32) -> Result<bool, Error> {
        Ok(self
            .conn()
            .prepare_cached("SELECT 1 FROM images_messages WHERE mailbox_id = ?1 AND uid = ?2")?
            .exists(params![mailbox_id, uid])?)
    }

    /// Changes only this message's grant. Sender-wide permission remains independent.
    pub fn set_images_message_checked(
        &self,
        identity: &MailboxIdentity,
        uid: u32,
        allowed: bool,
        epoch: i64,
    ) -> Result<bool, Error> {
        let tx = self.conn().unchecked_transaction()?;
        self.verify_image_target(identity, uid)?;
        if allowed {
            self.allow_images_message(identity.mailbox_id, uid, epoch)?;
        } else {
            tx.execute(
                "DELETE FROM images_messages WHERE mailbox_id = ?1 AND uid = ?2",
                params![identity.mailbox_id, uid],
            )?;
        }
        let remaining = self.images_allowed(identity.mailbox_id, uid)?;
        tx.commit()?;
        Ok(remaining)
    }

    pub fn allow_images_sender_checked(
        &self,
        identity: &MailboxIdentity,
        uid: u32,
        epoch: i64,
    ) -> Result<Option<String>, Error> {
        let tx = self.conn().unchecked_transaction()?;
        self.verify_image_target(identity, uid)?;
        let address = self.allow_images_sender_of(identity.mailbox_id, uid, epoch)?;
        tx.commit()?;
        Ok(address)
    }
    fn verify_image_target(&self, identity: &MailboxIdentity, uid: u32) -> Result<(), Error> {
        self.verify_mailbox_identity(identity)?;
        if !self
            .conn()
            .prepare_cached("SELECT 1 FROM envelopes WHERE mailbox_id = ?1 AND uid = ?2")?
            .exists(params![identity.mailbox_id, uid])?
        {
            return Err(Error::StaleMailbox);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::FakeServer;

    fn setup() -> (Store, MailboxIdentity) {
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("guest@example.fr", "gmail")
            .unwrap();
        let mut server = FakeServer::new(false);
        server.add_with_body(1, "Message", "body");
        server.messages.get_mut(&1).unwrap().0.sender_address = Some("owner@example.fr".into());
        crate::SyncEngine::default()
            .sync(&mut server, &mut store, account, "INBOX")
            .unwrap();
        let identity = store.mailbox_identity(account, "INBOX").unwrap().unwrap();
        (store, identity)
    }

    #[test]
    fn message_revocation_removes_only_its_grant_and_is_idempotent() {
        let (store, identity) = setup();
        store
            .set_images_message_checked(&identity, 1, true, 42)
            .unwrap();
        assert!(
            !store
                .set_images_message_checked(&identity, 1, false, 43)
                .unwrap()
        );
        assert!(
            !store
                .set_images_message_checked(&identity, 1, false, 44)
                .unwrap()
        );
        assert!(
            !store
                .images_allowed_message(identity.mailbox_id, 1)
                .unwrap()
        );
        store.allow_images_sender_checked(&identity, 1, 45).unwrap();
        store
            .set_images_message_checked(&identity, 1, true, 46)
            .unwrap();
        assert!(
            store
                .set_images_message_checked(&identity, 1, false, 47)
                .unwrap(),
            "sender rule still permits images"
        );
        assert!(
            !store
                .images_allowed_message(identity.mailbox_id, 1)
                .unwrap()
        );
        assert_eq!(store.images_senders().unwrap(), vec!["owner@example.fr"]);
    }

    #[test]
    fn stale_image_actions_cannot_change_a_reused_namespace_or_leave_orphans() {
        let (store, identity) = setup();
        assert!(
            store
                .set_images_message_checked(&identity, 99, true, 1)
                .is_err()
        );
        store.reset_mailbox(identity.mailbox_id, 2).unwrap();
        assert!(
            store
                .set_images_message_checked(&identity, 1, true, 2)
                .is_err()
        );
        assert!(
            store
                .set_images_message_checked(&identity, 1, false, 3)
                .is_err()
        );
        assert!(store.allow_images_sender_checked(&identity, 1, 4).is_err());
        assert!(
            !store
                .images_allowed_message(identity.mailbox_id, 1)
                .unwrap()
        );
    }
}

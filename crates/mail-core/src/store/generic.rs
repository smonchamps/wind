use super::*;

impl Store {
    /// Publishes connection settings and their nonsecret vault slot together.
    pub fn save_generic_connection(
        &self,
        account_id: Option<i64>,
        email: &str,
        config: &AccountConfig,
        horizon: Option<&str>,
    ) -> Result<i64, Error> {
        if !matches!(config.credential_slot.as_deref(), Some("a" | "b"))
            || config.imap_port.is_none_or(|port| port == 0)
            || config.smtp_port.is_none_or(|port| port == 0)
            || [&config.imap_host, &config.smtp_host, &config.username]
                .iter()
                .any(|value| value.as_deref().is_none_or(|text| text.trim().is_empty()))
        {
            return Err(Error::Corrupt("incomplete connection settings".into()));
        }
        if horizon.is_some_and(|value| !crate::HORIZONS_IMPORT.contains(&value)) {
            return Err(Error::Corrupt("invalid import history".into()));
        }
        let tx = self.0.unchecked_transaction()?;
        let id = if let Some(id) = account_id {
            let changed = tx.execute(
                "UPDATE accounts SET imap_port = ?4, smtp_host = ?5, smtp_port = ?6,
                     credential_slot = ?7
                 WHERE id = ?1 AND email = ?2 AND provider = 'imap'
                     AND COALESCE(username, email) = ?3 AND imap_host = ?8
                     AND credential_slot IS NOT ?7",
                params![
                    id,
                    email,
                    config.username,
                    config.imap_port,
                    config.smtp_host,
                    config.smtp_port,
                    config.credential_slot,
                    config.imap_host
                ],
            )?;
            if changed != 1 {
                return Err(Error::Corrupt(
                    "the original IMAP account changed; reopen its connection settings".into(),
                ));
            }
            // A repaired connection is a user-requested attempt: refusals
            // waiting for a manual retry become due, the diagnostic stays
            // until the operation actually succeeds.
            self.retry_operations(id)?;
            id
        } else {
            let duplicate: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM accounts WHERE email = ?1 COLLATE NOCASE)",
                [email],
                |row| row.get(0),
            )?;
            if duplicate {
                return Err(Error::Corrupt(
                    "account already exists; use its connection settings".into(),
                ));
            }
            tx.execute(
                "INSERT INTO accounts (email, provider, username, imap_host, imap_port,
                     smtp_host, smtp_port, credential_slot)
                 VALUES (?1, 'imap', ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    email,
                    config.username,
                    config.imap_host,
                    config.imap_port,
                    config.smtp_host,
                    config.smtp_port,
                    config.credential_slot
                ],
            )?;
            tx.last_insert_rowid()
        };
        if account_id.is_none()
            && let Some(horizon) = horizon
        {
            tx.execute(
                "INSERT INTO prefs (key, value) VALUES (?1, ?2)",
                params![format!("horizon_import.{id}"), horizon],
            )?;
        }
        tx.commit()?;
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_repaired_connection_releases_old_refusals_without_erasing_the_diagnostic() {
        let (store, account, mut config) = fixture();
        store
            .settle_operation(
                account,
                "Archive",
                "sync",
                100,
                Some(&Error::Refusal("old configuration".into())),
            )
            .unwrap();
        config.credential_slot = Some("a".into());
        store
            .save_generic_connection(Some(account), "owner@example.fr", &config, None)
            .unwrap();
        assert!(
            store
                .operation_due(account, "Archive", "sync", 100)
                .unwrap()
        );
        assert_eq!(store.operation_issues().unwrap().len(), 1);
    }

    fn fixture() -> (Store, i64, AccountConfig) {
        let store = Store::open_in_memory().unwrap();
        let id = store
            .create_generic_account(
                "owner@example.fr",
                "login",
                "imap.example.fr",
                993,
                "smtp.example.fr",
                465,
            )
            .unwrap();
        let config = store.account_config(id).unwrap();
        assert_eq!(config.credential_slot, None);
        (store, id, config)
    }

    #[test]
    fn credential_reference_migration_preserves_legacy_and_already_repaired_accounts() {
        let (store, id, old) = fixture();
        store
            .0
            .execute_batch("ALTER TABLE accounts DROP COLUMN credential_slot")
            .unwrap();
        migrate(&store.0, &mut |_| ControlFlow::Continue(())).unwrap();
        assert_eq!(store.account_config(id).unwrap(), old);
        let mut config = old;
        config.credential_slot = Some("a".into());
        store
            .save_generic_connection(Some(id), "owner@example.fr", &config, None)
            .unwrap();
        migrate(&store.0, &mut |_| ControlFlow::Continue(())).unwrap();
        assert_eq!(store.account_config(id).unwrap(), config);
    }

    #[test]
    fn successful_repair_preserves_mail_drafts_delivery_intentions_and_preferences() {
        let (store, id, mut config) = fixture();
        let other = store
            .adopt_or_create_account("other@example.fr", "gmail")
            .unwrap();
        let mailbox = store.create_mailbox(id, "INBOX", 42).unwrap();
        store.enqueue_action(mailbox, 7, Action::MarkSeen).unwrap();
        let saved = store
            .save_draft(
                id,
                None,
                None,
                crate::DraftContent {
                    to_raw: "recipient@example.fr",
                    body: "local draft",
                    ..Default::default()
                },
            )
            .unwrap();
        store
            .add_draft_attachment(saved.id, "fixture.txt", "text/plain", b"attachment")
            .unwrap();
        let draft = crate::compose(
            "owner@example.fr",
            "recipient@example.fr",
            "",
            "",
            "queued",
            "retained",
            None,
        )
        .unwrap();
        let outbox = store
            .enqueue_outbox_from_draft(id, &draft, saved.id)
            .unwrap();
        store
            .0
            .execute(
                "UPDATE outbox SET state = 'interrupted' WHERE id = ?1",
                [outbox],
            )
            .unwrap();
        let source = store.draft(saved.id).unwrap().unwrap();
        store
            .begin_draft_edit(
                "repair-fixture",
                id,
                Some(saved.id),
                Some(&source.incarnation),
                Some(source.updated_epoch),
            )
            .unwrap();
        store.set_horizon_import(id, "tout").unwrap(); // lang:fr
        store
            .set_text_pref(&format!("name.{id}"), "Retained name")
            .unwrap();
        let snapshot = || {
            [
                "mailboxes",
                "pending_actions",
                "drafts",
                "draft_attachments",
                "draft_blobs",
                "outbox",
                "outbox_attachments",
                "prefs",
                "draft_edit",
                "draft_edit_files",
            ]
            .map(|table| {
                let mut stmt = store
                    .0
                    .prepare(&format!("SELECT * FROM {table} ORDER BY rowid"))
                    .unwrap();
                let count = stmt.column_count();
                stmt.query_map([], |row| {
                    (0..count)
                        .map(|index| row.get::<_, rusqlite::types::Value>(index))
                        .collect::<Result<Vec<_>, _>>()
                })
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
            })
        };
        let before = snapshot();
        config.credential_slot = Some("a".into());
        config.smtp_host = Some("repaired.example.fr".into());
        config.smtp_port = Some(587);
        assert_eq!(
            store
                .save_generic_connection(Some(id), "owner@example.fr", &config, Some("1a"))
                .unwrap(),
            id
        );
        assert_eq!(snapshot(), before);
        assert_eq!(store.account_config(id).unwrap(), config);
        assert_eq!(
            store
                .accounts()
                .unwrap()
                .into_iter()
                .find(|account| account.id == other)
                .unwrap()
                .provider,
            "gmail"
        );
        assert!(
            store
                .save_generic_connection(Some(id), "owner@example.fr", &config, None)
                .is_err(),
            "the same active slot cannot be republished"
        );
    }

    #[test]
    fn repair_rejects_mailbox_identity_changes_missing_targets_and_duplicate_additions() {
        for change in 0..5 {
            let (store, id, old) = fixture();
            let mut config = old.clone();
            config.credential_slot = Some("a".into());
            let mut target = Some(id);
            let mut email = "owner@example.fr";
            match change {
                0 => config.imap_host = Some("other.example.fr".into()),
                1 => config.username = Some("another-login".into()),
                2 => email = "another@example.fr",
                3 => target = Some(id + 100),
                _ => target = None,
            }
            assert!(
                store
                    .save_generic_connection(target, email, &config, None)
                    .is_err(),
                "case {change}"
            );
            assert_eq!(store.account_config(id).unwrap(), old);
            assert_eq!(store.accounts().unwrap().len(), 1);
        }
    }

    #[test]
    fn generic_repair_cannot_convert_an_oauth_account() {
        let (store, _, mut config) = fixture();
        config.credential_slot = Some("a".into());
        let oauth = store
            .adopt_or_create_account("google@example.fr", "gmail")
            .unwrap();
        assert!(
            store
                .save_generic_connection(Some(oauth), "google@example.fr", &config, None)
                .is_err()
        );
        assert_eq!(
            store
                .accounts()
                .unwrap()
                .into_iter()
                .find(|a| a.id == oauth)
                .unwrap()
                .provider,
            "gmail"
        );
    }

    #[test]
    fn failed_slot_publication_rolls_back_every_connection_setting() {
        let (store, id, old) = fixture();
        store
            .0
            .execute_batch(
                "CREATE TRIGGER reject_slot BEFORE UPDATE OF credential_slot ON accounts
            BEGIN SELECT RAISE(ABORT, 'synthetic publication failure'); END;",
            )
            .unwrap();
        let mut config = old.clone();
        config.smtp_host = Some("replacement.example.fr".into());
        config.credential_slot = Some("a".into());
        assert!(
            store
                .save_generic_connection(Some(id), "owner@example.fr", &config, None)
                .is_err()
        );
        assert_eq!(store.account_config(id).unwrap(), old);
    }

    #[test]
    fn failed_initial_preference_write_does_not_leave_an_account() {
        let (store, _, mut config) = fixture();
        config.credential_slot = Some("a".into());
        store
            .0
            .execute_batch(
                "CREATE TRIGGER reject_pref BEFORE INSERT ON prefs
            BEGIN SELECT RAISE(ABORT, 'synthetic preference failure'); END;",
            )
            .unwrap();
        assert!(
            store
                .save_generic_connection(None, "new@example.fr", &config, Some("1a"))
                .is_err()
        );
        assert_eq!(store.account_id("new@example.fr").unwrap(), None);
    }
}

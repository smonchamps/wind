use crate::{Error, Store};
use rusqlite::{OptionalExtension, params};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationIssue {
    pub account_id: i64,
    pub mailbox: String,
    pub operation: String,
    pub reason: String,
    pub attempts: u32,
    pub retry_at: Option<i64>,
}

impl Store {
    pub fn operation_due(
        &self,
        account: i64,
        mailbox: &str,
        operation: &str,
        now: i64,
    ) -> Result<bool, Error> {
        let retry: Option<Option<i64>> = self.conn().query_row("SELECT retry_at FROM operation_issues WHERE account_id = ?1 AND mailbox = ?2 AND operation = ?3", params![account, mailbox, operation], |r| r.get(0)).optional()?;
        Ok(match retry {
            None => true,
            Some(Some(at)) => now >= at,
            Some(None) => false,
        })
    }

    pub fn settle_operation(
        &self,
        account: i64,
        mailbox: &str,
        operation: &str,
        now: i64,
        error: Option<&Error>,
    ) -> Result<(), Error> {
        let Some(error) = error else {
            self.conn().execute("DELETE FROM operation_issues WHERE account_id = ?1 AND mailbox = ?2 AND operation = ?3", params![account, mailbox, operation])?;
            return Ok(());
        };
        let tx = self.conn().unchecked_transaction()?;
        let attempts: u32 = self.conn().query_row("SELECT attempts FROM operation_issues WHERE account_id = ?1 AND mailbox = ?2 AND operation = ?3", params![account, mailbox, operation], |r| r.get(0)).optional()?.unwrap_or(0_u32).saturating_add(1);
        // A server refusal (any tagged NO/BAD, transient or not) backs off like
        // every other failure, capped at 30 minutes: a parked folder would need a
        // gesture to resume after one "NO [UNAVAILABLE]". Only a corrupt local
        // scope waits for a manual retry; per-UID size refusals keep their own
        // markers and are not operation failures.
        let retry_at = match error {
            Error::Corrupt(_) => None,
            Error::RemoteMessageTooLarge { .. } => Some(now),
            _ => Some(now.saturating_add((30_i64 << attempts.saturating_sub(1).min(6)).min(1800))),
        };
        let reason = error.to_string().chars().take(512).collect::<String>();
        self.conn().execute("INSERT INTO operation_issues (account_id, mailbox, operation, reason, attempts, retry_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6) ON CONFLICT(account_id, mailbox, operation) DO UPDATE SET reason = excluded.reason, attempts = excluded.attempts, retry_at = excluded.retry_at", params![account, mailbox, operation, reason, attempts, retry_at])?;
        tx.commit()?;
        Ok(())
    }

    /// Keep the diagnostic visible while allowing a user-requested attempt.
    pub fn retry_operations(&self, account: i64) -> Result<(), Error> {
        self.conn().execute(
            "UPDATE operation_issues SET retry_at = 0 WHERE account_id = ?1",
            [account],
        )?;
        Ok(())
    }

    pub fn operation_issues(&self) -> Result<Vec<OperationIssue>, Error> {
        Ok(self.conn().prepare("SELECT account_id, mailbox, operation, reason, attempts, retry_at FROM operation_issues ORDER BY account_id, mailbox, operation")?
            .query_map([], |r| Ok(OperationIssue { account_id: r.get(0)?, mailbox: r.get(1)?, operation: r.get(2)?, reason: r.get(3)?, attempts: r.get(4)?, retry_at: r.get(5)? }))?.collect::<Result<_, _>>()?)
    }

    pub(crate) fn attempt_operation<T>(
        &mut self,
        account: i64,
        mailbox: &str,
        operation: &str,
        work: impl FnOnce(&mut Store) -> Result<T, Error>,
    ) -> Result<Option<T>, Error> {
        let now = chrono::Utc::now().timestamp();
        if !self.operation_due(account, mailbox, operation, now)? {
            return Ok(None);
        }
        let result = work(self);
        self.settle_operation(
            account,
            mailbox,
            operation,
            chrono::Utc::now().timestamp(),
            result.as_ref().err(),
        )?;
        result.map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_retry_keeps_evidence_and_deleted_accounts_cannot_inherit_it() {
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("old@example.test", "imap")
            .unwrap();
        store
            .settle_operation(
                account,
                "Archive",
                "sync",
                100,
                Some(&Error::Corrupt("scope".into())),
            )
            .unwrap();
        store.retry_operations(account).unwrap();
        assert!(
            store
                .operation_due(account, "Archive", "sync", 100)
                .unwrap()
        );
        assert_eq!(store.operation_issues().unwrap().len(), 1);
        store.delete_account(account).unwrap();
        let replacement = store
            .adopt_or_create_account("new@example.test", "imap")
            .unwrap();
        assert_eq!(replacement, account);
        assert!(store.operation_issues().unwrap().is_empty());
    }

    #[test]
    fn success_elsewhere_does_not_clear_a_partial_failure_or_its_delay() {
        let store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("a@example.test", "imap")
            .unwrap();
        store
            .settle_operation(
                account,
                "Archive",
                "sync",
                100,
                Some(&Error::Server("timeout".into())),
            )
            .unwrap();
        store
            .settle_operation(account, "INBOX", "sync", 101, None)
            .unwrap();
        assert!(
            !store
                .operation_due(account, "Archive", "sync", 101)
                .unwrap()
        );
        let issues = store.operation_issues().unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].retry_at, Some(130));
        assert!(
            store
                .operation_due(account, "Archive", "sync", 130)
                .unwrap()
        );
        store
            .settle_operation(account, "Archive", "sync", 131, None)
            .unwrap();
        assert!(store.operation_issues().unwrap().is_empty());
    }

    #[test]
    fn corrupt_scopes_wait_for_manual_retry_and_refusals_back_off() {
        let store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("a@example.test", "imap")
            .unwrap();
        store
            .settle_operation(
                account,
                "Archive",
                "sync",
                100,
                Some(&Error::Corrupt("scope".into())),
            )
            .unwrap();
        assert!(
            !store
                .operation_due(account, "Archive", "sync", i64::MAX)
                .unwrap()
        );
        // One transient "NO [UNAVAILABLE]" must not park a folder until a gesture.
        store
            .settle_operation(
                account,
                "Old",
                "sync",
                100,
                Some(&Error::Refusal("NO [UNAVAILABLE] temporary".into())),
            )
            .unwrap();
        assert!(!store.operation_due(account, "Old", "sync", 129).unwrap());
        assert!(store.operation_due(account, "Old", "sync", 130).unwrap());
        for attempt in 1..=16 {
            store
                .settle_operation(
                    account,
                    "INBOX",
                    "headers",
                    100,
                    Some(&Error::Server("timeout".into())),
                )
                .unwrap();
            let issue = store
                .operation_issues()
                .unwrap()
                .into_iter()
                .find(|i| i.operation == "headers")
                .unwrap();
            assert_eq!(issue.attempts, attempt);
            assert_eq!(
                issue.retry_at,
                Some(100 + (30_i64 << (attempt - 1).min(6)).min(1800))
            );
        }
    }
}

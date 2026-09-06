use std::collections::HashMap;
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

use crate::commands::recovered;

#[derive(Clone, Default)]
pub(crate) struct Registry(Arc<Mutex<HashMap<i64, Ticket>>>, Arc<Provisioning>);

#[derive(Default)]
struct Provisioning {
    state: Mutex<(usize, usize)>, // active authentication flows, active removals
    drained: Condvar,
}

pub(crate) struct Registration(Registry);
impl Drop for Registration {
    fn drop(&mut self) {
        recovered(&self.0.1.state).0 -= 1;
        self.0.1.drained.notify_all();
    }
}

#[derive(Clone)]
pub(crate) struct Ticket {
    pub(crate) account_id: i64,
    life: Arc<Life>,
}

impl PartialEq for Ticket {
    fn eq(&self, other: &Self) -> bool {
        self.account_id == other.account_id && Arc::ptr_eq(&self.life, &other.life)
    }
}
impl Eq for Ticket {}
impl std::hash::Hash for Ticket {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&self.account_id, state);
        std::hash::Hash::hash(&Arc::as_ptr(&self.life), state);
    }
}

#[derive(Default)]
struct Life {
    closed: Arc<AtomicBool>,
    active: Arc<Mutex<usize>>,
    drained: Arc<Condvar>,
}

impl Registry {
    /// OAuth can reveal a different address only after writing its credential.
    /// Keep those identity-unknown flows ahead of any vault removal.
    pub(crate) fn registration(&self) -> Result<Registration, String> {
        let mut state = recovered(&self.1.state);
        if state.1 != 0 {
            return Err("finish account removal before connecting accounts".to_string());
        }
        state.0 += 1;
        Ok(Registration(self.clone()))
    }
    /// Capture only while the caller still holds the account-registry snapshot lock.
    pub(crate) fn capture(&self, account_id: i64) -> Result<Ticket, String> {
        let ticket = recovered(&self.0)
            .entry(account_id)
            .or_insert_with(|| Ticket::new(account_id))
            .clone();
        ticket.check()?;
        Ok(ticket)
    }

    pub(crate) fn is_current(&self, ticket: &Ticket) -> bool {
        recovered(&self.0)
            .get(&ticket.account_id)
            .is_some_and(|current| Arc::ptr_eq(&current.life, &ticket.life))
            && ticket.check().is_ok()
    }

    pub(crate) fn retire(&self, account_id: i64) -> Result<Retirement, String> {
        let mut provisioning = recovered(&self.1.state);
        let ticket = self.capture(account_id)?;
        {
            let _active = recovered(&ticket.life.active);
            if ticket.life.closed.swap(true, Ordering::AcqRel) {
                return Err("account removal already in progress".to_string());
            }
        }
        provisioning.1 += 1;
        Ok(Retirement {
            registry: self.clone(),
            ticket,
            committed: false,
        })
    }
}

impl Ticket {
    fn new(account_id: i64) -> Self {
        Self {
            account_id,
            life: Arc::new(Life::default()),
        }
    }

    pub(crate) fn stop_flag(&self) -> Arc<AtomicBool> {
        self.life.closed.clone()
    }

    pub(crate) fn check(&self) -> Result<(), String> {
        if self.life.closed.load(Ordering::Acquire) {
            Err("account is closing or has been removed".to_string())
        } else {
            Ok(())
        }
    }

    pub(crate) fn lease(&self) -> Result<Lease, String> {
        let mut active = recovered(&self.life.active);
        self.check()?;
        *active += 1;
        Ok(Lease(self.clone()))
    }
}

pub(crate) struct Lease(Ticket);
impl Drop for Lease {
    fn drop(&mut self) {
        let mut active = recovered(&self.0.life.active);
        *active -= 1;
        self.0.life.drained.notify_all();
    }
}

pub(crate) struct Retirement {
    registry: Registry,
    ticket: Ticket,
    committed: bool,
}

impl Retirement {
    pub(crate) fn wait(&self, timeout: Duration) -> Result<(), String> {
        let started = Instant::now();
        let mut active = recovered(&self.ticket.life.active);
        while *active != 0 {
            let remaining = timeout.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                return Err(
                    "account still has active work; nothing was removed, retry shortly".to_string(),
                );
            }
            let result = self
                .ticket
                .life
                .drained
                .wait_timeout(active, remaining)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            active = result.0;
        }
        drop(active);
        let mut provisioning = recovered(&self.registry.1.state);
        while provisioning.0 != 0 {
            let remaining = timeout.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                return Err("account connection still in progress; nothing was removed".to_string());
            }
            provisioning = self
                .registry
                .1
                .drained
                .wait_timeout(provisioning, remaining)
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .0;
        }
        Ok(())
    }

    pub(crate) fn commit(mut self) {
        let mut provisioning = recovered(&self.registry.1.state);
        let mut entries = recovered(&self.registry.0);
        if entries
            .get(&self.ticket.account_id)
            .is_some_and(|current| Arc::ptr_eq(&current.life, &self.ticket.life))
        {
            entries.remove(&self.ticket.account_id);
        }
        self.committed = true;
        provisioning.1 -= 1;
    }
}

impl Drop for Retirement {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        let mut provisioning = recovered(&self.registry.1.state);
        let mut entries = recovered(&self.registry.0);
        if entries
            .get(&self.ticket.account_id)
            .is_some_and(|current| Arc::ptr_eq(&current.life, &self.ticket.life))
        {
            // Old jobs stay stopped even when removal fails and the account reopens.
            entries.insert(
                self.ticket.account_id,
                Ticket {
                    account_id: self.ticket.account_id,
                    life: Arc::new(Life {
                        closed: Arc::new(AtomicBool::new(false)),
                        active: self.ticket.life.active.clone(),
                        drained: self.ticket.life.drained.clone(),
                    }),
                },
            );
        }
        provisioning.1 -= 1;
    }
}

#[derive(Clone)]
pub(crate) struct AccountWork {
    pub(crate) session: mail_auth::AccountSession,
    pub(crate) ticket: Ticket,
}

impl AccountWork {
    pub(crate) fn email(&self) -> &str {
        self.session.email()
    }
    pub(crate) fn refreshed(&self, session: mail_auth::AccountSession) -> Self {
        Self {
            session,
            ticket: self.ticket.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mail_core::{MailTransport, OutboxMessage, OutboxState, SendError, Store};
    use std::sync::mpsc;

    struct HeldTransport {
        entered: mpsc::Sender<()>,
        release: mpsc::Receiver<()>,
        unknown: bool,
    }

    impl MailTransport for HeldTransport {
        fn send(&mut self, _: &OutboxMessage) -> Result<(), SendError> {
            self.entered.send(()).unwrap();
            self.release.recv_timeout(Duration::from_secs(3)).unwrap();
            if self.unknown {
                Err(SendError::Unknown(
                    "synthetic missing acknowledgement".to_string(),
                ))
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn held_smtp_finishes_and_persists_before_removal_can_purge() {
        for unknown in [false, true] {
            let suffix = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "wind-account-work-{}-{suffix}.db",
                std::process::id()
            ));
            let registry = Registry::default();
            let mut store = Store::open(&path).unwrap();
            let account = store
                .adopt_or_create_account("lifetime@example.invalid", "gmail")
                .unwrap();
            let draft = mail_core::compose(
                "lifetime@example.invalid",
                "recipient@example.invalid",
                "",
                "",
                "lifetime fixture",
                "body",
                None,
            )
            .unwrap();
            let first = store.enqueue_outbox(account, &draft).unwrap();
            store.enqueue_outbox(account, &draft).unwrap();
            let ticket = registry.capture(account).unwrap();
            let worker_ticket = ticket.clone();
            let worker_path = path.clone();
            let (entered_tx, entered_rx) = mpsc::channel();
            let (release_tx, release_rx) = mpsc::channel();
            let worker = std::thread::spawn(move || {
                let _lease = worker_ticket.lease().unwrap();
                let mut store = Store::open(&worker_path).unwrap();
                let mut smtp = HeldTransport {
                    entered: entered_tx,
                    release: release_rx,
                    unknown,
                };
                mail_core::flush_outbox_while(&mut smtp, &mut store, account, &mut || {
                    worker_ticket.check().is_ok()
                })
                .unwrap()
            });
            entered_rx.recv_timeout(Duration::from_secs(3)).unwrap();
            let removal = registry.retire(account).unwrap();
            assert!(removal.wait(Duration::from_millis(20)).is_err());
            assert!(ticket.lease().is_err());
            assert!(matches!(
                store.check_account_removal(account),
                Err(mail_core::Error::UnresolvedDelivery)
            ));
            release_tx.send(()).unwrap();
            removal.wait(Duration::from_secs(3)).unwrap();
            let report = worker.join().unwrap();
            let state = if unknown {
                OutboxState::Interrupted
            } else {
                OutboxState::Sent
            };
            assert_eq!(store.outbox_in_state(state).unwrap()[0].id, first);
            assert_eq!(report.sent + report.quarantined, 1);
            assert_eq!(store.outbox_to_send(account).unwrap()[0].attempts, 0);
            if unknown {
                assert!(store.delete_account(account).is_err());
                drop(removal);
            } else {
                store.delete_account(account).unwrap();
                removal.commit();
            }
            drop(store);
            for suffix in ["", "-wal", "-shm"] {
                let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
            }
        }
    }
    #[test]
    fn unknown_oauth_identity_cannot_write_credentials_after_removal() {
        let registry = Registry::default();
        let flow = registry.registration().unwrap();
        let removal = registry.retire(1).unwrap();
        assert!(registry.registration().is_err());
        assert!(removal.wait(Duration::from_millis(10)).is_err());
        drop(flow);
        removal.wait(Duration::from_secs(1)).unwrap();
        removal.commit();
        assert!(registry.registration().is_ok());
    }
    #[test]
    fn removal_drains_held_work_and_rejects_waiting_jobs_without_blocking_another_account() {
        let registry = Registry::default();
        let captured = registry.capture(1).unwrap();
        let active = captured.lease().unwrap();
        let removal = registry.retire(1).unwrap();
        assert!(captured.lease().is_err());
        assert!(removal.wait(Duration::from_millis(10)).is_err());
        let _neighbor = registry.capture(2).unwrap().lease().unwrap();
        drop(active);
        removal.wait(Duration::from_secs(1)).unwrap();
        removal.commit();
        let replacement = registry.capture(1).unwrap();
        assert!(replacement.lease().is_ok());
        assert!(captured.lease().is_err());
        assert!(!registry.is_current(&captured));
    }

    #[test]
    fn aborted_removal_never_revives_old_jobs_or_late_refreshes() {
        let registry = Registry::default();
        let old = registry.capture(1).unwrap();
        let held = old.lease().unwrap();
        let removal = registry.retire(1).unwrap();
        assert!(registry.retire(1).is_err());
        drop(removal);
        assert!(registry.capture(1).unwrap().lease().is_ok());
        assert!(old.lease().is_err());
        assert!(!registry.is_current(&old));
        let retry = registry.retire(1).unwrap();
        assert!(retry.wait(Duration::from_millis(10)).is_err());
        drop(held);
        retry.wait(Duration::from_secs(1)).unwrap();
    }
}

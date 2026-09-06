use crate::{ActionIncident, MailServer, PendingAction, RemovalMethod, RemovalPlan, RemovalStep};
use crate::{Error, Store};
use rusqlite::{OptionalExtension, params};

struct Effect {
    id: i64,
    source_generation: u32,
    plan: RemovalPlan,
    phase: String,
}

fn method_key(method: RemovalMethod) -> &'static str {
    match method {
        RemovalMethod::Atomic => "atomic",
        RemovalMethod::Move => "move",
        RemovalMethod::CopyThenRemove => "copy",
        RemovalMethod::Remove => "remove",
    }
}
fn method_value(value: &str) -> Result<RemovalMethod, Error> {
    match value {
        "atomic" => Ok(RemovalMethod::Atomic),
        "move" => Ok(RemovalMethod::Move),
        "copy" => Ok(RemovalMethod::CopyThenRemove),
        "remove" => Ok(RemovalMethod::Remove),
        _ => Err(Error::Corrupt("invalid removal method".to_string())),
    }
}
impl Store {
    /// Run once at process startup, before any new transfer can start.
    pub fn recover_action_effects(&self) -> Result<(), Error> {
        let tx = self.conn().unchecked_transaction()?;
        tx.execute("UPDATE action_effects SET phase = 'uncertain', reason = 'process stopped before transfer confirmation was persisted' WHERE phase = 'in_flight'", [])?;
        tx.execute("UPDATE pending_actions SET refusee = 1, last_error = 'transfer confirmation unavailable' WHERE id IN (SELECT action_id FROM action_effects WHERE phase = 'uncertain') AND refusee = 0", [])?;
        tx.commit()?;
        Ok(())
    }
    pub fn refused_action_reason(&self) -> Result<Option<String>, Error> {
        Ok(self.conn().query_row("SELECT last_error FROM pending_actions p WHERE refusee = 1
            AND NOT EXISTS (SELECT 1 FROM action_effects e WHERE e.action_id = p.id AND e.phase = 'uncertain')
            ORDER BY id LIMIT 1", [], |row| row.get(0)).optional()?.flatten())
    }

    pub fn action_incidents(&self) -> Result<Vec<ActionIncident>, Error> {
        let mut stmt = self.conn().prepare("SELECT id, account_id, source, destination, uid,
            COALESCE(reason, 'confirmation unavailable'), message_subject, message_sender FROM action_effects WHERE phase = 'uncertain' ORDER BY id")?;
        Ok(stmt
            .query_map([], |row| {
                Ok(ActionIncident {
                    id: row.get(0)?,
                    account_id: row.get(1)?,
                    source: row.get(2)?,
                    destination: row.get(3)?,
                    uid: row.get(4)?,
                    reason: row.get(5)?,
                    subject: row.get(6)?,
                    sender: row.get(7)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?)
    }

    /// Acknowledgement only: never requeues an uncertain operation or targets its old UID.
    pub fn dismiss_action_incident(&self, id: i64) -> Result<(), Error> {
        let tx = self.conn().unchecked_transaction()?;
        let action: Option<i64> = tx
            .query_row(
                "SELECT action_id FROM action_effects WHERE id = ?1 AND phase = 'uncertain'",
                [id],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
        tx.execute(
            "DELETE FROM action_effects WHERE id = ?1 AND phase = 'uncertain'",
            [id],
        )?;
        if let Some(action) = action {
            tx.execute("DELETE FROM pending_actions WHERE id = ?1", [action])?;
        }
        tx.commit()?;
        Ok(())
    }

    fn effect(&self, action: i64) -> Result<Option<Effect>, Error> {
        let raw = self
            .conn()
            .query_row(
                "SELECT id, source_generation, destination, destination_generation, method, phase
            FROM action_effects WHERE action_id = ?1",
                [action],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, u32>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<u32>>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                    ))
                },
            )
            .optional()?;
        raw.map(
            |(id, source_generation, destination, destination_generation, method, phase)| {
                Ok(Effect {
                    id,
                    source_generation,
                    plan: RemovalPlan {
                        method: method_value(&method)?,
                        destination,
                        destination_generation,
                    },
                    phase,
                })
            },
        )
        .transpose()
    }
    fn begin_effect(
        &self,
        mailbox_id: i64,
        pending: &PendingAction,
        plan: RemovalPlan,
    ) -> Result<Effect, Error> {
        self.conn().execute("INSERT INTO action_effects (action_id, account_id, source, source_generation, uid,
            destination, destination_generation, method, phase, message_subject, message_sender)
            SELECT ?1, m.account_id, m.name, m.uid_validity, ?2, ?3, ?4, ?5, 'prepared',
                COALESCE(p.message_subject, e.subject), COALESCE(p.message_sender, e.sender_address, e.sender)
            FROM mailboxes m JOIN pending_actions p ON p.id = ?1
            LEFT JOIN envelopes e ON e.mailbox_id = m.id AND e.uid = ?2 WHERE m.id = ?6",
            params![pending.id, pending.uid, plan.destination, plan.destination_generation, method_key(plan.method), mailbox_id])?;
        self.effect(pending.id)?.ok_or(Error::StaleMailbox)
    }
    fn effect_phase(&self, id: i64, phase: &str) -> Result<(), Error> {
        self.conn().execute(
            "UPDATE action_effects SET phase = ?2 WHERE id = ?1",
            params![id, phase],
        )?;
        Ok(())
    }
    fn uncertain_effect(&self, id: i64, reason: &str) -> Result<(), Error> {
        let tx = self.conn().unchecked_transaction()?;
        tx.execute(
            "UPDATE action_effects SET phase = 'uncertain', reason = ?2 WHERE id = ?1",
            params![id, reason],
        )?;
        tx.execute("UPDATE pending_actions SET refusee = 1, last_error = ?2 WHERE id = (SELECT action_id FROM action_effects WHERE id = ?1)", params![id, reason])?;
        tx.commit()?;
        Ok(())
    }
}

pub(crate) fn replay_removal(
    server: &mut dyn MailServer,
    store: &Store,
    mailbox: &str,
    mailbox_id: i64,
    pending: &PendingAction,
) -> Result<(), Error> {
    let mut effect = match store.effect(pending.id)? {
        Some(effect) => effect,
        None => store.begin_effect(
            mailbox_id,
            pending,
            server.plan_removal(mailbox, &pending.action)?,
        )?,
    };
    if matches!(effect.phase.as_str(), "in_flight" | "uncertain") {
        store.uncertain_effect(
            effect.id,
            "transfer confirmation unavailable; inspect source and destination before a new action",
        )?;
        return Err(Error::Refusal(
            "transfer result needs verification".to_string(),
        ));
    }
    let snapshot = server.select(mailbox)?;
    if snapshot.uid_validity != effect.source_generation {
        store.uncertain_effect(
            effect.id,
            "source identity changed; old UID will not be reused",
        )?;
        return Err(Error::Refusal("source identity changed".to_string()));
    }
    if let Some(destination) = &effect.plan.destination {
        let observed = server.folder_status(destination)?.uid_validity;
        if observed != effect.plan.destination_generation || observed.is_none() {
            store.uncertain_effect(
                effect.id,
                "destination identity changed or cannot be verified",
            )?;
            return Err(Error::Refusal("destination identity changed".to_string()));
        }
    }
    if effect.phase == "prepared" && effect.plan.method != RemovalMethod::Remove {
        store.effect_phase(effect.id, "in_flight")?;
        if let Err(err) = server.removal_step(
            mailbox,
            pending.uid,
            &pending.action,
            &effect.plan,
            RemovalStep::Transfer,
        ) {
            if matches!(err, Error::Refusal(_)) {
                store
                    .conn()
                    .execute("DELETE FROM action_effects WHERE id = ?1", [effect.id])?;
                return Err(err);
            }
            store.uncertain_effect(effect.id, &err.to_string())?;
            return Err(Error::Refusal(
                "transfer result needs verification".to_string(),
            ));
        }
        if effect.plan.method != RemovalMethod::CopyThenRemove {
            return Ok(());
        }
        store.effect_phase(effect.id, "copied")?;
        effect.phase = "copied".to_string();
    }
    if !matches!(effect.phase.as_str(), "prepared" | "copied" | "removing") {
        return Err(Error::Corrupt("invalid removal phase".to_string()));
    }
    // Only source deletion is repeatable; the source generation was checked above.
    store.effect_phase(effect.id, "removing")?;
    server.removal_step(
        mailbox,
        pending.uid,
        &pending.action,
        &effect.plan,
        RemovalStep::RemoveSource,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Action;
    fn queued(store: &Store, mailbox: i64) -> PendingAction {
        store
            .enqueue_action(mailbox, 1, Action::MoveTo("Archive".to_string()))
            .unwrap();
        store.pending_actions(mailbox).unwrap().pop().unwrap()
    }
    fn plan() -> RemovalPlan {
        RemovalPlan {
            method: RemovalMethod::CopyThenRemove,
            destination: Some("Archive".to_string()),
            destination_generation: Some(7),
        }
    }
    #[test]
    fn startup_recovery_preserves_confirmed_phases_and_quarantines_only_inflight() {
        let store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("journal@example.invalid", "generic")
            .unwrap();
        let mailbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        for phase in ["prepared", "in_flight", "copied", "removing"] {
            let pending = queued(&store, mailbox);
            let effect = store.begin_effect(mailbox, &pending, plan()).unwrap();
            store.effect_phase(effect.id, phase).unwrap();
            store.recover_action_effects().unwrap();
            assert_eq!(
                store.effect(pending.id).unwrap().unwrap().phase,
                if phase == "in_flight" {
                    "uncertain"
                } else {
                    phase
                }
            );
            assert_eq!(
                store.action_incidents().unwrap().len(),
                usize::from(phase == "in_flight")
            );
            store.remove_action(pending.id).unwrap();
        }
    }

    #[test]
    fn incident_context_survives_local_disappearance_and_generation_reset() {
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("journal@example.invalid", "generic")
            .unwrap();
        let mailbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        let envelope = crate::test_support::FakeServer::simple_envelope(1, "Contract fixture");
        store.upsert_envelopes(mailbox, &[envelope]).unwrap();
        let pending = queued(&store, mailbox);
        store.remove_local(mailbox, 1).unwrap();
        let effect = store.begin_effect(mailbox, &pending, plan()).unwrap();
        store.effect_phase(effect.id, "in_flight").unwrap();
        store.reset_mailbox(mailbox, 2).unwrap();
        let incidents = store.action_incidents().unwrap();
        assert_eq!(incidents.len(), 1);
        assert_eq!(incidents[0].account_id, account);
        assert_eq!(incidents[0].subject.as_deref(), Some("Contract fixture"));
        assert_eq!(incidents[0].sender.as_deref(), Some("alice@example.com"));
    }

    #[test]
    fn an_orphan_incident_survives_reset_and_cannot_attach_to_a_reused_action_id() {
        let store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("journal@example.invalid", "generic")
            .unwrap();
        let mailbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        let old = queued(&store, mailbox);
        let effect = store.begin_effect(mailbox, &old, plan()).unwrap();
        store.effect_phase(effect.id, "in_flight").unwrap();
        store.reset_mailbox(mailbox, 2).unwrap();
        let new = queued(&store, mailbox);
        assert_eq!(old.id, new.id, "exercise SQLite rowid reuse");
        assert!(store.effect(new.id).unwrap().is_none());
        assert_eq!(store.action_incidents().unwrap().len(), 1);
        store.dismiss_action_incident(effect.id).unwrap();
        assert_eq!(store.pending_actions(mailbox).unwrap(), vec![new]);
    }
    #[test]
    fn persisted_inflight_and_copied_phases_survive_a_new_connection() {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "wind-mutation-journal-{}-{suffix}.db",
            std::process::id()
        ));
        for phase in ["in_flight", "copied"] {
            let store = Store::open(&path).unwrap();
            let account = store
                .adopt_or_create_account("journal@example.invalid", "generic")
                .unwrap();
            let mailbox = store
                .sync_state(account, "INBOX")
                .unwrap()
                .map(|state| state.mailbox_id)
                .unwrap_or_else(|| store.create_mailbox(account, "INBOX", 1).unwrap());
            let pending = queued(&store, mailbox);
            let effect = store.begin_effect(mailbox, &pending, plan()).unwrap();
            store.effect_phase(effect.id, phase).unwrap();
            drop(store);
            let store = Store::open(&path).unwrap();
            let restored = store.effect(pending.id).unwrap().unwrap();
            assert_eq!(restored.phase, phase);
            assert_eq!(restored.plan, plan());
            assert_eq!(restored.source_generation, 1);
            if phase == "in_flight" {
                let mut server = crate::test_support::FakeServer::new(false);
                assert!(replay_removal(&mut server, &store, "INBOX", mailbox, &pending).is_err());
                assert!(server.action_calls.is_empty());
                assert!(server.moved.is_empty());
                assert_eq!(store.action_incidents().unwrap().len(), 1);
            }
            store.remove_action(pending.id).unwrap();
        }
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
        }
    }
}

//! The directory of contacts (PLAN-RETOURS-5, decision D4): the
//! addresses that mail has shown — senders seen (with their display
//! name), recipients of OUR sends — in service of address
//! autocompletion in the To/Cc/Bcc fields. Never an edited address book.
//!
//! Three rules:
//! - **a SMALL table, queried on keystroke**: one contact =
//!   one row — never a scan of `envelopes` per keystroke in the
//!   serialized queue (the lesson of PLAN-DEFILEMENT-PROFOND);
//! - **nothing from junk or trash** (D4): a spammer never
//!   becomes a suggestion;
//! - **the most recent name wins**, an address appears only once
//!   (deduplication by lowercased address).
//!
//! The directory feeds itself as mail flows in (sync: NEW messages
//! only — a re-sync does not inflate the frequency; sending: the
//! address written is a known address) and catches up ONCE on the
//! existing data at startup (marked in `prefs`, set-based pass).

use rusqlite::{Connection, OptionalExtension, params};

use crate::error::Error;
use crate::store::Store;

/// An address suggestion: the (lowercase) address and the last known
/// display name, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contact {
    pub address: String,
    pub name: Option<String>,
}

const MONTH: i64 = 30 * 24 * 3600;
const YEAR: i64 = 365 * 24 * 3600;

/// The ranking (a pure, testable decision): recency AND frequency — a
/// recent contact weighs more at equal frequency, a frequent
/// contact weighs more at equal recency. Steps rather than a
/// continuous decay: derivable off the top of one's head, stable under
/// test.
pub(crate) fn score(hits: i64, last_epoch: i64, now: i64) -> i64 {
    let age = now.saturating_sub(last_epoch);
    let weight = if age <= MONTH {
        4
    } else if age <= YEAR {
        2
    } else {
        1
    };
    hits.max(1) * weight
}

/// Escapes `%`, `_` and `\` in a user prefix for a LIKE pattern
/// (`ESCAPE '\'` clause).
fn escape_like(raw: &str) -> String {
    let mut output = String::with_capacity(raw.len());
    for c in raw.chars() {
        if matches!(c, '%' | '_' | '\\') {
            output.push('\\');
        }
        output.push(c);
    }
    output
}

/// Notes an address to the directory: creation or update (frequency
/// +1, recency bumped forward, the most recent name wins — an empty
/// name never replaces a known name).
pub(crate) fn note(
    conn: &Connection,
    account_id: i64,
    address: &str,
    name: Option<&str>,
    epoch: i64,
) -> Result<(), Error> {
    let address = address.trim().to_lowercase();
    if address.is_empty() {
        return Ok(());
    }
    let name = name.map(str::trim).filter(|name| !name.is_empty());
    conn.prepare_cached(
        "INSERT INTO contact_origins (account_id, address, name, name_epoch, last_epoch, hits)
         VALUES (?1, ?2, ?3, CASE WHEN ?3 IS NULL THEN NULL ELSE ?4 END, ?4, 1)
         ON CONFLICT(address, account_id) DO UPDATE SET
             name = CASE WHEN excluded.name IS NOT NULL AND (name IS NULL OR excluded.name_epoch >= name_epoch)
                         THEN excluded.name ELSE name END,
             name_epoch = CASE WHEN excluded.name IS NOT NULL AND (name_epoch IS NULL OR excluded.name_epoch >= name_epoch)
                               THEN excluded.name_epoch ELSE name_epoch END,
             last_epoch = MAX(last_epoch, excluded.last_epoch), hits = hits + 1",
    )?.execute(params![account_id, address, name, epoch])?;
    Ok(())
}

/// Unknown historical ownership is retained as source 0; new observations use their account.
pub(crate) fn migrate_provenance(conn: &Connection) -> Result<(), Error> {
    if conn
        .prepare("SELECT 1 FROM sqlite_master WHERE type='table' AND name='contact_origins'")?
        .exists([])?
    {
        return Ok(());
    }
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch("CREATE TABLE contact_origins (
        address TEXT NOT NULL, account_id INTEGER NOT NULL, name TEXT, name_epoch INTEGER,
        last_epoch INTEGER NOT NULL, hits INTEGER NOT NULL,
        PRIMARY KEY (address, account_id));
        CREATE INDEX idx_contact_origins_account ON contact_origins(account_id, address);
        INSERT INTO contact_origins (address, account_id, name, name_epoch, last_epoch, hits)
        SELECT address, 0, name, CASE WHEN name IS NOT NULL THEN last_epoch END, last_epoch, hits FROM correspondants;")?;
    for (event, source) in [("INSERT", "NEW"), ("UPDATE", "NEW"), ("DELETE", "OLD")] {
        tx.execute_batch(&format!("CREATE TRIGGER contact_origin_{event} AFTER {event} ON contact_origins BEGIN
            INSERT INTO correspondants (address, name, last_epoch, hits)
            SELECT address,
                (SELECT name FROM contact_origins names WHERE names.address = {source}.address AND name IS NOT NULL
                    ORDER BY name_epoch DESC, account_id ASC LIMIT 1),
                MAX(last_epoch), SUM(hits)
            FROM contact_origins WHERE address = {source}.address GROUP BY address
            ON CONFLICT(address) DO UPDATE SET name = excluded.name, last_epoch = excluded.last_epoch, hits = excluded.hits;
            DELETE FROM correspondants WHERE address = {source}.address
                AND NOT EXISTS (SELECT 1 FROM contact_origins WHERE address = {source}.address);
        END;"))?;
    }
    tx.commit()?;
    Ok(())
}

impl Store {
    /// What the directory learns from a mailbox: `(senders,
    /// recipients)`. Junk and trash learn NOTHING (D4); the sent
    /// folder ALSO learns the recipients.
    pub(crate) fn directory_role(&self, mailbox_id: i64) -> Result<(bool, bool), Error> {
        let (account_id, name): (i64, String) = self.conn().query_row(
            "SELECT account_id, name FROM mailboxes WHERE id = ?1",
            [mailbox_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let folders = self.canonical_folders(account_id)?;
        if folders.mailbox("indesirables").as_deref() == Some(name.as_str())
            || folders.mailbox("corbeille").as_deref() == Some(name.as_str())
        {
            return Ok((false, false));
        }
        let sent = folders.mailbox("envoyes").as_deref() == Some(name.as_str());
        Ok((true, sent))
    }

    /// The suggestions for a prefix: matching on the START of
    /// the address, the name, or a word of the name (LIKE — ASCII
    /// case-insensitive, an accepted limit on accented initials);
    /// ranked by recency + frequency ([`score`]), tie-broken by
    /// recency. The `LIMIT 512` net bounds the Rust sort — beyond
    /// that, the prefix is too short for a perfect rank to matter.
    pub fn complete_addresses(&self, prefix: &str, limit: usize) -> Result<Vec<Contact>, Error> {
        let prefix = prefix.trim().to_lowercase();
        if prefix.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        let pattern = format!("{}%", escape_like(&prefix));
        let word_pattern = format!("% {pattern}");
        let mut rows: Vec<(String, Option<String>, i64, i64)> = self
            .conn()
            .prepare_cached(
                "SELECT address, name, hits, last_epoch FROM correspondants
                 WHERE address LIKE ?1 ESCAPE '\\'
                    OR name LIKE ?1 ESCAPE '\\'
                    OR name LIKE ?2 ESCAPE '\\'
                 ORDER BY last_epoch DESC
                 LIMIT 512",
            )?
            .query_map(params![pattern, word_pattern], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .collect::<Result<_, _>>()?;
        let now = chrono::Utc::now().timestamp();
        rows.sort_by_key(|(_, _, hits, last)| (-score(*hits, *last, now), -*last));
        Ok(rows
            .into_iter()
            .take(limit)
            .map(|(address, name, _, _)| Contact { address, name })
            .collect())
    }

    /// The known names for a batch of addresses (thread header,
    /// PLAN-RETOURS-12 R5): a primary-key lookup, bounded to the
    /// recipients of the displayed message page — NEVER a scan of
    /// envelopes (lesson A64). Keys returned lowercase (the
    /// directory's form); an unknown address or one without a name is
    /// absent from the result — the UI falls back to the bare address.
    pub fn address_names(
        &self,
        addresses: &[String],
    ) -> Result<std::collections::HashMap<String, String>, Error> {
        let mut names = std::collections::HashMap::new();
        // `name IS NOT NULL` in the SQL: an absent row and an absent
        // name have the same outcome (out of the result) — a single
        // level of Option.
        let mut stmt = self.conn().prepare_cached(
            "SELECT name FROM correspondants WHERE address = ?1 AND name IS NOT NULL",
        )?;
        for address in addresses {
            let key = address.trim().to_lowercase();
            if key.is_empty() || names.contains_key(&key) {
                continue;
            }
            let name: Option<String> = stmt.query_row(params![key], |row| row.get(0)).optional()?;
            if let Some(name) = name {
                names.insert(key, name);
            }
        }
        Ok(names)
    }

    /// The ONE-TIME backfill of existing data: populates the directory
    /// from envelopes already in the database (set-based, the name of
    /// the most recent message wins — bare column MAX, a documented
    /// SQLite behavior), then sets the `prefs` marker. Idempotent: the
    /// marker is re-checked under the write lock — two concurrent
    /// connections never double-count.
    pub(crate) fn backfill_contacts(&self) -> Result<(), Error> {
        const BRAND: &str = "annuaire_correspondants_v1";
        let done: Option<String> = self
            .conn()
            .query_row(
                "SELECT value FROM prefs WHERE key = ?1",
                params![BRAND],
                |row| row.get(0),
            )
            .optional()?;
        if done.is_some() {
            return Ok(());
        }
        self.conn().execute_batch("BEGIN IMMEDIATE")?;
        let pass = (|| -> Result<(), Error> {
            let already_done: Option<String> = self
                .conn()
                .query_row(
                    "SELECT value FROM prefs WHERE key = ?1",
                    params![BRAND],
                    |row| row.get(0),
                )
                .optional()?;
            if already_done.is_some() {
                return Ok(());
            }
            let accounts: Vec<i64> = self
                .conn()
                .prepare("SELECT id FROM accounts")?
                .query_map([], |row| row.get(0))?
                .collect::<Result<_, _>>()?;
            for account in accounts {
                let folders = self.canonical_folders(account)?;
                let mut excluded: Vec<i64> = Vec::new();
                for category in ["indesirables", "corbeille"] {
                    if let Some(name) = folders.mailbox(category)
                        && let Some(state) = self.sync_state(account, &name)?
                    {
                        excluded.push(state.mailbox_id);
                    }
                }
                let clause = if excluded.is_empty() {
                    String::new()
                } else {
                    let list = excluded
                        .iter()
                        .map(i64::to_string)
                        .collect::<Vec<_>>()
                        .join(",");
                    format!(" AND e.mailbox_id NOT IN ({list})")
                };
                self.conn().execute(
                    &format!(
                        "INSERT INTO contact_origins (account_id, address, name, name_epoch, last_epoch, hits)
                         SELECT ?1, lower(e.sender_address), e.sender,
                                MAX(COALESCE(e.date_epoch, 0)), MAX(COALESCE(e.date_epoch, 0)), COUNT(*)
                         FROM envelopes e
                         JOIN mailboxes m ON m.id = e.mailbox_id
                         WHERE m.account_id = ?1
                           AND e.sender_address IS NOT NULL
                           AND e.sender_address <> ''{clause}
                         GROUP BY lower(e.sender_address)
                         ON CONFLICT(address, account_id) DO UPDATE SET
                             name = CASE WHEN excluded.last_epoch >= last_epoch
                                          AND excluded.name IS NOT NULL
                                         THEN excluded.name
                                         ELSE COALESCE(name, excluded.name) END,
                             name_epoch = CASE WHEN excluded.last_epoch >= last_epoch AND excluded.name IS NOT NULL
                                               THEN excluded.name_epoch ELSE COALESCE(name_epoch, excluded.name_epoch) END,
                             last_epoch = MAX(last_epoch, excluded.last_epoch),
                             hits = hits + excluded.hits"
                    ),
                    params![account],
                )?;
                // The recipients of OUR sends: the lists are joined
                // by '\n' — split in Rust, the sent folder is small
                // next to the corpus.
                if let Some(name) = folders.mailbox("envoyes")
                    && let Some(state) = self.sync_state(account, &name)?
                {
                    let sends: Vec<(Option<String>, Option<String>, Option<i64>)> = self
                        .conn()
                        .prepare(
                            "SELECT to_addrs, cc_addrs, date_epoch
                             FROM envelopes WHERE mailbox_id = ?1",
                        )?
                        .query_map([state.mailbox_id], |row| {
                            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                        })?
                        .collect::<Result<_, _>>()?;
                    for (to, cc, date) in sends {
                        for list in [to, cc].into_iter().flatten() {
                            for address in list.split('\n').filter(|a| !a.is_empty()) {
                                note(self.conn(), account, address, None, date.unwrap_or(0))?;
                            }
                        }
                    }
                }
            }
            self.conn().execute(
                "INSERT INTO prefs (key, value) VALUES (?1, '1')
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![BRAND],
            )?;
            Ok(())
        })();
        match pass {
            Ok(()) => {
                self.conn().execute_batch("COMMIT")?;
                Ok(())
            }
            Err(err) => {
                let _ = self.conn().execute_batch("ROLLBACK");
                Err(err)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;
    use crate::envelope::Envelope;

    fn envelope(uid: u32, subject: &str, name: &str, address: &str, epoch: i64) -> Envelope {
        Envelope {
            reply_to: None,
            uid,
            subject: Some(subject.to_string()),
            sender: Some(name.to_string()),
            sender_address: Some(address.to_string()),
            message_id: Some(format!("<m{uid}@exemple.fr>")),
            in_reply_to: None,
            date: Some(Utc.timestamp_opt(epoch, 0).unwrap()),
            seen: true,
            flagged: false,
            to_addrs: Vec::new(),
            cc_addrs: Vec::new(),
        }
    }

    fn fixture() -> (Store, i64, i64, i64, i64) {
        let store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("moi@exemple.fr", "gmail")
            .unwrap();
        store.set_thread_scope(account, Some("Envoyés")).unwrap(); // lang:fr
        store
            .replace_folders(
                account,
                &["INBOX", "Envoyés", "Spam", "Corbeille"] // lang:fr
                    .iter()
                    .map(|name| crate::Folder {
                        wire: name.to_string(),
                        display: name.to_string(),
                        selectable: true,
                        special_use: None,
                        delimiter: None,
                    })
                    .collect::<Vec<_>>(),
            )
            .unwrap();
        let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        let sent = store.create_mailbox(account, "Envoyés", 1).unwrap(); // lang:fr
        let spam = store.create_mailbox(account, "Spam", 1).unwrap();
        (store, account, inbox, sent, spam)
    }

    #[test]
    fn account_removal_forgets_exclusive_contacts_and_recomputes_shared_names() {
        let (mut store, first, inbox, _, _) = fixture();
        let second = store
            .adopt_or_create_account("second@example.fr", "gmail")
            .unwrap();
        let other = store.create_mailbox(second, "INBOX", 1).unwrap();
        store
            .upsert_envelopes(
                other,
                &[
                    envelope(1, "Shared", "Shared name", "shared@example.fr", 100),
                    envelope(2, "Other", "Other only", "other@example.fr", 100),
                ],
            )
            .unwrap();
        store
            .upsert_envelopes(
                inbox,
                &[
                    envelope(1, "Private", "Private name", "private@example.fr", 200),
                    envelope(
                        2,
                        "Shared",
                        "Removed account name",
                        "shared@example.fr",
                        200,
                    ),
                ],
            )
            .unwrap();
        store.allow_images_sender_of(inbox, 2, 200).unwrap();
        store.delete_account(first).unwrap();
        assert!(store.complete_addresses("private@", 10).unwrap().is_empty());
        assert_eq!(
            store.complete_addresses("shared@", 10).unwrap()[0]
                .name
                .as_deref(),
            Some("Shared name")
        );
        assert_eq!(store.complete_addresses("other@", 10).unwrap().len(), 1);
        assert_eq!(
            store.images_senders().unwrap(),
            vec!["shared@example.fr"],
            "explicit shared preference survives"
        );
        store.delete_account(second).unwrap();
        assert!(store.complete_addresses("shared@", 10).unwrap().is_empty());
    }

    #[test]
    fn contact_removal_rolls_back_with_failed_account_removal() {
        let (mut store, account, inbox, _, _) = fixture();
        store
            .upsert_envelopes(
                inbox,
                &[envelope(
                    1,
                    "Private",
                    "Private name",
                    "private@example.fr",
                    200,
                )],
            )
            .unwrap();
        store.conn().execute_batch("CREATE TRIGGER fail_account_remove BEFORE DELETE ON accounts BEGIN SELECT RAISE(ABORT, 'injected account removal failure'); END;").unwrap();
        assert!(store.delete_account(account).is_err());
        assert_eq!(store.complete_addresses("private@", 10).unwrap().len(), 1);
        store
            .conn()
            .execute_batch("DROP TRIGGER fail_account_remove")
            .unwrap();
        store.delete_account(account).unwrap();
        assert!(store.complete_addresses("private@", 10).unwrap().is_empty());
    }

    #[test]
    fn legacy_contacts_keep_unknown_provenance_and_restore_their_original_name() {
        let (mut store, account, inbox, _, _) = fixture();
        store.conn().execute_batch("DROP TABLE IF EXISTS contact_origins;
            INSERT INTO correspondants (address,name,last_epoch,hits) VALUES ('legacy@example.fr','Legacy name',100,5);").unwrap();
        migrate_provenance(store.conn()).unwrap();
        store
            .upsert_envelopes(
                inbox,
                &[envelope(
                    1,
                    "Legacy",
                    "Removed account name",
                    "legacy@example.fr",
                    200,
                )],
            )
            .unwrap();
        store.delete_account(account).unwrap();
        let legacy = store.complete_addresses("legacy@", 10).unwrap();
        assert_eq!(legacy[0].name.as_deref(), Some("Legacy name"));
        let hits: i64 = store
            .conn()
            .query_row(
                "SELECT hits FROM correspondants WHERE address='legacy@example.fr'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(hits, 5);
        migrate_provenance(store.conn()).unwrap();
        let repeated: i64 = store
            .conn()
            .query_row(
                "SELECT hits FROM correspondants WHERE address='legacy@example.fr'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(repeated, 5);
    }

    #[test]
    fn provenance_migration_failure_rolls_back_its_schema_and_can_retry() {
        let (store, _, _, _, _) = fixture();
        store.conn().execute_batch("DROP TABLE contact_origins;
            INSERT INTO correspondants (address,name,last_epoch,hits) VALUES ('legacy@example.fr','Legacy',100,5);
            CREATE INDEX idx_contact_origins_account ON correspondants(address);").unwrap();
        assert!(migrate_provenance(store.conn()).is_err());
        assert!(
            !store
                .conn()
                .prepare(
                    "SELECT 1 FROM sqlite_master WHERE type='table' AND name='contact_origins'"
                )
                .unwrap()
                .exists([])
                .unwrap()
        );
        assert_eq!(store.complete_addresses("legacy@", 10).unwrap().len(), 1);
        store
            .conn()
            .execute_batch("DROP INDEX idx_contact_origins_account")
            .unwrap();
        migrate_provenance(store.conn()).unwrap();
        let sources: i64 = store
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM contact_origins WHERE account_id=0",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(sources, 1);
    }

    #[test]
    fn unnamed_recent_mail_does_not_replace_a_more_recent_known_shared_name() {
        let store = Store::open_in_memory().unwrap();
        note(store.conn(), 1, "shared@example.fr", Some("Earlier"), 10).unwrap();
        note(store.conn(), 2, "shared@example.fr", Some("Current"), 20).unwrap();
        note(store.conn(), 1, "shared@example.fr", None, 30).unwrap();
        assert_eq!(
            store.complete_addresses("shared@", 10).unwrap()[0]
                .name
                .as_deref(),
            Some("Current")
        );
    }

    /// Recency + frequency: recent weighs more at equal frequency, the
    /// frequent weighs more at equal recency — and a recent contact
    /// beats a frequent one from years ago.
    #[test]
    fn the_score_weighs_recency_and_frequency() {
        let now = 10 * YEAR;
        // Equal frequency: the recent one wins.
        assert!(score(10, now - 3600, now) > score(10, now - 2 * YEAR, now));
        // Equal recency: the frequent one wins.
        assert!(score(20, now - 3600, now) > score(3, now - 3600, now));
        // 2 messages this month beat 7 from years ago.
        assert!(score(2, now - 3600, now) > score(7, now - 2 * YEAR, now));
    }

    /// Sync learns the senders — NEW messages only: a re-sync of
    /// the same batch does not inflate the frequency.
    #[test]
    fn sync_learns_senders_without_double_counting() {
        let (mut store, _account, inbox, _, _) = fixture();
        let batch = [envelope(1, "one", "Alice Martin", "Alice@Exemple.fr", 100)];
        store.upsert_envelopes(inbox, &batch).unwrap();
        store.upsert_envelopes(inbox, &batch).unwrap();

        let (hits, name): (i64, Option<String>) = store
            .conn()
            .query_row(
                "SELECT hits, name FROM correspondants WHERE address = 'alice@exemple.fr'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(hits, 1, "re-synced, not re-counted");
        assert_eq!(name.as_deref(), Some("Alice Martin"));
    }

    /// D4: junk and trash learn nothing.
    #[test]
    fn nothing_learned_from_junk() {
        let (mut store, _account, _, _, spam) = fixture();
        store
            .upsert_envelopes(
                spam,
                &[envelope(1, "ad", "Spammeur", "spam@nuisible.fr", 100)],
            )
            .unwrap();
        let total: i64 = store
            .conn()
            .query_row("SELECT COUNT(*) FROM correspondants", [], |row| row.get(0))
            .unwrap();
        assert_eq!(total, 0);
    }

    /// R5 (PLAN-RETOURS-12): the thread header resolves recipient
    /// addresses to names via the directory — a known name comes back
    /// (key lowercased regardless of the requested case), an address
    /// without a name or unknown is ABSENT from the result (the UI
    /// falls back to the bare address).
    #[test]
    fn address_names_returns_known_names_and_omits_the_rest() {
        let store = Store::open_in_memory().unwrap();
        note(
            store.conn(),
            0,
            "Camille@Exemple.fr",
            Some("Camille Rousseau"),
            10,
        )
        .unwrap();
        note(store.conn(), 0, "muette@exemple.fr", None, 10).unwrap();

        let names = store
            .address_names(&[
                "CAMILLE@exemple.fr".to_string(),
                "muette@exemple.fr".to_string(),
                "inconnue@exemple.fr".to_string(),
            ])
            .unwrap();
        assert_eq!(names.len(), 1);
        assert_eq!(
            names.get("camille@exemple.fr").map(String::as_str),
            Some("Camille Rousseau")
        );
    }

    /// D4: the sent folder ALSO learns the recipients (bare
    /// address — their name will come from a received message, if it
    /// ever does).
    #[test]
    fn sent_messages_learn_the_recipients() {
        let (mut store, _account, _, sent, _) = fixture();
        let mut env = envelope(1, "our send", "Moi", "moi@exemple.fr", 100);
        env.to_addrs = vec!["Camille.Rousseau@atelier.fr".to_string()];
        env.cc_addrs = vec!["s.nardi@atelier.fr".to_string()];
        store.upsert_envelopes(sent, &[env]).unwrap();

        let found = store.complete_addresses("camille", 8).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].address, "camille.rousseau@atelier.fr");
        assert!(store.complete_addresses("s.nardi", 8).unwrap().len() == 1);
    }

    /// Review: recipients BACKFILLED from an old send
    /// (`set_recipients`, PLAN-RETOURS-MAIL pump) also enter the
    /// directory — the startup backfill runs before them.
    #[test]
    fn backfilled_recipients_enter_the_directory() {
        let (mut store, _account, _, sent, _) = fixture();
        // A send from before To/Cc storage: no recipients.
        store
            .upsert_envelopes(
                sent,
                &[envelope(1, "old send", "Moi", "moi@exemple.fr", 100)],
            )
            .unwrap();
        assert!(store.complete_addresses("old", 8).unwrap().is_empty());

        store
            .set_recipients(sent, 1, &["old@dest.fr".to_string()], &[])
            .unwrap();

        assert_eq!(store.complete_addresses("old", 8).unwrap().len(), 1);
    }

    /// The address written is a known address: sending notes its
    /// recipients as soon as it is queued.
    #[test]
    fn sending_notes_its_recipients() {
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("moi@exemple.fr", "gmail")
            .unwrap();
        let draft = crate::compose(
            "moi@exemple.fr",
            "new@contact.fr",
            "",
            "cc@contact.fr",
            "subject",
            "body",
            None,
        )
        .unwrap();
        store.enqueue_outbox(account, &draft).unwrap();

        assert_eq!(store.complete_addresses("new", 8).unwrap().len(), 1);
        assert_eq!(store.complete_addresses("cc", 8).unwrap().len(), 1);
        store.delete_account(account).unwrap();
        assert!(store.complete_addresses("new", 8).unwrap().is_empty());
        assert!(store.complete_addresses("cc", 8).unwrap().is_empty());
    }

    /// Matching: start of address, start of name, start of a WORD of
    /// the name — and the prefix is case-insensitive.
    #[test]
    fn matching_covers_address_name_and_words_of_the_name() {
        let (mut store, _account, inbox, _, _) = fixture();
        store
            .upsert_envelopes(
                inbox,
                &[envelope(
                    1,
                    "x",
                    "Camille Rousseau",
                    "c.rousseau@atelier.fr",
                    100,
                )],
            )
            .unwrap();

        for prefix in ["c.rous", "camille", "rousseau", "ROUSSEAU"] {
            let found = store.complete_addresses(prefix, 8).unwrap();
            assert_eq!(found.len(), 1, "prefix {prefix:?}");
            assert_eq!(found[0].name.as_deref(), Some("Camille Rousseau"));
        }
        assert!(store.complete_addresses("ousseau", 8).unwrap().is_empty());
        // LIKE metacharacters are prefix LITERALS.
        assert!(store.complete_addresses("%", 8).unwrap().is_empty());
    }

    /// One address = one row, the most recent name wins, and the
    /// ranking serves the recent-and-frequent first.
    #[test]
    fn dedup_keeps_the_latest_name_and_ranks_correctly() {
        let (mut store, _account, inbox, _, _) = fixture();
        let now = Utc::now().timestamp();
        store
            .upsert_envelopes(
                inbox,
                &[
                    envelope(
                        1,
                        "old",
                        "C. Rousseau",
                        "c.rousseau@atelier.fr",
                        now - 3 * YEAR,
                    ),
                    envelope(
                        2,
                        "recent",
                        "Camille Rousseau",
                        "c.rousseau@atelier.fr",
                        now - 60,
                    ),
                    envelope(
                        3,
                        "other",
                        "Casimir Vieux",
                        "casimir@archives.fr",
                        now - 2 * YEAR,
                    ),
                ],
            )
            .unwrap();

        let found = store.complete_addresses("c", 8).unwrap();
        assert_eq!(found.len(), 2, "{found:?}");
        assert_eq!(found[0].address, "c.rousseau@atelier.fr");
        assert_eq!(
            found[0].name.as_deref(),
            Some("Camille Rousseau"),
            "the most recent name wins"
        );
        assert_eq!(found[1].address, "casimir@archives.fr");
        // The limit is respected.
        assert_eq!(store.complete_addresses("c", 1).unwrap().len(), 1);
    }

    /// The keystroke-budget bench (PLAN-RETOURS-5, gate E4): 50,000
    /// contacts — more than the unique senders on the real base of
    /// 256k messages —, a ONE-letter prefix (the worst case: the most
    /// matches). `cargo test --release -- --ignored
    /// bench_complete --nocapture`; budget < 50 ms.
    #[test]
    #[ignore]
    fn bench_complete_addresses_50k() {
        let store = Store::open_in_memory().unwrap();
        {
            let conn = store.conn();
            conn.execute_batch("BEGIN").unwrap();
            let mut stmt = conn
                .prepare(
                    "INSERT INTO correspondants (address, name, last_epoch, hits)
                     VALUES (?1, ?2, ?3, ?4)",
                )
                .unwrap();
            for n in 0..50_000i64 {
                stmt.execute(params![
                    format!("contact{n}@domaine{}.fr", n % 977),
                    format!("Contact Num{n}"),
                    n * 60,
                    (n % 40) + 1
                ])
                .unwrap();
            }
            conn.execute_batch("COMMIT").unwrap();
        }
        let start = std::time::Instant::now();
        let found = store.complete_addresses("c", 8).unwrap();
        let duration = start.elapsed();
        println!("complete_addresses('c') on 50,000: {duration:?}");
        assert_eq!(found.len(), 8);
    }

    /// The bench of the backfill pass (once, on first launch on
    /// an existing database): 200,000 envelopes, ~20,000 unique
    /// senders. `cargo test --release bench_backfill -- --ignored
    /// --nocapture`.
    #[test]
    #[ignore]
    fn bench_backfill_200k() {
        let (store, _account, inbox, _, _) = fixture();
        {
            let conn = store.conn();
            conn.execute_batch("BEGIN").unwrap();
            let mut stmt = conn
                .prepare(
                    "INSERT INTO envelopes (mailbox_id, uid, subject, sender,
                        sender_address, message_id, date_epoch, seen, flagged)
                     VALUES (?1, ?2, 'subject', ?3, ?4, ?5, ?6, 1, 0)",
                )
                .unwrap();
            for n in 0..200_000i64 {
                stmt.execute(params![
                    inbox,
                    n + 1,
                    format!("Contact Num{}", n % 20_000),
                    format!("contact{}@domaine.fr", n % 20_000),
                    format!("<bench-{n}@exemple.fr>"),
                    n * 60
                ])
                .unwrap();
            }
            conn.execute_batch(
                "COMMIT;
                 DELETE FROM contact_origins; DELETE FROM correspondants;
                 DELETE FROM prefs WHERE key = 'annuaire_correspondants_v1';",
            )
            .unwrap();
        }
        let start = std::time::Instant::now();
        store.backfill_contacts().unwrap();
        let duration = start.elapsed();
        let total: i64 = store
            .conn()
            .query_row("SELECT COUNT(*) FROM correspondants", [], |row| row.get(0))
            .unwrap();
        println!("backfill of 200,000 envelopes -> {total} contacts: {duration:?}");
        assert_eq!(total, 20_000);
    }

    /// The backfill: an existing database populates the directory
    /// ONCE — the marker holds, replaying doubles nothing; junk
    /// excluded.
    #[test]
    fn backfill_populates_once_without_junk() {
        let (mut store, account, inbox, sent, spam) = fixture();
        store
            .upsert_envelopes(inbox, &[envelope(1, "x", "Alice", "alice@exemple.fr", 100)])
            .unwrap();
        let mut outgoing = envelope(1, "our send", "Moi", "moi@exemple.fr", 200);
        outgoing.to_addrs = vec!["dest@exemple.fr".to_string()];
        store.upsert_envelopes(sent, &[outgoing]).unwrap();
        store
            .upsert_envelopes(
                spam,
                &[envelope(1, "ad", "Spammeur", "spam@nuisible.fr", 300)],
            )
            .unwrap();
        // The "existing data": the directory emptied, the marker
        // removed — the exact state of a database from before
        // PLAN-RETOURS-5.
        store
            .conn()
            .execute_batch(
                "DELETE FROM contact_origins; DELETE FROM correspondants;
                 DELETE FROM prefs WHERE key = 'annuaire_correspondants_v1';",
            )
            .unwrap();

        store.backfill_contacts().unwrap();
        store.backfill_contacts().unwrap();

        let hits: i64 = store
            .conn()
            .query_row(
                "SELECT hits FROM correspondants WHERE address = 'alice@exemple.fr'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(hits, 1, "the marker holds: never two passes");
        assert_eq!(store.complete_addresses("dest", 8).unwrap().len(), 1);
        assert!(store.complete_addresses("spam", 8).unwrap().is_empty());
        store.delete_account(account).unwrap();
        assert!(store.complete_addresses("dest", 8).unwrap().is_empty());
        assert!(store.complete_addresses("alice", 8).unwrap().is_empty());
    }
}

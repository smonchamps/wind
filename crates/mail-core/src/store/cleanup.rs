use super::*;

/// The CLOSED vocabularies of Spring cleaning (part B, D6) — checked on
/// the core side before any write, like routing.
pub const CLEANUP_RANGES: &[&str] = &["3m", "6m", "1a", "2a", "5a", "tout"];

pub const CLEANUP_SCOPES: &[&str] = &["reception", "dossiers", "dossiersArchives", "archives"];

/// The cleanup session in progress (at most one, persisted — D8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupSession {
    pub range: String,
    pub scope: String,
    pub bound_epoch: i64,
    pub total: u64,
    pub handled: u64,
}

/// A Cleanup group: one sender × its mail within the range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupGroup {
    pub address: String,
    pub who: Option<String>,
    pub messages: u64,
    pub last_epoch: i64,
    pub last_subject: Option<String>,
}

impl Store {
    /// The mailboxes a scope covers (D6, CE vocabulary) — resolved
    /// per account from the canonicals. Sent, Drafts, Junk and Trash
    /// are ALWAYS out of scope (we don't sort what is already handled
    /// or written by oneself). The FULL archive (“All messages”)
    /// would replay the entire mailbox: out of scope — a stated limit
    /// in the PLAN.
    fn mailboxes_in_scope(&self, scope: &str) -> Result<Vec<i64>, Error> {
        let folders_included = matches!(scope, "dossiers" | "dossiersArchives");
        let archives_included = matches!(scope, "archives" | "dossiersArchives");
        let mut ids = Vec::new();
        let mut stmt = self
            .0
            .prepare_cached("SELECT id, name FROM mailboxes WHERE account_id = ?1")?;
        for account in self.accounts()? {
            let canon = self.canonical_folders(account.id)?;
            let mailboxes = stmt
                .query_map([account.id], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            for (id, name) in mailboxes {
                let is = |canonical: &Option<String>| canonical.as_deref() == Some(name.as_str());
                let included = if name == canon.inbox {
                    true
                } else if is(&canon.archives) {
                    archives_included && !canon.archives_full
                } else if is(&canon.sent)
                    || is(&canon.drafts)
                    || is(&canon.junk)
                    || is(&canon.trash)
                {
                    false
                } else {
                    folders_included
                };
                if included {
                    ids.push(id);
                }
            }
        }
        Ok(ids)
    }

    /// The SQL list of mailbox ids in scope — ONE definition (review
    /// 2026-08-30: three copies were starting to diverge). The ids
    /// come from OUR own database — never from user input.
    fn id_list(ids: &[i64]) -> String {
        if ids.is_empty() {
            "NULL".to_string()
        } else {
            ids.iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(",")
        }
    }

    /// The shared criterion for groups: mail in the range within the
    /// scope, excluding senders already routed (D7), excluding
    /// oneself, excluding envelopes without an address. THE only
    /// definition of “the range within the scope” — groups, the
    /// verdict's stock, and a group's view all share it. A message
    /// WITHOUT a date counts in every range (precedent A98: “no date
    /// = today” — a stated limit in the PLAN: it also follows the
    /// stock's rules).
    fn cleanup_criterion(ids: &[i64]) -> String {
        let list = Self::id_list(ids);
        format!(
            "e.mailbox_id IN ({list})
               AND (e.date_epoch > ?1 OR e.date_epoch IS NULL)
               AND e.sender_norm IS NOT NULL
               AND e.sender_norm NOT IN (SELECT address FROM routage_expediteurs WHERE address IS NOT NULL)
               AND e.sender_norm NOT IN (SELECT lower(trim(email)) FROM accounts WHERE email IS NOT NULL)"
        )
    }

    /// The SQL for groups — ONE pass: with a SINGLE max(), SQLite
    /// guarantees the bare columns (sender, subject) come from the max
    /// row — the rank shows the subject of the last message IN SCOPE
    /// (review 2026-08-30: two unbounded correlated subqueries could
    /// show the subject of a message outside the session, and repaid
    /// the sender sort four times per group).
    ///
    /// In TWO phases over the sender index (PLAN-AUDIT-V2 E4): the
    /// aggregate is COVERED by `idx_envelopes_sender` (sender, date,
    /// mailbox — never a table row read), then the subject and name
    /// of the last message are looked up through the same index.
    /// Measured on 200 k envelopes and 5 000 senders: 380 ms → under
    /// 100 (the old pass walked the DATE index then a temporary
    /// B-tree). `INDEXED BY`: the planner preferred the date index —
    /// the plan test `cleanup_groups_are_read_via_the_senders_index`
    /// keeps the promise. The outer `GROUP BY` absorbs a date tie
    /// (two messages from a sender in the same second): one rank per
    /// group, never two.
    pub(super) fn cleanup_groups_sql(ids: &[i64]) -> String {
        let criterion = Self::cleanup_criterion(ids);
        let list = Self::id_list(ids);
        format!(
            "SELECT g.sender_norm, g.n, g.dernier, e.sender, e.subject
               FROM (SELECT e.sender_norm AS sender_norm, COUNT(*) AS n,
                            MAX(e.date_epoch) AS dernier
                       FROM envelopes e INDEXED BY {SENDERS_INDEX}
                      WHERE {criterion}
                      GROUP BY e.sender_norm) g
               CROSS JOIN envelopes e INDEXED BY {SENDERS_INDEX}
                 ON e.sender_norm = g.sender_norm
                AND e.date_epoch IS g.dernier
                AND e.mailbox_id IN ({list})
              GROUP BY g.sender_norm
              ORDER BY g.dernier DESC, g.sender_norm"
        )
    }

    /// A group's mail — THE shared criterion: the view shows exactly
    /// what the verdict will process. `INDEXED BY` (PLAN-AUDIT-V2
    /// E4): without it, the embedded SQLite preferred the date index
    /// and scanned the entire mailbox for 40 rows — 116 ms on 200k.
    pub(super) fn cleanup_messages_sql(ids: &[i64]) -> String {
        let criterion = Self::cleanup_criterion(ids);
        format!(
            "{SELECT_UNIFIED}, COALESCE(t.size, 1), COALESCE(t.unseen, 1 - e.seen)
             FROM envelopes e INDEXED BY {SENDERS_INDEX}
             JOIN mailboxes m ON m.id = e.mailbox_id
             JOIN accounts a ON a.id = m.account_id
             LEFT JOIN threads t ON t.id = e.thread_id
             LEFT JOIN bodies b ON b.mailbox_id = e.mailbox_id AND b.uid = e.uid
             WHERE e.sender_norm = ?2 AND {criterion}
             ORDER BY e.date_epoch DESC, e.uid DESC"
        )
    }

    fn cleanup_count_groups(&self, ids: &[i64], bound: i64) -> Result<u64, Error> {
        let criterion = Self::cleanup_criterion(ids);
        let total: i64 = self.0.query_row(
            &format!(
                "SELECT COUNT(*) FROM (SELECT 1 FROM envelopes e INDEXED BY {SENDERS_INDEX}
                  WHERE {criterion} GROUP BY e.sender_norm)"
            ),
            params![bound],
            |row| row.get(0),
        )?;
        Ok(total as u64)
    }

    /// The current session — `None`: no cleanup started.
    pub fn cleanup_state(&self) -> Result<Option<CleanupSession>, Error> {
        Ok(self
            .0
            .query_row(
                "SELECT plage, perimetre, borne_epoch, total, traites
                   FROM nettoyage_session WHERE id = 1",
                [],
                |row| {
                    Ok(CleanupSession {
                        range: row.get(0)?,
                        scope: row.get(1)?,
                        bound_epoch: row.get(2)?,
                        total: row.get::<_, i64>(3)? as u64,
                        handled: row.get::<_, i64>(4)? as u64,
                    })
                },
            )
            .optional()?)
    }

    /// Starts a cleanup (replaces the current session): the bound is
    /// FIXED here — a session doesn't drift with the clock — and the
    /// group total becomes the denominator of progress.
    pub fn cleanup_start(
        &self,
        range: &str,
        scope: &str,
        now: i64,
    ) -> Result<CleanupSession, Error> {
        if !CLEANUP_RANGES.contains(&range) {
            return Err(Error::Corrupt(format!("unknown range: {range:?}")));
        }
        if !CLEANUP_SCOPES.contains(&scope) {
            return Err(Error::Corrupt(format!("unknown scope: {scope:?}")));
        }
        let bound = crate::backfill::horizon_epoch(range, now);
        let ids = self.mailboxes_in_scope(scope)?;
        let total = self.cleanup_count_groups(&ids, bound)?;
        self.0.execute(
            "INSERT OR REPLACE INTO nettoyage_session
               (id, plage, perimetre, borne_epoch, total, traites)
             VALUES (1, ?1, ?2, ?3, ?4, 0)",
            params![range, scope, bound, total as i64],
        )?;
        Ok(CleanupSession {
            range: range.to_string(),
            scope: scope.to_string(),
            bound_epoch: bound,
            total,
            handled: 0,
        })
    }

    /// The session's remaining groups: a sender × their mail in the
    /// range, the most recent first. Empty without a session.
    pub fn cleanup_groups(&self) -> Result<Vec<CleanupGroup>, Error> {
        let Some(session) = self.cleanup_state()? else {
            return Ok(Vec::new());
        };
        let ids = self.mailboxes_in_scope(&session.scope)?;
        let mut stmt = self.0.prepare(&Self::cleanup_groups_sql(&ids))?;
        let groups = stmt
            .query_map(params![session.bound_epoch], |row| {
                Ok(CleanupGroup {
                    address: row.get(0)?,
                    messages: row.get::<_, i64>(1)? as u64,
                    last_epoch: row.get::<_, Option<i64>>(2)?.unwrap_or(0),
                    who: row.get(3)?,
                    last_subject: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(groups)
    }

    /// The GROUP verdict (D5: the stock AND the future) — the
    /// Screener's gate for the future (routing, exit from waiting,
    /// flags), plus applying the rule to the stock WITHIN THE RANGE:
    /// one action per message in `pending_actions`, WITHIN the
    /// verdict's transaction (E3 pattern — never a crash window
    /// between the mail and the intent), duplicate guard, `trash`
    /// → the server's trash, NEVER a permanent deletion (D4); `spam`
    /// without a resolved folder does NOTHING (never an invented
    /// destination). Returns the number of stock messages handled.
    pub fn cleanup_verdict(
        &mut self,
        address: &str,
        destination: &str,
        rule: Option<&str>,
        epoch: i64,
    ) -> Result<usize, Error> {
        let Some(session) = self.cleanup_state()? else {
            return Err(Error::Corrupt("no cleanup in progress".to_string()));
        };
        validate_routing(destination, rule)?;
        let Some(address) = images_address(Some(address.to_string())) else {
            return Err(Error::InvalidEmailAddress(address.to_string()));
        };
        let ids = self.mailboxes_in_scope(&session.scope)?;
        // Each account's junk folder, resolved BEFORE the transaction
        // (same rule as arrival E3).
        let mut junk: BTreeMap<i64, Option<String>> = BTreeMap::new();
        if destination == "ecarte" && rule == Some("spam") {
            for account in self.accounts()? {
                junk.insert(account.id, self.canonical_folders(account.id)?.junk);
            }
        }
        let mut removals: Vec<(i64, Uid)> = Vec::new();
        let tx = self.0.unchecked_transaction()?;
        if destination == "ecarte"
            && let Some(rule) = rule
        {
            // The stock: THE shared criterion (same definition as the
            // groups and the view), restricted to the address — read
            // BEFORE `set_verdict`, which would take the sender out of
            // the criterion (D7 excludes routed senders).
            let criterion = Self::cleanup_criterion(&ids);
            let stock: Vec<(i64, Uid, i64)> = {
                let mut stmt = tx.prepare(&format!(
                    "SELECT e.mailbox_id, e.uid, m.account_id
                       FROM envelopes e JOIN mailboxes m ON m.id = e.mailbox_id
                      WHERE e.sender_norm = ?2 AND {criterion}"
                ))?;
                stmt.query_map(params![session.bound_epoch, address], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                })?
                .collect::<Result<Vec<_>, _>>()?
            };
            for (mailbox_id, uid, account_id) in stock {
                let action = match rule {
                    "archive" => Some(Action::Archive),
                    "corbeille" => Some(Action::Delete),
                    "spam" => junk.get(&account_id).cloned().flatten().map(Action::MoveTo),
                    _ => None,
                };
                let Some(action) = action else { continue };
                // An action ALREADY queued (a user gesture from a few
                // seconds ago — mark_seen, archiving): we do NOT log
                // ours AND we do NOT remove the local copy (review
                // 2026-08-30: the E3 arrival pattern assumes a NEW
                // message with no possible action; on stock, removing
                // without having set the intent would make cleanup
                // believe a message the server keeps has gone — it
                // would come back on the next poll). The message stays
                // visible, consistent with the server — a stated limit.
                let already = tx
                    .prepare_cached(
                        "SELECT 1 FROM pending_actions WHERE mailbox_id = ?1 AND uid = ?2",
                    )?
                    .exists(params![mailbox_id, uid])?;
                if already {
                    continue;
                }
                tx.prepare_cached(
                    "INSERT INTO pending_actions (mailbox_id, uid, kind) VALUES (?1, ?2, ?3)",
                )?
                .execute(params![mailbox_id, uid, action.to_kind()])?;
                removals.push((mailbox_id, uid));
            }
        }
        set_verdict(&tx, &address, destination, rule, epoch)?;
        tx.execute(
            "UPDATE nettoyage_session SET traites = traites + 1 WHERE id = 1",
            [],
        )?;
        tx.commit()?;
        // The local removal AFTER the commit (E3 pattern): the intent
        // is in the database, a crash here loses nothing — the local
        // copy will go at the next reconciliation. In ONE transaction
        // (review 2026-08-30: a removal per autocommit paid an fsync
        // per message — seconds on a large group, under the commands
        // lock).
        let handled = removals.len();
        if !removals.is_empty() {
            let tx = self.0.unchecked_transaction()?;
            // ONCE per thread touched, never per message
            // (PLAN-AUDIT-V2 E4, the `remove_absent` pattern): a group
            // of N messages from the same sender often lives in a few
            // threads — `remove_local` per message refreshed each one
            // N times.
            let mut touched: BTreeSet<i64> = BTreeSet::new();
            for (mailbox_id, uid) in removals {
                if let Some(thread) = purge_message(&tx, mailbox_id, uid)? {
                    touched.insert(thread);
                }
            }
            for thread in &touched {
                thread::refresh(&tx, *thread)?;
            }
            tx.commit()?;
        }
        Ok(handled)
    }

    /// A group's mail, within the session's range and scope — the
    /// read the sort screen offers when entering a group (view only,
    /// never sort at the message level: the verdict stays at the
    /// group, a scope refusal from the PLAN). The most recent first.
    /// Empty without a session.
    pub fn cleanup_messages(&self, address: &str) -> Result<Vec<UnifiedRow>, Error> {
        let Some(session) = self.cleanup_state()? else {
            return Ok(Vec::new());
        };
        let Some(address) = images_address(Some(address.to_string())) else {
            return Ok(Vec::new());
        };
        let ids = self.mailboxes_in_scope(&session.scope)?;
        // THE shared criterion: a group's view shows exactly what the
        // verdict will process.
        let sql = Self::cleanup_messages_sql(&ids);
        let mut stmt = self.0.prepare(&sql)?;
        let rows = stmt
            .query_map(params![session.bound_epoch, address], row_to_threaded)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Closes the session (the progress is cleared; the verdicts,
    /// though, stay set — they live in the routing).
    pub fn cleanup_finish(&self) -> Result<(), Error> {
        self.0
            .execute("DELETE FROM nettoyage_session WHERE id = 1", [])?;
        Ok(())
    }
}

use super::*;

/// The computation of the `organise_hors` flag of A thread (E2) — THE
/// fragment shared by `thread::refresh` (upkeep) and the migration
/// backfill: one single piece of writing for the rule, never two
/// copies that diverge. `thread_param` designates the thread
/// (parameter or column).
///
/// Golden rule (E2 review) — never lose mail:
/// - a sender routed to a VIEW (feed/paper trail) ejects the thread
///   from ONE message on — the thread lives in its view (mirror of
///   [`fil_route_sql`]), nothing is lost;
/// - a screened-out or waiting sender has NO view: the thread only
///   hides if it is ENTIRELY theirs — a mixed thread (a screened-out
///   intruder replying in a known contact's thread) STAYS in the
///   Inbox.
///
/// First WHEN: both tables empty (mode never used) — two O(1)
/// probes, adopting a legacy database costs nothing.
pub(crate) fn organized_off_sql(thread_param: &str) -> String {
    format!(
        "CASE
           WHEN NOT EXISTS (SELECT 1 FROM routage_expediteurs LIMIT 1)
            AND NOT EXISTS (SELECT 1 FROM portier_attente LIMIT 1) THEN 0
           WHEN EXISTS (
             SELECT 1 FROM envelopes te
               JOIN routage_expediteurs r
                 ON r.address = te.sender_norm
                AND r.destination IN ('kiosque', 'registre')
              WHERE te.thread_id = {thread_param}) THEN 1
           WHEN NOT EXISTS (
             SELECT 1 FROM envelopes o
              WHERE o.thread_id = {thread_param}
                AND NOT EXISTS (SELECT 1 FROM portier_attente pa
                                 WHERE pa.address = o.sender_norm)
                AND NOT EXISTS (SELECT 1 FROM routage_expediteurs re
                                 WHERE re.address = o.sender_norm
                                   AND re.destination = 'ecarte')) THEN 1
           ELSE 0 END"
    )
}

/// The Feed and Paper trail filter (PLAN-MODE-ORGANISE E1, review): a
/// thread belongs to the destination if ANY of its messages comes
/// from a sender routed there — never just the HEAD, which is the
/// last message across all mailboxes: replying to it moves it to
/// Sent and the thread would be ejected from its destination (proven
/// RED). Probed via `idx_envelopes_thread` then the routing PK (spike
/// S2), placed INSIDE the paginated skeleton — never after the LIMIT
/// (short pages, S2 reserve). `sender_norm` (generated column, E2)
/// IS the original `lower(trim(sender_address))` — a single
/// expression, defined once; its divergence from `images_address`
/// (Rust) on non-ASCII remains the assumed E1 limit: a real address
/// is ASCII.
pub(crate) fn thread_route_sql(destination_param: &str) -> String {
    format!(
        "EXISTS (
                   SELECT 1 FROM envelopes te
                     JOIN routage_expediteurs r
                       ON r.address = te.sender_norm
                      AND r.destination = {destination_param}
                    WHERE te.thread_id = threads.id
               )"
    )
}

/// The Feed/Paper trail page — the EXACT skeleton of
/// [`unified_page_sql`] plus [`fil_route_sql`]: same sort, same
/// joins. PINNED threads are NOT excluded (E1 review): their
/// dedicated section only exists in the Inbox — excluding them here
/// would make a pinned thread routed elsewhere disappear from ALL
/// organized views. `?1` limit, `?2` offset, `?3` destination, `?4`
/// account (if `by_account`).
pub(crate) fn routing_page_sql(by_account: bool, unread_only: bool) -> String {
    let filter = if by_account {
        " AND account_id = ?4"
    } else {
        ""
    };
    let unread_only_clause = if unread_only { " AND unseen > 0" } else { "" };
    let thread_route = thread_route_sql("?3");
    let tail = unified_join_tail(false);
    // E5: the Feed and Paper trail are ORGANIZED views — a thread set
    // aside leaves them too (it lives in the pile).
    let out_of_pile = format!(" AND id NOT IN ({SET_ASIDE_THREADS})");
    format!(
        "{SELECT_UNIFIED}{THREAD_AGGREGATE}
         FROM (SELECT account_id, last_mailbox_id, last_uid, last_epoch, size, unseen
                 FROM threads
                WHERE inbox_size > 0
                  AND {thread_route}{out_of_pile}{filter}{unread_only_clause}
                ORDER BY last_epoch DESC, last_uid DESC, account_id
                LIMIT ?1 OFFSET ?2) t{tail}"
    )
}

/// The CLOSED vocabularies of routing (PLAN-MODE-ORGANISE E1) — the
/// same table serves Rust validation and, as a belt, the schema's
/// CHECK constraints. `ecarte` is the only destination that accepts a
/// rule.
const ROUTING_DESTINATIONS: [&str; 4] = ["reception", "kiosque", "registre", "ecarte"];

const ROUTING_RULES: [&str; 3] = ["spam", "archive", "corbeille"];

/// The `prefs` keys of Organized mode — the state, and the retention
/// bound of the Screener (first activation, never rewritten).
const PREF_ORGANIZED_MODE: &str = "mode_organise";

const PREF_ORGANIZED_MODE_EPOCH: &str = "mode_organise_epoch";

/// RETOURS-13 R5/R9 — the Screener buttons' defaults: Yes takes a
/// destination (never `ecarte`), No a rule from the screened-out
/// vocabulary or bare `ecarte` ("screen out without moving").
/// DERIVED from the routing tables — never a second copy of the
/// vocabulary (review: a destination added to ROUTING_DESTINATIONS
/// would have left the Settings selector silently refusing it).
const PREF_SCREENER_DEFAULT_YES: &str = "portier_defaut_oui";

const PREF_SCREENER_DEFAULT_NO: &str = "portier_defaut_non";

fn valid_screener_default_yes(v: &str) -> bool {
    v != "ecarte" && ROUTING_DESTINATIONS.contains(&v)
}

fn valid_screener_default_no(v: &str) -> bool {
    v == "ecarte" || ROUTING_RULES.contains(&v)
}

/// THE single validation gate for the routing vocabulary — called
/// before every write AND before every address resolution (a holed
/// vocabulary never hides behind another refusal).
pub(super) fn validate_routing(destination: &str, rule: Option<&str>) -> Result<(), Error> {
    if !ROUTING_DESTINATIONS.contains(&destination) {
        return Err(Error::InvalidRouting(format!(
            "unknown destination: {destination:?}"
        )));
    }
    if let Some(r) = rule {
        if destination != "ecarte" {
            return Err(Error::InvalidRouting(format!(
                "a No rule requires a screened-out sender, not {destination:?}"
            )));
        }
        if !ROUTING_RULES.contains(&r) {
            return Err(Error::InvalidRouting(format!("unknown rule: {r:?}")));
        }
    }
    Ok(())
}

/// The Screener's verdict on a sender — a row of
/// `routage_expediteurs`, as history shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Routing {
    pub address: String,
    pub destination: String,
    pub rule: Option<String>,
    pub epoch: i64,
}

fn read_routing(row: &rusqlite::Row<'_>) -> rusqlite::Result<Routing> {
    Ok(Routing {
        address: row.get(0)?,
        destination: row.get(1)?,
        rule: row.get(2)?,
        epoch: row.get(3)?,
    })
}

/// A rank of the Screener's desk (E2): the WAITING address —
/// normalized, the key the verdict will take — and its last message.
#[derive(Debug)]
pub struct ScreenerRank {
    pub address: String,
    pub row: UnifiedRow,
}

/// The threads of ONE sender — THE single definition, shared by the
/// verdict recompute and the pending-state clearing of the sync path.
pub(super) fn threads_of(conn: &Connection, address: &str) -> Result<Vec<i64>, Error> {
    let threads = conn
        .prepare_cached(
            "SELECT DISTINCT thread_id FROM envelopes
              WHERE sender_norm = ?1 AND thread_id IS NOT NULL",
        )?
        .query_map(params![address], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(threads)
}

/// The transactional CORE of the verdict — THE single gate, shared by
/// [`Store::router_expediteur`] (Screener, "Move to…") and
/// [`Store::nettoyage_verdict`] (2026-08-30 review: Cleanup was
/// recopying the body; a future addition to "set a verdict" would
/// have diverged depending on the originating screen). The caller
/// validates the vocabulary and normalizes the address BEFOREHAND.
pub(super) fn set_verdict(
    tx: &Connection,
    address: &str,
    destination: &str,
    rule: Option<&str>,
    epoch: i64,
) -> Result<(), Error> {
    tx.execute(
        "INSERT OR REPLACE INTO routage_expediteurs (address, destination, regle, epoch)
         VALUES (?1, ?2, ?3, ?4)",
        params![address, destination, rule, epoch],
    )?;
    // RETOURS-14 R8 (field finding 2026-08-31): a YES means trust —
    // the verdict ALSO sets the rule "always show images from this
    // sender" (same table, same normalization as the R1 guard;
    // revocable in Settings > Display). A No does not touch the
    // guard — it has its own way out.
    if destination != "ecarte" {
        tx.execute(
            "INSERT OR REPLACE INTO images_expediteurs (address, epoch) VALUES (?1, ?2)",
            params![address, epoch],
        )?;
    }
    // The verdict takes over from the pending state — Yes as much as
    // No.
    tx.execute(
        "DELETE FROM portier_attente WHERE address = ?1",
        params![address],
    )?;
    refresh_threads_of(tx, address)
}

/// Recomputes the flags of ONE sender's threads through THE single
/// gate (`thread::refresh`) — after a verdict or a reinstatement.
/// Bounded to the address's threads (63 ms measured on a sender with
/// 10,000 threads, single gesture).
fn refresh_threads_of(conn: &Connection, address: &str) -> Result<(), Error> {
    for thread in threads_of(conn, address)? {
        thread::refresh(conn, thread)?;
    }
    Ok(())
}

/// Is the address one of OUR accounts? Never oneself at the Screener
/// (E1 lesson: the user's own address is never a sender to sort).
/// `prepare_cached`: the probe lives on the hot path of sync (E2
/// review).
pub(super) fn account_address(conn: &Connection, address: &str) -> Result<bool, Error> {
    Ok(conn
        .prepare_cached("SELECT 1 FROM accounts WHERE lower(trim(email)) = ?1")?
        .exists(params![address])?)
}

/// Does the sender have mail PRIOR to the activation epoch? This is
/// THE definition of "known" from D3 (arrivals only) — one single
/// piece of writing, shared by the arrival decision and the
/// reinstatement: two copies would diverge on the very meaning of the
/// desk. Across all mailboxes: history in Archive or Junk is still
/// history.
pub(super) fn known_before_epoch(
    conn: &Connection,
    address: &str,
    epoch: i64,
) -> Result<bool, Error> {
    Ok(conn
        .prepare_cached(
            "SELECT 1 FROM envelopes
              WHERE sender_norm = ?1 AND date_epoch <= ?2 LIMIT 1",
        )?
        .exists(params![address, epoch])?)
}

pub(super) fn purge_orphan_pending(conn: &Connection) -> Result<(), Error> {
    conn.execute(
        "DELETE FROM portier_attente WHERE NOT EXISTS (
             SELECT 1 FROM envelopes e WHERE e.sender_norm = portier_attente.address)",
        [],
    )?;
    Ok(())
}

impl Store {
    /// The state of Organized mode (PLAN-MODE-ORGANISE E1, D2 amended:
    /// `prefs` SQLite — the core needs to know, the No rules die with
    /// the mode). Off as long as nothing has been set.
    pub fn organized_mode(&self) -> Result<bool, Error> {
        Ok(self.text_pref(PREF_ORGANIZED_MODE)?.as_deref() == Some("1"))
    }

    /// The epoch of the FIRST activation of the mode — the bound of
    /// the Screener's retention (D3 “arrivals only”). None as long as
    /// the mode has never been activated.
    pub fn organized_mode_epoch(&self) -> Result<Option<i64>, Error> {
        Ok(self
            .text_pref(PREF_ORGANIZED_MODE_EPOCH)?
            .and_then(|v| v.parse().ok()))
    }

    /// Toggles the mode. On the FIRST activation, the state and the
    /// epoch are written TOGETHER (transaction — never an active mode
    /// without its bound); the epoch is NEVER rewritten afterwards:
    /// rewriting it on a reactivation would silently dump mail that
    /// arrived in the meantime into the Screener (or the Inbox).
    pub fn set_organized_mode(&mut self, active: bool, epoch: i64) -> Result<(), Error> {
        if active && self.text_pref(PREF_ORGANIZED_MODE_EPOCH)?.is_none() {
            self.set_text_prefs(&[
                (PREF_ORGANIZED_MODE, "1"),
                (PREF_ORGANIZED_MODE_EPOCH, &epoch.to_string()),
            ])
        } else {
            self.set_text_pref(PREF_ORGANIZED_MODE, if active { "1" } else { "0" })
        }
    }

    /// RETOURS-13 R10 — marks a Feed card as read (the bottom of its
    /// elevation has been shown). Idempotent; envelope key, the `pins`
    /// pattern — never the IMAP `seen` (different semantics, overwritten
    /// by sync).
    pub fn mark_feed_read(&self, mailbox_id: i64, uid: Uid, epoch: i64) -> Result<(), Error> {
        self.0.execute(
            "INSERT OR IGNORE INTO kiosque_lus (mailbox_id, uid, epoch) VALUES (?1, ?2, ?3)",
            params![mailbox_id, uid, epoch],
        )?;
        Ok(())
    }

    /// Has a Feed card already been read? (PK probe)
    pub fn feed_read(&self, mailbox_id: i64, uid: Uid) -> Result<bool, Error> {
        Ok(self
            .0
            .prepare("SELECT 1 FROM kiosque_lus WHERE mailbox_id = ?1 AND uid = ?2")?
            .exists(params![mailbox_id, uid])?)
    }

    /// RETOURS-13 R5/R9 — the default actions of the Screener's
    /// Yes/No buttons. Shipped: Yes → `inbox`, No → `trash`. A
    /// value outside the vocabulary in the database (written outside
    /// the gate) falls back to the default: the bare click NEVER sets
    /// a broken verdict.
    pub fn screener_defaults(&self) -> Result<(String, String), Error> {
        let yes = self
            .text_pref(PREF_SCREENER_DEFAULT_YES)?
            .filter(|v| valid_screener_default_yes(v))
            .unwrap_or_else(|| "reception".to_string());
        let no = self
            .text_pref(PREF_SCREENER_DEFAULT_NO)?
            .filter(|v| valid_screener_default_no(v))
            .unwrap_or_else(|| "corbeille".to_string());
        Ok((yes, no))
    }

    /// Sets the Screener's defaults — a CLOSED vocabulary, checked
    /// before any write (pure decision): Yes takes a destination
    /// (never `ecarte`), No a rule or “screen out without moving”.
    pub fn set_screener_defaults(&mut self, yes: &str, no: &str) -> Result<(), Error> {
        if !valid_screener_default_yes(yes) {
            return Err(Error::InvalidRouting(format!(
                "unknown Yes default: {yes:?}"
            )));
        }
        if !valid_screener_default_no(no) {
            return Err(Error::InvalidRouting(format!("unknown No default: {no:?}")));
        }
        self.set_text_prefs(&[
            (PREF_SCREENER_DEFAULT_YES, yes),
            (PREF_SCREENER_DEFAULT_NO, no),
        ])
    }

    /// Sets the Organized mode verdict on a sender
    /// (PLAN-MODE-ORGANISE E1, D1: LOCAL routing only). One verdict per
    /// address — setting it overwrites the previous decision (changing
    /// one's mind is a right, the Screener's pattern). The vocabulary
    /// is closed and checked BEFORE the write (pure decision); a No
    /// rule only makes sense on a screened-out sender.
    pub fn route_sender(
        &self,
        address: &str,
        destination: &str,
        rule: Option<&str>,
        epoch: i64,
    ) -> Result<(), Error> {
        validate_routing(destination, rule)?;
        let Some(address) = images_address(Some(address.to_string())) else {
            return Err(Error::InvalidEmailAddress(address.to_string()));
        };
        // Verdict, exit from waiting, and thread flags in ONE
        // transaction (E2): a half-applied verdict would leave a sender in
        // the Screener AND in its view.
        let tx = self.0.unchecked_transaction()?;
        set_verdict(&tx, &address, destination, rule, epoch)?;
        tx.commit()?;
        Ok(())
    }

    /// “Move to…” (E1): sets the verdict FROM a message — the address
    /// is read from the database (never from the UI), normalized and
    /// validated by [`Store::route_sender`], THE single gate.
    ///
    /// E1 review: the row served is the HEAD of the thread — the last
    /// message across all mailboxes, Sent included. Anchoring on it
    /// would route the user's OWN address as soon as they replied last.
    /// The routed address is therefore that of the last message of the
    /// thread that does NOT come from the account (send aliases stay
    /// outside this guard — a stated limit); a message outside a thread
    /// falls back to its own envelope. Returns the routed address;
    /// None if nothing carries an address (never a phantom verdict).
    pub fn route_sender_of(
        &self,
        mailbox_id: i64,
        uid: Uid,
        destination: &str,
        rule: Option<&str>,
        epoch: i64,
    ) -> Result<Option<String>, Error> {
        // The validation gate first — a broken vocabulary never hides
        // behind “message without an address” (E1 review).
        validate_routing(destination, rule)?;
        let of_thread: Option<String> = self
            .0
            .query_row(
                "SELECT te.sender_address
                   FROM envelopes te
                  WHERE te.thread_id = (SELECT thread_id FROM envelopes
                                         WHERE mailbox_id = ?1 AND uid = ?2)
                    AND te.sender_address IS NOT NULL
                    AND lower(trim(te.sender_address)) <> (
                          SELECT lower(trim(a.email)) FROM accounts a
                            JOIN mailboxes m ON m.account_id = a.id
                           WHERE m.id = ?1)
                  ORDER BY te.date_epoch DESC, te.uid DESC
                  LIMIT 1",
                params![mailbox_id, uid],
                |row| row.get(0),
            )
            .optional()?;
        let address = match of_thread {
            Some(a) => Some(a),
            None => self.sender_address_of(mailbox_id, uid)?,
        };
        let Some(address) = images_address(address) else {
            return Ok(None);
        };
        self.route_sender(&address, destination, rule, epoch)?;
        Ok(Some(address))
    }

    /// The sender address of ONE envelope — the shared read of the
    /// gates that resolve on the core side (image guard, routing): a
    /// single copy, never a divergence (lesson A80).
    fn sender_address_of(&self, mailbox_id: i64, uid: Uid) -> Result<Option<String>, Error> {
        Ok(self
            .0
            .query_row(
                "SELECT sender_address FROM envelopes WHERE mailbox_id = ?1 AND uid = ?2",
                params![mailbox_id, uid],
                |row| row.get(0),
            )
            .optional()?
            .flatten())
    }

    /// “Reinstate” from the Screener's history: the verdict
    /// disappears, the sender becomes unknown again. The normalization
    /// goes through the SAME authority as the write — otherwise a
    /// verdict would become irrevocable the day it changes (lesson
    /// `revoke_images_sender`).
    pub fn remove_routing(&self, address: &str) -> Result<(), Error> {
        let Some(address) = images_address(Some(address.to_string())) else {
            return Ok(());
        };
        let epoch = self.organized_mode_epoch()?;
        let tx = self.0.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM routage_expediteurs WHERE address = ?1",
            params![address],
        )?;
        // “Reinstate” (E2): an UNKNOWN sender — no mail before the
        // epoch — becomes a waiting sender again, their messages
        // reappear in the Screener; a known one is simply returned to
        // the Inbox, never to the desk (D3: their history is
        // authoritative). Never oneself (lesson E1). The exit gate
        // follows the SAME rule as arrival (E2 review): only mail that
        // ARRIVED after the epoch reinstates — a sender seen only in
        // Archive or Junk never passed the desk.
        if let Some(epoch) = epoch
            && !account_address(&tx, &address)?
            && !known_before_epoch(&tx, &address, epoch)?
            && tx
                .prepare(
                    "SELECT 1 FROM envelopes e
                       JOIN mailboxes m ON m.id = e.mailbox_id
                      WHERE e.sender_norm = ?1
                        AND (e.date_epoch > ?2 OR e.date_epoch IS NULL)
                        AND m.name = ?3 LIMIT 1",
                )?
                .exists(params![address, epoch, thread::RECEIVED_MAILBOX])?
        {
            tx.execute(
                "INSERT OR IGNORE INTO portier_attente (address) VALUES (?1)",
                params![address],
            )?;
        }
        refresh_threads_of(&tx, &address)?;
        tx.commit()?;
        Ok(())
    }

    /// The verdict set on a sender, if it exists.
    pub fn routing_of(&self, address: &str) -> Result<Option<Routing>, Error> {
        let Some(address) = images_address(Some(address.to_string())) else {
            return Ok(None);
        };
        let routing = self
            .0
            .query_row(
                "SELECT address, destination, regle, epoch
                 FROM routage_expediteurs WHERE address = ?1",
                params![address],
                read_routing,
            )
            .optional()?;
        Ok(routing)
    }

    /// The Screener's history: all decisions, the most recent first —
    /// the eye looks for the last verdict there.
    pub fn routings(&self) -> Result<Vec<Routing>, Error> {
        let mut stmt = self.0.prepare(
            "SELECT address, destination, regle, epoch
             FROM routage_expediteurs ORDER BY epoch DESC, address",
        )?;
        let routings = stmt
            .query_map([], read_routing)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(routings)
    }

    /// The Screener's desk (E2): one rank per waiting sender — the
    /// normalized address (the key the verdict will take) and its
    /// LAST arrival after the epoch, in the format of list rows. The
    /// most recent first. Empty as long as the mode has never been
    /// activated. The desk only counts ARRIVALS (E2 review): a message
    /// from the same sender already discarded or archived is neither
    /// the rank nor the count — the rank's mailbox is the INBOX. The
    /// probes follow `idx_envelopes_sender` (0.32 ms at 200 k,
    /// S2-bis); a rank whose mail has disappeared is not served.
    pub fn screener_waiting(&self) -> Result<Vec<ScreenerRank>, Error> {
        let Some(epoch) = self.organized_mode_epoch()? else {
            return Ok(Vec::new());
        };
        // `COALESCE` on the aggregate: the rank shows ONE message — if
        // its thread doesn't exist (mailbox out of scope), it counts for
        // itself.
        let sql = format!(
            "{SELECT_UNIFIED}, COALESCE(t.size, 1), COALESCE(t.unseen, 1 - e.seen), pa.address
             FROM portier_attente pa
             JOIN envelopes e ON e.rowid = (
                  SELECT e2.rowid FROM envelopes e2
                    JOIN mailboxes m2 ON m2.id = e2.mailbox_id AND m2.name = ?2
                   WHERE e2.sender_norm = pa.address
                     AND (e2.date_epoch > ?1 OR e2.date_epoch IS NULL)
                   ORDER BY e2.date_epoch DESC, e2.uid DESC LIMIT 1)
             JOIN mailboxes m ON m.id = e.mailbox_id
             JOIN accounts a ON a.id = m.account_id
             LEFT JOIN threads t ON t.id = e.thread_id
             LEFT JOIN bodies b ON b.mailbox_id = e.mailbox_id AND b.uid = e.uid
             ORDER BY e.date_epoch DESC, e.uid DESC"
        );
        let mut stmt = self.0.prepare(&sql)?;
        let ranks = stmt
            .query_map(params![epoch, thread::RECEIVED_MAILBOX], |row| {
                Ok(ScreenerRank {
                    row: row_to_threaded(row)?,
                    address: row.get(19)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ranks)
    }

    /// The Screener's badge: how many MESSAGES are waiting — the
    /// arrivals after the epoch of the waiting senders, the same scope
    /// as the desk (never a count the page couldn't show). Sum of
    /// index intervals, 0.26 ms at 200 k.
    pub fn screener_total(&self) -> Result<u64, Error> {
        let Some(epoch) = self.organized_mode_epoch()? else {
            return Ok(0);
        };
        let total: i64 = self.0.query_row(
            "SELECT COALESCE(SUM((SELECT COUNT(*) FROM envelopes e
                     JOIN mailboxes m ON m.id = e.mailbox_id AND m.name = ?2
                     WHERE e.sender_norm = pa.address
                       AND (e.date_epoch > ?1 OR e.date_epoch IS NULL))), 0)
             FROM portier_attente pa",
            params![epoch, thread::RECEIVED_MAILBOX],
            |row| row.get(0),
        )?;
        Ok(total as u64)
    }

    /// RETOURS-14 R4 (review) — the ADDRESSES alone from the desk,
    /// for the thread's “Waiting in the Screener” badge:
    /// `screener_waiting()` builds a full row per sender, which the
    /// badge has no use for — and the desk is unbounded.
    pub fn screener_addresses(&self) -> Result<Vec<String>, Error> {
        let mut stmt = self
            .0
            .prepare("SELECT address FROM portier_attente ORDER BY address")?;
        let addresses = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(addresses)
    }
}

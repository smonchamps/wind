//! Backfill of message bodies — the background pump of [ADR 0007].
//!
//! The "envelopes first" sync (PLAN.md §3) makes the list usable instantly,
//! but only downloads a body on click. Measured in the field: 18 bodies out
//! of 537, 1 out of 2193. Full-text search therefore covered, in practice,
//! only subjects and senders.
//!
//! This pump completes the sync without contradicting it: it runs AFTER, in
//! the background, and fetches the bodies of recent messages.
//!
//! Candidate work is debited before I/O. Persistent date/UID scan positions
//! survive failed responses; missing bodies remain eligible after wrap. The
//! byte and time allowance is shared by the caller across mailboxes.
//!
//! [ADR 0007]: ../../../docs/adr/0007-body-backfill.md

use crate::{WorkBudget, backfill_cursor::Kind};
use std::collections::HashSet;

use crate::envelope::Uid;
use crate::error::Error;
use crate::remote::MailServer;
use crate::store::Store;

/// Bodies requested in one command. 50 is the chosen trade-off: enough to
/// amortize the round trip, few enough that an interruption only loses a
/// small batch and progress stays alive on screen.
pub const BACKFILL_BATCH: usize = 50;

/// "No horizon": the value of `since_epoch` that bounds nothing.
///
/// The 12-month horizon of [ADR 0007] existed to hold the disk budget
/// (< 1 GB). [ADR 0010] lifts that budget: production now passes this
/// constant, and the bound only survives as a parameter — tests use it to
/// replay bounded scenarios, and a future user setting would find it
/// unchanged.
///
/// `i64::MIN` and not `0`: a date before 1970 — a wrong clock, a corrupt
/// header — produces a negative epoch, and "everything" must cover that
/// too.
///
/// [ADR 0010]: ../../../docs/adr/0010-full-synchronization.md
pub const NO_HORIZON: i64 = i64::MIN;

/// The CLOSED vocabulary of the "history depth" setting (ADR 0029,
/// PLAN-HORIZON-NETTOYAGE D1) — the values offered at the account-add desk,
/// in the order of the selector. The value lives as a per-account pref
/// (`horizon_import.{id}`, [`crate::store::PREFS_PER_ACCOUNT`]).
pub const HORIZONS_IMPORT: &[&str] = &["1m", "2m", "3m", "6m", "1a", "2a", "tout"];

/// Translates the symbolic value into an epoch bound for the BODY pumps
/// (envelopes stay complete — D1: the list and the subject/sender search
/// cover everything).
///
/// Full days, derived on READ: the bound follows the clock, never a date
/// frozen at the moment the account was added. The unknown bounds NOTHING:
/// clipping the import on a corrupt pref would be a silent loss — the safe
/// default is "everything" (D4).
pub fn horizon_epoch(value: &str, now: i64) -> i64 {
    const DAY: i64 = 86_400;
    let days = match value {
        "1m" => 30,
        "2m" => 61,
        "3m" => 91,
        "6m" => 183,
        "1a" => 365,
        "2a" => 730,
        // "5a" belongs only to the Spring cleaning vocabulary
        // (CLEANUP_RANGES) — HORIZONS_IMPORT guards the door of the
        // import setting, the translation is shared.
        "5a" => 1826,
        _ => return NO_HORIZON,
    };
    now - days * DAY
}

/// The percentage of bodies ALREADY fetched over the corpus in scope
/// (R1, PLAN-RETOURS-3) — `done` = messages with a body, `total` = all
/// messages in scope.
///
/// A **pure and testable** decision (PASSATION §4 pattern), sibling of
/// [`crate::sync_percent`] with which it shares both guards: `None` with no
/// denominator (no message — "0%" would be indistinguishable from a
/// backfill at a standstill), and "100%" reserved for a backfill that is
/// TRULY finished — 255,999/256,000 rounds to 99, never to 100, or the
/// status bar would announce the end while the long tail is still running.
pub fn backfill_percent(done: u64, total: u64) -> Option<u8> {
    if total == 0 {
        return None;
    }
    if done >= total {
        return Some(100);
    }
    Some((done * 100 / total).min(99) as u8)
}

/// What a pass did, and what remains to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackfillReport {
    /// Bodies fetched and indexed during this pass.
    pub fetched: usize,
    /// Messages in the horizon still waiting for their body.
    pub remaining: u64,
    pub scanned: usize,
    pub more: bool,
}

pub const THREAD_HEADER_BATCH: usize = 200;

pub(crate) fn fetch_arrival_bodies(
    server: &mut dyn MailServer,
    store: &Store,
    account_id: i64,
    mailbox: &str,
    after: Uid,
    since: i64,
    limit: usize,
) -> Result<(), Error> {
    let Some(identity) = store.mailbox_identity(account_id, mailbox)? else {
        return Ok(());
    };
    store.admit_background_write(0)?;
    let uids = store.conn().prepare("SELECT uid FROM envelopes WHERE mailbox_id = ?1 AND uid > ?2 AND (date_epoch IS NULL OR date_epoch >= ?3) ORDER BY uid DESC LIMIT ?4")?
        .query_map(rusqlite::params![identity.mailbox_id, after, since, limit as i64], |r| r.get(0))?.collect::<Result<Vec<Uid>, _>>()?;
    let mut budget = WorkBudget::new(limit);
    let before = budget.begin_fetch(server);
    let result = crate::body::fetch_bodies_for_store(server, store, &identity, &uids);
    budget.end_fetch(server, before);
    for (uid, body) in result? {
        let invitation = crate::body::invitation_from(store, account_id, body.ics.as_deref())?;
        store.save_body_checked(
            &identity,
            uid,
            &body.html,
            &body.attachments,
            invitation.as_ref(),
        )?;
    }
    Ok(())
}

pub fn backfill_bodies(
    server: &mut dyn MailServer,
    store: &mut Store,
    account_id: i64,
    mailbox: &str,
    since_epoch: i64,
    budget: usize,
) -> Result<BackfillReport, Error> {
    backfill_bodies_budgeted(
        server,
        store,
        account_id,
        mailbox,
        since_epoch,
        &mut WorkBudget::new(budget),
    )
}

pub fn backfill_bodies_budgeted(
    server: &mut dyn MailServer,
    store: &mut Store,
    account_id: i64,
    mailbox: &str,
    since_epoch: i64,
    budget: &mut WorkBudget,
) -> Result<BackfillReport, Error> {
    backfill(
        server,
        store,
        account_id,
        mailbox,
        since_epoch,
        Kind::Bodies,
        budget,
    )
}

pub fn backfill_recipients(
    server: &mut dyn MailServer,
    store: &mut Store,
    account_id: i64,
    mailbox: &str,
    budget: usize,
) -> Result<BackfillReport, Error> {
    backfill_recipients_budgeted(
        server,
        store,
        account_id,
        mailbox,
        &mut WorkBudget::new(budget),
    )
}

pub fn backfill_recipients_budgeted(
    server: &mut dyn MailServer,
    store: &mut Store,
    account_id: i64,
    mailbox: &str,
    budget: &mut WorkBudget,
) -> Result<BackfillReport, Error> {
    backfill(
        server,
        store,
        account_id,
        mailbox,
        NO_HORIZON,
        Kind::Recipients,
        budget,
    )
}

pub fn backfill_thread_headers(
    server: &mut dyn MailServer,
    store: &mut Store,
    account_id: i64,
    mailbox: &str,
    since_epoch: i64,
    budget: usize,
) -> Result<BackfillReport, Error> {
    backfill_thread_headers_budgeted(
        server,
        store,
        account_id,
        mailbox,
        since_epoch,
        &mut WorkBudget::new(budget),
    )
}

pub fn backfill_thread_headers_budgeted(
    server: &mut dyn MailServer,
    store: &mut Store,
    account_id: i64,
    mailbox: &str,
    since_epoch: i64,
    budget: &mut WorkBudget,
) -> Result<BackfillReport, Error> {
    backfill(
        server,
        store,
        account_id,
        mailbox,
        since_epoch,
        Kind::Headers,
        budget,
    )
}

fn backfill(
    server: &mut dyn MailServer,
    store: &mut Store,
    account_id: i64,
    mailbox: &str,
    since: i64,
    kind: Kind,
    budget: &mut WorkBudget,
) -> Result<BackfillReport, Error> {
    if !store.operation_due(
        account_id,
        mailbox,
        kind.key(),
        chrono::Utc::now().timestamp(),
    )? {
        return Ok(BackfillReport {
            fetched: 0,
            remaining: pending_count(store, account_id, mailbox, since, kind)?,
            scanned: 0,
            more: false,
        });
    }
    let Some(identity) = store.mailbox_identity(account_id, mailbox)? else {
        return Ok(BackfillReport {
            fetched: 0,
            remaining: 0,
            scanned: 0,
            more: false,
        });
    };
    let mut fetched = 0;
    let mut scanned = 0;
    let mut more = false;
    let batch_size = match kind {
        Kind::Bodies => BACKFILL_BATCH,
        _ => THREAD_HEADER_BATCH,
    };
    while budget.remaining() > 0 {
        if let Err(error) = store.admit_background_write(0) {
            store.settle_operation(
                account_id,
                mailbox,
                kind.key(),
                chrono::Utc::now().timestamp(),
                Some(&error),
            )?;
            return Err(error);
        }
        let window = store.claim_backfill_window(
            &identity,
            kind,
            since,
            budget.remaining().min(batch_size),
        )?;
        budget.spend_candidates(window.scanned);
        scanned += window.scanned;
        more = !window.at_end;
        if !window.uids.is_empty() {
            let before = budget.begin_fetch(server);
            let result = (|| {
                match kind {
                    Kind::Bodies => {
                        for (uid, body) in crate::body::fetch_bodies_for_store(
                            server,
                            store,
                            &identity,
                            &window.uids,
                        )? {
                            let invitation = crate::body::invitation_from(
                                store,
                                account_id,
                                body.ics.as_deref(),
                            )?;
                            store.save_body_checked(
                                &identity,
                                uid,
                                &body.html,
                                &body.attachments,
                                invitation.as_ref(),
                            )?;
                            fetched += 1;
                            budget.record_saved();
                        }
                    }
                    Kind::Recipients => {
                        crate::remote::verify_mailbox_generation(
                            server,
                            mailbox,
                            identity.uid_validity,
                        )?;
                        let envelopes = server.fetch_envelopes(mailbox, &window.uids)?;
                        crate::remote::verify_mailbox_generation(
                            server,
                            mailbox,
                            identity.uid_validity,
                        )?;
                        validate_uids(&window.uids, envelopes.iter().map(|e| e.uid))?;
                        for envelope in envelopes {
                            store.set_recipients_checked(
                                &identity,
                                envelope.uid,
                                &envelope.to_addrs,
                                &envelope.cc_addrs,
                            )?;
                            fetched += 1;
                            budget.record_saved();
                        }
                    }
                    Kind::Headers => {
                        crate::remote::verify_mailbox_generation(
                            server,
                            mailbox,
                            identity.uid_validity,
                        )?;
                        let headers = server.fetch_thread_headers(mailbox, &window.uids)?;
                        crate::remote::verify_mailbox_generation(
                            server,
                            mailbox,
                            identity.uid_validity,
                        )?;
                        validate_uids(&window.uids, headers.iter().map(|(uid, _)| *uid))?;
                        for (uid, headers) in headers {
                            store.set_thread_headers_checked(
                                &identity,
                                uid,
                                headers.in_reply_to.as_deref(),
                                headers.references.as_deref().unwrap_or_default(),
                            )?;
                            fetched += 1;
                            budget.record_saved();
                        }
                    }
                }
                Ok::<_, Error>(())
            })();
            budget.end_fetch(server, before);
            if let Err(error) = &result {
                store.settle_operation(
                    account_id,
                    mailbox,
                    kind.key(),
                    chrono::Utc::now().timestamp(),
                    Some(error),
                )?;
            }
            result?;
        }
        if window.at_end || window.scanned == 0 {
            break;
        }
    }
    let remaining = pending_count(store, account_id, mailbox, since, kind)?;
    // Every window ran without error: the diagnostic, if any, is settled.
    // Content the server does not hold is not a failure — it stays counted
    // as missing (so nothing claims a complete download) and returns on a
    // later sweep.
    store.settle_operation(
        account_id,
        mailbox,
        kind.key(),
        chrono::Utc::now().timestamp(),
        None,
    )?;
    Ok(BackfillReport {
        fetched,
        remaining,
        scanned,
        more,
    })
}

fn pending_count(
    store: &Store,
    account_id: i64,
    mailbox: &str,
    since: i64,
    kind: Kind,
) -> Result<u64, Error> {
    match kind {
        Kind::Bodies => store.bodies_pending_count(account_id, mailbox, since),
        Kind::Recipients => store.recipients_pending_count(account_id, mailbox),
        Kind::Headers => store.thread_headers_pending_count(account_id, mailbox, since),
    }
}

fn validate_uids(requested: &[Uid], returned: impl Iterator<Item = Uid>) -> Result<(), Error> {
    let mut seen = HashSet::new();
    for uid in returned {
        if !requested.contains(&uid) || !seen.insert(uid) {
            return Err(Error::Server("invalid backfill response UID".into()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod percent_tests {
    use super::backfill_percent;

    /// With no denominator, we say nothing — especially not "0%", which
    /// would be indistinguishable from a backfill at a standstill (same
    /// rule as `sync_percent`).
    #[test]
    fn without_a_denominator_nothing_is_said() {
        assert_eq!(backfill_percent(0, 0), None);
        assert_eq!(backfill_percent(42, 0), None);
    }

    #[test]
    fn the_common_case() {
        assert_eq!(backfill_percent(0, 200), Some(0));
        assert_eq!(backfill_percent(50, 200), Some(25));
        assert_eq!(backfill_percent(200, 200), Some(100));
    }

    /// THE classic trap: "100%" while bodies still remain. On the real
    /// database (~256k), 255,999/256,000 must say 99, never 100 — or the
    /// row would announce the end while the tail is still running.
    #[test]
    fn almost_done_is_not_done() {
        assert_eq!(backfill_percent(255_999, 256_000), Some(99));
    }

    /// The "done" count cannot exceed the total (remaining ≤ total by
    /// construction), but an inconsistent fixture must not produce
    /// "103%": it is capped, like `sync_percent`.
    #[test]
    fn the_count_that_exceeds_is_capped() {
        assert_eq!(backfill_percent(210, 200), Some(100));
    }
}

#[cfg(test)]
mod horizon_tests {
    use super::{NO_HORIZON, horizon_epoch};

    const DAY: i64 = 86_400;
    const NOW: i64 = 1_756_500_000;

    /// Each vocabulary value bounds to its duration — in full days,
    /// derived on READ: the bound follows the clock, never a date frozen
    /// at the moment the account was added.
    #[test]
    fn each_vocabulary_value_bounds_to_its_duration() {
        assert_eq!(horizon_epoch("1m", NOW), NOW - 30 * DAY);
        assert_eq!(horizon_epoch("2m", NOW), NOW - 61 * DAY);
        assert_eq!(horizon_epoch("3m", NOW), NOW - 91 * DAY);
        assert_eq!(horizon_epoch("6m", NOW), NOW - 183 * DAY);
        assert_eq!(horizon_epoch("1a", NOW), NOW - 365 * DAY);
        assert_eq!(horizon_epoch("2a", NOW), NOW - 730 * DAY);
    }

    /// "Everything since the start" bounds nothing — including negative
    /// epochs (wrong clock, corrupt header), same rule as `NO_HORIZON`.
    #[test]
    fn everything_bounds_nothing() {
        assert_eq!(horizon_epoch("tout", NOW), NO_HORIZON);
    }

    /// An unknown value (corrupt pref, future vocabulary) bounds nothing:
    /// clipping the import on an unreadable value would be a silent loss —
    /// the safe default is "everything".
    #[test]
    fn the_unknown_bounds_nothing() {
        assert_eq!(horizon_epoch("6 semaines", NOW), NO_HORIZON);
        assert_eq!(horizon_epoch("", NOW), NO_HORIZON);
    }

    /// The completeness net (2026-08-30 review): every member of BOTH
    /// vocabularies (import AND cleanup) has its arm in `horizon_epoch` —
    /// except "tout". Without it, adding "10a" to `CLEANUP_RANGES` without
    /// touching the match would make the range fall to the "tout" default:
    /// a trash cleanup would sweep the ENTIRE history instead of the 10
    /// chosen years. For import, the same hole is benign (more gets
    /// imported) — for cleanup it is DESTRUCTIVE.
    #[test]
    fn each_vocabulary_value_has_its_duration() {
        for value in crate::store::CLEANUP_RANGES
            .iter()
            .chain(super::HORIZONS_IMPORT)
        {
            if *value == "tout" {
                continue;
            }
            assert_ne!(
                horizon_epoch(value, NOW),
                NO_HORIZON,
                "{value:?} falls to the \"tout\" default — horizon_epoch's match did not keep up with the vocabulary"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::FakeServer;

    /// Fixture: `n` messages with a body on the server, synced (so
    /// envelopes are in the database) but no body downloaded.
    fn synced(n: u32) -> (FakeServer, Store, i64) {
        let mut server = FakeServer::new(false);
        for uid in 1..=n {
            server.add_with_body(
                uid,
                &format!("subject {uid}"),
                &format!("<p>body {uid}</p>"),
            );
        }
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("test@exemple.fr", "gmail")
            .unwrap();
        crate::SyncEngine::default()
            .sync(&mut server, &mut store, account, "INBOX")
            .unwrap();
        (server, store, account)
    }

    #[test]
    fn empty_replies_spend_the_budget_for_all_three_pumps() {
        for kind in 0..3 {
            let (mut server, mut store, account) = synced(6);
            store
                .conn()
                .execute_batch("UPDATE envelopes SET refs = NULL, to_addrs = NULL;")
                .unwrap();
            server.messages.clear();
            server.bodies.clear();
            server.fetch_batches.clear();
            match kind {
                0 => {
                    backfill_bodies(&mut server, &mut store, account, "INBOX", 0, 2).unwrap();
                }
                1 => {
                    backfill_recipients(&mut server, &mut store, account, "INBOX", 2).unwrap();
                }
                _ => {
                    backfill_thread_headers(&mut server, &mut store, account, "INBOX", 0, 2)
                        .unwrap();
                }
            }
            let requests = match kind {
                0 => server.body_batches,
                1 => server.fetch_batches,
                _ => server.header_batches,
            };
            assert_eq!(
                requests.iter().map(Vec::len).sum::<usize>(),
                2,
                "pump {kind} exceeded its attempt budget"
            );
        }
    }

    #[test]
    fn unsuccessful_body_passes_continue_below_the_previous_window() {
        let (mut server, mut store, account) = synced(6);
        server.bodies.clear();
        backfill_bodies(&mut server, &mut store, account, "INBOX", 0, 2).unwrap();
        let previous: HashSet<_> = server.body_batches.iter().flatten().copied().collect();
        server.body_batches.clear();
        backfill_bodies(&mut server, &mut store, account, "INBOX", 0, 2).unwrap();
        assert!(
            server
                .body_batches
                .iter()
                .flatten()
                .all(|uid| !previous.contains(uid)),
            "the next pass repeated the same missing messages"
        );
    }

    #[test]
    fn a_narrower_horizon_does_not_resume_an_out_of_scope_date() {
        let (_, store, account) = synced(4);
        store
            .conn()
            .execute_batch("UPDATE envelopes SET date_epoch = 1")
            .unwrap();
        let identity = store.mailbox_identity(account, "INBOX").unwrap().unwrap();
        assert_eq!(
            store
                .claim_backfill_window(&identity, Kind::Bodies, 0, 2)
                .unwrap()
                .uids,
            [4, 3]
        );
        let next = store
            .claim_backfill_window(&identity, Kind::Bodies, 2, 2)
            .unwrap();
        assert!(next.uids.is_empty());
        assert!(next.at_end);
    }

    #[test]
    fn cursor_scans_cached_prefixes_null_dates_and_wraps_without_losing_work() {
        let (_, store, account) = synced(6);
        store.conn().execute_batch("UPDATE envelopes SET date_epoch = CASE WHEN uid >= 3 THEN 10 WHEN uid = 2 THEN -9223372036854775808 ELSE NULL END").unwrap();
        let identity = store.mailbox_identity(account, "INBOX").unwrap().unwrap();
        store
            .save_body(identity.mailbox_id, 6, "cached", &[])
            .unwrap();
        store
            .save_body(identity.mailbox_id, 5, "cached", &[])
            .unwrap();
        let first = store
            .claim_backfill_window(&identity, Kind::Bodies, NO_HORIZON, 2)
            .unwrap();
        assert_eq!(first.scanned, 2);
        assert!(first.uids.is_empty());
        assert!(!first.at_end);
        assert_eq!(
            store
                .claim_backfill_window(&identity, Kind::Bodies, NO_HORIZON, 2)
                .unwrap()
                .uids,
            [4, 3]
        );
        assert_eq!(
            store
                .claim_backfill_window(&identity, Kind::Bodies, NO_HORIZON, 3)
                .unwrap()
                .uids,
            [2, 1]
        );
        let wrapped = store
            .claim_backfill_window(&identity, Kind::Bodies, NO_HORIZON, 6)
            .unwrap();
        assert_eq!(wrapped.uids, [4, 3, 2, 1]);
    }

    #[test]
    fn failed_fetches_do_not_refund_the_shared_candidate_allowance() {
        let (mut server, mut store, account) = synced(6);
        server.oversized_body = Some(6);
        let mut budget = WorkBudget::new(2);
        assert!(
            backfill_bodies_budgeted(&mut server, &mut store, account, "INBOX", 0, &mut budget)
                .is_err()
        );
        assert_eq!(budget.remaining(), 0);
        assert_eq!(
            backfill_bodies_budgeted(&mut server, &mut store, account, "INBOX", 0, &mut budget)
                .unwrap()
                .scanned,
            0
        );
    }

    #[test]
    fn arrival_previews_do_not_follow_the_historical_cursor() {
        let (mut server, mut store, account) = synced(6);
        backfill_bodies(&mut server, &mut store, account, "INBOX", 0, 2).unwrap();
        server.add_with_body(7, "new arrival", "<p>new preview</p>");
        crate::cycle::poll_inbox(
            &mut server,
            &mut store,
            account,
            &crate::cycle::NoHooks,
            &mut Vec::new(),
        )
        .unwrap();
        assert!(store.body(account, "INBOX", 7).unwrap().is_some());
    }

    #[test]
    fn a_size_refusal_leaves_the_next_pass_free_to_fetch_other_bodies() {
        let (mut server, mut store, account) = synced(3);
        server.oversized_body = Some(3);
        assert!(backfill_bodies(&mut server, &mut store, account, "INBOX", 0, 100).is_err());
        let report = backfill_bodies(&mut server, &mut store, account, "INBOX", 0, 100).unwrap();
        assert_eq!(report.fetched, 2);
        assert_eq!(
            report.remaining, 1,
            "refused content must not count as downloaded"
        );
        assert!(
            store
                .bodies_to_backfill(account, "INBOX", 0, 100)
                .unwrap()
                .is_empty()
        );
        assert_eq!(server.body_batches.last().unwrap().len(), 2);
        assert!(store.body(account, "INBOX", 3).unwrap().is_none());
        let identity = store.mailbox_identity(account, "INBOX").unwrap().unwrap();
        let requests = server.body_batches.len();
        assert!(matches!(
            crate::load_body_version(&mut server, &mut store, &identity, 3),
            Err(Error::RemoteMessageTooLarge { uid: 3, .. })
        ));
        assert_eq!(
            server.body_batches.len(),
            requests,
            "known refusals should work offline"
        );
        store.reset_mailbox(identity.mailbox_id, 2).unwrap();
        server.uid_validity = 2;
        server.oversized_body = None;
        crate::SyncEngine::default()
            .sync(&mut server, &mut store, account, "INBOX")
            .unwrap();
        assert_eq!(
            backfill_bodies(&mut server, &mut store, account, "INBOX", 0, 100)
                .unwrap()
                .fetched,
            3
        );
    }

    #[test]
    fn a_size_refusal_during_namespace_replacement_is_not_persisted() {
        let (mut server, mut store, account) = synced(1);
        server.oversized_body = Some(1);
        server.reset_during_body_fetch = true;
        assert!(matches!(
            backfill_bodies(&mut server, &mut store, account, "INBOX", 0, 10),
            Err(Error::StaleMailbox)
        ));
        assert_eq!(
            store.bodies_to_backfill(account, "INBOX", 0, 10).unwrap(),
            [1]
        );
    }

    /// The pump's reason to exist: after it runs, a word from the BODY
    /// becomes findable — which was impossible before.
    #[test]
    fn body_backfill_refuses_generation_changes_and_unsolicited_uids() {
        for during_fetch in [false, true] {
            let (mut server, mut store, account) = synced(2);
            if during_fetch {
                server.reset_during_body_fetch = true;
            } else {
                server.uid_validity += 1;
            }
            assert!(backfill_bodies(&mut server, &mut store, account, "INBOX", 0, 2).is_err());
            assert_eq!(store.body(account, "INBOX", 1).unwrap(), None);
            assert_eq!(store.body(account, "INBOX", 2).unwrap(), None);
        }
        let (mut server, mut store, account) = synced(2);
        server.body_reply_uid = Some(99);
        let report = backfill_bodies(&mut server, &mut store, account, "INBOX", 0, 2).unwrap();
        assert_eq!(report.fetched, 0);
        assert_eq!(store.body(account, "INBOX", 99).unwrap(), None);
    }

    #[test]
    fn backfilled_bodies_become_searchable() {
        let (mut server, mut store, account) = synced(3);
        server
            .bodies
            .insert(2, "<p>le contrat de licence</p>".to_string()); // lang:fr
        assert_eq!(store.search("contrat", 10).unwrap().len(), 0);

        let report = backfill_bodies(&mut server, &mut store, account, "INBOX", 0, 100).unwrap();

        assert_eq!(report.fetched, 3);
        assert_eq!(report.remaining, 0);
        assert_eq!(
            store.search("contrat", 10).unwrap().len(),
            1,
            "the fetched body must be indexed"
        );
    }

    /// R4, backfill of sent messages (D2): a sent message already synced
    /// with no recipient in the database (the old schema) receives, after
    /// the pass, the To and Cc that the server's ENVELOPE carried. The
    /// pump CONVERGES: a second pass requests nothing more.
    #[test]
    fn backfill_recipients_fills_sent_messages_without_a_recipient() {
        let (mut server, mut store, account) = synced(3);
        // The full ENVELOPE lives on the server; the database has nothing
        // (legacy state — `synced` writes no recipient).
        server.set_envelope_recipients(2, &["sebastien.monchamps@gmail.com"], &["copie@x.fr"]);
        assert_eq!(
            store.recipients_pending_count(account, "INBOX").unwrap(),
            3,
            "all three sent messages are waiting for their recipients"
        );

        let report = backfill_recipients(&mut server, &mut store, account, "INBOX", 100).unwrap();
        assert_eq!(report.fetched, 3);
        assert_eq!(report.remaining, 0);

        let reread = store.recent(account, "INBOX", 0, 10).unwrap();
        let m2 = reread.iter().find(|e| e.uid == 2).unwrap();
        assert_eq!(m2.to_addrs, vec!["sebastien.monchamps@gmail.com"]);
        assert_eq!(m2.cc_addrs, vec!["copie@x.fr"]);

        // Convergence: the second pass requests nothing more (the message
        // with no recipient now carries the empty marker, not NULL).
        let second = backfill_recipients(&mut server, &mut store, account, "INBOX", 100).unwrap();
        assert_eq!(second.fetched, 0);
        assert_eq!(second.remaining, 0);
    }

    /// The heart of the measured gain: one command for the whole batch,
    /// not one round trip per message.
    #[test]
    fn backfill_groups_its_fetches() {
        let (mut server, mut store, account) = synced(5);

        backfill_bodies(&mut server, &mut store, account, "INBOX", 0, 100).unwrap();

        assert_eq!(
            server.body_batches.len(),
            1,
            "5 bodies must fit in ONE command, not 5"
        );
        assert_eq!(server.body_batches[0].len(), 5);
        assert_eq!(
            server.body_fetches, 0,
            "the one-at-a-time path must not be taken"
        );
    }

    /// The budget bounds the pass: it is what keeps a backfill from
    /// monopolizing the network.
    #[test]
    fn backfill_stops_at_its_budget() {
        let (mut server, mut store, account) = synced(10);

        let report = backfill_bodies(&mut server, &mut store, account, "INBOX", 0, 4).unwrap();

        assert_eq!(report.fetched, 4);
        assert_eq!(report.remaining, 6);
    }

    /// Resuming after an interruption: no cursor to restore, the state is
    /// the database. The second pass continues without redoing the first
    /// one's work.
    #[test]
    fn backfill_resumes_where_it_stopped_without_redoing_work() {
        let (mut server, mut store, account) = synced(6);

        let first = backfill_bodies(&mut server, &mut store, account, "INBOX", 0, 2).unwrap();
        let second = backfill_bodies(&mut server, &mut store, account, "INBOX", 0, 2).unwrap();

        assert_eq!(first.fetched, 2);
        assert_eq!(second.fetched, 2);
        assert_eq!(second.remaining, 2);
        // The two passes requested DIFFERENT UIDs.
        let requested: Vec<Uid> = server.body_batches.concat();
        let unique: HashSet<Uid> = requested.iter().copied().collect();
        assert_eq!(
            requested.len(),
            unique.len(),
            "no body must be requested twice"
        );
    }

    /// Newest first: that is where search has the most value, and it makes
    /// an interrupted backfill useful anyway.
    #[test]
    fn backfill_starts_with_the_newest() {
        let (mut server, mut store, account) = synced(5);

        backfill_bodies(&mut server, &mut store, account, "INBOX", 0, 2).unwrap();

        assert_eq!(server.body_batches[0], vec![5, 4]);
    }

    /// THE trap: a message the server no longer serves stays forever in
    /// the list of missing ones. Without a memory of attempts, the pump
    /// would run forever.
    #[test]
    fn backfill_does_not_loop_on_a_body_the_server_never_returns() {
        let (mut server, mut store, account) = synced(3);
        server.bodies.remove(&2); // the envelope exists, the body does not

        let report = backfill_bodies(&mut server, &mut store, account, "INBOX", 0, 100).unwrap();

        assert_eq!(report.fetched, 2, "the two bodies served");
        assert_eq!(
            report.remaining, 1,
            "the silent one is still counted as missing"
        );
    }

    /// The horizon bounds the cost: beyond it, nothing is fetched.
    #[test]
    fn backfill_ignores_what_lies_beyond_the_horizon() {
        let (mut server, mut store, account) = synced(4);
        // FakeServer dates messages at 1_700_000_000 + uid.
        let horizon = 1_700_000_000 + 3;

        let report =
            backfill_bodies(&mut server, &mut store, account, "INBOX", horizon, 100).unwrap();

        assert_eq!(
            report.fetched, 2,
            "only UIDs 3 and 4 are within the horizon"
        );
        assert_eq!(report.remaining, 0);
    }

    #[test]
    fn backfill_on_a_never_synced_mailbox_does_nothing() {
        let mut server = FakeServer::new(false);
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("test@exemple.fr", "gmail")
            .unwrap();

        let report = backfill_bodies(&mut server, &mut store, account, "INBOX", 0, 100).unwrap();

        assert_eq!(report.fetched, 0);
        assert!(server.body_batches.is_empty());
    }

    /// Fixture for the header backfill: two messages from the same
    /// exchange, but the middle message — the one that would have been
    /// sent — is not in the mailbox. Nothing links them until `References`
    /// is read.
    fn cut_exchange() -> (FakeServer, Store, i64) {
        let mut server = FakeServer::new(false);
        server.add(1, "Quote");
        server.add(3, "Re: Quote");
        server.set_references(3, "<fake-1@example.com> <fake-2@example.com>");

        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("test@exemple.fr", "gmail")
            .unwrap();
        crate::SyncEngine::default()
            .sync(&mut server, &mut store, account, "INBOX")
            .unwrap();
        (server, store, account)
    }

    fn conversations(store: &Store) -> usize {
        store.unified_recent(0, 50).unwrap().len()
    }

    /// The pass's reason to exist, in one assertion: two rows before, one
    /// after.
    #[test]
    fn backfilled_headers_reglue_a_cut_thread() {
        let (mut server, mut store, account) = cut_exchange();
        assert_eq!(conversations(&store), 2, "missing the link");

        let report =
            backfill_thread_headers(&mut server, &mut store, account, "INBOX", 0, 100).unwrap();

        assert_eq!(report.fetched, 2);
        assert_eq!(report.remaining, 0);
        assert_eq!(conversations(&store), 1, "the exchange is reconstructed");
    }

    /// A message WITHOUT `References` must leave the list of missing ones
    /// for good. Otherwise the pass would request it again on every sync,
    /// forever.
    #[test]
    fn a_message_without_references_is_not_requested_again() {
        let (mut server, mut store, account) = cut_exchange();
        backfill_thread_headers(&mut server, &mut store, account, "INBOX", 0, 100).unwrap();

        let again =
            backfill_thread_headers(&mut server, &mut store, account, "INBOX", 0, 100).unwrap();

        assert_eq!(again.fetched, 0, "nothing left to read");
        assert_eq!(server.header_batches.len(), 1, "no second round trip");
    }

    /// Batched, like bodies: a round trip per message would make the pass
    /// untenable on a full mailbox.
    #[test]
    fn the_pass_requests_headers_in_batches() {
        let mut server = FakeServer::new(false);
        for uid in 1..=5 {
            server.add(uid, &format!("subject {uid}"));
        }
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("test@exemple.fr", "gmail")
            .unwrap();
        crate::SyncEngine::default()
            .sync(&mut server, &mut store, account, "INBOX")
            .unwrap();

        backfill_thread_headers(&mut server, &mut store, account, "INBOX", 0, 100).unwrap();

        assert_eq!(server.header_batches, vec![vec![5, 4, 3, 2, 1]]);
    }

    /// Bounded: an exhausted budget leaves the rest for the next pass, and
    /// the report says so.
    #[test]
    fn the_budget_bounds_a_pass_and_the_rest_is_reported() {
        let mut server = FakeServer::new(false);
        for uid in 1..=5 {
            server.add(uid, &format!("subject {uid}"));
        }
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("test@exemple.fr", "gmail")
            .unwrap();
        crate::SyncEngine::default()
            .sync(&mut server, &mut store, account, "INBOX")
            .unwrap();

        let report =
            backfill_thread_headers(&mut server, &mut store, account, "INBOX", 0, 2).unwrap();

        assert_eq!(report.fetched, 2);
        assert_eq!(report.remaining, 3);
    }
}

#[cfg(test)]
mod review_regressions {
    //! Fresh-eyes review of Lot 3 (2026-09-07): maintained regressions for the
    //! confirmed findings on diagnostics, arrivals and budgets.
    use super::*;
    use crate::test_support::FakeServer;

    fn synced(n: u32) -> (FakeServer, Store, i64) {
        let mut server = FakeServer::new(false);
        for uid in 1..=n {
            server.add_with_body(
                uid,
                &format!("subject {uid}"),
                &format!("<p>body {uid}</p>"),
            );
        }
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("review@example.test", "imap")
            .unwrap();
        crate::SyncEngine::default()
            .sync(&mut server, &mut store, account, "INBOX")
            .unwrap();
        (server, store, account)
    }

    #[test]
    fn content_the_server_no_longer_holds_is_missing_not_a_failure() {
        let (mut server, mut store, account) = synced(3);
        for uid in 1..=3 {
            server.expunge(uid);
        }
        let report =
            backfill_thread_headers(&mut server, &mut store, account, "INBOX", NO_HORIZON, 10)
                .unwrap();
        assert_eq!(report.fetched, 0);
        assert_eq!(report.remaining, 3, "still counted as missing");
        assert!(
            store.operation_issues().unwrap().is_empty(),
            "an omitted answer is not an operation failure"
        );
    }

    #[test]
    fn an_error_free_sweep_settles_an_earlier_diagnostic() {
        let (mut server, mut store, account) = synced(2);
        store
            .settle_operation(
                account,
                "INBOX",
                "bodies",
                0,
                Some(&Error::Server("earlier outage".into())),
            )
            .unwrap();
        store.retry_operations(account).unwrap();
        backfill_bodies(&mut server, &mut store, account, "INBOX", 0, 10).unwrap();
        assert!(store.operation_issues().unwrap().is_empty());
    }

    #[test]
    fn an_oversized_arrival_keeps_its_marker_without_an_operation_failure() {
        let (mut server, mut store, account) = synced(2);
        server.add_with_body(3, "huge", "<p>huge</p>");
        server.oversized_body = Some(3);
        let mut problems = Vec::new();
        crate::cycle::poll_inbox(
            &mut server,
            &mut store,
            account,
            &crate::cycle::NoHooks,
            &mut problems,
        )
        .unwrap();
        let identity = store.mailbox_identity(account, "INBOX").unwrap().unwrap();
        assert!(store.body_download_limit(&identity, 3).unwrap().is_some());
        assert!(
            store.operation_issues().unwrap().is_empty(),
            "the per-UID marker is the whole diagnostic"
        );
    }

    #[test]
    fn arrivals_are_served_before_the_historical_sweep_resumes() {
        let (mut server, mut store, account) = synced(6);
        assert_eq!(
            backfill_bodies(&mut server, &mut store, account, "INBOX", 0, 2)
                .unwrap()
                .fetched,
            2
        );
        for uid in 7..=9 {
            server.add_with_body(uid, "arrival", "<p>arrival</p>");
        }
        crate::SyncEngine::default()
            .sync(&mut server, &mut store, account, "INBOX")
            .unwrap();
        let report = backfill_bodies(&mut server, &mut store, account, "INBOX", 0, 2).unwrap();
        assert_eq!(report.fetched, 2);
        assert!(store.body(account, "INBOX", 7).unwrap().is_some());
        assert!(store.body(account, "INBOX", 8).unwrap().is_some());
        assert!(store.body(account, "INBOX", 4).unwrap().is_none());
        let report = backfill_bodies(&mut server, &mut store, account, "INBOX", 0, 2).unwrap();
        assert_eq!(report.fetched, 2);
        assert!(store.body(account, "INBOX", 9).unwrap().is_some());
        assert!(store.body(account, "INBOX", 4).unwrap().is_some());
        assert!(store.body(account, "INBOX", 3).unwrap().is_none());
    }

    #[test]
    fn a_shared_allowance_below_one_maximal_message_admits_nothing() {
        let budget = WorkBudget::with_limits(
            5,
            crate::REMOTE_MESSAGE_BYTES - 1,
            std::time::Duration::from_secs(60),
        );
        assert_eq!(budget.remaining(), 0);
        let budget = WorkBudget::with_limits(
            5,
            crate::REMOTE_MESSAGE_BYTES,
            std::time::Duration::from_secs(60),
        );
        assert_eq!(budget.remaining(), 5);
    }
}

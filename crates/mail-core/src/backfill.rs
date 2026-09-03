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
//! Three properties define it:
//!
//! - **bounded**: a recency horizon and a budget per pass, so the cost stays
//!   predictable (< 1 GB, PLAN.md §1);
//! - **resumable**: it holds no cursor — the state is the database. A body
//!   already written falls out of the list of missing ones, so an
//!   interruption only costs the batch in progress;
//! - **batched**: a round trip per message costs ~192 ms on a real server
//!   (`spikes/body-backfill`). Bodies are requested in batches.
//!
//! [ADR 0007]: ../../../docs/adr/0007-body-backfill.md

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
}

/// Fetches up to `budget` missing bodies, from newest to oldest, and
/// indexes them along the way (it is [`Store::save_body`] that handles
/// that, inside its transaction).
///
/// `since_epoch` is the horizon: beyond it, nothing is fetched.
pub fn backfill_bodies(
    server: &mut dyn MailServer,
    store: &mut Store,
    account_id: i64,
    mailbox: &str,
    since_epoch: i64,
    budget: usize,
) -> Result<BackfillReport, Error> {
    let Some(state) = store.sync_state(account_id, mailbox)? else {
        return Ok(BackfillReport {
            fetched: 0,
            remaining: 0,
        });
    };

    let mut fetched = 0usize;
    // UIDs already attempted during THIS pass. Without this memory, a
    // message the server no longer serves would come back into the list
    // of missing ones on every round — and the pump would run forever.
    let mut attempted: HashSet<Uid> = HashSet::new();

    while fetched < budget {
        let window = (budget - fetched + attempted.len()).min(BACKFILL_BATCH + attempted.len());
        let candidates = store.bodies_to_backfill(account_id, mailbox, since_epoch, window)?;
        let batch: Vec<Uid> = candidates
            .into_iter()
            .filter(|uid| !attempted.contains(uid))
            .take((budget - fetched).min(BACKFILL_BATCH))
            .collect();
        if batch.is_empty() {
            break;
        }
        attempted.extend(batch.iter().copied());

        for (uid, body) in server.fetch_bodies_html(mailbox, &batch)? {
            let invitation = crate::body::invitation_from(store, account_id, body.ics.as_deref())?;
            store.save_body_full(
                state.mailbox_id,
                uid,
                &body.html,
                &body.attachments,
                invitation.as_ref(),
            )?;
            fetched += 1;
        }
    }

    Ok(BackfillReport {
        fetched,
        remaining: store.bodies_pending_count(account_id, mailbox, since_epoch)?,
    })
}

/// Headers requested in one command. Much more than bodies: a header block
/// weighs ~3 KB against ~50 KB for a whole message, and the expense that
/// matters here is the round trip, not the bytes.
pub const THREAD_HEADER_BATCH: usize = 200;

/// Fetches the missing recipients (To/Cc) of a Sent folder, from newest to
/// oldest (R4, backfill of sent messages — D2, PLAN-RETOURS-MAIL).
///
/// Same bounded/resumable/batched shape as [`backfill_bodies`], but it
/// rereads the ENVELOPE — where To/Cc travel for free with the sender,
/// without a single byte of body — and writes ONLY those two columns
/// (never the thread nor `refs`). In a Sent folder the sender is ONESELF:
/// without the recipient, neither the list nor the reading pane can say
/// who the message went to. Once the header pass has converged, already
/// synced sent messages have no recipient in the database — this is the
/// pump that catches them up.
pub fn backfill_recipients(
    server: &mut dyn MailServer,
    store: &mut Store,
    account_id: i64,
    mailbox: &str,
    budget: usize,
) -> Result<BackfillReport, Error> {
    let Some(state) = store.sync_state(account_id, mailbox)? else {
        return Ok(BackfillReport {
            fetched: 0,
            remaining: 0,
        });
    };

    let mut fetched = 0usize;
    // UIDs already attempted during THIS pass — a message the server no
    // longer serves must not make the pump run forever (same guard as
    // [`backfill_bodies`]).
    let mut attempted: HashSet<Uid> = HashSet::new();

    while fetched < budget {
        let window =
            (budget - fetched + attempted.len()).min(THREAD_HEADER_BATCH + attempted.len());
        let candidates = store.recipients_to_backfill(account_id, mailbox, window)?;
        let batch: Vec<Uid> = candidates
            .into_iter()
            .filter(|uid| !attempted.contains(uid))
            .take((budget - fetched).min(THREAD_HEADER_BATCH))
            .collect();
        if batch.is_empty() {
            break;
        }
        attempted.extend(batch.iter().copied());

        for envelope in server.fetch_envelopes(mailbox, &batch)? {
            store.set_recipients(
                state.mailbox_id,
                envelope.uid,
                &envelope.to_addrs,
                &envelope.cc_addrs,
            )?;
            fetched += 1;
        }
    }

    Ok(BackfillReport {
        fetched,
        remaining: store.recipients_pending_count(account_id, mailbox)?,
    })
}

/// Fetches the missing thread headers, from newest to oldest, and reglues
/// the conversations along the way.
///
/// Same shape as [`backfill_bodies`] — bounded, resumable, batched — but a
/// different reason to exist: this one does not complete search, it repairs
/// GROUPING. A message whose `References` were never read stays alone in
/// its thread even though it belongs to an exchange.
pub fn backfill_thread_headers(
    server: &mut dyn MailServer,
    store: &mut Store,
    account_id: i64,
    mailbox: &str,
    since_epoch: i64,
    budget: usize,
) -> Result<BackfillReport, Error> {
    let Some(state) = store.sync_state(account_id, mailbox)? else {
        return Ok(BackfillReport {
            fetched: 0,
            remaining: 0,
        });
    };

    let mut fetched = 0usize;
    // Same guard as for bodies: a message the server no longer serves
    // would otherwise come back on every round, and the pump would run
    // forever.
    let mut attempted: HashSet<Uid> = HashSet::new();

    while fetched < budget {
        let window =
            (budget - fetched + attempted.len()).min(THREAD_HEADER_BATCH + attempted.len());
        let candidates =
            store.thread_headers_to_backfill(account_id, mailbox, since_epoch, window)?;
        let batch: Vec<Uid> = candidates
            .into_iter()
            .filter(|uid| !attempted.contains(uid))
            .take((budget - fetched).min(THREAD_HEADER_BATCH))
            .collect();
        if batch.is_empty() {
            break;
        }
        attempted.extend(batch.iter().copied());

        for (uid, headers) in server.fetch_thread_headers(mailbox, &batch)? {
            store.set_thread_headers(
                state.mailbox_id,
                uid,
                headers.in_reply_to.as_deref(),
                headers.references.as_deref().unwrap_or_default(),
            )?;
            fetched += 1;
        }
    }

    Ok(BackfillReport {
        fetched,
        remaining: store.thread_headers_pending_count(account_id, mailbox, since_epoch)?,
    })
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

    /// The pump's reason to exist: after it runs, a word from the BODY
    /// becomes findable — which was impossible before.
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

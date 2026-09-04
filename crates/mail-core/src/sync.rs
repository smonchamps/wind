//! The "envelopes first" synchronization engine.
//!
//! Protocol (frozen decisions, PHASE0.md §2):
//! - initial sync from **newest to oldest**, in batches — the list becomes
//!   usable from the first batch;
//! - incremental sync: CONDSTORE when the server exposes it (new messages +
//!   flag changes), otherwise a UID diff for new messages; deletions always
//!   go through the diff;
//! - a UIDVALIDITY change triggers a full resynchronization.

use std::collections::HashSet;

use crate::action::Action;
use crate::envelope::Uid;
use crate::error::Error;
use crate::remote::{MailServer, MailboxSnapshot};
use crate::store::{Store, SyncState};

const DEFAULT_BATCH_SIZE: usize = 500;

/// What a pass must do, decided BEFORE any write I/O (STANDARD §4: the
/// pure decision, the execution elsewhere). Audit 2026-09-01 S1-6: the
/// decision used to read `last_uid == 0`, and an EMPTIED mailbox went
/// back to "initial" — silent (no bubble) and expensive (full inventory).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SyncPlan {
    /// UIDVALIDITY changed: everything we know about the mailbox is wrong.
    Reset,
    /// Unknown mailbox, or known but never synchronized to completion.
    Initial,
    /// Mailbox already initialized: CONDSTORE delta if `modseq`, otherwise
    /// UID diff.
    Incremental { modseq: Option<u64> },
}

/// The decision, on the FRESHNESS of the state — never on the largest UID
/// in the database.
pub(crate) fn plan_sync(state: Option<&SyncState>, snapshot: &MailboxSnapshot) -> SyncPlan {
    match state {
        None => SyncPlan::Initial,
        Some(state) if state.uid_validity != snapshot.uid_validity => SyncPlan::Reset,
        Some(state) if !state.initialized => SyncPlan::Initial,
        Some(state) => SyncPlan::Incremental {
            modseq: state.highest_modseq,
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncMode {
    Initial,
    Incremental,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncReport {
    pub mode: SyncMode,
    /// Envelopes fetched or updated (new messages + flags).
    pub fetched: usize,
    /// Local envelopes removed because they vanished from the server.
    pub deleted: usize,
    /// Local actions replayed to the server at the head of the sync.
    pub replayed: usize,
    /// Actions put into QUARANTINE during this replay (E3): a definitive
    /// refusal from the server, or a fifth transient failure.
    pub refused: usize,
    /// The server does not announce CONDSTORE: its flags never
    /// resynchronize (D-51, CE decision D3 of PLAN-AUDIT-V2 — declared
    /// debt, tracked by the shell).
    pub without_condstore: bool,
}

pub struct SyncEngine {
    batch_size: usize,
}

impl Default for SyncEngine {
    fn default() -> Self {
        Self {
            batch_size: DEFAULT_BATCH_SIZE,
        }
    }
}

impl SyncEngine {
    pub fn new(batch_size: usize) -> Self {
        Self {
            batch_size: batch_size.max(1),
        }
    }

    pub fn sync(
        &self,
        server: &mut dyn MailServer,
        store: &mut Store,
        account_id: i64,
        mailbox: &str,
    ) -> Result<SyncReport, Error> {
        let snapshot = server.select(mailbox)?;
        let without_condstore = snapshot.highest_modseq.is_none();

        // The DECISION is pure (`plan_sync`, STANDARD §4); here we only
        // execute it.
        let known = store.sync_state(account_id, mailbox)?;
        let plan = plan_sync(known.as_ref(), &snapshot);
        let state = match (plan, known) {
            (SyncPlan::Reset, Some(stale)) => {
                store.reset_mailbox(stale.mailbox_id, snapshot.uid_validity)?;
                SyncState {
                    uid_validity: snapshot.uid_validity,
                    last_uid: 0,
                    highest_modseq: None,
                    initialized: false,
                    ..stale
                }
            }
            (_, Some(state)) => state,
            (_, None) => {
                let mailbox_id =
                    store.create_mailbox(account_id, mailbox, snapshot.uid_validity)?;
                SyncState {
                    mailbox_id,
                    uid_validity: snapshot.uid_validity,
                    last_uid: 0,
                    highest_modseq: None,
                    initialized: false,
                }
            }
        };

        // What the server announces, polled at EVERY pass: this is the
        // denominator of the progress (ADR 0010 §5). Polled here and not
        // when the mailbox is created, otherwise it would freeze the
        // first day's value and progress would drift as mail arrives.
        store.record_remote_total(state.mailbox_id, snapshot.exists)?;

        // Local intentions first: the sync that follows thus reflects
        // their effect (the replay bumps the modseq server-side).
        let (replayed, refused) = replay_actions(server, store, mailbox, state.mailbox_id)?;

        let mut report = match plan {
            SyncPlan::Incremental { .. } => {
                self.incremental_sync(server, store, mailbox, &state, snapshot.exists)?
            }
            SyncPlan::Initial | SyncPlan::Reset => {
                self.initial_sync(server, store, mailbox, state.mailbox_id)?
            }
        };
        report.replayed = replayed;
        report.refused = refused;

        let last_uid = store.max_uid(state.mailbox_id)?;
        store.update_state(state.mailbox_id, last_uid, snapshot.highest_modseq)?;

        // The folder list is NO LONGER refreshed here: every poll used to
        // pay for an identical LIST — ~51 per account and per cycle on the
        // 2026-08-13 field data (ADR 0017). The orchestrator refreshes it
        // ONCE per cycle, at inventory time, with the list it already has
        // in hand — offline moves stay served.
        Ok(SyncReport {
            without_condstore,
            ..report
        })
    }

    fn initial_sync(
        &self,
        server: &mut dyn MailServer,
        store: &mut Store,
        mailbox: &str,
        mailbox_id: i64,
    ) -> Result<SyncReport, Error> {
        let mut uids = server.list_uids(mailbox)?;
        // Resumable (PLAN-AUDIT-V2 E5): an initial sync cut off at batch k
        // (throttling, disconnect) used to start over from scratch. What
        // is already in the database is no longer requested again — the
        // pass resumes where it stopped.
        let known = store.known_uids(mailbox_id)?;
        uids.retain(|uid| !known.contains(uid));
        uids.sort_unstable_by(|a, b| b.cmp(a));

        let mut fetched = 0;
        for chunk in uids.chunks(self.batch_size) {
            let envelopes = server.fetch_envelopes(mailbox, chunk)?;
            fetched += envelopes.len();
            store.upsert_envelopes(mailbox_id, &envelopes)?;
        }
        Ok(SyncReport {
            mode: SyncMode::Initial,
            fetched,
            deleted: 0,
            replayed: 0,
            refused: 0,
            without_condstore: false,
        })
    }

    fn incremental_sync(
        &self,
        server: &mut dyn MailServer,
        store: &mut Store,
        mailbox: &str,
        state: &SyncState,
        exists: u32,
    ) -> Result<SyncReport, Error> {
        let mut fetched = 0;
        let mut deleted = 0;

        let condstore_changes = match state.highest_modseq {
            Some(modseq) => server.changes_since(mailbox, modseq)?,
            None => None,
        };
        match condstore_changes {
            Some(changed) => {
                fetched += changed.len();
                store.upsert_envelopes(state.mailbox_id, &changed)?;
                // CONDSTORE does not signal deletions (that would need
                // QRESYNC, absent from Gmail): the UID diff remains the
                // only way to detect them — but it is only paid for WHEN
                // the count requires it (E2b). Delta applied, database
                // and announcement agree: nothing has vanished, and the
                // full inventory (`UID SEARCH ALL`, 34 s on the field
                // INBOX) would have nothing to say.
                let local = store.envelope_count(state.mailbox_id)?;
                if local != u64::from(exists) {
                    let present: HashSet<Uid> = server.list_uids(mailbox)?.into_iter().collect();
                    deleted = store.remove_absent(state.mailbox_id, &present)?;
                }
            }
            None => {
                // Without CONDSTORE: only new messages are detected; flag
                // changes will wait for a full resync.
                let server_uids = server.list_uids(mailbox)?;
                let mut new_uids: Vec<Uid> = server_uids
                    .iter()
                    .copied()
                    .filter(|uid| *uid > state.last_uid)
                    .collect();
                new_uids.sort_unstable_by(|a, b| b.cmp(a));
                for chunk in new_uids.chunks(self.batch_size) {
                    let envelopes = server.fetch_envelopes(mailbox, chunk)?;
                    fetched += envelopes.len();
                    store.upsert_envelopes(state.mailbox_id, &envelopes)?;
                }
                // Here the inventory is already paid for (it served the
                // new messages): the deletion diff is free.
                let present: HashSet<Uid> = server_uids.into_iter().collect();
                deleted = store.remove_absent(state.mailbox_id, &present)?;
            }
        }

        Ok(SyncReport {
            mode: SyncMode::Incremental,
            fetched,
            deleted,
            replayed: 0,
            refused: 0,
            without_condstore: false,
        })
    }
}

/// Replays the action queue to the server, in emission order.
/// Returns (replayed, put into quarantine).
///
/// A TRANSIENT failure (network, `Error::Server`) stops the replay and the
/// rest of the queue survives for the next sync — on the fifth consecutive
/// failure of the SAME action, it enters quarantine and frees the queue. A
/// definitive REFUSAL (`Error::Refusal`, NO/BAD) puts the action into
/// quarantine on the spot and the replay CONTINUES: before E3, a folder
/// that had vanished server-side blocked the whole mailbox forever, in
/// silence (audit 2026-09-01 S1-7).
fn replay_actions(
    server: &mut dyn MailServer,
    store: &mut Store,
    mailbox: &str,
    mailbox_id: i64,
) -> Result<(usize, usize), Error> {
    let mut replayed = 0;
    let mut refused = 0;
    for pending in store.pending_actions(mailbox_id)? {
        let outcome = match &pending.action {
            Action::MarkSeen => server.set_seen(mailbox, pending.uid, true),
            Action::MarkUnseen => server.set_seen(mailbox, pending.uid, false),
            Action::MarkFlagged => server.set_flagged(mailbox, pending.uid, true),
            Action::MarkUnflagged => server.set_flagged(mailbox, pending.uid, false),
            Action::Archive => server.archive(mailbox, pending.uid),
            Action::Delete => server.delete(mailbox, pending.uid),
            Action::MoveTo(target) => server.move_to(mailbox, pending.uid, target),
        };
        match outcome {
            Ok(()) => {
                store.remove_action(pending.id)?;
                replayed += 1;
            }
            Err(Error::Refusal(reason)) => {
                store.refuse_action(pending.id, &reason)?;
                refused += 1;
            }
            Err(err) => {
                if store.note_action_failure(pending.id, &err.to_string())? {
                    refused += 1;
                }
                break;
            }
        }
    }
    Ok((replayed, refused))
}

/// In which ORDER to synchronize an account's mailboxes — a pure decision,
/// no I/O, testable against the quirks of real servers.
///
/// Since [ADR 0010] we synchronize **everything**, with no exception:
/// archive, trash and spam included. The order is therefore no longer a
/// detail — it decides what the user sees first.
///
/// 1. **INBOX first, always.** It is the only mailbox the list displays:
///    running it after an 80,000-message archive folder would leave an
///    empty screen throughout the whole first synchronization.
/// 2. **"Sent" next**, because it is the one that completes threads
///    ([ADR 0009]) — nothing else is ever grouped.
/// 3. The rest in the server's order.
///
/// Non-selectable folders are excluded: they are containers with no mail
/// (`\Noselect`), and SELECT would fail on each of them one by one.
///
/// [ADR 0009]: ../../../docs/adr/0009-thread-scope-per-account.md
/// [ADR 0010]: ../../../docs/adr/0010-full-synchronization.md
pub fn sync_order(folders: &[crate::remote::Folder], sent: Option<&str>) -> Vec<String> {
    let mut order: Vec<String> = Vec::with_capacity(folders.len() + 1);
    // INBOX even if the server does not list it: it always exists, and an
    // inbox missing from the list is a known quirk of servers that treat
    // it separately.
    order.push(crate::thread::RECEIVED_MAILBOX.to_string());
    if let Some(sent) = sent.filter(|sent| *sent != crate::thread::RECEIVED_MAILBOX) {
        order.push(sent.to_string());
    }
    for folder in folders {
        if folder.selectable && !order.iter().any(|already| already == &folder.wire) {
            order.push(folder.wire.clone());
        }
    }
    order
}

/// Estimated disk cost of ONE message, all included — envelope, index,
/// body, attachments.
///
/// Two measurements from the project, not a made-up figure ([ADR 0010]
/// §4): ~49 KB per body (137 MB for 2,801 messages, a full backfill of
/// the real mailbox) + ~1.2 KB of envelope and index (derived from
/// `gate3-corps.db`: 778.9 MB for 200,000 envelopes + 16,002 bodies).
///
/// **Deliberately high**: announcing too much and delivering beats
/// starting and failing halfway through.
///
/// [ADR 0010]: ../../../docs/adr/0010-full-synchronization.md
pub const SYNC_BYTES_PER_MESSAGE: u64 = 50 * 1024;

/// The space that WOULD BE MISSING to bring `pending` messages home — a
/// pure decision, the guard from [ADR 0010] §4.
///
/// `None`: it fits, go ahead. `Some(bytes)`: we REFUSE before starting,
/// and the figure serves the message — "1.2 GB missing" is understood,
/// while a bare "insufficient space" leaves the user guessing whether to
/// free up 100 MB or 100 GB.
///
/// No hidden margin on top: the per-message estimate is already high, and
/// two stacked margins end up refusing syncs that would actually fit.
///
/// [ADR 0010]: ../../../docs/adr/0010-full-synchronization.md
pub fn disk_shortfall(pending: u64, available_bytes: u64) -> Option<u64> {
    let needed = pending.saturating_mul(SYNC_BYTES_PER_MESSAGE);
    if needed <= available_bytes {
        None
    } else {
        Some(needed - available_bytes)
    }
}

#[cfg(test)]
mod disk_shortfall_tests {
    use super::{SYNC_BYTES_PER_MESSAGE, disk_shortfall};

    /// Nothing to bring home = nothing to refuse, even on a full disk.
    /// The COMMON case: every incremental sync of an up-to-date mailbox
    /// goes through here, and a guard that blocked them on a well-filled
    /// disk would forbid polling one's mail at all.
    #[test]
    fn nothing_to_bring_home_passes_even_on_a_full_disk() {
        assert_eq!(disk_shortfall(0, 0), None);
        assert_eq!(disk_shortfall(0, u64::MAX), None);
    }

    #[test]
    fn it_fits_exactly() {
        assert_eq!(disk_shortfall(100, 100 * SYNC_BYTES_PER_MESSAGE), None);
    }

    /// The shortfall is QUANTIFIED: that is what makes the refusal
    /// actionable.
    #[test]
    fn the_shortfall_is_quantified() {
        assert_eq!(
            disk_shortfall(100, 99 * SYNC_BYTES_PER_MESSAGE),
            Some(SYNC_BYTES_PER_MESSAGE)
        );
    }

    /// 200,000 messages x 50 KB = ~9.8 GB: the product would overflow a
    /// u32, and an even bigger mailbox must not make the guard panic on
    /// overflow — in debug builds, a bare multiplication on u64 panics
    /// instead of wrapping.
    #[test]
    fn a_huge_mailbox_does_not_overflow() {
        assert_eq!(disk_shortfall(u64::MAX, 0), Some(u64::MAX));
    }
}

/// The sync's progress, as a percentage — a pure decision.
///
/// `None` means **"I don't know"**, and it is a result in its own right:
/// as long as no mailbox has been selected, there is no denominator.
/// Showing "0%" would say "I've done nothing", and "100%" would say "I'm
/// done" — two lies. The caller then displays nothing.
///
/// The result is capped at 100: the local count can legitimately exceed
/// the server's announcement — messages deleted server-side between two
/// passes still live in the database until the next diff. A "103%" would
/// cast doubt on the rest of the screen.
///
/// And it never returns 100 while something remains: naive rounding
/// would show "100%" at 19,999 messages out of 20,000, and the user
/// would see a full bar that never finishes.
/// What storage knows about a folder at decision time (ADR 0017).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalMarker {
    pub uid_validity: u32,
    /// The UIDNEXT seen at the poll that preceded the LAST completed
    /// poll — not `last_uid`: a server never lowers its UIDNEXT, whereas
    /// `last_uid` drops when the most recent message is deleted, which
    /// would condemn the folder to never be skipped again.
    pub uidnext_seen: Option<u32>,
    /// Messages in the database for this folder.
    pub local_messages: u64,
    /// Local actions are waiting for their replay: skipping would strand them.
    pub pending_actions: bool,
    /// The HIGHESTMODSEQ seen at the SELECT of the last completed poll
    /// (`sync_state`) — `None` without CONDSTORE, or as long as no poll
    /// has happened since E2b.
    pub modseq_seen: Option<u64>,
}

/// Must this folder be polled, or has nothing moved (ADR 0017)?
///
/// The pure decision behind the "sober cycle" — 2026-08-13 field data:
/// the recurring cycle cost ~38 min on a real mailbox, each folder
/// paying for SELECT + UID SEARCH ALL even when nothing had changed. A
/// per-folder STATUS (already paid for by the space guard) is enough to
/// decide. Any uncertainty — never polled, values withheld by the
/// server, UIDVALIDITY changed, pending actions — polls: sobriety has no
/// right to cost a message.
pub fn must_poll(remote: &crate::remote::FolderStatus, local: Option<&LocalMarker>) -> bool {
    poll_reason(remote, local).is_some()
}

/// WHY this folder polls — `None`: nothing moved, skip. The reason is
/// pure diagnosis (PLAN-AUDIT-V3 field, 2026-09-04: a full resweep in
/// the field could not be NAMED — the cycle now traces which criterion
/// tripped, per folder).
pub fn poll_reason(
    remote: &crate::remote::FolderStatus,
    local: Option<&LocalMarker>,
) -> Option<&'static str> {
    let Some(local) = local else {
        return Some("never polled");
    };
    if local.pending_actions {
        return Some("pending actions");
    }
    let (Some(uid_validity), Some(uid_next)) = (remote.uid_validity, remote.uid_next) else {
        return Some("status withheld");
    };
    if uid_validity != local.uid_validity {
        return Some("uidvalidity changed");
    }
    let Some(uidnext_seen) = local.uidnext_seen else {
        return Some("no uidnext marker");
    };
    if uid_next != uidnext_seen {
        return Some("uidnext moved");
    }
    if u64::from(remote.messages) != local.local_messages {
        return Some("message count drift");
    }
    // E2b: a flag change ALONE moves neither UIDNEXT nor MESSAGES — only
    // HIGHESTMODSEQ betrays it. A signal is required on both sides: a
    // silent server (no CONDSTORE) keeps the pre-E2b behavior (ADR 0017:
    // nothing that was served is lost); a local marker never set (a
    // database from before E2b) polls ONCE — the SELECT of that poll sets
    // the modseq, and the folder becomes sober again.
    match (remote.highest_modseq, local.modseq_seen) {
        (Some(remote), Some(seen)) if remote != seen => Some("modseq moved"),
        (Some(_), None) => Some("no modseq marker"),
        _ => None,
    }
}

#[cfg(test)]
mod must_poll_tests {
    use super::{LocalMarker, must_poll};
    use crate::remote::FolderStatus;

    fn remote() -> FolderStatus {
        FolderStatus {
            messages: 40,
            uid_next: Some(101),
            uid_validity: Some(7),
            highest_modseq: Some(900),
        }
    }
    fn local() -> LocalMarker {
        LocalMarker {
            uid_validity: 7,
            uidnext_seen: Some(101),
            local_messages: 40,
            pending_actions: false,
            modseq_seen: Some(900),
        }
    }

    /// THE case that makes the cycle sober: nothing has moved, we skip.
    #[test]
    fn nothing_has_moved_we_skip() {
        assert!(!must_poll(&remote(), Some(&local())));
    }

    /// Never polled: no basis for comparison, we poll.
    #[test]
    fn a_never_polled_folder_gets_polled() {
        assert!(must_poll(&remote(), None));
    }

    /// An arrival moves UIDNEXT — even if a simultaneous departure leaves
    /// the count identical (the drift that either test alone would miss).
    #[test]
    fn an_arrival_moves_uidnext_even_at_equal_count() {
        let moved = FolderStatus {
            uid_next: Some(102),
            ..remote()
        };
        assert!(must_poll(&moved, Some(&local())));
    }

    /// A deletion lowers MESSAGES without touching UIDNEXT.
    #[test]
    fn a_deletion_lowers_the_count() {
        let cut = FolderStatus {
            messages: 39,
            ..remote()
        };
        assert!(must_poll(&cut, Some(&local())));
    }

    /// UIDVALIDITY changed: local UIDs no longer mean anything — polling
    /// (and its reset) is mandatory, invariant §6.6.
    #[test]
    fn a_changed_uidvalidity_forces_a_poll() {
        let regenerated = FolderStatus {
            uid_validity: Some(8),
            ..remote()
        };
        assert!(must_poll(&regenerated, Some(&local())));
    }

    /// Local actions are waiting for their replay: skipping would strand
    /// them until a hypothetical remote change.
    #[test]
    fn pending_actions_force_a_poll() {
        let loaded = LocalMarker {
            pending_actions: true,
            ..local()
        };
        assert!(must_poll(&remote(), Some(&loaded)));
    }

    /// THE E2b case: mail read on a phone moves neither UIDNEXT nor
    /// MESSAGES — only HIGHESTMODSEQ shifts, and the folder MUST be
    /// polled to reflect the flag.
    #[test]
    fn a_flag_change_alone_wakes_the_folder() {
        let flags = FolderStatus {
            highest_modseq: Some(901),
            ..remote()
        };
        assert!(must_poll(&flags, Some(&local())));
    }

    /// A server WITHOUT CONDSTORE withholds HIGHESTMODSEQ: the pre-E2b
    /// behavior is kept — flags were already not resynchronized (ADR
    /// 0017: nothing that was served is lost), and forcing a poll would
    /// ruin E2a's sobriety for nothing.
    #[test]
    fn a_server_without_condstore_keeps_the_sobriety() {
        let silent = FolderStatus {
            highest_modseq: None,
            ..remote()
        };
        assert!(!must_poll(&silent, Some(&local())));
    }

    /// A database from before E2b: the local modseq was never set while
    /// the server announces one — ONE convergence poll, which sets the
    /// marker, and the folder becomes sober again.
    #[test]
    fn a_never_seen_modseq_polls_once_to_converge() {
        let inherited = LocalMarker {
            modseq_seen: None,
            ..local()
        };
        assert!(must_poll(&remote(), Some(&inherited)));
    }

    /// A server that withholds UIDNEXT or UIDVALIDITY makes the decision
    /// conservative — we poll, we do not guess.
    #[test]
    fn a_silent_server_forces_the_poll() {
        let silent = FolderStatus {
            uid_next: None,
            ..remote()
        };
        assert!(must_poll(&silent, Some(&local())));
        let without_validity = FolderStatus {
            uid_validity: None,
            ..remote()
        };
        assert!(must_poll(&without_validity, Some(&local())));
        let never_seen = LocalMarker {
            uidnext_seen: None,
            ..local()
        };
        assert!(must_poll(&remote(), Some(&never_seen)));
    }
}

pub fn sync_percent(local: u64, remote: u64) -> Option<u8> {
    if remote == 0 {
        return None;
    }
    if local >= remote {
        return Some(100);
    }
    let percent = (local * 100 / remote) as u8;
    Some(percent.min(99))
}

#[cfg(test)]
mod sync_percent_tests {
    use super::sync_percent;

    /// Without a denominator, we say nothing — especially not "0%", which
    /// would be indistinguishable from a sync that is not progressing.
    #[test]
    fn without_a_denominator_we_say_nothing() {
        assert_eq!(sync_percent(0, 0), None);
        assert_eq!(sync_percent(42, 0), None);
    }

    #[test]
    fn the_common_case() {
        assert_eq!(sync_percent(0, 200), Some(0));
        assert_eq!(sync_percent(50, 200), Some(25));
        assert_eq!(sync_percent(200, 200), Some(100));
    }

    /// The local count exceeds the server's announcement as soon as a
    /// message is deleted there between two passes: it still lives in
    /// the database until the next diff. "103%" would cast doubt on the
    /// rest of the screen.
    #[test]
    fn a_local_count_that_exceeds_is_capped() {
        assert_eq!(sync_percent(210, 200), Some(100));
    }

    /// THE classic display bug: a full bar that keeps spinning. 19,999
    /// out of 20,000 rounds to 100% — and the user concludes the app is
    /// stuck.
    #[test]
    fn almost_done_is_not_done() {
        assert_eq!(sync_percent(19_999, 20_000), Some(99));
    }
}

#[cfg(test)]
mod sync_order_tests {
    use super::sync_order;
    use crate::remote::Folder;

    fn folder(wire: &str, selectable: bool) -> Folder {
        Folder {
            wire: wire.to_string(),
            display: wire.to_string(),
            selectable,
            special_use: None,
            delimiter: None,
        }
    }

    /// The case that matters: the list shows only INBOX. If a server
    /// announces its folders in alphabetical order, "Archive" comes
    /// first — and the user stares at an empty screen while 80,000
    /// archive messages come down.
    #[test]
    fn inbox_always_comes_first() {
        let folders = [
            folder("Archive", true),
            folder("INBOX", true),
            folder("Spam", true),
        ];
        assert_eq!(sync_order(&folders, None)[0], "INBOX");
    }

    /// "Sent" completes threads (ADR 0009): it comes before the folders
    /// that will never be grouped.
    #[test]
    fn sent_comes_before_the_rest() {
        let folders = [
            folder("Archive", true),
            folder("INBOX", true),
            folder("Sent", true),
        ];
        let order = sync_order(&folders, Some("Sent"));
        assert_eq!(order, vec!["INBOX", "Sent", "Archive"]);
    }

    /// A mailbox synchronized twice is not a benign bug: it is a full
    /// network round trip paid for nothing, on the product's longest
    /// path.
    #[test]
    fn no_mailbox_is_synchronized_twice() {
        let folders = [
            folder("INBOX", true),
            folder("Sent", true),
            folder("Sent", true),
        ];
        let order = sync_order(&folders, Some("Sent"));
        assert_eq!(order.len(), 2, "order obtained: {order:?}");
    }

    /// `\Noselect`: a container that carries no mail. Selecting it
    /// fails — no point trying.
    #[test]
    fn mail_less_containers_are_excluded() {
        let folders = [folder("INBOX", true), folder("[Gmail]", false)];
        assert_eq!(sync_order(&folders, None), vec!["INBOX"]);
    }

    /// Gmail exposes "[Gmail]/Sent Mail" AND INBOX. Some generic servers,
    /// however, list NOTHING — the inbox must still be synchronized
    /// regardless.
    #[test]
    fn a_server_that_lists_nothing_still_syncs_the_inbox() {
        assert_eq!(sync_order(&[], None), vec!["INBOX"]);
    }

    /// A server that designates INBOX as its sent folder (seen on exotic
    /// configurations) must not have it synchronized twice.
    #[test]
    fn a_sent_folder_mistaken_for_the_inbox_does_not_duplicate() {
        assert_eq!(sync_order(&[], Some("INBOX")), vec!["INBOX"]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::FakeServer;

    fn test_account(store: &Store) -> i64 {
        store
            .adopt_or_create_account("test@example.com", "gmail")
            .unwrap()
    }

    fn synced(server: &mut FakeServer, store: &mut Store, engine: &SyncEngine) -> SyncReport {
        let account = test_account(store);
        engine.sync(server, store, account, "INBOX").unwrap()
    }

    fn recent(store: &Store, offset: usize, limit: usize) -> Vec<crate::Envelope> {
        let account = test_account(store);
        store.recent(account, "INBOX", offset, limit).unwrap()
    }

    /// PLAN-AUDIT-V2 E5: an initial sync cut off at batch 2 (Gmail
    /// throttling, disconnect) used to start over from scratch —
    /// `list_uids` then EVERY batch replayed. UIDs already in the
    /// database are removed before chunking: the resume only requests
    /// what is missing.
    #[test]
    fn an_initial_sync_cut_at_batch_2_resumes_at_batch_2() {
        let mut server = FakeServer::new(false);
        for uid in 1..=6 {
            server.add(uid, "message");
        }
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::new(2);
        let account = test_account(&store);

        server.envelope_batch_failure = Some(2);
        assert!(
            engine
                .sync(&mut server, &mut store, account, "INBOX")
                .is_err(),
            "the simulated disconnect must make the pass fail"
        );
        assert_eq!(
            server.fetch_batches,
            vec![vec![6, 5]],
            "only one batch succeeded"
        );

        server.envelope_batch_failure = None;
        server.fetch_batches.clear();
        let resumed = synced(&mut server, &mut store, &engine);
        assert_eq!(
            server.fetch_batches,
            vec![vec![4, 3], vec![2, 1]],
            "the resume only requests UIDs absent from the database"
        );
        assert_eq!(resumed.fetched, 4);
        assert_eq!(recent(&store, 0, 10).len(), 6);
    }

    #[test]
    fn initial_sync_fetches_newest_first_in_batches() {
        let mut server = FakeServer::new(false);
        for uid in 1..=5 {
            server.add(uid, "subject");
        }
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::new(2);

        let report = synced(&mut server, &mut store, &engine);

        assert_eq!(report.mode, SyncMode::Initial);
        assert_eq!(report.fetched, 5);
        assert_eq!(
            server.fetch_batches,
            vec![vec![5, 4], vec![3, 2], vec![1]],
            "the initial sync must serve the most recent first"
        );
    }

    #[test]
    fn initial_sync_of_empty_mailbox_fetches_nothing() {
        let mut server = FakeServer::new(false);
        let mut store = Store::open_in_memory().unwrap();

        let report = synced(&mut server, &mut store, &SyncEngine::default());

        assert_eq!(report.fetched, 0);
        assert!(server.fetch_batches.is_empty());
    }

    /// Audit 2026-09-01 S1-6 (PLAN-AUDIT-V1 E2): an EMPTIED mailbox (all
    /// archived) went back to "initial sync" because the decision read
    /// `last_uid == 0` — and `SyncMode::Initial` never bubbles
    /// (`notify::arrivals_to_notify`). The "inbox zero" user lost the
    /// first notification after every emptying, and paid for a full
    /// `list_uids` + fetch. The decision is made on the FRESHNESS of the
    /// state (mailbox already initialized), never on the largest UID in
    /// the database.
    #[test]
    fn an_emptied_mailbox_stays_incremental_and_bubbles() {
        for condstore in [false, true] {
            let mut server = FakeServer::new(condstore);
            server.add(1, "first");
            let mut store = Store::open_in_memory().unwrap();
            let engine = SyncEngine::default();

            synced(&mut server, &mut store, &engine);
            server.expunge(1);
            let emptied = synced(&mut server, &mut store, &engine);
            assert_eq!(emptied.mode, SyncMode::Incremental, "condstore={condstore}");
            assert_eq!(emptied.deleted, 1);

            server.add(2, "second");
            let arrival = synced(&mut server, &mut store, &engine);
            assert_eq!(
                arrival.mode,
                SyncMode::Incremental,
                "condstore={condstore}: a mailbox emptied then refilled is NOT an initial sync"
            );
            assert_eq!(arrival.fetched, 1);
            let arrivals = server.fetch_envelopes("INBOX", &[2]).unwrap();
            let bubbles = crate::notify::arrivals_to_notify(arrival.mode, arrivals);
            assert_eq!(
                bubbles.len(),
                1,
                "the arrival after an emptying must bubble"
            );
        }
    }

    fn snapshot(uid_validity: u32, highest_modseq: Option<u64>) -> MailboxSnapshot {
        MailboxSnapshot {
            uid_validity,
            highest_modseq,
            exists: 0,
        }
    }

    fn state(uid_validity: u32, initialized: bool, highest_modseq: Option<u64>) -> SyncState {
        SyncState {
            mailbox_id: 7,
            uid_validity,
            last_uid: 0,
            highest_modseq,
            initialized,
        }
    }

    /// Audit 2026-09-01 S1-7 (PLAN-AUDIT-V1 E3): a DEFINITIVE refusal
    /// from the server (NO/BAD — folder gone, `[CANNOT]`) on an action
    /// used to block the mailbox's ENTIRE queue, forever, in silence:
    /// `break` on the first failure, no way out of the queue. The
    /// refused action enters quarantine (it is visible, it no longer
    /// blocks) and the following ones replay within the same pass.
    #[test]
    fn a_refused_action_does_not_block_the_following_ones() {
        let mut server = FakeServer::new(false);
        server.add(1, "a");
        server.add(2, "b");
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::default();
        synced(&mut server, &mut store, &engine);

        let id = mailbox_id(&store);
        store
            .enqueue_action(id, 1, Action::MoveTo("Disparu".to_string()))
            .unwrap();
        store.enqueue_action(id, 2, Action::MarkSeen).unwrap();
        server.refused_moves = true;

        let report = synced(&mut server, &mut store, &engine);

        assert_eq!(report.refused, 1, "the refused move leaves the queue");
        assert_eq!(report.replayed, 1, "the marking that followed was replayed");
        assert!(server.messages[&2].0.seen);
        assert!(
            store.pending_actions(id).unwrap().is_empty(),
            "the ACTIVE queue is empty: the refused one is no longer in it"
        );
        assert_eq!(store.refused_actions().unwrap(), 1);
        assert!(
            !store.has_pending_actions(id).unwrap(),
            "a refused action no longer forces a poll every cycle (must_poll)"
        );
    }

    /// A TRANSIENT failure (network) keeps being retried — but not
    /// forever: on the fifth, the action enters quarantine and frees the
    /// queue.
    #[test]
    fn five_transient_failures_trigger_quarantine() {
        let mut server = FakeServer::new(false);
        server.add(1, "a");
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::default();
        synced(&mut server, &mut store, &engine);
        let id = mailbox_id(&store);
        store
            .enqueue_action(id, 1, Action::MoveTo("Factures".to_string()))
            .unwrap();
        server.actions_fail = true;

        for attempt in 1..=4 {
            let report = synced(&mut server, &mut store, &engine);
            assert_eq!(report.refused, 0, "attempt {attempt}: still in the queue");
            assert_eq!(store.pending_actions(id).unwrap().len(), 1);
        }
        let fifth = synced(&mut server, &mut store, &engine);
        assert_eq!(fifth.refused, 1, "fifth failure: quarantine");
        assert!(store.pending_actions(id).unwrap().is_empty());

        // The queue is free: a new intention gets through as soon as the network returns.
        server.actions_fail = false;
        store.enqueue_action(id, 1, Action::MarkSeen).unwrap();
        let after = synced(&mut server, &mut store, &engine);
        assert_eq!(after.replayed, 1);
        assert!(server.messages[&1].0.seen);
    }

    /// The pure decision (STANDARD §4): what `sync` used to do inline
    /// with `select`, `record_remote_total` and `replay_actions`.
    #[test]
    fn plan_sync_decides_on_the_freshness_of_the_state() {
        assert_eq!(plan_sync(None, &snapshot(1, None)), SyncPlan::Initial);
        assert_eq!(
            plan_sync(Some(&state(1, true, Some(9))), &snapshot(2, None)),
            SyncPlan::Reset,
            "UIDVALIDITY changed: everything must be redone"
        );
        assert_eq!(
            plan_sync(Some(&state(1, false, None)), &snapshot(1, None)),
            SyncPlan::Initial,
            "mailbox known but never initialized (previous pass died mid-way)"
        );
        assert_eq!(
            plan_sync(Some(&state(1, true, None)), &snapshot(1, None)),
            SyncPlan::Incremental { modseq: None }
        );
        assert_eq!(
            plan_sync(Some(&state(1, true, Some(42))), &snapshot(1, Some(50))),
            SyncPlan::Incremental { modseq: Some(42) },
            "the local state's modseq, not the server's"
        );
    }

    #[test]
    fn resync_without_changes_is_incremental_and_idempotent() {
        for condstore in [false, true] {
            let mut server = FakeServer::new(condstore);
            server.add(1, "a");
            let mut store = Store::open_in_memory().unwrap();
            let engine = SyncEngine::default();

            synced(&mut server, &mut store, &engine);
            let second = synced(&mut server, &mut store, &engine);

            assert_eq!(second.mode, SyncMode::Incremental);
            assert_eq!(second.fetched, 0, "condstore={condstore}");
            assert_eq!(second.deleted, 0);
        }
    }

    #[test]
    fn incremental_fetches_only_new_messages() {
        let mut server = FakeServer::new(false);
        server.add(1, "old");
        server.add(2, "old");
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::default();
        synced(&mut server, &mut store, &engine);

        server.add(3, "new");
        server.add(4, "new");
        let report = synced(&mut server, &mut store, &engine);

        assert_eq!(report.fetched, 2);
        assert_eq!(server.fetch_batches.last(), Some(&vec![4, 3]));
        assert_eq!(recent(&store, 0, 10).len(), 4);
    }

    #[test]
    fn incremental_removes_expunged_messages() {
        let mut server = FakeServer::new(false);
        for uid in 1..=3 {
            server.add(uid, "subject");
        }
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::default();
        synced(&mut server, &mut store, &engine);

        server.expunge(2);
        let report = synced(&mut server, &mut store, &engine);

        assert_eq!(report.deleted, 1);
        let uids: Vec<Uid> = recent(&store, 0, 10).iter().map(|e| e.uid).collect();
        assert_eq!(uids, vec![3, 1]);
    }

    #[test]
    fn condstore_propagates_flag_changes() {
        let mut server = FakeServer::new(true);
        server.add(1, "to read");
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::default();
        synced(&mut server, &mut store, &engine);

        server.mark_seen(1);
        let report = synced(&mut server, &mut store, &engine);

        assert_eq!(report.fetched, 1);
        assert!(recent(&store, 0, 1)[0].seen);
    }

    #[test]
    fn condstore_picks_up_new_messages_too() {
        let mut server = FakeServer::new(true);
        server.add(1, "old");
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::default();
        synced(&mut server, &mut store, &engine);

        server.add(2, "new");
        let report = synced(&mut server, &mut store, &engine);

        assert_eq!(report.fetched, 1);
        assert_eq!(recent(&store, 0, 10).len(), 2);
    }

    /// E2b's sobriety: when CONDSTORE carries the delta and the counts
    /// agree, the full UID inventory — the `UID SEARCH ALL` that took
    /// 34 s on the field INBOX — is NOT paid for. It is only paid for
    /// when a deletion makes it necessary.
    #[test]
    fn condstore_only_pays_for_the_inventory_if_the_count_requires_it() {
        let mut server = FakeServer::new(true);
        server.add(1, "a");
        server.add(2, "b");
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::default();
        synced(&mut server, &mut store, &engine);
        let after_initial = server.uid_list_calls;

        // Flag alone: CONDSTORE delta, equal counts — zero inventory.
        server.mark_seen(1);
        let report = synced(&mut server, &mut store, &engine);
        assert_eq!(report.fetched, 1);
        assert!(recent(&store, 0, 1)[0].seen || recent(&store, 0, 2)[1].seen);
        assert_eq!(
            server.uid_list_calls, after_initial,
            "a flag does not justify a full inventory"
        );

        // Deletion: the count diverges, the inventory becomes due again.
        server.expunge(2);
        let report = synced(&mut server, &mut store, &engine);
        assert_eq!(report.deleted, 1);
        assert_eq!(
            server.uid_list_calls,
            after_initial + 1,
            "a deletion requires the UID diff"
        );
    }

    /// A known and accepted limit: without CONDSTORE, a flag changed
    /// server-side is not refreshed by the incremental sync. This test
    /// documents the behavior so that a future fix is a choice, not an
    /// accident.
    #[test]
    fn without_condstore_flag_changes_are_not_detected() {
        let mut server = FakeServer::new(false);
        server.add(1, "to read");
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::default();
        synced(&mut server, &mut store, &engine);

        server.mark_seen(1);
        synced(&mut server, &mut store, &engine);

        assert!(!recent(&store, 0, 1)[0].seen);
    }

    fn mailbox_id(store: &Store) -> i64 {
        store
            .sync_state(test_account(store), "INBOX")
            .unwrap()
            .unwrap()
            .mailbox_id
    }

    #[test]
    fn replay_pushes_queued_actions_to_server_in_order() {
        let mut server = FakeServer::new(false);
        server.add(1, "a");
        server.add(2, "b");
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::default();
        synced(&mut server, &mut store, &engine);

        let id = mailbox_id(&store);
        store.enqueue_action(id, 1, Action::MarkSeen).unwrap();
        store.enqueue_action(id, 2, Action::MarkSeen).unwrap();
        store.enqueue_action(id, 1, Action::MarkUnseen).unwrap();

        let report = synced(&mut server, &mut store, &engine);

        assert_eq!(report.replayed, 3);
        assert_eq!(
            server.action_calls,
            vec!["seen:1:true", "seen:2:true", "seen:1:false"],
            "the replay must preserve emission order"
        );
        assert!(store.pending_actions(id).unwrap().is_empty());
    }

    /// Since ADR 0017, polling a folder no longer refreshes the folder
    /// list: this LIST used to be paid for EVERY folder (~51 per cycle
    /// on the 2026-08-13 field data). The orchestrator caches it, ONCE
    /// per cycle, at inventory time — the offline-first move is served
    /// there. This test holds the new contract: if the engine starts
    /// listing again, the network bill comes back in silence.
    #[test]
    fn syncing_does_not_refetch_the_folder_list() {
        let mut server = FakeServer::new(false);
        server.add(1, "a");
        server.folders = vec![crate::remote::Folder {
            wire: "Archiv&AOk-s".to_string(),
            display: "Archivés".to_string(), // lang:fr
            selectable: true,
            special_use: None,
            delimiter: None,
        }];
        let mut store = Store::open_in_memory().unwrap();
        synced(&mut server, &mut store, &SyncEngine::default());

        // The poll cached nothing: the list belongs to the cycle's
        // inventory, not to the engine.
        let cached = store.folders(test_account(&store)).unwrap();
        assert!(cached.is_empty());
    }

    /// A move follows the same offline loop as everything else: logged
    /// on click, replayed at the next sync. The folder's WIRE name must
    /// come out intact — an action can be replayed days later, on an
    /// accented folder.
    #[test]
    fn replay_moves_the_message_to_its_journaled_folder() {
        let mut server = FakeServer::new(false);
        server.add(1, "a");
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::default();
        synced(&mut server, &mut store, &engine);

        let id = mailbox_id(&store);
        store
            .enqueue_action(id, 1, Action::MoveTo("Archiv&AOk-s".to_string()))
            .unwrap();

        let report = synced(&mut server, &mut store, &engine);

        assert_eq!(report.replayed, 1);
        assert_eq!(
            server.moved,
            vec![(1, "Archiv&AOk-s".to_string())],
            "the wire name must arrive intact at the server"
        );
        assert!(store.pending_actions(id).unwrap().is_empty());
    }

    /// A disconnect during the replay must lose nothing: the action
    /// stays in the queue for the next sync. Same guarantee as for the
    /// other actions — the move is no exception.
    #[test]
    fn a_failed_move_stays_queued() {
        let mut server = FakeServer::new(false);
        server.add(1, "a");
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::default();
        synced(&mut server, &mut store, &engine);

        let id = mailbox_id(&store);
        store
            .enqueue_action(id, 1, Action::MoveTo("Factures".to_string()))
            .unwrap();
        server.actions_fail = true;

        let report = synced(&mut server, &mut store, &engine);

        assert_eq!(report.replayed, 0);
        assert!(server.moved.is_empty());
        assert_eq!(
            store.pending_actions(id).unwrap().len(),
            1,
            "the intention must survive the disconnect"
        );
    }

    #[test]
    fn replay_stars_and_unstars_on_server() {
        let mut server = FakeServer::new(false);
        server.add(1, "to star");
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::default();
        synced(&mut server, &mut store, &engine);

        let id = mailbox_id(&store);
        store.enqueue_action(id, 1, Action::MarkFlagged).unwrap();
        store.enqueue_action(id, 1, Action::MarkUnflagged).unwrap();

        let report = synced(&mut server, &mut store, &engine);

        assert_eq!(report.replayed, 2);
        assert_eq!(server.action_calls, vec!["flag:1:true", "flag:1:false"]);
        assert!(!server.messages[&1].0.flagged);
    }

    #[test]
    fn condstore_propagates_star_changes() {
        let mut server = FakeServer::new(true);
        server.add(1, "starred elsewhere");
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::default();
        synced(&mut server, &mut store, &engine);

        server.mark_flagged(1);
        let report = synced(&mut server, &mut store, &engine);

        assert_eq!(report.fetched, 1);
        assert!(recent(&store, 0, 1)[0].flagged);
    }

    #[test]
    fn replay_archives_and_deletes_on_server() {
        let mut server = FakeServer::new(false);
        server.add(1, "to archive");
        server.add(2, "to delete");
        server.add(3, "to keep");
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::default();
        synced(&mut server, &mut store, &engine);

        let id = mailbox_id(&store);
        store.remove_local(id, 1).unwrap();
        store.remove_local(id, 2).unwrap();
        store.enqueue_action(id, 1, Action::Archive).unwrap();
        store.enqueue_action(id, 2, Action::Delete).unwrap();

        let report = synced(&mut server, &mut store, &engine);

        assert_eq!(report.replayed, 2);
        assert_eq!(server.action_calls, vec!["archive:1", "delete:2"]);
        assert!(!server.messages.contains_key(&1));
        assert!(!server.messages.contains_key(&2));
        let uids: Vec<Uid> = recent(&store, 0, 10).iter().map(|e| e.uid).collect();
        assert_eq!(uids, vec![3], "only the kept message stays locally");
    }

    /// The Phase 2 gate: a disconnect during the replay loses nothing —
    /// the queue survives and resumes at the next sync.
    #[test]
    fn failed_replay_keeps_actions_queued_for_next_sync() {
        let mut server = FakeServer::new(false);
        server.add(1, "a");
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::default();
        synced(&mut server, &mut store, &engine);

        let id = mailbox_id(&store);
        store.enqueue_action(id, 1, Action::MarkSeen).unwrap();

        server.actions_fail = true;
        let cut = synced(&mut server, &mut store, &engine);
        assert_eq!(cut.replayed, 0);
        assert_eq!(store.pending_actions(id).unwrap().len(), 1);

        server.actions_fail = false;
        let recovered = synced(&mut server, &mut store, &engine);
        assert_eq!(recovered.replayed, 1);
        assert!(store.pending_actions(id).unwrap().is_empty());
        assert!(server.messages[&1].0.seen);
    }

    #[test]
    fn uid_validity_reset_drops_now_meaningless_actions() {
        let mut server = FakeServer::new(false);
        server.add(1, "a");
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::default();
        synced(&mut server, &mut store, &engine);

        let id = mailbox_id(&store);
        store.enqueue_action(id, 1, Action::MarkSeen).unwrap();
        server.bump_uid_validity();

        let report = synced(&mut server, &mut store, &engine);

        assert_eq!(report.replayed, 0);
        assert!(server.action_calls.is_empty());
        assert!(store.pending_actions(id).unwrap().is_empty());
    }

    #[test]
    fn uid_validity_change_triggers_full_resync() {
        let mut server = FakeServer::new(false);
        server.add(1, "before");
        server.add(2, "before");
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::default();
        synced(&mut server, &mut store, &engine);

        server.bump_uid_validity();
        server.messages.clear();
        server.add(10, "after");
        let report = synced(&mut server, &mut store, &engine);

        assert_eq!(report.mode, SyncMode::Initial);
        let rows = recent(&store, 0, 10);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].uid, 10);
        assert_eq!(rows[0].subject.as_deref(), Some("after"));
    }
}

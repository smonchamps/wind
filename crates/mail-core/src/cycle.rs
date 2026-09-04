//! The poll cycle's orchestration — the policy the shell used to own
//! (PLAN-AUDIT-V3 E4, ADR 0033 "poll policy lives in the core").
//!
//! `run_sync` (the full per-account pipeline: INBOX poll, inventory,
//! guarded folder sweep, thread headers, recipients, drafts, echo
//! reconciliation) and `poll_inbox` (the light pass's heart) run HERE,
//! against the [`MailServer`] trait — testable on [`FakeServer`]
//! without a shell. What stays outside comes in through [`CycleHooks`]:
//! progress bookkeeping, the arrival toast, tracing, the once-per-run
//! CONDSTORE warning, the disk-space probe.
//!
//! Two IMAP-specific capabilities live OUTSIDE `MailServer` on purpose:
//! naming the sent folder (RFC 6154 + heuristics, no fake server needs
//! to fake it) and pulling remote drafts (its own HTML-sanitizing
//! boundary, a dependency the core must never carry). [`CycleConnection`]
//! is that second, narrower door — the shell's `ImapServer` answers it
//! through a thin wrapper (`apps/desktop/src/poll.rs`), `FakeServer`
//! answers it with the honest defaults ("no such folder").

use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

use crate::envelope::Envelope;
use crate::error::Error;
use crate::remote::{FolderStatus, MailServer};
use crate::store::Store;
use crate::sync::{
    SYNC_BYTES_PER_MESSAGE, SyncEngine, SyncMode, SyncReport, disk_shortfall, sync_order,
};

/// INBOX's wire name — the mail-core notion the shell used to keep to
/// itself (`commands::MAILBOX`). Kept as one name: the poll cycle and
/// the rest of the shell (the watcher's `server.watch`) must never
/// drift apart on it.
pub const INBOX: &str = "INBOX";

/// Thread headers backfilled per account and per synchronization.
///
/// Generous compared to the body budget (200) because the cost isn't the
/// same: a header block weighs ~3 KB against ~50 KB for a whole message.
/// On the user's mailbox (~2,700 messages), two synchronizations are
/// enough to group the entire mailbox.
const THREAD_HEADER_BUDGET: usize = 2_000;
/// Recipients backfilled per account and per synchronization (R4/R1),
/// budget SHARED between INBOX and Sent. Same cost as a header (one
/// ENVELOPE), and the scope is the same, already converged, one as the
/// thread pass: the pass catches up over a few cycles, then goes quiet.
const RECIPIENTS_BUDGET: usize = 2_000;
/// Arrivals surfaced per account for notifications. Beyond that, only the
/// COUNT matters — the bubble summarizes anyway.
const NOTIFY_MAX_ARRIVALS: usize = 50;
/// Beyond this, the poll no longer fetches bodies itself: the rows
/// first, the pump will do the bodies. ~192 ms per message amortized
/// per batch (`spikes/body-backfill`): ten bodies cost ~2 s on the
/// bubble path — the < 30 s bound of PLAN-SYNCHRO keeps its margin.
const BODY_ON_ARRIVAL_MAX: usize = 10;

/// The shell's side of a cycle: progress counters for the status bar,
/// the arrival notification, the trace file, session-scoped memory,
/// the disk probe. A test drives the cycle with a no-op.
pub trait CycleHooks {
    /// Names the mailbox being polled (clears the phase).
    fn set_mailbox(&self, name: &str);
    /// Names the non-mailbox step: "inventory", "threads", "drafts".
    fn set_phase(&self, name: &str);
    /// New mail landed — the status bar's counter.
    fn add_mail(&self, n: u64);
    /// A visible change happened — the UI's reload witness.
    fn bump_generation(&self);
    /// Shows the arrival notification; a failure comes back as a
    /// problem line, never as an error (the mail IS there).
    fn notify_arrivals(&self, store: &Store, arrivals: &[Envelope]) -> Option<String>;
    /// One dated line in the trace file (no PII — STANDARD §6.8).
    fn trace(&self, line: &str);
    /// True the FIRST time this account is seen without CONDSTORE in
    /// this process — the once-per-session log line's memory.
    fn condstore_missing_first_time(&self, account_id: i64) -> bool;
    /// Bytes available on the database's volume (the disk guard,
    /// ADR 0010 §4). An error means "immeasurable", never "full".
    fn available_space(&self, db_path: &Path) -> Result<u64, String>;
}

/// A cycle driven by nothing: the tests' hooks, and the honest default
/// for any caller that has no shell.
pub struct NoHooks;

impl CycleHooks for NoHooks {
    fn set_mailbox(&self, _name: &str) {}
    fn set_phase(&self, _name: &str) {}
    fn add_mail(&self, _n: u64) {}
    fn bump_generation(&self) {}
    fn notify_arrivals(&self, _store: &Store, _arrivals: &[Envelope]) -> Option<String> {
        None
    }
    fn trace(&self, _line: &str) {}
    fn condstore_missing_first_time(&self, _account_id: i64) -> bool {
        false
    }
    fn available_space(&self, _db_path: &Path) -> Result<u64, String> {
        // Immeasurable, deliberately: the guard then lets the sweep run
        // (blocking mail on a missing probe would be worse than the
        // risk covered — same rule as the shell).
        Err("no probe".to_string())
    }
}

/// The account's own protocol, beyond the portable [`MailServer`]
/// surface: naming its sent folder, and pulling drafts started
/// elsewhere. Deliberately its OWN trait rather than an addition to
/// `MailServer`: no fake server has a reason to fake RFC 6154
/// heuristics or a Drafts-folder round trip, and the draft half drags
/// in an HTML-sanitizing boundary the core must never depend on.
pub trait CycleConnection: MailServer {
    /// The account's sent-folder name, when the connection can name
    /// one. `None`: no such folder — not a failure, an absent
    /// capability, same discipline the shell always applied.
    fn sent_folder_name(&mut self) -> Result<Option<String>, String>;
    /// Pulls in drafts started elsewhere and drops stale mirrors —
    /// best effort like the rest of the cycle: its own failure is
    /// returned, never left unreported, and never fails the poll.
    fn pull_drafts(&mut self, store: &Store, account_id: i64) -> Result<(), String>;
}

/// What an account synchronization reports, beyond the counts.
///
/// The refreshed session (a renewed OAuth token) does NOT travel here
/// any more (PLAN-AUDIT-V3 E4): `mail_auth::AccountSession` is a shell
/// type the core must not know about. The shell already holds it —
/// `connect_imap` hands it back BEFORE `run_sync` is ever called — and
/// pairs it with this outcome itself.
pub struct SyncOutcome {
    pub report: SyncReport,
    /// Non-blocking incidents: the synchronization succeeded, but some
    /// background work that goes with it failed. Reported, never
    /// swallowed — a symptom without a trace is undiagnosable.
    pub problems: Vec<String>,
}

/// The guarded poll (ADR 0017): should this folder be polled? Any
/// uncertainty — poll refused by the server, unreadable marker — polls:
/// sobriety doesn't have the right to cost a message.
/// Reads the folder's local marker — THE construction both the cycle
/// and the shell's post-gesture pass compare against a STATUS
/// (review, wave 3: it existed in three hand-synced copies; a marker
/// field added in one and not the others silently splits the guarded
/// poll).
pub fn local_marker(
    store: &Store,
    account_id: i64,
    mailbox: &str,
) -> Result<Option<crate::sync::LocalMarker>, Error> {
    let Some(state) = store.sync_state(account_id, mailbox)? else {
        return Ok(None);
    };
    Ok(Some(crate::sync::LocalMarker {
        uid_validity: state.uid_validity,
        uidnext_seen: store.remote_uidnext(state.mailbox_id)?,
        local_messages: store.envelope_count(state.mailbox_id)?,
        pending_actions: store.has_pending_actions(state.mailbox_id)?,
        // E2b: the modseq of the last settled SELECT — it's what
        // wakes up a folder where only the flags have shifted.
        modseq_seen: state.highest_modseq,
    }))
}

fn must_poll<H: CycleHooks>(
    store: &Store,
    account_id: i64,
    mailbox: &str,
    status: Option<&FolderStatus>,
    problems: &mut Vec<String>,
    hooks: &H,
) -> bool {
    let Some(status) = status else {
        return true;
    };
    match local_marker(store, account_id, mailbox) {
        Ok(marker) => match crate::sync::poll_reason(status, marker.as_ref()) {
            Some(reason) => {
                // The resweep diagnostic (field 2026-09-04): a polled
                // folder names its REASON in the trace — a cycle that
                // re-polls everything can then be read, not guessed at.
                // The folder itself is MASKED (STANDARD §6, invariant 8:
                // shape only — a user-named folder can carry PII); INBOX
                // is a protocol constant and may be named.
                let shown = if mailbox == INBOX {
                    INBOX.to_string()
                } else {
                    format!("folder({} chars)", mailbox.chars().count())
                };
                hooks.trace(&format!("poll {shown}: {reason}"));
                true
            }
            None => false,
        },
        Err(err) => {
            problems.push(format!("marker of \"{mailbox}\": {err}"));
            true
        }
    }
}

/// Settles the marker of a SUCCESSFUL poll: the UIDNEXT of the status
/// that preceded it. Never on a failed poll — a marker set on a folder
/// that wasn't caught up would wrongly make it skip at the next cycle.
fn settle_marker(
    store: &Store,
    account_id: i64,
    mailbox: &str,
    status: Option<&FolderStatus>,
    problems: &mut Vec<String>,
) {
    let Some(uidnext) = status.and_then(|status| status.uid_next) else {
        return;
    };
    let outcome = store.sync_state(account_id, mailbox).and_then(|state| {
        if let Some(state) = state {
            store.set_remote_uidnext(state.mailbox_id, uidnext)?;
        }
        Ok(())
    });
    if let Err(err) = outcome {
        problems.push(format!("marker of \"{mailbox}\": {err}"));
    }
}

/// How many bodies to fetch WITHIN the INBOX poll that just brought
/// `arrivals` NEW messages (UID above the marker — never the report's
/// `fetched`, inflated by a CONDSTORE delta's flags) — a pure decision
/// (PLAN-REACTIVITE E4, R-D2). A normal batch: all its bodies, the row
/// is born with its preview. A batch that overflows (catch-up after an
/// outage, full sync): zero — the bump goes out first, the rows fast,
/// and the bodies fall to the pump.
fn body_on_arrival(arrivals: usize) -> usize {
    if arrivals > BODY_ON_ARRIVAL_MAX {
        0
    } else {
        arrivals
    }
}

/// The epoch bound of the BODY pumps for an account (ADR 0029,
/// PLAN-HORIZON-NETTOYAGE D1): the import horizon read from the pref,
/// derived at READ time — the bound follows the clock. Envelopes and
/// thread headers stay whole, only the bodies are bounded. Best effort:
/// a failed read bounds nothing — never a silent loss on an error.
fn body_horizon(store: &Store, account_id: i64, hooks: &impl CycleHooks) -> i64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    match store.horizon_import(account_id) {
        Ok(value) => crate::backfill::horizon_epoch(&value, now),
        Err(err) => {
            // §9: the failure is SAID (readable trace via run-wind.ps1),
            // even when the fallback is safe.
            hooks.trace(&format!(
                "horizon_import unreadable (account {account_id}): {err}; importing in full out of caution"
            ));
            crate::backfill::NO_HORIZON
        }
    }
}

/// "1.2 GB" or "850 MB" — the user needs to know HOW MUCH to free up,
/// not convert bytes in their head. Decimal prefixes (the Explorer's
/// would be GiB, but GB is what the general public reads on a disk
/// box).
fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_000_000_000 {
        format!("{:.1} GB", bytes as f64 / 1e9)
    } else {
        // Rounded UP to the MB: announcing "0 MB" to free up would be
        // absurd, and under-announcing would make the retry fail.
        format!("{} MB", bytes.div_ceil(1_000_000).max(1))
    }
}

/// The INBOX poll of an account — the shared core of the full cycle and
/// the light pass (E3): STATUS status, guarded poll (E2a), marker
/// settled, mail counted and account bubbles (P1). Returns the report
/// AND the status paid for — the full cycle reuses it for the space
/// guard, it's never paid for twice.
pub fn poll_inbox<S: MailServer, H: CycleHooks>(
    server: &mut S,
    store: &mut Store,
    account_id: i64,
    hooks: &H,
    problems: &mut Vec<String>,
) -> Result<(SyncReport, Option<FolderStatus>), String> {
    hooks.set_mailbox(INBOX);
    // The highest UID BEFORE the sync: it's what separates "new" from
    // "already known". Fetched before, otherwise the sync would already
    // have moved it.
    let last_uid_before = store
        .sync_state(account_id, INBOX)
        .map_err(|err| err.to_string())?
        .map(|state| state.last_uid)
        .unwrap_or(0);
    // INBOX is guarded like the others (ADR 0017): a STATUS status, the
    // poll only if something has moved.
    let inbox_status = server.folder_status(INBOX).ok();
    let report = if must_poll(
        store,
        account_id,
        INBOX,
        inbox_status.as_ref(),
        problems,
        hooks,
    ) {
        let report = SyncEngine::default()
            .sync(server, store, account_id, INBOX)
            .map_err(|err| err.to_string())?;
        settle_marker(store, account_id, INBOX, inbox_status.as_ref(), problems);
        report
    } else {
        // Nothing moved by COUNT. On a CONDSTORE-less server a
        // flag-only change is invisible to STATUS (no HIGHESTMODSEQ,
        // EXISTS/UIDNEXT silent): without the light flags pass the
        // D-51 window never played on a quiet mailbox and a mail read
        // on the phone stayed bold here (RETOURS-15 review). One
        // bounded `(UID FLAGS)` round trip from the STORE's own UIDs,
        // no inventory — ADR 0017's sobriety holds. A CONDSTORE
        // server skips it: its delta will speak when polled.
        let flags_applied = match inbox_status.as_ref() {
            Some(status) if status.highest_modseq.is_none() => SyncEngine::default()
                .flags_pass(server, store, account_id, INBOX)
                .map_err(|err| err.to_string())?,
            _ => 0,
        };
        SyncReport {
            mode: SyncMode::Incremental,
            fetched: 0,
            deleted: 0,
            replayed: 0,
            refused: 0,
            without_condstore: false,
            flags_applied,
        }
    };

    // E4 (PLAN-REACTIVITE, R-D2): the bodies of ARRIVALS are backfilled
    // on the connection already open, BEFORE the generation bump — the
    // row is born WITH its preview, in the cycle as in the light pass as
    // in the watcher (a single display, never a mute row that fills in
    // later). Bounded: a batch that overflows (catch-up after an
    // outage) bumps first — the rows show fast — and the bodies fall to
    // the pump, which the UI primes on the generation. `bodies_to_backfill`
    // serves from the most recent to the oldest: the "number of
    // arrivals" budget covers exactly the batch that just came in.
    //
    // The bound is measured on ARRIVALS (UID above the pre-poll marker),
    // NEVER on `report.fetched` — first E4 field finding (2026-08-14): on
    // Gmail, every arrival shifts HIGHESTMODSEQ and the CONDSTORE delta
    // returns dozens of retouched envelopes (the recorded observation of
    // PLAN-SYNCHRO); measured on `fetched`, the batch "overflowed" on
    // EVERY arrival and the row was born mute, filled in 3-4s later by
    // the pump.
    let arrivals = match store.arrivals_since(account_id, INBOX, last_uid_before) {
        Ok(n) => n as usize,
        Err(err) => {
            problems.push(format!("count of arrivals: {err}"));
            0
        }
    };
    let body_count = body_on_arrival(arrivals);
    // The import horizon applies here too (uniform with the pump). The
    // bound compares the message's DATE, not its arrival: an arrival
    // with an old Date header (delayed resend, message moved into
    // INBOX by another client) stays out of scope — intended, that's
    // the D1 semantics: its body loads on click.
    let horizon = body_horizon(store, account_id, hooks);
    if body_count > 0
        && let Err(err) =
            crate::backfill::backfill_bodies(server, store, account_id, INBOX, horizon, body_count)
    {
        problems.push(format!("bodies of arrivals: {err}"));
    }

    // P1 (PLAN-SYNCHRO): INBOX's mail is seen RIGHT AWAY — the polled
    // counter reloads the list on the UI side, and the account's
    // bubbles go out HERE, without waiting for the inventory, the
    // folders, or the OTHER accounts (the end-of-cycle aggregate always
    // lost the race against the phone). Arrivals only come from INBOX:
    // nothing is announced late. Best effort, like the neighboring
    // passes: the mail is there, an announcement that fails is logged.
    if report.fetched > 0 || report.deleted > 0 {
        hooks.add_mail((report.fetched + report.deleted) as u64);
        // E4: the MONOTONIC generation — the UI polls it via
        // `sync_progress` and reloads the list when it moves. It's the
        // path through which mail signaled by an IDLE watcher shows up
        // at rest, with no new channel (R0-S5).
        hooks.bump_generation();
    } else if report.flags_applied > 0 {
        // Flags alone (the D-51 window): nothing to count as mail, but
        // the UI must re-serve — without this bump the database
        // updated and the rendered row stayed bold (RETOURS-15 review).
        hooks.bump_generation();
    }
    match store.new_unread_after(account_id, INBOX, last_uid_before, NOTIFY_MAX_ARRIVALS) {
        Ok(arrivals) => {
            let arrivals = crate::notify::arrivals_to_notify(report.mode, arrivals);
            if let Some(problem) = hooks.notify_arrivals(store, &arrivals) {
                problems.push(problem);
            }
        }
        Err(err) => problems.push(format!("arrivals to announce: {err}")),
    }
    // PLAN-AUDIT-V1 review: the cycle that quarantines an action SAYS
    // SO — otherwise only the slot's global counter reveals it, with no
    // link to the faulty cycle. Single exit point for the four paths.
    // D-51 paid at PLAN-RETOURS-15 E3 (Chief-Engineer decision D4, 2026-09-04): a
    // server without CONDSTORE gets a BOUNDED flag window per poll —
    // the line stays, named ONCE per account and per session in
    // `wind.log`, because beyond the window flags remain stale.
    if report.without_condstore && hooks.condstore_missing_first_time(account_id) {
        hooks.trace(&format!(
            "account {account_id}: without CONDSTORE, flags resynchronized by bounded window only (D-51)"
        ));
    }
    if report.refused > 0 {
        hooks.trace(&format!(
            "poll account {account_id}: {} action(s) quarantined",
            report.refused
        ));
    }
    Ok((report, inbox_status))
}

/// The full per-account pipeline (PLAN-AUDIT-V3 E4): INBOX poll,
/// inventory (sent folder, scope, folder list, space guard), the
/// guarded folder sweep, thread headers, recipients, drafts, echo
/// reconciliation. The shell keeps the connection's OWNERSHIP —
/// `&mut S`, never `S` — so it can `logout()` afterwards; the core
/// never gets to consume it.
pub fn run_sync<S: CycleConnection, H: CycleHooks>(
    server: &mut S,
    store: &mut Store,
    account_id: i64,
    db_path: &Path,
    hooks: &H,
) -> Result<SyncOutcome, String> {
    // Stopwatch per phase (field finding 2026-08-13: "INBOX" mute for
    // 2 min 15 — the observation must become a measurement). Durations
    // and counts ONLY: no address, no folder name (diagnostics rule,
    // HANDOVER §6.8) — the account id is an internal integer.
    let stopwatch = Instant::now();
    let mut problems: Vec<String> = Vec::new();
    let (report, inbox_status) = poll_inbox(server, store, account_id, hooks, &mut problems)?;
    let inbox_duration = stopwatch.elapsed();

    // The inventory: sent folder, scope, folder list, space guard
    // (STATUS on each folder) — four tasks that used to live under the
    // "INBOX" label, wrongly.
    hooks.set_phase("inventory");
    let stopwatch = Instant::now();

    // "Sent": without it, a thread only carries the received half of
    // the exchange. Measured on the real mailbox — 15 conversations of
    // more than one message before, 234 after.
    //
    // Only becomes safe because a message's identity now carries its
    // MAILBOX (ADR 0009, step 4b): without that, a UID from this folder
    // would be read in INBOX, and since UIDs restart at 1 in each
    // mailbox, collision would be the norm.
    //
    // Best effort, like the neighboring passes: INCOMING mail is the
    // result that counts, and a server without a sent folder must keep
    // working. The failure is reported, never swallowed — otherwise a
    // mailbox that refused to group would be undiagnosable.
    let sent = match server.sent_folder_name() {
        Ok(found) => found,
        Err(reason) => {
            problems.push(format!("sent folder: {reason}"));
            None
        }
    };

    // The grouping SCOPE, declared before pouring anything else into
    // the account (ADR 0010 §3). Without it, messages from the folders
    // about to be synchronized would join threads on their own: a spam
    // message would bump a conversation to the top of the list.
    //
    // Re-declared on EVERY synchronization, not only on account
    // creation: a server can rename its sent folder.
    //
    // BEFORE the loop, and that's the whole point: the store keeps it in
    // memory on the account, so the mailboxes the loop is about to
    // CREATE are born already on the right side of the scope. Declaring
    // it afterwards would make them born without a thread, and their
    // messages would wait for the next startup.
    if let Err(reason) = store.set_thread_scope(account_id, sent.as_deref()) {
        problems.push(format!("conversation scope: {reason}"));
    }

    // ALL the other folders — archive, trash, spam, user folders (ADR
    // 0010 §1). INBOX has just been done; `sync_order` puts it back
    // first and avoids it twice.
    //
    // LIST-STATUS (RFC 5819) when the server announces it: the list AND
    // the status of EACH folder in ONE round trip — field finding of
    // 2026-08-13, the inventory was the last bottleneck (66s of ~51
    // sequential STATUS on the Gmail account). `statuses` comes out of
    // it pre-filled; the space guard below has nothing left to ask.
    // Fallback (capability absent OR LIST-STATUS failure): a plain
    // LIST, the STATUS calls will go out one by one — the old path,
    // intact.
    let mut statuses: HashMap<String, FolderStatus> = HashMap::new();
    let with_status = match server.folders_with_status() {
        Ok(v) => v,
        Err(reason) => {
            problems.push(format!("LIST-STATUS inventory: {reason}"));
            None
        }
    };
    let folders = if let Some(with_status) = with_status {
        let mut folders = Vec::with_capacity(with_status.len());
        for (folder, status) in with_status {
            // The server MAY omit a folder's status (RFC 5819 §2): this
            // folder then starts out unguarded, the loop below will
            // catch it up with a targeted STATUS.
            if let Some(status) = status {
                statuses.insert(folder.wire.clone(), status);
            }
            folders.push(folder);
        }
        folders
    } else {
        server.folders().unwrap_or_else(|reason| {
            problems.push(format!("folder list: {reason}"));
            Vec::new()
        })
    };
    // Refreshed ONCE per cycle — hoisted out of `SyncEngine::sync` which
    // used to pay for it on EVERY folder (~51 LIST per cycle, ADR 0017).
    // Moving it out keeps its list.
    if let Err(reason) = store.replace_folders(account_id, &folders) {
        problems.push(format!("folder list: {reason}"));
    }
    let order = sync_order(&folders, sent.as_deref());

    // The disk space guard (ADR 0010 §4): estimate BEFORE committing,
    // refuse with a figure if it's short.
    //
    // INBOX is counted on both sides (announced AND local database):
    // removing it from just one would underestimate the remainder.
    //
    // Each folder's status is GUARDED (ADR 0017): the space guard and
    // the poll decision use the same status — the one from LIST-STATUS
    // if it answered, a targeted STATUS otherwise.
    let mut announced: u64 = 0;
    for mailbox in &order {
        let status = if mailbox == INBOX {
            // INBOX already has its status, paid for before its poll.
            inbox_status.ok_or_else(|| "missing INBOX status".to_string())
        } else if let Some(status) = statuses.get(mailbox).copied() {
            // Already fetched by LIST-STATUS: no second round trip.
            Ok(status)
        } else {
            server.folder_status(mailbox).map_err(|err| err.to_string())
        };
        match status {
            Ok(status) => {
                announced += u64::from(status.messages);
                statuses.insert(mailbox.clone(), status);
            }
            // A folder that refuses the status makes the estimate low
            // and will be polled without a guard. We continue: the
            // guard is a protection, not a veto right — and the failure
            // is logged.
            Err(reason) => problems.push(format!("status of \"{mailbox}\": {reason}")),
        }
    }
    let local = store
        .account_message_count(account_id)
        .map_err(|err| err.to_string())?;
    let pending = announced.saturating_sub(local);
    // Space is measured on the database's VOLUME: that's what will
    // absorb the writes, not the system disk. The probe itself is the
    // shell's (`fs4`, outside the core) — reached through the hook.
    let shortfall = match hooks.available_space(db_path) {
        Ok(available) => disk_shortfall(pending, available),
        Err(reason) => {
            // Immeasurable ≠ insufficient space. Blocking mail because a
            // system call failed would be worse than the risk covered;
            // the failure is stated, and SQLite will signal a full disk
            // anyway, write by write.
            problems.push(format!("disk space not measurable: {reason}"));
            None
        }
    };
    let inventory_duration = stopwatch.elapsed();
    let n_folders = order.len().saturating_sub(1);
    let mut n_skipped = 0usize;
    let stopwatch = Instant::now();
    if let Some(missing) = shortfall {
        problems.push(format!(
            "insufficient disk space: ~{} needed for {} remaining \
             message(s), {} short; folder recovery suspended until \
             space is freed up",
            format_bytes(pending.saturating_mul(SYNC_BYTES_PER_MESSAGE)),
            pending,
            format_bytes(missing),
        ));
    } else {
        for mailbox in order.into_iter().skip(1) {
            // The guarded poll (ADR 0017): nothing moved → skipped. The
            // field paid 26 min of SELECT + SEARCH ALL per cycle for
            // motionless folders.
            let status = statuses.get(&mailbox);
            if !must_poll(store, account_id, &mailbox, status, &mut problems, hooks) {
                n_skipped += 1;
                continue;
            }
            hooks.set_mailbox(&mailbox);
            match SyncEngine::default().sync(server, store, account_id, &mailbox) {
                Ok(_) => settle_marker(store, account_id, &mailbox, status, &mut problems),
                Err(reason) => problems.push(format!("folder \"{mailbox}\": {reason}")),
            }
        }
    }
    let folders_duration = stopwatch.elapsed();
    // The header pass isn't a mailbox: the step is named.
    hooks.set_phase("threads");
    let stopwatch = Instant::now();

    // The header pass benefits from the connection already open: that's
    // what makes it free in round trips. Its failure must NOT make the
    // synchronization fail — the mail has arrived, that's the only
    // result that counts — but it's reported, never swallowed. Without a
    // trace, a mailbox that refuses to group would be undiagnosable.
    //
    // It runs on BOTH mailboxes. `References` carries the thread root
    // where `In-Reply-To` only designates the immediate parent: without
    // it, a reply whose original message was archived out of INBOX
    // couldn't reattach. ADR 0008 (measurement 2) is explicit —
    // `References` is mandatory, not a refinement.
    //
    // The budget is SHARED, not doubled: the second mailbox only
    // consumes what the first left. The network cost of a
    // synchronization thus stays exactly what it was before, and since
    // the pass is resumable, the remainder goes out the next round.
    //
    // WITHOUT a horizon since ADR 0010: the field diagnostic showed the
    // pass converged at 1,656 messages read out of 1,656 eligible — and
    // 5,883 messages outside the 12 months that would NEVER be read.
    // The bound came from the bodies' disk budget; a header block
    // weighs ~3 KB and isn't stored on disk like a body.
    //
    // The pass stays on INBOX + Sent, though: `References` is the
    // grouping's fuel, and the grouping stops at that scope (ADR 0010
    // §3). Reading Spam's headers would pay round trips for messages
    // that attach to nothing.
    let mut budget = THREAD_HEADER_BUDGET;
    for mailbox in std::iter::once(INBOX).chain(sent.as_deref()) {
        if budget == 0 {
            break;
        }
        match crate::backfill::backfill_thread_headers(
            server,
            store,
            account_id,
            mailbox,
            crate::backfill::NO_HORIZON,
            budget,
        ) {
            Ok(report) => budget = budget.saturating_sub(report.fetched),
            Err(err) => problems.push(format!("incomplete conversations: {err}")),
        }
    }

    // R4/R1 (PLAN-RETOURS-MAIL): backfill of RECIPIENTS. The header pass
    // has converged — already-synchronized messages have no To/Cc in
    // the database. Two needs: in a sent folder, the sender is SELF and
    // only the recipient says who the message went to (R4 display); and
    // "Reply all" reads these same To/Cc to be instant, offline (R1 —
    // the old server poll on click cost >10s). We re-read the ENVELOPE
    // (To/Cc free, along with the sender) on the open connection,
    // bounded, resumable and at a SHARED budget, on the SAME INBOX +
    // Sent scope as the thread pass. Best effort: a failure is logged,
    // it doesn't make the poll fail.
    let mut budget_recipients = RECIPIENTS_BUDGET;
    for mailbox in std::iter::once(INBOX).chain(sent.as_deref()) {
        if budget_recipients == 0 {
            break;
        }
        match crate::backfill::backfill_recipients(
            server,
            store,
            account_id,
            mailbox,
            budget_recipients,
        ) {
            Ok(report) => budget_recipients = budget_recipients.saturating_sub(report.fetched),
            Err(err) => problems.push(format!("missing recipients: {err}")),
        }
    }

    let threads_duration = stopwatch.elapsed();
    // Drafts pulling also benefits from the open connection. It CANNOT
    // live in the push cycle: that one stops early when there's nothing
    // to push — rightly so, otherwise every keystroke would open a
    // connection. A draft started elsewhere would then never arrive.
    hooks.set_phase("drafts");
    let stopwatch = Instant::now();
    if let Err(reason) = server.pull_drafts(store, account_id) {
        problems.push(format!("remote drafts: {reason}"));
    }
    let drafts_duration = stopwatch.elapsed();

    // E3 (PLAN-REACTIVITE): the cycle may have just brought in the real
    // row of an echo's destination — the reconciliation notices it, and
    // the generation reserves the list (the echo fades under its real
    // row, invisible to the eye).
    match store.reconcile_echos(account_id) {
        Ok(n) if n > 0 => hooks.bump_generation(),
        Ok(_) => {}
        Err(reason) => problems.push(format!("echo reconciliation: {reason}")),
    }

    // The trace that turns "it's stuck" into a measurement — readable
    // in a `cargo run` console. The shell logs out AFTER this returns:
    // a logout that hangs must not take the trace down with it, and
    // `run_sync` only ever borrowed the connection.
    hooks.trace(&format!(
        "poll account {account_id}: INBOX {:.1}s · inventory {:.1}s · {n_folders} folders ({n_skipped} skipped) {:.1}s · threads {:.1}s · drafts {:.1}s",
        inbox_duration.as_secs_f32(),
        inventory_duration.as_secs_f32(),
        folders_duration.as_secs_f32(),
        threads_duration.as_secs_f32(),
        drafts_duration.as_secs_f32(),
    ));

    Ok(SyncOutcome { report, problems })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::FakeServer;

    /// The whole per-account pipeline runs against the trait — the
    /// proof the policy left the shell (it had NO test reachable
    /// without Tauri before E4).
    #[test]
    fn a_full_cycle_runs_on_a_fake_server_without_a_shell() {
        let mut store = Store::open_in_memory().expect("store");
        let account_id = store
            .adopt_or_create_account("cycle@test.io", "imap.test.io")
            .expect("account");
        let mut server = FakeServer::new(false);
        server.add(1, "hello");

        let outcome = run_sync(
            &mut server,
            &mut store,
            account_id,
            Path::new(":memory:"),
            &NoHooks,
        )
        .expect("cycle");
        assert_eq!(outcome.report.fetched, 1, "the INBOX message arrived");
        let mailbox_id = store
            .sync_state(account_id, INBOX)
            .expect("state readable")
            .expect("INBOX state exists after the cycle")
            .mailbox_id;
        assert!(
            store.envelope_count(mailbox_id).unwrap_or(0) >= 1,
            "the envelope is in the store"
        );
    }

    /// The guarded poll (ADR 0017) across a WHOLE cycle: a second
    /// cycle over a motionless server re-lists nothing — the field
    /// paid 26 minutes of SELECT + SEARCH per cycle before the guard,
    /// and the E5 field pass caught a resweep (every folder
    /// "0 skipped") that this net must make impossible to reintroduce.
    #[test]
    fn a_motionless_second_cycle_lists_no_folder_again_with_condstore() {
        motionless_second_cycle_lists_nothing(true);
    }

    #[test]
    fn a_motionless_second_cycle_lists_no_folder_again() {
        motionless_second_cycle_lists_nothing(false);
    }

    fn motionless_second_cycle_lists_nothing(condstore: bool) {
        let mut store = Store::open_in_memory().expect("store");
        let account_id = store
            .adopt_or_create_account("cycle@test.io", "imap.test.io")
            .expect("account");
        let mut server = FakeServer::new(condstore);
        server.add(1, "hello");

        run_sync(
            &mut server,
            &mut store,
            account_id,
            Path::new(":memory:"),
            &NoHooks,
        )
        .expect("first cycle");
        let listed_after_first = server.uid_list_calls;

        let outcome = run_sync(
            &mut server,
            &mut store,
            account_id,
            Path::new(":memory:"),
            &NoHooks,
        )
        .expect("second cycle");
        assert_eq!(outcome.report.fetched, 0, "nothing moved, nothing fetched");
        assert_eq!(
            server.uid_list_calls, listed_after_first,
            "a motionless folder must be SKIPPED, never re-listed (ADR 0017)"
        );
    }

    /// D-51's blind spot (RETOURS-15 review): on a CONDSTORE-less
    /// server a flag-only change moves neither EXISTS nor UIDNEXT, so
    /// the guarded poll rightly skips the mailbox — and the window
    /// inside `incremental_sync` never plays. The light flags pass
    /// must bring the phone's read back anyway, WITHOUT paying an
    /// inventory (ADR 0017's sobriety holds).
    #[test]
    fn a_flag_change_on_a_quiet_condstore_less_server_still_lands() {
        let mut store = Store::open_in_memory().expect("store");
        let account_id = store
            .adopt_or_create_account("cycle@test.io", "imap.test.io")
            .expect("account");
        let mut server = FakeServer::new(false);
        server.add(1, "read on the phone");

        run_sync(
            &mut server,
            &mut store,
            account_id,
            Path::new(":memory:"),
            &NoHooks,
        )
        .expect("first cycle");
        let listed_after_first = server.uid_list_calls;

        // Read elsewhere: flags move, counts do not.
        server.mark_seen(1);
        run_sync(
            &mut server,
            &mut store,
            account_id,
            Path::new(":memory:"),
            &NoHooks,
        )
        .expect("second cycle");

        let mailbox_id = store
            .sync_state(account_id, INBOX)
            .expect("state readable")
            .expect("INBOX state exists")
            .mailbox_id;
        let rows = store.recent(account_id, INBOX, 0, 1).expect("rows");
        assert!(rows[0].seen, "the phone's read must land here");
        assert_eq!(
            server.uid_list_calls, listed_after_first,
            "the light pass pays NO inventory"
        );
        assert!(store.envelope_count(mailbox_id).unwrap_or(0) >= 1);
    }
}

/// What one scheduler tick decides to run — nothing, the light pass,
/// or the full cycle (PLAN-AUDIT-V3 E5: the cadence's DECISION is
/// policy and lives here; the shell owns only the clock that calls it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Due {
    Nothing,
    LightPass,
    FullCycle,
}

/// The poll cadence, as the UI's timers used to encode it (App.svelte,
/// R1 + E3 sleep-wake): a full cycle every `full_every`, a light INBOX
/// pass every `light_every` as a net against a dropped watcher, and a
/// tick arriving late by more than `wake_lag` signals a sleep-wake —
/// THE moment the user looks at the screen: the light pass leaves
/// right away. A full cycle outranks a light pass due at the same
/// tick; the caller skips whatever it decides while a cycle is
/// already in flight (never two polls of the same INBOX).
pub struct Cadence {
    full_every: std::time::Duration,
    light_every: std::time::Duration,
    wake_lag: std::time::Duration,
    last_full: Option<Instant>,
    last_light: Option<Instant>,
    last_tick: Option<Instant>,
}

impl Cadence {
    pub fn new(
        full_every: std::time::Duration,
        light_every: std::time::Duration,
        wake_lag: std::time::Duration,
    ) -> Self {
        Self {
            full_every,
            light_every,
            wake_lag,
            last_full: None,
            last_light: None,
            last_tick: None,
        }
    }

    /// One clock tick: what is due at `now`? The FIRST tick runs the
    /// full cycle (startup: the mail is fetched without waiting half
    /// an hour — the UI used to fire it right after `connect()`).
    pub fn tick(&mut self, now: Instant) -> Due {
        let woke = self
            .last_tick
            .is_some_and(|last| now.saturating_duration_since(last) > self.wake_lag);
        self.last_tick = Some(now);
        let full_due = self
            .last_full
            .is_none_or(|last| now.saturating_duration_since(last) >= self.full_every);
        if full_due {
            self.last_full = Some(now);
            self.last_light = Some(now);
            return Due::FullCycle;
        }
        let light_due = self
            .last_light
            .is_none_or(|last| now.saturating_duration_since(last) >= self.light_every);
        if woke || light_due {
            self.last_light = Some(now);
            return Due::LightPass;
        }
        Due::Nothing
    }

    /// The network came back: the mail held back during the outage
    /// arrives on return (P0-bis) — a light pass leaves right away,
    /// and the cadence counts it.
    pub fn network_returned(&mut self, now: Instant) -> Due {
        self.last_light = Some(now);
        Due::LightPass
    }

    /// A full cycle RAN — whoever triggered it (the scheduler, the
    /// UI's startup sequence, a test). The cadence tracks reality,
    /// never only its own intentions: a cycle the UI just ran must
    /// not be doubled by the next tick.
    pub fn ran_full(&mut self, now: Instant) {
        self.last_full = Some(now);
        self.last_light = Some(now);
    }

    /// A light pass RAN (the manual button included).
    pub fn ran_light(&mut self, now: Instant) {
        self.last_light = Some(now);
    }
}

#[cfg(test)]
mod cadence_tests {
    use super::*;
    use std::time::Duration;

    fn cadence() -> Cadence {
        Cadence::new(
            Duration::from_secs(1800),
            Duration::from_secs(300),
            Duration::from_secs(120),
        )
    }

    #[test]
    fn the_first_tick_runs_the_full_cycle_and_the_cadence_settles() {
        let mut cadence = cadence();
        let t0 = Instant::now();
        assert_eq!(cadence.tick(t0), Due::FullCycle, "startup fetches now");
        assert_eq!(
            cadence.tick(t0 + Duration::from_secs(15)),
            Due::Nothing,
            "nothing is due fifteen seconds in"
        );
        assert_eq!(
            cadence.tick(t0 + Duration::from_secs(301)),
            Due::LightPass,
            "the five-minute net fires"
        );
        assert_eq!(
            cadence.tick(t0 + Duration::from_secs(1801)),
            Due::FullCycle,
            "the half-hour cycle fires and outranks the light pass"
        );
    }

    #[test]
    fn a_late_tick_is_a_wake_and_the_light_pass_leaves_right_away() {
        let mut cadence = cadence();
        let t0 = Instant::now();
        cadence.tick(t0);
        cadence.tick(t0 + Duration::from_secs(15));
        // The machine slept: the next tick lands 10 minutes late but
        // under the full cycle's due time.
        assert_eq!(
            cadence.tick(t0 + Duration::from_secs(15 + 600)),
            Due::LightPass,
            "a clock jump beyond the lag bound polls without waiting"
        );
    }

    #[test]
    fn a_network_return_polls_and_resets_the_light_timer() {
        let mut cadence = cadence();
        let t0 = Instant::now();
        cadence.tick(t0);
        assert_eq!(
            cadence.network_returned(t0 + Duration::from_secs(60)),
            Due::LightPass
        );
        assert_eq!(
            cadence.tick(t0 + Duration::from_secs(75)),
            Due::Nothing,
            "the return's pass counted: the five-minute net is rearmed"
        );
    }
}

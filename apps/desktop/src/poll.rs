//! Per-account poll orchestration — the shell's half, since
//! PLAN-AUDIT-V3 E4 moved the policy itself into
//! [`mail_core::cycle`] (ADR 0033 "poll policy lives in the core").
//!
//! What's left here is exactly what the core cannot be: the connected
//! `ImapServer` (and the two protocol-specific capabilities the
//! `MailServer` trait deliberately excludes, reached through
//! [`ShellServer`] / [`mail_core::cycle::CycleConnection`]), the
//! backoff/lock bookkeeping that shares `AppState`, and the
//! [`ShellHooks`] that feed the core's `CycleHooks` — the status bar,
//! the arrival toast, the trace file, the disk probe.
//!
//! `watcher.rs` calls into this module, never into `commands` (ADR
//! 0018's boundary): the light pass is the one poll path the button,
//! the cycle, and the IDLE watcher all share.

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use mail_auth::{AccountSession, Authenticator};
use mail_core::{Envelope, Store};
use mail_imap::ImapServer;
use tauri::{AppHandle, Manager};

use crate::commands::{self, recovered};
use crate::{AppState, SyncShared};

/// INBOX's wire name (mail-core's `cycle::INBOX`, aliased here so the
/// watcher and the rest of the shell keep one name for it).
pub(crate) const MAILBOX: &str = mail_core::cycle::INBOX;

/// Re-exported so `watcher.rs` reaches them through `poll::`, never
/// through `commands::` (ADR 0018's boundary): both stay defined in
/// `commands.rs` — `lock_accounts` guards `AppState.accounts` used far
/// beyond polling, `db_path` is the one memoized computation of the
/// database's path.
pub(crate) use commands::{db_path, lock_accounts};

/// Wraps the shell's real `ImapServer` so `mail_core::cycle::run_sync`
/// can drive it generically: `MailServer`'s portable operations
/// delegate straight through, and the two IMAP-specific capabilities
/// `CycleConnection` adds — the sent folder's name, pulling drafts —
/// are answered HERE, on the crate that already knows both mail-core
/// and mail-imap (neither may know the other exists).
struct ShellServer<'a>(&'a mut ImapServer);

impl mail_core::MailServer for ShellServer<'_> {
    fn select(&mut self, mailbox: &str) -> Result<mail_core::MailboxSnapshot, mail_core::Error> {
        self.0.select(mailbox)
    }
    fn list_uids(&mut self, mailbox: &str) -> Result<Vec<mail_core::Uid>, mail_core::Error> {
        self.0.list_uids(mailbox)
    }
    fn fetch_envelopes(
        &mut self,
        mailbox: &str,
        uids: &[mail_core::Uid],
    ) -> Result<Vec<Envelope>, mail_core::Error> {
        self.0.fetch_envelopes(mailbox, uids)
    }
    fn changes_since(
        &mut self,
        mailbox: &str,
        modseq: u64,
    ) -> Result<Option<Vec<Envelope>>, mail_core::Error> {
        self.0.changes_since(mailbox, modseq)
    }
    fn fetch_bodies_html(
        &mut self,
        mailbox: &str,
        uids: &[mail_core::Uid],
    ) -> Result<Vec<(mail_core::Uid, mail_core::FetchedBody)>, mail_core::Error> {
        self.0.fetch_bodies_html(mailbox, uids)
    }
    fn fetch_thread_headers(
        &mut self,
        mailbox: &str,
        uids: &[mail_core::Uid],
    ) -> Result<Vec<(mail_core::Uid, mail_core::ThreadHeaders)>, mail_core::Error> {
        self.0.fetch_thread_headers(mailbox, uids)
    }
    fn fetch_attachment(
        &mut self,
        mailbox: &str,
        uid: mail_core::Uid,
        index: usize,
    ) -> Result<Option<Vec<u8>>, mail_core::Error> {
        self.0.fetch_attachment(mailbox, uid, index)
    }
    fn set_seen(
        &mut self,
        mailbox: &str,
        uid: mail_core::Uid,
        seen: bool,
    ) -> Result<(), mail_core::Error> {
        self.0.set_seen(mailbox, uid, seen)
    }
    fn set_flagged(
        &mut self,
        mailbox: &str,
        uid: mail_core::Uid,
        flagged: bool,
    ) -> Result<(), mail_core::Error> {
        self.0.set_flagged(mailbox, uid, flagged)
    }
    fn archive(&mut self, mailbox: &str, uid: mail_core::Uid) -> Result<(), mail_core::Error> {
        self.0.archive(mailbox, uid)
    }
    fn delete(&mut self, mailbox: &str, uid: mail_core::Uid) -> Result<(), mail_core::Error> {
        self.0.delete(mailbox, uid)
    }
    fn folders(&mut self) -> Result<Vec<mail_core::Folder>, mail_core::Error> {
        self.0.folders()
    }
    fn folders_with_status(
        &mut self,
    ) -> Result<Option<Vec<mail_core::FolderWithStatus>>, mail_core::Error> {
        self.0.folders_with_status()
    }
    fn folder_status(
        &mut self,
        mailbox: &str,
    ) -> Result<mail_core::FolderStatus, mail_core::Error> {
        self.0.folder_status(mailbox)
    }
    fn move_to(
        &mut self,
        mailbox: &str,
        uid: mail_core::Uid,
        target: &str,
    ) -> Result<(), mail_core::Error> {
        self.0.move_to(mailbox, uid, target)
    }
}

impl mail_core::cycle::CycleConnection for ShellServer<'_> {
    fn sent_folder_name(&mut self) -> Result<Option<String>, String> {
        self.0.sent_folder_name().map_err(|err| err.to_string())
    }

    /// Pulls in drafts started elsewhere, and removes mirrors that have
    /// become stale.
    ///
    /// The decision belongs to the core ([`mail_core::plan_draft_pull`],
    /// pure and tested); here we only execute it.
    fn pull_drafts(&mut self, store: &Store, account_id: i64) -> Result<(), String> {
        // The marker guard first: if UIDVALIDITY has changed, the
        // recorded `remote_uid`s no longer designate anything.
        // Comparing the remote list to stale markers would make ALL
        // mirrors look stale and would reimport the whole mailbox.
        // No Drafts folder announced: nothing to pull, and nothing to
        // report. The server isn't down, it just doesn't have the
        // capability.
        if self
            .0
            .drafts_folder_name()
            .map_err(|err| err.to_string())?
            .is_none()
        {
            return Ok(());
        }
        let validity = self.0.drafts_uidvalidity().map_err(|err| err.to_string())?;
        let reset = store
            .align_drafts_uidvalidity(account_id, validity)
            .map_err(|err| err.to_string())?;
        if reset {
            // Markers abandoned: nothing distinguishes our own copies
            // from others' anymore. We let the push cycle re-establish
            // them and will pull at the next pass. A duplicate stays
            // possible — that's the golden rule already in force, and
            // it prefers a duplicate to a loss.
            return Ok(());
        }

        let remote = self.0.draft_uids().map_err(|err| err.to_string())?;
        let local = store.drafts_of(account_id).map_err(|err| err.to_string())?;
        let tombstones = store
            .draft_tombstones(account_id)
            .map_err(|err| err.to_string())?;
        let plan = mail_core::plan_draft_pull(&local, &remote, &tombstones);

        for id in plan.stale {
            store.drop_stale_draft(id).map_err(|err| err.to_string())?;
        }
        for uid in plan.fetch {
            let Some(draft) = self.0.fetch_draft(uid).map_err(|err| err.to_string())? else {
                // Gone between the listing and the read: no
                // consequence.
                continue;
            };
            // The body arrives in one of two possible MIME forms; it
            // goes through THE boundary (`body_boundary`) like any
            // body entering the database: sanitized HTML kept (a rich
            // draft pushed then pulled back keeps its formatting),
            // text derived — the MIME text only serves as a fallback
            // when there's no HTML.
            let text = draft.text.unwrap_or_default();
            let (body, body_html) = commands::body_boundary(text, draft.html.as_deref());
            store
                .import_remote_draft(
                    account_id,
                    uid,
                    &draft.to_raw,
                    &draft.subject,
                    &body,
                    body_html.as_deref(),
                )
                .map_err(|err| err.to_string())?;
        }
        Ok(())
    }
}

/// The shell's [`mail_core::cycle::CycleHooks`]: the status bar's
/// activity, the arrival toast, the trace file, the once-per-session
/// CONDSTORE memory, the disk probe — everything the core deliberately
/// cannot do itself.
pub(crate) struct ShellHooks<'a> {
    cycle: &'a SyncShared,
    app: AppHandle,
}

impl<'a> ShellHooks<'a> {
    pub(crate) fn new(cycle: &'a SyncShared, app: AppHandle) -> Self {
        Self { cycle, app }
    }
}

impl mail_core::cycle::CycleHooks for ShellHooks<'_> {
    /// Names the mailbox currently being polled in the shared activity
    /// ("2/2 frozen for 7 minutes"). Empty string between two
    /// mailboxes.
    fn set_mailbox(&self, name: &str) {
        if let Ok(mut mailbox) = self.cycle.mailbox.lock() {
            mailbox.clear();
            mailbox.push_str(name);
        }
        if let Ok(mut phase) = self.cycle.phase.lock() {
            phase.clear();
        }
    }

    /// Names the step WITHOUT a mailbox (folder inventory, threads,
    /// drafts): `name` is a key, the UI translates it — the shell
    /// doesn't compose UI text (A15). Exclusive with the mailbox.
    fn set_phase(&self, name: &str) {
        if let Ok(mut phase) = self.cycle.phase.lock() {
            phase.clear();
            phase.push_str(name);
        }
        if let Ok(mut mailbox) = self.cycle.mailbox.lock() {
            mailbox.clear();
        }
    }

    fn add_mail(&self, n: u64) {
        self.cycle.mail.fetch_add(n, Ordering::Relaxed);
    }

    fn bump_generation(&self) {
        self.cycle.generation.fetch_add(1, Ordering::Relaxed);
    }

    /// Shows the system bubble for a batch of arrivals, if warranted.
    ///
    /// A failure — permission denied, application identity not
    /// registered — must NEVER make a sync fail: the mail has arrived,
    /// that's the only result that counts. But it is **reported**.
    fn notify_arrivals(&self, store: &Store, arrivals: &[Envelope]) -> Option<String> {
        use tauri_plugin_notification::NotificationExt;

        // R-D2 (PLAN-REGLAGES): the preference lives IN THE DATABASE
        // and is read HERE, at emission time — the setting cuts the
        // bubble, never the sync. Unreadable database = enabled: the
        // default protects the announcement, and the sync that just
        // wrote these arrivals makes this case theoretical. The same
        // read carries the texts' language (PLAN-LANGUES, E2):
        // `prefs.lang`, set by the UI — absent or unknown, French. On
        // the caller's connection (PLAN-AUDIT-V2 E1): the poll already
        // holds one, reopening a second one would protect nothing.
        let active = store
            .bool_pref(mail_core::PREF_ARRIVAL_BUBBLES, true)
            .unwrap_or(true);
        if !active {
            return None;
        }
        let lang = mail_core::Lang::from_pref(
            store
                .text_pref(mail_core::PREF_LANG)
                .ok()
                .flatten()
                .as_deref(),
        );
        let notification = mail_core::notification_for(arrivals, lang)?;
        self.app
            .notification()
            .builder()
            .title(notification.title)
            .body(notification.body)
            .show()
            .err()
            .map(|err| format!("notification not shown: {err}"))
    }

    fn trace(&self, line: &str) {
        crate::trace::trace(line);
    }

    /// True the FIRST time this account is seen without CONDSTORE in
    /// this process — the line is said once, not on every poll.
    fn condstore_missing_first_time(&self, account_id: i64) -> bool {
        static TOLD: std::sync::OnceLock<Mutex<std::collections::HashSet<i64>>> =
            std::sync::OnceLock::new();
        let told = TOLD.get_or_init(|| Mutex::new(std::collections::HashSet::new()));
        match told.lock() {
            Ok(mut told) => told.insert(account_id),
            Err(poisoned) => poisoned.into_inner().insert(account_id),
        }
    }

    /// Bytes available on the database's volume (the disk guard, ADR
    /// 0010 §4). An error means "immeasurable", never "full".
    fn available_space(&self, db_path: &Path) -> Result<u64, String> {
        fs4::available_space(db_path.parent().unwrap_or(db_path)).map_err(|err| err.to_string())
    }
}

/// How long to wait after `failures` CONSECUTIVE failures of an account —
/// pure decision (P0 complement, anti-hammering). 0 or 1 failure:
/// nothing, the 5-min cadence is already a courtesy; after that the
/// delay DOUBLES (10, 20, 40 min), capped at 60 — a server that throttles
/// needs air, not a client that insists.
pub(crate) fn wait_after_failures(failures: u32) -> Duration {
    if failures <= 1 {
        return Duration::ZERO;
    }
    let factor = 1u64 << (failures - 1).min(4);
    Duration::from_secs((300 * factor).min(3600))
}

/// The time remaining on this account's backoff, if it's still running.
/// An unreadable lock counts as "no backoff": the protection yields to
/// the poll, never the reverse.
pub(crate) fn current_backoff(
    backoffs: &Mutex<HashMap<String, crate::Backoff>>,
    email: &str,
) -> Option<Duration> {
    let backoffs = backoffs.lock().ok()?;
    let backoff = backoffs.get(email)?;
    wait_after_failures(backoff.failures)
        .checked_sub(backoff.since.elapsed())
        .filter(|remaining| !remaining.is_zero())
}

/// This account's poll lock (E4): the cycle, the button, and the IDLE
/// watcher may all want to poll the same INBOX at the same moment — one
/// account at a time. A poisoned MAP lock is repaired by taking it back:
/// losing the serialization is better than losing the poll.
pub(crate) fn account_lock(
    locks: &Mutex<HashMap<String, Arc<Mutex<()>>>>,
    email: &str,
) -> Arc<Mutex<()>> {
    let mut locks = recovered(locks);
    locks.entry(email.to_string()).or_default().clone()
}

/// Settles the outcome of an attempt: success clears the backoff,
/// failure worsens it and restarts from now.
fn note_outcome(backoffs: &Mutex<HashMap<String, crate::Backoff>>, email: &str, success: bool) {
    let Ok(mut backoffs) = backoffs.lock() else {
        return;
    };
    if success {
        backoffs.remove(email);
        return;
    }
    let backoff = backoffs.entry(email.to_string()).or_insert(crate::Backoff {
        failures: 0,
        since: Instant::now(),
    });
    backoff.failures = backoff.failures.saturating_add(1);
    backoff.since = Instant::now();
}

/// Opens an IMAP connection matching the account type. For an OAuth2
/// account, a failure triggers a silent refresh; for a generic account,
/// the password is fixed.
pub(crate) fn connect_imap(
    session: &AccountSession,
) -> Result<(ImapServer, Option<AccountSession>), String> {
    match session {
        AccountSession::OAuth(auth) => {
            let imap = auth.provider.imap;
            match ImapServer::connect_xoauth2(imap.host, imap.port, &auth.email, &auth.access_token)
            {
                Ok(server) => Ok((server, None)),
                // A CONNECTION failure is not a dead token: no
                // refreshing — hammering the OAuth endpoint on every
                // cycle during a network outage is the best way to turn
                // an IMAP throttle into an account freeze (P0
                // complement, anti-hammering).
                Err(err) if mail_imap::is_connection_error(&err) => Err(err.to_string()),
                Err(_) => {
                    let fresh = Authenticator::from_env(auth.provider)
                        .map_err(|err| err.to_string())?
                        .authenticate_silent(&auth.email)
                        .map_err(|err| err.to_string())?;
                    let server = ImapServer::connect_xoauth2(
                        imap.host,
                        imap.port,
                        &fresh.email,
                        &fresh.access_token,
                    )
                    .map_err(|err| err.to_string())?;
                    Ok((server, Some(AccountSession::OAuth(fresh))))
                }
            }
        }
        AccountSession::Generic(creds) => {
            let server = ImapServer::connect_password(
                &creds.imap_host,
                creds.imap_port,
                &creds.username,
                &creds.password,
            )
            .map_err(|err| err.to_string())?;
            Ok((server, None))
        }
    }
}

/// The accounts from the registry that are connected (session in
/// memory) — the unit of work for the sync/drain/drafts loops.
/// Accounts both known AND connected — opens the database: call it
/// UNDER `off_pump` (E5), never in the glue of an async command.
pub(crate) fn connected_jobs(app: &AppHandle) -> Result<Vec<(i64, AccountSession)>, String> {
    let store = Store::open(&commands::db_path(app)?).map_err(|err| err.to_string())?;
    let known = store.accounts().map_err(|err| err.to_string())?;
    let state = app.state::<AppState>();
    let connected = commands::lock_accounts(&state)?;
    Ok(known
        .into_iter()
        .filter_map(|account| {
            connected
                .get(&account.email)
                .cloned()
                .map(|session| (account.id, session))
        })
        .collect())
}

/// The light pass of ONE account (ADR 0018): the one the IDLE watcher
/// triggers — on `EXISTS`, and on every (re)connection (a mail that
/// arrived during an outage never emits EXISTS, 2nd field finding). Same
/// work as `sync_inbox_light` for this account: guarded poll (E2a), mail
/// counted and generation bumped (the UI reloads on the poll), bubbles
/// (P1). Best effort: incidents go to the console — account id and
/// counts only (§6.8).
pub(crate) fn light_pass_account(app: &AppHandle, email: &str) -> Result<(), String> {
    let state = app.state::<AppState>();
    let path = commands::db_path(app)?;
    let session = commands::lock_accounts(&state)?
        .get(email)
        .cloned()
        .ok_or_else(|| "account not connected".to_string())?;
    // One account at a time: the cycle's or the button's poll may be in
    // progress on THIS account — we wait our turn.
    let lock = account_lock(&state.poll_locks, email);
    let _poll = lock.lock().map_err(|_| "poisoned poll lock".to_string())?;
    // The backoff is respected (read-only): if the account is in
    // repeated failure, the cycle will pick it up — the watcher doesn't
    // insist.
    if current_backoff(&state.sync_backoffs, email).is_some() {
        return Ok(());
    }
    // ONE connection for the pass (PLAN-AUDIT-V2 E1): it crosses the
    // IMAP connection without holding anything — in WAL, a connection
    // open outside a transaction locks no one; the id read before the
    // network is stable, it isn't a state we'd replay afterwards.
    let mut store = Store::open(&path).map_err(|err| err.to_string())?;
    let account_id = store
        .accounts()
        .map_err(|err| err.to_string())?
        .into_iter()
        .find(|account| account.email == email)
        .map(|account| account.id)
        .ok_or_else(|| "unknown account in database".to_string())?;

    let (mut server, refreshed) = connect_imap(&session)?;
    let mut problems = Vec::new();
    let hooks = ShellHooks {
        cycle: state.sync_cycle.as_ref(),
        app: app.clone(),
    };
    let outcome =
        mail_core::cycle::poll_inbox(&mut server, &mut store, account_id, &hooks, &mut problems);
    server.logout();
    match outcome {
        Ok(_) => {
            note_outcome(&state.sync_backoffs, email, true);
            if let Some(fresh) = refreshed {
                commands::lock_accounts(&state)?.insert(fresh.email().to_string(), fresh);
            }
            // The timestamp counts for this poll as for the others:
            // INBOX has just been checked.
            if let Ok(epoch) = SystemTime::now().duration_since(UNIX_EPOCH) {
                let _ =
                    store.set_text_pref(mail_core::PREF_LAST_SYNC, &epoch.as_secs().to_string());
            }
            for problem in problems {
                crate::trace::trace(&format!("watcher account {account_id}: {problem}"));
            }
            Ok(())
        }
        Err(err) => {
            note_outcome(&state.sync_backoffs, email, false);
            Err(err)
        }
    }
}

/// On leaving the cycle — normally or via panic — the activity turns
/// off: a status bar that would announce a phantom cycle would be the
/// exact lie E1 corrects.
struct CycleEnd(Arc<SyncShared>);

impl Drop for CycleEnd {
    fn drop(&mut self) {
        self.0.in_progress.store(false, Ordering::Relaxed);
    }
}

/// What one cycle's account loop tallies — shared by the full cycle
/// and the light pass (PLAN-AUDIT-V3 E3: the two loops were twins,
/// their scaffolding copied line for line).
pub(crate) struct CycleTally {
    pub(crate) accounts: usize,
    pub(crate) accounts_failed: usize,
    pub(crate) fetched: usize,
    pub(crate) deleted: usize,
    pub(crate) replayed: usize,
    pub(crate) errors: Vec<String>,
    pub(crate) refreshed: Vec<AccountSession>,
}

/// ONE account loop for both cycles: activity bookkeeping (E1),
/// backoff (P0 — bypassed only by the manual gesture's `force`),
/// per-account lock (E4: an IDLE watcher may be mid-pass on the same
/// account), outcome accounting. Only the per-account WORK differs —
/// the full `run_sync` or the light `poll_inbox` — and it comes in as
/// a closure returning the same (report, problems, refreshed session)
/// shape.
pub(crate) fn poll_cycle(
    jobs: Vec<(i64, AccountSession)>,
    cycle: &Arc<SyncShared>,
    backoffs: &Mutex<HashMap<String, crate::Backoff>>,
    locks: &Mutex<HashMap<String, Arc<Mutex<()>>>>,
    force: bool,
    mut per_account: impl FnMut(
        i64,
        &AccountSession,
    ) -> Result<
        (mail_core::SyncReport, Vec<String>, Option<AccountSession>),
        String,
    >,
) -> CycleTally {
    // The activity for the status bar (PLAN-SYNCHRO E1): set BEFORE the
    // first account, turned off by the guard no matter what happens. An
    // empty cycle (no account connected) announces nothing.
    let _end = CycleEnd(cycle.clone());
    cycle.done.store(0, Ordering::Relaxed);
    cycle.total.store(jobs.len() as u64, Ordering::Relaxed);
    cycle.mail.store(0, Ordering::Relaxed);
    cycle.in_progress.store(!jobs.is_empty(), Ordering::Relaxed);
    let mut tally = CycleTally {
        accounts: 0,
        accounts_failed: 0,
        fetched: 0,
        deleted: 0,
        replayed: 0,
        errors: Vec::new(),
        refreshed: Vec::new(),
    };
    for (account_id, session) in jobs {
        let email = session.email().to_string();
        // The backoff (P0 complement): an account in repeated failures
        // is SKIPPED while its delay runs — no connection, no OAuth
        // refresh. Without being SILENCED: it stays counted as
        // unreachable, otherwise the bar's alert would turn off on an
        // account still dead. The manual gesture is an order — `force`
        // always attempts.
        if !force && let Some(remaining) = current_backoff(backoffs, &email) {
            tally.accounts_failed += 1;
            tally.errors.push(format!(
                "{email}: backing off after repeated failures; retrying in {} min",
                remaining.as_secs().div_ceil(60).max(1)
            ));
            cycle.done.fetch_add(1, Ordering::Relaxed);
            continue;
        }
        // E4: one account at a time — an IDLE watcher may be in the
        // middle of a light pass on THIS account at the same moment.
        let lock = account_lock(locks, &email);
        let _poll = lock.lock();
        if let Ok(mut account) = cycle.account.lock() {
            account.clone_from(&email);
        }
        if let Ok(mut mailbox) = cycle.mailbox.lock() {
            mailbox.clear();
        }
        if let Ok(mut phase) = cycle.phase.lock() {
            phase.clear();
        }
        match per_account(account_id, &session) {
            Ok((report, problems, fresh)) => {
                note_outcome(backoffs, &email, true);
                tally.accounts += 1;
                tally.fetched += report.fetched;
                tally.deleted += report.deleted;
                tally.replayed += report.replayed;
                if let Some(fresh) = fresh {
                    tally.refreshed.push(fresh);
                }
                for problem in problems {
                    tally.errors.push(format!("{email}: {problem}"));
                }
            }
            Err(err) => {
                note_outcome(backoffs, &email, false);
                tally.accounts_failed += 1;
                tally.errors.push(format!("{email}: {err}"));
            }
        }
        cycle.done.fetch_add(1, Ordering::Relaxed);
    }
    tally
}

/// The end of a cycle (full or light): the timestamp of the last
/// successful poll (E1) — set only when AT LEAST one account answered;
/// an empty cycle does not refresh "last sync." A write failure is
/// reported, never swallowed; it does not fail the poll, the mail is
/// there. Under `off_pump` (E5): database + commands lock. The
/// `unified_count()` that used to live here fed `SyncSummary.total`,
/// which the UI never read (PLAN-AUDIT-V2 E1).
pub(crate) async fn settle_poll(
    app: &AppHandle,
    accounts: usize,
    errors: &mut Vec<String>,
) -> Result<(), String> {
    if accounts == 0 {
        return Ok(());
    }
    let timestamp = commands::off_pump(app.clone(), move |app| {
        let store = Store::open(&commands::db_path(&app)?).map_err(|err| err.to_string())?;
        let mut timestamp = None;
        if let Ok(epoch) = SystemTime::now().duration_since(UNIX_EPOCH)
            && let Err(err) =
                store.set_text_pref(mail_core::PREF_LAST_SYNC, &epoch.as_secs().to_string())
        {
            timestamp = Some(format!("poll timestamp: {err}"));
        }
        Ok::<_, String>(timestamp)
    })
    .await?;
    errors.extend(timestamp);
    Ok(())
}

/// What an account synchronization reports, beyond the counts — the
/// shell's own outer layer around [`mail_core::cycle::SyncOutcome`]:
/// the refreshed session pairs back up HERE, since `connect_imap` (not
/// `run_sync`) is what produced it (PLAN-AUDIT-V3 E4). The core must
/// not know `mail_auth::AccountSession` exists.
pub(crate) struct SyncOutcome {
    pub(crate) report: mail_core::SyncReport,
    /// Session whose token has just been renewed, to put back in cache.
    pub(crate) refreshed: Option<AccountSession>,
    /// Non-blocking incidents: the synchronization succeeded, but some
    /// background work that goes with it failed. Reported, never
    /// swallowed — a symptom without a trace is undiagnosable.
    pub(crate) problems: Vec<String>,
}

/// The account's FULL cycle (PLAN-AUDIT-V3 E4): connects, wraps the
/// connection so `mail_core::cycle::run_sync` can drive it, runs the
/// moved pipeline, logs out. The core only ever BORROWS the connection
/// (`&mut S`) — it never owns it, so it can never log out; that step,
/// like the connect that opened it, stays here.
pub(crate) fn run_sync(
    session: &AccountSession,
    account_id: i64,
    db_path: &Path,
    cycle: &SyncShared,
    app: &AppHandle,
) -> Result<SyncOutcome, String> {
    let (mut server, refreshed) = connect_imap(session)?;
    let mut store = Store::open(db_path).map_err(|err| err.to_string())?;
    let hooks = ShellHooks {
        cycle,
        app: app.clone(),
    };
    let mut wrapped = ShellServer(&mut server);
    let outcome = mail_core::cycle::run_sync(&mut wrapped, &mut store, account_id, db_path, &hooks);
    server.logout();
    let outcome = outcome?;
    Ok(SyncOutcome {
        report: outcome.report,
        refreshed,
        problems: outcome.problems,
    })
}

// --- The scheduler (PLAN-AUDIT-V3 E5, audit 3.2) ---------------------
//
// The CADENCE used to live in the UI's `setInterval`s (App.svelte): a
// window closed to the tray, or a busy renderer, and no cycle ran but
// the watcher's. The clock now ticks HERE; the DECISION of what a tick
// runs stays policy and lives in `mail_core::cycle::Cadence` (three
// unit tests). The tick invokes the SAME Tauri commands the UI's
// timers invoked — full cycle then outbox flush then draft
// reflection, light pass then flush — so the sequence is
// behavior-identical by construction; the UI keeps the manual button
// and its 5 s resting probe, which already reloads the views when the
// generation moves.

/// The scheduler's own clock: one look at the cadence every 15 s —
/// also the sleep-wake detector's resolution (a tick arriving late by
/// more than `WAKE_LAG` means the machine slept; the UI's E3 rule,
/// verbatim).
const TICK: Duration = Duration::from_secs(15);
const FULL_CYCLE_EVERY: Duration = Duration::from_secs(30 * 60);
const LIGHT_PASS_EVERY: Duration = Duration::from_secs(5 * 60);
const WAKE_LAG: Duration = Duration::from_secs(120);

pub(crate) fn new_cadence() -> mail_core::cycle::Cadence {
    mail_core::cycle::Cadence::new(FULL_CYCLE_EVERY, LIGHT_PASS_EVERY, WAKE_LAG)
}

/// Starts the cadence thread. One per process, spawned at setup; it
/// never stops — the process's end is its end.
pub(crate) fn spawn_scheduler(app: AppHandle) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(TICK);
            // No account connected yet: the cadence is not consumed —
            // the FIRST tick after the UI connects runs the startup
            // full cycle (and `connect_accounts` kicks one
            // immediately, so mail never waits on this clock).
            let connected = {
                let state = app.state::<AppState>();
                lock_accounts(&state).map(|accounts| !accounts.is_empty())
            };
            if !matches!(connected, Ok(true)) {
                continue;
            }
            let due = {
                let state = app.state::<AppState>();
                let mut cadence = recovered(&state.cadence);
                cadence.tick(Instant::now())
            };
            tauri::async_runtime::block_on(automatic(&app, due));
        }
    });
}

/// The network came back, or the accounts just connected: a pass
/// leaves right away instead of waiting for the next tick.
pub(crate) fn kick(app: &AppHandle, due: mail_core::cycle::Due) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        automatic(&app, due).await;
    });
}

/// Runs what the tick decided — the UI timers' exact sequences, through
/// the SAME commands they called. Offline, automatic passes wait (the
/// network's return kicks one); a cycle already in flight inhibits the
/// tick the way the UI's `syncing` guard inhibited its timers.
async fn automatic(app: &AppHandle, due: mail_core::cycle::Due) {
    use mail_core::cycle::Due;
    if matches!(due, Due::Nothing) {
        return;
    }
    {
        let state = app.state::<AppState>();
        if !state.online.load(Ordering::Relaxed)
            || state.sync_cycle.in_progress.load(Ordering::Relaxed)
        {
            return;
        }
    }
    match due {
        Due::Nothing => {}
        Due::FullCycle => {
            let outcome = commands::sync_inbox(app.clone(), app.state()).await;
            if let Err(err) = &outcome {
                crate::trace::trace(&format!("scheduler: full cycle: {err}"));
            }
            // The network may be back: the outbox tries its luck
            // again, then the drafts are reflected (push + purge) —
            // the UI cycle's exact tail.
            if let Err(err) = commands::flush_outbox(app.clone(), app.state()).await {
                crate::trace::trace(&format!("scheduler: outbox flush: {err}"));
            }
            match commands::sync_drafts(app.clone(), app.state()).await {
                Err(err) => crate::trace::trace(&format!("scheduler: draft sync: {err}")),
                // A summary that CARRIES an error is a soft failure the
                // UI's catch used to swallow (field 2026-09-04: a draft
                // sat local with no trace anywhere). Named here, once
                // per automatic cycle.
                Ok(summary) => {
                    if let Some(err) = summary.error {
                        crate::trace::trace(&format!("scheduler: draft sync: {err}"));
                    }
                    if summary.kept_local > 0 {
                        crate::trace::trace(&format!(
                            "scheduler: draft sync: {} draft(s) not constructible, kept local",
                            summary.kept_local
                        ));
                    }
                }
            }
        }
        Due::LightPass => {
            let outcome = commands::sync_inbox_light(app.clone(), app.state(), false).await;
            if let Err(err) = &outcome {
                crate::trace::trace(&format!("scheduler: light pass: {err}"));
            }
            if let Err(err) = commands::flush_outbox(app.clone(), app.state()).await {
                crate::trace::trace(&format!("scheduler: outbox flush: {err}"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The backoff table (P0 complement): nothing before two failures —
    /// the 5 min cadence is already a courtesy —, then the delay
    /// doubles, capped at one hour. An overflow of failures (a runaway
    /// counter) must never panic the bit shift.
    #[test]
    fn the_backoff_doubles_then_caps() {
        assert_eq!(wait_after_failures(0), Duration::ZERO);
        assert_eq!(wait_after_failures(1), Duration::ZERO);
        assert_eq!(wait_after_failures(2), Duration::from_secs(600));
        assert_eq!(wait_after_failures(3), Duration::from_secs(1200));
        assert_eq!(wait_after_failures(4), Duration::from_secs(2400));
        assert_eq!(wait_after_failures(5), Duration::from_secs(3600));
        assert_eq!(wait_after_failures(u32::MAX), Duration::from_secs(3600));
    }

    /// The full lifecycle: two failures set a backoff that runs, a
    /// success clears it entirely — the account starts over confident.
    #[test]
    fn a_success_clears_the_backoff() {
        let backoffs = Mutex::new(HashMap::new());
        assert_eq!(current_backoff(&backoffs, "a@exemple.fr"), None);

        note_outcome(&backoffs, "a@exemple.fr", false);
        assert_eq!(
            current_backoff(&backoffs, "a@exemple.fr"),
            None,
            "a single failure doesn't back off: the normal cadence is enough"
        );

        note_outcome(&backoffs, "a@exemple.fr", false);
        assert!(
            current_backoff(&backoffs, "a@exemple.fr").is_some(),
            "two consecutive failures set the backoff"
        );
        // The other account is not touched: the backoff is PER account.
        assert_eq!(current_backoff(&backoffs, "b@exemple.fr"), None);

        note_outcome(&backoffs, "a@exemple.fr", true);
        assert_eq!(current_backoff(&backoffs, "a@exemple.fr"), None);
    }
}

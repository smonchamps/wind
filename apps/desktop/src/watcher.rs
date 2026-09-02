//! The IDLE watcher (ADR 0018) — real time, per account.
//!
//! One thread per connected account, on a DEDICATED IMAP connection
//! (never the cycle's own: the crate's `idle` handle clears the P0
//! timeout on exit — isolating it protects the cycle's lifetime from
//! the rest). The watcher NEVER touches the database: it SIGNALS, and
//! the account's light pass ([`crate::commands::light_pass_account`])
//! does the work — a single poll path, the one the button and the
//! cycle share.
//!
//! Everything below comes out of the measured spike (`spikes/idle/`,
//! field sessions of 2026-08-14): a short restart because it is ALSO
//! the dead-connection detector, a pass on every (re)connection because
//! mail arrived during an outage never emits an EXISTS, reconnection
//! with a doubled delay, token re-read from the keyring on every
//! connection.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use tauri::Manager;

use crate::AppState;
use crate::commands;

/// IDLE restart: the max delay to detect a dead connection (2nd field
/// session: a network drop or Windows sleep produces NO error, the
/// read blocks silently until this deadline). 3 min — well under the
/// 29 allowed by RFC 2177, and 2 commands per cycle: nothing.
const RESTART: Duration = Duration::from_secs(3 * 60);
/// Reconnection with a doubled delay: 2 s → 60 s, rearmed after 2 min
/// of a stable session (taken from the spike, proven in the field).
const PAUSE_MIN: Duration = Duration::from_secs(2);
const PAUSE_MAX: Duration = Duration::from_secs(60);
const SESSION_STABLE: Duration = Duration::from_secs(120);
/// Recheck cadence while the watcher SLEEPS (offline, account in
/// backoff): one atomic read, zero network bytes.
const SLEEP: Duration = Duration::from_secs(5);

/// Reconciles the watchers with the connected accounts: one watcher
/// per session, those of departed accounts turn off on their next
/// turn. Idempotent — called after every connect, add, and remove of
/// an account.
pub(crate) fn reconcile(app: &tauri::AppHandle) {
    let state = app.state::<AppState>();
    // PLAN-AUDIT-V1 review: a poisoned lock is RECOVERED here just as
    // in `lock_accounts` (E5) — abandoning silently left the watchers
    // neither started nor stopped until the next restart, without a
    // word.
    let connected: Vec<String> = match state.accounts.lock() {
        Ok(accounts) => accounts.keys().cloned().collect(),
        Err(poisoned) => poisoned.into_inner().keys().cloned().collect(),
    };
    let mut watchers = match state.watchers.lock() {
        Ok(watchers) => watchers,
        Err(poisoned) => poisoned.into_inner(),
    };
    watchers.retain(|email, alive| {
        let keep = connected.iter().any(|kept| kept == email);
        if !keep {
            alive.store(false, Ordering::Relaxed);
        }
        keep
    });
    for email in connected {
        if watchers.contains_key(&email) {
            continue;
        }
        let alive = Arc::new(AtomicBool::new(true));
        watchers.insert(email.clone(), alive.clone());
        let app = app.clone();
        // A named thread: in a stack dump, "veilleur-idle" reads
        // clearly; the email never appears there (§6.8).
        let _ = std::thread::Builder::new()
            .name("veilleur-idle".to_string())
            .spawn(move || run_loop(app, email, alive));
    }
}

/// A watcher's loop: sleep when it must (offline, backoff), otherwise
/// a dedicated connection → (re)connection pass → watch — and
/// reconnection with a doubled delay when the session drops.
fn run_loop(app: tauri::AppHandle, email: String, alive: Arc<AtomicBool>) {
    // The account's NUMERIC id, for the console (§6.8: never an
    // address in traces). Not found = trace "?".
    let account_id = account_id(&app, &email);
    let mut pause = PAUSE_MIN;
    while alive.load(Ordering::Relaxed) {
        {
            let state = app.state::<AppState>();
            // Offline (P0-bis): sleep, do not hammer.
            if !state.en_ligne.load(Ordering::Relaxed) {
                std::thread::sleep(SLEEP);
                continue;
            }
            // Backoff is respected, READ-only: the watcher never
            // worsens it (its own doubled delay is politeness enough),
            // but it does not push on an account in failure.
            if commands::current_backoff(&state.sync_backoffs, &email).is_some() {
                std::thread::sleep(SLEEP);
                continue;
            }
        }
        let start = Instant::now();
        match watch_session(&app, &email, &alive) {
            // Clean exit: the flag dropped (account removed) or the
            // network left — the loop will decide.
            Ok(()) => continue,
            Err(err) => {
                crate::trace::trace(&format!(
                    "watcher account {account_id}: session dropped: {err}"
                ));
            }
        }
        if !alive.load(Ordering::Relaxed) {
            break;
        }
        if start.elapsed() > SESSION_STABLE {
            pause = PAUSE_MIN;
        }
        crate::trace::trace(&format!(
            "watcher account {account_id}: reconnecting in {} s",
            pause.as_secs()
        ));
        std::thread::sleep(pause);
        pause = (pause * 2).min(PAUSE_MAX);
    }
}

/// A watch session: a dedicated connection (token re-read from the
/// keyring by `connect_imap`), a (re)connection pass, then IDLE turns.
/// `Ok(())` = voluntary exit; `Err` = the connection is dead, the
/// caller reconnects.
fn watch_session(
    app: &tauri::AppHandle,
    email: &str,
    alive: &Arc<AtomicBool>,
) -> Result<(), String> {
    let session = {
        let state = app.state::<AppState>();
        let Some(session) = commands::lock_accounts(&state)?.get(email).cloned() else {
            // No more session (account removed): clean exit, the
            // reconciliation has already turned off the flag or will.
            return Ok(());
        };
        session
    };
    let (mut server, refreshed) = commands::connect_imap(&session)?;
    if let Some(fresh) = refreshed {
        let state = app.state::<AppState>();
        commands::lock_accounts(&state)?.insert(fresh.email().to_string(), fresh);
    }
    // The (RE)CONNECTION pass, never optional: mail that arrived
    // during the absence is already in the mailbox — no EXISTS will
    // signal it (2nd field session). Best effort: its failure does not
    // bring down the watch, the next mail will trigger it.
    if let Err(err) = commands::light_pass_account(app, email) {
        crate::trace::trace(&format!("watcher: connection pass failed: {err}"));
    }
    loop {
        if !alive.load(Ordering::Relaxed) {
            server.logout();
            return Ok(());
        }
        {
            let state = app.state::<AppState>();
            if !state.en_ligne.load(Ordering::Relaxed) {
                // The OS said "offline" (P0-bis): give back the
                // connection — it is probably already dead — and the
                // loop will sleep until it returns.
                server.logout();
                return Ok(());
            }
        }
        match server.watch(commands::MAILBOX, RESTART) {
            Ok(mail_imap::Watch::Mail) => {
                // Mail! The account's light pass polls it — on ITS OWN
                // connection (P0 timeouts intact), while this one goes
                // back to watching.
                if let Err(err) = commands::light_pass_account(app, email) {
                    crate::trace::trace(&format!("watcher: light pass failed: {err}"));
                }
            }
            // Heartbeat: the DONE/re-IDLE of the next turn will prove
            // the connection is alive.
            Ok(mail_imap::Watch::Timeout) => {}
            Err(err) => return Err(err.to_string()),
        }
    }
}

/// The account's numeric id — the only name a trace is allowed to
/// carry (§6.8).
fn account_id(app: &tauri::AppHandle, email: &str) -> String {
    let found = || -> Option<i64> {
        let path = commands::db_path(app).ok()?;
        let store = mail_core::Store::open(&path).ok()?;
        store
            .accounts()
            .ok()?
            .into_iter()
            .find(|account| account.email == email)
            .map(|account| account.id)
    };
    found().map_or_else(|| "?".to_string(), |id| id.to_string())
}

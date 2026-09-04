// The `mesure` feature (startup bench) KEEPS the console: a binary
// built with `windows_subsystem = "windows"` has no stderr, and the
// upstream spans would have nowhere to write. The shipped binary
// itself does not change — the release build never carries the
// feature.
#![cfg_attr(
    all(not(debug_assertions), not(feature = "mesure")),
    windows_subsystem = "windows"
)]
//! Desktop shell: the Tauri window wired to the core.
//!
//! The UI is "dumb" (PLAN.md §3): it displays the state and emits
//! intents through the commands of [`commands`]; all the intelligence
//! lives in mail-core / mail-imap / mail-smtp / mail-auth.

mod commands;
mod fault;
mod instance;
mod poll;
mod relocation;
mod telemetry;
mod trace;
mod watcher;
mod wire;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// The progress of a legacy database's migration, shared between the
/// pass (which writes) and the UI (which polls in a loop and can
/// cancel). Atomics, not a Mutex: the pass writes every 1,000
/// messages, polling must never make it wait.
#[derive(Default)]
pub(crate) struct MigrationShared {
    pub done: AtomicU64,
    pub total: AtomicU64,
    pub cancel: AtomicBool,
}

/// The sync cycle's activity (PLAN-SYNCHRO E1), shared between the
/// loop (which writes) and the UI (which polls every second during
/// the cycle). Atomics, like the migration: polling must never make
/// the loop wait. The current account — the only text — lives under a
/// Mutex written once per account, never in a hot loop.
#[derive(Default)]
pub(crate) struct SyncShared {
    pub in_progress: AtomicBool,
    pub done: AtomicU64,
    pub total: AtomicU64,
    pub account: Mutex<String>,
    /// The mailbox currently being polled WITHIN the account — field
    /// finding of 2026-08-13: "2/2 · account" frozen for 7 minutes
    /// during the folder sweep, with no information at all. Empty
    /// between two mailboxes.
    pub mailbox: Mutex<String>,
    /// The step with no mailbox (catalogue key on the UI side:
    /// `inventory`, `threads`, `drafts`) — second field finding of
    /// 2026-08-13: "INBOX" covered four distinct phases, observation
    /// was blind. Exclusive with `mailbox`; empty otherwise.
    pub phase: Mutex<String>,
    /// INBOX mail already visible in the database WITHIN the current
    /// cycle (arrivals + removals, cumulated account after account) —
    /// P1 (PLAN-SYNCHRO): the probe reads it and reloads the list as
    /// soon as an account's INBOX poll is settled, without waiting for
    /// the cycle to end. A polled counter, not a channel: the UI port
    /// stays R0-S5.
    pub mail: AtomicU64,
    /// Mail generation, MONOTONIC and never reset (E4): bumped on
    /// every INBOX poll that brought in or removed mail — cycle,
    /// button, IDLE watcher alike. The UI reads it at the
    /// `sync_progress` poll (5 s, already in place) and reloads the
    /// list when it moves: that is how mail signaled by a watcher
    /// shows up AT REST, with no new channel (R0-S5).
    pub generation: AtomicU64,
}

/// The backoff of an account in failure (P0 complement, anti-hammering):
/// how many CONSECUTIVE failures, and since when. In memory only — a
/// restart starts over trusting, and that is intentional: the backoff
/// protects the server from a loop, not from a user restarting their
/// application.
pub(crate) struct Backoff {
    pub failures: u32,
    pub since: Instant,
}

/// The state of an account's post-gesture pass (PLAN-REACTIVITE E3):
/// one flight at a time, coalesced — a request during the flight
/// raises the flag, the pass replays ONCE on the way out. Archiving
/// ten messages does not open ten passes.
#[derive(Default)]
pub(crate) struct PassFlight {
    pub in_flight: bool,
    pub rerequest: bool,
}

pub(crate) struct AppState {
    /// Connected accounts' sessions, by email (multi-account).
    pub accounts: Mutex<HashMap<String, mail_auth::AccountSession>>,
    /// Serializes outbox flushes: two concurrent pumps would quarantine
    /// each other's sends.
    pub outbox_flush: Arc<Mutex<()>>,
    /// Serializes pushing drafts to Gmail: two concurrent pushes would
    /// create duplicate remote copies.
    pub drafts_push: Arc<Mutex<()>>,
    /// Serializes the backfill of bodies: two concurrent pumps would
    /// fight over bandwidth and the same messages.
    pub bodies_backfill: Arc<Mutex<()>>,
    /// Progress and cancellation of the visible migration (Phase 5).
    pub migration: Arc<MigrationShared>,
    /// Sync cycle activity, for the status bar (E1).
    pub sync_cycle: Arc<SyncShared>,
    /// Backoffs per account (email → consecutive failures): the cycle
    /// and the light pass SKIP an account in backoff — without hiding
    /// it, it stays counted as unreachable. A manual gesture always
    /// forces the attempt.
    pub sync_backoffs: Arc<Mutex<HashMap<String, Backoff>>>,
    /// One poll lock PER account (email → lock): the cycle, the
    /// button, and the IDLE watcher may all want to poll the same
    /// INBOX at the same time — two concurrent polls of the same
    /// account would be idempotent but pay twice. One account at a
    /// time.
    pub poll_locks: Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>>,
    /// The IDLE watchers (ADR 0018): email → life flag. Turning off the
    /// flag stops the watcher on its next turn (≤ restart).
    pub watchers: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    /// The network state reported by the UI (P0-bis): offline, the
    /// watchers sleep instead of reconnecting in a loop.
    pub online: Arc<AtomicBool>,
    /// Post-gesture passes in flight, per account (E3): one flight at
    /// a time, requests during the flight coalesce.
    pub gesture_passes: Arc<Mutex<HashMap<String, PassFlight>>>,
    /// The commands lock (PLAN-GELS): blocking commands run off the
    /// pump (`spawn_blocking`) BUT one at a time, as when the main
    /// thread used to serialize them — without this lock, two
    /// read-decide-write pairs would cross (local state vs the
    /// action queue of `mark_flagged`, drafts TOCTOU). The window
    /// stays free; the serialization stays.
    pub commands: Arc<Mutex<()>>,
}

/// Startup bench (`mesure` feature): arm the collector BEFORE anything
/// else, so the upstream spans are captured from the moment they open.
///
/// Nothing to instrument ourselves — the spans already exist:
/// `wry::window::create` (the tao window alone), `wry::webview::create`
/// (the whole function, window + webview), `wry::window::draw` (window
/// created → first frame). The SLICE sought is their difference:
/// `webview::create` − `window::create`.
///
/// These are `debug_span!`s: at the default level nothing is recorded,
/// hence the explicit `DEBUG`. `FmtSpan::CLOSE` only prints on close,
/// with the duration — we do not want event-level detail. The filter
/// is ESSENTIAL, not a convenience: without it, tauri's `app::setup`
/// span (laid down by `#[instrument]`, hence invisible to a `grep` for
/// `span!`) carries the ENTIRE Debug of `App` — several thousand
/// characters spat out on stderr on EVERY span close, and that write
/// lands INSIDE `wry::webview::create`, i.e. inside the very slice
/// being measured. Found on the first observation run.
///
/// So we keep only the two useful targets: `tauri_runtime_wry` at
/// DEBUG (the three creation spans) and `wry` at INFO (IPC and asset
/// serving, which are `info_span!`s).
#[cfg(feature = "mesure")]
fn arm_spans() {
    use tracing_subscriber::filter::LevelFilter;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    let layer = tracing_subscriber::fmt::layer()
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
        .with_ansi(false)
        .with_writer(std::io::stderr);
    let targets = tracing_subscriber::filter::Targets::new()
        .with_target("tauri_runtime_wry", LevelFilter::DEBUG)
        .with_target("wry", LevelFilter::INFO)
        // `tauri::ipc` ONLY, not `tauri`: the prefix avoids the
        // `app::setup` span, which would spit out the entire Debug of
        // `App`. Gives one span per COMMAND handled — enough to tell
        // a doubled transport from a command genuinely executed
        // twice.
        .with_target("tauri::ipc", LevelFilter::TRACE)
        // Our own markers (`mesure::*`), placed inside the commands
        // whose upstream span gives the TOTAL without saying what it
        // covers.
        .with_target("wind_desktop", LevelFilter::DEBUG);
    let _ = tracing_subscriber::registry()
        .with(layer)
        .with(targets)
        .try_init();
}

/// A modal message BEFORE any Tauri window, then exit. `rfd` is what
/// the dialog plugin wraps; there is no `AppHandle` yet at this point
/// (the window would be born before `setup`, tauri `app.rs`). The
/// trace says it too, for console launches.
fn warn_and_exit(message: &str, code: i32) -> ! {
    eprintln!("{message}");
    rfd::MessageDialog::new()
        .set_title("Wind")
        .set_level(if code == 0 {
            rfd::MessageLevel::Info
        } else {
            rfd::MessageLevel::Error
        })
        .set_description(message)
        .show();
    std::process::exit(code)
}

fn main() {
    #[cfg(feature = "mesure")]
    arm_spans();
    // The Discovery → Wind relocation (PLAN-WIND E3) comes BEFORE
    // anything else: neither the database nor the WebView2 profile
    // must be born on the Wind side while a Discovery workstation is
    // waiting on its rename. Failure = a hard stop — continuing would
    // offer an empty application to a user whose data is one rename
    // away.
    if let Err(err) = relocation::relocate() {
        // In release the binary has no console: without a dialog box,
        // "the application does not start" without a word (2026-09-01
        // audit).
        warn_and_exit(
            &format!(
                "Échec du déménagement des données Discovery → Wind : {err}\n\
                 Fermez toute autre instance de l'application, puis relancez." // lang:fr
            ),
            1,
        );
    }
    // Single instance (PLAN-AUDIT-V1 E1, D1): BEFORE any database and
    // any window — two concurrent pumps would quarantine each other's
    // sends, and double the watchers and the arrival notifications.
    // The guard lives until the end of the process; the OS releases
    // the lock.
    let folder = instance::database_folder();
    // E9: field trace knows where to write from the very first
    // gesture.
    if let Some(folder) = &folder {
        trace::init(folder.clone());
    }
    let _instance_guard = match folder.as_deref().map(instance::lock) {
        Some(Ok(Some(guard))) => Some(guard),
        Some(Ok(None)) => warn_and_exit("Wind est déjà ouvert.", 0), // lang:fr
        // Lock impossible (read-only folder, disk full…): we do not
        // deprive the user of their mail for a lock file — said to
        // the trace, without a guard.
        Some(Err(err)) => {
            trace::trace(&format!(
                "instance lock impossible: {err} — launching without a guard"
            ));
            None
        }
        None => None,
    };
    let state = AppState {
        accounts: Mutex::new(HashMap::new()),
        outbox_flush: Arc::new(Mutex::new(())),
        drafts_push: Arc::new(Mutex::new(())),
        bodies_backfill: Arc::new(Mutex::new(())),
        migration: Arc::new(MigrationShared::default()),
        sync_cycle: Arc::new(SyncShared::default()),
        sync_backoffs: Arc::new(Mutex::new(HashMap::new())),
        poll_locks: Arc::new(Mutex::new(HashMap::new())),
        watchers: Arc::new(Mutex::new(HashMap::new())),
        // Online by default: the UI reports the real state on its
        // first render (P0-bis) — until then, better to try than to
        // sleep.
        online: Arc::new(AtomicBool::new(true)),
        gesture_passes: Arc::new(Mutex::new(HashMap::new())),
        commands: Arc::new(Mutex::new(())),
    };
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        // Install the panic hook and load consent BEFORE everything
        // else: an early crash must be capturable (if the user has
        // consented). Never touches the database (ADR 0014).
        .setup(|app| {
            telemetry::init(app);
            // PLAN-AUDIT-V1 review: the ONLY call to `db_path` that
            // does I/O (folder created, path memorized) happens here,
            // on the main thread before the window — never in the
            // bare async body of a command.
            let _ = commands::db_path(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::connect_accounts,
            commands::add_account,
            commands::add_microsoft_account,
            commands::add_generic_account,
            commands::reconnect_account,
            commands::remove_account,
            commands::sync_inbox,
            commands::sync_inbox_light,
            commands::nav_snapshot,
            commands::list_category,
            commands::category_total,
            commands::preview_catchup,
            commands::thread_messages,
            commands::search_messages,
            commands::message_body,
            commands::message_attachments,
            commands::reply_invitation,
            commands::suggested_save_path,
            commands::save_attachment,
            commands::mark_seen,
            commands::mark_flagged,
            commands::toggle_pin,
            commands::allow_images_message,
            commands::allow_images_sender,
            commands::images_senders,
            commands::revoke_images_sender,
            commands::organized_mode_get,
            commands::organized_mode_set,
            commands::route_sender,
            commands::route_sender_from,
            commands::remove_routing,
            commands::routings,
            commands::screener_waiting,
            commands::screener_total,
            commands::feed_unopened,
            commands::screener_addresses,
            commands::paper_trail_groups,
            commands::paper_trail_group_page,
            commands::screener_defaults_get,
            commands::screener_defaults_set,
            commands::horizon_import_get,
            commands::horizon_import_set,
            commands::cleanup_state,
            commands::cleanup_start,
            commands::cleanup_groups,
            commands::cleanup_messages,
            commands::cleanup_verdict,
            commands::cleanup_finish,
            commands::toggle_set_aside,
            commands::set_aside_pile,
            commands::feed_cards,
            commands::feed_mark_read,
            commands::pinned_rows,
            commands::archive_message,
            commands::act_on_group,
            commands::ui_state,
            commands::list_folders,
            commands::move_message,
            commands::delete_message,
            commands::report_spam,
            commands::mark_not_spam,
            commands::reply_context,
            commands::reply_all_context,
            commands::forward_context,
            commands::queue_send,
            commands::flush_outbox,
            commands::sync_after_gesture,
            commands::echo_body,
            commands::echo_attachments,
            commands::complete_addresses,
            commands::outbox_status,
            commands::outbox_requeue,
            commands::outbox_delete,
            commands::outbox_cancel_scheduled,
            commands::signature_get,
            commands::signature_set,
            commands::markers_get,
            commands::marker_set,
            commands::names_get,
            commands::name_set,
            commands::save_draft,
            commands::list_drafts,
            commands::delete_draft,
            commands::attach_files,
            commands::detach_file,
            commands::draft_attachments,
            commands::fetch_source_attachment,
            commands::sync_drafts,
            commands::sync_progress,
            commands::sync_activity,
            commands::network_state,
            commands::backfill_status,
            commands::backfill_bodies,
            commands::migration_check,
            commands::migration_run,
            commands::migration_progress,
            commands::migration_cancel,
            commands::update_check,
            commands::update_install,
            commands::address_names,
            commands::app_version,
            commands::open_link,
            commands::notif_pref_get,
            commands::notif_pref_set,
            commands::lang_get,
            commands::lang_set,
            telemetry::telemetry_consent_get,
            telemetry::telemetry_consent_set,
            telemetry::telemetry_pending,
            telemetry::telemetry_open_folder,
            telemetry::telemetry_selftest_panic,
        ])
        .run(tauri::generate_context!());
    if let Err(err) = result {
        eprintln!("window startup failed: {err}");
        std::process::exit(1);
    }
}

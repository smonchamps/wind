//! Crash telemetry — LOCAL capture of panics, opt-in (ADR 0014).
//!
//! Destination: one file per crash in `app_data_dir/crashes/`.
//! **No network, no third party** — the app shows the reports, the
//! user decides to send them. The content is redacted by
//! [`mail_core::redact`], which discards the panic message (the only
//! vector of personal data) and keeps only code artifacts.
//!
//! **Two hard rules:**
//! 1. The panic hook NEVER touches the database (it may be the cause
//!    of the panic, or hold a poisoned lock): consent lives in a
//!    file, read at startup into an atomic, the report is written in
//!    pure `std::fs`.
//! 2. The hook must never panic in turn (a panic during a panic =
//!    `abort`): everything in it is wrapped and non-`unwrap`.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use mail_core::{RawPanic, redact};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

/// Effective consent, read by the panic hook. An atomic and not the
/// database: the hook must stay cheap and safe even if SQLite is at
/// fault.
static ENABLED: AtomicBool = AtomicBool::new(false);

/// Sequence number of reports within one run: it makes the file name
/// unique. A crash on the main thread produces TWO (the original
/// panic, then the FFI boundary's `cannot unwind`), in the same
/// second — without this counter, the second would overwrite the
/// first, the only useful one. Found in the field (ADR 0014).
static SEQ: AtomicU32 = AtomicU32::new(0);

/// The E2E hook: under test, never a banner nor a real write (same
/// seams as everything else, handover §7.5).
fn is_e2e() -> bool {
    std::env::var("WIND_DB_PATH").is_ok()
}

/// The app's base data folder — where consent and reports live. In
/// E2E, co-located with the disposable database.
fn base_dir(app: &AppHandle) -> Result<PathBuf, String> {
    if let Ok(db) = std::env::var("WIND_DB_PATH") {
        return Ok(PathBuf::from(db)
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from(".")));
    }
    app.path().app_data_dir().map_err(|err| err.to_string())
}

fn crashes_dir(base: &Path) -> PathBuf {
    base.join("crashes")
}

fn consent_file(base: &Path) -> PathBuf {
    base.join("telemetry.json")
}

#[derive(Serialize, Deserialize)]
struct ConsentFile {
    crash_reports: bool,
}

/// To call once at startup (in `.setup`): loads consent and installs
/// the panic hook.
pub fn init(app: &tauri::App) {
    let Ok(base) = base_dir(app.handle()) else {
        return;
    };
    let _ = std::fs::create_dir_all(&base);
    // Loads the consent persisted into the atomic. Missing = never
    // asked = disabled: nothing is written until the user has said
    // yes.
    if let Ok(text) = std::fs::read_to_string(consent_file(&base))
        && let Ok(c) = serde_json::from_str::<ConsentFile>(&text)
    {
        ENABLED.store(c.crash_reports, Ordering::Relaxed);
    }
    install_hook(crashes_dir(&base), app.package_info().version.to_string());
}

/// Installs the hook. It chains the default hook: a panic keeps its
/// normal behavior (trace on stderr), we just add the report write
/// when consent is active.
fn install_hook(dir: PathBuf, app_version: String) {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if ENABLED.load(Ordering::Relaxed) {
            // A panic inside the hook would abort the process: we
            // bound it.
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                write_report(&dir, &app_version, info);
            }));
        }
        default_hook(info);
    }));
}

fn write_report(dir: &Path, app_version: &str, info: &std::panic::PanicHookInfo<'_>) {
    let message = payload_message(info);
    // A panic on the main thread crosses WebView2's FFI boundary
    // (nounwind) and triggers a SECOND "cannot unwind" panic that
    // aborts. This second panic points at the runtime, not the
    // original bug — which the first panic has already reported. We
    // therefore do not write it.
    if is_secondary_nounwind(&message) {
        return;
    }
    let raw = RawPanic {
        message,
        location: info
            .location()
            .map(|loc| format!("{}:{}", loc.file(), loc.line())),
        backtrace: std::backtrace::Backtrace::force_capture()
            .to_string()
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect(),
        app_version: app_version.to_string(),
        os: os_label(),
        timestamp: now_iso8601(),
    };
    let _ = persist(dir, raw);
}

/// Redacts then writes the report to disk. Separated from
/// [`write_report`] to be TESTABLE: `PanicHookInfo` cannot be built by
/// hand, and what matters is not that the redaction is correct in
/// memory but that the bytes written to disk carry no personal data.
fn persist(dir: &Path, raw: RawPanic) -> std::io::Result<PathBuf> {
    // This is WHERE personal data is discarded — before any write.
    // What follows only handles the redacted report.
    let report = redact(raw);

    #[derive(Serialize)]
    struct ReportFile<'a> {
        app_version: &'a str,
        os: &'a str,
        location: Option<&'a str>,
        backtrace: &'a [String],
        timestamp: &'a str,
    }

    std::fs::create_dir_all(dir)?;
    // ':' is forbidden in a Windows file name; the `SEQ` counter makes
    // the name unique even for two crashes in the same second.
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let name = format!("crash-{}-{seq}.json", report.timestamp.replace(':', "-"));
    let path = dir.join(name);
    let payload = ReportFile {
        app_version: &report.app_version,
        os: &report.os,
        location: report.location.as_deref(),
        backtrace: &report.backtrace,
        timestamp: &report.timestamp,
    };
    let json = serde_json::to_string_pretty(&payload).map_err(std::io::Error::other)?;
    std::fs::write(&path, json)?;
    Ok(path)
}

/// A secondary runtime panic — "panic in a function that cannot
/// unwind", triggered when a panic crosses an FFI boundary. It does
/// not name the original bug; we discard it. Best-effort filter: if
/// the runtime's message ever changes, we write one report too many
/// (never one too few), and the `SEQ` counter prevents any overwrite.
fn is_secondary_nounwind(message: &str) -> bool {
    message.contains("cannot unwind")
}

fn payload_message(info: &std::panic::PanicHookInfo<'_>) -> String {
    let payload = info.payload();
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "panic (non-textual payload)".to_string()
    }
}

fn os_label() -> String {
    format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)
}

fn now_iso8601() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

// --- Tauri commands ----------------------------------------------------

/// Consent state for the UI: `unset` triggers the opt-in prompt, once.
/// In E2E, always `disabled` — no banner under test.
#[tauri::command]
pub async fn telemetry_consent_get(app: AppHandle) -> String {
    // File read: off the pump (PLAN-GELS). A join failure is worth
    // "disabled" — the opt-in banner is not invented.
    crate::commands::off_pump(app, |app| {
        if is_e2e() {
            return Ok::<_, String>("disabled".to_string());
        }
        let Ok(base) = base_dir(&app) else {
            return Ok::<_, String>("disabled".to_string());
        };
        Ok(match std::fs::read_to_string(consent_file(&base)) {
            Ok(text) => match serde_json::from_str::<ConsentFile>(&text) {
                Ok(c) if c.crash_reports => "enabled".to_string(),
                Ok(_) => "disabled".to_string(),
                Err(_) => "unset".to_string(),
            },
            Err(_) => "unset".to_string(),
        })
    })
    .await
    .unwrap_or_else(|_| "disabled".to_string())
}

/// Sets consent (opt-in or refusal), and applies it immediately to
/// the hook via the atomic.
#[tauri::command]
pub async fn telemetry_consent_set(app: AppHandle, enabled: bool) -> Result<(), String> {
    // The atomic first (the hook must follow right away), the file
    // off the pump (PLAN-GELS).
    ENABLED.store(enabled, Ordering::Relaxed);
    crate::commands::off_pump(app, move |app| {
        let base = base_dir(&app)?;
        std::fs::create_dir_all(&base).map_err(|err| err.to_string())?;
        let json = serde_json::to_string(&ConsentFile {
            crash_reports: enabled,
        })
        .map_err(|err| err.to_string())?;
        std::fs::write(consent_file(&base), json).map_err(|err| err.to_string())
    })
    .await
}

/// How many reports are waiting to be sent.
#[tauri::command]
pub async fn telemetry_pending(app: AppHandle) -> u32 {
    // Folder traversal: off the pump (PLAN-GELS). A join failure is
    // worth zero reports — same fallback as an unreadable folder.
    crate::commands::off_pump(app, |app| {
        if is_e2e() {
            return Ok::<_, String>(0);
        }
        let Ok(base) = base_dir(&app) else {
            return Ok::<_, String>(0);
        };
        Ok(std::fs::read_dir(crashes_dir(&base))
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
                    .count() as u32
            })
            .unwrap_or(0))
    })
    .await
    .unwrap_or(0)
}

/// Opens the reports folder in the file explorer — a way to find them
/// and send them yourself (local destination, ADR 0014).
#[tauri::command]
pub async fn telemetry_open_folder(app: AppHandle) -> Result<(), String> {
    crate::commands::off_pump(app, |app| {
        let base = base_dir(&app)?;
        let dir = crashes_dir(&base);
        std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
        std::process::Command::new("explorer")
            .arg(&dir)
            .spawn()
            .map_err(|err| err.to_string())?;
        Ok(())
    })
    .await
}

/// Triggers a panic to VERIFY the capture in the field (ADR 0014 §5).
///
/// Two definitions: in **debug** it panics, in **release** it
/// refuses — the shipped binary cannot be made to panic on command.
/// The message carries fake personal data: the written report must
/// prove it has vanished. To invoke from the WebView console
/// (`window.__TAURI__.core.invoke('telemetry_selftest_panic')`).
#[cfg(debug_assertions)]
#[tauri::command]
pub fn telemetry_selftest_panic() -> Result<(), String> {
    panic!("telemetry selftest: fake@example.com — subject \"secret\"")
}

#[cfg(not(debug_assertions))]
#[tauri::command]
pub fn telemetry_selftest_panic() -> Result<(), String> {
    Err("unavailable outside a debug build".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The guarantee that matters: the BYTES WRITTEN to disk carry no
    /// data from the panic message. The `mail-core` test proves the
    /// in-memory redaction; this one proves the real, serialized
    /// file.
    #[test]
    fn the_written_file_contains_no_data_from_the_message() {
        let dir = std::env::temp_dir().join(format!("disc-telemetry-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        let raw = RawPanic {
            message: "boom: victim@example.com — subject \"Confidential medical file\"".to_string(),
            location: Some("apps/desktop/src/commands.rs:1".to_string()),
            backtrace: vec!["wind_desktop::commands::something".to_string()],
            app_version: "0.1.2".to_string(),
            os: "windows x86_64".to_string(),
            timestamp: "2026-07-26T15:00:00Z".to_string(),
        };

        let path = persist(&dir, raw).unwrap();
        let written = std::fs::read_to_string(&path).unwrap();

        assert!(
            !written.contains("victim@example.com"),
            "the address is in the written file"
        );
        assert!(
            !written.contains('@'),
            "an at-sign remains in the written file"
        );
        assert!(
            !written.to_lowercase().contains("medical"),
            "the subject is in the written file"
        );
        // But the file KEEPS enough to locate the bug.
        assert!(
            written.contains("apps/desktop/src/commands.rs:1"),
            "the location must remain"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The runtime's secondary panic ("cannot unwind", at the
    /// WebView2 FFI boundary) does not name the original bug: we
    /// discard it. Defect found in the field — the self-test produced
    /// two reports, and the abort overwrote the first, the only
    /// useful one.
    #[test]
    fn the_runtime_secondary_panic_is_discarded() {
        assert!(is_secondary_nounwind(
            "panic in a function that cannot unwind"
        ));
        assert!(!is_secondary_nounwind(
            "index out of bounds: len 3 but index 5"
        ));
        assert!(!is_secondary_nounwind(
            "telemetry selftest: fake@example.com"
        ));
    }

    /// Two reports written in the same second do not step on each
    /// other: the `SEQ` counter makes each name unique. Without it,
    /// the double panic of a main-thread crash lost the useful
    /// report.
    #[test]
    fn two_reports_from_the_same_second_do_not_overwrite_each_other() {
        let dir = std::env::temp_dir().join(format!("disc-telemetry-seq-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        let raw = || RawPanic {
            message: "doesn't matter".to_string(),
            location: Some("apps/desktop/src/telemetry.rs:1".to_string()),
            backtrace: vec![],
            app_version: "0.1.2".to_string(),
            os: "windows x86_64".to_string(),
            timestamp: "2026-07-26T16:43:47Z".to_string(),
        };

        let one = persist(&dir, raw()).unwrap();
        let two = persist(&dir, raw()).unwrap();
        assert_ne!(one, two, "same timestamp, but distinct names");
        assert_eq!(
            std::fs::read_dir(&dir).unwrap().count(),
            2,
            "both reports coexist"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}

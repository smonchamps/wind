//! The field trace (PLAN-AUDIT-V1 E9, STANDARD §6.8): a dated line on
//! stderr AND appended to `wind.log` next to the database, bounded to
//! one meg (CE decision D4: truncated, the file starts over from
//! zero).
//!
//! Why a file: the shipped app is a *windows* subsystem, it has no
//! stderr — three updates (0.13.0 → 0.15.0) went by without any
//! measurement surviving, until the `maj.log` poka-yoke (`trace_update`).
//! The same pattern, generalized: poll, pass-after-gesture, drain,
//! watchers, unreadable horizon.
//!
//! What NEVER goes in here (§6.8): no subject, no sender, no body —
//! identifiers, durations, counts, errors.
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Beyond this, the file is truncated (D4).
pub(crate) const BYTE_LIMIT: u64 = 1_000_000;
const NAME: &str = "wind.log";

static FOLDER: OnceLock<PathBuf> = OnceLock::new();

/// To call ONCE at startup — before that, the trace only goes out on
/// stderr.
pub(crate) fn init(folder: PathBuf) {
    let _ = FOLDER.set(folder);
}

/// A trace line: stderr (console of a `cargo run`) + `wind.log`. Any
/// write error is ignored — a trace must never make the gesture it
/// describes fail.
pub(crate) fn trace(line: &str) {
    eprintln!("{line}");
    if let Some(folder) = FOLDER.get() {
        write_to(folder, line);
    }
}

pub(crate) fn write_to(folder: &Path, line: &str) {
    let _ = std::fs::create_dir_all(folder);
    let path = folder.join(NAME);
    let too_big = std::fs::metadata(&path)
        .map(|m| m.len() >= BYTE_LIMIT)
        .unwrap_or(false);
    let dated = format!(
        "{} {line}\n",
        chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ")
    );
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(!too_big)
        .write(true)
        .truncate(too_big)
        .open(&path)
        .and_then(|mut file| file.write_all(dated.as_bytes()));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_folder(name: &str) -> PathBuf {
        let folder = std::env::temp_dir().join(format!("wind-trace-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        folder
    }

    /// D4: past one meg, the file starts over from zero — never a log
    /// that grows forever next to the database.
    #[test]
    fn the_trace_is_bounded_to_one_meg() {
        let folder = temp_folder("bound");
        let line = "x".repeat(10_000);
        for _ in 0..110 {
            write_to(&folder, &line);
        }
        let size = std::fs::metadata(folder.join(NAME)).unwrap().len();
        assert!(
            size < BYTE_LIMIT + 20_000,
            "truncated past one meg: {size} bytes"
        );
        assert!(size > 0);
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// Every line is dated in UTC ISO 8601 — readable afterwards,
    /// alignable with a gesture's timestamp.
    #[test]
    fn every_line_is_dated() {
        let folder = temp_folder("dated");
        write_to(&folder, "poll account 1: INBOX 0.4s");
        let content = std::fs::read_to_string(folder.join(NAME)).unwrap();
        assert!(
            content.starts_with("20")
                && content.contains("T")
                && content.contains("Z poll account 1"),
            "{content}"
        );
        let _ = std::fs::remove_dir_all(&folder);
    }
}

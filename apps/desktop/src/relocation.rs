//! Relocation of Discovery → Wind data (PLAN-WIND E3, W-D1/W-D5).
//!
//! The switch of the app id (`dev.discovery.app` → `dev.elements.wind`)
//! changes both data folders: `%APPDATA%` (the database and its WAL
//! companions) and `%LOCALAPPDATA%` (the WebView2 profile, hence
//! localStorage). A Discovery workstation must find ALL of its state
//! again on the first Wind launch — never a reconnection, never an
//! empty database.
//!
//! The move is a `rename` per folder (same volume: atomic, zero bytes
//! copied — the field database weighs 715 MB), then `discovery.db`
//! becomes `wind.db`, companions first and the master file last: if a
//! pass dies halfway through, the retry resumes from the marker (the
//! `.db` still under its old name) without losing or overwriting
//! anything.
//!
//! Short-circuit: `WIND_DB_PATH` set (e2e harness, ADR 0014) — the
//! disposable databases of the benches have nothing to relocate.

use std::io;
use std::path::{Path, PathBuf};

/// The app id before the Wind switch. The only place in the code
/// allowed to cite it: the bridge lives as long as Discovery
/// workstations exist.
const OLD_APP_ID: &str = "dev.discovery.app";
/// Must equal `identifier` in `tauri.conf.json`.
pub(crate) const APP_ID: &str = "dev.elements.wind";

/// Relocates a Discovery workstation's data to the Wind paths.
/// Repeatable: a workstation already relocated — or a fresh one — does
/// nothing.
pub fn relocate() -> io::Result<()> {
    if std::env::var("WIND_DB_PATH").is_ok() {
        return Ok(());
    }
    for root in ["APPDATA", "LOCALAPPDATA"] {
        let Ok(base) = std::env::var(root) else {
            continue;
        };
        let base = PathBuf::from(base);
        relocate_folder(&base.join(OLD_APP_ID), &base.join(APP_ID))?;
    }
    if let Ok(base) = std::env::var("APPDATA") {
        rename_database(&PathBuf::from(base).join(APP_ID))?;
    }
    Ok(())
}

/// The whole folder, in a single `rename` — never if the target
/// already exists: it would be the result of an earlier Wind launch,
/// and overwriting it would destroy the most recent state.
fn relocate_folder(old: &Path, new: &Path) -> io::Result<()> {
    if old.is_dir() && !new.exists() {
        rename_tolerant(old, new)?;
    }
    Ok(())
}

/// A `rename` that fails while the TARGET exists and the SOURCE has
/// disappeared means the other instance did it between our check and
/// our move (PLAN-AUDIT-V1 review: the single-instance lock cannot
/// precede the relocation — it would create the target folder and
/// break it). The desired outcome is there: success, not "relocation
/// failed" on a workstation where everything went fine.
fn rename_tolerant(source: &Path, target: &Path) -> io::Result<()> {
    match std::fs::rename(source, target) {
        Ok(()) => Ok(()),
        Err(_) if target.exists() && !source.exists() => Ok(()),
        Err(err) => Err(err),
    }
}

/// `discovery.db` → `wind.db` in the relocated folder. The companions
/// (`-wal`, `-shm`, ADR 0011) go first, the `.db` last: the master
/// file is the marker of the pass — as long as it carries the old
/// name, the retry resumes where the pass died.
fn rename_database(folder: &Path) -> io::Result<()> {
    if !folder.join("discovery.db").is_file() || folder.join("wind.db").exists() {
        return Ok(());
    }
    for suffix in ["-wal", "-shm", ""] {
        let source = folder.join(format!("discovery.db{suffix}"));
        if source.is_file() {
            rename_tolerant(&source, &folder.join(format!("wind.db{suffix}")))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// PLAN-AUDIT-V1 review: two instances launched together on a
    /// Discovery workstation — the second arrives after the first's
    /// `rename`. Its own `rename` fails, but the target is there: that
    /// is a success.
    #[test]
    fn a_rename_lost_to_the_other_instance_is_a_success() {
        let root =
            std::env::temp_dir().join(format!("wind-relocation-race-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let target = root.join("dev.elements.wind");
        std::fs::create_dir_all(&target).unwrap();
        let source = root.join("dev.discovery.app");
        assert!(!source.exists());
        // The source was already moved by the other one: rename
        // fails, the target exists — tolerated.
        rename_tolerant(&source, &target).unwrap();
        // A real error (neither source nor target) stays an error.
        let nowhere = root.join("elsewhere");
        assert!(rename_tolerant(&source, &nowhere).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    fn sandbox(name: &str) -> PathBuf {
        let folder = std::env::temp_dir().join(format!(
            "wind-test-relocation-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        folder
    }

    #[test]
    fn a_discovery_workstation_relocates_folder_and_database() {
        let sandbox = sandbox("full");
        let old = sandbox.join(OLD_APP_ID);
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("discovery.db"), b"base").unwrap();
        std::fs::write(old.join("discovery.db-wal"), b"wal").unwrap();
        let new = sandbox.join(APP_ID);

        relocate_folder(&old, &new).unwrap();
        rename_database(&new).unwrap();

        assert!(!old.exists());
        assert_eq!(std::fs::read(new.join("wind.db")).unwrap(), b"base");
        assert_eq!(std::fs::read(new.join("wind.db-wal")).unwrap(), b"wal");
        assert!(!new.join("discovery.db").exists());
        let _ = std::fs::remove_dir_all(&sandbox);
    }

    #[test]
    fn an_existing_wind_workstation_is_never_overwritten() {
        let sandbox = sandbox("never-overwritten");
        let old = sandbox.join(OLD_APP_ID);
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("discovery.db"), b"old").unwrap();
        let new = sandbox.join(APP_ID);
        std::fs::create_dir_all(&new).unwrap();
        std::fs::write(new.join("wind.db"), b"recent").unwrap();

        relocate_folder(&old, &new).unwrap();
        rename_database(&new).unwrap();

        assert_eq!(std::fs::read(old.join("discovery.db")).unwrap(), b"old");
        assert_eq!(std::fs::read(new.join("wind.db")).unwrap(), b"recent");
        let _ = std::fs::remove_dir_all(&sandbox);
    }

    #[test]
    fn an_interrupted_pass_resumes_without_loss() {
        // The `-wal` companion has already crossed over to Wind, the
        // `.db` is still old: the retry must finish the move without
        // touching the already-relocated companion.
        let sandbox = sandbox("resume");
        let folder = sandbox.join(APP_ID);
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("discovery.db"), b"base").unwrap();
        std::fs::write(folder.join("wind.db-wal"), b"wal").unwrap();

        rename_database(&folder).unwrap();

        assert_eq!(std::fs::read(folder.join("wind.db")).unwrap(), b"base");
        assert_eq!(std::fs::read(folder.join("wind.db-wal")).unwrap(), b"wal");
        assert!(!folder.join("discovery.db").exists());
        let _ = std::fs::remove_dir_all(&sandbox);
    }

    #[test]
    fn a_fresh_workstation_does_nothing() {
        let sandbox = sandbox("fresh");
        let old = sandbox.join(OLD_APP_ID);
        let new = sandbox.join(APP_ID);

        relocate_folder(&old, &new).unwrap();
        rename_database(&new).unwrap();

        assert!(!old.exists());
        assert!(!new.exists());
        let _ = std::fs::remove_dir_all(&sandbox);
    }
}

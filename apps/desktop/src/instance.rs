//! Single instance via a file lock (PLAN-AUDIT-V1 E1, CE decision D1
//! of 2026-09-01): `wind.lock` next to `wind.db`, taken exclusively
//! by the first process, refused to the second. The OS releases the
//! lock when the process dies — a crash never leaves a "sticky" lock,
//! the file itself may remain, it says nothing.
//!
//! Why a file and not a plugin (single-instance): `fs4` is already
//! there (disk space guard), the lock is a pure, testable decision
//! without Tauri, and the second instance has nothing else to do than
//! say so and exit (D1: message then exit).
//!
//! `WIND_DB_PATH` (e2e, freeze probe) places the lock next to the
//! disposable database: test instances do not see each other, nor do
//! they see the real application.

use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use fs4::fs_std::FileExt;

/// The lock file, next to the database. Its content says nothing:
/// only the OS's exclusive lock matters.
pub(crate) const LOCK_NAME: &str = "wind.lock";

/// The instance guard: as long as it lives, the lock is held.
/// Releasing it (or dying) frees it.
pub(crate) struct InstanceGuard {
    _file: File,
}

/// Attempts the exclusive lock on `folder/wind.lock`. `Ok(None)`:
/// another instance already holds it. The folder is created if
/// missing (first launch: `db_path` would create it anyway).
pub(crate) fn lock(folder: &Path) -> io::Result<Option<InstanceGuard>> {
    std::fs::create_dir_all(folder)?;
    let file = File::options()
        .create(true)
        .write(true)
        .truncate(false)
        .open(folder.join(LOCK_NAME))?;
    if file.try_lock_exclusive()? {
        Ok(Some(InstanceGuard { _file: file }))
    } else {
        Ok(None)
    }
}

/// The database folder WITHOUT `AppHandle` — the lock is taken before
/// Tauri builds anything (the window is born before `setup`, tauri
/// `app.rs`: a second instance checking in `setup` would make a
/// window flicker). Same rule as `commands::db_path`: `WIND_DB_PATH`
/// first, otherwise `%APPDATA%\<app id>` — what `app_data_dir()`
/// returns on Windows.
pub(crate) fn database_folder() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("WIND_DB_PATH") {
        return PathBuf::from(path).parent().map(Path::to_path_buf);
    }
    let appdata = std::env::var("APPDATA").ok()?;
    Some(PathBuf::from(appdata).join(crate::relocation::APP_ID))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_folder(name: &str) -> std::path::PathBuf {
        let folder =
            std::env::temp_dir().join(format!("wind-instance-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        folder
    }

    /// The core of the guard: two attempts on the same folder, the
    /// second is refused as long as the first lives. On Windows,
    /// exclusive LockFileEx is refused PER HANDLE — the test fits in
    /// one process.
    #[test]
    fn two_locks_on_the_same_folder_the_second_is_refused() {
        let folder = temp_folder("double");
        let first = lock(&folder).unwrap();
        assert!(first.is_some(), "the first instance must get the lock");
        let second = lock(&folder).unwrap();
        assert!(second.is_none(), "the second instance must be refused");
        drop(first);
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// Releasing the guard (end of the first instance) makes the lock
    /// available again: no sticky lock.
    #[test]
    fn a_released_lock_can_be_taken_again() {
        let folder = temp_folder("release");
        let first = lock(&folder).unwrap();
        assert!(first.is_some());
        drop(first);
        let next = lock(&folder).unwrap();
        assert!(
            next.is_some(),
            "after the first ends, the next one gets the lock"
        );
        drop(next);
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// The database folder may not exist yet (first launch): the
    /// guard creates it, as `db_path` does.
    #[test]
    fn the_missing_folder_is_created() {
        let folder = temp_folder("missing").join("sub-folder");
        assert!(!folder.exists());
        let guard = lock(&folder).unwrap();
        assert!(guard.is_some());
        assert!(folder.join(LOCK_NAME).is_file());
        drop(guard);
        let _ = std::fs::remove_dir_all(folder.parent().unwrap());
    }
}

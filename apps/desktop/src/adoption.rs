//! The adopted database and the blocking boundary (PLAN-AUDIT-2026-09
//! Lot 5, E13a/E13b — audit A03/A07, D-9).
//!
//! Two structural nets replace an ordering convention and a text guard:
//!
//! - **Adoption.** Every command used to open the database at
//!   `db_path()`, so only the UI's discipline (`migration_check` before
//!   anything else, ADR 0012) kept a legacy database from being adopted
//!   silently, in a freeze, by the first command to come along; and a
//!   file replaced at the same path after that check (a restore, a hand
//!   copy) reused the memoized path as if nothing had happened. The
//!   shell now records the file's identity at adoption and hands the
//!   path out only while the file on disk is that one — [`Adoption`].
//! - **Blocking.** The path of the adopted database is only reachable
//!   through a [`Blocking`] token that `off_pump` builds on its
//!   `spawn_blocking` thread: an async body cannot name the database by
//!   construction. Dedicated threads (the scheduler, the IDLE watcher)
//!   build theirs with [`Blocking::on_dedicated_thread`] — the one
//!   escape hatch, and the one name the text guard still watches.

use std::hash::{Hash, Hasher};
use std::io;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tauri::{AppHandle, Manager};

/// What the shell remembers of the file it adopted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Identity {
    /// Adopted while the file did not exist yet (first install): the
    /// first full opening creates it, the next check records it.
    Fresh,
    /// The file system's identity of the database file (volume + index
    /// on Windows, device + inode elsewhere), hashed and dropped — no
    /// handle is kept open on the database.
    File(u64),
}

fn identity_of(path: &Path) -> io::Result<Identity> {
    // One syscall, not `exists()` then open (review 2026-09-07: a file
    // deleted between the two read as an I/O failure, not as absent).
    let handle = match same_file::Handle::from_path(path) {
        Ok(handle) => handle,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Identity::Fresh),
        Err(err) => return Err(err),
    };
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    handle.hash(&mut hasher);
    Ok(Identity::File(hasher.finish()))
}

/// Why the adopted path is refused.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum AdoptionError {
    /// No adoption yet: `migration_check` (then `migration_run` when a
    /// pass is pending) has not run — the command came too early.
    NotAdopted,
    /// The file at the adopted path is not the adopted file any more:
    /// the record is cleared, the probe must run again.
    Replaced,
    Io(String),
}

impl std::fmt::Display for AdoptionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAdopted => write!(f, "database not adopted: migration_check first"),
            Self::Replaced => write!(
                f,
                "database file replaced under the application: adopt it again"
            ),
            Self::Io(err) => write!(f, "database identity: {err}"),
        }
    }
}

impl From<io::Error> for AdoptionError {
    fn from(err: io::Error) -> Self {
        Self::Io(err.to_string())
    }
}

/// The record of the adopted database file. `None` until adopted.
#[derive(Default)]
pub(crate) struct Adoption {
    recorded: Mutex<Option<Identity>>,
}

impl Adoption {
    /// Records the file at `path` as the adopted database (after the
    /// probe found nothing pending, or after the visible pass ran).
    pub(crate) fn adopt(&self, path: &Path) -> Result<(), AdoptionError> {
        let identity = identity_of(path)?;
        *crate::commands::recovered(&self.recorded) = Some(identity);
        Ok(())
    }

    /// Is the file at `path` the adopted one? One `stat` per call.
    /// A replaced file clears the record before refusing.
    pub(crate) fn check(&self, path: &Path) -> Result<(), AdoptionError> {
        let mut recorded = crate::commands::recovered(&self.recorded);
        let Some(identity) = *recorded else {
            return Err(AdoptionError::NotAdopted);
        };
        let current = identity_of(path)?;
        match (identity, current) {
            (Identity::Fresh, Identity::Fresh) => Ok(()),
            (Identity::Fresh, file @ Identity::File(_)) => {
                *recorded = Some(file);
                Ok(())
            }
            (Identity::File(_), Identity::Fresh) => {
                *recorded = None;
                Err(AdoptionError::Replaced)
            }
            (Identity::File(before), Identity::File(now)) if before == now => Ok(()),
            (Identity::File(_), Identity::File(_)) => {
                *recorded = None;
                Err(AdoptionError::Replaced)
            }
        }
    }
}

/// Proof that the caller runs off the message pump, on a blocking
/// thread: built by `off_pump` for its closure, and by the dedicated
/// threads for themselves. Derefs to the [`AppHandle`] so the closures
/// keep their shape.
#[derive(Clone)]
pub(crate) struct Blocking(AppHandle);

impl Blocking {
    pub(crate) fn from_off_pump(app: AppHandle) -> Self {
        Self(app)
    }

    /// For a blocking thread the shell drives itself — `std::thread::spawn`
    /// (the scheduler, the IDLE watcher) or a bare `spawn_blocking` that
    /// takes the commands' lock by hand (the account repair and removal
    /// workers) — never an async worker. The text guard lists this name:
    /// an async command body must not contain it.
    pub(crate) fn on_dedicated_thread(app: AppHandle) -> Self {
        Self(app)
    }

    pub(crate) fn handle(&self) -> &AppHandle {
        &self.0
    }
}

impl Deref for Blocking {
    type Target = AppHandle;

    fn deref(&self) -> &AppHandle {
        &self.0
    }
}

/// The path of the adopted database — the only way a blocking body
/// reaches `Store::open`.
///
/// A file replaced under the application is probed again on the spot:
/// nothing pending → adopted anew and the command goes on (the user
/// sees nothing); a pass pending → refused with the reason, the UI's
/// migration screen is the way back (review 2026-09-07: the first cut
/// left the app refusing everything until a restart).
pub(crate) fn adopted_db(blocking: &Blocking) -> Result<PathBuf, String> {
    let path = crate::commands::probe_path(blocking.handle())?;
    let adoption = &blocking.state::<crate::AppState>().migration.adopted;
    match adoption.check(&path) {
        Ok(()) => Ok(path),
        Err(AdoptionError::Replaced) => {
            match mail_core::Store::pending_adoption(&path).map_err(|err| err.to_string())? {
                None => {
                    adoption.adopt(&path).map_err(|err| err.to_string())?;
                    Ok(path)
                }
                Some(pending) => Err(format!(
                    "database file replaced under the application and {pending} messages await adoption: run the migration again"
                )),
            }
        }
        Err(err) => Err(err.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "wind-adoption-{name}-{}-{suffix}.db",
            std::process::id()
        ))
    }

    #[test]
    fn before_any_adoption_the_path_is_refused() {
        let path = scratch("early");
        std::fs::write(&path, b"database").unwrap();
        let adoption = Adoption::default();
        assert_eq!(adoption.check(&path), Err(AdoptionError::NotAdopted));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn the_adopted_file_stays_accepted_while_it_grows() {
        let path = scratch("grows");
        std::fs::write(&path, b"database").unwrap();
        let adoption = Adoption::default();
        adoption.adopt(&path).unwrap();
        assert_eq!(adoption.check(&path), Ok(()));
        // A database grows and is rewritten in place: same file.
        std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b" and more pages")
            .unwrap();
        assert_eq!(adoption.check(&path), Ok(()));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_file_replaced_at_the_same_path_is_refused_once_then_needs_adoption() {
        let path = scratch("replaced");
        std::fs::write(&path, b"first database").unwrap();
        let adoption = Adoption::default();
        adoption.adopt(&path).unwrap();
        // A restore: another file renamed over the adopted one.
        let other = scratch("other");
        std::fs::write(&other, b"restored database").unwrap();
        std::fs::rename(&other, &path).unwrap();
        assert_eq!(adoption.check(&path), Err(AdoptionError::Replaced));
        // The record is cleared: the probe must run again.
        assert_eq!(adoption.check(&path), Err(AdoptionError::NotAdopted));
        adoption.adopt(&path).unwrap();
        assert_eq!(adoption.check(&path), Ok(()));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_first_install_is_adopted_before_the_file_exists() {
        let path = scratch("fresh");
        let _ = std::fs::remove_file(&path);
        let adoption = Adoption::default();
        adoption.adopt(&path).unwrap();
        assert_eq!(adoption.check(&path), Ok(()));
        // The first full opening creates the file: it becomes the one.
        std::fs::write(&path, b"created by the first open").unwrap();
        assert_eq!(adoption.check(&path), Ok(()));
        let other = scratch("fresh-other");
        std::fs::write(&other, b"another").unwrap();
        std::fs::rename(&other, &path).unwrap();
        assert_eq!(adoption.check(&path), Err(AdoptionError::Replaced));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_deleted_file_is_a_replacement() {
        let path = scratch("deleted");
        std::fs::write(&path, b"database").unwrap();
        let adoption = Adoption::default();
        adoption.adopt(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(adoption.check(&path), Err(AdoptionError::Replaced));
    }

    use std::io::Write;
}

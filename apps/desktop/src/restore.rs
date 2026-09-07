//! Restoring the database from a copy (PLAN-AUDIT-2026-09 Lot 5, E14b —
//! audit G02, Chief Engineer decision D3).
//!
//! A copy is what `Store::snapshot_into` wrote: one SQLite file, no
//! secret (the keyring holds them). Restoring it is a swap of files that
//! nothing may be reading: it is STAGED while Wind runs (the copy is
//! validated and placed next to the database as `wind.db.restore`) and
//! APPLIED at the next start, before the instance lock lets anything
//! open the database. The replaced database is kept beside it, never
//! deleted. The outbox of the restored copy is HELD: a marker file asks
//! the first adoption to hold every send that could still go out (the
//! copy may predate a send the user made since, or one they gave up).

use std::io;
use std::path::{Path, PathBuf};

/// The staged copy, next to the database.
const PENDING_SUFFIX: &str = ".restore";
/// The marker that asks the next adoption to hold the outbox.
const HOLD_SUFFIX: &str = ".hold-sends";

fn sibling(db: &Path, suffix: &str) -> PathBuf {
    let mut name = db.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(suffix);
    db.with_file_name(name)
}

/// Stages `snapshot` for the next start: validated by the core's
/// read-only probe (`Store::inspect_copy`: SQLite header, `accounts`
/// table, a threading version this build knows), then copied next to
/// the database. Nothing else moves while Wind runs.
pub(crate) fn stage(db: &Path, snapshot: &Path) -> io::Result<PathBuf> {
    mail_core::Store::inspect_copy(snapshot)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
    let pending = sibling(db, PENDING_SUFFIX);
    let partial = sibling(db, ".restore.partial");
    std::fs::copy(snapshot, &partial)?;
    std::fs::rename(&partial, &pending)?;
    Ok(pending)
}

/// What `apply_pending` did.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Applied {
    /// Where the replaced database went (kept, never deleted).
    pub replaced: Option<PathBuf>,
}

/// At start, before anything opens the database: if a copy is staged,
/// the current database steps aside (`wind.db.replaced-<epoch>`, its
/// WAL and shm removed with it — they belong to the old file) and the
/// copy takes its place; the hold marker is written for the adoption.
/// Nothing staged: `None`, nothing touched.
pub(crate) fn apply_pending(db: &Path) -> io::Result<Option<Applied>> {
    let pending = sibling(db, PENDING_SUFFIX);
    if !pending.is_file() {
        return Ok(None);
    }
    // The marker FIRST (review 2026-09-07): written last, a failure
    // after the swap left the copy in place with its outbox free to go
    // out. And a swap that fails takes its marker back with it: nothing
    // was restored, the database's own sends stay its own.
    let marker = sibling(db, HOLD_SUFFIX);
    std::fs::write(&marker, b"hold")?;
    let swap = || -> io::Result<Option<PathBuf>> {
        let replaced = if db.exists() {
            let epoch = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let aside = sibling(db, &format!(".replaced-{epoch}"));
            std::fs::rename(db, &aside)?;
            Some(aside)
        } else {
            None
        };
        if let Err(err) = std::fs::rename(&pending, db) {
            // The copy could not take the place: the database steps
            // back in — never a path that opens as a first install.
            if let Some(aside) = &replaced {
                let _ = std::fs::rename(aside, db);
            }
            return Err(err);
        }
        // The old file's WAL and shm belong to it, not to the copy.
        for suffix in ["-wal", "-shm"] {
            let _ = std::fs::remove_file(sibling(db, suffix));
        }
        Ok(replaced)
    };
    match swap() {
        Ok(replaced) => Ok(Some(Applied { replaced })),
        Err(err) => {
            let _ = std::fs::remove_file(&marker);
            Err(err)
        }
    }
}

/// After the adoption of the database: if the hold marker is there,
/// every send that could still go out is held, and the marker goes.
/// Returns how many were held (0 when no marker).
pub(crate) fn hold_if_marked(db: &Path) -> Result<usize, String> {
    let marker = sibling(db, HOLD_SUFFIX);
    if !marker.is_file() {
        return Ok(0);
    }
    let store = mail_core::Store::open(db).map_err(|err| err.to_string())?;
    let held = store.hold_pending_sends().map_err(|err| err.to_string())?;
    std::fs::remove_file(&marker).map_err(|err| err.to_string())?;
    Ok(held)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "wind-restore-{name}-{}-{suffix}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("wind.db")
    }

    /// A copy as "Save a copy" writes it: a snapshot of a database with
    /// one send queued — self-contained, no WAL sidecar.
    fn copy_with_a_queued_send(copy: &Path) {
        let live = copy.with_file_name("source-of-copy.db");
        let store = mail_core::Store::open(&live).unwrap();
        let account = store
            .adopt_or_create_account("me@example.fr", "gmail")
            .unwrap();
        let draft = mail_core::compose(
            "me@example.fr",
            "you@example.fr",
            "",
            "",
            "queued in the copy",
            "body",
            None,
        )
        .unwrap();
        store.enqueue_outbox(account, &draft).unwrap();
        store.snapshot_into(copy).unwrap();
    }

    #[test]
    fn a_text_file_is_not_a_copy() {
        let db = scratch("text");
        let fake = db.with_file_name("fake.db");
        std::fs::write(&fake, b"not a database at all").unwrap();
        assert!(stage(&db, &fake).is_err());
        assert!(!sibling(&db, PENDING_SUFFIX).exists(), "nothing staged");
    }

    /// A swap that fails takes its marker back: nothing restored, the
    /// database's own sends must not be held (review 2026-09-07). The
    /// deterministic refusal is Windows': a handle opened without any
    /// share mode makes the rename of the database fail.
    #[cfg(windows)]
    #[test]
    fn a_failed_swap_holds_nothing_and_puts_the_database_back() {
        use std::os::windows::fs::OpenOptionsExt;
        let db = scratch("failed-swap");
        copy_with_a_queued_send(&db);
        let before = std::fs::read(&db).unwrap();
        let copy = db.with_file_name("copy.db");
        copy_with_a_queued_send(&copy);
        stage(&db, &copy).unwrap();
        let exclusive = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&db)
            .unwrap();
        assert!(
            apply_pending(&db).is_err(),
            "the swap is refused while the database is held"
        );
        drop(exclusive);
        assert!(
            !sibling(&db, HOLD_SUFFIX).exists(),
            "no marker after a failed swap"
        );
        assert_eq!(
            std::fs::read(&db).unwrap(),
            before,
            "the database is what it was"
        );
        assert!(
            sibling(&db, PENDING_SUFFIX).is_file(),
            "the copy stays staged for another try"
        );
    }

    #[test]
    fn nothing_staged_touches_nothing() {
        let db = scratch("idle");
        copy_with_a_queued_send(&db);
        let before = std::fs::read(&db).unwrap();
        assert_eq!(apply_pending(&db).unwrap(), None);
        assert_eq!(std::fs::read(&db).unwrap(), before);
    }

    #[test]
    fn a_staged_copy_replaces_the_database_at_start_and_holds_its_sends() {
        let db = scratch("swap");
        // The live database, with its own content and a WAL sidecar.
        let live = mail_core::Store::open(&db).unwrap();
        live.adopt_or_create_account("live@example.fr", "gmail")
            .unwrap();
        drop(live);
        std::fs::write(sibling(&db, "-wal"), b"stale wal").unwrap();
        // The copy, with a send queued in another life.
        let copy = db.with_file_name("copy.db");
        copy_with_a_queued_send(&copy);
        let pending = stage(&db, &copy).unwrap();
        assert!(pending.is_file());
        // Before the start: nothing moved.
        assert!(
            mail_core::Store::open(&db)
                .unwrap()
                .accounts()
                .unwrap()
                .iter()
                .any(|a| a.email == "live@example.fr")
        );

        let applied = apply_pending(&db).unwrap().expect("a copy was staged");
        let aside = applied.replaced.expect("the live database stepped aside");
        assert!(aside.is_file(), "the replaced database is kept");
        assert!(
            !sibling(&db, "-wal").exists(),
            "the old WAL does not haunt the copy"
        );
        assert!(!pending.exists());
        let store = mail_core::Store::open(&db).unwrap();
        assert!(
            store
                .accounts()
                .unwrap()
                .iter()
                .any(|a| a.email == "me@example.fr"),
            "the copy is the database now"
        );
        assert_eq!(
            store.outbox().unwrap()[0].state,
            mail_core::OutboxState::Queued,
            "held only at adoption"
        );
        drop(store);

        assert_eq!(hold_if_marked(&db).unwrap(), 1, "the copy's send is held");
        assert_eq!(hold_if_marked(&db).unwrap(), 0, "the marker is consumed");
        let store = mail_core::Store::open(&db).unwrap();
        assert_eq!(
            store.outbox().unwrap()[0].state,
            mail_core::OutboxState::Held
        );
    }
}

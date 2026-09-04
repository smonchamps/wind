use super::*;

/// Prefixes of the prefs suffixed per account (`{prefix}.{account_id}`).
/// THE list `delete_account` purges: `accounts.id` is an INTEGER
/// PRIMARY KEY without AUTOINCREMENT — SQLite reuses the largest freed
/// rowid, and an account added after a removal would otherwise inherit
/// the old one's identity (review PLAN-RETOURS-8). Any new per-account
/// pref is added HERE, not at a call site (review 2026-08-23: the list
/// lived hardcoded in the query, a crate away from the helpers that hit
/// the keys).
pub const PREFS_PER_ACCOUNT: &[&str] = &[
    "signature",
    "signature_replies",
    "repere_icone",
    "repere_teinte",
    "nom_compte",
    "horizon_import",
];

impl Store {
    /// Reads a preference WITHOUT opening the database — a probe in
    /// **read-only** mode, sibling of [`Store::pending_adoption`]:
    /// nothing is triggered, nothing is created. This is what lets the
    /// desktop restore the language BEFORE the migration screen (ADR
    /// 0012) — with a full open, adopting a legacy database was paid
    /// for silently while loading the language, with no modal (field
    /// finding 2026-08-15).
    ///
    /// Accepted limit (same-day review): after an abrupt stop, a hot
    /// `-wal` file can make read-only opening impossible — the probe
    /// then fails instead of recovering the journal the way a full
    /// open would. The UI treats this failure as a SESSION fallback
    /// (system language), never as an absence of preference: nothing
    /// gets persisted on the strength of a silent probe.
    pub fn text_pref_readonly(path: &Path, key: &str) -> Result<Option<String>, Error> {
        if !path.exists() {
            // First install: nothing to read, and opening would create
            // the file — a probe leaves no trace.
            return Ok(None);
        }
        let conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        // The same wait budget as a full open: a database from BEFORE
        // WAL is in rollback mode, where a writer blocks readers —
        // without this budget, the probe would die with SQLITE_BUSY on
        // the first try (late beats dead).
        conn.busy_timeout(std::time::Duration::from_secs(30))?;
        // A database from before preferences may not have the table:
        // the probe must answer ("no preference"), not explain.
        let has_prefs: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'prefs'",
            [],
            |row| row.get(0),
        )?;
        if has_prefs == 0 {
            return Ok(None);
        }
        let value = conn
            .query_row(
                "SELECT value FROM prefs WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?;
        Ok(value)
    }

    /// Boolean preference persisted in the database. Absent = `default`:
    /// a preference never touched writes nothing — the database only
    /// carries explicit choices.
    pub fn bool_pref(&self, key: &str, default: bool) -> Result<bool, Error> {
        let value: Option<String> = self
            .0
            .query_row(
                "SELECT value FROM prefs WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?;
        Ok(value.map_or(default, |v| v == "1"))
    }

    pub fn set_bool_pref(&self, key: &str, value: bool) -> Result<(), Error> {
        self.0.execute(
            "INSERT INTO prefs (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, if value { "1" } else { "0" }],
        )?;
        Ok(())
    }

    /// Text preference persisted in the database — the counterpart of
    /// `bool_pref` for named values (the UI language, PLAN-LANGUES).
    /// Absent = `None`: a preference never touched writes nothing, it
    /// is the caller that knows its default.
    pub fn text_pref(&self, key: &str) -> Result<Option<String>, Error> {
        let value = self
            .0
            .query_row(
                "SELECT value FROM prefs WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?;
        Ok(value)
    }

    pub fn set_text_pref(&self, key: &str, value: &str) -> Result<(), Error> {
        self.0.execute(
            "INSERT INTO prefs (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// An account's import history (PLAN-HORIZON-NETTOYAGE D1/D3) —
    /// pref `horizon_import.{id}`, vocabulary
    /// [`crate::backfill::HORIZONS_IMPORT`]. With no pref, or on a
    /// value outside the vocabulary: "tout" (D4 — an account from
    /// before the setting, or a corrupted pref, imports everything;
    /// never a silent loss).
    pub fn horizon_import(&self, account_id: i64) -> Result<String, Error> {
        Ok(self
            .text_pref(&format!("horizon_import.{account_id}"))?
            .filter(|v| crate::backfill::HORIZONS_IMPORT.contains(&v.as_str()))
            .unwrap_or_else(|| "tout".to_string()))
    }

    /// Sets the import history — the door validates the vocabulary
    /// BEFORE writing (same rule as `validate_routing`: a vocabulary
    /// with a hole does not hide behind another refusal).
    pub fn set_horizon_import(&self, account_id: i64, value: &str) -> Result<(), Error> {
        if !crate::backfill::HORIZONS_IMPORT.contains(&value) {
            return Err(Error::Corrupt(format!("unknown history: {value:?}")));
        }
        self.set_text_pref(&format!("horizon_import.{account_id}"), value)
    }

    /// Several text preferences at ONCE, transactionally: keys that
    /// only make sense together (the icon AND the hue of an account
    /// marker) must never end up half-written — a failure between the
    /// two would leave a pair nobody chose (review PLAN-RETOURS-8,
    /// 2026-08-22).
    pub fn set_text_prefs(&mut self, prefs: &[(&str, &str)]) -> Result<(), Error> {
        let tx = self.0.transaction()?;
        for (key, value) in prefs {
            tx.execute(
                "INSERT INTO prefs (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
}

//! The navigation of screen 02 (v2 redesign, PLAN-UI-V2 §P2): the six
//! canonical folders of the prototype — inbox, sent, drafts,
//! junk, archives, trash — resolved onto the REAL mailboxes,
//! their counters, and the list pages per category.
//!
//! The classification is POSITIONAL — a lesson from the field
//! (`diag_mailboxes`): a plain `contains()` gave 26 “archive”
//! candidates on a Gmail account carrying a PST migration. Only the
//! LAST segment counts, and the folder must live at the root or under
//! the sole provider prefix (`[Gmail]/x`) — never deeper. With
//! multiple candidates, the provider prefix wins over the root
//! homonym. “Sent” is not guessed: `accounts.sent_mailbox` is the
//! authority (ADR 0009 §7). The observed separator is `/`; a server
//! with an exotic separator degrades cleanly — only the roots match.
//!
//! Everything is READ-ONLY: the nav shows a state, nothing more (ADR 0001).

use std::collections::HashMap;

use rusqlite::{OptionalExtension, params, params_from_iter};

use crate::error::Error;
use crate::store::{
    InvitationRank, PINNED_THREADS, SELECT_UNIFIED, Store, THREAD_AGGREGATE, UnifiedRow,
    routing_page_sql, row_to_threaded, thread_route_sql, unified_page_sql,
};
use crate::thread::RECEIVED_MAILBOX;

/// RETOURS-14 R6 (D7): a Paper trail group — the sender, the
/// number of its threads, the recency and the subject of the last
/// message (the rank of the grouped view).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaperTrailGroup {
    pub address: String,
    pub threads: u64,
    pub last_epoch: i64,
    pub who: Option<String>,
    pub last_subject: Option<String>,
}

/// The canonical folders of ONE account, in WIRE names (`folders.wire`,
/// the same vocabulary as `sync_state`). `None` = the category has no
/// recognized folder on this account — the nav shows it empty, never a
/// bad choice (“an unknown name beats a bad choice”, `mail-imap`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalFolders {
    pub inbox: String,
    pub sent: Option<String>,
    pub drafts: Option<String>,
    pub junk: Option<String>,
    pub archives: Option<String>,
    /// True when `archives` is a FULL mailbox (“All Mail”):
    /// the category must then exclude the messages of the other
    /// canonicals, otherwise it shows the whole mailbox.
    pub archives_full: bool,
    pub trash: Option<String>,
}

/// The nav counters for ONE account. Inbox and junk carry the
/// prototype's unread hero; the others, a plain total.
/// Inbox counts CONVERSATIONS (that is what the list shows);
/// the other categories count messages.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct NavCounts {
    pub inbox_total: u64,
    pub inbox_unread: u64,
    pub sent: u64,
    pub drafts: u64,
    pub junk_total: u64,
    pub junk_unread: u64,
    pub archives: u64,
    pub trash: u64,
}

impl CanonicalFolders {
    /// The mailbox of a category, ready for `sync_state` — `None` when
    /// the category is not resolved on this account, or unknown.
    pub fn mailbox(&self, category: &str) -> Option<String> {
        match category {
            "reception" => Some(self.inbox.clone()),
            "envoyes" => self.sent.clone(),
            "brouillons" => self.drafts.clone(),
            "indesirables" => self.junk.clone(),
            "archives" => self.archives.clone(),
            "corbeille" => self.trash.clone(),
            _ => None,
        }
    }
}

const DRAFTS: &[&str] = &["drafts", "brouillons"];
const JUNK: &[&str] = &[
    "spam",
    "junk",
    "junk e-mail",
    "courrier indésirable",
    "indésirables",
];
const TRASH: &[&str] = &[
    "trash",
    "corbeille",
    "deleted",
    "deleted items",
    "éléments supprimés",
];
const ARCHIVES: &[&str] = &["archive", "archives"];
// The FULL mailboxes: Gmail's “All Mail” contains EVERYTHING — inbox
// included. Serving it as is in Archives would show the whole mailbox
// (a defect seen in the field, 2026-08-12): a full mailbox is only an
// archive once it is stripped of the messages living in another
// canonical.
const FULL_MAILBOXES: &[&str] = &["all mail", "tous les messages"];

/// Root, or exactly one level under `[Gmail]` — nothing deeper.
fn canonical_leaf(display: &str) -> Option<(bool, String)> {
    let segments: Vec<&str> = display.split('/').collect();
    match segments.as_slice() {
        [only] => Some((false, only.to_lowercase())),
        [prefix, leaf] if prefix.eq_ignore_ascii_case("[Gmail]") => {
            Some((true, leaf.to_lowercase()))
        }
        _ => None,
    }
}

/// The exclusion clause of a full mailbox: a message whose
/// `message_id` ALSO lives in one of the given mailboxes is not
/// archived. The identifiers come from OUR base (i64): literal
/// inclusion, the partial index `idx_envelopes_message` carries the
/// probe. A message with no `message_id` stays: it cannot be proven
/// to live elsewhere.
fn exclusion_clause(exclude: &[i64]) -> String {
    if exclude.is_empty() {
        return String::new();
    }
    let list = exclude
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(",");
    format!(
        " AND (e.message_id IS NULL OR NOT EXISTS (
             SELECT 1 FROM envelopes x
             WHERE x.message_id = e.message_id AND x.mailbox_id IN ({list})))"
    )
}

fn keep(folders: &[(String, String)], patterns: &[&str]) -> Option<String> {
    let candidates: Vec<&(String, String)> = folders
        .iter()
        .filter(|(_, display)| {
            canonical_leaf(display).is_some_and(|(_, leaf)| patterns.contains(&leaf.as_str()))
        })
        .collect();
    candidates
        .iter()
        .find(|(_, display)| canonical_leaf(display).is_some_and(|(gmail, _)| gmail))
        .or_else(|| candidates.first())
        .map(|(wire, _)| wire.clone())
}

impl Store {
    /// Resolves the six canonical folders of an account from the
    /// `folders` cache (filled by the sync) and `accounts.sent_mailbox`.
    pub fn canonical_folders(&self, account_id: i64) -> Result<CanonicalFolders, Error> {
        // An unreadable base REMOUNTS the error (PLAN-AUDIT-V2 E5):
        // `unwrap_or(None)` turned a locked base into “no Sent”, threads
        // silently un-reconciled.
        let sent: Option<String> = self
            .conn()
            .query_row(
                "SELECT sent_mailbox FROM accounts WHERE id = ?1",
                params![account_id],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
        let folders: Vec<(String, String, Option<String>)> = self
            .conn()
            .prepare(
                "SELECT wire, display, special_use FROM folders
                 WHERE account_id = ?1 AND selectable ORDER BY display",
            )?
            .query_map(params![account_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<Result<_, _>>()?;
        // The role announced by the server (RFC 6154) first — that is
        // what it KNOWS; the name as a fallback, for a server that
        // announces nothing.
        let by_role = |role: crate::SpecialUse| {
            folders
                .iter()
                .find(|(_, _, code)| code.as_deref() == Some(role.code()))
                .map(|(wire, _, _)| wire.clone())
        };
        let names: Vec<(String, String)> = folders
            .iter()
            .map(|(wire, display, _)| (wire.clone(), display.clone()))
            .collect();
        // A PURE archives folder first; the Gmail full mailbox as a
        // fallback, marked as such.
        let (archives, archives_full) =
            match by_role(crate::SpecialUse::Archive).or_else(|| keep(&names, ARCHIVES)) {
                Some(pure) => (Some(pure), false),
                None => {
                    match by_role(crate::SpecialUse::All).or_else(|| keep(&names, FULL_MAILBOXES)) {
                        Some(full) => (Some(full), true),
                        None => (None, false),
                    }
                }
            };
        Ok(CanonicalFolders {
            inbox: RECEIVED_MAILBOX.to_string(),
            sent,
            drafts: by_role(crate::SpecialUse::Drafts).or_else(|| keep(&names, DRAFTS)),
            junk: by_role(crate::SpecialUse::Junk).or_else(|| keep(&names, JUNK)),
            archives,
            archives_full,
            trash: by_role(crate::SpecialUse::Trash).or_else(|| keep(&names, TRASH)),
        })
    }

    /// The `mailbox_id` of the canonicals OTHER than archives — the
    /// exclusion clause of a full mailbox: a message present in one of
    /// them is not archived.
    pub fn canonical_except_archive(
        &self,
        account_id: i64,
        folders: &CanonicalFolders,
    ) -> Result<Vec<i64>, Error> {
        let mut ids = Vec::new();
        let names = [
            Some(folders.inbox.as_str()),
            folders.sent.as_deref(),
            folders.drafts.as_deref(),
            folders.junk.as_deref(),
            folders.trash.as_deref(),
        ];
        for name in names.into_iter().flatten() {
            if let Some(state) = self.sync_state(account_id, name)? {
                ids.push(state.mailbox_id);
            }
        }
        Ok(ids)
    }

    /// `(total, unread)` of the messages of a mailbox designated by its
    /// network name. A mailbox never synced counts zero — no error: the
    /// nav shows before the first sync. `exclude`: the mailboxes whose
    /// presence of the same `message_id` disqualifies (the full mailbox
    /// stripped of the other canonicals).
    fn mailbox_account(
        &self,
        account_id: i64,
        name: Option<&str>,
        exclude: &[i64],
    ) -> Result<(u64, u64), Error> {
        let Some(name) = name else { return Ok((0, 0)) };
        let Some(state) = self.sync_state(account_id, name)? else {
            return Ok((0, 0));
        };
        let sql = format!(
            "SELECT COUNT(*), COALESCE(SUM(NOT e.seen), 0)
             FROM envelopes e WHERE e.mailbox_id = ?1{}",
            exclusion_clause(exclude)
        );
        let (total, unread): (i64, i64) =
            self.conn()
                .query_row(&sql, params![state.mailbox_id], |row| {
                    Ok((row.get(0)?, row.get(1)?))
                })?;
        Ok((total as u64, unread as u64))
    }

    /// The nav counters of an account, on its resolved folders.
    pub fn nav_counts(
        &self,
        account_id: i64,
        folders: &CanonicalFolders,
    ) -> Result<NavCounts, Error> {
        let (inbox_total, inbox_unread): (i64, i64) = self.conn().query_row(
            "SELECT COUNT(*), COALESCE(SUM(unseen > 0), 0)
             FROM threads WHERE account_id = ?1 AND inbox_size > 0",
            params![account_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let (sent, _) = self.mailbox_account(account_id, folders.sent.as_deref(), &[])?;
        // PLAN-BROUILLONS (B-D1): the Drafts folder shows the LOCAL
        // drafts — the counter counts the same thing, never the mirror
        // IMAP mailbox: a figure that does not match what the click
        // opens would be a nav lie.
        let drafts: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM drafts WHERE account_id = ?1",
            params![account_id],
            |row| row.get(0),
        )?;
        let (junk_total, junk_unread) =
            self.mailbox_account(account_id, folders.junk.as_deref(), &[])?;
        let archives_exclusion = if folders.archives_full {
            self.canonical_except_archive(account_id, folders)?
        } else {
            Vec::new()
        };
        let (archives, _) =
            self.mailbox_account(account_id, folders.archives.as_deref(), &archives_exclusion)?;
        let (trash, _) = self.mailbox_account(account_id, folders.trash.as_deref(), &[])?;
        // The local echoes (PLAN-REACTIVITE E3) count along with the
        // envelopes: the counter says what the click opens — never two
        // truths between the nav and the list.
        Ok(NavCounts {
            inbox_total: inbox_total as u64,
            inbox_unread: inbox_unread as u64,
            sent: sent + self.count_echos("envoyes", Some(account_id))?,
            drafts: drafts as u64,
            junk_total,
            junk_unread,
            archives: archives + self.count_echos("archives", Some(account_id))?,
            trash: trash + self.count_echos("corbeille", Some(account_id))?,
        })
    }

    /// The only two counters the nav SHOWS (A29: the nav only says the
    /// unread) — the path of the 10 s probe. The eight counters of
    /// [`Store::nav_counts`] remain the complete inventory (invariant
    /// tests, upcoming screens) but are no longer paid at the nav's
    /// pace: the archives total of a full mailbox (a `NOT EXISTS` probe
    /// per row) cost ~240 ms per account and per probe, standing in
    /// front of every first render (field, 2026-08-20,
    /// PLAN-DEFILEMENT-PROFOND).
    pub fn nav_unread_counts(
        &self,
        account_id: i64,
        folders: &CanonicalFolders,
        organized: bool,
    ) -> Result<(u64, u64), Error> {
        // E2/E5: in organized mode, the unread count of a thread held
        // at the Screener, routed to the Feed or SET ASIDE does not
        // inflate the Inbox badge — it would say a message the list
        // refuses to show (E5 field finding: badge at 2 in front of a
        // list with no unread).
        let hold = if organized {
            crate::store::organized_exclusion()
        } else {
            String::new()
        };
        let inbox: i64 = self.conn().query_row(
            &format!(
                "SELECT COALESCE(SUM(unseen > 0), 0)
                 FROM threads WHERE account_id = ?1 AND inbox_size > 0{hold}"
            ),
            params![account_id],
            |row| row.get(0),
        )?;
        let (_, junk) = self.mailbox_account(account_id, folders.junk.as_deref(), &[])?;
        Ok((inbox as u64, junk))
    }

    /// The unified mailbox, bounded to an account when the nav filters
    /// by “Mailbox”, to the unread when the prototype's tab requires
    /// it — same pagination skeleton as [`Store::unified_recent`].
    pub fn unified_recent_scoped(
        &self,
        account_id: Option<i64>,
        unread: bool,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<UnifiedRow>, Error> {
        let mut stmt =
            self.conn()
                .prepare(&unified_page_sql(account_id.is_some(), unread, false))?;
        let rows = match account_id {
            None => stmt
                .query_map(params![limit as i64, offset as i64], row_to_threaded)?
                .collect::<Result<Vec<_>, _>>()?,
            Some(id) => stmt
                .query_map(params![limit as i64, offset as i64, id], row_to_threaded)?
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(rows)
    }

    /// Total of the unified mailbox, under the same bounds as the page.
    pub fn unified_count_scoped(
        &self,
        account_id: Option<i64>,
        unread: bool,
    ) -> Result<u64, Error> {
        self.unified_account(account_id, unread, false)
    }

    /// THE unified count of the two modes (E2) — a single write path:
    /// classic and organized cannot diverge on what the scrollbar
    /// reserves.
    fn unified_account(
        &self,
        account_id: Option<i64>,
        unread: bool,
        organized: bool,
    ) -> Result<u64, Error> {
        let account_filter = if account_id.is_some() {
            " AND account_id = ?1"
        } else {
            ""
        };
        let unread_filter = if unread { " AND unseen > 0" } else { "" };
        let hold = if organized {
            crate::store::organized_exclusion()
        } else {
            String::new()
        };
        // R4 (D5): the total follows the FLOW — the pinned, served
        // apart at the top, do not count in it; without this exclusion,
        // the scrollbar would reserve phantom rows and the total of a
        // short page would contradict `category_total`.
        let sql = format!(
            "SELECT COUNT(*) FROM threads
              WHERE inbox_size > 0{hold} AND id NOT IN ({PINNED_THREADS}){account_filter}{unread_filter}"
        );
        let count: i64 = match account_id {
            None => self.conn().query_row(&sql, [], |row| row.get(0))?,
            Some(id) => self.conn().query_row(&sql, params![id], |row| row.get(0))?,
        };
        Ok(count as u64)
    }

    /// The page of the ORGANIZED Inbox (E2): the unified flow MINUS the
    /// threads routed elsewhere and the threads held at the Screener —
    /// the `organise_hors` flag, kept up to date by `thread::refresh`,
    /// served by the mirror partial index (S2-bis: at the witness
    /// level, offset stable by construction). THE SAME query as the
    /// classic one ([`unified_page_sql`], `organise` parameter) — never
    /// a copy.
    pub fn organized_inbox_scoped(
        &self,
        account_id: Option<i64>,
        unread: bool,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<UnifiedRow>, Error> {
        let mut stmt =
            self.conn()
                .prepare(&unified_page_sql(account_id.is_some(), unread, true))?;
        let rows = match account_id {
            None => stmt
                .query_map(params![limit as i64, offset as i64], row_to_threaded)?
                .collect::<Result<Vec<_>, _>>()?,
            Some(id) => stmt
                .query_map(params![limit as i64, offset as i64, id], row_to_threaded)?
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(rows)
    }

    /// Total of the organized Inbox, under the SAME bounds as its page
    /// (shared exclusion, `pins` lesson) — the unified count, never a
    /// copy.
    pub fn organized_inbox_count_scoped(
        &self,
        account_id: Option<i64>,
        unread: bool,
    ) -> Result<u64, Error> {
        self.unified_account(account_id, unread, true)
    }

    /// The page of the Feed or the Paper trail (PLAN-MODE-ORGANISE E1)
    /// — the unified flow bounded to the threads whose head comes from
    /// a sender routed to `destination` ([`routing_page_sql`]: same
    /// skeleton, same exclusions, same sort as the Inbox).
    pub fn routing_unified_scoped(
        &self,
        destination: &str,
        account_id: Option<i64>,
        unread: bool,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<UnifiedRow>, Error> {
        let mut stmt = self
            .conn()
            .prepare(&routing_page_sql(account_id.is_some(), unread))?;
        let rows = match account_id {
            None => stmt
                .query_map(
                    params![limit as i64, offset as i64, destination],
                    row_to_threaded,
                )?
                .collect::<Result<Vec<_>, _>>()?,
            Some(id) => stmt
                .query_map(
                    params![limit as i64, offset as i64, destination, id],
                    row_to_threaded,
                )?
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(rows)
    }

    /// Total of the Feed/Paper trail, under the same bounds as its
    /// page — pinned INCLUDED (E1 review: their preferred section only
    /// exists in the Inbox; the shared `pins` exclusion only applies
    /// where the pins are served apart).
    pub fn routing_count_scoped(
        &self,
        destination: &str,
        account_id: Option<i64>,
        unread: bool,
    ) -> Result<u64, Error> {
        let account_filter = if account_id.is_some() {
            " AND account_id = ?2"
        } else {
            ""
        };
        let unread_filter = if unread { " AND unseen > 0" } else { "" };
        // The COUNT renumbers its parameters (?1 destination, ?2
        // account) — the shared fragment takes the index as an
        // argument, never a diverging copy of the EXISTS.
        let thread_route = thread_route_sql("?1");
        let outside_pile = format!(" AND id NOT IN ({})", crate::store::SET_ASIDE_THREADS);
        let sql = format!(
            "SELECT COUNT(*) FROM threads
              WHERE inbox_size > 0
                AND {thread_route}{outside_pile}{account_filter}{unread_filter}"
        );
        let count: i64 = match account_id {
            None => self
                .conn()
                .query_row(&sql, params![destination], |row| row.get(0))?,
            Some(id) => self
                .conn()
                .query_row(&sql, params![destination, id], |row| row.get(0))?,
        };
        Ok(count as u64)
    }

    /// RETOURS-14 R7 (D8) — the Feed's nav badge: the number of cards
    /// NOT YET OPENED. The semantics are those of the PAGE (memory
    /// `kiosque_lus`, RETOURS-13 R10), never the IMAP `unseen` — the
    /// two diverge as soon as another client marks read. Same bounds as
    /// the page ([`routing_page_sql`]: threads routed `kiosque`,
    /// outside the pile); the identity of a card is the HEAD of the
    /// thread (`last_mailbox_id`/`last_uid`), the one `kiosque_cartes`
    /// probes — a thread whose head changes becomes “to open” again,
    /// just as its card becomes new again.
    pub fn feed_unopened(&self, account_id: Option<i64>) -> Result<u64, Error> {
        let account_filter = if account_id.is_some() {
            " AND account_id = ?1"
        } else {
            ""
        };
        let thread_route = thread_route_sql("'kiosque'");
        let outside_pile = format!(" AND id NOT IN ({})", crate::store::SET_ASIDE_THREADS);
        let sql = format!(
            "SELECT COUNT(*) FROM threads
              WHERE inbox_size > 0
                AND {thread_route}{outside_pile}{account_filter}
                AND NOT EXISTS (SELECT 1 FROM kiosque_lus kl
                                 WHERE kl.mailbox_id = threads.last_mailbox_id
                                   AND kl.uid = threads.last_uid)"
        );
        let count: i64 = match account_id {
            None => self.conn().query_row(&sql, [], |row| row.get(0))?,
            Some(id) => self.conn().query_row(&sql, params![id], |row| row.get(0))?,
        };
        Ok(count as u64)
    }

    /// RETOURS-14 R6 (D7) — the Paper trail groups: a sender × its
    /// threads, sorted by the recency of the last message (the
    /// `cleanup_groups` pattern, never the alphabet — D7). The group
    /// key is the sender of the HEAD of the thread — the one the view
    /// shows. One pass: with a SINGLE max(), SQLite guarantees the bare
    /// columns (sender, subject) come from the max's row. `threads`
    /// keeps its full name: [`thread_route_sql`] and
    /// [`SET_ASIDE_THREADS`] target it as is. A thread whose head has
    /// NO sender address (`sender_norm` NULL — a message with no From)
    /// is skipped, never an error that would empty the whole view
    /// (review).
    pub fn paper_trail_groups(
        &self,
        account_id: Option<i64>,
    ) -> Result<Vec<PaperTrailGroup>, Error> {
        let filter = if account_id.is_some() {
            " AND threads.account_id = ?1"
        } else {
            ""
        };
        let thread_route = thread_route_sql("'registre'");
        let outside_pile = format!(
            " AND threads.id NOT IN ({})",
            crate::store::SET_ASIDE_THREADS
        );
        let sql = format!(
            "SELECT he.sender_norm, COUNT(*), MAX(threads.last_epoch), he.sender, he.subject
               FROM threads
               JOIN envelopes he ON he.mailbox_id = threads.last_mailbox_id
                                AND he.uid = threads.last_uid
              WHERE threads.inbox_size > 0
                AND he.sender_norm IS NOT NULL
                AND {thread_route}{outside_pile}{filter}
              GROUP BY he.sender_norm
              ORDER BY MAX(threads.last_epoch) DESC, he.sender_norm"
        );
        let mut stmt = self.conn().prepare(&sql)?;
        let to_group = |row: &rusqlite::Row<'_>| -> rusqlite::Result<PaperTrailGroup> {
            Ok(PaperTrailGroup {
                address: row.get(0)?,
                threads: row.get::<_, i64>(1)? as u64,
                last_epoch: row.get::<_, Option<i64>>(2)?.unwrap_or(0),
                who: row.get(3)?,
                last_subject: row.get(4)?,
            })
        };
        let groups = match account_id {
            None => stmt
                .query_map([], to_group)?
                .collect::<Result<Vec<_>, _>>()?,
            Some(id) => stmt
                .query_map(params![id], to_group)?
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(groups)
    }

    /// The page of ONE Paper trail group — the threads whose head
    /// comes from this sender, at the skeleton and sort of the view
    /// ([`routing_page_sql`]). `?1` limit, `?2` offset, `?3` address,
    /// `?4` account (when bounded).
    pub fn paper_trail_group_scoped(
        &self,
        address: &str,
        account_id: Option<i64>,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<UnifiedRow>, Error> {
        let filter = if account_id.is_some() {
            " AND account_id = ?4"
        } else {
            ""
        };
        let thread_route = thread_route_sql("'registre'");
        let outside_pile = format!(" AND id NOT IN ({})", crate::store::SET_ASIDE_THREADS);
        let head = "AND EXISTS (SELECT 1 FROM envelopes he
                                 WHERE he.mailbox_id = threads.last_mailbox_id
                                   AND he.uid = threads.last_uid
                                   AND he.sender_norm = ?3)";
        let tail = crate::store::unified_join_tail(false);
        let sql = format!(
            "{SELECT_UNIFIED}{THREAD_AGGREGATE}
             FROM (SELECT account_id, last_mailbox_id, last_uid, last_epoch, size, unseen
                     FROM threads
                    WHERE inbox_size > 0
                      AND {thread_route}{outside_pile} {head}{filter}
                    ORDER BY last_epoch DESC, last_uid DESC, account_id
                    LIMIT ?1 OFFSET ?2) t{tail}"
        );
        let mut stmt = self.conn().prepare(&sql)?;
        let rows = match account_id {
            None => stmt
                .query_map(
                    params![limit as i64, offset as i64, address],
                    row_to_threaded,
                )?
                .collect::<Result<Vec<_>, _>>()?,
            Some(id) => stmt
                .query_map(
                    params![limit as i64, offset as i64, address, id],
                    row_to_threaded,
                )?
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(rows)
    }

    /// The PINNED conversations of the scope (R4) — served APART, at
    /// the top of page 0 of the Inbox (D4: Inbox only), never in the
    /// paginated flow (D5). Same columns and same join/sort tail as
    /// the page ([`UNIFIED_JOIN_TAIL`] — the two queries cannot
    /// diverge), list order (date descending, O3). No LIMIT: at most a
    /// handful of pins, and the join on `envelopes` discards on its own
    /// the pins orphaned by an expunged message. READ only — the
    /// writes (`toggle_pin`) live in storage, `store.rs`.
    pub fn pinned_unified_scoped(
        &self,
        account_id: Option<i64>,
        unread: bool,
        organized: bool,
    ) -> Result<Vec<UnifiedRow>, Error> {
        let filter = if account_id.is_some() {
            " AND account_id = ?1"
        } else {
            ""
        };
        let unread_only = if unread { " AND unseen > 0" } else { "" };
        // E2: the shared exclusion extends to the preferred section —
        // in the organized Inbox, a pinned thread routed to the Feed
        // lives in its own view, a held thread waits at the Screener;
        // preferring it here would show a row the total refuses to
        // count.
        let hold = if organized {
            crate::store::organized_exclusion()
        } else {
            String::new()
        };
        let tail = crate::store::unified_join_tail(false);
        let sql = format!(
            "{SELECT_UNIFIED}{THREAD_AGGREGATE}
             FROM (SELECT account_id, last_mailbox_id, last_uid, last_epoch, size, unseen
                     FROM threads
                    WHERE inbox_size > 0{hold} AND id IN ({PINNED_THREADS}){filter}{unread_only}) t{tail}"
        );
        let mut stmt = self.conn().prepare(&sql)?;
        let rows = match account_id {
            None => stmt
                .query_map([], row_to_threaded)?
                .collect::<Result<Vec<_>, _>>()?,
            Some(id) => stmt
                .query_map(params![id], row_to_threaded)?
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(rows)
    }

    /// Enriches ONE PAGE of rows (field, R10-R12, 2026-08-22) — three
    /// reads bounded to the served page, NEVER the hot query (lesson of
    /// PLAN-DEFILEMENT-PROFOND: nothing gets added to the path that
    /// paginates 200k rows):
    ///
    /// 1. **a THREAD's attachment count sums all its messages**
    ///    (R12 — the head alone lied as soon as the attachment lived
    ///    on another message: replying to a message with an attachment
    ///    made the “n files” chip disappear);
    /// 2. **the thread's invitation joins the row** (R10: the badge's
    ///    rank carries the three gestures, reply without opening);
    /// 3. **a REPLIED-TO invitation lends its face to the row**
    ///    (R11: subject, sender and preview of the INVITATION — the
    ///    only case where the list does not show the thread's last
    ///    message; the badge says the reply, the sort order does not
    ///    move).
    pub fn enrich_rows(&self, rows: &mut [UnifiedRow]) -> Result<(), Error> {
        let mut thread_ids: Vec<i64> = rows.iter().filter_map(|r| r.thread_id).collect();
        thread_ids.sort_unstable();
        thread_ids.dedup();
        struct InvitationFace {
            rank: InvitationRank,
            subject: Option<String>,
            sender: Option<String>,
            sender_address: Option<String>,
            preview: Option<String>,
        }
        let mut attachments_by_thread: HashMap<i64, u32> = HashMap::new();
        let mut invitation_by_thread: HashMap<i64, InvitationFace> = HashMap::new();
        if !thread_ids.is_empty() {
            let holes = vec!["?"; thread_ids.len()].join(",");
            // R12: the attachment count of the WHOLE thread (idx_envelopes_thread).
            let mut stmt = self.conn().prepare(&format!(
                "SELECT e.thread_id, COUNT(*)
                   FROM attachments a
                   JOIN envelopes e ON e.mailbox_id = a.mailbox_id AND e.uid = a.uid
                  WHERE e.thread_id IN ({holes})
                  GROUP BY e.thread_id"
            ))?;
            let counts = stmt.query_map(params_from_iter(thread_ids.iter()), |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?.max(0) as u32))
            })?;
            for count in counts {
                let (thread, n) = count?;
                attachments_by_thread.insert(thread, n);
            }
            // R10/R11: the thread's invitation, the most recent one if
            // several (ascending ORDER BY: the last one overwrites).
            let mut stmt = self.conn().prepare(&format!(
                "SELECT e.thread_id, m.name, i.uid, i.titre, i.reponse, i.annule,
                        i.organisateur_adresse IS NOT NULL AND i.annule = 0,
                        e.subject, e.sender, e.sender_address, b.preview
                   FROM invitations i
                   JOIN envelopes e ON e.mailbox_id = i.mailbox_id AND e.uid = i.uid
                   JOIN mailboxes m ON m.id = i.mailbox_id
                   LEFT JOIN bodies b ON b.mailbox_id = i.mailbox_id AND b.uid = i.uid
                  WHERE e.thread_id IN ({holes}) AND i.methode = 'request'
                  ORDER BY e.date_epoch ASC"
            ))?;
            let faces = stmt.query_map(params_from_iter(thread_ids.iter()), |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    InvitationFace {
                        rank: InvitationRank {
                            mailbox: row.get(1)?,
                            uid: row.get(2)?,
                            title: row.get(3)?,
                            reply: row.get(4)?,
                            cancelled: row.get(5)?,
                            can_reply: row.get(6)?,
                        },
                        subject: row.get(7)?,
                        sender: row.get(8)?,
                        sender_address: row.get(9)?,
                        preview: row.get(10)?,
                    },
                ))
            })?;
            for face in faces {
                let (thread, face) = face?;
                invitation_by_thread.insert(thread, face);
            }
        }
        for row in rows.iter_mut() {
            match row.thread_id {
                Some(thread) => {
                    if let Some(n) = attachments_by_thread.get(&thread) {
                        row.attachment_count = *n;
                        row.has_attachment = *n > 0;
                    }
                    let Some(face) = invitation_by_thread.get(&thread) else {
                        continue;
                    };
                    // R11: replied-to → the row's face is the
                    // invitation, not our last email.
                    if face.rank.reply.is_some() {
                        row.envelope.subject = face.subject.clone();
                        row.envelope.sender = face.sender.clone();
                        row.envelope.sender_address = face.sender_address.clone();
                        row.preview = face.preview.clone();
                    }
                    row.invitation = Some(face.rank.clone());
                }
                // A row WITHOUT a thread is itself the message: if it
                // carries an invitation, the key is its own — a lookup
                // indexed by row, bounded page. The echoes (synthetic
                // mailbox) never carry one.
                None if !row.mailbox.starts_with("echo:") => {
                    let rank = self
                        .conn()
                        .query_row(
                            "SELECT i.titre, i.reponse, i.annule,
                                    i.organisateur_adresse IS NOT NULL AND i.annule = 0
                               FROM invitations i
                               JOIN mailboxes m ON m.id = i.mailbox_id
                              WHERE m.account_id = ?1 AND m.name = ?2 AND i.uid = ?3
                                AND i.methode = 'request'",
                            params![row.account_id, row.mailbox, row.envelope.uid],
                            |sql_row| {
                                Ok(InvitationRank {
                                    mailbox: row.mailbox.clone(),
                                    uid: row.envelope.uid,
                                    title: sql_row.get(0)?,
                                    reply: sql_row.get(1)?,
                                    cancelled: sql_row.get(2)?,
                                    can_reply: sql_row.get(3)?,
                                })
                            },
                        )
                        .optional()?;
                    row.invitation = rank;
                }
                None => {}
            }
        }
        Ok(())
    }

    /// `(total, unread)` cumulated across the given mailboxes — the
    /// total of the pagination of a category, and the unread hero of
    /// junk.
    ///
    /// `echos` (PLAN-REACTIVITE E3): the category and the accounts
    /// whose local echoes count TOO — the counter and the list say the
    /// same thing, never two truths. An echo is read by nature: it
    /// never enters `unread`.
    pub fn category_totals(
        &self,
        mailbox_ids: &[i64],
        exclude: &[i64],
        echos: Option<(&str, &[i64])>,
    ) -> Result<(u64, u64), Error> {
        let mut total = 0u64;
        let mut unread = 0u64;
        let sql = format!(
            "SELECT COUNT(*), COALESCE(SUM(NOT e.seen), 0)
             FROM envelopes e WHERE e.mailbox_id = ?1{}",
            exclusion_clause(exclude)
        );
        for id in mailbox_ids {
            let (t, n): (i64, i64) = self
                .conn()
                .query_row(&sql, params![id], |row| Ok((row.get(0)?, row.get(1)?)))?;
            total += t as u64;
            unread += n as u64;
        }
        if let Some((destination, accounts)) = echos {
            for account in accounts {
                total += self.count_echos(destination, Some(*account))?;
            }
        }
        Ok((total, unread))
    }

    /// A page of a category other than the inbox: the messages of the
    /// given mailboxes, from the most recent to the oldest.
    ///
    /// The pagination follows the gate P1 rule: each mailbox supplies a
    /// slice BOUNDED by its `(mailbox_id, date_epoch DESC)` index, the
    /// merge and the `OFFSET` apply on these slices — never a sort of
    /// the whole mailbox, and the joins only run on the retained rows.
    /// Rows outside a thread count as size 1 and unread per `seen`
    /// (`LEFT JOIN threads`).
    pub fn category_page(
        &self,
        mailbox_ids: &[i64],
        unread: bool,
        exclude: &[i64],
        echos: Option<(&str, &[i64])>,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<UnifiedRow>, Error> {
        // The echoes (PLAN-REACTIVITE E3) only enter outside the
        // “Unread” tab (an echo is read by nature) and if some accounts
        // carry them. Without echoes, the SQL is EXACTLY the one from
        // before — the hot path pays nothing.
        let echos = match echos {
            Some((destination, accounts)) if !unread && !accounts.is_empty() => {
                Some((destination, accounts))
            }
            _ => None,
        };
        if mailbox_ids.is_empty() && echos.is_none() {
            return Ok(Vec::new());
        }
        let n = mailbox_ids.len();
        let filter = if unread { " AND NOT e.seen" } else { "" };
        // The exclusion applies INSIDE each slice: applied afterward,
        // the pagination would count rows it does not serve.
        let exclusion = exclusion_clause(exclude);
        let bound_idx = n + 1;
        let limit_idx = n + 2;
        let offset_idx = n + 3;
        let mut slices: Vec<String> = (1..=n)
            .map(|i| {
                format!(
                    "SELECT * FROM (SELECT e.mailbox_id, e.uid, e.date_epoch FROM envelopes e
                      WHERE e.mailbox_id = ?{i}{filter}{exclusion}
                      ORDER BY e.date_epoch DESC, e.uid DESC LIMIT ?{bound_idx})"
                )
            })
            .collect();
        if echos.is_some() {
            // The echoes' slice: identified by a NEGATIVE mailbox_id
            // (-echo id) — never confused with a real mailbox, and the
            // output join knows how to recognize it. The accounts are
            // OUR integers: literal inclusion, like the exclusion.
            let accounts = echos
                .as_ref()
                .map(|(_, accounts)| {
                    accounts
                        .iter()
                        .map(i64::to_string)
                        .collect::<Vec<_>>()
                        .join(",")
                })
                .unwrap_or_default();
            slices.push(format!(
                "SELECT * FROM (SELECT -ec.id AS mailbox_id, 0 AS uid, ec.date_epoch
                  FROM echos ec
                  WHERE ec.destination = ?{dest} AND ec.account_id IN ({accounts})
                  ORDER BY ec.date_epoch DESC LIMIT ?{bound_idx})",
                dest = n + 4,
            ));
        }
        let sql = if echos.is_none() {
            format!(
                "{SELECT_UNIFIED}, COALESCE(t.size, 1),
                        COALESCE(t.unseen, CASE WHEN e.seen THEN 0 ELSE 1 END)
                 FROM (SELECT mailbox_id, uid FROM ({slices})
                       ORDER BY date_epoch DESC, uid DESC, mailbox_id
                       LIMIT ?{limit_idx} OFFSET ?{offset_idx}) page
                 JOIN envelopes e ON e.mailbox_id = page.mailbox_id AND e.uid = page.uid
                 JOIN mailboxes m ON m.id = e.mailbox_id
                 JOIN accounts a ON a.id = m.account_id
                 LEFT JOIN bodies b ON b.mailbox_id = e.mailbox_id AND b.uid = e.uid
                 LEFT JOIN threads t ON t.id = e.thread_id
                 ORDER BY e.date_epoch DESC, e.uid DESC, e.mailbox_id",
                slices = slices.join(" UNION ALL "),
            )
        } else {
            // With echoes: the merged page carries rows from BOTH
            // worlds — the envelopes join their tables, the echoes
            // their own, and the UNION yields the same columns as
            // SELECT_UNIFIED (uid 0, synthetic mailbox `echo:<id>`,
            // read, no star nor thread). The final sort replays the
            // page's key.
            format!(
                "WITH page AS (SELECT mailbox_id, uid, date_epoch FROM ({slices})
                       ORDER BY date_epoch DESC, uid DESC, mailbox_id
                       LIMIT ?{limit_idx} OFFSET ?{offset_idx})
                 SELECT * FROM (
                   {SELECT_UNIFIED}, COALESCE(t.size, 1),
                        COALESCE(t.unseen, CASE WHEN e.seen THEN 0 ELSE 1 END),
                        page.date_epoch AS sort_date, page.uid AS sort_uid,
                        page.mailbox_id AS sort_mailbox
                   FROM page
                   JOIN envelopes e ON e.mailbox_id = page.mailbox_id AND e.uid = page.uid
                   JOIN mailboxes m ON m.id = e.mailbox_id
                   JOIN accounts a ON a.id = m.account_id
                   LEFT JOIN bodies b ON b.mailbox_id = e.mailbox_id AND b.uid = e.uid
                   LEFT JOIN threads t ON t.id = e.thread_id
                   UNION ALL
                   SELECT a.id, a.email, 0, ec.subject, ec.sender, ec.sender_address,
                        ec.message_id, ec.date_epoch, 1, 0, ec.attachment_count,
                        NULL, NULL, 'echo:' || ec.id, ec.preview,
                        -- PLAN-RETOURS-5: the REAL recipients of the
                        -- echo (copies from the sending log or from
                        -- the source envelope) — never the destination
                        -- slug (“To: envoyes”, field, 2026-08-21).
                        ec.to_addrs, NULL, 1, 0,
                        page.date_epoch AS sort_date, page.uid AS sort_uid,
                        page.mailbox_id AS sort_mailbox
                   FROM page
                   JOIN echos ec ON page.mailbox_id = -ec.id AND page.mailbox_id < 0
                   JOIN accounts a ON a.id = ec.account_id
                 )
                 ORDER BY sort_date DESC, sort_uid DESC, sort_mailbox",
                slices = slices.join(" UNION ALL "),
            )
        };
        let bound = (offset + limit) as i64;
        let mut parameters: Vec<rusqlite::types::Value> = mailbox_ids
            .iter()
            .map(|id| rusqlite::types::Value::Integer(*id))
            .collect();
        parameters.push(rusqlite::types::Value::Integer(bound));
        parameters.push(rusqlite::types::Value::Integer(limit as i64));
        parameters.push(rusqlite::types::Value::Integer(offset as i64));
        if let Some((destination, _)) = echos {
            parameters.push(rusqlite::types::Value::Text(destination.to_string()));
        }
        let mut stmt = self.conn().prepare(&sql)?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(parameters), row_to_threaded)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;
    use crate::envelope::Envelope;

    /// PLAN-AUDIT-V2 E5: `canonical_folders` swallowed every SQLite
    /// error reading the sent folder (`unwrap_or(None)`) — a locked
    /// base was worth “no Sent”, threads silently un-reconciled.
    #[test]
    fn an_unreadable_base_is_an_error_not_an_absence_of_sent() {
        let store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("moi@exemple.fr", "gmail")
            .unwrap();
        store
            .conn()
            .execute_batch("ALTER TABLE accounts RENAME TO accounts_indisponibles")
            .unwrap();
        assert!(
            store.canonical_folders(account).is_err(),
            "an unreadable read must propagate, never be worth None"
        );
    }

    /// PLAN-AUDIT-V2 E5: `[Gmail]` was hardcoded — an account with
    /// “[Google Mail]/…” (UK, Germany) lost Archives, Spam and Trash.
    /// The RFC 6154 role carried by the folder wins; the name stays
    /// the fallback.
    #[test]
    fn google_mail_uk_has_its_archives() {
        use crate::{Folder, SpecialUse};
        let store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("moi@exemple.fr", "gmail")
            .unwrap();
        let folder = |name: &str, role: Option<SpecialUse>| Folder {
            wire: name.to_string(),
            display: name.to_string(),
            selectable: true,
            special_use: role,
            delimiter: None,
        };
        store
            .replace_folders(
                account,
                &[
                    folder("INBOX", None),
                    folder("[Google Mail]/All Mail", Some(SpecialUse::All)),
                    folder("[Google Mail]/Spam", Some(SpecialUse::Junk)),
                    folder("[Google Mail]/Bin", Some(SpecialUse::Trash)),
                ],
            )
            .unwrap();
        let canon = store.canonical_folders(account).unwrap();
        assert_eq!(canon.archives.as_deref(), Some("[Google Mail]/All Mail"));
        assert!(canon.archives_full, "“All Mail” is a full mailbox");
        assert_eq!(canon.junk.as_deref(), Some("[Google Mail]/Spam"));
        assert_eq!(canon.trash.as_deref(), Some("[Google Mail]/Bin"));
    }

    fn envelope(uid: u32, subject: &str, epoch: i64, seen: bool) -> Envelope {
        Envelope {
            reply_to: None,
            uid,
            subject: Some(subject.to_string()),
            sender: Some("Alice Martin".to_string()),
            sender_address: Some("alice@example.com".to_string()),
            message_id: Some(format!("<m{uid}@example.com>")),
            in_reply_to: None,
            date: Some(Utc.timestamp_opt(epoch, 0).unwrap()),
            seen,
            flagged: false,
            to_addrs: Vec::new(),
            cc_addrs: Vec::new(),
        }
    }

    fn folder(wire: &str) -> crate::Folder {
        crate::Folder {
            wire: wire.to_string(),
            display: wire.to_string(),
            selectable: true,
            special_use: None,
            delimiter: None,
        }
    }

    #[test]
    fn a_pst_migration_does_not_hijack_the_canonicals() {
        // The field fixture: [Gmail] canonicals, root homonyms, and a
        // PST migration full of deep “Archive” segments.
        let store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("a@exemple.fr", "gmail")
            .unwrap();
        store
            .set_thread_scope(account, Some("[Gmail]/Messages envoyes")) // lang:fr
            .unwrap();
        store
            .replace_folders(
                account,
                &[
                    folder("INBOX"),
                    folder("Brouillons"),         // lang:fr
                    folder("[Gmail]/Brouillons"), // lang:fr
                    folder("[Gmail]/Spam"),
                    folder("Corbeille"),                              // lang:fr
                    folder("[Gmail]/Corbeille"),                      // lang:fr
                    folder("[Gmail]/Tous les messages"),              // lang:fr
                    folder("[Gmail]/Corbeille/x@y.fr/Archive"),       // lang:fr
                    folder("[Gmail]/Corbeille/x@y.fr/Archive/Sport"), // lang:fr
                    folder("pst/Archive/Sante"),                      // lang:fr
                    folder("pst/Trash"),
                ],
            )
            .unwrap();
        let canon = store.canonical_folders(account).unwrap();
        assert_eq!(canon.inbox, "INBOX");
        assert_eq!(canon.sent.as_deref(), Some("[Gmail]/Messages envoyes")); // lang:fr
        assert_eq!(canon.drafts.as_deref(), Some("[Gmail]/Brouillons")); // lang:fr
        assert_eq!(canon.junk.as_deref(), Some("[Gmail]/Spam"));
        assert_eq!(canon.archives.as_deref(), Some("[Gmail]/Tous les messages")); // lang:fr
        assert_eq!(canon.trash.as_deref(), Some("[Gmail]/Corbeille")); // lang:fr
    }

    #[test]
    fn the_counters_follow_the_resolved_folders_and_silent_mailboxes_are_zero() {
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("a@exemple.fr", "gmail")
            .unwrap();
        let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        store
            .upsert_envelopes(
                inbox,
                &[
                    envelope(1, "read", 100, true),
                    envelope(2, "unread", 200, false),
                ],
            )
            .unwrap();
        let archives = store.create_mailbox(account, "Archives", 1).unwrap();
        store
            .upsert_envelopes(
                archives,
                &[
                    envelope(1, "archived read", 300, true),
                    envelope(2, "archived unread", 400, false),
                    envelope(3, "archived read too", 500, true),
                ],
            )
            .unwrap();
        store
            .replace_folders(account, &[folder("INBOX"), folder("Archives")])
            .unwrap();
        let canon = store.canonical_folders(account).unwrap();
        let counts = store.nav_counts(account, &canon).unwrap();
        assert_eq!(counts.inbox_total, 2);
        assert_eq!(counts.inbox_unread, 1);
        assert_eq!(counts.archives, 3);
        // Drafts: no local draft -> zero, never an error.
        assert_eq!(counts.drafts, 0);
    }

    /// PLAN-DEFILEMENT-PROFOND (field, 2026-08-20): the nav probe now
    /// pays only for the two SHOWN counters (A29: the nav only says the
    /// unread) — and says exactly what the complete inventory would
    /// say. Parity locked: if `nav_counts` evolves, the light version
    /// must follow or this test screams.
    #[test]
    fn the_light_nav_counters_say_what_the_inventory_says() {
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("a@exemple.fr", "gmail")
            .unwrap();
        let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        store
            .upsert_envelopes(
                inbox,
                &[
                    envelope(1, "read", 100, true),
                    envelope(2, "unread", 200, false),
                ],
            )
            .unwrap();
        let spam = store.create_mailbox(account, "Spam", 1).unwrap();
        store
            .upsert_envelopes(
                spam,
                &[
                    envelope(1, "junk", 300, false),
                    envelope(2, "junk read", 400, true),
                ],
            )
            .unwrap();
        store
            .replace_folders(account, &[folder("INBOX"), folder("Spam")])
            .unwrap();
        let canon = store.canonical_folders(account).unwrap();
        let full = store.nav_counts(account, &canon).unwrap();
        let (inbox, junk) = store.nav_unread_counts(account, &canon, false).unwrap();
        assert_eq!(inbox, full.inbox_unread);
        assert_eq!(junk, full.junk_unread);
        assert_eq!(inbox, 1);
        assert_eq!(junk, 1);
    }

    /// B-D1 (PLAN-BROUILLONS): the counter counts the account's
    /// `drafts` table — the mirror IMAP mailbox, even full, does not
    /// count, since it is not the one the click opens.
    #[test]
    fn the_draft_counter_counts_the_local_drafts() {
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("a@exemple.fr", "gmail")
            .unwrap();
        let other = store
            .adopt_or_create_account("b@exemple.fr", "gmail")
            .unwrap();
        let mirror = store.create_mailbox(account, "Brouillons", 1).unwrap();
        store
            .upsert_envelopes(
                mirror,
                &[
                    envelope(1, "pushed copy", 100, true),
                    envelope(2, "pushed copy too", 200, true),
                ],
            )
            .unwrap();
        store
            .replace_folders(account, &[folder("INBOX"), folder("Brouillons")])
            .unwrap();
        store
            .save_draft(
                account,
                None,
                None,
                crate::DraftContent {
                    to_raw: "",
                    cc_raw: "",
                    bcc_raw: "",
                    body_html: None,
                    subject: "local",
                    body: "text",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                    important: false,
                },
            )
            .unwrap();
        store
            .save_draft(
                other,
                None,
                None,
                crate::DraftContent {
                    to_raw: "",
                    cc_raw: "",
                    bcc_raw: "",
                    body_html: None,
                    subject: "neighbor's",
                    body: "text",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                    important: false,
                },
            )
            .unwrap();

        let canon = store.canonical_folders(account).unwrap();
        let counts = store.nav_counts(account, &canon).unwrap();
        assert_eq!(
            counts.drafts, 1,
            "the local one counts, the mirror does not"
        );
    }

    /// PLAN-RETOURS-5 (field, 2026-08-21): the send echo's row says its
    /// REAL recipients — never the category slug (“To: envoyes” on
    /// screen during the reconciliation window).
    #[test]
    fn the_send_echo_row_says_its_recipients() {
        let store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("t@exemple.fr", "gmail")
            .unwrap();
        let draft = crate::compose(
            "t@exemple.fr",
            "a@b.fr, c@d.fr",
            "",
            "",
            "subject",
            "body",
            None,
        )
        .unwrap();
        store.enqueue_outbox(account, &draft).unwrap();
        let id = store.outbox_to_send(account).unwrap()[0].id;
        store
            .set_outbox_state(id, crate::OutboxState::Sent)
            .unwrap();
        assert!(store.send_echo(id).unwrap());

        let accounts = [account];
        let page = store
            .category_page(&[], false, &[], Some(("envoyes", &accounts)), 0, 10)
            .unwrap();
        assert_eq!(page.len(), 1);
        assert!(page[0].mailbox.starts_with("echo:"), "{}", page[0].mailbox);
        assert_eq!(
            page[0].envelope.to_addrs,
            vec!["a@b.fr".to_string(), "c@d.fr".to_string()],
            "the real recipients, never “envoyes”"
        );
    }

    /// E3 (PLAN-REACTIVITE): the echoes enter the page AT THEIR date's
    /// PLACE — an old deleted message does not jump to the top of
    /// Trash — and the total counts them (never two truths). The
    /// echo's row carries its synthetic mailbox `echo:<id>`, its
    /// preview, and the “Unread” tab ignores them (an echo is read by
    /// nature).
    #[test]
    fn the_category_page_serves_echoes_at_their_date() {
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("a@exemple.fr", "gmail")
            .unwrap();
        let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        let trash = store.create_mailbox(account, "Trash", 1).unwrap();
        store
            .replace_folders(
                account,
                &[crate::Folder {
                    wire: "Trash".into(),
                    display: "Trash".into(),
                    selectable: true,
                    special_use: None,
                    delimiter: None,
                }],
            )
            .unwrap();
        store
            .upsert_envelopes(
                trash,
                &[
                    envelope(1, "old", 100, true),
                    envelope(2, "recent", 300, true),
                ],
            )
            .unwrap();
        // The deleted message's date falls BETWEEN the two: the echo
        // must slot into the middle, not jump to the top.
        store
            .upsert_envelopes(inbox, &[envelope(7, "middle", 200, true)])
            .unwrap();
        store
            .save_body(inbox, 7, "<p>middle body</p>", &[])
            .unwrap();
        store
            .gesture_with_echo(inbox, 7, crate::Action::Delete, Some("corbeille"))
            .unwrap();

        let accounts = [account];
        let page = store
            .category_page(&[trash], false, &[], Some(("corbeille", &accounts)), 0, 10)
            .unwrap();
        let subjects: Vec<&str> = page
            .iter()
            .map(|row| row.envelope.subject.as_deref().unwrap())
            .collect();
        assert_eq!(subjects, ["recent", "middle", "old"]);
        assert!(page[1].mailbox.starts_with("echo:"), "{}", page[1].mailbox);
        assert_eq!(page[1].preview.as_deref(), Some("middle body"));
        assert_eq!(page[1].thread_unseen, 0, "an echo is read");
        // The pagination crosses the echo without losing or duplicating.
        let cut = store
            .category_page(&[trash], false, &[], Some(("corbeille", &accounts)), 1, 1)
            .unwrap();
        assert_eq!(cut[0].envelope.subject.as_deref(), Some("middle"));
        // The total says the same thing as the page.
        let (total, _) = store
            .category_totals(&[trash], &[], Some(("corbeille", &accounts)))
            .unwrap();
        assert_eq!(total, 3);
        // “Unread”: the echoes do not enter it.
        let unread = store
            .category_page(&[trash], true, &[], Some(("corbeille", &accounts)), 0, 10)
            .unwrap();
        assert!(unread.is_empty());
    }

    #[test]
    fn the_category_page_merges_the_mailboxes_from_most_recent_to_oldest() {
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("a@exemple.fr", "gmail")
            .unwrap();
        let left = store.create_mailbox(account, "Archives", 1).unwrap();
        let right = store.create_mailbox(account, "pst/Archives", 1).unwrap();
        store
            .upsert_envelopes(
                left,
                &[envelope(1, "a1", 100, true), envelope(2, "a3", 300, false)],
            )
            .unwrap();
        store
            .upsert_envelopes(
                right,
                &[envelope(1, "b2", 200, true), envelope(2, "b4", 400, true)],
            )
            .unwrap();
        let page = store
            .category_page(&[left, right], false, &[], None, 0, 3)
            .unwrap();
        let subjects: Vec<&str> = page
            .iter()
            .map(|row| row.envelope.subject.as_deref().unwrap())
            .collect();
        assert_eq!(subjects, ["b4", "a3", "b2"]);
        // Outside a thread: size 1, unread per `seen`.
        assert_eq!(page[1].thread_size, 1);
        assert_eq!(page[1].thread_unseen, 1);
        assert_eq!(page[0].thread_unseen, 0);
        // The OFFSET crosses the merge without losing or duplicating.
        let next = store
            .category_page(&[left, right], false, &[], None, 3, 3)
            .unwrap();
        let subjects: Vec<&str> = next
            .iter()
            .map(|row| row.envelope.subject.as_deref().unwrap())
            .collect();
        assert_eq!(subjects, ["a1"]);
        // The “Unread” tab filters on the core side, inside the slices themselves.
        let unread = store
            .category_page(&[left, right], true, &[], None, 0, 10)
            .unwrap();
        let subjects: Vec<&str> = unread
            .iter()
            .map(|row| row.envelope.subject.as_deref().unwrap())
            .collect();
        assert_eq!(subjects, ["a3"]);
        // Preview and attachment COUNT: set when the body is written.
        store
            .save_body(
                left,
                2,
                "<p>Preview of a3</p>",
                &[
                    crate::Attachment {
                        index: 0,
                        name: "one.pdf".into(),
                        mime: "application/pdf".into(),
                        size: 10,
                    },
                    crate::Attachment {
                        index: 1,
                        name: "two.pdf".into(),
                        mime: "application/pdf".into(),
                        size: 10,
                    },
                ],
            )
            .unwrap();
        let page = store
            .category_page(&[left, right], false, &[], None, 0, 10)
            .unwrap();
        let a3 = page
            .iter()
            .find(|row| row.envelope.subject.as_deref() == Some("a3"))
            .unwrap();
        assert_eq!(a3.preview.as_deref(), Some("Preview of a3"));
        assert_eq!(a3.attachment_count, 2);
        assert!(a3.has_attachment);
    }

    #[test]
    fn a_full_gmail_mailbox_stripped_of_canonicals_makes_the_archives() {
        // The field defect (2026-08-12): “All Mail” contains
        // EVERYTHING — the Archives category showed the whole mailbox.
        // The full mailbox must strip itself of the other canonicals.
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("a@exemple.fr", "gmail")
            .unwrap();
        store
            .replace_folders(
                account,
                &[folder("INBOX"), folder("[Gmail]/Tous les messages")], // lang:fr
            )
            .unwrap();
        let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        let full = store
            .create_mailbox(account, "[Gmail]/Tous les messages", 1) // lang:fr
            .unwrap();
        // <m1> lives in INBOX AND the full mailbox (received, not
        // archived); <m2> only in the full mailbox (truly archived).
        store
            .upsert_envelopes(inbox, &[envelope(1, "received", 100, true)])
            .unwrap();
        store
            .upsert_envelopes(
                full,
                &[
                    envelope(1, "received", 100, true),
                    envelope(2, "archived", 200, true),
                ],
            )
            .unwrap();

        let canon = store.canonical_folders(account).unwrap();
        assert!(canon.archives_full, "the full mailbox is marked");
        assert_eq!(canon.archives.as_deref(), Some("[Gmail]/Tous les messages")); // lang:fr
        let counts = store.nav_counts(account, &canon).unwrap();
        assert_eq!(
            counts.archives, 1,
            "only the message outside the other canonicals is archived"
        );
        let exclude = store.canonical_except_archive(account, &canon).unwrap();
        assert_eq!(exclude, vec![inbox]);
        let page = store
            .category_page(&[full], false, &exclude, None, 0, 10)
            .unwrap();
        assert_eq!(page.len(), 1);
        assert_eq!(page[0].envelope.subject.as_deref(), Some("archived"));
        let (total, _) = store.category_totals(&[full], &exclude, None).unwrap();
        assert_eq!(total, 1);
        // A PURE archives folder is never stripped of anything.
        store
            .replace_folders(
                account,
                &[
                    folder("INBOX"),
                    folder("Archives"),
                    folder("[Gmail]/Tous les messages"), // lang:fr
                ],
            )
            .unwrap();
        let canon = store.canonical_folders(account).unwrap();
        assert!(!canon.archives_full);
        assert_eq!(canon.archives.as_deref(), Some("Archives"));
    }

    #[test]
    fn the_preview_catchup_settles_earlier_bodies() {
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("a@exemple.fr", "gmail")
            .unwrap();
        let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        store
            .upsert_envelopes(inbox, &[envelope(1, "old", 100, true)])
            .unwrap();
        // A body written the OLD way: the `preview` column did not exist yet.
        store
            .conn()
            .execute(
                "INSERT INTO bodies (mailbox_id, uid, html, scanned, preview)
                 VALUES (?1, 1, '<p>Old body</p>', 1, NULL)",
                params![inbox],
            )
            .unwrap();
        assert_eq!(store.preview_catchup(10).unwrap(), 0, "no more stragglers");
        let page = store
            .category_page(&[inbox], false, &[], None, 0, 10)
            .unwrap();
        assert_eq!(page[0].preview.as_deref(), Some("Old body"));
    }

    /// Field R10-R12 (PLAN-INVITATIONS): the PAGE enrichment — attachments
    /// summed over the THREAD, invitation badge at the rank, and the
    /// invitation's face lent to the row once the reply is logged.
    #[test]
    fn the_enrichment_sums_the_attachments_and_lends_the_invitation_s_face() {
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("nous@exemple.fr", "gmail")
            .unwrap();
        let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        // The thread: the INVITATION (uid 1, it carries the attachment)
        // then our reply email (uid 2 — the thread's displayed HEAD).
        let invitation = envelope(1, "September workshop", 100, true);
        let mut head = envelope(2, "Accepted: September workshop", 200, true);
        head.in_reply_to = Some("<m1@example.com>".to_string());
        store.upsert_envelopes(inbox, &[invitation, head]).unwrap();
        store
            .save_body_full(
                inbox,
                1,
                "<p>please join us</p>",
                &[crate::Attachment {
                    index: 0,
                    name: "plan.pdf".to_string(),
                    mime: "application/pdf".to_string(),
                    size: 1024,
                }],
                Some(&crate::InvitationRow {
                    method: "request".to_string(),
                    event_uid: "workshop@exemple.fr".to_string(),
                    title: "September workshop".to_string(),
                    organizer_address: Some("sofia@exemple.fr".to_string()),
                    partstat: Some("sans_reponse".to_string()),
                    ..Default::default()
                }),
            )
            .unwrap();

        let mut page = store.unified_recent_scoped(None, false, 0, 10).unwrap();
        store.enrich_rows(&mut page).unwrap();
        assert_eq!(page.len(), 1);
        // R12: the attachment lives on the invitation, NOT on the head
        // — the thread's chip counts it anyway.
        assert_eq!(page[0].attachment_count, 1);
        assert!(page[0].has_attachment);
        // R10: the badge targets the invitation MESSAGE, not the head.
        let badge = page[0].invitation.as_ref().expect("badge");
        assert_eq!(badge.uid, 1);
        assert_eq!(badge.title, "September workshop");
        assert!(badge.can_reply);
        assert_eq!(badge.reply, None);
        // With no reply logged, the row keeps the head's face.
        assert_eq!(
            page[0].envelope.subject.as_deref(),
            Some("Accepted: September workshop")
        );

        // R11: the logged reply lends the invitation's FACE.
        let mut draft = crate::compose(
            "nous@exemple.fr",
            "sofia@exemple.fr",
            "",
            "",
            "Accepted: September workshop",
            "Accepted: September workshop",
            None,
        )
        .unwrap();
        draft.ics_reply = Some("BEGIN:VCALENDAR\r\nEND:VCALENDAR\r\n".to_string());
        store
            .enqueue_invitation_reply(account, &draft, "INBOX", 1, "accepte", 42)
            .unwrap()
            .expect("logged");
        let mut page = store.unified_recent_scoped(None, false, 0, 10).unwrap();
        store.enrich_rows(&mut page).unwrap();
        assert_eq!(
            page[0].envelope.subject.as_deref(),
            Some("September workshop"),
            "the only case where the list does not show the last message"
        );
        assert_eq!(page[0].preview.as_deref(), Some("please join us"));
        assert_eq!(
            page[0].invitation.as_ref().unwrap().reply.as_deref(),
            Some("accepte")
        );
    }

    #[test]
    fn the_unified_mailbox_is_bounded_to_an_account() {
        let mut store = Store::open_in_memory().unwrap();
        let first = store
            .adopt_or_create_account("a@exemple.fr", "gmail")
            .unwrap();
        let second = store
            .adopt_or_create_account("b@exemple.fr", "gmail")
            .unwrap();
        let inbox_a = store.create_mailbox(first, "INBOX", 1).unwrap();
        let inbox_b = store.create_mailbox(second, "INBOX", 1).unwrap();
        store
            .upsert_envelopes(inbox_a, &[envelope(1, "at a's", 100, false)])
            .unwrap();
        store
            .upsert_envelopes(
                inbox_b,
                &[
                    envelope(1, "at b's", 200, false),
                    envelope(2, "at b's too", 300, true),
                ],
            )
            .unwrap();

        let all = store.unified_recent_scoped(None, false, 0, 10).unwrap();
        assert_eq!(all.len(), 3);
        let only_b = store
            .unified_recent_scoped(Some(second), false, 0, 10)
            .unwrap();
        assert_eq!(only_b.len(), 2);
        assert!(only_b.iter().all(|row| row.account_id == second));
        assert_eq!(store.unified_count_scoped(Some(first), false).unwrap(), 1);
        assert_eq!(store.unified_count_scoped(None, false).unwrap(), 3);
        // “At b's too” is read: the unread tab keeps only two.
        assert_eq!(store.unified_count_scoped(None, true).unwrap(), 2);
        assert_eq!(
            store
                .unified_recent_scoped(None, true, 0, 10)
                .unwrap()
                .len(),
            2
        );
    }
}

//! Local SQLite storage: envelopes and sync state, multi-mailbox.
//!
//! Concrete structure (no trait): SQLite is a frozen product decision
//! (PHASE0.md §2.1) and the tests use an in-memory database — the network
//! abstraction ([`crate::MailServer`]) is the only boundary that is needed.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::ops::ControlFlow;
use std::path::Path;

use chrono::DateTime;
use rusqlite::{Connection, OptionalExtension, params};

use crate::action::{Action, PendingAction};
use crate::attachment::Attachment;
use crate::envelope::{Envelope, Uid};
use crate::error::Error;
use crate::invitation::{InvitationRow, StoredInvitation};
use crate::remote::Folder;
use crate::remote::SpecialUse;
use crate::search;
use crate::thread;

mod cleanup;
mod migrations;
mod prefs;
mod screener;
mod sql;

pub use cleanup::{CLEANUP_RANGES, CLEANUP_SCOPES, CleanupGroup, CleanupSession};
#[cfg(test)]
use migrations::{migrate, table_columns};
pub use prefs::{PREF_ARRIVAL_BUBBLES, PREF_LANG, PREF_LAST_SYNC, PREFS_PER_ACCOUNT};
pub(crate) use screener::*;
pub(crate) use sql::*;

const SCHEMA: &str = "
PRAGMA foreign_keys = ON;
CREATE TABLE IF NOT EXISTS accounts (
    id       INTEGER PRIMARY KEY,
    email    TEXT NOT NULL UNIQUE,
    provider TEXT NOT NULL DEFAULT 'gmail',
    -- The sent-mail folder, under its NETWORK name, when the server
    -- exposes one. It completes the threading scope (ADR 0009), and its
    -- name varies from one server to another — so it cannot be hardcoded.
    --
    -- Carried by the ACCOUNT and not deduced on the fly: the 'Sent'
    -- mailbox is CREATED by the sync loop, so it does not exist yet when
    -- the scope is declared. Without this memory it would be born out of
    -- scope and its messages would stay threadless until the next
    -- startup — the deferred-adoption trap.
    sent_mailbox TEXT
);
CREATE TABLE IF NOT EXISTS mailboxes (
    id             INTEGER PRIMARY KEY,
    account_id     INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    name           TEXT NOT NULL,
    uid_validity   INTEGER NOT NULL,
    last_uid       INTEGER NOT NULL DEFAULT 0,
    highest_modseq INTEGER,
    -- Has the mailbox ALREADY been synchronized once (PLAN-AUDIT-V1
    -- E2)? It is THIS flag that decides initial / incremental — never
    -- `last_uid == 0`: an EMPTIED mailbox (everything archived) has a
    -- null max(uid) and would go back to 'initial', so silent (no
    -- notification) and expensive.
    initialisee    INTEGER NOT NULL DEFAULT 0,
    -- Last SUCCESSFUL poll of the mailbox (epoch), set by update_state:
    -- the sweep of send echoes requires Sent to have been polled AFTER
    -- the send (PLAN-AUDIT-V2 E5).
    relevee_epoch  INTEGER,
    -- Does this mailbox take part in thread GROUPING?
    --
    -- Since ADR 0010 we synchronize ALL mailboxes, but a thread's scope
    -- stays INBOX + Sent (ADR 0009). Without this flag, a spam or an
    -- archived message would join the thread on its own —
    -- `thread::attach` works PER ACCOUNT — and would bump the
    -- conversation to the top of the list. A correctness defect, not an
    -- ergonomics one.
    --
    -- DEFAULT OF 1: this is the answer for MIGRATION, not for the
    -- product. A database from before ADR 0010 contains only INBOX and
    -- 'Sent', both in scope; setting them to 0 would empty the list on
    -- first launch. `create_mailbox` always writes the value explicitly,
    -- so this default never decides for a new mailbox.
    threaded       INTEGER NOT NULL DEFAULT 1,
    -- How many messages the SERVER announces in this mailbox (EXISTS),
    -- at the last pass. Denominator of the progress bar (ADR 0010 §5).
    --
    -- 0 = never selected. This is NOT 'empty mailbox': the two are kept
    -- distinct because the progress bar must stay silent when it does
    -- not know, instead of showing '0%' or '100%'.
    remote_total   INTEGER NOT NULL DEFAULT 0,
    UNIQUE (account_id, name)
);
CREATE TABLE IF NOT EXISTS envelopes (
    mailbox_id     INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    uid            INTEGER NOT NULL,
    subject        TEXT,
    sender         TEXT,
    sender_address TEXT,
    -- To / Cc recipients, one per newline, NULL when the ENVELOPE
    -- carries none (R4, PLAN-RETOURS-MAIL). They come from the SAME
    -- ENVELOPE as the sender: in a sent folder the sender is US, only
    -- the recipient says who the message went to.
    to_addrs       TEXT,
    cc_addrs       TEXT,
    -- Reply-To, first address, from the same ENVELOPE (PLAN-AUDIT-V2 E5).
    reply_to       TEXT,
    message_id     TEXT,
    -- The two headers used for thread grouping. `in_reply_to` comes from
    -- the ENVELOPE (free); `refs` comes from a separate pass over the
    -- full headers, and stays NULL until then.
    in_reply_to    TEXT,
    refs           TEXT,
    thread_id      INTEGER,
    date_epoch     INTEGER,
    seen           INTEGER NOT NULL DEFAULT 0,
    flagged        INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (mailbox_id, uid)
);
-- `uid` as the THIRD column, and that is not decoration: without it the
-- index does not COVER the backfill's queries, which filter by date
-- then probe `bodies` by (mailbox_id, uid). SQLite then had to fetch
-- the envelope ROW just to read its uid, once per message. Measured on
-- 2026-08-26 on the field database: `pending_total` 521.9 ms with the
-- two-column index, 107.9 ms with this one (worst folder, 87,117
-- envelopes: 400.5 -> 46.3 ms). The DESC order on the date stays the
-- pagination order; uid does not disturb it, it completes it.
CREATE INDEX IF NOT EXISTS idx_envelopes_date ON envelopes(mailbox_id, date_epoch DESC, uid);
CREATE TABLE IF NOT EXISTS bodies (
    mailbox_id INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    uid        INTEGER NOT NULL,
    html       TEXT NOT NULL,
    -- VESTIGIAL since 2026-08-26 (PLAN-DEMARRAGE, decision D8): written
    -- to 1 by save_body_full, never READ again. It marked bodies fetched
    -- BEFORE attachments existed, whose MIME had never been inspected;
    -- the backfill picked them up again. Reading it cost 251k fat-row
    -- lookups over 11.4 GB — 20,839 ms cold versus 396 ms without
    -- (measured 2026-08-26) — to protect ZERO rows: the legacy pass is
    -- closed out fleet-wide, and nothing in production writes 0 anymore.
    -- Removing it would need an 11.4 GB rewrite: it will leave with
    -- whichever job next touches `bodies` anyway.
    scanned    INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (mailbox_id, uid)
);
-- Metadata only, never the bytes. They get redownloaded on demand
-- (ADR 0007 — the disk budget would not survive keeping the files).
CREATE TABLE IF NOT EXISTS attachments (
    mailbox_id INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    uid        INTEGER NOT NULL,
    idx        INTEGER NOT NULL,
    name       TEXT NOT NULL,
    mime       TEXT NOT NULL,
    size       INTEGER NOT NULL,
    PRIMARY KEY (mailbox_id, uid, idx)
);
-- Folder list, cached like the envelopes: picking a destination must
-- work OFFLINE, otherwise sorting stops with the network. Refreshed on
-- every sync.
CREATE TABLE IF NOT EXISTS folders (
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    wire       TEXT NOT NULL,
    display    TEXT NOT NULL,
    selectable INTEGER NOT NULL DEFAULT 1,
    -- The RFC 6154 role (all, archive, drafts, junk, sent, trash), NULL
    -- when the server does not announce one (PLAN-AUDIT-V2 E5).
    special_use TEXT,
    PRIMARY KEY (account_id, wire)
);
CREATE TABLE IF NOT EXISTS pending_actions (
    id         INTEGER PRIMARY KEY,
    mailbox_id INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    uid        INTEGER NOT NULL,
    kind       TEXT NOT NULL,
    -- PLAN-AUDIT-V1 E3: an action the server refuses (NO/BAD) or that
    -- fails QUARANTINE_THRESHOLD times enters QUARANTINE (refusee = 1):
    -- it leaves the active queue — nothing blocks the next ones anymore
    -- — but stays visible (notice slot, D2) with its reason.
    attempts   INTEGER NOT NULL DEFAULT 0,
    refusee    INTEGER NOT NULL DEFAULT 0,
    last_error TEXT
);
CREATE INDEX IF NOT EXISTS idx_pending_actions_message ON pending_actions(mailbox_id, uid);
CREATE TABLE IF NOT EXISTS drafts (
    id            INTEGER PRIMARY KEY,
    account_id    INTEGER NOT NULL DEFAULT 1,
    to_raw        TEXT NOT NULL,
    -- Raw Cc and Bcc, unvalidated (like to_raw) — strict validation only
    -- happens at send time (compose). Empty by default: a draft from
    -- before these columns has neither.
    cc_raw        TEXT NOT NULL DEFAULT '',
    bcc_raw       TEXT NOT NULL DEFAULT '',
    subject       TEXT NOT NULL,
    body          TEXT NOT NULL,
    -- Rich body of the draft (PLAN-COMPOSITION-HTML). NULL = plain text
    -- draft (from before the column, or fetched from the server);
    -- `body` is ALWAYS populated — the derived text serves as preview
    -- and fallback.
    body_html     TEXT,
    reply_to_uid  INTEGER,
    -- The mailbox that gives reply_to_uid its meaning (ADR 0009) — the
    -- draft -> conversation link (PLAN-BROUILLONS, B-D2). NULL before
    -- the column: those drafts stay threadless, never wrongly linked.
    reply_to_mailbox TEXT,
    -- Marked 'important' (R3, PLAN-RETOURS-6): the state follows the
    -- draft until it is sent.
    important     INTEGER NOT NULL DEFAULT 0,
    updated_epoch INTEGER NOT NULL,
    remote_uid    INTEGER,
    pushed_epoch  INTEGER
);
-- The bytes of a draft's attachments, copied ON GESTURE
-- (PLAN-PIECES-JOINTES, PJ-D1): never a bare path in the database — a
-- file moved or deleted after the gesture cannot break anything. The
-- opposite of `attachments` (receiving, metadata only): here the bytes
-- are OURS — this is the message we promise to send.
CREATE TABLE IF NOT EXISTS draft_attachments (
    id       INTEGER PRIMARY KEY,
    draft_id INTEGER NOT NULL REFERENCES drafts(id) ON DELETE CASCADE,
    name     TEXT NOT NULL,
    mime     TEXT NOT NULL,
    size     INTEGER NOT NULL,
    bytes    BLOB NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_draft_attachments_draft ON draft_attachments(draft_id);
CREATE TABLE IF NOT EXISTS draft_tombstones (
    account_id INTEGER NOT NULL,
    remote_uid INTEGER NOT NULL,
    PRIMARY KEY (account_id, remote_uid)
);
CREATE TABLE IF NOT EXISTS drafts_remote (
    account_id   INTEGER PRIMARY KEY,
    uid_validity INTEGER NOT NULL
);
-- App preferences persisted IN THE DATABASE (not localStorage): they
-- must be readable by the Rust shell — the arrival-notification guard
-- is enforced at emission, on the Rust side (PLAN-REGLAGES, R-D2).
CREATE TABLE IF NOT EXISTS prefs (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS outbox (
    id           INTEGER PRIMARY KEY,
    account_id   INTEGER NOT NULL DEFAULT 1,
    message_id   TEXT NOT NULL,
    sender       TEXT NOT NULL,
    recipients   TEXT NOT NULL,
    cc_addrs     TEXT NOT NULL DEFAULT '',
    bcc_addrs    TEXT NOT NULL DEFAULT '',
    subject      TEXT NOT NULL,
    body_text    TEXT NOT NULL,
    -- Rich body of the send (PLAN-COMPOSITION-HTML): what the text/html
    -- part of the multipart/alternative carries. NULL = plain-text send
    -- (historical path, byte for byte unchanged).
    body_html    TEXT,
    in_reply_to  TEXT,
    -- E7: the full References chain (RFC 5322 §3.6.4), NULL = the
    -- parent alone (the older path). `refs` as in envelopes: REFERENCES
    -- is an SQLite reserved word.
    refs         TEXT,
    -- Marked 'important' (R3): delivery will set the priority headers
    -- (X-Priority + Importance).
    important    INTEGER NOT NULL DEFAULT 0,
    -- Delayed send (R2, PLAN-RETOURS-6): the epoch (seconds) before
    -- which the flush must NOT pick up this message. NULL = right away
    -- (historical path).
    send_at_epoch INTEGER,
    -- iTIP reply to an invitation (PLAN-INVITATIONS): delivery carries
    -- it in a text/calendar part; method=REPLY. NULL = ordinary send
    -- (historical path, byte for byte unchanged).
    ics_reply    TEXT,
    state        TEXT NOT NULL DEFAULT 'queued',
    attempts     INTEGER NOT NULL DEFAULT 0,
    last_error   TEXT,
    queued_epoch INTEGER NOT NULL
);
-- The attachments of the outgoing journal, copied from
-- `draft_attachments` in the send gesture's transaction (PJ-D2: 'never
-- a lost send' covers the bytes). `bytes` goes to NULL on the move to
-- `sent` (PJ-D7): the metadata stays readable, quarantine and refusal
-- keep their bytes — a resend on the user's decision must stay whole.
CREATE TABLE IF NOT EXISTS outbox_attachments (
    id        INTEGER PRIMARY KEY,
    outbox_id INTEGER NOT NULL REFERENCES outbox(id) ON DELETE CASCADE,
    name      TEXT NOT NULL,
    mime      TEXT NOT NULL,
    size      INTEGER NOT NULL,
    bytes     BLOB
);
CREATE INDEX IF NOT EXISTS idx_outbox_attachments_outbox ON outbox_attachments(outbox_id);
-- The local echo of a gesture (PLAN-REACTIVITE E3, R-D1 '< 1 s'): the
-- DESTINATION copy of a deletion, an archiving, or a send, visible in
-- the list BEFORE the server has caught up. NEVER in `envelopes`: a
-- made-up UID would forge the (mailbox, uid) key everything rests on.
-- The echo dies at reconciliation (the real row arrives, same
-- message_id) or at the sweep (the server denies it). `destination` is
-- a canonical category: 'envoyes' | 'archives' | 'corbeille'.
-- `origin_action_id` (logged gesture) and `origin_outbox_id` (send)
-- say the INTENT the echo reflects — an echo without intent does not
-- exist.
CREATE TABLE IF NOT EXISTS echos (
    id               INTEGER PRIMARY KEY,
    account_id       INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    destination      TEXT NOT NULL,
    message_id       TEXT NOT NULL,
    sender           TEXT,
    sender_address   TEXT,
    subject          TEXT,
    date_epoch       INTEGER,
    preview          TEXT,
    html             TEXT,
    attachment_count INTEGER NOT NULL DEFAULT 0,
    -- PLAN-RETOURS-5: the recipients of the echo, in the same format as
    -- envelopes (addresses joined by a newline) — the Sent list shows
    -- 'To: X', never the destination slug. NULL on existing rows
    -- (echoes die at reconciliation anyway).
    -- D-36: NEVER a Rust escape sequence (backslash-n) in this SQL
    -- comment — it turned into a real newline and SQLite swallowed
    -- what followed as a phantom column (fresh database, 2026-08-26).
    to_addrs         TEXT,
    origin_action_id INTEGER,
    origin_outbox_id INTEGER,
    created_epoch    INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_echos_destination ON echos(destination, account_id);
-- The contacts directory (PLAN-RETOURS-5, D4): learned from mail seen
-- (senders outside junk/trash, recipients of OUR sends), never a
-- hand-edited address book. Address lowercased (de-duplication), the
-- most recent display name wins. A SMALL table queried on keystrokes —
-- never a scan of envelopes per keystroke in the serialized queue
-- (lesson from PLAN-DEFILEMENT-PROFOND).
CREATE TABLE IF NOT EXISTS correspondants (
    address    TEXT PRIMARY KEY,
    name       TEXT,
    last_epoch INTEGER NOT NULL DEFAULT 0,
    hits       INTEGER NOT NULL DEFAULT 0
);
-- The LOCAL pin of a conversation (PLAN-RETOURS-7, R4): keyed by
-- ENVELOPE, not thread — thread tables get DROPped on adoption
-- (thread::drop_if_outdated), a pin carried by `threads` would die at
-- the next migration. The thread is found back through a join. NEVER
-- the `flagged` column: it is overwritten by server truth on every
-- sync (upsert_envelopes), and the IMAP star is a different semantics.
-- Local by decision (D-refus): IMAP has no such concept.
CREATE TABLE IF NOT EXISTS pins (
    mailbox_id INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    uid        INTEGER NOT NULL,
    epoch      INTEGER NOT NULL,
    PRIMARY KEY (mailbox_id, uid)
);
-- Set aside (PLAN-MODE-ORGANISE E5): the organized mode's pile — a
-- copy of the `pins` pattern (ENVELOPE key: survives thread rebuilding,
-- dies with its mailbox — `reset_mailbox`/`remove_local` purges
-- included, lesson from RETOURS-11). A thread that is set aside leaves
-- ALL organized views; 'Done' (DELETE) sends it back where it came
-- from. The classic view knows nothing of it.
CREATE TABLE IF NOT EXISTS mis_de_cote (
    mailbox_id INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    uid        INTEGER NOT NULL,
    epoch      INTEGER NOT NULL,
    PRIMARY KEY (mailbox_id, uid)
);
-- The Feed's 'read' memory (RETOURS-13 R10): a card scrolled all the
-- way down is read — a copy of the `pins`/`mis_de_cote` pattern
-- (envelope key, local to the workstation, dies with its mailbox and
-- its message). IMAP 'read' (`seen`) is a different semantics: it is
-- overwritten by server truth on every sync, and the Feed does not
-- 'process' anything.
CREATE TABLE IF NOT EXISTS kiosque_lus (
    mailbox_id INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    uid        INTEGER NOT NULL,
    epoch      INTEGER NOT NULL,
    PRIMARY KEY (mailbox_id, uid)
);
-- The images-guard memory (PLAN-RETOURS-11, R1 — D1 reverses invariant
-- A43): two EXPLICIT exceptions to blocking by default, never a global
-- setting. Per MESSAGE: envelope key, the `pins` pattern (survives
-- thread rebuilding, dies with its mailbox). Per SENDER: exact address
-- lowercased (normalized by the Rust side, like `correspondants`),
-- GLOBAL to the workstation (D3 — survives an account being removed).
CREATE TABLE IF NOT EXISTS images_messages (
    mailbox_id INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    uid        INTEGER NOT NULL,
    epoch      INTEGER NOT NULL,
    PRIMARY KEY (mailbox_id, uid)
);
CREATE TABLE IF NOT EXISTS images_expediteurs (
    address TEXT PRIMARY KEY,
    epoch   INTEGER NOT NULL
);
-- Organized mode routing (PLAN-MODE-ORGANISE E1, decision D1: LOCAL
-- routing only — the destination is a PRESENTATION, never an IMAP
-- move; other clients see the mail unchanged). Key: exact address
-- lowercased (the SAME normalization authority as the images guard),
-- GLOBAL to the workstation like images_expediteurs — the verdict on a
-- sender survives an account being removed. `regle`: the No automatism
-- (spam/archive/trash — D4: NEVER a permanent deletion), NULL =
-- screened out with no rule; a rule only exists on a `ecarte` sender.
-- 'Reinstate' from the Screener's history = DELETE of the row. The
-- vocabulary is checked in Rust BEFORE the write; the CHECKs are only
-- the belt.
CREATE TABLE IF NOT EXISTS routage_expediteurs (
    address     TEXT PRIMARY KEY,
    destination TEXT NOT NULL CHECK (destination IN ('reception','kiosque','registre','ecarte')),
    regle       TEXT CHECK (regle IN ('spam','archive','corbeille')),
    epoch       INTEGER NOT NULL
);
-- The Screener's waiting list (PLAN-MODE-ORGANISE E2, D3 'arrivals
-- only'): senders WITHOUT a routing row whose mail only exists AFTER
-- the activation epoch. MATERIALIZED and maintained ON ARRIVAL (spike
-- S2-bis: computing it in the hot query costs 299 ms at deep offset,
-- the PK probe is free; upkeep costs 7 µs/message). DERIVED from the
-- mail — never a decision: it undoes itself when older mail arrives
-- (backfill) or disappears (reset), and dies at the verdict (the
-- routing row takes over). A single column: membership IS the data —
-- everything else (last message, counts) is read from the mail.
CREATE TABLE IF NOT EXISTS portier_attente (
    address TEXT PRIMARY KEY
);
-- The Spring cleaning session (PLAN-HORIZON-NETTOYAGE part B, D8:
-- persisted — a cleanup started resumes after a restart). AT MOST one
-- row (id = 1). The bound is FIXED at start (borne_epoch, derived from
-- the chosen range); verdicts live in routage_expediteurs — the
-- session only carries the range, the scope and the progress (total
-- groups at the start, handled).
CREATE TABLE IF NOT EXISTS nettoyage_session (
    id          INTEGER PRIMARY KEY CHECK (id = 1),
    plage       TEXT NOT NULL,
    perimetre   TEXT NOT NULL,
    borne_epoch INTEGER NOT NULL,
    total       INTEGER NOT NULL,
    traites     INTEGER NOT NULL DEFAULT 0
);
-- A message's meeting invitation (PLAN-INVITATIONS): the CACHE of the
-- text/calendar part, extracted when the body is scanned
-- (save_body_full) or on open for a message from before the feature
-- (write-back, adoption invariant 6.7 — never a mass migration).
-- Envelope key, like `attachments`; the raw MIME is never stored.
-- `partstat` = our READ status from the REQUEST; `reponse` = our last
-- reply SENT via the outbox (D6) — two distinct truths. Epochs are
-- UTC; when a time cannot be resolved (all-day event, unknown TZID),
-- the TEXT form is authoritative and the epoch stays NULL (guard D1:
-- never a misleading conversion).
CREATE TABLE IF NOT EXISTS invitations (
    mailbox_id           INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    uid                  INTEGER NOT NULL,
    methode              TEXT NOT NULL,
    event_uid            TEXT NOT NULL,
    sequence             INTEGER NOT NULL DEFAULT 0,
    titre                TEXT NOT NULL DEFAULT '',
    lieu                 TEXT,
    organisateur_adresse TEXT,
    organisateur_nom     TEXT,
    debut_epoch          INTEGER,
    fin_epoch            INTEGER,
    debut_texte          TEXT,
    fin_texte            TEXT,
    journee_entiere      INTEGER NOT NULL DEFAULT 0,
    recurrent            INTEGER NOT NULL DEFAULT 0,
    partstat             TEXT,
    repondant_adresse    TEXT,
    repondant_nom        TEXT,
    repondant_statut     TEXT,
    -- The CROSSED cancellation link (field finding R6, 2026-08-22): a
    -- CANCEL extinguishes the REQUEST of the same meeting (same
    -- event_uid, same account), regardless of scan arrival order —
    -- without it, the cancellation would land in a new conversation and
    -- the original invitation would keep offering Accept.
    annule               INTEGER NOT NULL DEFAULT 0,
    reponse              TEXT,
    reponse_epoch        INTEGER,
    PRIMARY KEY (mailbox_id, uid)
);
";

/// Writes (or replaces) a message's invitation row, PRESERVING our
/// local reply: `reply`/`reply_epoch` are never touched here (D6)
/// — the PARTSTAT reread from the message and the reply Wind sent are
/// two distinct truths.
fn write_invitation(
    conn: &Connection,
    mailbox_id: i64,
    uid: Uid,
    row: &InvitationRow,
) -> Result<(), Error> {
    // The crossed cancellation link (field finding R6), in BOTH arrival
    // orders: a REQUEST written AFTER the CANCEL of its meeting is born
    // cancelled; a CANCEL written AFTER extinguishes existing REQUESTs.
    // The meeting is identified by (event_uid, account) — never
    // event_uid alone: two accounts can receive the same meeting.
    let cancelled = row.cancelled
        || (row.method == "request"
            && conn
                .prepare(
                    "SELECT 1 FROM invitations i
                      JOIN mailboxes m ON m.id = i.mailbox_id
                     WHERE i.event_uid = ?1 AND i.methode = 'cancel'
                       AND m.account_id =
                           (SELECT account_id FROM mailboxes WHERE id = ?2)",
                )?
                .exists(params![row.event_uid, mailbox_id])?);
    conn.execute(
        "INSERT INTO invitations (mailbox_id, uid, methode, event_uid, sequence, titre,
             lieu, organisateur_adresse, organisateur_nom, debut_epoch, fin_epoch,
             debut_texte, fin_texte, journee_entiere, recurrent, partstat,
             repondant_adresse, repondant_nom, repondant_statut, annule)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
             ?16, ?17, ?18, ?19, ?20)
         ON CONFLICT(mailbox_id, uid) DO UPDATE SET
             methode = excluded.methode, event_uid = excluded.event_uid,
             sequence = excluded.sequence, titre = excluded.titre,
             lieu = excluded.lieu,
             organisateur_adresse = excluded.organisateur_adresse,
             organisateur_nom = excluded.organisateur_nom,
             debut_epoch = excluded.debut_epoch, fin_epoch = excluded.fin_epoch,
             debut_texte = excluded.debut_texte, fin_texte = excluded.fin_texte,
             journee_entiere = excluded.journee_entiere,
             recurrent = excluded.recurrent, partstat = excluded.partstat,
             repondant_adresse = excluded.repondant_adresse,
             repondant_nom = excluded.repondant_nom,
             repondant_statut = excluded.repondant_statut,
             annule = excluded.annule",
        params![
            mailbox_id,
            uid,
            row.method,
            row.event_uid,
            row.sequence,
            row.title,
            row.location,
            row.organizer_address,
            row.organizer_name,
            row.start_epoch,
            row.end_epoch,
            row.start_text,
            row.end_text,
            row.all_day,
            row.recurrent,
            row.partstat,
            row.attendee_address,
            row.attendee_name,
            row.attendee_status,
            cancelled
        ],
    )?;
    if row.method == "cancel" {
        conn.execute(
            "UPDATE invitations SET annule = 1
             WHERE event_uid = ?1 AND methode = 'request' AND annule = 0
               AND mailbox_id IN
                   (SELECT id FROM mailboxes WHERE account_id =
                        (SELECT account_id FROM mailboxes WHERE id = ?2))",
            params![row.event_uid, mailbox_id],
        )?;
    }
    Ok(())
}

/// Progress of a legacy database's adoption, for display.
///
/// `total` is an UPPER BOUND declared upfront (it never moves during a
/// pass: a progress bar that goes backward is worse than an imprecise
/// one), and `fait == total` is only announced once the pass is
/// COMMITTED — never before, that is the "a signal must be observable"
/// requirement (§9 of the handover). Display goes through
/// [`crate::sync_percent`], which already handles the degenerate
/// cases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdoptionProgress {
    pub done: u64,
    pub total: u64,
}

/// Persisted sync state of a mailbox.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncState {
    pub mailbox_id: i64,
    pub uid_validity: u32,
    pub last_uid: Uid,
    pub highest_modseq: Option<u64>,
    /// The mailbox has already been synced once: this is what decides
    /// initial / incremental (E2), never `last_uid`.
    pub initialized: bool,
}

/// An account connected to the client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    pub id: i64,
    pub email: String,
    pub provider: String,
}

/// A row of the unified inbox: the envelope AND its account — a UID
/// alone no longer identifies a message once several accounts are in
/// play.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnifiedRow {
    pub account_id: i64,
    pub account_email: String,
    /// The mailbox that CONTAINS this message, under its network name.
    ///
    /// Without it, `(account_id, uid)` no longer identifies anything
    /// now that threads span several mailboxes ([ADR 0009]): UIDs are
    /// assigned per mailbox and restart at 1, so message #1 of INBOX
    /// and #1 of "Sent" are two different messages of the same
    /// account. Every read and every action must carry it.
    pub mailbox: String,
    pub envelope: Envelope,
    /// Does the message carry at least one attachment?
    ///
    /// False until its body has been read — the same condition as text
    /// search. The paperclip therefore appears as backfill proceeds,
    /// never wrongly.
    pub has_attachment: bool,
    /// HOW MANY attachments — the prototype's chip says "2 files", not
    /// "some files". 0 until the body has been read, same as
    /// `has_attachment`.
    pub attachment_count: u32,
    /// The text preview under the subject (screen 02) — computed when
    /// the body is written, `None` until the body is fetched.
    pub preview: Option<String>,
    /// The thread this message belongs to. `None` only during the
    /// window where a legacy database has not yet been adopted.
    pub thread_id: Option<i64>,
    /// Number of messages in the thread, **received and sent
    /// combined**. 1 = a lone message.
    ///
    /// Since ADR 0009, a thread belongs to the ACCOUNT and not to a
    /// mailbox: our own replies are part of it. The counter must
    /// therefore include them, or it would contradict the conversation
    /// banner on screen, which shows the whole exchange.
    pub thread_size: u32,
    /// Unread count of the thread. A thread shows as unread as long as
    /// one remains, even if its last message is read.
    pub thread_unseen: u32,
    /// The thread's invitation (field findings R10/R11) — set by
    /// [`Store::enrichir_lignes`] on the served PAGE, never by the hot
    /// query. `None` on every other path.
    pub invitation: Option<InvitationRank>,
}

/// The invitation of a list row (field findings R10/R11): what the
/// chip row shows (reply given, cancellation) and the key to reply
/// FROM the list — the mailbox and UID of the invitation message,
/// which is not necessarily the displayed head of the thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvitationRank {
    pub mailbox: String,
    pub uid: Uid,
    /// The meeting's title — the reply's subject is built from it,
    /// never from the thread head's subject ("Re: …").
    pub title: String,
    /// Our last sent reply (`accepte`|`provisoire`|`refuse`).
    pub reply: Option<String>,
    pub cancelled: bool,
    pub can_reply: bool,
}

/// The exclusion for the ORGANIZED Inbox — THE single place it is
/// written (review E4/E5: the fragment lived in four copies, the next
/// exclusion — E6, groups — would have missed one, exactly the "badge
/// shows 2 in front of an empty list" bug the E5 screenshot caught):
/// retained/routed threads (flag) and SET-ASIDE threads (the pile).
pub(crate) fn organized_exclusion() -> String {
    format!(" AND organise_hors = 0 AND id NOT IN ({SET_ASIDE_THREADS})")
}

/// The sort of the unified stream — plain date (classic) or the
/// SECTIONS of the organized Inbox (E4, verdict S1/A2): unread first —
/// "New for you" — then the rest — "Already seen" —, date within each
/// section. ONE stream, ONE offset: the order carries the sections,
/// the seam is the unread COUNT (0.37 ms measured).
pub(crate) fn unified_join_tail(sections: bool) -> String {
    let ordre = if sections {
        "ORDER BY (t.unseen > 0) DESC, t.last_epoch DESC, t.last_uid DESC, a.id"
    } else {
        "ORDER BY t.last_epoch DESC, t.last_uid DESC, a.id"
    };
    format!("{UNIFIED_JOINS}\n         {ordre}")
}

pub struct Store(Connection);

impl Store {
    /// Access reserved to crate modules that extend storage (the
    /// outbox, in `outbox.rs`) without growing this file.
    pub(crate) fn conn(&self) -> &Connection {
        &self.0
    }

    pub fn open(path: &Path) -> Result<Self, Error> {
        Self::init(Connection::open(path)?)
    }

    /// Opens while making a legacy database's adoption VISIBLE and
    /// INTERRUPTIBLE (Phase 5, arbitrated job — handover §8).
    ///
    /// `on_progress` is called during the adoption pass with progress
    /// `(done, total)`. Answering [`ControlFlow::Break`] cancels:
    /// **everything is undone** (`ROLLBACK`), `PRAGMA user_version`
    /// stays unchanged, and opening returns [`Error::Interrupted`] —
    /// the whole pass will replay on the next launch. Never a partial
    /// adoption persisted: the list starts from `threads`, a
    /// half-adopted database would be a half-empty mailbox.
    ///
    /// On an up-to-date database, `on_progress` is NEVER called:
    /// nothing to adopt, nothing to report — no fake banner on every
    /// launch.
    pub fn open_with_progress(
        path: &Path,
        mut on_progress: impl FnMut(AdoptionProgress) -> ControlFlow<()>,
    ) -> Result<Self, Error> {
        Self::init_with(Connection::open(path)?, &mut on_progress)
    }

    pub fn open_in_memory() -> Result<Self, Error> {
        Self::init(Connection::open_in_memory()?)
    }

    pub fn sync_state(&self, account_id: i64, mailbox: &str) -> Result<Option<SyncState>, Error> {
        let state = self
            .0
            .query_row(
                "SELECT id, uid_validity, last_uid, highest_modseq, initialisee
                 FROM mailboxes WHERE account_id = ?1 AND name = ?2",
                params![account_id, mailbox],
                |row| {
                    Ok(SyncState {
                        mailbox_id: row.get(0)?,
                        uid_validity: row.get(1)?,
                        last_uid: row.get(2)?,
                        highest_modseq: row.get::<_, Option<i64>>(3)?.map(|m| m as u64),
                        initialized: row.get::<_, i64>(4)? != 0,
                    })
                },
            )
            .optional()?;
        Ok(state)
    }

    /// Registers a mailbox. It only enters the grouping scope if it is
    /// the inbox: the "Sent" folder also enters it, but its NAME varies
    /// from one server to another (ADR 0009 §7), so only the caller
    /// that discovered it can declare it — [`Store::set_thread_scope`].
    ///
    /// Every other one — Archive, Trash, Spam, user folders — is
    /// stored and indexed, never grouped (ADR 0010 §3).
    pub fn create_mailbox(
        &self,
        account_id: i64,
        mailbox: &str,
        uid_validity: u32,
    ) -> Result<i64, Error> {
        // `COALESCE`: without it, an account with no known sent folder
        // would make `?2 = NULL` — hence NULL — and `false OR NULL` is
        // NULL in SQL. The column being NOT NULL, the insert would fail
        // for every ordinary folder of an account that has not yet
        // discovered its sent folder. That is, on the very first pass.
        self.0.execute(
            "INSERT INTO mailboxes (account_id, name, uid_validity, threaded)
             VALUES (?1, ?2, ?3,
                     ?2 = ?4 OR COALESCE(
                         ?2 = (SELECT sent_mailbox FROM accounts WHERE id = ?1), 0))",
            params![account_id, mailbox, uid_validity, thread::RECEIVED_MAILBOX],
        )?;
        Ok(self.0.last_insert_rowid())
    }

    /// An account's mailboxes, in the order backfill should serve them:
    /// the inbox first (it is what the list shows and what day-to-day
    /// search queries), sent next (it completes threads), the rest by
    /// name — deterministic, hence resumable from one session to the
    /// next.
    ///
    /// OFFLINE mirror of `sync_order`: same priority, but the source is
    /// the database and not the server — backfill must not pay for a
    /// LIST just to know what to pump.
    pub fn mailbox_names(&self, account_id: i64) -> Result<Vec<String>, Error> {
        let mut stmt = self.0.prepare(
            "SELECT name FROM mailboxes WHERE account_id = ?1
             ORDER BY (name = ?2) DESC,
                      (name = (SELECT sent_mailbox FROM accounts WHERE id = ?1)) DESC,
                      name",
        )?;
        let names = stmt
            .query_map(params![account_id, thread::RECEIVED_MAILBOX], |row| {
                row.get(0)
            })?
            .collect::<Result<_, _>>()?;
        Ok(names)
    }

    /// How many messages this ACCOUNT holds in the database, across all
    /// mailboxes. This is the "already done" the disk-space guard
    /// subtracts from what the servers announce (ADR 0010 §4): without
    /// it, a mailbox already three-quarters fetched would be refused as
    /// if everything remained to download.
    pub fn account_message_count(&self, account_id: i64) -> Result<u64, Error> {
        let count: i64 = self.0.query_row(
            "SELECT COUNT(*) FROM envelopes e
             JOIN mailboxes m ON m.id = e.mailbox_id
             WHERE m.account_id = ?1",
            [account_id],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }

    /// The UIDNEXT seen at the poll that preceded the last poll
    /// committed (ADR 0017) — `None` as long as no committed poll has
    /// happened.
    pub fn remote_uidnext(&self, mailbox_id: i64) -> Result<Option<u32>, Error> {
        Ok(self.0.query_row(
            "SELECT remote_uidnext FROM mailboxes WHERE id = ?1",
            params![mailbox_id],
            |row| row.get(0),
        )?)
    }

    /// Sets the UIDNEXT seen — AFTER the poll has been committed, never
    /// before: a marker set on an interrupted poll would make a folder
    /// that has not yet caught up get skipped.
    pub fn set_remote_uidnext(&self, mailbox_id: i64, uidnext: u32) -> Result<(), Error> {
        self.0.execute(
            "UPDATE mailboxes SET remote_uidnext = ?2 WHERE id = ?1",
            params![mailbox_id, uidnext],
        )?;
        Ok(())
    }

    /// Messages in the database for THIS folder — the local counterpart
    /// of STATUS's MESSAGES, compared by `must_poll` (ADR 0017).
    pub fn envelope_count(&self, mailbox_id: i64) -> Result<u64, Error> {
        let count: i64 = self.0.query_row(
            "SELECT COUNT(*) FROM envelopes WHERE mailbox_id = ?1",
            params![mailbox_id],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }

    /// Are any local actions waiting to be replayed in this folder?
    /// EXISTS and not the list: the question is closed, so is the
    /// answer.
    pub fn has_pending_actions(&self, mailbox_id: i64) -> Result<bool, Error> {
        Ok(self.0.query_row(
            "SELECT EXISTS(SELECT 1 FROM pending_actions WHERE mailbox_id = ?1 AND refusee = 0)",
            params![mailbox_id],
            |row| row.get(0),
        )?)
    }

    /// Records what the server announces in this mailbox (EXISTS).
    pub fn record_remote_total(&self, mailbox_id: i64, exists: u32) -> Result<(), Error> {
        self.0.execute(
            "UPDATE mailboxes SET remote_total = ?2 WHERE id = ?1",
            params![mailbox_id, exists],
        )?;
        Ok(())
    }

    /// Sync progress, across all mailboxes and all accounts: (messages
    /// in the database, messages announced by the servers).
    ///
    /// Only counts mailboxes ALREADY selected at least once
    /// (`remote_total > 0`). Otherwise an account where half the
    /// folders have not yet been visited would show progress that GOES
    /// BACKWARD as they are discovered — the denominator growing faster
    /// than the numerator. Progress going backward is worse than no
    /// progress at all.
    ///
    /// The denominator adjusts for PENDING departures awaiting replay
    /// (archive, delete, move — `pending_actions`): the gesture removes
    /// the local row immediately (echo, E3) but `remote_total` dates
    /// from the last SELECT — without the adjustment, a single sort
    /// action would freeze progress at 99% (never 100 as long as local
    /// < remote) and the status bar's line with it, for the whole
    /// duration of the replay (field finding 2026-08-15, PLAN-GELS).
    /// Marking (read, star) removes nothing: it does not touch the
    /// denominator. Floored at zero per mailbox: a `remote_total`
    /// running behind does not make the others go backward.
    pub fn sync_progress(&self) -> Result<(u64, u64), Error> {
        let (local, remote): (i64, i64) = self.0.query_row(
            "SELECT COALESCE(SUM(
                        (SELECT COUNT(*) FROM envelopes e WHERE e.mailbox_id = m.id)), 0),
                    COALESCE(SUM(MAX(0, m.remote_total -
                        (SELECT COUNT(*) FROM pending_actions p
                          WHERE p.mailbox_id = m.id AND p.refusee = 0
                            AND (p.kind IN ('archive', 'delete')
                                 OR p.kind LIKE 'move_to:%')))), 0)
             FROM mailboxes m WHERE m.remote_total > 0",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        Ok((local as u64, remote as u64))
    }

    /// Declares an account's grouping scope: the inbox, plus the sent
    /// folder when the server exposes one.
    ///
    /// The account's sent folder, under its NETWORK name — `None`
    /// until folder discovery has memorized it. This is the target of
    /// the targeted post-send poll: the copy the sending server adds
    /// must show up without waiting for a full cycle (field finding
    /// 0.1.4, 2026-08-14: 4 minutes with no visible copy).
    pub fn sent_mailbox(&self, account_id: i64) -> Result<Option<String>, Error> {
        Ok(self
            .0
            .query_row(
                "SELECT sent_mailbox FROM accounts WHERE id = ?1",
                [account_id],
                |row| row.get(0),
            )
            .optional()?
            .flatten())
    }

    /// Called AFTER folder discovery, on every sync: a server can
    /// rename its sent folder, and an account may have none — in which
    /// case threads only group received mail, exactly as before ADR
    /// 0009. Idempotent.
    pub fn set_thread_scope(&self, account_id: i64, sent: Option<&str>) -> Result<(), Error> {
        // E4: account and mailboxes agree or nothing — a single
        // transaction.
        let tx = self.0.unchecked_transaction()?;
        // Memorized on the account FIRST: this is the memory
        // `create_mailbox` will consult for the mailboxes the sync loop
        // has not created yet.
        self.0.execute(
            "UPDATE accounts SET sent_mailbox = ?2 WHERE id = ?1",
            params![account_id, sent],
        )?;
        self.0.execute(
            "UPDATE mailboxes SET threaded = (name = ?2 OR (?3 IS NOT NULL AND name = ?3))
             WHERE account_id = ?1",
            params![account_id, thread::RECEIVED_MAILBOX, sent],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Registers an account, or claims the account "waiting for
    /// adoption" created by the Phase 2 → 3 migration (empty email):
    /// the first login after the update is, in practice, the same
    /// Gmail account as before — its data is waiting for it.
    pub fn adopt_or_create_account(&self, email: &str, provider: &str) -> Result<i64, Error> {
        if let Some(id) = self.account_id(email)? {
            return Ok(id);
        }
        let claimed = self.0.execute(
            "UPDATE accounts SET email = ?1, provider = ?2
             WHERE email = '' AND id = (SELECT MIN(id) FROM accounts WHERE email = '')",
            params![email, provider],
        )?;
        if claimed == 0 {
            self.0.execute(
                "INSERT INTO accounts (email, provider) VALUES (?1, ?2)",
                params![email, provider],
            )?;
            return Ok(self.0.last_insert_rowid());
        }
        self.account_id(email)?
            .ok_or_else(|| Error::Corrupt("claimed account not found".to_string()))
    }

    fn account_id(&self, email: &str) -> Result<Option<i64>, Error> {
        let id = self
            .0
            .query_row("SELECT id FROM accounts WHERE email = ?1", [email], |row| {
                row.get(0)
            })
            .optional()?;
        Ok(id)
    }

    /// Known accounts — excluding any account waiting for adoption.
    pub fn accounts(&self) -> Result<Vec<Account>, Error> {
        let mut stmt = self
            .0
            .prepare("SELECT id, email, provider FROM accounts WHERE email != '' ORDER BY id")?;
        let rows = stmt
            .query_map([], |row| {
                Ok(Account {
                    id: row.get(0)?,
                    email: row.get(1)?,
                    provider: row.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// An account's server configuration (Gmail or generic IMAP).
    pub fn account_config(&self, account_id: i64) -> Result<AccountConfig, Error> {
        let config = self
            .0
            .query_row(
                "SELECT imap_host, imap_port, smtp_host, smtp_port, username
                 FROM accounts WHERE id = ?1",
                [account_id],
                |row| {
                    Ok(AccountConfig {
                        imap_host: row.get(0)?,
                        imap_port: row.get(1)?,
                        smtp_host: row.get(2)?,
                        smtp_port: row.get(3)?,
                        username: row.get(4)?,
                    })
                },
            )
            .optional()?
            .unwrap_or(AccountConfig {
                imap_host: None,
                imap_port: None,
                smtp_host: None,
                smtp_port: None,
                username: None,
            });
        Ok(config)
    }

    /// Creates or updates a generic IMAP/SMTP account.
    pub fn create_generic_account(
        &self,
        email: &str,
        username: &str,
        imap_host: &str,
        imap_port: u16,
        smtp_host: &str,
        smtp_port: u16,
    ) -> Result<i64, Error> {
        self.0.execute(
            "INSERT INTO accounts (email, provider, username, imap_host, imap_port, smtp_host, smtp_port)
             VALUES (?1, 'imap', ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(email) DO UPDATE SET
                provider = 'imap',
                username = excluded.username,
                imap_host = excluded.imap_host,
                imap_port = excluded.imap_port,
                smtp_host = excluded.smtp_host,
                smtp_port = excluded.smtp_port",
            params![
                email,
                username,
                imap_host,
                imap_port,
                smtp_host,
                smtp_port
            ],
        )?;
        // NEVER `last_insert_rowid()`: on the UPDATE path (re-add), no
        // row is inserted and it would return 0 (or an id from another
        // write on the connection). The database id is always
        // authoritative.
        self.account_id(email)?
            .ok_or_else(|| Error::Corrupt("generic account not found after write".to_string()))
    }

    /// Deletes an account and EVERYTHING attached to it, in one
    /// transaction.
    ///
    /// Prefixes of prefs suffixed per account live in
    /// [`PREFS_PER_ACCOUNT`] — the author of a new pref adds it THERE.
    ///
    /// The schema's cascades take mailboxes, envelopes, bodies,
    /// attachments, pending actions, folders and threads with them.
    /// Three families have NO foreign key and are cleared by hand: the
    /// search index (mailbox by mailbox, BEFORE the cascade makes the
    /// mailboxes disappear), the drafts (with tombstones and remote
    /// marker) and the outbox. Nothing must outlive the account — an
    /// orphan leftover would never be read again, but would keep
    /// showing up in search or leaving on the next flush.
    pub fn delete_account(&mut self, account_id: i64) -> Result<(), Error> {
        let tx = self.0.transaction()?;
        let mailboxes: Vec<i64> = {
            let mut stmt = tx.prepare("SELECT id FROM mailboxes WHERE account_id = ?1")?;
            stmt.query_map([account_id], |row| row.get(0))?
                .collect::<Result<Vec<i64>, _>>()?
        };
        for mailbox_id in mailboxes {
            search::deindex_mailbox(&tx, mailbox_id)?;
        }
        tx.execute("DELETE FROM drafts WHERE account_id = ?1", [account_id])?;
        tx.execute(
            "DELETE FROM draft_tombstones WHERE account_id = ?1",
            [account_id],
        )?;
        tx.execute(
            "DELETE FROM drafts_remote WHERE account_id = ?1",
            [account_id],
        )?;
        tx.execute("DELETE FROM outbox WHERE account_id = ?1", [account_id])?;
        // The prefs suffixed by the id (signature, account marker) die
        // with it: `accounts.id` is an INTEGER PRIMARY KEY without
        // AUTOINCREMENT — SQLite reuses the largest freed rowid, and an
        // account added after the removal would otherwise inherit the
        // old one's identity (review PLAN-RETOURS-8, 2026-08-22).
        for prefixe in PREFS_PER_ACCOUNT {
            tx.execute(
                "DELETE FROM prefs WHERE key = ?1",
                [format!("{prefixe}.{account_id}")],
            )?;
        }
        tx.execute("DELETE FROM accounts WHERE id = ?1", [account_id])?;
        // The Screener's waiting list follows the mail (E2): the rows
        // the cascades just cleared die with the account. Routing,
        // itself, is GLOBAL to the workstation and survives (the
        // `images_expediteurs` pattern).
        purge_orphan_pending(&tx)?;
        tx.commit()?;
        Ok(())
    }

    /// Starts from zero for a mailbox whose UIDVALIDITY has changed:
    /// the UIDs no longer mean anything — bodies and pending actions
    /// included (an intent on an invalidated UID cannot be carried out
    /// by construction).
    pub fn reset_mailbox(&self, mailbox_id: i64, uid_validity: u32) -> Result<(), Error> {
        // PLAN-AUDIT-V1 E4: ONE transaction — nine autocommit writes
        // left, on a crash between two of them, threads with no
        // envelopes (the "badge in front of an empty list" bug). Proven
        // by a trigger that refuses envelope deletion.
        let tx = self.0.unchecked_transaction()?;
        search::deindex_mailbox(&self.0, mailbox_id)?;
        // Pending actions: an intent on an invalidated UID cannot be
        // carried out by construction.
        self.0.execute(
            "DELETE FROM pending_actions WHERE mailbox_id = ?1",
            [mailbox_id],
        )?;
        // ALL per-message tables follow (review PLAN-INVITATIONS, R1
        // RETOURS-11, E5, R10 RETOURS-13): after a UIDVALIDITY change
        // the UIDs no longer mean anything — an invitation, an images
        // agreement, a set-aside mark or a "read" flag that survived
        // would stick to the message that recycles the UID. THE list is
        // `TABLES_PER_MESSAGE` (review PLAN-AUDIT-V1: no more copies).
        for table in TABLES_PER_MESSAGE {
            self.0.execute(
                &format!("DELETE FROM {table} WHERE mailbox_id = ?1"),
                [mailbox_id],
            )?;
        }
        // The Screener's waiting list is DERIVED from the mail (E2): a
        // row that no longer rests on anything dies with the mailbox —
        // a recycled UID inherits no waiting status (A43/A89).
        purge_orphan_pending(&self.0)?;
        self.0.execute(
            "UPDATE mailboxes
             SET uid_validity = ?2, last_uid = 0, highest_modseq = NULL
             WHERE id = ?1",
            params![mailbox_id, uid_validity],
        )?;
        // AFTER envelope deletion, never before: a thread is recomputed
        // from what remains. Doing it first would make it point at
        // messages about to be erased.
        //
        // And on the ACCOUNT, not the mailbox: since ADR 0009 a thread
        // can span INBOX and "Sent", so resetting one forces
        // reconsidering both.
        let account_id: i64 = self.0.query_row(
            "SELECT account_id FROM mailboxes WHERE id = ?1",
            [mailbox_id],
            |row| row.get(0),
        )?;
        thread::rebuild_account(&self.0, account_id)?;
        tx.commit()?;
        Ok(())
    }

    pub fn update_state(
        &self,
        mailbox_id: i64,
        last_uid: Uid,
        highest_modseq: Option<u64>,
    ) -> Result<(), Error> {
        // `initialisee = 1`: a pass that has committed — the next one
        // is incremental no matter what remains in the database (E2).
        self.0.execute(
            "UPDATE mailboxes SET last_uid = ?2, highest_modseq = ?3, initialisee = 1,
                                  relevee_epoch = unixepoch()
             WHERE id = ?1",
            params![mailbox_id, last_uid, highest_modseq.map(|m| m as i64)],
        )?;
        Ok(())
    }

    pub fn upsert_envelopes(
        &mut self,
        mailbox_id: i64,
        envelopes: &[Envelope],
    ) -> Result<(), Error> {
        // What the contacts directory learns from THIS mailbox —
        // resolved once per batch, like the thread (PLAN-RETOURS-5,
        // D4).
        let (note_senders, note_recipients) = self.directory_role(mailbox_id)?;
        // The Screener's epoch (E2) — read BEFORE the transaction (a
        // pref, never rewritten mid-batch). None = the mode has never
        // been activated, the arrival decision costs nothing.
        let screener_epoch = self.organized_mode_epoch()?;
        // The No rules (E3, D2): they only play in ACTIVE mode —
        // disabled, they SLEEP (the verdict stays recorded).
        let active_rules = self.organized_mode()?;
        // Resolved ONCE: the mailbox does not change within a batch,
        // and threading is now reasoned about per account (ADR 0009).
        // Doing it per message would add one query per envelope on the
        // hottest path of sync.
        // Same reason for the scope: it belongs to the mailbox, not the
        // message. Out of scope, we store and index without grouping —
        // `thread_id` stays NULL (ADR 0010 §3).
        let (account_id, threaded, mailbox_name): (i64, bool, String) = self.0.query_row(
            "SELECT account_id, threaded, name FROM mailboxes WHERE id = ?1",
            [mailbox_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        // The account's junk folder, resolved BEFORE the transaction
        // (the `spam` rule needs it); None = no recognized folder, the
        // spam rule does NOTHING — never a made-up destination. The
        // message then degrades to a "bare No": hidden from organized
        // mode (flag), never moved — a stated limit of the PLAN.
        let junk_folder = if active_rules {
            self.canonical_folders(account_id)?.junk
        } else {
            None
        };
        // Local removals decided during the batch — applied AFTER the
        // commit (`remove_local` recomputes thread and index in its own
        // transaction). The ACTION, itself, is logged WITHIN the
        // batch's transaction (review E3): a crash between the two
        // loses nothing — the intent is in the database, the replay
        // applies it to the server and the local copy leaves at the
        // next reconciliation.
        let mut no_removals: Vec<Uid> = Vec::new();
        let tx = self.0.transaction()?;
        // The Screener only judges ARRIVALS (E2, D3): the incoming-mail
        // mailbox, like `inbox_size`. A sender first seen in Junk or
        // Archive does not wait at the door — but their mail, wherever
        // it is, counts as "known before the epoch".
        let arrival = mailbox_name == thread::RECEIVED_MAILBOX;
        // Addresses whose waiting status was UNDONE in this batch
        // (their old mail arrives after the fact): their threads from
        // earlier batches recompute their flag after the loop.
        let mut undone_waiting: BTreeSet<String> = BTreeSet::new();
        // A SET, not a list: the same quadratic flaw measured in
        // adoption (`Vec::contains` is linear). Bounded here by the
        // batch size, hence less dramatic — but it is the same hot
        // path, and the same fix.
        let mut touched: BTreeSet<i64> = BTreeSet::new();
        {
            // `INSERT OR REPLACE` would reset to NULL any column absent
            // from the list — and `refs` as well as `thread_id` are
            // written by OTHER paths than sync. A re-sync would then
            // silently erase the header-backfill's work, exactly as it
            // would have erased attachments. So we enumerate the
            // columns sync owns, and only those.
            let mut stmt = tx.prepare(
                "INSERT INTO envelopes
                 (mailbox_id, uid, subject, sender, sender_address, message_id,
                  in_reply_to, date_epoch, seen, flagged, to_addrs, cc_addrs, reply_to)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                 ON CONFLICT (mailbox_id, uid) DO UPDATE SET
                     subject = excluded.subject,
                     sender = excluded.sender,
                     sender_address = excluded.sender_address,
                     message_id = excluded.message_id,
                     in_reply_to = excluded.in_reply_to,
                     date_epoch = excluded.date_epoch,
                     seen = excluded.seen,
                     flagged = excluded.flagged,
                     to_addrs = excluded.to_addrs,
                     cc_addrs = excluded.cc_addrs,
                     reply_to = excluded.reply_to",
            )?;
            let mut body_stmt =
                tx.prepare("SELECT html FROM bodies WHERE mailbox_id = ?1 AND uid = ?2")?;
            let mut refs_stmt =
                tx.prepare("SELECT refs FROM envelopes WHERE mailbox_id = ?1 AND uid = ?2")?;
            let mut existing_stmt = tx.prepare(
                "SELECT subject, sender, sender_address, to_addrs, cc_addrs
                 FROM envelopes WHERE mailbox_id = ?1 AND uid = ?2",
            )?;
            for envelope in envelopes {
                // The directory only learns from NEW messages: a
                // re-sync (CONDSTORE flags, re-poll) does not inflate a
                // contact's frequency. The same read tells whether the
                // INDEXED fields changed: if not, the index does not
                // move (PLAN-AUDIT-V2 E2 — before, every re-read
                // envelope made its body get reread and re-tokenized
                // under the write lock).
                let to_field = join_addrs(&envelope.to_addrs);
                let cc_field = join_addrs(&envelope.cc_addrs);
                let existing: Option<IndexedFields> = existing_stmt
                    .query_row(params![mailbox_id, envelope.uid], |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                        ))
                    })
                    .optional()?;
                let is_new = existing.is_none();
                let needs_reindex = needs_reindex(
                    existing.as_ref(),
                    envelope.subject.as_deref(),
                    envelope.sender.as_deref(),
                    envelope.sender_address.as_deref(),
                    to_field.as_deref(),
                    cc_field.as_deref(),
                );
                stmt.execute(params![
                    mailbox_id,
                    envelope.uid,
                    envelope.subject,
                    envelope.sender,
                    envelope.sender_address,
                    envelope.message_id,
                    envelope.in_reply_to,
                    envelope.date.map(|d| d.timestamp()),
                    envelope.seen,
                    envelope.flagged,
                    join_addrs(&envelope.to_addrs),
                    join_addrs(&envelope.cc_addrs),
                    envelope.reply_to,
                ])?;

                // The Screener's arrival decision (E2) — cached probes
                // by key, all cached, 7 µs/message measured (S2-bis): an
                // unknown sender (no routing row, no mail before the
                // epoch, never ourselves) enters the waiting list on
                // their first arrival after the epoch; OLD mail arriving
                // after the fact (sync reordering) proves them known and
                // UNDOES a wrongly set waiting status. A message WITHOUT
                // a date NEVER proves known (review E2): spam with no
                // Date header would otherwise bypass the very gate — it
                // is treated as arriving today.
                // The No rule (E3): a message that ARRIVES from a sender
                // screened out with a rule, AFTER the verdict ("their
                // next messages" — a backfill of history never archives
                // nor discards; no date = treated as arriving today; a
                // stated limit: a falsified Date header earlier than the
                // verdict dodges the rule — the message stays hidden
                // from organized mode by the flag, it is the server it
                // does not reach). The action is logged HERE, within the
                // batch's transaction (review E3 — never a crash window
                // between the commit and the intent); `trash` →
                // Delete, the server's trash, NEVER a permanent deletion
                // (D4). The anti-duplicate guard covers re-delivery (the
                // local removal makes `max_uid` go backward, a failed
                // replay re-presented the message — a second identical
                // action would jam the queue).
                if is_new
                    && arrival
                    && active_rules
                    && let Some(address) = images_address(envelope.sender_address.clone())
                    && let Some((rule, verdict)) = tx
                        .prepare_cached(
                            "SELECT regle, epoch FROM routage_expediteurs
                              WHERE address = ?1 AND destination = 'ecarte'
                                AND regle IS NOT NULL",
                        )?
                        .query_row(params![address], |row| {
                            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
                        })
                        .optional()?
                    && arrived_after_verdict(envelope.date.map(|d| d.timestamp()), verdict)
                {
                    let action = no_rule_action(&rule, junk_folder.as_deref());
                    if let Some(action) = action {
                        let already_queued = tx
                            .prepare_cached(
                                "SELECT 1 FROM pending_actions
                                  WHERE mailbox_id = ?1 AND uid = ?2 AND refusee = 0",
                            )?
                            .exists(params![mailbox_id, envelope.uid])?;
                        if !already_queued {
                            tx.prepare_cached(
                                "INSERT INTO pending_actions (mailbox_id, uid, kind)
                                 VALUES (?1, ?2, ?3)",
                            )?
                            .execute(params![
                                mailbox_id,
                                envelope.uid,
                                action.to_kind()
                            ])?;
                        }
                        no_removals.push(envelope.uid);
                    }
                }

                if is_new
                    && let Some(epoch) = screener_epoch
                    && let Some(address) = images_address(envelope.sender_address.clone())
                {
                    let date = envelope.date.map(|d| d.timestamp());
                    if let Some(date) = date
                        && date <= epoch
                    {
                        if tx
                            .prepare_cached("DELETE FROM portier_attente WHERE address = ?1")?
                            .execute(params![address])?
                            > 0
                        {
                            undone_waiting.insert(address);
                        }
                    } else if arrival {
                        let known = tx
                            .prepare_cached("SELECT 1 FROM portier_attente WHERE address = ?1")?
                            .exists(params![address])?
                            || tx
                                .prepare_cached(
                                    "SELECT 1 FROM routage_expediteurs WHERE address = ?1",
                                )?
                                .exists(params![address])?
                            || account_address(&tx, &address)?
                            || known_before_epoch(&tx, &address, epoch)?;
                        if !known {
                            tx.prepare_cached("INSERT INTO portier_attente (address) VALUES (?1)")?
                                .execute(params![address])?;
                        }
                    }
                }

                if is_new {
                    let date = envelope.date.map(|d| d.timestamp()).unwrap_or(0);
                    if note_senders && let Some(address) = envelope.sender_address.as_deref() {
                        crate::contacts::note(&tx, address, envelope.sender.as_deref(), date)?;
                    }
                    if note_recipients {
                        for address in envelope.to_addrs.iter().chain(envelope.cc_addrs.iter()) {
                            crate::contacts::note(&tx, address, None, date)?;
                        }
                    }
                }

                // Already-acquired `References` count towards
                // reattaching: a re-sync must not un-group a thread the
                // header pass had reattached.
                let references: Option<String> = refs_stmt
                    .query_row(params![mailbox_id, envelope.uid], |row| row.get(0))
                    .optional()?
                    .flatten();
                if threaded {
                    let thread = thread::attach(
                        &tx,
                        account_id,
                        envelope.message_id.as_deref(),
                        envelope.in_reply_to.as_deref(),
                        references.as_deref(),
                        &addresses_from(envelope),
                    )?;
                    tx.execute(
                        "UPDATE envelopes SET thread_id = ?3 WHERE mailbox_id = ?1 AND uid = ?2",
                        params![mailbox_id, envelope.uid, thread],
                    )?;
                    touched.insert(thread);
                }

                if needs_reindex {
                    let html: Option<String> = body_stmt
                        .query_row(params![mailbox_id, envelope.uid], |row| row.get(0))
                        .optional()?;
                    search::index_message(
                        &tx,
                        mailbox_id,
                        envelope.uid,
                        search::Indexed {
                            subject: envelope.subject.as_deref(),
                            sender: envelope.sender.as_deref(),
                            sender_address: envelope.sender_address.as_deref(),
                            to_addrs: to_field.as_deref(),
                            cc_addrs: cc_field.as_deref(),
                            body_html: html.as_deref(),
                        },
                    )?;
                }
            }
            // Threads from PREVIOUS batches with an undone waiting
            // status: their retention flag dates from a state that has
            // since been contradicted — they enter the same recompute
            // pass.
            for address in &undone_waiting {
                touched.extend(threads_of(&tx, address)?);
            }
            // After the loop, and only once per thread: recomputing on
            // every message would do the work N times on a conversation
            // of N messages arriving in the same batch.
            for thread in &touched {
                thread::refresh(&tx, *thread)?;
            }
        }
        tx.commit()?;
        // Local removal of handled messages (E3) — WITHOUT an echo (not
        // a user gesture; the Screener's history already states the
        // rule). The action, itself, is ALREADY committed with the
        // batch: a failure here leaves the local copy, which server
        // reconciliation will carry away after the replay — never a
        // message that escapes its rule.
        if !no_removals.is_empty() {
            // E4: in ONE transaction (the `cleanup_verdict` pattern) —
            // an autocommit removal cost eight fsyncs per message.
            let tx = self.0.unchecked_transaction()?;
            let mut threads: BTreeSet<i64> = BTreeSet::new();
            for uid in no_removals {
                if let Some(thread) = purge_message(&tx, mailbox_id, uid)? {
                    threads.insert(thread);
                }
            }
            for thread in &threads {
                thread::refresh(&tx, *thread)?;
            }
            tx.commit()?;
        }
        Ok(())
    }

    /// Records the thread headers read into the full-headers block,
    /// and reattaches the thread if warranted.
    ///
    /// `references` is `""` when the message carries none: that is the
    /// mark of "already read, nothing to find there". Writing NULL
    /// would make it get asked again on every pass, indefinitely.
    ///
    /// Returns `true` if reattachment changed — the caller then knows
    /// the displayed list is stale.
    pub fn set_thread_headers(
        &mut self,
        mailbox_id: i64,
        uid: Uid,
        in_reply_to: Option<&str>,
        references: &str,
    ) -> Result<bool, Error> {
        let tx = self.0.transaction()?;
        let before: Option<i64> = tx
            .query_row(
                "SELECT thread_id FROM envelopes WHERE mailbox_id = ?1 AND uid = ?2",
                params![mailbox_id, uid],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
        let context: Option<(Option<String>, Option<String>, Vec<String>)> = tx
            .query_row(
                "SELECT message_id, in_reply_to, sender_address, to_addrs, cc_addrs
                 FROM envelopes WHERE mailbox_id = ?1 AND uid = ?2",
                params![mailbox_id, uid],
                |row| {
                    let mut addresses: Vec<String> = Vec::new();
                    addresses.extend(row.get::<_, Option<String>>(2)?);
                    addresses.extend(split_addrs(row.get(3)?));
                    addresses.extend(split_addrs(row.get(4)?));
                    Ok((row.get(0)?, row.get(1)?, addresses))
                },
            )
            .optional()?;
        let Some((message_id, known_parent, addresses)) = context else {
            // The message vanished between reading the headers and writing
            // them (archived, deleted): there is nothing left to attach.
            return Ok(false);
        };

        // `COALESCE`: the header block is authoritative when it says
        // something, but an `In-Reply-To` already given by the ENVELOPE
        // must not be erased by a read that finds none.
        tx.execute(
            "UPDATE envelopes SET refs = ?3, in_reply_to = COALESCE(?4, in_reply_to)
             WHERE mailbox_id = ?1 AND uid = ?2",
            params![mailbox_id, uid, references, in_reply_to],
        )?;
        let parent = in_reply_to.map(str::to_string).or(known_parent);
        // The ACCOUNT, not the mailbox (ADR 0009). Both are `i64`: the
        // compiler cannot tell one from the other, and getting this wrong
        // here would not break anything — it would simply attach the
        // messages to the wrong thread space, silently.
        let (account_id, threaded): (i64, bool) = tx.query_row(
            "SELECT account_id, threaded FROM mailboxes WHERE id = ?1",
            [mailbox_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        // Out of scope, the headers are still kept — they serve search, and
        // they will be useful again if the mailbox ever enters scope — but
        // they attach nothing (ADR 0010 §3).
        if !threaded {
            tx.commit()?;
            return Ok(false);
        }
        let thread = thread::attach(
            &tx,
            account_id,
            message_id.as_deref(),
            parent.as_deref(),
            Some(references),
            &addresses,
        )?;
        tx.execute(
            "UPDATE envelopes SET thread_id = ?3 WHERE mailbox_id = ?1 AND uid = ?2",
            params![mailbox_id, uid, thread],
        )?;
        thread::refresh(&tx, thread)?;
        if let Some(previous) = before.filter(|previous| *previous != thread) {
            thread::refresh(&tx, previous)?;
        }
        tx.commit()?;
        Ok(before != Some(thread))
    }

    /// Removes envelopes absent from the server; returns their count.
    /// The UIDs a mailbox already carries in the database — what a resumed
    /// initial sync does not ask for again (PLAN-AUDIT-V2 E5).
    pub fn known_uids(&self, mailbox_id: i64) -> Result<HashSet<Uid>, Error> {
        Ok(self
            .0
            .prepare_cached("SELECT uid FROM envelopes WHERE mailbox_id = ?1")?
            .query_map([mailbox_id], |row| row.get(0))?
            .collect::<Result<_, _>>()?)
    }

    pub fn remove_absent(
        &mut self,
        mailbox_id: i64,
        present: &HashSet<Uid>,
    ) -> Result<usize, Error> {
        let stale: Vec<Uid> = self
            .known_uids(mailbox_id)?
            .into_iter()
            .filter(|uid| !present.contains(uid))
            .collect();
        let tx = self.0.transaction()?;
        {
            // E4: THE list of per-message tables (`purge_message`) —
            // before, three tables out of seven — attachments, invitation,
            // image memory, set-aside and the Feed's "read" — were left
            // orphaned (no foreign key on `envelopes`). A message that
            // leaves the server also takes its pending actions with it: an
            // intention on a UID that no longer exists cannot be carried
            // out.
            let mut actions =
                tx.prepare("DELETE FROM pending_actions WHERE mailbox_id = ?1 AND uid = ?2")?;
            let mut touched: BTreeSet<i64> = BTreeSet::new();
            for uid in &stale {
                if let Some(thread) = purge_message(&tx, mailbox_id, *uid)? {
                    touched.insert(thread);
                }
                actions.execute(params![mailbox_id, uid])?;
            }
            // ONCE per touched thread, never per message.
            for thread in &touched {
                thread::refresh(&tx, *thread)?;
            }
        }
        tx.commit()?;
        Ok(stale.len())
    }

    /// Locally removes an envelope and its body (optimistic
    /// archiving/deletion); the server will follow via the action queue —
    /// pending actions are NOT touched, they are what carries the gesture.
    ///
    /// Atomic (E4): inside the caller's transaction if it has one
    /// (`gesture_with_echo`, `cleanup_verdict`, `upsert_envelopes`),
    /// otherwise inside its own — never eight writes in autocommit.
    pub fn remove_local(&self, mailbox_id: i64, uid: Uid) -> Result<(), Error> {
        if self.0.is_autocommit() {
            let tx = self.0.unchecked_transaction()?;
            if let Some(thread) = purge_message(&tx, mailbox_id, uid)? {
                thread::refresh(&tx, thread)?;
            }
            tx.commit()?;
            Ok(())
        } else {
            if let Some(thread) = purge_message(&self.0, mailbox_id, uid)? {
                thread::refresh(&self.0, thread)?;
            }
            Ok(())
        }
    }

    /// Locally applies a read/unread change (UI optimism).
    /// Returns `false` if the envelope was already in this state.
    pub fn set_seen_local(&self, mailbox_id: i64, uid: Uid, seen: bool) -> Result<bool, Error> {
        let changed = self.0.execute(
            "UPDATE envelopes SET seen = ?3
             WHERE mailbox_id = ?1 AND uid = ?2 AND seen != ?3",
            params![mailbox_id, uid, seen],
        )?;
        if changed > 0 {
            // The thread's unread counter just moved. Forgetting it would
            // leave a conversation in bold even though its last unread
            // message was just read.
            if let Some(thread) = thread::thread_of(&self.0, mailbox_id, uid)? {
                thread::refresh(&self.0, thread)?;
            }
        }
        Ok(changed > 0)
    }

    /// Locally applies the star (UI optimism).
    /// Returns `false` if the envelope was already in this state.
    pub fn set_flagged_local(
        &self,
        mailbox_id: i64,
        uid: Uid,
        flagged: bool,
    ) -> Result<bool, Error> {
        let changed = self.0.execute(
            "UPDATE envelopes SET flagged = ?3
             WHERE mailbox_id = ?1 AND uid = ?2 AND flagged != ?3",
            params![mailbox_id, uid, flagged],
        )?;
        Ok(changed > 0)
    }

    /// Logs an intention to be replayed to the server. A new gesture on a
    /// message REPLACES its refused ones (PLAN-AUDIT-V1 review): without
    /// this, a quarantined action would stay there forever and the notice
    /// slot's line could only grow.
    pub fn enqueue_action(&self, mailbox_id: i64, uid: Uid, action: Action) -> Result<(), Error> {
        forget_refused(&self.0, mailbox_id, uid)?;
        self.0.execute(
            "INSERT INTO pending_actions (mailbox_id, uid, kind) VALUES (?1, ?2, ?3)",
            params![mailbox_id, uid, action.to_kind()],
        )?;
        Ok(())
    }

    /// The ACTIVE action queue, in emission order — the refused ones
    /// (quarantine, E3) are no longer in it. A line whose `kind` is
    /// unreadable (future version, corruption) is quarantined with its
    /// reason, never fatal: before E3 it made the WHOLE queue fail.
    pub fn pending_actions(&self, mailbox_id: i64) -> Result<Vec<PendingAction>, Error> {
        let mut stmt = self.0.prepare(
            "SELECT id, uid, kind FROM pending_actions
              WHERE mailbox_id = ?1 AND refusee = 0 ORDER BY id",
        )?;
        let rows = stmt
            .query_map([mailbox_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get(1)?, row.get::<_, String>(2)?))
            })?
            .collect::<Result<Vec<(i64, Uid, String)>, _>>()?;
        let mut queue = Vec::with_capacity(rows.len());
        for (id, uid, kind) in rows {
            match Action::parse(&kind) {
                Some(action) => queue.push(PendingAction { id, uid, action }),
                None => self.refuse_action(id, &format!("unreadable action: {kind}"))?,
            }
        }
        Ok(queue)
    }

    /// Consecutive transient failures beyond which an action enters
    /// quarantine (D2: five cycles).
    pub const QUARANTINE_THRESHOLD: i64 = 5;

    /// One more TRANSIENT failure on this action. Returns `true` when the
    /// threshold is reached: the action just entered quarantine.
    pub fn note_action_failure(&self, action_id: i64, error: &str) -> Result<bool, Error> {
        let refused: i64 = self.0.query_row(
            "UPDATE pending_actions
                SET attempts = attempts + 1,
                    last_error = ?2,
                    refusee = CASE WHEN attempts + 1 >= ?3 THEN 1 ELSE refusee END
              WHERE id = ?1
              RETURNING refusee",
            params![action_id, error, Self::QUARANTINE_THRESHOLD],
            |row| row.get(0),
        )?;
        Ok(refused != 0)
    }

    /// DEFINITIVE refusal: immediate quarantine, with the reason.
    pub fn refuse_action(&self, action_id: i64, error: &str) -> Result<(), Error> {
        self.0.execute(
            "UPDATE pending_actions SET refusee = 1, last_error = ?2 WHERE id = ?1",
            params![action_id, error],
        )?;
        Ok(())
    }

    /// How many actions are in quarantine, across all accounts —
    /// the notice slot's line (D2).
    pub fn refused_actions(&self) -> Result<u64, Error> {
        let n: i64 = self.0.query_row(
            "SELECT COUNT(*) FROM pending_actions WHERE refusee = 1",
            [],
            |row| row.get(0),
        )?;
        Ok(u64::try_from(n).unwrap_or(0))
    }

    pub fn remove_action(&self, action_id: i64) -> Result<(), Error> {
        self.0
            .execute("DELETE FROM pending_actions WHERE id = ?1", [action_id])?;
        Ok(())
    }

    /// Raw HTML body (pre-sanitization) of a message, if it is cached.
    pub fn body(&self, account_id: i64, mailbox: &str, uid: Uid) -> Result<Option<String>, Error> {
        let body = self
            .0
            .query_row(
                "SELECT b.html FROM bodies b JOIN mailboxes m ON m.id = b.mailbox_id
                 WHERE m.account_id = ?1 AND m.name = ?2 AND b.uid = ?3",
                params![account_id, mailbox, uid],
                |row| row.get(0),
            )
            .optional()?;
        Ok(body)
    }

    /// Records a body, its search index and the description of its
    /// attachments — **in a single transaction**.
    ///
    /// The three are read from the same bytes and only make sense
    /// together: a body without its index would fall out of search, a
    /// body without its attachments would leave them invisible until the
    /// next re-download. A crash between two writes must never be able to
    /// produce that state.
    pub fn save_body(
        &self,
        mailbox_id: i64,
        uid: Uid,
        html: &str,
        attachments: &[Attachment],
    ) -> Result<(), Error> {
        self.save_body_full(mailbox_id, uid, html, attachments, None)
    }

    /// [`Store::save_body`], plus the invitation row when a `text/calendar`
    /// part accompanied the body (PLAN-INVITATIONS). Same transaction as
    /// the body, full replacement like the attachments: a re-scan without
    /// a calendar part erases the row.
    pub fn save_body_full(
        &self,
        mailbox_id: i64,
        uid: Uid,
        html: &str,
        attachments: &[Attachment],
        invitation: Option<&InvitationRow>,
    ) -> Result<(), Error> {
        // Same rule as the preview backfill: HTML parsing is paid for
        // BEFORE opening the transaction — never any CPU inside the
        // write-lock window.
        let preview = crate::body::extract_preview(html);
        let tx = self.0.unchecked_transaction()?;
        tx.execute(
            "INSERT OR REPLACE INTO bodies (mailbox_id, uid, html, scanned, preview)
             VALUES (?1, ?2, ?3, 1, ?4)",
            params![mailbox_id, uid, html, preview],
        )?;
        // Full replacement: a re-downloaded message whose attachment
        // disappeared must not keep the old phantom row.
        tx.execute(
            "DELETE FROM attachments WHERE mailbox_id = ?1 AND uid = ?2",
            params![mailbox_id, uid],
        )?;
        for attachment in attachments {
            tx.execute(
                "INSERT INTO attachments (mailbox_id, uid, idx, name, mime, size)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    mailbox_id,
                    uid,
                    attachment.index as i64,
                    attachment.name,
                    attachment.mime,
                    attachment.size as i64
                ],
            )?;
        }
        match invitation {
            Some(row) => write_invitation(&tx, mailbox_id, uid, row)?,
            // Same rule as the attachments: a re-scan WITHOUT a calendar
            // part does not keep a phantom card.
            None => {
                tx.execute(
                    "DELETE FROM invitations WHERE mailbox_id = ?1 AND uid = ?2",
                    params![mailbox_id, uid],
                )?;
            }
        }
        if let Some((subject, sender, sender_address, to_field, cc_field)) = tx
            .query_row(
                "SELECT subject, sender, sender_address, to_addrs, cc_addrs
                 FROM envelopes WHERE mailbox_id = ?1 AND uid = ?2",
                params![mailbox_id, uid],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                    ))
                },
            )
            .optional()?
        {
            search::index_message(
                &tx,
                mailbox_id,
                uid,
                search::Indexed {
                    subject: subject.as_deref(),
                    sender: sender.as_deref(),
                    sender_address: sender_address.as_deref(),
                    to_addrs: to_field.as_deref(),
                    cc_addrs: cc_field.as_deref(),
                    body_html: Some(html),
                },
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// A message's invitation, with our local reply — LOCAL read, never
    /// the network. `None`: this message does not carry one (or its MIME
    /// has not been inspected yet).
    pub fn invitation(
        &self,
        account_id: i64,
        mailbox: &str,
        uid: Uid,
    ) -> Result<Option<StoredInvitation>, Error> {
        let stored = self
            .0
            .query_row(
                // Columns read BY NAME: a column added in the middle of
                // the SELECT never shifts the fields — nineteen Options
                // of the same type, a positional shift would be silent
                // and would send the iTIP reply to the wrong address
                // (review).
                "SELECT i.* FROM invitations i JOIN mailboxes m ON m.id = i.mailbox_id
                 WHERE m.account_id = ?1 AND m.name = ?2 AND i.uid = ?3",
                params![account_id, mailbox, uid],
                |row| {
                    Ok(StoredInvitation {
                        row: InvitationRow {
                            method: row.get("methode")?,
                            event_uid: row.get("event_uid")?,
                            sequence: row.get("sequence")?,
                            title: row.get("titre")?,
                            location: row.get("lieu")?,
                            organizer_address: row.get("organisateur_adresse")?,
                            organizer_name: row.get("organisateur_nom")?,
                            start_epoch: row.get("debut_epoch")?,
                            end_epoch: row.get("fin_epoch")?,
                            start_text: row.get("debut_texte")?,
                            end_text: row.get("fin_texte")?,
                            all_day: row.get("journee_entiere")?,
                            recurrent: row.get("recurrent")?,
                            partstat: row.get("partstat")?,
                            attendee_address: row.get("repondant_adresse")?,
                            attendee_name: row.get("repondant_nom")?,
                            attendee_status: row.get("repondant_statut")?,
                            cancelled: row.get("annule")?,
                        },
                        reply: row.get("reponse")?,
                        reply_epoch: row.get("reponse_epoch")?,
                    })
                },
            )
            .optional()?;
        Ok(stored)
    }

    /// An account's address by its id — the read key for invitations (our
    /// PARTSTAT is looked up by address). EMPTY address = a half
    /// provisioned account: reads as `None`, like [`Store::accounts`]
    /// which filters these rows out.
    pub fn account_email(&self, account_id: i64) -> Result<Option<String>, Error> {
        let email: Option<String> = self
            .0
            .query_row(
                "SELECT email FROM accounts WHERE id = ?1 AND email != ''",
                params![account_id],
                |row| row.get(0),
            )
            .optional()?;
        Ok(email)
    }

    /// Replaces an account's list of known folders.
    ///
    /// Full, transactional replacement: a folder deleted server-side must
    /// not stay offered as a destination — the move would fail on replay,
    /// long after the click.
    pub fn replace_folders(&self, account_id: i64, folders: &[Folder]) -> Result<(), Error> {
        let tx = self.0.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM folders WHERE account_id = ?1",
            params![account_id],
        )?;
        for folder in folders {
            tx.execute(
                "INSERT OR REPLACE INTO folders (account_id, wire, display, selectable, special_use)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    account_id,
                    folder.wire,
                    folder.display,
                    folder.selectable,
                    folder.special_use.map(SpecialUse::code)
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// An account's known folders — LOCAL read, never the network.
    pub fn folders(&self, account_id: i64) -> Result<Vec<Folder>, Error> {
        let mut statement = self.0.prepare(
            "SELECT wire, display, selectable, special_use FROM folders
             WHERE account_id = ?1 ORDER BY display",
        )?;
        let rows = statement.query_map(params![account_id], |row| {
            Ok(Folder {
                wire: row.get(0)?,
                display: row.get(1)?,
                selectable: row.get(2)?,
                special_use: row
                    .get::<_, Option<String>>(3)?
                    .as_deref()
                    .and_then(SpecialUse::from_code),
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Backfills the preview of bodies written BEFORE the `preview`
    /// column — in bounded batches. Called by the shell as it polls: never
    /// on the opening path (startup budget < 1 s, the lesson of the
    /// orphan hunt), never on scroll. Returns the number of stragglers
    /// remaining — zero when the pass is complete.
    pub fn preview_catchup(&self, limit: usize) -> Result<u64, Error> {
        // In sub-batches of 100 bodies (PLAN-AUDIT-V2 E2): the shell asks
        // for 500 at once, and 500 whole HTML bodies in RAM weighed
        // ~28 MB at the 56 KB average — five times less per sub-batch,
        // same contract.
        const SUB_BATCH: usize = 100;
        let mut remaining = limit;
        while remaining > 0 {
            let taken = self.backfill_previews_batch(remaining.min(SUB_BATCH))?;
            if taken == 0 {
                break;
            }
            remaining -= taken;
        }
        let remaining_count: i64 = self.0.query_row(
            "SELECT COUNT(*) FROM bodies WHERE preview IS NULL",
            [],
            |row| row.get(0),
        )?;
        Ok(remaining_count as u64)
    }

    /// One sub-batch of [`Store::preview_catchup`]; returns the number of
    /// bodies processed (zero = no more stragglers).
    fn backfill_previews_batch(&self, limit: usize) -> Result<usize, Error> {
        let batch: Vec<(i64, Uid, String)> = self
            .0
            .prepare("SELECT mailbox_id, uid, html FROM bodies WHERE preview IS NULL LIMIT ?1")?
            .query_map(params![limit as i64], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<Result<_, _>>()?;
        if !batch.is_empty() {
            // CPU OUTSIDE the lock window (field finding 2026-08-15):
            // extracting the previews INSIDE the transaction held the
            // write lock for the whole parsing of the batch (2 000 HTML
            // bodies at the shell's poll) — a concurrent UI write
            // (`delete_draft` of an emptied draft) would time out its
            // busy_timeout and fail with BUSY. We parse first, the
            // transaction now only writes — short by construction.
            let previews: Vec<(i64, Uid, String)> = batch
                .iter()
                .map(|(mailbox_id, uid, html)| {
                    (*mailbox_id, *uid, crate::body::extract_preview(html))
                })
                .collect();
            let tx = self.0.unchecked_transaction()?;
            for (mailbox_id, uid, preview) in &previews {
                tx.execute(
                    "UPDATE bodies SET preview = ?3 WHERE mailbox_id = ?1 AND uid = ?2",
                    params![mailbox_id, uid, preview],
                )?;
            }
            tx.commit()?;
        }
        Ok(batch.len())
    }

    /// A message's known attachments, in MIME order.
    ///
    /// Empty as long as the body has not been fetched: it is the same
    /// condition as text search, and the backfill lifts it for the whole
    /// recency horizon.
    pub fn attachments(
        &self,
        account_id: i64,
        mailbox: &str,
        uid: Uid,
    ) -> Result<Vec<Attachment>, Error> {
        let mut statement = self.0.prepare(
            "SELECT a.idx, a.name, a.mime, a.size
             FROM attachments a
             JOIN mailboxes m ON m.id = a.mailbox_id
             WHERE m.account_id = ?1 AND m.name = ?2 AND a.uid = ?3
             ORDER BY a.idx",
        )?;
        let rows = statement.query_map(params![account_id, mailbox, uid], |row| {
            Ok(Attachment {
                index: row.get::<_, i64>(0)? as usize,
                name: row.get(1)?,
                mime: row.get(2)?,
                size: row.get::<_, i64>(3)? as u64,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// UNREAD messages that arrived after `uid_gt`, oldest to newest — the
    /// material for notifications.
    ///
    /// The criterion is the UID, not the date: it is the arrival order the
    /// server guarantees, and it is what distinguishes "new" from "old but
    /// recently dated". Messages already read elsewhere are excluded:
    /// notifying a message the user just read on their phone is pure
    /// noise.
    pub fn new_unread_after(
        &self,
        account_id: i64,
        mailbox: &str,
        uid_gt: Uid,
        limit: usize,
    ) -> Result<Vec<Envelope>, Error> {
        let mut statement = self.0.prepare(
            "SELECT e.uid, e.subject, e.sender, e.sender_address, e.message_id,
                    e.date_epoch, e.seen, e.flagged, e.in_reply_to, e.to_addrs, e.cc_addrs
             FROM envelopes e
             JOIN mailboxes m ON m.id = e.mailbox_id
             WHERE m.account_id = ?1 AND m.name = ?2
               AND e.uid > ?3 AND e.seen = 0
             ORDER BY e.uid
             LIMIT ?4",
        )?;
        let rows = statement.query_map(
            params![account_id, mailbox, uid_gt, limit as i64],
            row_to_envelope,
        )?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// RECENT messages whose body is still missing, newest to oldest — the
    /// backfill's work ([ADR 0007](../../../docs/adr/0007-body-backfill.md)).
    ///
    /// `since_epoch` bounds the cost: it is the recency horizon.
    /// Descending order makes recovery after an interruption natural — we
    /// simply ask for the list again, the bodies already written are no
    /// longer in it.
    ///
    /// A message without a date is ALWAYS eligible — revised by ADR 0010.
    /// The old rule excluded it as "not placeable in the horizon"; since
    /// production no longer bounds anything ([`crate::NO_HORIZON`]),
    /// excluding it would be a silent hole: a message whose date cannot be
    /// read would never get a body, hence never be searchable, with
    /// nothing to signal it. It goes last (NULLs close a DESC sort): the
    /// doubt only costs it its rank.
    pub fn bodies_to_backfill(
        &self,
        account_id: i64,
        mailbox: &str,
        since_epoch: i64,
        limit: usize,
    ) -> Result<Vec<Uid>, Error> {
        let mut stmt = self.0.prepare(&bodies_to_backfill_sql())?;
        let uids = stmt
            .query_map(
                params![account_id, mailbox, since_epoch, limit as i64],
                |row| row.get(0),
            )?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(uids)
    }

    /// Messages whose thread headers have not yet been read, newest to
    /// oldest.
    ///
    /// `refs IS NULL` = never read. A message without a `References`
    /// receives `""` and leaves this list for good.
    pub fn thread_headers_to_backfill(
        &self,
        account_id: i64,
        mailbox: &str,
        since_epoch: i64,
        limit: usize,
    ) -> Result<Vec<Uid>, Error> {
        let mut stmt = self.0.prepare(
            "SELECT e.uid
             FROM envelopes e
             JOIN mailboxes m ON m.id = e.mailbox_id
             WHERE m.account_id = ?1 AND m.name = ?2
               AND (e.date_epoch IS NULL OR e.date_epoch >= ?3)
               AND e.refs IS NULL
             ORDER BY e.date_epoch DESC, e.uid DESC
             LIMIT ?4",
        )?;
        let uids = stmt
            .query_map(
                params![account_id, mailbox, since_epoch, limit as i64],
                |row| row.get(0),
            )?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(uids)
    }

    /// Messages of a mailbox whose recipients have not yet been read,
    /// newest to oldest (R4, sent-mail backfill D2).
    ///
    /// `to_addrs IS NULL` = never read. A message without a To receives
    /// `""` (empty string, NOT NULL) and leaves this list for good — the
    /// same sentinel as `refs` for thread headers, without which the pump
    /// would keep asking for it forever (convergence lesson, HANDOVER
    /// §9).
    pub fn recipients_to_backfill(
        &self,
        account_id: i64,
        mailbox: &str,
        limit: usize,
    ) -> Result<Vec<Uid>, Error> {
        let mut stmt = self.0.prepare(
            "SELECT e.uid
             FROM envelopes e
             JOIN mailboxes m ON m.id = e.mailbox_id
             WHERE m.account_id = ?1 AND m.name = ?2
               AND e.to_addrs IS NULL
             ORDER BY e.date_epoch DESC, e.uid DESC
             LIMIT ?3",
        )?;
        let uids = stmt
            .query_map(params![account_id, mailbox, limit as i64], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(uids)
    }

    /// How many sent messages are still waiting for their recipients.
    pub fn recipients_pending_count(&self, account_id: i64, mailbox: &str) -> Result<u64, Error> {
        let count: i64 = self.0.query_row(
            "SELECT COUNT(*)
             FROM envelopes e
             JOIN mailboxes m ON m.id = e.mailbox_id
             WHERE m.account_id = ?1 AND m.name = ?2 AND e.to_addrs IS NULL",
            params![account_id, mailbox],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }

    /// Writes the To/Cc recipients of an already-stored message (sent-mail
    /// backfill). Writes `""` — never NULL — when the list is empty: it is
    /// the "read, none" mark that makes the pump converge. Touches NO
    /// other column (neither thread nor refs).
    pub fn set_recipients(
        &self,
        mailbox_id: i64,
        uid: Uid,
        to: &[String],
        cc: &[String],
    ) -> Result<(), Error> {
        // E4: recipients and the address book agree, or nothing does.
        let tx = self.0.unchecked_transaction()?;
        self.0.execute(
            "UPDATE envelopes SET to_addrs = ?3, cc_addrs = ?4
             WHERE mailbox_id = ?1 AND uid = ?2",
            params![mailbox_id, uid, to.join("\n"), cc.join("\n")],
        )?;
        // PLAN-RETOURS-5 (D4, review): these backfilled recipients are
        // those of OUR sends from before To/Cc were stored — without this,
        // they would never enter the address book (the opening backfill
        // already ran before them). The extra cost (two reads) is
        // invisible behind the server round trip that precedes it.
        let (_, record_recipients) = self.directory_role(mailbox_id)?;
        if record_recipients && (!to.is_empty() || !cc.is_empty()) {
            let date: Option<i64> = self
                .0
                .query_row(
                    "SELECT date_epoch FROM envelopes WHERE mailbox_id = ?1 AND uid = ?2",
                    params![mailbox_id, uid],
                    |row| row.get(0),
                )
                .optional()?
                .flatten();
            for address in to.iter().chain(cc.iter()) {
                crate::contacts::note(self.conn(), address, None, date.unwrap_or(0))?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// How many messages are still waiting for their thread headers.
    pub fn thread_headers_pending_count(
        &self,
        account_id: i64,
        mailbox: &str,
        since_epoch: i64,
    ) -> Result<u64, Error> {
        let count: i64 = self.0.query_row(
            "SELECT COUNT(*)
             FROM envelopes e
             JOIN mailboxes m ON m.id = e.mailbox_id
             WHERE m.account_id = ?1 AND m.name = ?2
               AND (e.date_epoch IS NULL OR e.date_epoch >= ?3)
               AND e.refs IS NULL",
            params![account_id, mailbox, since_epoch],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }

    /// How many messages are still waiting for their body within the
    /// horizon — enough to show an honest progress figure.
    pub fn bodies_pending_count(
        &self,
        account_id: i64,
        mailbox: &str,
        since_epoch: i64,
    ) -> Result<u64, Error> {
        let count: i64 = self.0.query_row(
            &bodies_pending_count_sql(),
            params![account_id, mailbox, since_epoch],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }

    /// The corpus IN SCOPE: every message that CAN carry a body (same
    /// filter as [`Self::bodies_pending_count`], without the missing-body
    /// clause). It is the denominator of the backfill percentage (R1,
    /// PLAN-RETOURS-3) — `total - pending` gives the bodies present.
    /// Lighter than counting the missing ones: no `NOT EXISTS`
    /// subquery.
    pub fn bodies_total_count(
        &self,
        account_id: i64,
        mailbox: &str,
        since_epoch: i64,
    ) -> Result<u64, Error> {
        let count: i64 = self.0.query_row(
            "SELECT COUNT(*)
             FROM envelopes e
             JOIN mailboxes m ON m.id = e.mailbox_id
             WHERE m.account_id = ?1 AND m.name = ?2
               AND (e.date_epoch IS NULL OR e.date_epoch >= ?3)",
            params![account_id, mailbox, since_epoch],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }

    /// A page of envelopes of ONE account, most recent first.
    pub fn recent(
        &self,
        account_id: i64,
        mailbox: &str,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<Envelope>, Error> {
        let mut stmt = self.0.prepare(
            "SELECT e.uid, e.subject, e.sender, e.sender_address, e.message_id,
                    e.date_epoch, e.seen, e.flagged, e.in_reply_to, e.to_addrs, e.cc_addrs
             FROM envelopes e JOIN mailboxes m ON m.id = e.mailbox_id
             WHERE m.account_id = ?1 AND m.name = ?2
             ORDER BY e.date_epoch DESC, e.uid DESC
             LIMIT ?3 OFFSET ?4",
        )?;
        let rows = stmt
            .query_map(
                params![account_id, mailbox, limit as i64, offset as i64],
                row_to_envelope,
            )?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// The unified mailbox: the same mailbox (INBOX) of ALL accounts,
    /// merged by date — the flagship product of multi-account. Each row
    /// carries its account: a UID alone no longer identifies a message.
    pub fn unified_recent(&self, offset: usize, limit: usize) -> Result<Vec<UnifiedRow>, Error> {
        // One row per CONVERSATION, represented by its last message.
        //
        // The query starts from `threads`, not from `envelopes`: a
        // `GROUP BY thread_id` with a `MAX(date)` would force SQLite to
        // scan and then sort the 200,000 envelopes on every scroll page.
        // Here the `idx_threads_date_globale` index carries both the sort
        // and the pagination — the cost of a page no longer depends on
        // the mailbox's size. It is the materialized aggregate that pays
        // for that, and it is maintained inside the write transaction.
        //
        // Pagination lives in a SUBQUERY on `threads` alone: see
        // `unified_page_sql`.
        let mut stmt = self.0.prepare(&unified_page_sql(false, false, false))?;
        let rows = stmt
            .query_map(params![limit as i64, offset as i64], row_to_threaded)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Total of the unified mailbox — in CONVERSATIONS, since that is
    /// what the list displays. Counting messages would make it scroll
    /// into the void. PINNED threads do not count in it (R4, D5) — the
    /// page excludes them, the total MUST describe the same set as it
    /// does (review 2026-08-21: a mismatched page/total pair would
    /// manufacture phantom rows).
    //
    // (`unified_page_sql`, below, carries the page's query.)
    pub fn unified_count(&self) -> Result<u64, Error> {
        let count: i64 = self.0.query_row(
            &format!(
                "SELECT COUNT(*) FROM threads
                  WHERE inbox_size > 0 AND id NOT IN ({PINNED_THREADS})"
            ),
            [],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }

    /// R4 (PLAN-RETOURS-7): pins or unpins the CONVERSATION of the given
    /// message — returns the new state. Setting the pin records the
    /// envelope key of the gesture; removing it frees the WHOLE thread
    /// (every key that leads to it), without which a pin set yesterday
    /// from another head of the thread would stay stuck. The thread is
    /// resolved ONCE — the state and the write look at the same one
    /// (review 2026-08-21: two resolutions could diverge if a sync slid
    /// in between them).
    pub fn toggle_pin(&self, mailbox_id: i64, uid: Uid, epoch: i64) -> Result<bool, Error> {
        let thread = thread::thread_of(&self.0, mailbox_id, uid)?;
        if self.thread_pin_state(thread, mailbox_id, uid)? {
            match thread {
                Some(thread) => self.0.execute(
                    "DELETE FROM pins WHERE (mailbox_id, uid) IN
                       (SELECT mailbox_id, uid FROM envelopes WHERE thread_id = ?1)",
                    params![thread],
                )?,
                None => self.0.execute(
                    "DELETE FROM pins WHERE mailbox_id = ?1 AND uid = ?2",
                    params![mailbox_id, uid],
                )?,
            };
            Ok(false)
        } else {
            self.0.execute(
                "INSERT OR REPLACE INTO pins (mailbox_id, uid, epoch) VALUES (?1, ?2, ?3)",
                params![mailbox_id, uid, epoch],
            )?;
            Ok(true)
        }
    }

    /// Is the message's conversation pinned? The state is read by the
    /// THREAD: a pin set on any message of the thread holds for its
    /// current head — the thread bar tells the truth even when a reply
    /// has moved the head since the gesture.
    pub fn pin_state(&self, mailbox_id: i64, uid: Uid) -> Result<bool, Error> {
        let thread = thread::thread_of(&self.0, mailbox_id, uid)?;
        self.thread_pin_state(thread, mailbox_id, uid)
    }

    fn thread_pin_state(
        &self,
        thread: Option<i64>,
        mailbox_id: i64,
        uid: Uid,
    ) -> Result<bool, Error> {
        let pinned = match thread {
            Some(thread) => self
                .0
                .prepare(
                    "SELECT 1 FROM pins p JOIN envelopes e
                       ON e.mailbox_id = p.mailbox_id AND e.uid = p.uid
                     WHERE e.thread_id = ?1",
                )?
                .exists(params![thread])?,
            None => self
                .0
                .prepare("SELECT 1 FROM pins WHERE mailbox_id = ?1 AND uid = ?2")?
                .exists(params![mailbox_id, uid])?,
        };
        Ok(pinned)
    }

    /// E5 — Set aside: the SAME contract as pin (the `toggle_pin`
    /// pattern, thread resolved ONCE) — set on a message, the state
    /// applies to the whole thread; “Done” from any head releases
    /// everything. Returns the state AFTER the gesture.
    pub fn toggle_set_aside(&self, mailbox_id: i64, uid: Uid, epoch: i64) -> Result<bool, Error> {
        let thread = thread::thread_of(&self.0, mailbox_id, uid)?;
        if self.thread_set_aside(thread, mailbox_id, uid)? {
            match thread {
                Some(thread) => self.0.execute(
                    "DELETE FROM mis_de_cote WHERE (mailbox_id, uid) IN
                       (SELECT mailbox_id, uid FROM envelopes WHERE thread_id = ?1)",
                    params![thread],
                )?,
                None => self.0.execute(
                    "DELETE FROM mis_de_cote WHERE mailbox_id = ?1 AND uid = ?2",
                    params![mailbox_id, uid],
                )?,
            };
            Ok(false)
        } else {
            self.0.execute(
                "INSERT OR REPLACE INTO mis_de_cote (mailbox_id, uid, epoch) VALUES (?1, ?2, ?3)",
                params![mailbox_id, uid, epoch],
            )?;
            Ok(true)
        }
    }

    /// Is this message's thread set aside? — the state is by THREAD,
    /// new head included (same rule as `pin_state`).
    pub fn set_aside_state(&self, mailbox_id: i64, uid: Uid) -> Result<bool, Error> {
        let thread = thread::thread_of(&self.0, mailbox_id, uid)?;
        self.thread_set_aside(thread, mailbox_id, uid)
    }

    fn thread_set_aside(
        &self,
        thread: Option<i64>,
        mailbox_id: i64,
        uid: Uid,
    ) -> Result<bool, Error> {
        let aside = match thread {
            Some(thread) => self
                .0
                .prepare(
                    "SELECT 1 FROM mis_de_cote c JOIN envelopes e
                       ON e.mailbox_id = c.mailbox_id AND e.uid = c.uid
                     WHERE e.thread_id = ?1",
                )?
                .exists(params![thread])?,
            None => self
                .0
                .prepare("SELECT 1 FROM mis_de_cote WHERE mailbox_id = ?1 AND uid = ?2")?
                .exists(params![mailbox_id, uid])?,
        };
        Ok(aside)
    }

    /// The pile (E5): the heads of set-aside threads, in the unified
    /// shape — the list ordering (date), the fan and the table use it
    /// as-is. Small by construction.
    pub fn set_aside_pile(&self) -> Result<Vec<UnifiedRow>, Error> {
        let queue = unified_join_tail(false);
        let sql = format!(
            "{SELECT_UNIFIED}{THREAD_AGGREGATE}
             FROM (SELECT account_id, last_mailbox_id, last_uid, last_epoch, size, unseen
                     FROM threads
                    WHERE inbox_size > 0 AND id IN ({SET_ASIDE_THREADS})) t{queue}"
        );
        let mut stmt = self.0.prepare(&sql)?;
        let rows = stmt
            .query_map([], row_to_threaded)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// R1 (PLAN-RETOURS-11, D1-D2): the “Show images” choice for the
    /// message — envelope key, the `pins` pattern. Replaying the
    /// gesture changes nothing (REPLACE).
    pub fn allow_images_message(&self, mailbox_id: i64, uid: Uid, epoch: i64) -> Result<(), Error> {
        self.0.execute(
            "INSERT OR REPLACE INTO images_messages (mailbox_id, uid, epoch)
             VALUES (?1, ?2, ?3)",
            params![mailbox_id, uid, epoch],
        )?;
        Ok(())
    }

    /// D3: sets the rule “always show images from this sender” FROM
    /// a message — the address is read from the envelope (never from
    /// the UI), normalized to lowercase. Returns the address set; None
    /// if the envelope has no address (never an empty rule). Does NOT
    /// write a per-message choice: the sender rule must stand alone,
    /// and its revocation undo everything.
    pub fn allow_images_sender_of(
        &self,
        mailbox_id: i64,
        uid: Uid,
        epoch: i64,
    ) -> Result<Option<String>, Error> {
        let address: Option<String> = self
            .0
            .query_row(
                "SELECT sender_address FROM envelopes WHERE mailbox_id = ?1 AND uid = ?2",
                params![mailbox_id, uid],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
        let Some(address) = images_address(address) else {
            return Ok(None);
        };
        self.0.execute(
            "INSERT OR REPLACE INTO images_expediteurs (address, epoch) VALUES (?1, ?2)",
            params![address, epoch],
        )?;
        Ok(Some(address))
    }

    /// D4: removes a sender rule (the exit door of “always”). The
    /// normalization goes through the SAME authority as the write —
    /// otherwise a rule once set would become irrevocable the day
    /// `images_address` changes.
    pub fn revoke_images_sender(&self, address: &str) -> Result<(), Error> {
        let Some(address) = images_address(Some(address.to_string())) else {
            return Ok(());
        };
        self.0.execute(
            "DELETE FROM images_expediteurs WHERE address = ?1",
            params![address],
        )?;
        Ok(())
    }

    /// The sender rules, for the Settings list (D4) — alphabetical
    /// order: the eye looks for an address there.
    pub fn images_senders(&self) -> Result<Vec<String>, Error> {
        let mut stmt = self
            .0
            .prepare("SELECT address FROM images_expediteurs ORDER BY address")?;
        let addresses = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(addresses)
    }

    /// The RENDER gate (message_body): is this message entitled to
    /// remote images? A per-message choice OR a sender rule — the
    /// envelope's address is normalized through the SAME path as the
    /// write (a single authority, never SQLite's ASCII lower()).
    pub fn images_allowed(&self, mailbox_id: i64, uid: Uid) -> Result<bool, Error> {
        if self
            .0
            .prepare("SELECT 1 FROM images_messages WHERE mailbox_id = ?1 AND uid = ?2")?
            .exists(params![mailbox_id, uid])?
        {
            return Ok(true);
        }
        let address: Option<String> = self
            .0
            .query_row(
                "SELECT sender_address FROM envelopes WHERE mailbox_id = ?1 AND uid = ?2",
                params![mailbox_id, uid],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
        match images_address(address) {
            Some(address) => Ok(self
                .0
                .prepare("SELECT 1 FROM images_expediteurs WHERE address = ?1")?
                .exists(params![address])?),
            None => Ok(false),
        }
    }

    // -----------------------------------------------------------------
    // Spring cleaning (PLAN-HORIZON-NETTOYAGE part B).
    // -----------------------------------------------------------------

    /// The messages of a conversation, from oldest to most recent —
    /// the reading order of an exchange.
    /// The messages of a thread, in THREE columns (account, mailbox,
    /// UID) — what a bulk gesture needs to know, without hydrating
    /// full rows (wave 2 review: `thread_messages` joined body and
    /// threads for three scalars).
    pub fn messages_of_thread(&self, thread_id: i64) -> Result<Vec<(i64, String, Uid)>, Error> {
        let mut stmt = self.0.prepare_cached(
            "SELECT m.account_id, m.name, e.uid FROM envelopes e
             JOIN mailboxes m ON m.id = e.mailbox_id
             WHERE e.thread_id = ?1 ORDER BY e.date_epoch DESC, e.uid DESC",
        )?;
        let rows = stmt
            .query_map([thread_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn thread_messages(&self, thread_id: i64) -> Result<Vec<UnifiedRow>, Error> {
        // Join on `threads`, not the “message alone” mapping: each
        // message must come back knowing the size of ITS thread.
        // Without it, it would be 1, and the screen would conclude
        // there is no conversation to show — at the exact moment it's
        // being browsed.
        let mut stmt = self.0.prepare(&format!(
            "{SELECT_UNIFIED}{THREAD_AGGREGATE}
             FROM envelopes e
             JOIN threads t ON t.id = e.thread_id
             JOIN mailboxes m ON m.id = e.mailbox_id
             JOIN accounts a ON a.id = m.account_id
             LEFT JOIN bodies b ON b.mailbox_id = e.mailbox_id AND b.uid = e.uid
             WHERE e.thread_id = ?1
             ORDER BY e.date_epoch ASC, e.uid ASC"
        ))?;
        let rows = stmt
            .query_map([thread_id], row_to_threaded)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// A specific envelope — the context needed to reply (the
    /// sender's raw address, the thread's Message-ID).
    /// A message's `Reply-To`, if it carries one — read on demand
    /// (“Reply”), never in list rows.
    pub fn reply_to_of(
        &self,
        account_id: i64,
        mailbox: &str,
        uid: Uid,
    ) -> Result<Option<String>, Error> {
        Ok(self
            .0
            .query_row(
                "SELECT e.reply_to FROM envelopes e
                 JOIN mailboxes m ON m.id = e.mailbox_id
                 WHERE m.account_id = ?1 AND m.name = ?2 AND e.uid = ?3",
                params![account_id, mailbox, uid],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten()
            .map(|address| address.trim().to_string())
            .filter(|address| !address.is_empty()))
    }

    pub fn envelope(
        &self,
        account_id: i64,
        mailbox: &str,
        uid: Uid,
    ) -> Result<Option<Envelope>, Error> {
        let envelope = self
            .0
            .query_row(
                "SELECT e.uid, e.subject, e.sender, e.sender_address, e.message_id,
                        e.date_epoch, e.seen, e.flagged, e.in_reply_to, e.to_addrs, e.cc_addrs
                 FROM envelopes e JOIN mailboxes m ON m.id = e.mailbox_id
                 WHERE m.account_id = ?1 AND m.name = ?2 AND e.uid = ?3",
                params![account_id, mailbox, uid],
                row_to_envelope,
            )
            .optional()?;
        Ok(envelope)
    }

    /// The `References` chain a reply to this message must carry
    /// (RFC 5322 §3.6.4): the parent's `References` + its
    /// `Message-ID`. `None`: unknown message or no Message-ID. E7:
    /// before, the send only carried the parent and broke the thread
    /// for the recipient.
    pub fn references_of(
        &self,
        account_id: i64,
        mailbox: &str,
        uid: Uid,
    ) -> Result<Option<String>, Error> {
        let line: Option<(Option<String>, Option<String>)> = self
            .0
            .query_row(
                "SELECT e.refs, e.message_id
                 FROM envelopes e JOIN mailboxes m ON m.id = e.mailbox_id
                 WHERE m.account_id = ?1 AND m.name = ?2 AND e.uid = ?3",
                params![account_id, mailbox, uid],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        Ok(line.and_then(|(refs, message_id)| {
            let message_id = message_id?;
            let refs = refs.unwrap_or_default();
            let refs = refs.trim();
            Some(if refs.is_empty() {
                message_id
            } else {
                format!("{refs} {message_id}")
            })
        }))
    }

    pub fn count(&self, mailbox_id: i64) -> Result<u64, Error> {
        let count: i64 = self.0.query_row(
            "SELECT COUNT(*) FROM envelopes WHERE mailbox_id = ?1",
            [mailbox_id],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }

    pub fn max_uid(&self, mailbox_id: i64) -> Result<Uid, Error> {
        let max: Uid = self.0.query_row(
            "SELECT COALESCE(MAX(uid), 0) FROM envelopes WHERE mailbox_id = ?1",
            [mailbox_id],
            |row| row.get(0),
        )?;
        Ok(max)
    }

    /// How many envelopes of a mailbox carry a UID STRICTLY above the
    /// marker — the ARRIVALS of a poll that has just closed out
    /// (PLAN-REACTIVITE E4, field session of 2026-08-14). The
    /// report's `fetched` can't count them: a CONDSTORE delta mixes
    /// in every flag that slipped — and Gmail slips the modseq on
    /// every label. Only the UID separates the new from the merely
    /// touched.
    pub fn arrivals_since(&self, account_id: i64, mailbox: &str, uid: Uid) -> Result<u64, Error> {
        let Some(state) = self.sync_state(account_id, mailbox)? else {
            return Ok(0);
        };
        let count: i64 = self.0.query_row(
            "SELECT COUNT(*) FROM envelopes WHERE mailbox_id = ?1 AND uid > ?2",
            params![state.mailbox_id, uid],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }
}

/// Evolves a database from a previous version in place: columns are
/// added without losing what's already there, and the multi-account
/// switch (Phase 3) rebuilds the tables whose constraints change.
/// Generic IMAP/SMTP account server configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountConfig {
    pub imap_host: Option<String>,
    pub imap_port: Option<u16>,
    pub smtp_host: Option<String>,
    pub smtp_port: Option<u16>,
    pub username: Option<String>,
}

/// Does this envelope change any of the five indexed fields? Pure
/// decision of `upsert_envelopes` (PLAN-AUDIT-V3 E2). Comparison by
/// reference (review): five clones per re-read envelope was 25,000
/// allocations for nothing on 5,000 CONDSTORE deltas.
fn needs_reindex(
    existing: Option<&IndexedFields>,
    subject: Option<&str>,
    sender: Option<&str>,
    sender_address: Option<&str>,
    to_field: Option<&str>,
    cc_field: Option<&str>,
) -> bool {
    existing.is_none_or(|existing| {
        existing.0.as_deref() != subject
            || existing.1.as_deref() != sender
            || existing.2.as_deref() != sender_address
            || existing.3.as_deref() != to_field
            || existing.4.as_deref() != cc_field
    })
}

/// The action a Screener No rule takes on an arriving message — pure
/// decision of `upsert_envelopes`. `corbeille` is the server's trash,
/// NEVER a permanent deletion (D4); `spam` acts only when the account
/// resolved a junk folder (stated limit of PLAN-MODE-ORGANISE E3).
fn no_rule_action(rule: &str, junk_folder: Option<&str>) -> Option<Action> {
    match rule {
        "archive" => Some(Action::Archive),
        "corbeille" => Some(Action::Delete),
        "spam" => junk_folder.map(|folder| Action::MoveTo(folder.to_string())),
        _ => None,
    }
}

/// Does a message dated `date_epoch` fall under a verdict passed at
/// `verdict`? "Their next messages": a backfill of history never
/// archives nor discards; a message WITHOUT a date is treated as
/// arriving today — spam with no Date header would otherwise dodge
/// the very gate (review E2/E3).
fn arrived_after_verdict(date_epoch: Option<i64>, verdict: i64) -> bool {
    date_epoch.is_none_or(|date| date > verdict)
}

#[cfg(test)]
mod tests;

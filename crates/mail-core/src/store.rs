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

/// Columns of the unified SELECT, shared by [`Store::unified_recent`]
/// and [`Store::search`] — the order is that of [`row_to_unified`].
/// The last column is an EXISTS on `attachments`: the list must be
/// able to show the paperclip without one query per row. The primary
/// key (mailbox_id, uid, idx) makes this test indexed.
// Requires the aliases `e` (envelopes), `m` (mailboxes), `a`
// (accounts) AND the join `LEFT JOIN bodies b` — the list preview
// comes from there, NULL until the body is fetched. The attachment
// COUNT replaces the old EXISTS: the prototype's chip says "2 files",
// not "some files". Both only run on the rows KEPT by pagination
// (gate P1).
pub(crate) const SELECT_UNIFIED: &str = "SELECT a.id, a.email, e.uid, e.subject, e.sender, e.sender_address, e.message_id, e.date_epoch, e.seen, e.flagged, (SELECT COUNT(*) FROM attachments att WHERE att.mailbox_id = e.mailbox_id AND att.uid = e.uid), e.thread_id, e.in_reply_to, m.name, b.preview, e.to_addrs, e.cc_addrs";

/// The SELECT for the grouped list: the columns above, plus the thread
/// aggregate. It requires the join on `threads` (alias `t`), which
/// search does not have — a search result is ONE message, not a
/// conversation. Comes AFTER `to_addrs`/`cc_addrs` of
/// [`SELECT_UNIFIED`]: `t.size`/`t.unseen` are therefore at indices
/// 17/18.
pub(crate) const THREAD_AGGREGATE: &str = ", t.size, t.unseen";

/// PINNED threads (R4, PLAN-RETOURS-7) — the subquery shared by the
/// page (exclusion, D5), the count, and the standalone service.
/// Materialized ONCE per query (LIST SUBQUERY), small by construction
/// (a handful of pins at most) — but ONLY IF `pins` is the outer
/// table: without `ANALYZE` (never run here), SQLite picks `envelopes`
/// as the outer table and pays a FULL scan of the widest table on the
/// hottest path (review 2026-08-21, measured on the bench: ~24 ms per
/// page at 200k). The `CROSS JOIN` is SQLite's join-order directive:
/// `pins` is scanned, `envelopes` is probed by its primary key. The
/// plan guard `la_boite_unifiee_ne_materialise_pas_son_tri` proves it.
pub(crate) const PINNED_THREADS: &str = "SELECT pe.thread_id FROM pins p CROSS JOIN envelopes pe ON pe.mailbox_id = p.mailbox_id AND pe.uid = p.uid WHERE pe.thread_id IS NOT NULL";

/// SET-ASIDE threads (E5) — the twin of [`PINNED_THREADS`], same
/// reasons: list materialized once, small by construction, and a
/// directive `CROSS JOIN` (without ANALYZE, SQLite would pick
/// `envelopes` as the outer table — a full scan on the hottest path).
pub(crate) const SET_ASIDE_THREADS: &str = "SELECT ce.thread_id FROM mis_de_cote c CROSS JOIN envelopes ce ON ce.mailbox_id = c.mailbox_id AND ce.uid = c.uid WHERE ce.thread_id IS NOT NULL";

/// The exclusion for the ORGANIZED Inbox — THE single place it is
/// written (review E4/E5: the fragment lived in four copies, the next
/// exclusion — E6, groups — would have missed one, exactly the "badge
/// shows 2 in front of an empty list" bug the E5 screenshot caught):
/// retained/routed threads (flag) and SET-ASIDE threads (the pile).
pub(crate) fn organized_exclusion() -> String {
    format!(" AND organise_hors = 0 AND id NOT IN ({SET_ASIDE_THREADS})")
}

/// The tail of the unified list — joins and final sort — shared by the
/// page ([`unified_page_sql`]) and the pinned section
/// ([`Store::pinned_unified_scoped`]): ONE place to write it, the two
/// queries can no longer drift apart (review 2026-08-21 — copying the
/// skeleton would have shifted the columns on the first addition).
pub(crate) const UNIFIED_JOINS: &str = "
         JOIN envelopes e ON e.mailbox_id = t.last_mailbox_id AND e.uid = t.last_uid
         JOIN mailboxes m ON m.id = e.mailbox_id
         JOIN accounts a ON a.id = t.account_id
         LEFT JOIN bodies b ON b.mailbox_id = e.mailbox_id AND b.uid = e.uid";

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

/// The CLOSED vocabularies of Spring cleaning (part B, D6) — checked on
/// the core side before any write, like routing.
pub const CLEANUP_RANGES: &[&str] = &["3m", "6m", "1a", "2a", "5a", "tout"];
pub const CLEANUP_SCOPES: &[&str] = &["reception", "dossiers", "dossiersArchives", "archives"];

/// The cleanup session in progress (at most one, persisted — D8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupSession {
    pub range: String,
    pub scope: String,
    pub bound_epoch: i64,
    pub total: u64,
    pub handled: u64,
}

/// A Cleanup group: one sender × its mail within the range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupGroup {
    pub address: String,
    pub who: Option<String>,
    pub messages: u64,
    pub last_epoch: i64,
    pub last_subject: Option<String>,
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

    /// Is a legacy database adoption waiting here? Probed in
    /// **read-only** mode: nothing is triggered, nothing is created —
    /// this is what lets the desktop show the migration screen BEFORE
    /// the first real opening, the one that will pay for the pass.
    ///
    /// Returns the number of messages concerned (`None` = nothing to
    /// do). It is an order of magnitude for the waiting screen, not the
    /// denominator of progress: that one comes from
    /// [`Store::open_with_progress`], the only one that knows the exact
    /// scope.
    pub fn pending_adoption(path: &Path) -> Result<Option<u64>, Error> {
        if !path.exists() {
            // First install: nothing legacy, and opening would create
            // the file — a probe leaves no trace.
            return Ok(None);
        }
        let conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        // Two distinct passes may claim the screen, independently:
        // thread adoption (a database from before ADR 0008) AND
        // rebuilding the search index (FTS schema from before the
        // `recipients` column). The second touches databases that are
        // ALREADY up to date on the thread side — without this
        // detection, it would freeze startup silently, outside any
        // screen (field finding 2026-08-17).
        let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        let threads_pending = version < thread::THREADING_VERSION;
        let search_pending = {
            let fts_sql: Option<String> = conn
                .query_row(
                    "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'search_fts'",
                    [],
                    |row| row.get(0),
                )
                .optional()?;
            fts_sql
                .as_deref()
                .is_some_and(|sql| !sql.contains("recipients"))
        };
        if !threads_pending && !search_pending {
            return Ok(None);
        }
        // A database from before threads may not have the table: the
        // direct COUNT would fail, and the probe must answer, not
        // explain.
        let has_envelopes: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'envelopes'",
            [],
            |row| row.get(0),
        )?;
        if has_envelopes == 0 {
            return Ok(None);
        }
        // Rebuilding the index scans ALL envelopes; thread adoption,
        // only the grouping scope (ADR 0010: INBOX + Sent, well below
        // the total — "256,312" for a pass that reattaches 7,500 would
        // not name what it says). The widest pending pass is announced;
        // it is only an order of magnitude, the real denominator comes
        // from `open_with_progress`.
        let messages: i64 = if search_pending {
            conn.query_row("SELECT COUNT(*) FROM envelopes", [], |row| row.get(0))?
        } else if table_columns(&conn, "mailboxes")?.contains("threaded") {
            conn.query_row(
                "SELECT COUNT(*) FROM envelopes e
                 JOIN mailboxes m ON m.id = e.mailbox_id
                 WHERE m.threaded = 1",
                [],
                |row| row.get(0),
            )?
        } else {
            conn.query_row("SELECT COUNT(*) FROM envelopes", [], |row| row.get(0))?
        };
        if messages == 0 {
            Ok(None)
        } else {
            Ok(Some(messages as u64))
        }
    }

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

    fn init(conn: Connection) -> Result<Self, Error> {
        Self::init_with(conn, &mut |_| ControlFlow::Continue(()))
    }

    /// Forgets initialization for ONE path — for tests that REWIND a
    /// database by hand between two openings (the fixture of a
    /// pre-existing database), which the single-instance rule forbids
    /// in production. One path, never the whole registry: tests run in
    /// parallel, and clearing the registry out from under another test
    /// would make it replay a schema it is precisely proving it does
    /// not replay.
    #[cfg(test)]
    pub(crate) fn forget_initialization(path: &Path) {
        // The SAME key as the registry: the one SQLite gives the file.
        if let Some(key) = Connection::open(path).ok().and_then(|conn| file_key(&conn)) {
            initialized_registry().lock().remove(&key);
        }
    }

    fn init_with(
        conn: Connection,
        on_progress: &mut dyn FnMut(AdoptionProgress) -> ControlFlow<()>,
    ) -> Result<Self, Error> {
        // Several commands each open their own connection: wait rather
        // than fail with SQLITE_BUSY on a concurrent write. 30 s and
        // not 5 (field finding 2026-08-15): under heavy machine load, a
        // sync write batch can hold the lock beyond 5 s — a UI gesture
        // (`delete_draft` on an emptied draft) would then die with BUSY
        // and its failure, silenced by the UI of that era, left a ghost
        // in the folder. In WAL, reads never wait; only a write behind
        // a write waits — late beats dead.
        conn.busy_timeout(std::time::Duration::from_secs(30))?;
        // WAL (ADR 0011): a read no longer ever blocks a write, nor the
        // reverse. Rollback mode held up while writes lasted a few
        // seconds; full sync (ADR 0010) stretches them into minutes,
        // and the FIRST field trial produced "database is locked" —
        // the progress probe and the list, by reading, made the header
        // pass's busy_timeout expire.
        //
        // `query_row` and not `pragma_update`: this PRAGMA answers with
        // one row (the effective mode). An in-memory database answers
        // "memory" — that is not a failure, tests live in it just fine
        // without WAL. The mode is PERSISTENT: written once in the file
        // header, reread on every open, legacy databases included.
        conn.query_row("PRAGMA journal_mode = wal", [], |row| {
            row.get::<_, String>(0)
        })?;
        // PLAN-AUDIT-V2 E1 — the fast door: each shell command opens
        // ITS OWN connection (103 call sites); replaying the schema
        // here, some twenty `table_xinfo` calls and the migrations,
        // cost 36 ms on 200k envelopes ON EVERY COMMAND. Once
        // initialization has SUCCEEDED once on a path in this process,
        // subsequent opens only do the two settings above. Safe because
        // single-instance (PLAN-AUDIT-V1 E1) guarantees no other
        // process migrates the database in the meantime, and
        // registration only happens after the adoption's COMMIT (a
        // cancellation, a failure: nothing registered, the whole pass
        // replays). An in-memory database has no path: never
        // registered.
        // Foreign keys are a PER-CONNECTION setting: `SCHEMA` turns
        // them on up front, and the fast door does not replay `SCHEMA`.
        // The wave-2 review found lost cascades there; the test meant
        // to prove it stayed GREEN without this line — rusqlite's
        // `bundled` compiles SQLite with `SQLITE_DEFAULT_FOREIGN_KEYS=1`.
        // The line stays, ahead of the fast door: a belt that does not
        // depend on a compile flag (the test keeps it honest).
        conn.execute_batch("PRAGMA foreign_keys = ON")?;
        conn.execute_batch("PRAGMA foreign_keys = ON")?;
        let key = file_key(&conn);
        if let Some(key) = &key
            && initialized_registry().contains(key)
        {
            return Ok(Self(conn));
        }
        conn.execute_batch(SCHEMA)?;
        // Light migrations first: columns, indexes. Rebuilding the
        // search index lives HERE but is NOT light on a database that
        // already has data (rereading the bodies): it is therefore
        // visible and interruptible via `on_progress`, and
        // `pending_adoption` has it preceded by a screen (otherwise, a
        // silent startup freeze — field finding 2026-08-17). Thread
        // adoption, just below, needs the columns these migrations add
        // (`thread_id`, `in_reply_to`, `refs`).
        migrate(&conn, on_progress)?;
        // ——— The unity of threads, as one piece (handover §8). ———
        // From the conditional DROP to `user_version`, everything lives
        // in ONE transaction: cancelling during adoption rewinds
        // EVERYTHING — a partial adoption persisted would be a
        // half-empty mailbox, the list starting from `threads`. The
        // BEGIN is DEFERRED: on an up-to-date database nothing writes,
        // the transaction stays a reader and never meets the writer of
        // a long sync (ADR 0011).
        conn.execute_batch("BEGIN")?;
        let unit = (|| {
            // BEFORE the thread schema, never after: if the grouping
            // rule has changed, both tables must DISAPPEAR so that the
            // `CREATE TABLE IF NOT EXISTS` just below recreates them in
            // their new shape. Without this, opening fails — see
            // `thread::drop_if_outdated`.
            thread::drop_if_outdated(&conn)?;
            conn.execute_batch(thread::SCHEMA)?;
            thread::migrate_threads_with(&conn, on_progress)
        })();
        let announced = match unit {
            Ok(announced) => {
                conn.execute_batch("COMMIT")?;
                announced
            }
            Err(err) => {
                // A rollback failure would teach nothing more than the
                // original error, which is the one that must be
                // surfaced — including a deliberate cancellation.
                let _ = conn.execute_batch("ROLLBACK");
                return Err(err);
            }
        };
        if let Some(total) = announced {
            // "Done" is only said once the pass is COMMITTED — never
            // before (a signal must be observable, handover §9). Too
            // late to cancel: the answer is ignored.
            let _ = on_progress(AdoptionProgress { done: total, total });
        }
        let store = Self(conn);
        // The contacts directory backfills ONCE from existing data
        // (PLAN-RETOURS-5): set-based, marked in `prefs` — on an
        // up-to-date database, one SELECT and nothing else.
        store.backfill_contacts()?;
        if let Some(key) = key {
            initialized_registry().insert(key);
        }
        Ok(store)
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
                // Comparison by reference (review): five clones per
                // re-read envelope was 25,000 allocations for nothing on
                // 5,000 CONDSTORE deltas.
                let needs_reindex = existing.as_ref().is_none_or(|existing| {
                    existing.0.as_deref() != envelope.subject.as_deref()
                        || existing.1.as_deref() != envelope.sender.as_deref()
                        || existing.2.as_deref() != envelope.sender_address.as_deref()
                        || existing.3.as_deref() != to_field.as_deref()
                        || existing.4.as_deref() != cc_field.as_deref()
                });
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
                    && envelope
                        .date
                        .map(|d| d.timestamp())
                        .is_none_or(|date| date > verdict)
                {
                    let action = match rule.as_str() {
                        "archive" => Some(Action::Archive),
                        "corbeille" => Some(Action::Delete),
                        "spam" => junk_folder.clone().map(Action::MoveTo),
                        _ => None,
                    };
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
    /// backfill's work ([ADR 0007](../../../docs/adr/0007-rattrapage-des-corps.md)).
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

    /// The state of Organized mode (PLAN-MODE-ORGANISE E1, D2 amended:
    /// `prefs` SQLite — the core needs to know, the No rules die with
    /// the mode). Off as long as nothing has been set.
    pub fn organized_mode(&self) -> Result<bool, Error> {
        Ok(self.text_pref(PREF_ORGANIZED_MODE)?.as_deref() == Some("1"))
    }

    /// The epoch of the FIRST activation of the mode — the bound of
    /// the Screener's retention (D3 “arrivals only”). None as long as
    /// the mode has never been activated.
    pub fn organized_mode_epoch(&self) -> Result<Option<i64>, Error> {
        Ok(self
            .text_pref(PREF_ORGANIZED_MODE_EPOCH)?
            .and_then(|v| v.parse().ok()))
    }

    /// Toggles the mode. On the FIRST activation, the state and the
    /// epoch are written TOGETHER (transaction — never an active mode
    /// without its bound); the epoch is NEVER rewritten afterwards:
    /// rewriting it on a reactivation would silently dump mail that
    /// arrived in the meantime into the Screener (or the Inbox).
    pub fn set_organized_mode(&mut self, active: bool, epoch: i64) -> Result<(), Error> {
        if active && self.text_pref(PREF_ORGANIZED_MODE_EPOCH)?.is_none() {
            self.set_text_prefs(&[
                (PREF_ORGANIZED_MODE, "1"),
                (PREF_ORGANIZED_MODE_EPOCH, &epoch.to_string()),
            ])
        } else {
            self.set_text_pref(PREF_ORGANIZED_MODE, if active { "1" } else { "0" })
        }
    }

    /// RETOURS-13 R10 — marks a Feed card as read (the bottom of its
    /// elevation has been shown). Idempotent; envelope key, the `pins`
    /// pattern — never the IMAP `seen` (different semantics, overwritten
    /// by sync).
    pub fn mark_feed_read(&self, mailbox_id: i64, uid: Uid, epoch: i64) -> Result<(), Error> {
        self.0.execute(
            "INSERT OR IGNORE INTO kiosque_lus (mailbox_id, uid, epoch) VALUES (?1, ?2, ?3)",
            params![mailbox_id, uid, epoch],
        )?;
        Ok(())
    }

    /// Has a Feed card already been read? (PK probe)
    pub fn feed_read(&self, mailbox_id: i64, uid: Uid) -> Result<bool, Error> {
        Ok(self
            .0
            .prepare("SELECT 1 FROM kiosque_lus WHERE mailbox_id = ?1 AND uid = ?2")?
            .exists(params![mailbox_id, uid])?)
    }

    /// RETOURS-13 R5/R9 — the default actions of the Screener's
    /// Yes/No buttons. Shipped: Yes → `inbox`, No → `trash`. A
    /// value outside the vocabulary in the database (written outside
    /// the gate) falls back to the default: the bare click NEVER sets
    /// a broken verdict.
    pub fn screener_defaults(&self) -> Result<(String, String), Error> {
        let yes = self
            .text_pref(PREF_SCREENER_DEFAULT_YES)?
            .filter(|v| valid_screener_default_yes(v))
            .unwrap_or_else(|| "reception".to_string());
        let no = self
            .text_pref(PREF_SCREENER_DEFAULT_NO)?
            .filter(|v| valid_screener_default_no(v))
            .unwrap_or_else(|| "corbeille".to_string());
        Ok((yes, no))
    }

    /// Sets the Screener's defaults — a CLOSED vocabulary, checked
    /// before any write (pure decision): Yes takes a destination
    /// (never `ecarte`), No a rule or “screen out without moving”.
    pub fn set_screener_defaults(&mut self, yes: &str, no: &str) -> Result<(), Error> {
        if !valid_screener_default_yes(yes) {
            return Err(Error::InvalidRouting(format!(
                "unknown Yes default: {yes:?}"
            )));
        }
        if !valid_screener_default_no(no) {
            return Err(Error::InvalidRouting(format!("unknown No default: {no:?}")));
        }
        self.set_text_prefs(&[
            (PREF_SCREENER_DEFAULT_YES, yes),
            (PREF_SCREENER_DEFAULT_NO, no),
        ])
    }

    /// Sets the Organized mode verdict on a sender
    /// (PLAN-MODE-ORGANISE E1, D1: LOCAL routing only). One verdict per
    /// address — setting it overwrites the previous decision (changing
    /// one's mind is a right, the Screener's pattern). The vocabulary
    /// is closed and checked BEFORE the write (pure decision); a No
    /// rule only makes sense on a screened-out sender.
    pub fn route_sender(
        &self,
        address: &str,
        destination: &str,
        rule: Option<&str>,
        epoch: i64,
    ) -> Result<(), Error> {
        validate_routing(destination, rule)?;
        let Some(address) = images_address(Some(address.to_string())) else {
            return Err(Error::InvalidEmailAddress(address.to_string()));
        };
        // Verdict, exit from waiting, and thread flags in ONE
        // transaction (E2): a half-applied verdict would leave a sender in
        // the Screener AND in its view.
        let tx = self.0.unchecked_transaction()?;
        set_verdict(&tx, &address, destination, rule, epoch)?;
        tx.commit()?;
        Ok(())
    }

    /// “Move to…” (E1): sets the verdict FROM a message — the address
    /// is read from the database (never from the UI), normalized and
    /// validated by [`Store::route_sender`], THE single gate.
    ///
    /// E1 review: the row served is the HEAD of the thread — the last
    /// message across all mailboxes, Sent included. Anchoring on it
    /// would route the user's OWN address as soon as they replied last.
    /// The routed address is therefore that of the last message of the
    /// thread that does NOT come from the account (send aliases stay
    /// outside this guard — a stated limit); a message outside a thread
    /// falls back to its own envelope. Returns the routed address;
    /// None if nothing carries an address (never a phantom verdict).
    pub fn route_sender_of(
        &self,
        mailbox_id: i64,
        uid: Uid,
        destination: &str,
        rule: Option<&str>,
        epoch: i64,
    ) -> Result<Option<String>, Error> {
        // The validation gate first — a broken vocabulary never hides
        // behind “message without an address” (E1 review).
        validate_routing(destination, rule)?;
        let of_thread: Option<String> = self
            .0
            .query_row(
                "SELECT te.sender_address
                   FROM envelopes te
                  WHERE te.thread_id = (SELECT thread_id FROM envelopes
                                         WHERE mailbox_id = ?1 AND uid = ?2)
                    AND te.sender_address IS NOT NULL
                    AND lower(trim(te.sender_address)) <> (
                          SELECT lower(trim(a.email)) FROM accounts a
                            JOIN mailboxes m ON m.account_id = a.id
                           WHERE m.id = ?1)
                  ORDER BY te.date_epoch DESC, te.uid DESC
                  LIMIT 1",
                params![mailbox_id, uid],
                |row| row.get(0),
            )
            .optional()?;
        let address = match of_thread {
            Some(a) => Some(a),
            None => self.sender_address_of(mailbox_id, uid)?,
        };
        let Some(address) = images_address(address) else {
            return Ok(None);
        };
        self.route_sender(&address, destination, rule, epoch)?;
        Ok(Some(address))
    }

    /// The sender address of ONE envelope — the shared read of the
    /// gates that resolve on the core side (image guard, routing): a
    /// single copy, never a divergence (lesson A80).
    fn sender_address_of(&self, mailbox_id: i64, uid: Uid) -> Result<Option<String>, Error> {
        Ok(self
            .0
            .query_row(
                "SELECT sender_address FROM envelopes WHERE mailbox_id = ?1 AND uid = ?2",
                params![mailbox_id, uid],
                |row| row.get(0),
            )
            .optional()?
            .flatten())
    }

    /// “Reinstate” from the Screener's history: the verdict
    /// disappears, the sender becomes unknown again. The normalization
    /// goes through the SAME authority as the write — otherwise a
    /// verdict would become irrevocable the day it changes (lesson
    /// `revoke_images_sender`).
    pub fn remove_routing(&self, address: &str) -> Result<(), Error> {
        let Some(address) = images_address(Some(address.to_string())) else {
            return Ok(());
        };
        let epoch = self.organized_mode_epoch()?;
        let tx = self.0.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM routage_expediteurs WHERE address = ?1",
            params![address],
        )?;
        // “Reinstate” (E2): an UNKNOWN sender — no mail before the
        // epoch — becomes a waiting sender again, their messages
        // reappear in the Screener; a known one is simply returned to
        // the Inbox, never to the desk (D3: their history is
        // authoritative). Never oneself (lesson E1). The exit gate
        // follows the SAME rule as arrival (E2 review): only mail that
        // ARRIVED after the epoch reinstates — a sender seen only in
        // Archive or Junk never passed the desk.
        if let Some(epoch) = epoch
            && !account_address(&tx, &address)?
            && !known_before_epoch(&tx, &address, epoch)?
            && tx
                .prepare(
                    "SELECT 1 FROM envelopes e
                       JOIN mailboxes m ON m.id = e.mailbox_id
                      WHERE e.sender_norm = ?1
                        AND (e.date_epoch > ?2 OR e.date_epoch IS NULL)
                        AND m.name = ?3 LIMIT 1",
                )?
                .exists(params![address, epoch, thread::RECEIVED_MAILBOX])?
        {
            tx.execute(
                "INSERT OR IGNORE INTO portier_attente (address) VALUES (?1)",
                params![address],
            )?;
        }
        refresh_threads_of(&tx, &address)?;
        tx.commit()?;
        Ok(())
    }

    /// The verdict set on a sender, if it exists.
    pub fn routing_of(&self, address: &str) -> Result<Option<Routing>, Error> {
        let Some(address) = images_address(Some(address.to_string())) else {
            return Ok(None);
        };
        let routing = self
            .0
            .query_row(
                "SELECT address, destination, regle, epoch
                 FROM routage_expediteurs WHERE address = ?1",
                params![address],
                read_routing,
            )
            .optional()?;
        Ok(routing)
    }

    /// The Screener's history: all decisions, the most recent first —
    /// the eye looks for the last verdict there.
    pub fn routings(&self) -> Result<Vec<Routing>, Error> {
        let mut stmt = self.0.prepare(
            "SELECT address, destination, regle, epoch
             FROM routage_expediteurs ORDER BY epoch DESC, address",
        )?;
        let routings = stmt
            .query_map([], read_routing)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(routings)
    }

    /// The Screener's desk (E2): one rank per waiting sender — the
    /// normalized address (the key the verdict will take) and its
    /// LAST arrival after the epoch, in the format of list rows. The
    /// most recent first. Empty as long as the mode has never been
    /// activated. The desk only counts ARRIVALS (E2 review): a message
    /// from the same sender already discarded or archived is neither
    /// the rank nor the count — the rank's mailbox is the INBOX. The
    /// probes follow `idx_envelopes_sender` (0.32 ms at 200 k,
    /// S2-bis); a rank whose mail has disappeared is not served.
    pub fn screener_waiting(&self) -> Result<Vec<ScreenerRank>, Error> {
        let Some(epoch) = self.organized_mode_epoch()? else {
            return Ok(Vec::new());
        };
        // `COALESCE` on the aggregate: the rank shows ONE message — if
        // its thread doesn't exist (mailbox out of scope), it counts for
        // itself.
        let sql = format!(
            "{SELECT_UNIFIED}, COALESCE(t.size, 1), COALESCE(t.unseen, 1 - e.seen), pa.address
             FROM portier_attente pa
             JOIN envelopes e ON e.rowid = (
                  SELECT e2.rowid FROM envelopes e2
                    JOIN mailboxes m2 ON m2.id = e2.mailbox_id AND m2.name = ?2
                   WHERE e2.sender_norm = pa.address
                     AND (e2.date_epoch > ?1 OR e2.date_epoch IS NULL)
                   ORDER BY e2.date_epoch DESC, e2.uid DESC LIMIT 1)
             JOIN mailboxes m ON m.id = e.mailbox_id
             JOIN accounts a ON a.id = m.account_id
             LEFT JOIN threads t ON t.id = e.thread_id
             LEFT JOIN bodies b ON b.mailbox_id = e.mailbox_id AND b.uid = e.uid
             ORDER BY e.date_epoch DESC, e.uid DESC"
        );
        let mut stmt = self.0.prepare(&sql)?;
        let ranks = stmt
            .query_map(params![epoch, thread::RECEIVED_MAILBOX], |row| {
                Ok(ScreenerRank {
                    row: row_to_threaded(row)?,
                    address: row.get(19)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ranks)
    }

    /// The Screener's badge: how many MESSAGES are waiting — the
    /// arrivals after the epoch of the waiting senders, the same scope
    /// as the desk (never a count the page couldn't show). Sum of
    /// index intervals, 0.26 ms at 200 k.
    pub fn screener_total(&self) -> Result<u64, Error> {
        let Some(epoch) = self.organized_mode_epoch()? else {
            return Ok(0);
        };
        let total: i64 = self.0.query_row(
            "SELECT COALESCE(SUM((SELECT COUNT(*) FROM envelopes e
                     JOIN mailboxes m ON m.id = e.mailbox_id AND m.name = ?2
                     WHERE e.sender_norm = pa.address
                       AND (e.date_epoch > ?1 OR e.date_epoch IS NULL))), 0)
             FROM portier_attente pa",
            params![epoch, thread::RECEIVED_MAILBOX],
            |row| row.get(0),
        )?;
        Ok(total as u64)
    }

    /// RETOURS-14 R4 (review) — the ADDRESSES alone from the desk,
    /// for the thread's “Waiting in the Screener” badge:
    /// `screener_waiting()` builds a full row per sender, which the
    /// badge has no use for — and the desk is unbounded.
    pub fn screener_addresses(&self) -> Result<Vec<String>, Error> {
        let mut stmt = self
            .0
            .prepare("SELECT address FROM portier_attente ORDER BY address")?;
        let addresses = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(addresses)
    }

    // -----------------------------------------------------------------
    // Spring cleaning (PLAN-HORIZON-NETTOYAGE part B).
    // -----------------------------------------------------------------

    /// The mailboxes a scope covers (D6, CE vocabulary) — resolved
    /// per account from the canonicals. Sent, Drafts, Junk and Trash
    /// are ALWAYS out of scope (we don't sort what is already handled
    /// or written by oneself). The FULL archive (“All messages”)
    /// would replay the entire mailbox: out of scope — a stated limit
    /// in the PLAN.
    fn mailboxes_in_scope(&self, scope: &str) -> Result<Vec<i64>, Error> {
        let folders_included = matches!(scope, "dossiers" | "dossiersArchives");
        let archives_included = matches!(scope, "archives" | "dossiersArchives");
        let mut ids = Vec::new();
        let mut stmt = self
            .0
            .prepare_cached("SELECT id, name FROM mailboxes WHERE account_id = ?1")?;
        for account in self.accounts()? {
            let canon = self.canonical_folders(account.id)?;
            let mailboxes = stmt
                .query_map([account.id], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            for (id, name) in mailboxes {
                let is = |canonical: &Option<String>| canonical.as_deref() == Some(name.as_str());
                let included = if name == canon.inbox {
                    true
                } else if is(&canon.archives) {
                    archives_included && !canon.archives_full
                } else if is(&canon.sent)
                    || is(&canon.drafts)
                    || is(&canon.junk)
                    || is(&canon.trash)
                {
                    false
                } else {
                    folders_included
                };
                if included {
                    ids.push(id);
                }
            }
        }
        Ok(ids)
    }

    /// The SQL list of mailbox ids in scope — ONE definition (review
    /// 2026-08-30: three copies were starting to diverge). The ids
    /// come from OUR own database — never from user input.
    fn id_list(ids: &[i64]) -> String {
        if ids.is_empty() {
            "NULL".to_string()
        } else {
            ids.iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(",")
        }
    }

    /// The shared criterion for groups: mail in the range within the
    /// scope, excluding senders already routed (D7), excluding
    /// oneself, excluding envelopes without an address. THE only
    /// definition of “the range within the scope” — groups, the
    /// verdict's stock, and a group's view all share it. A message
    /// WITHOUT a date counts in every range (precedent A98: “no date
    /// = today” — a stated limit in the PLAN: it also follows the
    /// stock's rules).
    fn cleanup_criterion(ids: &[i64]) -> String {
        let list = Self::id_list(ids);
        format!(
            "e.mailbox_id IN ({list})
               AND (e.date_epoch > ?1 OR e.date_epoch IS NULL)
               AND e.sender_norm IS NOT NULL
               AND e.sender_norm NOT IN (SELECT address FROM routage_expediteurs WHERE address IS NOT NULL)
               AND e.sender_norm NOT IN (SELECT lower(trim(email)) FROM accounts WHERE email IS NOT NULL)"
        )
    }

    /// The SQL for groups — ONE pass: with a SINGLE max(), SQLite
    /// guarantees the bare columns (sender, subject) come from the max
    /// row — the rank shows the subject of the last message IN SCOPE
    /// (review 2026-08-30: two unbounded correlated subqueries could
    /// show the subject of a message outside the session, and repaid
    /// the sender sort four times per group).
    ///
    /// In TWO phases over the sender index (PLAN-AUDIT-V2 E4): the
    /// aggregate is COVERED by `idx_envelopes_sender` (sender, date,
    /// mailbox — never a table row read), then the subject and name
    /// of the last message are looked up through the same index.
    /// Measured on 200 k envelopes and 5 000 senders: 380 ms → under
    /// 100 (the old pass walked the DATE index then a temporary
    /// B-tree). `INDEXED BY`: the planner preferred the date index —
    /// the plan test `cleanup_groups_are_read_via_the_senders_index`
    /// keeps the promise. The outer `GROUP BY` absorbs a date tie
    /// (two messages from a sender in the same second): one rank per
    /// group, never two.
    fn cleanup_groups_sql(ids: &[i64]) -> String {
        let criterion = Self::cleanup_criterion(ids);
        let list = Self::id_list(ids);
        format!(
            "SELECT g.sender_norm, g.n, g.dernier, e.sender, e.subject
               FROM (SELECT e.sender_norm AS sender_norm, COUNT(*) AS n,
                            MAX(e.date_epoch) AS dernier
                       FROM envelopes e INDEXED BY {SENDERS_INDEX}
                      WHERE {criterion}
                      GROUP BY e.sender_norm) g
               CROSS JOIN envelopes e INDEXED BY {SENDERS_INDEX}
                 ON e.sender_norm = g.sender_norm
                AND e.date_epoch IS g.dernier
                AND e.mailbox_id IN ({list})
              GROUP BY g.sender_norm
              ORDER BY g.dernier DESC, g.sender_norm"
        )
    }

    /// A group's mail — THE shared criterion: the view shows exactly
    /// what the verdict will process. `INDEXED BY` (PLAN-AUDIT-V2
    /// E4): without it, the embedded SQLite preferred the date index
    /// and scanned the entire mailbox for 40 rows — 116 ms on 200k.
    fn cleanup_messages_sql(ids: &[i64]) -> String {
        let criterion = Self::cleanup_criterion(ids);
        format!(
            "{SELECT_UNIFIED}, COALESCE(t.size, 1), COALESCE(t.unseen, 1 - e.seen)
             FROM envelopes e INDEXED BY {SENDERS_INDEX}
             JOIN mailboxes m ON m.id = e.mailbox_id
             JOIN accounts a ON a.id = m.account_id
             LEFT JOIN threads t ON t.id = e.thread_id
             LEFT JOIN bodies b ON b.mailbox_id = e.mailbox_id AND b.uid = e.uid
             WHERE e.sender_norm = ?2 AND {criterion}
             ORDER BY e.date_epoch DESC, e.uid DESC"
        )
    }

    fn cleanup_count_groups(&self, ids: &[i64], bound: i64) -> Result<u64, Error> {
        let criterion = Self::cleanup_criterion(ids);
        let total: i64 = self.0.query_row(
            &format!(
                "SELECT COUNT(*) FROM (SELECT 1 FROM envelopes e INDEXED BY {SENDERS_INDEX}
                  WHERE {criterion} GROUP BY e.sender_norm)"
            ),
            params![bound],
            |row| row.get(0),
        )?;
        Ok(total as u64)
    }

    /// The current session — `None`: no cleanup started.
    pub fn cleanup_state(&self) -> Result<Option<CleanupSession>, Error> {
        Ok(self
            .0
            .query_row(
                "SELECT plage, perimetre, borne_epoch, total, traites
                   FROM nettoyage_session WHERE id = 1",
                [],
                |row| {
                    Ok(CleanupSession {
                        range: row.get(0)?,
                        scope: row.get(1)?,
                        bound_epoch: row.get(2)?,
                        total: row.get::<_, i64>(3)? as u64,
                        handled: row.get::<_, i64>(4)? as u64,
                    })
                },
            )
            .optional()?)
    }

    /// Starts a cleanup (replaces the current session): the bound is
    /// FIXED here — a session doesn't drift with the clock — and the
    /// group total becomes the denominator of progress.
    pub fn cleanup_start(
        &self,
        range: &str,
        scope: &str,
        now: i64,
    ) -> Result<CleanupSession, Error> {
        if !CLEANUP_RANGES.contains(&range) {
            return Err(Error::Corrupt(format!("unknown range: {range:?}")));
        }
        if !CLEANUP_SCOPES.contains(&scope) {
            return Err(Error::Corrupt(format!("unknown scope: {scope:?}")));
        }
        let bound = crate::backfill::horizon_epoch(range, now);
        let ids = self.mailboxes_in_scope(scope)?;
        let total = self.cleanup_count_groups(&ids, bound)?;
        self.0.execute(
            "INSERT OR REPLACE INTO nettoyage_session
               (id, plage, perimetre, borne_epoch, total, traites)
             VALUES (1, ?1, ?2, ?3, ?4, 0)",
            params![range, scope, bound, total as i64],
        )?;
        Ok(CleanupSession {
            range: range.to_string(),
            scope: scope.to_string(),
            bound_epoch: bound,
            total,
            handled: 0,
        })
    }

    /// The session's remaining groups: a sender × their mail in the
    /// range, the most recent first. Empty without a session.
    pub fn cleanup_groups(&self) -> Result<Vec<CleanupGroup>, Error> {
        let Some(session) = self.cleanup_state()? else {
            return Ok(Vec::new());
        };
        let ids = self.mailboxes_in_scope(&session.scope)?;
        let mut stmt = self.0.prepare(&Self::cleanup_groups_sql(&ids))?;
        let groups = stmt
            .query_map(params![session.bound_epoch], |row| {
                Ok(CleanupGroup {
                    address: row.get(0)?,
                    messages: row.get::<_, i64>(1)? as u64,
                    last_epoch: row.get::<_, Option<i64>>(2)?.unwrap_or(0),
                    who: row.get(3)?,
                    last_subject: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(groups)
    }

    /// The GROUP verdict (D5: the stock AND the future) — the
    /// Screener's gate for the future (routing, exit from waiting,
    /// flags), plus applying the rule to the stock WITHIN THE RANGE:
    /// one action per message in `pending_actions`, WITHIN the
    /// verdict's transaction (E3 pattern — never a crash window
    /// between the mail and the intent), duplicate guard, `trash`
    /// → the server's trash, NEVER a permanent deletion (D4); `spam`
    /// without a resolved folder does NOTHING (never an invented
    /// destination). Returns the number of stock messages handled.
    pub fn cleanup_verdict(
        &mut self,
        address: &str,
        destination: &str,
        rule: Option<&str>,
        epoch: i64,
    ) -> Result<usize, Error> {
        let Some(session) = self.cleanup_state()? else {
            return Err(Error::Corrupt("no cleanup in progress".to_string()));
        };
        validate_routing(destination, rule)?;
        let Some(address) = images_address(Some(address.to_string())) else {
            return Err(Error::InvalidEmailAddress(address.to_string()));
        };
        let ids = self.mailboxes_in_scope(&session.scope)?;
        // Each account's junk folder, resolved BEFORE the transaction
        // (same rule as arrival E3).
        let mut junk: BTreeMap<i64, Option<String>> = BTreeMap::new();
        if destination == "ecarte" && rule == Some("spam") {
            for account in self.accounts()? {
                junk.insert(account.id, self.canonical_folders(account.id)?.junk);
            }
        }
        let mut removals: Vec<(i64, Uid)> = Vec::new();
        let tx = self.0.unchecked_transaction()?;
        if destination == "ecarte"
            && let Some(rule) = rule
        {
            // The stock: THE shared criterion (same definition as the
            // groups and the view), restricted to the address — read
            // BEFORE `set_verdict`, which would take the sender out of
            // the criterion (D7 excludes routed senders).
            let criterion = Self::cleanup_criterion(&ids);
            let stock: Vec<(i64, Uid, i64)> = {
                let mut stmt = tx.prepare(&format!(
                    "SELECT e.mailbox_id, e.uid, m.account_id
                       FROM envelopes e JOIN mailboxes m ON m.id = e.mailbox_id
                      WHERE e.sender_norm = ?2 AND {criterion}"
                ))?;
                stmt.query_map(params![session.bound_epoch, address], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                })?
                .collect::<Result<Vec<_>, _>>()?
            };
            for (mailbox_id, uid, account_id) in stock {
                let action = match rule {
                    "archive" => Some(Action::Archive),
                    "corbeille" => Some(Action::Delete),
                    "spam" => junk.get(&account_id).cloned().flatten().map(Action::MoveTo),
                    _ => None,
                };
                let Some(action) = action else { continue };
                // An action ALREADY queued (a user gesture from a few
                // seconds ago — mark_seen, archiving): we do NOT log
                // ours AND we do NOT remove the local copy (review
                // 2026-08-30: the E3 arrival pattern assumes a NEW
                // message with no possible action; on stock, removing
                // without having set the intent would make cleanup
                // believe a message the server keeps has gone — it
                // would come back on the next poll). The message stays
                // visible, consistent with the server — a stated limit.
                let already = tx
                    .prepare_cached(
                        "SELECT 1 FROM pending_actions WHERE mailbox_id = ?1 AND uid = ?2",
                    )?
                    .exists(params![mailbox_id, uid])?;
                if already {
                    continue;
                }
                tx.prepare_cached(
                    "INSERT INTO pending_actions (mailbox_id, uid, kind) VALUES (?1, ?2, ?3)",
                )?
                .execute(params![mailbox_id, uid, action.to_kind()])?;
                removals.push((mailbox_id, uid));
            }
        }
        set_verdict(&tx, &address, destination, rule, epoch)?;
        tx.execute(
            "UPDATE nettoyage_session SET traites = traites + 1 WHERE id = 1",
            [],
        )?;
        tx.commit()?;
        // The local removal AFTER the commit (E3 pattern): the intent
        // is in the database, a crash here loses nothing — the local
        // copy will go at the next reconciliation. In ONE transaction
        // (review 2026-08-30: a removal per autocommit paid an fsync
        // per message — seconds on a large group, under the commands
        // lock).
        let handled = removals.len();
        if !removals.is_empty() {
            let tx = self.0.unchecked_transaction()?;
            // ONCE per thread touched, never per message
            // (PLAN-AUDIT-V2 E4, the `remove_absent` pattern): a group
            // of N messages from the same sender often lives in a few
            // threads — `remove_local` per message refreshed each one
            // N times.
            let mut touched: BTreeSet<i64> = BTreeSet::new();
            for (mailbox_id, uid) in removals {
                if let Some(thread) = purge_message(&tx, mailbox_id, uid)? {
                    touched.insert(thread);
                }
            }
            for thread in &touched {
                thread::refresh(&tx, *thread)?;
            }
            tx.commit()?;
        }
        Ok(handled)
    }

    /// A group's mail, within the session's range and scope — the
    /// read the sort screen offers when entering a group (view only,
    /// never sort at the message level: the verdict stays at the
    /// group, a scope refusal from the PLAN). The most recent first.
    /// Empty without a session.
    pub fn cleanup_messages(&self, address: &str) -> Result<Vec<UnifiedRow>, Error> {
        let Some(session) = self.cleanup_state()? else {
            return Ok(Vec::new());
        };
        let Some(address) = images_address(Some(address.to_string())) else {
            return Ok(Vec::new());
        };
        let ids = self.mailboxes_in_scope(&session.scope)?;
        // THE shared criterion: a group's view shows exactly what the
        // verdict will process.
        let sql = Self::cleanup_messages_sql(&ids);
        let mut stmt = self.0.prepare(&sql)?;
        let rows = stmt
            .query_map(params![session.bound_epoch, address], row_to_threaded)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Closes the session (the progress is cleared; the verdicts,
    /// though, stay set — they live in the routing).
    pub fn cleanup_finish(&self) -> Result<(), Error> {
        self.0
            .execute("DELETE FROM nettoyage_session WHERE id = 1", [])?;
        Ok(())
    }

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

/// The predicate "this message still awaits its body", shared by the
/// ACCOUNT ([`Store::bodies_pending_count`]) and the working LIST
/// ([`Store::bodies_to_backfill`]).
///
/// ONE piece of writing: the two can no longer diverge — and it is
/// this piece of writing, never a copy, that the plan guard queries
/// (same reason as [`unified_page_sql`], and the same lesson paid
/// for).
///
/// **It reads NO column of `bodies`, and that is the whole point.**
/// The row's existence is decided from the auto-index of the primary
/// key `(mailbox_id, uid)` — so without ever recalling the row,
/// which weighs 56 KB on average in the field. Reading even a single
/// bit cost 251k random reads across 11.4 GB: **20,839 ms cold
/// versus 396 ms without** (measured 2026-08-26 on the field database).
///
/// This predicate used to carry `AND b.scanned = 1` — the trace of
/// bodies fetched BEFORE attachments existed, whose MIME had never
/// been inspected. **Removed 2026-08-26 (PLAN-DEMARRAGE, decision
/// D8)** on three measured facts: production NEVER writes
/// `scanned = 0` ([`Store::save_body_full`] hardcodes a `1`), both
/// fleet workstations carry **zero** rows at `scanned = 0`, and the
/// criterion cost an 8,870 ms startup freeze to protect zero rows. The
/// column survives, vestigial: removing it would require rewriting
/// 11.4 GB — it will leave with whatever job touches `bodies` anyway
/// (the preview, a debt).
///
/// **Requires the alias `e`** for `envelopes` wherever it is used —
/// as [`SELECT_UNIFIED`] requires its own. The fragment is a string:
/// a different alias compiles and fails at `prepare`, on a path where
/// the UI shows nothing (the backfill's `catch` is a
/// `console.error`).
pub(crate) const BODY_ABSENT: &str = "NOT EXISTS (
                   SELECT 1 FROM bodies b
                    WHERE b.mailbox_id = e.mailbox_id AND b.uid = e.uid
               )";

/// The COUNT of missing bodies for a mailbox: `?1` the account, `?2`
/// the mailbox, `?3` the horizon.
pub(crate) fn bodies_pending_count_sql() -> String {
    format!(
        "SELECT COUNT(*)
             FROM envelopes e
             JOIN mailboxes m ON m.id = e.mailbox_id
             WHERE m.account_id = ?1 AND m.name = ?2
               AND (e.date_epoch IS NULL OR e.date_epoch >= ?3)
               AND {BODY_ABSENT}"
    )
}

/// The working LIST of the backfill — same parameters, plus `?4`, the
/// batch bound.
pub(crate) fn bodies_to_backfill_sql() -> String {
    format!(
        "SELECT e.uid
             FROM envelopes e
             JOIN mailboxes m ON m.id = e.mailbox_id
             WHERE m.account_id = ?1 AND m.name = ?2
               AND (e.date_epoch IS NULL OR e.date_epoch >= ?3)
               AND {BODY_ABSENT}
             ORDER BY e.date_epoch DESC, e.uid DESC
             LIMIT ?4"
    )
}

/// The query for a page of the unified mailbox.
///
/// Isolated so a test can query **its own** execution plan, and not a
/// copy that would diverge the day one of the two changes. The cost
/// of this query is the hottest path of the product.
/// `organized` (E2): the ORGANIZED Inbox — the SAME skeleton plus the
/// retention flag, in the EXACT shape of the partial index
/// `idx_threads_date_organise` which then carries sort, filter and
/// pagination (S2-bis: the offset skips index entries, never probed
/// rows). ONE piece of writing for both modes — the E1 review had
/// isolated this query precisely so that no copy would diverge.
pub(crate) fn unified_page_sql(by_account: bool, unread_only: bool, organized: bool) -> String {
    // Pagination (`LIMIT`/`OFFSET`) applies in a subquery on
    // `threads` ALONE, not on the join: `OFFSET` produces then
    // discards each skipped row, so everything computed per row — the
    // triple join and the correlated `EXISTS` on `attachments` from
    // SELECT_UNIFIED — was being paid for the 200,000 rows of a deep
    // jump. Measured (rewrite gate P1, 205,050 conversations):
    // 252.6 ms at offset 200,000, linear growth. With the skeleton in
    // a subquery, the jump only walks the partial index
    // `idx_threads_date_globale` — which carries the COMPLETE sort
    // key (last_epoch DESC, last_uid DESC, account_id) and the filter
    // `inbox_size > 0` — and the joins only run on the `limit`
    // retained rows.
    //
    // The outer ORDER BY re-sorts the retained rows with the same
    // key: it guarantees the final order whatever the join strategy,
    // for the price of sorting `limit` rows.
    // `by_account` adds the `account_id = ?3` filter of nav v2
    // ("Mailboxes" of screen 02): same skeleton, the prefixed index
    // `idx_threads_date (account_id, …)` then carries sort and
    // pagination.
    // `unread_only` is the "Unread" tab of the prototype — filtered
    // HERE, not on the client side: 331 conversations out of 2,929 in
    // the field, a page must only carry what it displays.
    let filter = if by_account {
        " AND account_id = ?3"
    } else {
        ""
    };
    let unread_only_clause = if unread_only { " AND unseen > 0" } else { "" };
    // E5: in organized mode, SET-ASIDE threads leave the flow — they
    // live in the pile (shared exclusion, pins pattern). The classic
    // mode excludes nothing.
    let exclusion = if organized {
        organized_exclusion()
    } else {
        String::new()
    };
    // E4: the INTERNAL order (the one the partial index carries)
    // follows the sections in organized mode — same key as the join
    // tail.
    let sort_clause = if organized {
        "ORDER BY (unseen > 0) DESC, last_epoch DESC, last_uid DESC, account_id"
    } else {
        "ORDER BY last_epoch DESC, last_uid DESC, account_id"
    };
    let tail = unified_join_tail(organized);
    // R4 (PLAN-RETOURS-7, D5): PINNED conversations leave the
    // paginated flow — they are served SEPARATELY, at the top of page
    // 0 (`pinned_unified_scoped`); the list never shows the same
    // message twice. `NOT IN` on the pins subquery: a list
    // materialized once, tiny by construction.
    format!(
        "{SELECT_UNIFIED}{THREAD_AGGREGATE}
         FROM (SELECT account_id, last_mailbox_id, last_uid, last_epoch, size, unseen
                 FROM threads
                WHERE inbox_size > 0{exclusion} AND id NOT IN ({PINNED_THREADS}){filter}{unread_only_clause}
                {sort_clause}
                LIMIT ?1 OFFSET ?2) t{tail}"
    )
}

/// The computation of the `organise_hors` flag of A thread (E2) — THE
/// fragment shared by `thread::refresh` (upkeep) and the migration
/// backfill: one single piece of writing for the rule, never two
/// copies that diverge. `thread_param` designates the thread
/// (parameter or column).
///
/// Golden rule (E2 review) — never lose mail:
/// - a sender routed to a VIEW (feed/paper trail) ejects the thread
///   from ONE message on — the thread lives in its view (mirror of
///   [`fil_route_sql`]), nothing is lost;
/// - a screened-out or waiting sender has NO view: the thread only
///   hides if it is ENTIRELY theirs — a mixed thread (a screened-out
///   intruder replying in a known contact's thread) STAYS in the
///   Inbox.
///
/// First WHEN: both tables empty (mode never used) — two O(1)
/// probes, adopting a legacy database costs nothing.
pub(crate) fn organized_off_sql(thread_param: &str) -> String {
    format!(
        "CASE
           WHEN NOT EXISTS (SELECT 1 FROM routage_expediteurs LIMIT 1)
            AND NOT EXISTS (SELECT 1 FROM portier_attente LIMIT 1) THEN 0
           WHEN EXISTS (
             SELECT 1 FROM envelopes te
               JOIN routage_expediteurs r
                 ON r.address = te.sender_norm
                AND r.destination IN ('kiosque', 'registre')
              WHERE te.thread_id = {thread_param}) THEN 1
           WHEN NOT EXISTS (
             SELECT 1 FROM envelopes o
              WHERE o.thread_id = {thread_param}
                AND NOT EXISTS (SELECT 1 FROM portier_attente pa
                                 WHERE pa.address = o.sender_norm)
                AND NOT EXISTS (SELECT 1 FROM routage_expediteurs re
                                 WHERE re.address = o.sender_norm
                                   AND re.destination = 'ecarte')) THEN 1
           ELSE 0 END"
    )
}

/// The Feed and Paper trail filter (PLAN-MODE-ORGANISE E1, review): a
/// thread belongs to the destination if ANY of its messages comes
/// from a sender routed there — never just the HEAD, which is the
/// last message across all mailboxes: replying to it moves it to
/// Sent and the thread would be ejected from its destination (proven
/// RED). Probed via `idx_envelopes_thread` then the routing PK (spike
/// S2), placed INSIDE the paginated skeleton — never after the LIMIT
/// (short pages, S2 reserve). `sender_norm` (generated column, E2)
/// IS the original `lower(trim(sender_address))` — a single
/// expression, defined once; its divergence from `images_address`
/// (Rust) on non-ASCII remains the assumed E1 limit: a real address
/// is ASCII.
pub(crate) fn thread_route_sql(destination_param: &str) -> String {
    format!(
        "EXISTS (
                   SELECT 1 FROM envelopes te
                     JOIN routage_expediteurs r
                       ON r.address = te.sender_norm
                      AND r.destination = {destination_param}
                    WHERE te.thread_id = threads.id
               )"
    )
}

/// The Feed/Paper trail page — the EXACT skeleton of
/// [`unified_page_sql`] plus [`fil_route_sql`]: same sort, same
/// joins. PINNED threads are NOT excluded (E1 review): their
/// dedicated section only exists in the Inbox — excluding them here
/// would make a pinned thread routed elsewhere disappear from ALL
/// organized views. `?1` limit, `?2` offset, `?3` destination, `?4`
/// account (if `by_account`).
pub(crate) fn routing_page_sql(by_account: bool, unread_only: bool) -> String {
    let filter = if by_account {
        " AND account_id = ?4"
    } else {
        ""
    };
    let unread_only_clause = if unread_only { " AND unseen > 0" } else { "" };
    let thread_route = thread_route_sql("?3");
    let tail = unified_join_tail(false);
    // E5: the Feed and Paper trail are ORGANIZED views — a thread set
    // aside leaves them too (it lives in the pile).
    let out_of_pile = format!(" AND id NOT IN ({SET_ASIDE_THREADS})");
    format!(
        "{SELECT_UNIFIED}{THREAD_AGGREGATE}
         FROM (SELECT account_id, last_mailbox_id, last_uid, last_epoch, size, unseen
                 FROM threads
                WHERE inbox_size > 0
                  AND {thread_route}{out_of_pile}{filter}{unread_only_clause}
                ORDER BY last_epoch DESC, last_uid DESC, account_id
                LIMIT ?1 OFFSET ?2) t{tail}"
    )
}

fn migrate(
    conn: &Connection,
    on_progress: &mut dyn FnMut(AdoptionProgress) -> ControlFlow<()>,
) -> Result<(), Error> {
    migrate_multi_account(conn)?;
    add_missing_columns(
        conn,
        "drafts",
        &[
            ("account_id", "INTEGER NOT NULL DEFAULT 1"),
            ("reply_to_mailbox", "TEXT"),
        ],
    )?;
    // ADR 0010: the scope of grouping becomes explicit. The mailboxes
    // already in the database are INBOX and "Sent" — both included,
    // hence the default of 1. A legacy database therefore keeps
    // exactly the threads it had: the migration changes nothing about
    // what is displayed.
    add_missing_columns(
        conn,
        "mailboxes",
        &[("threaded", "INTEGER NOT NULL DEFAULT 1")],
    )?;
    add_missing_columns(conn, "accounts", &[("sent_mailbox", "TEXT")])?;
    add_missing_columns(conn, "folders", &[("special_use", "TEXT")])?;
    add_missing_columns(conn, "mailboxes", &[("relevee_epoch", "INTEGER")])?;
    add_missing_columns(
        conn,
        "mailboxes",
        &[("remote_total", "INTEGER NOT NULL DEFAULT 0")],
    )?;
    // ADR 0017: the UIDNEXT seen at the last poll — NULL as long as
    // no completed poll has taken place, so a legacy database polls
    // everything on its first cycle (conservative), then becomes
    // frugal.
    add_missing_columns(conn, "mailboxes", &[("remote_uidnext", "INTEGER")])?;
    // PLAN-AUDIT-V1 E3: the quarantine of refused actions.
    add_missing_columns(
        conn,
        "pending_actions",
        &[
            ("attempts", "INTEGER NOT NULL DEFAULT 0"),
            ("refusee", "INTEGER NOT NULL DEFAULT 0"),
            ("last_error", "TEXT"),
        ],
    )?;
    // PLAN-AUDIT-V1 E2: the initialization flag. On a legacy
    // database, ONCE, when the column is added: any mailbox that
    // already has a marker is deemed initialized — rows at 0 keep the
    // previous behavior (first pass = initial).
    if !table_columns(conn, "mailboxes")?.contains("initialisee") {
        add_missing_columns(
            conn,
            "mailboxes",
            &[("initialisee", "INTEGER NOT NULL DEFAULT 0")],
        )?;
        conn.execute(
            "UPDATE mailboxes SET initialisee = 1 WHERE last_uid > 0",
            [],
        )?;
    }
    add_missing_columns(
        conn,
        "outbox",
        &[
            ("account_id", "INTEGER NOT NULL DEFAULT 1"),
            ("refs", "TEXT"),
            ("reply_to", "TEXT"),
        ],
    )?;
    add_missing_columns(
        conn,
        "envelopes",
        &[
            ("sender_address", "TEXT"),
            ("message_id", "TEXT"),
            ("flagged", "INTEGER NOT NULL DEFAULT 0"),
            ("in_reply_to", "TEXT"),
            ("refs", "TEXT"),
            // NULL = "not yet attached". This is what
            // `thread::migrate_threads` looks for, further down.
            ("thread_id", "INTEGER"),
            // R4: recipients arrive NULL on existing rows — the send
            // backfill (D2) populates them, sync now writes them on
            // every new message.
            ("to_addrs", "TEXT"),
            ("cc_addrs", "TEXT"),
            // PLAN-AUDIT-V2 E5: the envelope's Reply-To. Field STOP 2
            // (2026-09-02): the column lived only in the CREATE
            // TABLE — "no column named reply_to" on every watcher
            // pass over a database from before wave 2. NULL on
            // existing rows: the poll writes it on every new or
            // resynced message.
            ("reply_to", "TEXT"),
        ],
    )?;
    add_missing_columns(
        conn,
        "drafts",
        &[
            ("remote_uid", "INTEGER"),
            ("pushed_epoch", "INTEGER"),
            // Cc/Bcc of a draft — empty on existing rows
            // (PLAN-RETOURS-2).
            ("cc_raw", "TEXT NOT NULL DEFAULT ''"),
            ("bcc_raw", "TEXT NOT NULL DEFAULT ''"),
            // Rich body — NULL on existing rows, plain-text path
            // intact (PLAN-COMPOSITION-HTML).
            ("body_html", "TEXT"),
        ],
    )?;
    // Cc/Bcc of the send log — empty on existing rows
    // (PLAN-RETOURS-2).
    add_missing_columns(
        conn,
        "outbox",
        &[
            ("cc_addrs", "TEXT NOT NULL DEFAULT ''"),
            ("bcc_addrs", "TEXT NOT NULL DEFAULT ''"),
            // Rich body — NULL on existing rows
            // (PLAN-COMPOSITION-HTML).
            ("body_html", "TEXT"),
        ],
    )?;
    // Bodies already in the database are worth 0: they predate
    // attachments, and the backfill will need to reread them once.
    add_missing_columns(conn, "bodies", &[("scanned", "INTEGER NOT NULL DEFAULT 0")])?;
    // Echo recipients — NULL on existing rows (PLAN-RETOURS-5).
    add_missing_columns(conn, "echos", &[("to_addrs", "TEXT")])?;
    // "Important" and delayed sending (PLAN-RETOURS-6): existing rows
    // are neither flagged nor scheduled.
    add_missing_columns(
        conn,
        "drafts",
        &[("important", "INTEGER NOT NULL DEFAULT 0")],
    )?;
    add_missing_columns(
        conn,
        "outbox",
        &[
            ("important", "INTEGER NOT NULL DEFAULT 0"),
            ("send_at_epoch", "INTEGER"),
        ],
    )?;
    // iTIP reply (PLAN-INVITATIONS) — NULL on existing rows,
    // historical send path unchanged.
    add_missing_columns(conn, "outbox", &[("ics_reply", "TEXT")])?;
    // The cross-cancellation link (field R6) — databases born during
    // the job have the table without the column.
    add_missing_columns(
        conn,
        "invitations",
        &[("annule", "INTEGER NOT NULL DEFAULT 0")],
    )?;
    // The list preview (rewrite screen 02) is computed at the WRITE
    // of the body; earlier bodies backfill it IN BATCHES
    // (`preview_catchup`, called by the shell as polling proceeds) —
    // never on the opening path nor while scrolling. The partial
    // index makes the "any stragglers?" probe free once the pass is
    // closed out.
    add_missing_columns(conn, "bodies", &[("preview", "TEXT")])?;
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_bodies_apercu_manquant
             ON bodies(mailbox_id, uid) WHERE preview IS NULL;",
    )?;
    // The envelopes date index gains `uid` (see the SCHEMA comment).
    // `CREATE INDEX IF NOT EXISTS` is NOT enough: on an existing
    // database the index already carries this name, the creation is
    // a silent no-op and the defect would survive. So its DEFINITION
    // is read and it is rebuilt if it lacks the column — same pattern
    // as the `recipients` probe of the search index.
    //
    // No freeze: the rebuild only reads `envelopes` (47 MB in the
    // field), never the bodies — 0.332 s measured on the CE's
    // database, versus the 18 s an index on `bodies` would have cost.
    // That is the whole difference between an acceptable silent
    // migration and the 2026-08-17 freeze.
    //
    // The reread and the rebuild live in ONE transaction, and this is
    // not caution for its own sake (fresh-eyes review of 2026-08-26):
    // `connect_accounts` calls `Store::open` DIRECTLY, outside the
    // commands' global lock (commands.rs), so two `migrate()` calls
    // really do run in parallel at startup. Without a transaction,
    // both would read the two-column index before either writes, and
    // rebuild it each in turn: ~3.5 s of freeze instead of 1.77 s.
    // `BEGIN IMMEDIATE` takes the write lock as soon as it reads —
    // the second one to arrive rereads AFTER the first, finds `uid`,
    // and does nothing.
    // DOUBLE CHECK, and the first check matters as much as the
    // second: `migrate()` runs on EVERY `Store::open`, so dozens of
    // times per startup. A bare read of `sqlite_master` takes no
    // lock; opening a write transaction just to check would cost the
    // write lock on every command.
    rebuild_index_if_old(
        conn,
        "idx_envelopes_date",
        "uid",
        "CREATE INDEX idx_envelopes_date ON envelopes(mailbox_id, date_epoch DESC, uid);",
    )?;
    // The full-messages exclusion probe (nav, Archive category on
    // Gmail) looks up by message_id: without this index, every row
    // of "All messages" would pay for a table scan.
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_envelopes_message
             ON envelopes(message_id) WHERE message_id IS NOT NULL;",
    )?;
    // Repair of previews extracted by the first decoder, which let
    // numeric entities (&#233;) and named ones (&eacute;, &zwnj;…)
    // slip through — a defect seen in the field. Setting back to NULL
    // is enough: the batch backfill recomputes them with the full
    // decoder, off the opening path. The criterion is THE decoder's
    // own scanner (not an approximate SQL pattern). ONE single pass,
    // held by a marker: a double-encoded body ("&amp;gt;")
    // legitimately produces "&gt;" in the new preview — without the
    // marker, the repair would reset it to NULL on every open, for
    // nothing.
    conn.execute_batch("CREATE TABLE IF NOT EXISTS reparations (nom TEXT PRIMARY KEY);")?;
    let already_done: bool = conn
        .prepare("SELECT 1 FROM reparations WHERE nom = 'apercus-entites'")?
        .exists([])?;
    if !already_done {
        let mut stmt = conn.prepare(
            "SELECT mailbox_id, uid, preview FROM bodies
                 WHERE preview IS NOT NULL AND preview LIKE '%&%'",
        )?;
        let polluted: Vec<(i64, u32)> = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, u32>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .filter_map(Result::ok)
            .filter(|(_, _, p)| crate::body::contains_residual_entity(p))
            .map(|(m, u, _)| (m, u))
            .collect();
        drop(stmt);
        for (mailbox_id, uid) in polluted {
            conn.execute(
                "UPDATE bodies SET preview = NULL WHERE mailbox_id = ?1 AND uid = ?2",
                params![mailbox_id, uid],
            )?;
        }
        conn.execute_batch("INSERT INTO reparations (nom) VALUES ('apercus-entites');")?;
    }
    // Repair of bodies mangled during decoding — a defect seen in the
    // field (25 bodies in the measurement database). Two causes,
    // fixed on the mail-imap side: multi-byte charsets (gb2312…)
    // required the `full_encoding` feature of mail-parser, and a
    // missing charset fell back to UTF-8 with replacement instead of
    // the actual windows-1252. Deleting the row is enough: the
    // backfill (`bodies_to_backfill`) redownloads any message without
    // a body, and `save_body` redoes the preview, the search index
    // and the attachments along the way. Genuine U+FFFD characters
    // (sent as such) will come back identical — that's a pointless
    // redownload, but only ONCE, held by the marker.
    let already_done: bool = conn
        .prepare("SELECT 1 FROM reparations WHERE nom = 'corps-fffd'")?
        .exists([])?;
    if !already_done {
        conn.execute_batch(
            "DELETE FROM bodies WHERE html LIKE '%' || char(65533) || '%';
             INSERT INTO reparations (nom) VALUES ('corps-fffd');",
        )?;
    }
    // Repair of messages with a calendar part scanned BEFORE
    // PLAN-INVITATIONS. Two reasons, one remedy: (1) the
    // `est_calendrier_inline` filter (mail-imap) changed the
    // numbering of parts — the stored `idx` values counted the
    // calendar part, rereading the bytes no longer counts it:
    // clicking an attachment would silently serve the WRONG file;
    // (2) these messages have no `invitations` row — their card must
    // be born (adoption, invariant §6.7). Deleting both the body AND
    // the attachment rows is enough: the backfill
    // (`bodies_to_backfill`) rereads the message, and
    // `save_body_full` redoes attachments (fresh indices), preview,
    // search index and invitation all at once. ONCE, held by the
    // marker.
    let already_done: bool = conn
        .prepare("SELECT 1 FROM reparations WHERE nom = 'pieces-calendrier'")?
        .exists([])?;
    if !already_done {
        conn.execute_batch(
            "CREATE TEMP TABLE reparation_calendrier AS
                 SELECT DISTINCT mailbox_id, uid FROM attachments
                 WHERE mime IN ('text/calendar', 'application/ics')
                    OR LOWER(name) LIKE '%.ics';
             DELETE FROM bodies WHERE (mailbox_id, uid) IN
                 (SELECT mailbox_id, uid FROM reparation_calendrier);
             DELETE FROM attachments WHERE (mailbox_id, uid) IN
                 (SELECT mailbox_id, uid FROM reparation_calendrier);
             DROP TABLE reparation_calendrier;
             INSERT INTO reparations (nom) VALUES ('pieces-calendrier');",
        )?;
    }
    // R2 (PLAN-RETOURS-MAIL): envelopes synced BEFORE the fix carry
    // the backslash-escapes of IMAP `quoted-string`s that
    // `imap-proto` leaves in the content (subject `Test \"Envoyés\"`,
    // sender name, address). The new decoding strips them at sync
    // time, but existing rows stay tainted: repaired ONCE. The
    // stored content is already RFC 2047-decoded; only the IMAP
    // escape layer remains, so un-escaping the stored value is
    // equivalent to the new decoding (an encoded-word carries neither
    // `"` nor `\`). The FTS index does not need to move: its
    // tokenizer already discards the backslash, search gave the same
    // results. char(92) = `\`.
    let already_done: bool = conn
        .prepare("SELECT 1 FROM reparations WHERE nom = 'objets-escapes'")?
        .exists([])?;
    if !already_done {
        let mut stmt = conn.prepare(
            "SELECT mailbox_id, uid, subject, sender, sender_address FROM envelopes
                 WHERE instr(subject, char(92)) > 0
                    OR instr(sender, char(92)) > 0
                    OR instr(sender_address, char(92)) > 0",
        )?;
        #[allow(clippy::type_complexity)]
        let tainted: Vec<(i64, u32, Option<String>, Option<String>, Option<String>)> = stmt
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })?
            .filter_map(Result::ok)
            .collect();
        drop(stmt);
        for (mailbox_id, uid, subject, sender, sender_address) in tainted {
            let clean =
                |v: Option<String>| v.map(|s| crate::unescape_imap_quoted_str(&s).into_owned());
            conn.execute(
                "UPDATE envelopes SET subject = ?3, sender = ?4, sender_address = ?5
                     WHERE mailbox_id = ?1 AND uid = ?2",
                params![
                    mailbox_id,
                    uid,
                    clean(subject),
                    clean(sender),
                    clean(sender_address),
                ],
            )?;
        }
        conn.execute_batch("INSERT INTO reparations (nom) VALUES ('objets-escapes');")?;
    }
    add_missing_columns(
        conn,
        "accounts",
        &[
            ("imap_host", "TEXT"),
            ("imap_port", "INTEGER"),
            ("smtp_host", "TEXT"),
            ("smtp_port", "INTEGER"),
            ("username", "TEXT"),
        ],
    )?;
    search::migrate_search(conn, on_progress)?;
    // The index comes AFTER `add_missing_columns`, not in `SCHEMA`:
    // on a legacy database, `CREATE TABLE IF NOT EXISTS envelopes`
    // does nothing and the `thread_id` column does not yet exist at
    // the moment the schema runs. Two migration tests proved it.
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_envelopes_thread
             ON envelopes(thread_id, date_epoch DESC);",
    )?;
    // The NORMALIZED sender address, as a generated column (Organized
    // mode E2, spike S2-bis): SQLite only uses an EXPRESSION index
    // against a literal — in a join (`= r.address`), it scans (2.3 s
    // measured at 200k). The VIRTUAL column stores nothing (ALTER
    // 14 ms); the real index (188 ms at 200k, once) makes SEARCH out
    // of every sender probe of routing and the Screener. Same
    // expression as `fil_route_sql` — known divergence with
    // `images_address` (Rust) on non-ASCII, assumed E1 limit: a real
    // address is ASCII.
    add_missing_columns(
        conn,
        "envelopes",
        &[(
            "sender_norm",
            "TEXT GENERATED ALWAYS AS (lower(trim(sender_address))) VIRTUAL",
        )],
    )?;
    // Three columns (PLAN-AUDIT-V2 E4): the Cleanup aggregate is
    // COVERED — sender, date, mailbox — without reading a single
    // table row; sender probes (Screener, storing a verdict) are
    // still served by its prefix. One fleet database carried the
    // two-column index: rebuilt, same pattern as the date index.
    let creation =
        format!("CREATE INDEX {SENDERS_INDEX} ON envelopes(sender_norm, date_epoch, mailbox_id);");
    conn.execute_batch(&creation.replace("CREATE INDEX", "CREATE INDEX IF NOT EXISTS"))?;
    rebuild_index_if_old(conn, SENDERS_INDEX, "mailbox_id", &creation)?;
    // The thread retention flag (E2, S2-bis verdict: V4 — maintained
    // by `thread::refresh` like `size`/`unseen`, served by the mirror
    // partial index). On a legacy database, `threads` already exists
    // without the column — and its partial index, created by
    // `thread::SCHEMA` AFTER this point, would fail without it: this
    // is the documented `drop_if_outdated` trap. A fresh database
    // does not have the table yet: the thread schema creates it
    // complete.
    // E4: the Organized Inbox index gains the SECTIONS in its key —
    // an E2 index (without the `unseen` expression) would no longer
    // carry the sort and every page would pay for a materialized sort
    // (S1: 548 ms). Same pattern as the idx_envelopes_date rebuild:
    // the name is not enough, the DEFINITION is read. The thread
    // schema (applied afterward) recreates the new shape.
    let organized_sql: Option<String> = conn
        .query_row(
            "SELECT sql FROM sqlite_master
              WHERE type = 'index' AND name = 'idx_threads_date_organise'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if organized_sql.is_some_and(|sql| !sql.contains("unseen")) {
        conn.execute_batch("DROP INDEX idx_threads_date_organise;")?;
    }
    let thread_columns = table_columns(conn, "threads")?;
    if thread_columns.contains("id") && !thread_columns.contains("organise_hors") {
        add_missing_columns(
            conn,
            "threads",
            &[("organise_hors", "INTEGER NOT NULL DEFAULT 0")],
        )?;
        // ONE-TIME backfill for a database from BEFORE E2 where the
        // mode has already been used (E1 field finding: the epoch
        // may have been recorded and unknowns may have arrived
        // BEFORE this update — without a backfill they would pass
        // the desk forever, silently). First the pending state (the
        // definition of arrival, replayed on the stock: 21 ms
        // measured at 200k), then the flags of affected threads,
        // through THE shared fragment — never a copy of the rule.
        let epoch: Option<i64> = conn
            .query_row(
                "SELECT value FROM prefs WHERE key = 'mode_organise_epoch'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .and_then(|v| v.parse().ok());
        if let Some(epoch) = epoch {
            conn.execute(
                "INSERT OR IGNORE INTO portier_attente (address)
                 SELECT e.sender_norm FROM envelopes e
                   JOIN mailboxes m ON m.id = e.mailbox_id AND m.name = ?2
                  WHERE (e.date_epoch > ?1 OR e.date_epoch IS NULL)
                    AND e.sender_norm IS NOT NULL
                  GROUP BY e.sender_norm
                 HAVING NOT EXISTS (SELECT 1 FROM routage_expediteurs r
                                     WHERE r.address = e.sender_norm)
                    AND NOT EXISTS (SELECT 1 FROM envelopes v
                                     WHERE v.sender_norm = e.sender_norm
                                       AND v.date_epoch <= ?1)
                    AND NOT EXISTS (SELECT 1 FROM accounts a
                                     WHERE lower(trim(a.email)) = e.sender_norm)",
                params![epoch, thread::RECEIVED_MAILBOX],
            )?;
        }
        conn.execute(
            &format!(
                "UPDATE threads SET organise_hors = {}
                  WHERE id IN (
                    SELECT DISTINCT te.thread_id FROM envelopes te
                     WHERE te.thread_id IS NOT NULL
                       AND (EXISTS (SELECT 1 FROM routage_expediteurs r
                                     WHERE r.address = te.sender_norm)
                            OR EXISTS (SELECT 1 FROM portier_attente pa
                                        WHERE pa.address = te.sender_norm)))",
                organized_off_sql("threads.id")
            ),
            [],
        )?;
    }
    // Thread adoption does NOT live here: it belongs to the
    // transactional unit of `init_with`, to be rewindable (§8). It
    // comes after this module — the column and the index must exist
    // before adopting legacy messages.
    Ok(())
}

/// Phase 2 → 3 switchover: the constraints of three tables change
/// (UNIQUE and per-account keys) — SQLite requires a rebuild.
/// Existing data is adopted by a "pending" account (empty email) that
/// the first connection will claim: in practice, the same Gmail
/// account as before the update. Zero loss, proven by test.
fn migrate_multi_account(conn: &Connection) -> Result<(), Error> {
    if table_columns(conn, "mailboxes")?.contains("account_id") {
        return Ok(());
    }
    conn.execute_batch(
        "PRAGMA foreign_keys = OFF;
         BEGIN;
         INSERT INTO accounts (id, email, provider) VALUES (1, '', 'gmail');

         CREATE TABLE mailboxes_v3 (
             id             INTEGER PRIMARY KEY,
             account_id     INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
             name           TEXT NOT NULL,
             uid_validity   INTEGER NOT NULL,
             last_uid       INTEGER NOT NULL DEFAULT 0,
             highest_modseq INTEGER,
             UNIQUE (account_id, name)
         );
         INSERT INTO mailboxes_v3 (id, account_id, name, uid_validity, last_uid, highest_modseq)
             SELECT id, 1, name, uid_validity, last_uid, highest_modseq FROM mailboxes;
         DROP TABLE mailboxes;
         ALTER TABLE mailboxes_v3 RENAME TO mailboxes;

         CREATE TABLE drafts_remote_v3 (
             account_id   INTEGER PRIMARY KEY,
             uid_validity INTEGER NOT NULL
         );
         INSERT INTO drafts_remote_v3 (account_id, uid_validity)
             SELECT 1, uid_validity FROM drafts_remote;
         DROP TABLE drafts_remote;
         ALTER TABLE drafts_remote_v3 RENAME TO drafts_remote;

         CREATE TABLE draft_tombstones_v3 (
             account_id INTEGER NOT NULL,
             remote_uid INTEGER NOT NULL,
             PRIMARY KEY (account_id, remote_uid)
         );
         INSERT INTO draft_tombstones_v3 (account_id, remote_uid)
             SELECT 1, remote_uid FROM draft_tombstones;
         DROP TABLE draft_tombstones;
         ALTER TABLE draft_tombstones_v3 RENAME TO draft_tombstones;

         COMMIT;
         PRAGMA foreign_keys = ON;",
    )?;
    Ok(())
}

/// The senders index (sender, date, mailbox) — named ONCE: Cleanup
/// queries require it via `INDEXED BY` (review: four copies of the
/// name, a rename would have silently missed one).
pub(crate) const SENDERS_INDEX: &str = "idx_envelopes_sender";

/// The fields of an envelope that live in the search index — as
/// reread from the database, to know whether a resync has changed
/// them (subject, sender, address, recipients, cc).
type IndexedFields = (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

/// The path of a connection to a FILE — `None` for an in-memory
/// database (SQLite answers an empty name), which never registers
/// itself.
fn file_key(conn: &Connection) -> Option<std::path::PathBuf> {
    conn.path()
        .filter(|path| !path.is_empty())
        .map(std::path::PathBuf::from)
}

/// The registry of paths whose full initialization has SUCCEEDED in
/// this process (PLAN-AUDIT-V2 E1). A poisoned lock is recovered:
/// losing the registry would replay the migrations, never skip them.
struct InitializedRegistry(std::sync::Mutex<HashSet<std::path::PathBuf>>);

impl InitializedRegistry {
    fn contains(&self, key: &std::path::Path) -> bool {
        self.lock().contains(key)
    }

    fn insert(&self, key: std::path::PathBuf) {
        self.lock().insert(key);
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashSet<std::path::PathBuf>> {
        match self.0.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

fn initialized_registry() -> &'static InitializedRegistry {
    static REGISTRY: std::sync::OnceLock<InitializedRegistry> = std::sync::OnceLock::new();
    REGISTRY.get_or_init(|| InitializedRegistry(std::sync::Mutex::new(HashSet::new())))
}

/// Rebuilds an index whose definition in the database does not yet
/// carry `marker` (a column added after the fact). DOUBLE CHECK, and
/// the first check matters as much as the second: a bare read of
/// `sqlite_master` takes no lock; then, under `BEGIN IMMEDIATE`, a
/// reread — two `migrate()` calls can run in parallel at startup
/// (`connect_accounts` opens outside the commands' lock): the second
/// one to arrive rereads AFTER the first, finds the marker, and does
/// nothing.
fn rebuild_index_if_old(
    conn: &Connection,
    name: &str,
    marker: &str,
    creation: &str,
) -> Result<(), Error> {
    let definition = |conn: &Connection| -> Result<Option<String>, Error> {
        Ok(conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'index' AND name = ?1",
                [name],
                |row| row.get(0),
            )
            .optional()?)
    };
    let outdated = |sql: Option<String>| sql.is_some_and(|sql| !sql.contains(marker));
    if !outdated(definition(conn)?) {
        return Ok(());
    }
    conn.execute_batch("BEGIN IMMEDIATE")?;
    let work = (|| -> Result<(), Error> {
        if outdated(definition(conn)?) {
            conn.execute_batch(&format!("DROP INDEX {name}; {creation}"))?;
        }
        Ok(())
    })();
    match work {
        Ok(()) => conn.execute_batch("COMMIT")?,
        Err(err) => {
            // A rollback failure would teach nothing more than the
            // original error — same choice as in the thread unit.
            let _ = conn.execute_batch("ROLLBACK");
            return Err(err);
        }
    }
    Ok(())
}

fn table_columns(conn: &Connection, table: &str) -> Result<HashSet<String>, Error> {
    // `table_xinfo`, not `table_info`: the latter HIDES generated
    // columns (`sender_norm`) — the existence probe would recreate
    // them on every reopen, "duplicate column name" (proven red at
    // E2).
    let mut stmt = conn.prepare(&format!("PRAGMA table_xinfo({table})"))?;
    let columns = stmt
        .query_map([], |row| row.get(1))?
        .collect::<Result<_, _>>()?;
    Ok(columns)
}

fn add_missing_columns(
    conn: &Connection,
    table: &str,
    columns: &[(&str, &str)],
) -> Result<(), Error> {
    let existing = table_columns(conn, table)?;
    for (column, ddl) in columns {
        if !existing.contains(*column) {
            conn.execute(
                &format!("ALTER TABLE {table} ADD COLUMN {column} {ddl}"),
                [],
            )?;
        }
    }
    Ok(())
}

/// Recipients stored on one row — one per `\n`, NULL when empty (R4).
/// `join`/`split` are reciprocal; an address never contains a line
/// break (it is `mailbox@host`).
/// The addresses an envelope carries (sender, To, Cc) — never thread
/// identifiers, even in angle brackets (PLAN-AUDIT-V2 E5).
fn addresses_from(envelope: &Envelope) -> Vec<String> {
    let mut addresses: Vec<String> = Vec::new();
    addresses.extend(envelope.sender_address.clone());
    addresses.extend(envelope.to_addrs.iter().cloned());
    addresses.extend(envelope.cc_addrs.iter().cloned());
    addresses
}

fn join_addrs(addrs: &[String]) -> Option<String> {
    if addrs.is_empty() {
        None
    } else {
        Some(addrs.join("\n"))
    }
}

fn split_addrs(raw: Option<String>) -> Vec<String> {
    raw.map(|s| {
        s.split('\n')
            .filter(|part| !part.is_empty())
            .map(str::to_string)
            .collect()
    })
    .unwrap_or_default()
}

/// Mapping shared by every envelope read — the column order is that
/// of the SELECTs above (`to_addrs`/`cc_addrs` at the tail, index
/// 9/10).
/// THE SINGLE authority for normalizing an address for the image
/// memory (R1, PLAN-RETOURS-11): Unicode lowercase on the Rust side —
/// writing (`allow_images_sender_of`, `revoke_images_sender`) and
/// reading (`images_allowed`) all go through here.
fn images_address(adresse: Option<String>) -> Option<String> {
    adresse
        .map(|a| a.trim().to_lowercase())
        .filter(|a| !a.is_empty())
}

/// The CLOSED vocabularies of routing (PLAN-MODE-ORGANISE E1) — the
/// same table serves Rust validation and, as a belt, the schema's
/// CHECK constraints. `ecarte` is the only destination that accepts a
/// rule.
const ROUTING_DESTINATIONS: [&str; 4] = ["reception", "kiosque", "registre", "ecarte"];
const ROUTING_RULES: [&str; 3] = ["spam", "archive", "corbeille"];

/// The `prefs` keys of Organized mode — the state, and the retention
/// bound of the Screener (first activation, never rewritten).
const PREF_ORGANIZED_MODE: &str = "mode_organise";
const PREF_ORGANIZED_MODE_EPOCH: &str = "mode_organise_epoch";

/// RETOURS-13 R5/R9 — the Screener buttons' defaults: Yes takes a
/// destination (never `ecarte`), No a rule from the screened-out
/// vocabulary or bare `ecarte` ("screen out without moving").
/// DERIVED from the routing tables — never a second copy of the
/// vocabulary (review: a destination added to ROUTING_DESTINATIONS
/// would have left the Settings selector silently refusing it).
const PREF_SCREENER_DEFAULT_YES: &str = "portier_defaut_oui";
const PREF_SCREENER_DEFAULT_NO: &str = "portier_defaut_non";
fn valid_screener_default_yes(v: &str) -> bool {
    v != "ecarte" && ROUTING_DESTINATIONS.contains(&v)
}
fn valid_screener_default_no(v: &str) -> bool {
    v == "ecarte" || ROUTING_RULES.contains(&v)
}

/// THE single validation gate for the routing vocabulary — called
/// before every write AND before every address resolution (a holed
/// vocabulary never hides behind another refusal).
fn validate_routing(destination: &str, rule: Option<&str>) -> Result<(), Error> {
    if !ROUTING_DESTINATIONS.contains(&destination) {
        return Err(Error::InvalidRouting(format!(
            "unknown destination: {destination:?}"
        )));
    }
    if let Some(r) = rule {
        if destination != "ecarte" {
            return Err(Error::InvalidRouting(format!(
                "a No rule requires a screened-out sender, not {destination:?}"
            )));
        }
        if !ROUTING_RULES.contains(&r) {
            return Err(Error::InvalidRouting(format!("unknown rule: {r:?}")));
        }
    }
    Ok(())
}

/// The Screener's verdict on a sender — a row of
/// `routage_expediteurs`, as history shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Routing {
    pub address: String,
    pub destination: String,
    pub rule: Option<String>,
    pub epoch: i64,
}

fn read_routing(row: &rusqlite::Row<'_>) -> rusqlite::Result<Routing> {
    Ok(Routing {
        address: row.get(0)?,
        destination: row.get(1)?,
        rule: row.get(2)?,
        epoch: row.get(3)?,
    })
}

/// A rank of the Screener's desk (E2): the WAITING address —
/// normalized, the key the verdict will take — and its last message.
#[derive(Debug)]
pub struct ScreenerRank {
    pub address: String,
    pub row: UnifiedRow,
}

/// The threads of ONE sender — THE single definition, shared by the
/// verdict recompute and the pending-state clearing of the sync path.
fn threads_of(conn: &Connection, address: &str) -> Result<Vec<i64>, Error> {
    let threads = conn
        .prepare_cached(
            "SELECT DISTINCT thread_id FROM envelopes
              WHERE sender_norm = ?1 AND thread_id IS NOT NULL",
        )?
        .query_map(params![address], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(threads)
}

/// The transactional CORE of the verdict — THE single gate, shared by
/// [`Store::router_expediteur`] (Screener, "Move to…") and
/// [`Store::nettoyage_verdict`] (2026-08-30 review: Cleanup was
/// recopying the body; a future addition to "set a verdict" would
/// have diverged depending on the originating screen). The caller
/// validates the vocabulary and normalizes the address BEFOREHAND.
fn set_verdict(
    tx: &Connection,
    address: &str,
    destination: &str,
    rule: Option<&str>,
    epoch: i64,
) -> Result<(), Error> {
    tx.execute(
        "INSERT OR REPLACE INTO routage_expediteurs (address, destination, regle, epoch)
         VALUES (?1, ?2, ?3, ?4)",
        params![address, destination, rule, epoch],
    )?;
    // RETOURS-14 R8 (field finding 2026-08-31): a YES means trust —
    // the verdict ALSO sets the rule "always show images from this
    // sender" (same table, same normalization as the R1 guard;
    // revocable in Settings > Display). A No does not touch the
    // guard — it has its own way out.
    if destination != "ecarte" {
        tx.execute(
            "INSERT OR REPLACE INTO images_expediteurs (address, epoch) VALUES (?1, ?2)",
            params![address, epoch],
        )?;
    }
    // The verdict takes over from the pending state — Yes as much as
    // No.
    tx.execute(
        "DELETE FROM portier_attente WHERE address = ?1",
        params![address],
    )?;
    refresh_threads_of(tx, address)
}

/// Recomputes the flags of ONE sender's threads through THE single
/// gate (`thread::refresh`) — after a verdict or a reinstatement.
/// Bounded to the address's threads (63 ms measured on a sender with
/// 10,000 threads, single gesture).
fn refresh_threads_of(conn: &Connection, address: &str) -> Result<(), Error> {
    for thread in threads_of(conn, address)? {
        thread::refresh(conn, thread)?;
    }
    Ok(())
}

/// Is the address one of OUR accounts? Never oneself at the Screener
/// (E1 lesson: the user's own address is never a sender to sort).
/// `prepare_cached`: the probe lives on the hot path of sync (E2
/// review).
fn account_address(conn: &Connection, address: &str) -> Result<bool, Error> {
    Ok(conn
        .prepare_cached("SELECT 1 FROM accounts WHERE lower(trim(email)) = ?1")?
        .exists(params![address])?)
}

/// Does the sender have mail PRIOR to the activation epoch? This is
/// THE definition of "known" from D3 (arrivals only) — one single
/// piece of writing, shared by the arrival decision and the
/// reinstatement: two copies would diverge on the very meaning of the
/// desk. Across all mailboxes: history in Archive or Junk is still
/// history.
fn known_before_epoch(conn: &Connection, address: &str, epoch: i64) -> Result<bool, Error> {
    Ok(conn
        .prepare_cached(
            "SELECT 1 FROM envelopes
              WHERE sender_norm = ?1 AND date_epoch <= ?2 LIMIT 1",
        )?
        .exists(params![address, epoch])?)
}

/// Purges the Screener's ranks that no longer rest on ANY mail (E2):
/// the pending state is DERIVED — a recycled UID inherits no decision
/// (A43/A89). Shared by account removal and mailbox reset.
/// THE list of "per message" tables, for the three purges
/// (`remove_local`, `remove_absent`, `reset_mailbox`) —
/// PLAN-AUDIT-V1 E4. Before: three diverging copies, `remove_absent`
/// was missing five. Pending actions are NOT in the list: depending
/// on the purge, they carry the gesture (`remove_local`) or are
/// unrealizable (`remove_absent`, `reset_mailbox` — which removes
/// them separately).
pub(crate) const TABLES_PER_MESSAGE: [&str; 7] = [
    "bodies",
    "invitations",
    "attachments",
    "images_messages",
    "mis_de_cote",
    "kiosque_lus",
    "envelopes",
];

/// Purges ONE message from all its tables and returns its thread,
/// READ BEFORE the deletion (after, the link is lost) — without
/// refreshing it: it is the caller who refreshes, ONCE per affected
/// thread (review PLAN-AUDIT-V1: a refresh per message cost ~500× on
/// a thread of 500 vanished messages).
pub(crate) fn purge_message(
    conn: &Connection,
    mailbox_id: i64,
    uid: Uid,
) -> Result<Option<thread::ThreadId>, Error> {
    let thread = thread::thread_of(conn, mailbox_id, uid)?;
    search::deindex_message(conn, mailbox_id, uid)?;
    for table in TABLES_PER_MESSAGE {
        conn.execute(
            &format!("DELETE FROM {table} WHERE mailbox_id = ?1 AND uid = ?2"),
            params![mailbox_id, uid],
        )?;
    }
    Ok(thread)
}

/// The refused actions of a message (quarantine E3): a fresh gesture
/// from the user replaces them.
fn forget_refused(conn: &Connection, mailbox_id: i64, uid: Uid) -> Result<(), Error> {
    conn.execute(
        "DELETE FROM pending_actions WHERE mailbox_id = ?1 AND uid = ?2 AND refusee = 1",
        params![mailbox_id, uid],
    )?;
    Ok(())
}

fn purge_orphan_pending(conn: &Connection) -> Result<(), Error> {
    conn.execute(
        "DELETE FROM portier_attente WHERE NOT EXISTS (
             SELECT 1 FROM envelopes e WHERE e.sender_norm = portier_attente.address)",
        [],
    )?;
    Ok(())
}

fn row_to_envelope(row: &rusqlite::Row<'_>) -> rusqlite::Result<Envelope> {
    Ok(Envelope {
        reply_to: None,
        uid: row.get(0)?,
        subject: row.get(1)?,
        sender: row.get(2)?,
        sender_address: row.get(3)?,
        message_id: row.get(4)?,
        date: row
            .get::<_, Option<i64>>(5)?
            .and_then(|epoch| DateTime::from_timestamp(epoch, 0)),
        seen: row.get(6)?,
        flagged: row.get(7)?,
        in_reply_to: row.get(8)?,
        to_addrs: split_addrs(row.get(9)?),
        cc_addrs: split_addrs(row.get(10)?),
    })
}

/// Mapping shared by reads of the unified mailbox — the column order
/// is that of [`SELECT_UNIFIED`].
pub(crate) fn row_to_unified(row: &rusqlite::Row<'_>) -> rusqlite::Result<UnifiedRow> {
    let attachment_count = row.get::<_, i64>(10)?.max(0) as u32;
    Ok(UnifiedRow {
        account_id: row.get(0)?,
        account_email: row.get(1)?,
        envelope: Envelope {
            reply_to: None,
            uid: row.get(2)?,
            subject: row.get(3)?,
            sender: row.get(4)?,
            sender_address: row.get(5)?,
            message_id: row.get(6)?,
            date: row
                .get::<_, Option<i64>>(7)?
                .and_then(|epoch| DateTime::from_timestamp(epoch, 0)),
            seen: row.get(8)?,
            flagged: row.get(9)?,
            in_reply_to: row.get(12)?,
            to_addrs: split_addrs(row.get(15)?),
            cc_addrs: split_addrs(row.get(16)?),
        },
        mailbox: row.get(13)?,
        has_attachment: attachment_count > 0,
        attachment_count,
        preview: row.get(14)?,
        thread_id: row.get(11)?,
        // Values for a message seen ALONE — this is the case for
        // search, which does not join `threads`. The grouped list
        // overwrites them with the real aggregate via
        // [`row_to_threaded`].
        thread_size: 1,
        thread_unseen: u32::from(!row.get::<_, bool>(8)?),
        // Set by the PAGE pass (`enrichir_lignes`), never here.
        invitation: None,
    })
}

/// Mapping for the grouped list: the unified columns, then the thread
/// aggregate added by [`THREAD_AGGREGATE`].
pub(crate) fn row_to_threaded(row: &rusqlite::Row<'_>) -> rusqlite::Result<UnifiedRow> {
    Ok(UnifiedRow {
        // `to_addrs`/`cc_addrs` pushed the aggregate to indexes 17/18.
        thread_size: row.get(17)?,
        thread_unseen: row.get(18)?,
        ..row_to_unified(row)?
    })
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;

    fn envelope(uid: Uid, subject: &str, epoch: i64, seen: bool) -> Envelope {
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
            flagged: uid.is_multiple_of(2),
            to_addrs: Vec::new(),
            cc_addrs: Vec::new(),
        }
    }

    fn test_account(store: &Store) -> i64 {
        store
            .adopt_or_create_account("test@exemple.fr", "gmail")
            .unwrap()
    }

    fn store_with_mailbox() -> (Store, i64) {
        let store = Store::open_in_memory().unwrap();
        let account = test_account(&store);
        let id = store.create_mailbox(account, "INBOX", 1).unwrap();
        (store, id)
    }

    /// Every "per message" table filled for a UID: what every purge must
    /// carry away (PLAN-AUDIT-V1 E4).
    fn fill_message(store: &mut Store, inbox: i64, uid: Uid) {
        store
            .upsert_envelopes(inbox, &[envelope(uid, "subject", 100, false)])
            .unwrap();
        store.save_body(inbox, uid, "<p>body</p>", &[]).unwrap();
        let conn = store.conn();
        conn.execute(
            "INSERT INTO attachments (mailbox_id, uid, idx, name, mime, size) VALUES (?1, ?2, 0, 'a.pdf', 'application/pdf', 1)",
            params![inbox, uid],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO invitations (mailbox_id, uid, methode, event_uid) VALUES (?1, ?2, 'REQUEST', 'evt')",
            params![inbox, uid],
        )
        .unwrap();
        for table in ["images_messages", "mis_de_cote", "kiosque_lus"] {
            conn.execute(
                &format!("INSERT INTO {table} (mailbox_id, uid, epoch) VALUES (?1, ?2, 1)"),
                params![inbox, uid],
            )
            .unwrap();
        }
    }

    /// How many rows, across every per-message table, still carry this
    /// UID.
    fn message_rows(store: &Store, inbox: i64, uid: Uid) -> Vec<(&'static str, i64)> {
        [
            "envelopes",
            "bodies",
            "attachments",
            "invitations",
            "images_messages",
            "mis_de_cote",
            "kiosque_lus",
        ]
        .into_iter()
        .map(|table| {
            let n: i64 = store
                .conn()
                .query_row(
                    &format!("SELECT COUNT(*) FROM {table} WHERE mailbox_id = ?1 AND uid = ?2"),
                    params![inbox, uid],
                    |row| row.get(0),
                )
                .unwrap();
            (table, n)
        })
        .filter(|(_, n)| *n > 0)
        .collect()
    }

    /// Audit 2026-09-01 S2 (E4): `remove_absent` only purged 3 tables out
    /// of 7 — a message gone from the server left attachments, invitation,
    /// image memory, set-aside and Feed "read" orphaned (no foreign key on
    /// `envelopes`). ONE list, the same for all three purges.
    #[test]
    fn a_message_gone_from_the_server_leaves_no_orphan() {
        let (mut store, inbox) = store_with_mailbox();
        fill_message(&mut store, inbox, 1);
        assert_eq!(message_rows(&store, inbox, 1).len(), 7, "fixture filled");

        let removed = store.remove_absent(inbox, &HashSet::new()).unwrap();

        assert_eq!(removed, 1);
        assert_eq!(
            message_rows(&store, inbox, 1),
            Vec::<(&str, i64)>::new(),
            "no row must survive the message"
        );
    }

    /// A SQLite trigger that refuses envelope deletion simulates a failure
    /// in the middle of the purge: everything that came before it (body,
    /// actions…) must be ROLLED BACK. Before E4, `reset_mailbox` chained
    /// nine autocommit writes — a crash between two of them left threads
    /// without envelopes (the "badge in front of an empty list" already
    /// paid for at organized mode's E5).
    fn block_envelope_deletions(store: &Store) {
        store
            .conn()
            .execute_batch(
                "CREATE TEMP TRIGGER panne BEFORE DELETE ON envelopes
                 BEGIN SELECT RAISE(ABORT, 'panne simulee'); END;",
            )
            .unwrap();
    }

    #[test]
    fn reset_mailbox_is_atomic() {
        let (mut store, inbox) = store_with_mailbox();
        fill_message(&mut store, inbox, 1);
        store.enqueue_action(inbox, 1, Action::MarkSeen).unwrap();
        block_envelope_deletions(&store);

        assert!(
            store.reset_mailbox(inbox, 2).is_err(),
            "the failure must propagate"
        );

        assert_eq!(
            message_rows(&store, inbox, 1).len(),
            7,
            "nothing was erased before the failure: a single transaction"
        );
        assert_eq!(store.pending_actions(inbox).unwrap().len(), 1);
        assert_eq!(
            store
                .sync_state(test_account(&store), "INBOX")
                .unwrap()
                .unwrap()
                .uid_validity,
            1,
            "the UIDVALIDITY did not move either"
        );
    }

    #[test]
    fn remove_local_is_atomic() {
        let (mut store, inbox) = store_with_mailbox();
        fill_message(&mut store, inbox, 1);
        block_envelope_deletions(&store);

        assert!(store.remove_local(inbox, 1).is_err());

        assert_eq!(
            message_rows(&store, inbox, 1).len(),
            7,
            "body, attachments, invitation… all still there: rolled back with the envelope"
        );
    }

    /// PLAN-AUDIT-V1 review: a refused action is not eternal — a fresh
    /// gesture from the user on the same message replaces it, and the
    /// screener-waiting row falls back down.
    #[test]
    fn a_new_gesture_replaces_the_old_refused_action() {
        let (store, id) = store_with_mailbox();
        store
            .enqueue_action(id, 1, Action::MoveTo("Gone".to_string()))
            .unwrap();
        let refused = store.pending_actions(id).unwrap().remove(0).id;
        store.refuse_action(refused, "[TRYCREATE]").unwrap();
        assert_eq!(store.refused_actions().unwrap(), 1);

        store.enqueue_action(id, 1, Action::MarkSeen).unwrap();

        assert_eq!(store.refused_actions().unwrap(), 0, "replaced");
        let queue = store.pending_actions(id).unwrap();
        assert_eq!(queue.len(), 1);
        assert_eq!(queue[0].action, Action::MarkSeen);
    }

    /// Audit 2026-09-01 (PLAN-AUDIT-V1 E3): a `pending_actions` row with an
    /// unreadable `kind` (future version, corruption) made the WHOLE
    /// `pending_actions(mailbox_id)` fail — the entire queue jammed by one
    /// row. It is quarantined with its reason, the queue goes on.
    #[test]
    fn an_unreadable_row_does_not_fail_the_whole_queue() {
        let (store, id) = store_with_mailbox();
        store.enqueue_action(id, 1, Action::MarkSeen).unwrap();
        store
            .conn()
            .execute(
                "INSERT INTO pending_actions (mailbox_id, uid, kind) VALUES (?1, 2, 'teleporter')",
                [id],
            )
            .unwrap();
        store.enqueue_action(id, 3, Action::Archive).unwrap();

        let queue = store.pending_actions(id).unwrap();
        assert_eq!(
            queue.iter().map(|p| p.uid).collect::<Vec<_>>(),
            vec![1, 3],
            "the readable ones pass, the unreadable one is set aside"
        );
        assert_eq!(store.refused_actions().unwrap(), 1);
        // Idempotent: a second read does not recount it.
        store.pending_actions(id).unwrap();
        assert_eq!(store.refused_actions().unwrap(), 1);
    }

    /// D-36 (closed at the 2026-09-01 audit): a `\n` inside a `--` comment
    /// of the `SCHEMA` literal became a real newline, SQLite swallowed the
    /// rest of the comment as a COLUMN, and every FRESH database was born
    /// with a phantom column in `echos`. The missing net: every column of
    /// every table of a fresh database carries a sane name — an
    /// identifier, never a scrap of sentence.
    #[test]
    fn a_fresh_database_has_no_phantom_column() {
        let store = Store::open_in_memory().unwrap();
        let conn = store.conn();
        let mut tables = conn
            .prepare(
                "SELECT name FROM sqlite_master \
                 WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
            )
            .unwrap();
        let names: Vec<String> = tables
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(
            names.iter().any(|t| t == "echos"),
            "the echos table is missing"
        );
        for table in names {
            let mut columns = conn
                .prepare(&format!("PRAGMA table_info(\"{table}\")"))
                .unwrap();
            let column_names: Vec<String> = columns
                .query_map([], |row| row.get(1))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            for column in &column_names {
                assert!(
                    column
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
                    "phantom column \"{column}\" in {table}: {column_names:?}"
                );
            }
        }
    }

    fn recent(store: &Store, offset: usize, limit: usize) -> Vec<Envelope> {
        store
            .recent(test_account(store), "INBOX", offset, limit)
            .unwrap()
    }

    /// R4: the To/Cc recipients written at sync read back exactly as
    /// written — it is what the Sent folder displays (the sender there is
    /// SELF) and what "Reply all" reads back offline. The "Test Attachment
    /// 3" case: a send to a third-party address.
    #[test]
    fn upsert_persists_the_recipients() {
        let (mut store, id) = store_with_mailbox();
        let mut env = envelope(1, "Test Attachment 3", 1_700_000_000, true);
        env.to_addrs = vec!["sebastien.monchamps@gmail.com".to_string()];
        env.cc_addrs = vec![
            "copie1@exemple.fr".to_string(),
            "copie2@exemple.fr".to_string(),
        ];
        store
            .upsert_envelopes(id, std::slice::from_ref(&env))
            .unwrap();
        assert_eq!(recent(&store, 0, 10), vec![env]);
    }

    /// A preference never set answers the requested default; set, it
    /// reads back exactly as written and overwrites without duplicating.
    #[test]
    fn bool_pref_default_then_roundtrip() {
        let store = Store::open_in_memory().unwrap();
        assert!(store.bool_pref("arrival_bubbles", true).unwrap());
        assert!(!store.bool_pref("arrival_bubbles", false).unwrap());
        store.set_bool_pref("arrival_bubbles", false).unwrap();
        assert!(!store.bool_pref("arrival_bubbles", true).unwrap());
        store.set_bool_pref("arrival_bubbles", true).unwrap();
        assert!(store.bool_pref("arrival_bubbles", false).unwrap());
    }

    /// The marker of the guarded poll (ADR 0017): never set -> `None` (a
    /// legacy database polls everything on its first cycle), set -> read
    /// back.
    #[test]
    fn remote_uidnext_absent_then_set() {
        let store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("a@exemple.fr", "gmail")
            .unwrap();
        let mailbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        // NULL as long as no guarded poll has happened: a legacy database
        // polls everything on its first cycle (ADR 0017).
        assert_eq!(store.remote_uidnext(mailbox).unwrap(), None);
        store.set_remote_uidnext(mailbox, 101).unwrap();
        assert_eq!(store.remote_uidnext(mailbox).unwrap(), Some(101));
        assert_eq!(store.envelope_count(mailbox).unwrap(), 0);
        assert!(!store.has_pending_actions(mailbox).unwrap());
    }

    /// A departure pending replay (archive, deletion, move) no longer
    /// counts in the progress denominator: the gesture removes the local
    /// row immediately (echo, PLAN-REACTIVITE E3) but `remote_total` dates
    /// from the last SELECT — without the adjustment, a SINGLE triage was
    /// enough to freeze progress at 99% and the status bar's hitofude
    /// stroke with it (field 2026-08-15, PLAN-GELS: 5 archives + 1 pending
    /// deletion = 99% for the whole duration of the replay). The real
    /// gesture path is called (`gesture_with_echo`), never a simulation.
    #[test]
    fn a_departure_pending_replay_no_longer_counts_in_the_denominator() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "stays", 100, true),
                    envelope(2, "leaves for archive", 200, true),
                    envelope(3, "stays too", 300, false),
                ],
            )
            .unwrap();
        store.record_remote_total(id, 3).unwrap();
        assert_eq!(store.sync_progress().unwrap(), (3, 3));
        // The triage: the echo removes the row, the action awaits its replay.
        store
            .gesture_with_echo(id, 2, Action::Archive, Some("archives"))
            .unwrap();
        assert_eq!(
            store.sync_progress().unwrap(),
            (2, 2),
            "the locally archived message must no longer be awaited"
        );
        // Marking as pending removes nothing from the mailbox: it does not
        // touch the denominator.
        store.enqueue_action(id, 1, Action::MarkSeen).unwrap();
        assert_eq!(store.sync_progress().unwrap(), (2, 2));
        // A move also removes; and the denominator never drops below zero
        // even when `remote_total` is behind.
        store
            .gesture_with_echo(id, 3, Action::MoveTo("Invoices".into()), None)
            .unwrap();
        store.record_remote_total(id, 1).unwrap();
        assert_eq!(store.sync_progress().unwrap(), (1, 0));
    }

    /// The text counterpart: never set -> `None` (the default belongs to
    /// the caller), set -> read back exactly as written, overwritten
    /// without duplicating.
    #[test]
    fn text_pref_none_then_roundtrip() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(store.text_pref("lang").unwrap(), None);
        store.set_text_pref("lang", "en").unwrap();
        assert_eq!(store.text_pref("lang").unwrap(), Some("en".to_string()));
        store.set_text_pref("lang", "fr").unwrap();
        assert_eq!(store.text_pref("lang").unwrap(), Some("fr".to_string()));
    }

    /// The transactional batch: everything written, everything read
    /// back — the multi-key counterpart of `text_pref_none_then_roundtrip`.
    #[test]
    fn set_text_prefs_writes_the_whole_batch() {
        let mut store = Store::open_in_memory().unwrap();
        store
            .set_text_prefs(&[("repere_icone.1", "home"), ("repere_teinte.1", "bleu")])
            .unwrap();
        assert_eq!(
            store.text_pref("repere_icone.1").unwrap(),
            Some("home".to_string())
        );
        assert_eq!(
            store.text_pref("repere_teinte.1").unwrap(),
            Some("bleu".to_string())
        );
    }

    #[test]
    fn roundtrips_all_envelope_fields() {
        let (mut store, id) = store_with_mailbox();
        let original = envelope(7, "Sujet accentué : été", 1_700_000_000, true); // lang:fr
        store
            .upsert_envelopes(id, std::slice::from_ref(&original))
            .unwrap();
        assert_eq!(recent(&store, 0, 10), vec![original]);
    }

    #[test]
    fn roundtrips_envelope_without_optional_fields() {
        let (mut store, id) = store_with_mailbox();
        let bare = Envelope {
            reply_to: None,
            uid: 1,
            subject: None,
            sender: None,
            sender_address: None,
            message_id: None,
            in_reply_to: None,
            date: None,
            seen: false,
            flagged: false,
            to_addrs: Vec::new(),
            cc_addrs: Vec::new(),
        };
        store
            .upsert_envelopes(id, std::slice::from_ref(&bare))
            .unwrap();
        assert_eq!(recent(&store, 0, 10), vec![bare]);
    }

    /// The backfill order is a PRODUCT choice, not an accident of SQL
    /// sort: INBOX first, Sent next, the rest by name. A server that
    /// lists "Archive" before INBOX must not backfill 80,000 archive
    /// bodies before the mail the list displays.
    #[test]
    fn mailboxes_backfill_inbox_first() {
        let store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("moi@exemple.fr", "gmail")
            .unwrap();
        store.create_mailbox(account, "Archive", 1).unwrap();
        store.create_mailbox(account, "Corbeille", 1).unwrap(); // lang:fr
        store.create_mailbox(account, "INBOX", 1).unwrap();
        store
            .create_mailbox(account, "Messages envoyés", 1) // lang:fr
            .unwrap();
        store
            .set_thread_scope(account, Some("Messages envoyés")) // lang:fr
            .unwrap();

        assert_eq!(
            store.mailbox_names(account).unwrap(),
            vec!["INBOX", "Messages envoyés", "Archive", "Corbeille"] // lang:fr
        );
    }

    /// The import horizon (PLAN-HORIZON-NETTOYAGE, D1-D4): a per-account
    /// pref with a CLOSED vocabulary; no pref -> "tout" (all) — an
    /// account from before the setting keeps the full import (D4); the
    /// value dies with the account, and a reused rowid does not inherit
    /// it (PREFS_PAR_COMPTE).
    #[test]
    fn horizon_import_defaults_to_all_closed_vocabulary_purged_on_removal() {
        let mut store = Store::open_in_memory().unwrap();
        let id = store
            .adopt_or_create_account("h@exemple.fr", "gmail")
            .unwrap();

        assert_eq!(store.horizon_import(id).unwrap(), "tout");
        store.set_horizon_import(id, "1a").unwrap();
        assert_eq!(store.horizon_import(id).unwrap(), "1a");
        assert!(store.set_horizon_import(id, "42 jours").is_err());
        assert_eq!(store.horizon_import(id).unwrap(), "1a");

        store.delete_account(id).unwrap();
        let heir = store
            .adopt_or_create_account("h2@exemple.fr", "gmail")
            .unwrap();
        assert_eq!(heir, id, "fixture: the rowid must be reused");
        assert_eq!(store.horizon_import(heir).unwrap(), "tout");
    }

    /// Removing an account leaves NOTHING behind: neither the cascading
    /// rows (mailboxes, envelopes, bodies), nor those without a foreign
    /// key (drafts, outbox, search index) — and the neighboring account
    /// keeps everything, search included.
    #[test]
    fn delete_account_erases_everything_and_does_not_touch_the_neighbor() {
        let mut store = Store::open_in_memory().unwrap();
        let departed = store
            .adopt_or_create_account("part@exemple.fr", "gmail")
            .unwrap();
        let neighbor = store
            .adopt_or_create_account("reste@exemple.fr", "gmail")
            .unwrap();
        for (account, subject) in [
            (departed, "Invoice for the departure"),
            (neighbor, "Quote that stays"),
        ] {
            let mailbox = store.create_mailbox(account, "INBOX", 1).unwrap();
            store
                .upsert_envelopes(mailbox, &[envelope(1, subject, 100, false)])
                .unwrap();
            store.save_body(mailbox, 1, "<p>body</p>", &[]).unwrap();
            store
                .save_draft(
                    account,
                    None,
                    None,
                    crate::DraftContent {
                        to_raw: "a@b.fr",
                        cc_raw: "",
                        bcc_raw: "",
                        body_html: None,
                        subject,
                        body: "draft",
                        reply_to_uid: None,
                        reply_to_mailbox: None,
                        important: false,
                    },
                )
                .unwrap();
            store
                .enqueue_outbox(
                    account,
                    &crate::compose::Draft {
                        message_id: format!("<outbound-{account}@exemple.fr>"),
                        from: "moi@exemple.fr".to_string(),
                        to: vec!["a@b.fr".to_string()],
                        cc: Vec::new(),
                        bcc: Vec::new(),
                        subject: subject.to_string(),
                        body_text: "body".to_string(),
                        body_html: None,
                        in_reply_to: None,
                        references: None,
                        important: false,
                        ics_reply: None,
                    },
                )
                .unwrap();
        }

        // The preferences suffixed by the id (signature, marker, name): an
        // SQLite id reused after removal would otherwise make the next
        // account inherit the old one's identity (PLAN-RETOURS-8 review;
        // custom name: PLAN-RETOURS-9).
        for (account, hue) in [(departed, "rouge"), (neighbor, "bleu")] {
            store
                .set_text_pref(&format!("signature.{account}"), "<p>sig</p>")
                .unwrap();
            store
                .set_text_pref(&format!("repere_icone.{account}"), "home")
                .unwrap();
            store
                .set_text_pref(&format!("repere_teinte.{account}"), hue)
                .unwrap();
            store
                .set_text_pref(&format!("nom_compte.{account}"), "Perso")
                .unwrap();
        }

        store.delete_account(departed).unwrap();

        let accounts = store.accounts().unwrap();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].email, "reste@exemple.fr");
        for key in ["signature", "repere_icone", "repere_teinte", "nom_compte"] {
            assert_eq!(
                store.text_pref(&format!("{key}.{departed}")).unwrap(),
                None,
                "{key} of the departed account: the pref must die with it"
            );
            assert!(
                store
                    .text_pref(&format!("{key}.{neighbor}"))
                    .unwrap()
                    .is_some(),
                "{key} of the neighbor: intact"
            );
        }
        for table in [
            "mailboxes",
            "envelopes",
            "bodies",
            "drafts",
            "outbox",
            "search_docs",
        ] {
            let total: i64 = store
                .0
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(total, 1, "{table}: only the neighbor's row must remain");
        }
        assert!(
            store.search("departure", 10).unwrap().is_empty(),
            "the departed account's mail must no longer come up in search"
        );
        assert_eq!(
            store.search("stays", 10).unwrap().len(),
            1,
            "the neighbor's search must survive the removal"
        );
    }

    /// ADR 0010: a message WITHOUT a date stays eligible for backfill,
    /// even under a bounded horizon. The old rule excluded it ("not
    /// placeable within the horizon") — a silent hole: never a body, so
    /// never search, and nothing on screen to flag it. The doubt now
    /// only costs its rank: the NULLs close the sort.
    #[test]
    fn a_message_without_a_date_stays_to_be_backfilled() {
        let (mut store, id) = store_with_mailbox();
        let without_date = Envelope {
            reply_to: None,
            uid: 9,
            subject: None,
            sender: None,
            sender_address: None,
            message_id: None,
            in_reply_to: None,
            date: None,
            seen: false,
            flagged: false,
            to_addrs: Vec::new(),
            cc_addrs: Vec::new(),
        };
        store
            .upsert_envelopes(id, std::slice::from_ref(&without_date))
            .unwrap();

        let account = test_account(&store);
        let uids = store
            .bodies_to_backfill(account, "INBOX", 1_000_000, 10)
            .unwrap();
        assert_eq!(
            uids,
            vec![9],
            "the bounded horizon no longer excludes the dateless"
        );
        assert_eq!(
            store
                .bodies_pending_count(account, "INBOX", 1_000_000)
                .unwrap(),
            1,
            "the progress counter sees it too — otherwise the bar would lie"
        );
    }

    #[test]
    fn upsert_replaces_existing_envelope() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(id, &[envelope(1, "before", 100, false)])
            .unwrap();
        store
            .upsert_envelopes(id, &[envelope(1, "after", 100, true)])
            .unwrap();
        let rows = recent(&store, 0, 10);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].subject.as_deref(), Some("after"));
        assert!(rows[0].seen);
    }

    #[test]
    fn recent_orders_by_date_then_uid_descending() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "old", 100, false),
                    envelope(3, "recent", 300, false),
                    envelope(2, "middle", 200, false),
                ],
            )
            .unwrap();
        let uids: Vec<Uid> = recent(&store, 0, 2).iter().map(|e| e.uid).collect();
        assert_eq!(uids, vec![3, 2]);
    }

    #[test]
    fn remove_absent_deletes_only_missing_uids() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "a", 100, false),
                    envelope(2, "b", 200, false),
                    envelope(3, "c", 300, false),
                ],
            )
            .unwrap();
        let present: HashSet<Uid> = [1, 3].into_iter().collect();
        assert_eq!(store.remove_absent(id, &present).unwrap(), 1);
        assert_eq!(store.count(id).unwrap(), 2);
    }

    #[test]
    fn sync_state_roundtrips_including_modseq() {
        let (store, id) = store_with_mailbox();
        assert_eq!(
            store.sync_state(test_account(&store), "INBOX").unwrap(),
            Some(SyncState {
                mailbox_id: id,
                uid_validity: 1,
                last_uid: 0,
                highest_modseq: None,
                initialized: false,
            })
        );
        store.update_state(id, 42, Some(9000)).unwrap();
        let state = store
            .sync_state(test_account(&store), "INBOX")
            .unwrap()
            .unwrap();
        assert_eq!(state.last_uid, 42);
        assert_eq!(state.highest_modseq, Some(9000));
    }

    #[test]
    fn sync_state_is_none_for_unknown_mailbox() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(
            store.sync_state(test_account(&store), "INBOX").unwrap(),
            None
        );
    }

    #[test]
    fn reset_mailbox_clears_envelopes_and_state() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(id, &[envelope(1, "a", 100, false)])
            .unwrap();
        store.update_state(id, 1, Some(5)).unwrap();
        store.reset_mailbox(id, 2).unwrap();
        assert_eq!(store.count(id).unwrap(), 0);
        let state = store
            .sync_state(test_account(&store), "INBOX")
            .unwrap()
            .unwrap();
        assert_eq!(state.uid_validity, 2);
        assert_eq!(state.last_uid, 0);
        assert_eq!(state.highest_modseq, None);
    }

    #[test]
    fn max_uid_is_zero_for_empty_mailbox() {
        let (store, id) = store_with_mailbox();
        assert_eq!(store.max_uid(id).unwrap(), 0);
    }

    #[test]
    fn recent_pages_with_offset() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &(1..=5)
                    .map(|uid| envelope(uid, "subject", 100 * i64::from(uid), false))
                    .collect::<Vec<_>>(),
            )
            .unwrap();
        let page: Vec<Uid> = recent(&store, 2, 2).iter().map(|e| e.uid).collect();
        assert_eq!(page, vec![3, 2], "offset 2 skips the two most recent");
        assert!(recent(&store, 10, 5).is_empty());
    }

    #[test]
    fn action_queue_roundtrips_in_emission_order() {
        let (store, id) = store_with_mailbox();
        store.enqueue_action(id, 5, Action::MarkSeen).unwrap();
        store.enqueue_action(id, 3, Action::MarkUnseen).unwrap();

        let queued = store.pending_actions(id).unwrap();
        assert_eq!(queued.len(), 2);
        assert_eq!(
            (queued[0].uid, queued[0].action.clone()),
            (5, Action::MarkSeen)
        );
        assert_eq!(
            (queued[1].uid, queued[1].action.clone()),
            (3, Action::MarkUnseen)
        );

        store.remove_action(queued[0].id).unwrap();
        assert_eq!(store.pending_actions(id).unwrap().len(), 1);
    }

    #[test]
    fn set_seen_local_updates_and_reports_actual_change() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(id, &[envelope(1, "a", 100, false)])
            .unwrap();

        assert!(store.set_seen_local(id, 1, true).unwrap());
        assert!(recent(&store, 0, 1)[0].seen);
        assert!(
            !store.set_seen_local(id, 1, true).unwrap(),
            "already seen: nothing to log"
        );
    }

    #[test]
    fn set_flagged_local_updates_and_reports_actual_change() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(id, &[envelope(1, "a", 100, false)])
            .unwrap();

        assert!(store.set_flagged_local(id, 1, true).unwrap());
        assert!(recent(&store, 0, 1)[0].flagged);
        assert!(
            !store.set_flagged_local(id, 1, true).unwrap(),
            "already flagged: nothing to log"
        );
    }

    /// E4 (PLAN-REACTIVITE, 1st field): ARRIVALS are counted by UID,
    /// never by the report's `fetched` — a CONDSTORE delta mixes in
    /// every shuffled flag (Gmail on every label), and the body limit
    /// "overflowed" on every arrival.
    #[test]
    fn arrivals_are_counted_by_uid() {
        let (mut store, id) = store_with_mailbox();
        let account = test_account(&store);
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "old", 100, true),
                    envelope(2, "old too", 200, true),
                ],
            )
            .unwrap();
        assert_eq!(store.arrivals_since(account, "INBOX", 2).unwrap(), 0);

        // Two arrivals + one old flag retouched (upsert of the same
        // uid 1): the count only moves for the new UIDs.
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "old", 100, false),
                    envelope(3, "new", 300, false),
                    envelope(4, "new too", 400, false),
                ],
            )
            .unwrap();
        assert_eq!(store.arrivals_since(account, "INBOX", 2).unwrap(), 2);
        // Unknown mailbox: zero, never an error — the poll of an account
        // never synced must not break on this account.
        assert_eq!(store.arrivals_since(account, "Elsewhere", 0).unwrap(), 0);
    }

    #[test]
    fn remove_local_drops_envelope_and_body() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(id, &[envelope(1, "a", 100, false)])
            .unwrap();
        store.save_body(id, 1, "<p>x</p>", &[]).unwrap();

        store.remove_local(id, 1).unwrap();

        assert!(recent(&store, 0, 10).is_empty());
        assert_eq!(store.body(test_account(&store), "INBOX", 1).unwrap(), None);
    }

    #[test]
    fn reset_mailbox_clears_pending_actions() {
        let (store, id) = store_with_mailbox();
        store.enqueue_action(id, 1, Action::MarkSeen).unwrap();
        store.reset_mailbox(id, 2).unwrap();
        assert!(store.pending_actions(id).unwrap().is_empty());
    }

    #[test]
    fn body_roundtrips_and_is_none_when_absent() {
        let (store, id) = store_with_mailbox();
        assert_eq!(store.body(test_account(&store), "INBOX", 1).unwrap(), None);
        store.save_body(id, 1, "<p>hello</p>", &[]).unwrap();
        assert_eq!(
            store
                .body(test_account(&store), "INBOX", 1)
                .unwrap()
                .as_deref(),
            Some("<p>hello</p>")
        );
    }

    fn pdf(index: usize, name: &str) -> Attachment {
        Attachment {
            index,
            name: name.to_string(),
            mime: "application/pdf".to_string(),
            size: 2048,
        }
    }

    /// What the backfill has searched for since 2026-08-26: **ABSENT
    /// bodies**, and nothing else.
    ///
    /// It long also searched for bodies fetched BEFORE attachments
    /// existed — `bodies.scanned = 0`, a MIME never inspected, not
    /// recoverable from the stored HTML. This criterion is **removed**
    /// (PLAN-DEMARRAGE, CE decision D8): it forced SQLite to recall the
    /// body row to read one bit, which held the global lock **8,870 ms
    /// on every startup** on the field database.
    ///
    /// The three facts that allowed it, all measured on 2026-08-26:
    /// production **never** writes `scanned = 0` ([`Store::save_body_full`]
    /// hardcodes a `1`); **both** workstations of the fleet carry **zero**
    /// rows at `scanned = 0`; and the legacy pass that produced them is
    /// closed everywhere. The criterion protected zero rows.
    ///
    /// What this test therefore keeps: a present body takes the message
    /// out of the backfill, and **nothing brings it back**. Plus the
    /// write invariant that made the removal safe — if something were to
    /// write `scanned = 0` one day, the decision would need reopening,
    /// and this test would say so.
    #[test]
    fn a_present_body_takes_the_message_out_of_the_backfill_and_nothing_brings_it_back() {
        let (mut store, id) = store_with_mailbox();
        let account = test_account(&store);
        store
            .upsert_envelopes(id, &[envelope(1, "subject", 100, false)])
            .unwrap();

        // Without a body: the message waits.
        assert_eq!(
            store.bodies_to_backfill(account, "INBOX", 0, 10).unwrap(),
            vec![1]
        );
        assert_eq!(store.bodies_pending_count(account, "INBOX", 0).unwrap(), 1);

        store.save_body(id, 1, "<p>body</p>", &[]).unwrap();

        // Body present: nothing left to do, definitively.
        assert!(
            store
                .bodies_to_backfill(account, "INBOX", 0, 10)
                .unwrap()
                .is_empty()
        );
        assert_eq!(store.bodies_pending_count(account, "INBOX", 0).unwrap(), 0);

        // The INVARIANT that made removing the criterion safe: production
        // always writes `scanned = 1`. The column is no longer read by
        // the backfill — if it had to become so again, it would still
        // tell the truth.
        let scanned: i64 = store
            .conn()
            .query_row("SELECT scanned FROM bodies", [], |row| row.get(0))
            .unwrap();
        assert_eq!(
            scanned, 1,
            "production must always write scanned = 1 — otherwise PLAN-DEMARRAGE's decision D8 needs reopening"
        );
    }

    /// R1 (PLAN-RETOURS-3): the percentage's denominator. The total does
    /// NOT move when a body arrives — only the missing count decreases;
    /// `total - pending` gives the present bodies, the basis of the
    /// displayed percentage.
    #[test]
    fn the_corpus_total_counts_messages_not_bodies() {
        let (mut store, id) = store_with_mailbox();
        let account = test_account(&store);
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "one", 100, false),
                    envelope(2, "two", 200, false),
                    envelope(3, "three", 300, false),
                ],
            )
            .unwrap();

        // Three messages in scope, no body read yet.
        assert_eq!(store.bodies_total_count(account, "INBOX", 0).unwrap(), 3);
        assert_eq!(store.bodies_pending_count(account, "INBOX", 0).unwrap(), 3);

        // A body arrives: the total holds, the rest drops by one.
        store.save_body(id, 2, "<p>body</p>", &[]).unwrap();
        assert_eq!(
            store.bodies_total_count(account, "INBOX", 0).unwrap(),
            3,
            "the total is the corpus, not the fetched bodies"
        );
        assert_eq!(store.bodies_pending_count(account, "INBOX", 0).unwrap(), 2);
    }

    /// A message already read elsewhere — phone, webmail — must not
    /// trigger a notification bubble: it is pure noise, and it is what
    /// gets notifications turned off.
    #[test]
    fn only_genuinely_new_and_unread_messages_are_notifiable() {
        let (mut store, id) = store_with_mailbox();
        let account = test_account(&store);
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(10, "old", 100, false),
                    envelope(11, "already read", 200, true),
                    envelope(12, "truly new", 300, false),
                ],
            )
            .unwrap();

        let arrivals = store.new_unread_after(account, "INBOX", 10, 20).unwrap();
        let subjects: Vec<_> = arrivals
            .iter()
            .map(|e| e.subject.clone().unwrap_or_default())
            .collect();
        assert_eq!(subjects, vec!["truly new".to_string()]);
    }

    fn folder(wire: &str, display: &str) -> Folder {
        Folder {
            wire: wire.to_string(),
            display: display.to_string(),
            selectable: true,
            special_use: None,
        }
    }

    /// Choosing a destination must work OFFLINE: the list is therefore
    /// read locally, like the envelopes. Both the wire name and the
    /// readable name are kept — losing the first would make the move
    /// unplayable at replay time.
    #[test]
    fn folders_are_cached_locally_with_both_names() {
        let (store, _) = store_with_mailbox();
        let account = test_account(&store);
        assert!(store.folders(account).unwrap().is_empty());

        store
            .replace_folders(account, &[folder("Archiv&AOk-s", "Archivés")]) // lang:fr
            .unwrap();

        let cached = store.folders(account).unwrap();
        assert_eq!(cached.len(), 1);
        assert_eq!(cached[0].wire, "Archiv&AOk-s");
        assert_eq!(cached[0].display, "Archivés"); // lang:fr
    }

    /// A folder deleted server-side must no longer be offered: the move
    /// would fail at replay time, long after the click — and the user
    /// would no longer see the connection.
    #[test]
    fn refreshing_folders_drops_the_ones_that_disappeared() {
        let (store, _) = store_with_mailbox();
        let account = test_account(&store);
        store
            .replace_folders(account, &[folder("Old", "Old"), folder("Stays", "Stays")])
            .unwrap();

        store
            .replace_folders(account, &[folder("Stays", "Stays")])
            .unwrap();

        let cached = store.folders(account).unwrap();
        assert_eq!(cached.len(), 1);
        assert_eq!(cached[0].wire, "Stays");
    }

    #[test]
    fn attachments_are_saved_with_the_body_and_read_back_in_order() {
        let (store, id) = store_with_mailbox();
        let account = test_account(&store);
        assert!(
            store.attachments(account, "INBOX", 1).unwrap().is_empty(),
            "nothing as long as the body has not been fetched"
        );

        store
            .save_body(
                id,
                1,
                "<p>attached</p>",
                &[pdf(0, "one.pdf"), pdf(1, "two.pdf")],
            )
            .unwrap();

        let found = store.attachments(account, "INBOX", 1).unwrap();
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].name, "one.pdf");
        assert_eq!(found[1].name, "two.pdf");
        assert_eq!(found[1].size, 2048);
    }

    /// A re-downloaded message whose attachment has disappeared must not
    /// keep the old row: the user would click a file the server no
    /// longer serves, and the failure would only surface at download
    /// time — far from the cause.
    #[test]
    fn re_saving_replaces_the_attachment_list_instead_of_accumulating() {
        let (store, id) = store_with_mailbox();
        let account = test_account(&store);
        store
            .save_body(id, 1, "<p>x</p>", &[pdf(0, "one.pdf"), pdf(1, "two.pdf")])
            .unwrap();

        store
            .save_body(id, 1, "<p>x</p>", &[pdf(0, "one.pdf")])
            .unwrap();

        let found = store.attachments(account, "INBOX", 1).unwrap();
        assert_eq!(
            found.len(),
            1,
            "the vanished attachment must be gone here too"
        );
        assert_eq!(found[0].name, "one.pdf");
    }

    /// Attachments belong to a message of an ACCOUNT: the same (mailbox,
    /// uid) pair on another account must see nothing.
    #[test]
    fn attachments_never_leak_across_accounts() {
        let (store, id) = store_with_mailbox();
        store
            .save_body(id, 1, "<p>x</p>", &[pdf(0, "private.pdf")])
            .unwrap();

        let other = store
            .adopt_or_create_account("autre@exemple.fr", "gmail")
            .unwrap();
        store.create_mailbox(other, "INBOX", 1).unwrap();

        assert!(store.attachments(other, "INBOX", 1).unwrap().is_empty());
    }

    fn project_invitation() -> crate::InvitationRow {
        crate::InvitationRow {
            method: "request".into(),
            event_uid: "reunion-1@exemple.fr".into(),
            sequence: 2,
            title: "Project sync".into(),
            location: Some("Room A".into()),
            organizer_address: Some("claire@exemple.fr".into()),
            organizer_name: Some("Claire Martin".into()),
            start_epoch: Some(1_788_400_200),
            end_epoch: Some(1_788_402_000),
            partstat: Some("sans_reponse".into()),
            ..Default::default()
        }
    }

    #[test]
    fn an_invitation_is_written_with_the_body_and_reads_back() {
        let (store, id) = store_with_mailbox();
        let account = test_account(&store);
        store
            .save_body_full(id, 1, "<p>x</p>", &[], Some(&project_invitation()))
            .unwrap();

        let stored = store.invitation(account, "INBOX", 1).unwrap().expect("row");
        assert_eq!(stored.row, project_invitation());
        assert_eq!(stored.reply, None, "not answered yet");
    }

    /// Same rule as attachments: a re-downloaded message WITHOUT a
    /// calendar part does not keep a phantom card.
    #[test]
    fn a_rescan_without_a_calendar_erases_the_row() {
        let (store, id) = store_with_mailbox();
        let account = test_account(&store);
        store
            .save_body_full(id, 1, "<p>x</p>", &[], Some(&project_invitation()))
            .unwrap();
        store.save_body(id, 1, "<p>x</p>", &[]).unwrap();
        assert_eq!(store.invitation(account, "INBOX", 1).unwrap(), None);
    }

    fn reply_draft() -> crate::compose::Draft {
        let mut draft = crate::compose(
            "moi@exemple.fr",
            "claire@exemple.fr",
            "",
            "",
            "Accepted: Project sync",
            "Accepted: Project sync",
            None,
        )
        .unwrap();
        draft.ics_reply = Some("BEGIN:VCALENDAR\r\nMETHOD:REPLY\r\nEND:VCALENDAR\r\n".into());
        draft
    }

    /// D6: the iTIP email gets logged AND the reply gets recorded — ONE
    /// transaction; the reply survives the body's rescan (two distinct
    /// truths — the PARTSTAT read from the message does not overwrite
    /// it).
    #[test]
    fn the_reply_is_logged_with_its_email_and_survives_the_rescan() {
        let (store, id) = store_with_mailbox();
        let account = test_account(&store);
        store
            .save_body_full(id, 1, "<p>x</p>", &[], Some(&project_invitation()))
            .unwrap();

        let outbox_id = store
            .enqueue_invitation_reply(
                account,
                &reply_draft(),
                "INBOX",
                1,
                "accepte",
                1_755_900_000,
            )
            .unwrap();
        assert!(outbox_id.is_some(), "email logged");
        store
            .save_body_full(id, 1, "<p>x</p>", &[], Some(&project_invitation()))
            .unwrap();

        let stored = store.invitation(account, "INBOX", 1).unwrap().expect("row");
        assert_eq!(stored.reply.as_deref(), Some("accepte"));
        assert_eq!(stored.reply_epoch, Some(1_755_900_000));
        assert_eq!(store.outbox_to_send(account).unwrap().len(), 1);
    }

    /// The row disappeared between display and click (purged, mailbox
    /// reset): NOTHING is sent — an email queued in front of a "not
    /// answered" card would invite a double send (review).
    #[test]
    fn a_reply_without_a_row_logs_nothing() {
        let (store, _id) = store_with_mailbox();
        let account = test_account(&store);
        assert_eq!(
            store
                .enqueue_invitation_reply(account, &reply_draft(), "INBOX", 9, "accepte", 1)
                .unwrap(),
            None
        );
        assert!(
            store.outbox_to_send(account).unwrap().is_empty(),
            "the transaction rolled back: no email in queue"
        );
    }

    /// The PLAN-INVITATIONS review: after a UIDVALIDITY change, the UIDs
    /// no longer mean anything — a card (and its reply!) that survived
    /// would stick to an unrelated message.
    #[test]
    fn reset_mailbox_erases_invitations_and_attachments() {
        let (store, id) = store_with_mailbox();
        let account = test_account(&store);
        store
            .save_body_full(
                id,
                1,
                "<p>x</p>",
                &[pdf(0, "one.pdf")],
                Some(&project_invitation()),
            )
            .unwrap();

        store.reset_mailbox(id, 2).unwrap();

        assert_eq!(store.invitation(account, "INBOX", 1).unwrap(), None);
        assert!(store.attachments(account, "INBOX", 1).unwrap().is_empty());
    }

    /// The `pieces-calendrier` repair: a message scanned BEFORE
    /// PLAN-INVITATIONS with a calendar part has SHIFTED attachment
    /// indices (the old numbering counted it) and no card. At the
    /// database's next opening, the body and attachments of these
    /// messages are dropped: the backfill will reread them with the new
    /// numbering — and the card will be born of the same scan (adoption,
    /// invariant §6.7). On a FILE database: it is the reopening that
    /// repairs it. Messages without a calendar do not move.
    #[test]
    fn the_calendar_attachments_repair_rereads_the_affected_messages() {
        let path =
            std::env::temp_dir().join(format!("wind-test-repair-cal-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let mut store = Store::open(&path).unwrap();
            let account = store
                .adopt_or_create_account("moi@exemple.fr", "gmail")
                .unwrap();
            let id = store.create_mailbox(account, "INBOX", 1).unwrap();
            store
                .upsert_envelopes(
                    id,
                    &[
                        envelope(1, "invitation", 100, true),
                        envelope(2, "simple", 90, true),
                    ],
                )
                .unwrap();
            // The BEFORE state: the calendar part counted as attachment 0.
            store
                .save_body(
                    id,
                    1,
                    "<p>invitation</p>",
                    &[
                        Attachment {
                            index: 0,
                            name: "attachment.calendar".into(),
                            mime: "text/calendar".into(),
                            size: 2048,
                        },
                        pdf(1, "contract.pdf"),
                    ],
                )
                .unwrap();
            store
                .save_body(id, 2, "<p>simple</p>", &[pdf(0, "note.pdf")])
                .unwrap();
            // Removes the marker set at opening (database born repaired):
            // we replay the arrival of a database from BEFORE the repair.
            store
                .conn()
                .execute(
                    "DELETE FROM reparations WHERE nom = 'pieces-calendrier'",
                    [],
                )
                .unwrap();
        }

        Store::forget_initialization(&path);
        let store = Store::open(&path).unwrap();
        let account = store
            .adopt_or_create_account("moi@exemple.fr", "gmail")
            .unwrap();
        assert_eq!(
            store.body(account, "INBOX", 1).unwrap(),
            None,
            "the message with a calendar will be reread"
        );
        assert!(store.attachments(account, "INBOX", 1).unwrap().is_empty());
        assert_eq!(
            store.body(account, "INBOX", 2).unwrap().as_deref(),
            Some("<p>simple</p>"),
            "the ordinary message does not move"
        );
        assert_eq!(store.attachments(account, "INBOX", 2).unwrap().len(), 1);
        drop(store);
        let _ = std::fs::remove_file(&path);
    }

    /// Field R6: a CANCEL extinguishes the REQUEST of the same meeting
    /// (same event_uid, same account), in BOTH arrival orders — the
    /// cancellation often arrives in a fresh conversation, it is the
    /// ORIGINAL card that must say so.
    #[test]
    fn a_cancel_extinguishes_the_request_of_the_same_meeting_in_both_arrival_orders() {
        let mut cancel = project_invitation();
        cancel.method = "cancel".to_string();
        cancel.cancelled = true;

        // Order 1: the REQUEST first, the CANCEL next.
        let (store, id) = store_with_mailbox();
        let account = test_account(&store);
        store
            .save_body_full(id, 1, "<p>i</p>", &[], Some(&project_invitation()))
            .unwrap();
        store
            .save_body_full(id, 2, "<p>a</p>", &[], Some(&cancel))
            .unwrap();
        assert!(
            store
                .invitation(account, "INBOX", 1)
                .unwrap()
                .expect("row")
                .row
                .cancelled,
            "the REQUEST is extinguished by the CANCEL"
        );

        // ANOTHER meeting of the same account does not move.
        let mut other = project_invitation();
        other.event_uid = "autre-reunion@exemple.fr".to_string();
        store
            .save_body_full(id, 3, "<p>x</p>", &[], Some(&other))
            .unwrap();
        assert!(
            !store
                .invitation(account, "INBOX", 3)
                .unwrap()
                .expect("row")
                .row
                .cancelled
        );

        // Order 2: the CANCEL scanned BEFORE (out-of-order backfill) —
        // the REQUEST is born cancelled.
        let (store, id) = store_with_mailbox();
        let account = test_account(&store);
        store
            .save_body_full(id, 2, "<p>a</p>", &[], Some(&cancel))
            .unwrap();
        store
            .save_body_full(id, 1, "<p>i</p>", &[], Some(&project_invitation()))
            .unwrap();
        assert!(
            store
                .invitation(account, "INBOX", 1)
                .unwrap()
                .expect("row")
                .row
                .cancelled
        );
    }

    #[test]
    fn an_invitation_does_not_leak_across_accounts() {
        let (store, id) = store_with_mailbox();
        store
            .save_body_full(id, 1, "<p>x</p>", &[], Some(&project_invitation()))
            .unwrap();

        let other = store
            .adopt_or_create_account("autre@exemple.fr", "gmail")
            .unwrap();
        store.create_mailbox(other, "INBOX", 1).unwrap();

        assert_eq!(store.invitation(other, "INBOX", 1).unwrap(), None);
    }

    #[test]
    fn reset_mailbox_clears_bodies_too() {
        let (store, id) = store_with_mailbox();
        store.save_body(id, 1, "<p>x</p>", &[]).unwrap();
        store.reset_mailbox(id, 2).unwrap();
        assert_eq!(store.body(test_account(&store), "INBOX", 1).unwrap(), None);
    }

    #[test]
    fn envelope_returns_reply_context_fields() {
        let (mut store, id) = store_with_mailbox();
        let original = envelope(7, "subject", 100, false);
        store
            .upsert_envelopes(id, std::slice::from_ref(&original))
            .unwrap();

        assert_eq!(
            store.envelope(test_account(&store), "INBOX", 7).unwrap(),
            Some(original)
        );
        assert_eq!(
            store.envelope(test_account(&store), "INBOX", 99).unwrap(),
            None
        );
    }

    /// ADR 0011: on a FILE database, opening switches to WAL — and the
    /// mode persists, a legacy database in rollback mode is converted.
    /// This is what prevents "database is locked" when the progress
    /// gauge reads while a full synchronization writes — the first
    /// defect the field returned on ADR 0010.
    ///
    /// On a file database and not in memory, like the field: an
    /// in-memory database answers "memory" to this PRAGMA, and the test
    /// would validate a false model.
    #[test]
    fn a_file_database_opens_in_wal() {
        let path = std::env::temp_dir().join(format!("wind-test-wal-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);

        // A legacy database, born BEFORE WAL: rollback mode (delete).
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch("CREATE TABLE heritage (id INTEGER)")
                .unwrap();
        }

        {
            let _store = Store::open(&path).unwrap();
            let conn = Connection::open(&path).unwrap();
            let mode: String = conn
                .query_row("PRAGMA journal_mode", [], |row| row.get(0))
                .unwrap();
            assert_eq!(
                mode.to_lowercase(),
                "wal",
                "the legacy database is converted"
            );
        }
        let _ = std::fs::remove_file(&path);
    }

    /// Field STOP 2 PLAN-AUDIT-V2 (2026-09-02): on the real database,
    /// "table envelopes has no column named reply_to" on every pass of
    /// the watcher — the column lived in the CREATE TABLE, never in the
    /// list of migrated columns; the e2e fixtures, freshly seeded, could
    /// not see it. A database from before wave 2 receives the column at
    /// reopening, and a poll writes to it.
    #[test]
    fn a_database_from_before_wave_2_receives_the_reply_to_column() {
        let path =
            std::env::temp_dir().join(format!("wind-test-reply-to-migr-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        drop(Store::open(&path).unwrap());
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch("ALTER TABLE envelopes DROP COLUMN reply_to")
                .unwrap();
        }
        Store::forget_initialization(&path);

        let mut store = Store::open(&path).unwrap();
        let account = test_account(&store);
        let mailbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        let mut list = envelope(1, "List", 100, false);
        list.reply_to = Some("liste@exemple.fr".to_string());
        store.upsert_envelopes(mailbox, &[list]).unwrap();
        assert_eq!(
            store.reply_to_of(account, "INBOX", 1).unwrap(),
            Some("liste@exemple.fr".to_string())
        );
        let _ = std::fs::remove_file(&path);
    }

    /// PLAN-AUDIT-V2 E1: every shell command opens ITS OWN connection —
    /// 103 sites — and each one replayed the schema, some twenty
    /// `table_xinfo` calls and the migrations (36 ms on 200k envelopes,
    /// on EVERY command). Once the full initialization has SUCCEEDED on
    /// a path, subsequent openings of the same process do not replay it.
    /// Proof without a spy in production code: an index is removed
    /// behind the Store's back; if the schema were replayed,
    /// `CREATE INDEX IF NOT EXISTS` would recreate it.
    #[test]
    fn a_second_opening_of_the_same_path_does_not_replay_the_schema() {
        let path =
            std::env::temp_dir().join(format!("wind-test-porte-rapide-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        drop(Store::open(&path).unwrap());

        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch("DROP INDEX idx_pending_actions_message")
                .unwrap();
        }
        drop(Store::open(&path).unwrap());

        let conn = Connection::open(&path).unwrap();
        let recreated: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_pending_actions_message'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(recreated, 0, "the second opening replayed the schema");
        let _ = std::fs::remove_file(&path);
    }

    /// Rebuilding the search index must make the migration screen show
    /// (ADR 0012) even on a database ALREADY up to date on the thread
    /// side: without this detection in `pending_adoption`, it would
    /// freeze the startup in silence (field finding 2026-08-17). On a
    /// file database, because the probe opens read-only — an in-memory
    /// database has no path.
    #[test]
    fn pending_adoption_sees_an_old_search_index() {
        let path =
            std::env::temp_dir().join(format!("wind-test-search-migr-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let mut store = Store::open(&path).unwrap();
            let account = test_account(&store);
            let mailbox = store.create_mailbox(account, "INBOX", 1).unwrap();
            store
                .upsert_envelopes(mailbox, &[envelope(1, "Subject", 100, false)])
                .unwrap();
            // Downgrades the index to the old three-column schema: the
            // threads stay adopted (`user_version` unchanged), only the
            // index predates this job — exactly the field's state.
            store
                .conn()
                .execute_batch(
                    "DROP TABLE search_fts;
                     DROP TABLE search_docs;
                     CREATE TABLE search_docs (
                        docid      INTEGER PRIMARY KEY,
                        mailbox_id INTEGER NOT NULL,
                        uid        INTEGER NOT NULL,
                        UNIQUE (mailbox_id, uid)
                     );
                     CREATE VIRTUAL TABLE search_fts USING fts5(
                        subject, sender, body,
                        content='', contentless_delete=1,
                        tokenize='unicode61 remove_diacritics 2'
                     );",
                )
                .unwrap();
        } // clean close -> WAL checkpoint, the read-only probe reads.

        assert_eq!(
            Store::pending_adoption(&path).unwrap(),
            Some(1),
            "the old FTS schema makes the screen show, threads already adopted"
        );

        // A full opening rebuilds it; after that, nothing left to report.
        Store::forget_initialization(&path);
        {
            Store::open(&path).unwrap();
        }
        assert_eq!(
            Store::pending_adoption(&path).unwrap(),
            None,
            "rebuilt -> the screen does not show again"
        );
        let _ = std::fs::remove_file(&path);
    }

    /// A Phase 1 database (without the reply columns) must open and
    /// enrich itself without losing the already-synced envelopes.
    #[test]
    fn opens_and_migrates_a_phase1_database() {
        let path =
            std::env::temp_dir().join(format!("wind-test-migration-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE mailboxes (
                    id             INTEGER PRIMARY KEY,
                    name           TEXT NOT NULL UNIQUE,
                    uid_validity   INTEGER NOT NULL,
                    last_uid       INTEGER NOT NULL DEFAULT 0,
                    highest_modseq INTEGER
                );
                CREATE TABLE envelopes (
                    mailbox_id INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
                    uid        INTEGER NOT NULL,
                    subject    TEXT,
                    sender     TEXT,
                    date_epoch INTEGER,
                    seen       INTEGER NOT NULL DEFAULT 0,
                    PRIMARY KEY (mailbox_id, uid)
                );
                INSERT INTO mailboxes (id, name, uid_validity) VALUES (1, 'INBOX', 1);
                INSERT INTO envelopes (mailbox_id, uid, subject, sender, date_epoch, seen)
                VALUES (1, 42, 'inherited from phase 1', 'Alice', 100, 1);",
            )
            .unwrap();
        }

        let store = Store::open(&path).unwrap();
        let rows = recent(&store, 0, 10);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].uid, 42);
        assert_eq!(rows[0].subject.as_deref(), Some("inherited from phase 1"));
        assert_eq!(
            rows[0].sender_address, None,
            "column added by migration: value unknown for the existing row"
        );
        assert!(!rows[0].flagged, "star absent by default after migration");

        drop(store);
        let _ = std::fs::remove_file(&path);
    }

    /// R2 (PLAN-RETOURS-MAIL): an envelope synced BEFORE the fix carries
    /// IMAP backslash-escapes in its subject and its sender name; the
    /// migration removes them once. The field case "Test \"Sent\"".
    #[test]
    fn migration_removes_the_imap_escapes_from_existing_subjects() {
        let path =
            std::env::temp_dir().join(format!("wind-test-escapes-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE mailboxes (
                    id             INTEGER PRIMARY KEY,
                    name           TEXT NOT NULL UNIQUE,
                    uid_validity   INTEGER NOT NULL,
                    last_uid       INTEGER NOT NULL DEFAULT 0,
                    highest_modseq INTEGER
                );
                CREATE TABLE envelopes (
                    mailbox_id     INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
                    uid            INTEGER NOT NULL,
                    subject        TEXT,
                    sender         TEXT,
                    sender_address TEXT,
                    date_epoch     INTEGER,
                    seen           INTEGER NOT NULL DEFAULT 0,
                    PRIMARY KEY (mailbox_id, uid)
                );
                INSERT INTO mailboxes (id, name, uid_validity) VALUES (1, 'INBOX', 1);",
            )
            .unwrap();
            conn.execute(
                "INSERT INTO envelopes
                    (mailbox_id, uid, subject, sender, sender_address, date_epoch, seen)
                 VALUES (1, 7, ?1, ?2, ?3, 100, 1)",
                params![r#"Test \"Sent\""#, r#"Company \"ACME\""#, "info@acme.fr"],
            )
            .unwrap();
            // A clean subject, without escapes: it must pass through intact.
            conn.execute(
                "INSERT INTO envelopes (mailbox_id, uid, subject, sender, date_epoch, seen)
                 VALUES (1, 8, 'Meeting tomorrow', 'Alice', 90, 1)",
                [],
            )
            .unwrap();
        }

        let store = Store::open(&path).unwrap();
        let rows = recent(&store, 0, 10);
        let seven = rows.iter().find(|e| e.uid == 7).unwrap();
        assert_eq!(seven.subject.as_deref(), Some(r#"Test "Sent""#));
        assert_eq!(seven.sender.as_deref(), Some(r#"Company "ACME""#));
        let eight = rows.iter().find(|e| e.uid == 8).unwrap();
        assert_eq!(eight.subject.as_deref(), Some("Meeting tomorrow"));

        drop(store);
        let _ = std::fs::remove_file(&path);
    }

    /// Phase 2 → 3 migration on a full database: all the data
    /// (envelopes, bodies, actions, drafts, tombstones, outbox)
    /// are adopted by the pending account — zero loss, and the first
    /// connection claims everything.
    #[test]
    fn migrates_a_full_phase2_database_and_adopts_everything() {
        let path =
            std::env::temp_dir().join(format!("wind-test-migration-p2-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE mailboxes (
                    id             INTEGER PRIMARY KEY,
                    name           TEXT NOT NULL UNIQUE,
                    uid_validity   INTEGER NOT NULL,
                    last_uid       INTEGER NOT NULL DEFAULT 0,
                    highest_modseq INTEGER
                );
                CREATE TABLE envelopes (
                    mailbox_id     INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
                    uid            INTEGER NOT NULL,
                    subject        TEXT,
                    sender         TEXT,
                    sender_address TEXT,
                    message_id     TEXT,
                    date_epoch     INTEGER,
                    seen           INTEGER NOT NULL DEFAULT 0,
                    flagged        INTEGER NOT NULL DEFAULT 0,
                    PRIMARY KEY (mailbox_id, uid)
                );
                CREATE TABLE bodies (
                    mailbox_id INTEGER NOT NULL,
                    uid        INTEGER NOT NULL,
                    html       TEXT NOT NULL,
                    PRIMARY KEY (mailbox_id, uid)
                );
                CREATE TABLE pending_actions (
                    id INTEGER PRIMARY KEY, mailbox_id INTEGER NOT NULL,
                    uid INTEGER NOT NULL, kind TEXT NOT NULL
                );
                CREATE TABLE drafts (
                    id INTEGER PRIMARY KEY, to_raw TEXT NOT NULL,
                    subject TEXT NOT NULL, body TEXT NOT NULL,
                    reply_to_uid INTEGER, updated_epoch INTEGER NOT NULL,
                    remote_uid INTEGER, pushed_epoch INTEGER
                );
                CREATE TABLE draft_tombstones (remote_uid INTEGER PRIMARY KEY);
                CREATE TABLE drafts_remote (
                    id INTEGER PRIMARY KEY CHECK (id = 1),
                    uid_validity INTEGER NOT NULL
                );
                CREATE TABLE outbox (
                    id INTEGER PRIMARY KEY, message_id TEXT NOT NULL,
                    sender TEXT NOT NULL, recipients TEXT NOT NULL,
                    subject TEXT NOT NULL, body_text TEXT NOT NULL,
                    in_reply_to TEXT, state TEXT NOT NULL DEFAULT 'queued',
                    attempts INTEGER NOT NULL DEFAULT 0, last_error TEXT,
                    queued_epoch INTEGER NOT NULL
                );
                INSERT INTO mailboxes (id, name, uid_validity) VALUES (1, 'INBOX', 7);
                INSERT INTO envelopes (mailbox_id, uid, subject, seen, flagged)
                    VALUES (1, 42, 'legacy', 1, 1);
                INSERT INTO bodies VALUES (1, 42, '<p>body</p>');
                INSERT INTO pending_actions (mailbox_id, uid, kind) VALUES (1, 42, 'mark_seen');
                INSERT INTO drafts (to_raw, subject, body, updated_epoch, remote_uid, pushed_epoch)
                    VALUES ('x@y.fr', 'precious', 'text', 10, 77, 10);
                INSERT INTO draft_tombstones VALUES (99);
                INSERT INTO drafts_remote VALUES (1, 1234);
                INSERT INTO outbox (message_id, sender, recipients, subject, body_text, queued_epoch)
                    VALUES ('<m@x>', 'me@y.fr', 'you@y.fr', 's', 'b', 20);",
            )
            .unwrap();
        }

        let store = Store::open(&path).unwrap();
        let account = store
            .adopt_or_create_account("legacy@example.fr", "gmail")
            .unwrap();
        assert_eq!(account, 1, "claiming takes over the pending account");

        assert_eq!(store.recent(account, "INBOX", 0, 10).unwrap()[0].uid, 42);
        assert_eq!(
            store.body(1, "INBOX", 42).unwrap().as_deref(),
            Some("<p>body</p>")
        );
        let drafts = store.drafts().unwrap();
        assert_eq!(drafts[0].account_id, 1);
        assert_eq!(drafts[0].remote_uid, Some(77));
        assert_eq!(store.draft_tombstones(1).unwrap(), vec![99]);
        assert!(
            !store.align_drafts_uidvalidity(1, 1234).unwrap(),
            "the drafts' UIDVALIDITY survived: no reset"
        );
        assert_eq!(store.outbox_to_send(1).unwrap().len(), 1);
        assert_eq!(store.accounts().unwrap().len(), 1);

        let second = store
            .adopt_or_create_account("two@example.fr", "gmail")
            .unwrap();
        assert_ne!(second, 1, "the placeholder is claimed only once");

        drop(store);
        let _ = std::fs::remove_file(&path);
    }

    /// PLAN-COMPOSITION-HTML E1: a legacy database (from before HTML
    /// bodies) gains the `body_html` columns of `drafts` and `outbox` on
    /// open — NULL on existing rows, the text path untouched.
    /// On a FILE database: it is the real migration pass that is proved,
    /// not a fresh schema (invariant #7).
    #[test]
    fn legacy_database_gains_body_html_columns_with_null_on_existing_rows() {
        let path =
            std::env::temp_dir().join(format!("wind-test-body-html-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE drafts (
                    id INTEGER PRIMARY KEY, to_raw TEXT NOT NULL,
                    subject TEXT NOT NULL, body TEXT NOT NULL,
                    reply_to_uid INTEGER, updated_epoch INTEGER NOT NULL,
                    remote_uid INTEGER, pushed_epoch INTEGER
                );
                CREATE TABLE outbox (
                    id INTEGER PRIMARY KEY, message_id TEXT NOT NULL,
                    sender TEXT NOT NULL, recipients TEXT NOT NULL,
                    subject TEXT NOT NULL, body_text TEXT NOT NULL,
                    in_reply_to TEXT, state TEXT NOT NULL DEFAULT 'queued',
                    attempts INTEGER NOT NULL DEFAULT 0, last_error TEXT,
                    queued_epoch INTEGER NOT NULL
                );
                INSERT INTO drafts (to_raw, subject, body, updated_epoch)
                    VALUES ('x@y.fr', 's', 'plain text', 10);
                INSERT INTO outbox (message_id, sender, recipients, subject, body_text, queued_epoch)
                    VALUES ('<m@x>', 'me@y.fr', 'you@y.fr', 's', 'b', 20);",
            )
            .unwrap();
        }

        let store = Store::open(&path).unwrap();
        for table in ["drafts", "outbox"] {
            assert!(
                table_columns(store.conn(), table)
                    .unwrap()
                    .contains("body_html"),
                "{table} must gain body_html on open"
            );
        }
        let old: Option<String> = store
            .conn()
            .query_row("SELECT body_html FROM drafts", [], |row| row.get(0))
            .unwrap();
        assert_eq!(old, None, "existing rows stay NULL: text path untouched");
        let old: Option<String> = store
            .conn()
            .query_row("SELECT body_html FROM outbox", [], |row| row.get(0))
            .unwrap();
        assert_eq!(old, None);

        drop(store);
        let _ = std::fs::remove_file(&path);
    }

    /// The core deliverable of multi-account: the same mailbox for every
    /// account, merged by date — each row knows its own account.
    #[test]
    fn unified_recent_merges_accounts_by_date() {
        let store = Store::open_in_memory().unwrap();
        let first = store
            .adopt_or_create_account("a@example.fr", "gmail")
            .unwrap();
        let second = store
            .adopt_or_create_account("b@example.fr", "gmail")
            .unwrap();
        let inbox_a = store.create_mailbox(first, "INBOX", 1).unwrap();
        let inbox_b = store.create_mailbox(second, "INBOX", 1).unwrap();

        let mut store = store;
        store
            .upsert_envelopes(
                inbox_a,
                &[
                    envelope(1, "a-old", 100, false),
                    envelope(2, "a-recent", 300, false),
                ],
            )
            .unwrap();
        store
            .upsert_envelopes(
                inbox_b,
                &[
                    envelope(1, "b-middle", 200, false),
                    envelope(2, "b-last", 400, false),
                ],
            )
            .unwrap();

        let rows = store.unified_recent(0, 10).unwrap();
        let order: Vec<(&str, &str)> = rows
            .iter()
            .map(|row| {
                (
                    row.account_email.as_str(),
                    row.envelope.subject.as_deref().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            order,
            vec![
                ("b@example.fr", "b-last"),
                ("a@example.fr", "a-recent"),
                ("b@example.fr", "b-middle"),
                ("a@example.fr", "a-old"),
            ],
            "merged by date, each row carries its account"
        );
        assert_eq!(store.unified_count().unwrap(), 4);
        // Same UID in two accounts: two distinct messages.
        assert!(store.envelope(first, "INBOX", 1).unwrap().is_some());
        assert!(store.envelope(second, "INBOX", 1).unwrap().is_some());
    }

    #[test]
    fn remove_absent_drops_orphaned_bodies() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(id, &[envelope(1, "a", 100, false)])
            .unwrap();
        store.save_body(id, 1, "<p>x</p>", &[]).unwrap();
        assert_eq!(store.remove_absent(id, &HashSet::new()).unwrap(), 1);
        assert_eq!(store.body(test_account(&store), "INBOX", 1).unwrap(), None);
    }

    /// `corps-fffd` repair: a body mutilated at decoding time (U+FFFD) is
    /// purged so that the backfill redownloads it with the fixed decoder;
    /// a healthy body is left in place.
    #[test]
    fn the_corps_fffd_repair_purges_mutilated_bodies() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[envelope(1, "a", 100, false), envelope(2, "b", 100, false)],
            )
            .unwrap();
        store.save_body(id, 1, "<p>tod\u{FFFD}ay</p>", &[]).unwrap();
        store.save_body(id, 2, "<p>healthy</p>", &[]).unwrap();
        // Simulates a database from before the repair: the marker
        // disappears, and the migration replays as on the next open.
        store
            .conn()
            .execute("DELETE FROM reparations WHERE nom = 'corps-fffd'", [])
            .unwrap();
        migrate(store.conn(), &mut |_| ControlFlow::Continue(())).unwrap();
        let account = test_account(&store);
        assert_eq!(
            store.body(account, "INBOX", 1).unwrap(),
            None,
            "mutilated body purged"
        );
        assert!(
            store.body(account, "INBOX", 2).unwrap().is_some(),
            "healthy body kept"
        );
        // The purged message becomes a backfill target again.
        assert_eq!(
            store.bodies_to_backfill(account, "INBOX", 0, 10).unwrap(),
            vec![1]
        );
    }

    /// Regression (bug #2): re-adding an already-known generic account
    /// must return the SAME id and apply the new configuration.
    /// On the UPDATE path of the upsert, `last_insert_rowid()` used to
    /// return 0 — a phantom id that the UI picked up for the badge and
    /// the selection. Each command opens ITS OWN connection: so the
    /// re-add is modeled with two distinct `Store`s on the same file
    /// database, because it is the fresh connection (with no prior
    /// INSERT) that takes the UPDATE path and exhibits the 0.
    #[test]
    fn re_adding_a_generic_account_returns_the_same_id_and_updates_config() {
        let path = std::env::temp_dir().join(format!(
            "wind-test-generic-{}-{}.sqlite",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let first = {
            let store = Store::open(&path).unwrap();
            store
                .create_generic_account(
                    "account@example.fr",
                    "account",
                    "imap.a.fr",
                    993,
                    "smtp.a.fr",
                    465,
                )
                .unwrap()
        };
        let second = {
            let store = Store::open(&path).unwrap();
            store
                .create_generic_account(
                    "account@example.fr",
                    "login",
                    "imap.b.fr",
                    143,
                    "smtp.b.fr",
                    587,
                )
                .unwrap()
        };
        let (count, config) = {
            let store = Store::open(&path).unwrap();
            (
                store.accounts().unwrap().len(),
                store.account_config(first).unwrap(),
            )
        };
        // Cleanup before the assertions: a failure must not leave a
        // temporary file behind.
        let _ = std::fs::remove_file(&path);

        assert!(first > 0, "the first creation must return a real id");
        assert_eq!(
            second, first,
            "re-adding must return the existing id, never 0"
        );
        assert_eq!(count, 1, "a single account, no duplicate");
        assert_eq!(config.username.as_deref(), Some("login"));
        assert_eq!(config.imap_host.as_deref(), Some("imap.b.fr"));
        assert_eq!(config.imap_port, Some(143));
        assert_eq!(config.smtp_host.as_deref(), Some("smtp.b.fr"));
        assert_eq!(config.smtp_port, Some(587));
    }

    /// The backfill targets RECENT bodyless messages, newest first: this
    /// is the order in which search gains the most value, and the one
    /// that makes resuming after an interruption feel natural.
    #[test]
    fn backfill_lists_recent_bodyless_messages_newest_first() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "old", 1_000, false),
                    envelope(2, "middle", 2_000, false),
                    envelope(3, "recent", 3_000, false),
                ],
            )
            .unwrap();
        let account = test_account(&store);

        let todo = store.bodies_to_backfill(account, "INBOX", 0, 10).unwrap();
        assert_eq!(todo, vec![3, 2, 1]);
    }

    #[test]
    fn backfill_skips_messages_that_already_have_a_body() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "without body", 1_000, false),
                    envelope(2, "with body", 2_000, false),
                ],
            )
            .unwrap();
        store.save_body(id, 2, "<p>already there</p>", &[]).unwrap();
        let account = test_account(&store);

        assert_eq!(
            store.bodies_to_backfill(account, "INBOX", 0, 10).unwrap(),
            vec![1]
        );
    }

    /// The recency horizon is what BOUNDS the cost (ADR 0007): beyond it,
    /// nothing is fetched back.
    #[test]
    fn backfill_respects_the_recency_horizon() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "outside the horizon", 1_000, false),
                    envelope(2, "inside the horizon", 5_000, false),
                ],
            )
            .unwrap();
        let account = test_account(&store);

        assert_eq!(
            store
                .bodies_to_backfill(account, "INBOX", 4_000, 10)
                .unwrap(),
            vec![2]
        );
    }

    #[test]
    fn backfill_honours_the_batch_limit() {
        let (mut store, id) = store_with_mailbox();
        let envelopes: Vec<Envelope> = (1..=10)
            .map(|uid| envelope(uid, "message", uid as i64 * 100, false))
            .collect();
        store.upsert_envelopes(id, &envelopes).unwrap();
        let account = test_account(&store);

        assert_eq!(
            store
                .bodies_to_backfill(account, "INBOX", 0, 3)
                .unwrap()
                .len(),
            3
        );
    }

    #[test]
    fn backfill_never_leaks_another_accounts_messages() {
        let (mut store, mine) = store_with_mailbox();
        let other = store
            .adopt_or_create_account("other@example.fr", "gmail")
            .unwrap();
        let theirs = store.create_mailbox(other, "INBOX", 1).unwrap();
        store
            .upsert_envelopes(mine, &[envelope(1, "mine", 1_000, false)])
            .unwrap();
        store
            .upsert_envelopes(theirs, &[envelope(1, "someone else's", 2_000, false)])
            .unwrap();
        let account = test_account(&store);

        assert_eq!(
            store.bodies_to_backfill(account, "INBOX", 0, 10).unwrap(),
            vec![1],
            "a single message: the one belonging to the requested account"
        );
        assert_eq!(
            store.bodies_to_backfill(other, "INBOX", 0, 10).unwrap(),
            vec![1]
        );
    }

    // -----------------------------------------------------------------
    // Grouping into conversations
    // -----------------------------------------------------------------

    /// A reply to `parent`, in the format of [`envelope`] — whose
    /// `Message-ID` is `<m{uid}@example.com>`.
    fn reply(uid: Uid, subject: &str, epoch: i64, seen: bool, parent: Uid) -> Envelope {
        Envelope {
            in_reply_to: Some(format!("<m{parent}@example.com>")),
            ..envelope(uid, subject, epoch, seen)
        }
    }

    fn unified(store: &Store) -> Vec<UnifiedRow> {
        store.unified_recent(0, 50).unwrap()
    }

    fn uids(rows: &[UnifiedRow]) -> Vec<Uid> {
        rows.iter().map(|row| row.envelope.uid).collect()
    }

    /// The heart of the job: two messages, a single row.
    #[test]
    fn the_list_shows_one_row_per_conversation() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "Quote", 100, true),
                    reply(2, "Re: Quote", 200, true, 1),
                ],
            )
            .unwrap();

        let rows = unified(&store);
        assert_eq!(rows.len(), 1, "one thread, not two messages");
        assert_eq!(rows[0].thread_size, 2);
        assert_eq!(rows[0].envelope.uid, 2, "the row shows the LAST message");
        assert_eq!(
            store.unified_count().unwrap(),
            1,
            "scrolling counts conversations, otherwise it scrolls into thin air"
        );
    }

    #[test]
    fn a_reply_brings_the_whole_thread_back_up() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "Quote", 100, true),
                    envelope(2, "Invoice", 200, true),
                ],
            )
            .unwrap();
        assert_eq!(uids(&unified(&store)), vec![2, 1]);

        store
            .upsert_envelopes(id, &[reply(3, "Re: Quote", 300, true, 1)])
            .unwrap();

        let rows = unified(&store);
        assert_eq!(
            uids(&rows),
            vec![3, 2],
            "the quote moves back ahead of the invoice"
        );
        assert_eq!(rows[0].thread_size, 2);
    }

    /// A thread whose last message is read, but which still holds an
    /// unread message higher up, must stay bold. Reading the state of
    /// only the displayed message would give the opposite answer.
    #[test]
    fn a_thread_stays_unread_while_any_of_its_messages_is() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "Quote", 100, false),
                    reply(2, "Re: Quote", 200, true, 1),
                ],
            )
            .unwrap();

        let rows = unified(&store);
        assert!(rows[0].envelope.seen, "the last message is read…");
        assert_eq!(
            rows[0].thread_unseen, 1,
            "…but the thread still holds an unread one"
        );

        store.set_seen_local(id, 1, true).unwrap();
        assert_eq!(
            unified(&store)[0].thread_unseen,
            0,
            "reading the missing message clears the thread"
        );
    }

    /// The case that justifies the pass over full headers: in an inbox,
    /// the middle message of an exchange is the one WE sent — it isn't
    /// there. `In-Reply-To` alone therefore leaves two threads;
    /// `References`, which also carries the root, glues them back
    /// together.
    #[test]
    fn references_glue_two_thread_halves_back_together() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "Quote", 100, true),
                    // Replies to <m2@…>: our own reply, absent.
                    reply(3, "Re: Quote", 300, true, 2),
                ],
            )
            .unwrap();
        assert_eq!(
            unified(&store).len(),
            2,
            "two threads, for lack of the missing link"
        );

        assert!(
            store
                .set_thread_headers(id, 3, None, "<m1@example.com> <m2@example.com>")
                .unwrap(),
            "the attachment changed"
        );

        let rows = unified(&store);
        assert_eq!(rows.len(), 1, "the two halves join back together");
        assert_eq!(rows[0].thread_size, 2);
        assert_eq!(rows[0].envelope.uid, 3);
    }

    /// A resync rewrites the envelope. If it overwrote the `References`
    /// already acquired, it would UNGROUP a glued thread: the grouping
    /// would silently come undone, with nothing to signal it. This is
    /// the trap that had cost us the attachments.
    #[test]
    fn a_resync_does_not_ungroup_a_glued_thread() {
        let (mut store, id) = store_with_mailbox();
        let arrival = [
            envelope(1, "Quote", 100, true),
            reply(3, "Re: Quote", 300, true, 2),
        ];
        store.upsert_envelopes(id, &arrival).unwrap();
        store
            .set_thread_headers(id, 3, None, "<m1@example.com> <m2@example.com>")
            .unwrap();
        assert_eq!(unified(&store).len(), 1);

        store.upsert_envelopes(id, &arrival).unwrap();

        let rows = unified(&store);
        assert_eq!(rows.len(), 1, "the thread holds through the resync");
        assert_eq!(rows[0].thread_size, 2);
    }

    /// The attachments trap applied to threads: a database from before
    /// grouping has `thread_id` NULL everywhere. The list starts from
    /// `threads` — without adoption, it would be EMPTY on the first
    /// open, and forever.
    #[test]
    fn a_legacy_database_sees_all_its_messages_adopted() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "Quote", 100, true),
                    envelope(2, "Invoice", 200, true),
                ],
            )
            .unwrap();

        // Rewind to the state of a database from before threads.
        store
            .conn()
            .execute_batch(
                "UPDATE envelopes SET thread_id = NULL;
                 DELETE FROM thread_links;
                 DELETE FROM threads;",
            )
            .unwrap();
        assert!(
            unified(&store).is_empty(),
            "without adoption, the entire mailbox disappears from the screen"
        );

        crate::thread::migrate_threads(store.conn()).unwrap();

        assert_eq!(uids(&unified(&store)), vec![2, 1]);
        assert_eq!(store.unified_count().unwrap(), 2);
    }

    /// Arrival order must change nothing: here the reply precedes its
    /// parent in the same batch.
    #[test]
    fn a_thread_reads_from_oldest_to_newest() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    reply(2, "Re: Quote", 200, true, 1),
                    envelope(1, "Quote", 100, true),
                ],
            )
            .unwrap();

        let rows = unified(&store);
        assert_eq!(rows.len(), 1, "arrival order does not break the thread");
        let thread = rows[0].thread_id.unwrap();
        let messages = store.thread_messages(thread).unwrap();
        assert_eq!(uids(&messages), vec![1, 2]);
        // Each message comes back knowing the size of ITS thread:
        // otherwise the screen that reopens it would conclude it's alone.
        assert!(messages.iter().all(|m| m.thread_size == 2));
    }

    #[test]
    fn removing_a_threads_messages_makes_it_disappear() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "Quote", 100, true),
                    reply(2, "Re: Quote", 200, true, 1),
                ],
            )
            .unwrap();

        store.remove_local(id, 2).unwrap();
        let rows = unified(&store);
        assert_eq!(
            uids(&rows),
            vec![1],
            "the thread falls back on what remains"
        );
        assert_eq!(rows[0].thread_size, 1);

        store.remove_local(id, 1).unwrap();
        assert!(unified(&store).is_empty());
        assert_eq!(store.unified_count().unwrap(), 0);
    }

    /// The field's own finding, end to end: two unrelated messages whose
    /// `In-Reply-To` is a SENTENCE — not an identifier — must remain two
    /// conversations.
    ///
    /// Before the fix, every word of the sentence became a shared anchor
    /// and merged them together. On a real mailbox this produced a
    /// 43-message thread with no relation between its messages.
    #[test]
    fn two_messages_whose_header_is_prose_do_not_merge() {
        let (mut store, id) = store_with_mailbox();
        let prose = "Your message of January 3rd";
        store
            .upsert_envelopes(
                id,
                &[
                    Envelope {
                        in_reply_to: Some(prose.to_string()),
                        ..envelope(1, "Promotion", 100, true)
                    },
                    Envelope {
                        in_reply_to: Some(prose.to_string()),
                        ..envelope(2, "Another promotion", 200, true)
                    },
                ],
            )
            .unwrap();

        assert_eq!(unified(&store).len(), 2, "no link between these two");
    }

    /// A database grouped by the old rule carries FALSE threads, and
    /// fixing the code does not repair them on its own. The version
    /// marker makes them redone on open — without a network, since the
    /// raw headers are intact in the database.
    #[test]
    fn a_badly_grouped_database_is_redone_on_open() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "Promotion", 100, true),
                    envelope(2, "Another promotion", 200, true),
                ],
            )
            .unwrap();
        assert_eq!(unified(&store).len(), 2);

        // Replays the state that the permissive rule used to produce: a
        // single thread for two unrelated messages, and the old version.
        store
            .conn()
            .execute_batch(
                "DELETE FROM thread_links WHERE thread_id = (SELECT MAX(id) FROM threads);
                 UPDATE envelopes SET thread_id = (SELECT MIN(id) FROM threads);
                 DELETE FROM threads WHERE id = (SELECT MAX(id) FROM threads);
                 UPDATE threads SET size = 2, last_uid = 2, last_epoch = 200;
                 PRAGMA user_version = 0;",
            )
            .unwrap();
        assert_eq!(
            unified(&store).len(),
            1,
            "the faulty state is correctly reproduced"
        );

        crate::thread::migrate_threads(store.conn()).unwrap();

        assert_eq!(unified(&store).len(), 2, "the threads are redone");
        let version: i64 = store
            .conn()
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        // Against the CONSTANT, never against a literal: every change to
        // the grouping rule increments it, and a hardcoded "1" would fail
        // this test for a reason that isn't its own.
        assert_eq!(
            version,
            crate::thread::THREADING_VERSION,
            "and the rebuild does not replay again"
        );
    }

    /// UIDVALIDITY invalidated: threads go with the rest, and the
    /// directory must not prevent a clean repopulation.
    #[test]
    fn reset_mailbox_also_clears_threads() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "Quote", 100, true),
                    reply(2, "Re: Quote", 200, true, 1),
                ],
            )
            .unwrap();
        store.reset_mailbox(id, 2).unwrap();
        assert!(unified(&store).is_empty());

        store
            .upsert_envelopes(id, &[envelope(1, "Quote", 100, true)])
            .unwrap();
        assert_eq!(
            unified(&store).len(),
            1,
            "the mailbox repopulates without a stop"
        );
    }

    /// Replays on `path` the tables as version 1 of threads created
    /// them — the only fixture where the adoption pass has real work to
    /// do. Shared by the open test below and by the rewind tests
    /// (Phase 5 job).
    fn rewind_to_schema_v1(path: &Path) {
        // A database rewound by hand is a database from BEFORE: the fast
        // path registry (E1) must no longer know about it.
        Store::forget_initialization(path);
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(
            "DROP TABLE thread_links;
             DROP TABLE threads;
             CREATE TABLE threads (
                 id         INTEGER PRIMARY KEY,
                 mailbox_id INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
                 last_uid   INTEGER NOT NULL DEFAULT 0,
                 last_epoch INTEGER,
                 size       INTEGER NOT NULL DEFAULT 0,
                 unseen     INTEGER NOT NULL DEFAULT 0
             );
             CREATE INDEX idx_threads_date
                 ON threads(mailbox_id, last_epoch DESC, last_uid DESC);
             CREATE TABLE thread_links (
                 mailbox_id INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
                 message_id TEXT NOT NULL,
                 thread_id  INTEGER NOT NULL REFERENCES threads(id) ON DELETE CASCADE,
                 PRIMARY KEY (mailbox_id, message_id)
             );
             CREATE INDEX idx_thread_links_thread ON thread_links(thread_id);
             UPDATE envelopes SET thread_id = NULL;
             PRAGMA user_version = 1;",
        )
        .unwrap();
    }

    /// Finding from the FIELD, not here: a database created by the
    /// previous version carries a `threads` table with no `inbox_size`.
    /// `CREATE TABLE IF NOT EXISTS` does not touch it — but the partial
    /// index does not exist yet, so SQLite really tries to create it:
    /// it fails on a missing column, and **the entire open is refused**
    /// ("no such column: inbox_size"). The app would no longer start.
    ///
    /// No test could catch it: they all create a fresh database, already
    /// on the current schema. This one REWINDS a real database to the
    /// previous schema — the only fixture where the defect exists.
    #[test]
    fn a_database_on_the_previous_threads_schema_opens_and_migrates() {
        let path =
            std::env::temp_dir().join(format!("wind-test-threads-v1-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);

        {
            let mut store = Store::open(&path).unwrap();
            let account = store
                .adopt_or_create_account("me@example.fr", "gmail")
                .unwrap();
            let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
            let mut first = envelope(1, "Quote", 100, true);
            first.message_id = Some("<a@example.fr>".to_string());
            let mut second = envelope(2, "Re: Quote", 200, true);
            second.message_id = Some("<b@example.fr>".to_string());
            second.in_reply_to = Some("<a@example.fr>".to_string());
            store.upsert_envelopes(inbox, &[first, second]).unwrap();
            assert_eq!(
                unified(&store).len(),
                1,
                "fixture: a thread of two messages"
            );
        }

        // Rewind: the tables as version 1 created them.
        rewind_to_schema_v1(&path);

        // This is the open that used to be refused.
        let store = Store::open(&path).unwrap();
        let rows = unified(&store);
        assert_eq!(rows.len(), 1, "the thread is redone, and the list shows it");
        assert_eq!(rows[0].thread_size, 2, "with its counter");

        drop(store);
        let _ = std::fs::remove_file(&path);
    }

    /// THE test of the Phase 5 job (handover §8): adoption is NOT
    /// splittable — the list starts from `threads`, a partially persisted
    /// adoption would be a half-empty mailbox. "Interruptible" therefore
    /// means: cancelling IN THE MIDDLE of the pass undoes EVERYTHING and
    /// leaves `user_version` unchanged, so the entire pass replays at the
    /// next launch — where the list is complete.
    #[test]
    fn cancelling_adoption_undoes_everything_and_leaves_user_version_unchanged() {
        let path = std::env::temp_dir().join(format!("wind-test-rewind-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);

        // Enough messages for the cancellation to land IN THE MIDDLE of
        // the pass: progress is reported in stages, one must be crossed.
        const MESSAGES: u32 = 1_200;
        {
            let mut store = Store::open(&path).unwrap();
            let account = store
                .adopt_or_create_account("me@example.fr", "gmail")
                .unwrap();
            let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
            let fixture: Vec<Envelope> = (1..=MESSAGES)
                .map(|uid| envelope(uid, "Subject", 100 + i64::from(uid), true))
                .collect();
            store.upsert_envelopes(inbox, &fixture).unwrap();
        }
        rewind_to_schema_v1(&path);

        // Cancel as soon as 1,000 messages have gone through — in the
        // middle, not at the gate's threshold: the rewind must undo real
        // work.
        let mut highest_done = 0;
        let result = Store::open_with_progress(&path, |p| {
            highest_done = highest_done.max(p.done);
            if p.done >= 1_000 {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        });
        assert!(
            matches!(result, Err(Error::Interrupted)),
            "cancelling must return Error::Interrupted, not a Store"
        );
        assert!(
            highest_done >= 1_000,
            "the fixture must exercise a cancellation IN PROGRESS \
             (highest reading: {highest_done})"
        );

        // Everything is undone: the database is back to the state
        // BEFORE the cancelled open.
        {
            let conn = Connection::open(&path).unwrap();
            let version: i64 = conn
                .query_row("PRAGMA user_version", [], |row| row.get(0))
                .unwrap();
            assert_eq!(version, 1, "user_version unchanged: the pass will replay");
            let new_shape: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('threads')
                     WHERE name = 'inbox_size'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(
                new_shape, 0,
                "the v1 table is intact: the DROP is rewound too"
            );
            let envelopes: i64 = conn
                .query_row("SELECT COUNT(*) FROM envelopes", [], |row| row.get(0))
                .unwrap();
            assert_eq!(envelopes, i64::from(MESSAGES), "no message lost");
        }

        // The next launch replays the WHOLE pass: complete list.
        {
            let store = Store::open(&path).unwrap();
            let threadless: i64 = store
                .conn()
                .query_row(
                    "SELECT COUNT(*) FROM envelopes WHERE thread_id IS NULL",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(threadless, 0, "every legacy message is adopted");
            let version: i64 = store
                .conn()
                .query_row("PRAGMA user_version", [], |row| row.get(0))
                .unwrap();
            assert_eq!(version, crate::thread::THREADING_VERSION);
        }
        let _ = std::fs::remove_file(&path);
    }

    /// Progress is OBSERVABLE (lesson §9): the total is announced up
    /// front and never moves again, progress never goes backwards, and
    /// "done" is only said at the end — never before.
    #[test]
    fn adoption_reports_its_progress_from_start_to_finish() {
        let path = std::env::temp_dir().join(format!(
            "wind-test-adoption-progress-{}.db",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        {
            let mut store = Store::open(&path).unwrap();
            let account = store
                .adopt_or_create_account("me@example.fr", "gmail")
                .unwrap();
            let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
            store
                .upsert_envelopes(
                    inbox,
                    &[
                        envelope(1, "Quote", 100, true),
                        reply(2, "Re: Quote", 200, true, 1),
                    ],
                )
                .unwrap();
        }
        rewind_to_schema_v1(&path);

        let mut reports: Vec<AdoptionProgress> = Vec::new();
        let store = Store::open_with_progress(&path, |p| {
            reports.push(p);
            ControlFlow::Continue(())
        })
        .unwrap();

        assert!(!reports.is_empty(), "a silent adoption is not observable");
        assert_eq!(reports[0].done, 0, "the start is announced right away");
        assert!(reports[0].total > 0, "the total is announced up front");
        for pair in reports.windows(2) {
            assert!(
                pair[1].done >= pair[0].done,
                "progress does not go backwards"
            );
            assert_eq!(
                pair[1].total, pair[0].total,
                "the total does not move mid-flight — a bar that goes \
                 backwards is worse than an imprecise bar"
            );
        }
        let last = reports.last().unwrap();
        assert_eq!(last.done, last.total, "the last report says \"done\"");
        assert!(
            reports[..reports.len() - 1]
                .iter()
                .all(|p| p.done < p.total),
            "and it is the ONLY one: never \"100%\" before the end"
        );

        let rows = unified(&store);
        assert_eq!(rows.len(), 1, "the thread is redone");
        assert_eq!(rows[0].thread_size, 2, "with its counter");
        drop(store);
        let _ = std::fs::remove_file(&path);
    }

    /// The probe answers without triggering anything: the desktop calls
    /// it BEFORE the first real open to decide whether to show the
    /// migration screen — if it migrated on its own, the screen would
    /// arrive after the fact.
    #[test]
    fn the_probe_says_when_an_adoption_is_pending_without_triggering_it() {
        let path = std::env::temp_dir().join(format!(
            "wind-test-probe-adoption-{}.db",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);

        // File absent: first install, nothing legacy — and the probe
        // must NOT create the file.
        assert_eq!(Store::pending_adoption(&path).unwrap(), None);
        assert!(!path.exists(), "a probe leaves no trace");

        {
            let mut store = Store::open(&path).unwrap();
            let account = store
                .adopt_or_create_account("me@example.fr", "gmail")
                .unwrap();
            let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
            store
                .upsert_envelopes(
                    inbox,
                    &[
                        envelope(1, "Quote", 100, true),
                        reply(2, "Re: Quote", 200, true, 1),
                    ],
                )
                .unwrap();
            // A message OUT OF SCOPE (ADR 0010 §3): the pass will never
            // adopt it, the probe must not announce it.
            let spam = store.create_mailbox(account, "Spam", 1).unwrap();
            store
                .upsert_envelopes(spam, &[envelope(1, "You won!", 300, true)])
                .unwrap();
        }
        // Up-to-date database: nothing to announce.
        assert_eq!(Store::pending_adoption(&path).unwrap(), None);

        rewind_to_schema_v1(&path);
        assert_eq!(
            Store::pending_adoption(&path).unwrap(),
            Some(2),
            "a legacy database announces its messages to adopt — the SCOPE, \
             not the whole database: a figure must name what it says"
        );
        // And NOTHING was triggered: the version has not moved.
        {
            let conn = Connection::open(&path).unwrap();
            let version: i64 = conn
                .query_row("PRAGMA user_version", [], |row| row.get(0))
                .unwrap();
            assert_eq!(version, 1, "the probe did not migrate on our behalf");
        }
        let _ = std::fs::remove_file(&path);
    }

    /// The language is restored BEFORE the first render, so BEFORE the
    /// migration screen (field finding 2026-08-15): reading it must be a
    /// read-only probe — with a full open, adopting a legacy database
    /// used to be paid for silently while loading the language, with no
    /// modal, no progress, no cancellation — everything ADR 0012
    /// forbids. The fixture REWINDS a real file database (invariant
    /// §6.7): the only one where the defect exists.
    #[test]
    fn the_language_reads_without_adopting_the_database() {
        let path = std::env::temp_dir().join(format!(
            "wind-test-language-probe-{}.db",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);

        // File absent: first install — and the probe must NOT create the
        // file.
        assert_eq!(Store::text_pref_readonly(&path, "lang").unwrap(), None);
        assert!(!path.exists(), "a probe leaves no trace");

        {
            let mut store = Store::open(&path).unwrap();
            let account = store
                .adopt_or_create_account("me@example.fr", "gmail")
                .unwrap();
            let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
            store
                .upsert_envelopes(
                    inbox,
                    &[
                        envelope(1, "Quote", 100, true),
                        reply(2, "Re: Quote", 200, true, 1),
                    ],
                )
                .unwrap();
            store.set_text_pref("lang", "en").unwrap();
        }
        rewind_to_schema_v1(&path);

        // The preference reads back…
        assert_eq!(
            Store::text_pref_readonly(&path, "lang").unwrap(),
            Some("en".to_string())
        );
        // …and NOTHING was triggered: the version has not moved, the
        // modal will still find the adoption pending.
        {
            let conn = Connection::open(&path).unwrap();
            let version: i64 = conn
                .query_row("PRAGMA user_version", [], |row| row.get(0))
                .unwrap();
            assert_eq!(
                version, 1,
                "reading the language did not migrate on our behalf"
            );
        }
        assert_eq!(
            Store::pending_adoption(&path).unwrap(),
            Some(2),
            "the migration screen still has a reason to exist"
        );

        // A legacy database from before WAL lives in rollback (delete)
        // mode — the real shape found in the field, not the one
        // `Store::open` leaves behind: the probe must answer there too.
        Connection::open(&path)
            .unwrap()
            .query_row("PRAGMA journal_mode = delete", [], |row| {
                row.get::<_, String>(0)
            })
            .unwrap();
        assert_eq!(
            Store::text_pref_readonly(&path, "lang").unwrap(),
            Some("en".to_string()),
            "the probe also answers on a database in rollback mode"
        );

        // A database from before preferences (no `prefs` table): the
        // probe answers "no preference", it does not fail.
        Connection::open(&path)
            .unwrap()
            .execute_batch("DROP TABLE prefs")
            .unwrap();
        assert_eq!(Store::text_pref_readonly(&path, "lang").unwrap(), None);
        let _ = std::fs::remove_file(&path);
    }

    /// On an up-to-date database there is NOTHING to adopt — and so
    /// nothing to say. A migration banner on every launch would be a
    /// false signal, and every desktop command opens its own connection.
    #[test]
    fn an_up_to_date_database_opens_without_announcing_a_migration() {
        let path = std::env::temp_dir().join(format!(
            "wind-test-silent-adoption-{}.db",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        {
            let mut store = Store::open(&path).unwrap();
            let account = store
                .adopt_or_create_account("me@example.fr", "gmail")
                .unwrap();
            let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
            store
                .upsert_envelopes(
                    inbox,
                    &[
                        envelope(1, "Quote", 100, true),
                        reply(2, "Re: Quote", 200, true, 1),
                    ],
                )
                .unwrap();
        }

        let mut calls = 0;
        let store = Store::open_with_progress(&path, |_| {
            calls += 1;
            ControlFlow::Continue(())
        })
        .unwrap();
        assert_eq!(calls, 0, "nothing to adopt, nothing to report");
        assert_eq!(unified(&store).len(), 1, "and the list is there");
        drop(store);
        let _ = std::fs::remove_file(&path);
    }

    /// THE point of the [ADR 0009] job: a received message and the reply
    /// we made to it belong to the same exchange, so the same thread —
    /// even though they live in **two different mailboxes**.
    ///
    /// Before, threads were siloed by mailbox: this reply would have
    /// formed its own thread in its own id space, and syncing "Sent"
    /// would have cost without paying anything back.
    ///
    /// The fixture deliberately gives the same UID (1) to both messages:
    /// a message's identity is `(account, mailbox, UID)`, and any
    /// grouping that confused two equal UIDs would show up here.
    #[test]
    fn a_reply_in_sent_joins_the_received_messages_thread() {
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("me@example.fr", "gmail")
            .unwrap();
        let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        let sent = store.create_mailbox(account, "Sent", 1).unwrap();
        // The fixture must DECLARE the scope it exercises: since ADR
        // 0010, a mailbox only groups if it has been told to, and the
        // name of the sent folder varies from one server to the next.
        store.set_thread_scope(account, Some("Sent")).unwrap();

        // Alice writes.
        let mut received = envelope(1, "Quote", 100, true);
        received.message_id = Some("<alice-1@example.fr>".to_string());
        store.upsert_envelopes(inbox, &[received]).unwrap();

        // I reply: the message goes into "Sent" and quotes the first one.
        let mut reply = envelope(1, "Re: Quote", 200, true);
        reply.message_id = Some("<me-1@example.fr>".to_string());
        reply.in_reply_to = Some("<alice-1@example.fr>".to_string());
        store.upsert_envelopes(sent, &[reply]).unwrap();

        let rows = unified(&store);
        assert_eq!(rows.len(), 1, "a single thread, not two");
        assert_eq!(
            rows[0].thread_size, 2,
            "the counter covers the whole exchange, sent items included"
        );
        assert_eq!(
            rows[0].envelope.subject.as_deref(),
            Some("Re: Quote"),
            "the thread is represented by its most recent message, \
             even when it is our own reply"
        );
    }

    /// Two messages from the SAME account can carry the SAME UID as soon
    /// as they live in two mailboxes — this is the rule, not the
    /// exception, since UIDs are assigned per mailbox and restart at 1.
    ///
    /// Each row must therefore say **where it lives**. Without this,
    /// opening our reply from the conversation banner would display the
    /// received message in its place, and mark it read — invariant §6.2
    /// of the handover, amended here for two mailboxes.
    #[test]
    fn each_row_says_which_mailbox_it_lives_in() {
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("me@example.fr", "gmail")
            .unwrap();
        let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        let sent = store.create_mailbox(account, "Sent", 1).unwrap();
        // The fixture must DECLARE the scope it exercises: since ADR
        // 0010, a mailbox only groups if it has been told to, and the
        // name of the sent folder varies from one server to the next.
        store.set_thread_scope(account, Some("Sent")).unwrap();

        let mut received = envelope(1, "Quote", 100, true);
        received.message_id = Some("<alice-9@example.fr>".to_string());
        store.upsert_envelopes(inbox, &[received]).unwrap();
        let mut reply = envelope(1, "Re: Quote", 200, true);
        reply.message_id = Some("<me-9@example.fr>".to_string());
        reply.in_reply_to = Some("<alice-9@example.fr>".to_string());
        store.upsert_envelopes(sent, &[reply]).unwrap();

        let thread = unified(&store)[0].thread_id.unwrap();
        let messages = store.thread_messages(thread).unwrap();

        assert_eq!(messages.len(), 2);
        assert!(
            messages.iter().all(|row| row.envelope.uid == 1),
            "the fixture does have two messages sharing the same UID: that's the whole point"
        );
        let mailboxes: Vec<&str> = messages.iter().map(|l| l.mailbox.as_str()).collect();
        assert!(
            mailboxes.contains(&"INBOX"),
            "mailboxes seen: {mailboxes:?}"
        );
        assert!(mailboxes.contains(&"Sent"), "mailboxes seen: {mailboxes:?}");
    }

    /// The other side of the same rule: writing to someone who never
    /// replies does NOT create a conversation in the inbox. This is what
    /// the `inbox_size` counter protects, and it is also what makes the
    /// partial index possible (ADR 0009 §2 and §4).
    #[test]
    fn a_purely_outgoing_thread_has_no_row() {
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("me@example.fr", "gmail")
            .unwrap();
        store.create_mailbox(account, "INBOX", 1).unwrap();
        let sent = store.create_mailbox(account, "Sent", 1).unwrap();
        // The fixture must DECLARE the scope it exercises: since ADR
        // 0010, a mailbox only groups if it has been told to, and the
        // name of the sent folder varies from one server to the next.
        store.set_thread_scope(account, Some("Sent")).unwrap();

        let mut outgoing = envelope(1, "My proposal", 100, true);
        outgoing.message_id = Some("<me-2@example.fr>".to_string());
        store.upsert_envelopes(sent, &[outgoing]).unwrap();

        assert!(
            unified(&store).is_empty(),
            "nothing was received: the inbox stays empty"
        );
        assert_eq!(store.unified_count().unwrap(), 0);
    }

    /// [ADR 0010] §3 — we STORE everything, we only GROUP within scope.
    ///
    /// Since [ADR 0009] a thread belongs to the ACCOUNT. As soon as full
    /// sync pours Archive, Trash and Spam into that same account, their
    /// messages would join threads **on their own** — and three
    /// aggregates would silently get corrupted, with no test to see it:
    ///
    /// - `size`: "12 messages" on a thread that shows 3;
    /// - `unseen`: a thread perpetually unread because of a spam message;
    /// - `last_epoch`: **the conversation jumps to the top of the list
    ///   because a spam message latched onto it**.
    ///
    /// The third is a CORRECTNESS defect: the list would lie about the
    /// order of exchanges, with no recourse for the user. Same reason
    /// for refusal as grouping by subject (ADR 0008 §2).
    ///
    /// The compiler protects nothing here — a mailbox is a string like
    /// any other (handover §6.2). It's this test that holds the
    /// invariant.
    #[test]
    fn a_message_out_of_scope_does_not_join_the_thread() {
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("me@example.fr", "gmail")
            .unwrap();
        let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        let spam = store.create_mailbox(account, "Spam", 1).unwrap();

        let mut received = envelope(1, "Quote", 100, true);
        received.message_id = Some("<alice-10@example.fr>".to_string());
        store.upsert_envelopes(inbox, &[received]).unwrap();

        // The spam message quotes the received message — exactly what
        // would make it join the thread. It is MORE RECENT and UNREAD:
        // if it got in, all three aggregates would move at once.
        let mut junk = envelope(1, "WIN 1000 EUROS", 300, false);
        junk.message_id = Some("<spam-1@elsewhere.example>".to_string());
        junk.in_reply_to = Some("<alice-10@example.fr>".to_string());
        store.upsert_envelopes(spam, &[junk]).unwrap();

        let rows = unified(&store);
        assert_eq!(rows.len(), 1, "a single thread");
        assert_eq!(
            rows[0].thread_size, 1,
            "the spam message does not count in the exchange"
        );
        assert_eq!(
            rows[0].envelope.subject.as_deref(),
            Some("Quote"),
            "the thread stays represented by the received message, not by \
             the spam that latched onto it"
        );
        assert_eq!(
            rows[0].thread_unseen, 0,
            "a spam message never opened does not make the conversation unread"
        );

        // The other half of ADR 0010: out of scope does not mean absent.
        // The message is stored — so it is searchable.
        assert!(
            store.envelope(account, "Spam", 1).unwrap().is_some(),
            "the spam message is indeed in the database: we store everything, we don't group everything"
        );
    }

    /// A scope declared BEFORE the mailbox exists must still count —
    /// this is the normal case, not the edge case.
    ///
    /// The [ADR 0010] sync loop **creates** the sent folder: at the
    /// moment the scope is declared, there is no row yet to update. If
    /// the scope only lived on `mailboxes`, this declaration would be
    /// lost, the mailbox would be born out of scope, and its messages
    /// would stay threadless until the next startup — the list would
    /// show an exchange amputated of our replies, with nothing to signal
    /// it.
    ///
    /// Hence the memory carried by the ACCOUNT, which this test guards.
    #[test]
    fn a_scope_declared_before_the_mailbox_is_created_still_counts() {
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("me@example.fr", "gmail")
            .unwrap();
        let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();

        // We declare "Sent" BEFORE creating it — the real order.
        store.set_thread_scope(account, Some("Sent")).unwrap();
        let sent = store.create_mailbox(account, "Sent", 1).unwrap();

        let mut received = envelope(1, "Quote", 100, true);
        received.message_id = Some("<alice-11@example.fr>".to_string());
        store.upsert_envelopes(inbox, &[received]).unwrap();
        let mut reply = envelope(1, "Re: Quote", 200, true);
        reply.message_id = Some("<me-11@example.fr>".to_string());
        reply.in_reply_to = Some("<alice-11@example.fr>".to_string());
        store.upsert_envelopes(sent, &[reply]).unwrap();

        let rows = unified(&store);
        assert_eq!(rows.len(), 1, "a single thread");
        assert_eq!(
            rows[0].thread_size, 2,
            "the reply joined the thread as soon as it was written, without \
             waiting for a restart"
        );
    }

    /// The promise of [ADR 0008] §4 — "the cost of a page no longer
    /// depends on the size of the mailbox" — rests ENTIRELY on an index
    /// that carries the sort order. If SQLite materializes the order in
    /// a temporary B-tree, the promise is broken: silently, and only at
    /// scale, exactly where no functional test looks anymore.
    ///
    /// It happened. Gate 3 measured **987 ms** for a page over 160,000
    /// conversations, against 0.66 ms once the index was in place. The
    /// original index was prefixed by `mailbox_id`: it served a single
    /// mailbox, but not the **unified mailbox**, which covers all of
    /// them and is the product's default view. Two accounts are enough
    /// to reproduce it — hence this fixture.
    ///
    /// We interrogate the query plan rather than a stopwatch: a duration
    /// depends on the machine, an execution plan does not.
    #[test]
    fn the_unified_mailbox_does_not_materialize_its_sort() {
        let mut store = Store::open_in_memory().unwrap();
        for (email, uids) in [("one@example.fr", 1..60u32), ("two@example.fr", 60..120)] {
            let account = store.adopt_or_create_account(email, "gmail").unwrap();
            let mailbox = store.create_mailbox(account, "INBOX", 1).unwrap();
            let envelopes: Vec<Envelope> = uids
                .map(|uid| envelope(uid, "Subject", 1_600_000_000 + i64::from(uid), true))
                .collect();
            store.upsert_envelopes(mailbox, &envelopes).unwrap();
        }

        let mut stmt = store
            .0
            .prepare(&format!(
                "EXPLAIN QUERY PLAN {}",
                unified_page_sql(false, false, false)
            ))
            .unwrap();
        let plan: Vec<String> = stmt
            .query_map(params![200i64, 0i64], |row| row.get::<_, String>(3))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();

        // "FOR LAST TERM OF ORDER BY" is acceptable: that sort only
        // breaks ties on date AND UID. It's the FULL sort that costs,
        // and only that one is forbidden here.
        assert!(
            !plan
                .iter()
                .any(|step| step.contains("TEMP B-TREE FOR ORDER BY")),
            "the unified mailbox page materializes its sort — the cost \
             becomes proportional to the mailbox size again.\nPlan:\n{}",
            plan.join("\n")
        );
        // R4: the pinned-threads subquery (PINNED_THREADS) must start
        // from `pins` (lowercase) and PROBE `envelopes` by its key —
        // without the directive CROSS JOIN, SQLite (without ANALYZE, the
        // production case) scans `envelopes` ENTIRELY on every page:
        // ~24 ms measured at 200k, on the hottest path (review
        // 2026-08-21).
        assert!(
            !plan.iter().any(|step| step.contains("SCAN pe")),
            "the pinned-threads subquery scans `envelopes` — the join \
             order has lost its directive.\nPlan:\n{}",
            plan.join("\n")
        );
        assert!(
            plan.iter().any(|step| step.contains("SCAN p")),
            "the pinned-threads subquery no longer starts from `pins`.\nPlan:\n{}",
            plan.join("\n")
        );
    }

    /// PLAN-AUDIT-V2 E4: the cleanup groups (one sender × their mail)
    /// cost 380 ms over 200k envelopes and 5,000 senders — a scan
    /// through the DATE index followed by a temporary grouping B-tree.
    /// The senders index, extended to the mailbox, COVERS the aggregate:
    /// the plan must go through it, never through the date index (a
    /// query-plan test, STANDARD §9 lesson).
    #[test]
    fn cleanup_groups_are_read_via_the_senders_index() {
        let store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("me@example.fr", "gmail")
            .unwrap();
        let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        let sql = Store::cleanup_groups_sql(&[inbox]);
        let plan: Vec<String> = store
            .conn()
            .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
            .unwrap()
            .query_map(params![0i64], |row| row.get::<_, String>(3))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(
            plan.iter().any(|row| row.contains("idx_envelopes_sender")),
            "the aggregate does not go through the senders index: {plan:?}"
        );
        assert!(
            !plan.iter().any(|row| row.contains("idx_envelopes_date")),
            "the aggregate scans the date index: {plan:?}"
        );
        // A group's mail, same requirement (116 ms at 200k otherwise).
        let sql = Store::cleanup_messages_sql(&[inbox]);
        let plan: Vec<String> = store
            .conn()
            .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
            .unwrap()
            .query_map(params![0i64, "x@y.fr"], |row| row.get::<_, String>(3))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(
            plan.iter()
                .any(|row| row.contains("idx_envelopes_sender (sender_norm=?)")),
            "a group's mail is not looked up by sender: {plan:?}"
        );
    }

    /// Wave 2 review: `PRAGMA foreign_keys = ON` lives in `SCHEMA` and
    /// holds PER CONNECTION — the fast path does not replay the schema.
    /// This test stayed green BEFORE the line was added to `init_with`:
    /// rusqlite's `bundled` enables foreign keys by default at compile
    /// time. It keeps the belt anyway: on a FILE database (an in-memory
    /// database never enters the registry), the second open still clears
    /// the mailboxes of a deleted account, whatever the compile flag.
    #[test]
    fn the_fast_path_keeps_foreign_keys_enabled() {
        let path =
            std::env::temp_dir().join(format!("wind-test-fast-path-fk-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        drop(Store::open(&path).unwrap());

        let mut store = Store::open(&path).unwrap();
        let enabled: i64 = store
            .conn()
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();
        assert_eq!(enabled, 1, "foreign keys off on the second connection");
        let account = store
            .adopt_or_create_account("me@example.fr", "gmail")
            .unwrap();
        store.create_mailbox(account, "INBOX", 1).unwrap();
        store.delete_account(account).unwrap();
        let mailboxes: i64 = store
            .conn()
            .query_row("SELECT COUNT(*) FROM mailboxes", [], |row| row.get(0))
            .unwrap();
        assert_eq!(
            mailboxes, 0,
            "the cascade of the deleted account did not fire"
        );
        drop(store);
        let _ = std::fs::remove_file(&path);
    }

    /// A database from the field carries the senders index with TWO
    /// columns; on reopen it gains the mailbox (same pattern as the date
    /// index below).
    #[test]
    fn the_inherited_senders_index_gains_the_mailbox_on_reopen() {
        let path =
            std::env::temp_dir().join(format!("wind-test-idx-sender-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let read_sql = |conn: &Connection| -> String {
            conn.query_row(
                "SELECT sql FROM sqlite_master WHERE name = 'idx_envelopes_sender'",
                [],
                |row| row.get(0),
            )
            .unwrap()
        };
        {
            let store = Store::open(&path).unwrap();
            store
                .conn()
                .execute_batch(
                    "DROP INDEX idx_envelopes_sender;
                     CREATE INDEX idx_envelopes_sender
                         ON envelopes(sender_norm, date_epoch);",
                )
                .unwrap();
            assert!(!read_sql(store.conn()).contains("mailbox_id"));
        }
        Store::forget_initialization(&path);
        let store = Store::open(&path).unwrap();
        assert!(
            read_sql(store.conn()).contains("mailbox_id"),
            "the inherited index was not rebuilt"
        );
        drop(store);
        let _ = std::fs::remove_file(&path);
    }

    /// PLAN-DEMARRAGE, E1-bis — the envelopes date index gains `uid`,
    /// and **`CREATE INDEX IF NOT EXISTS` is NOT enough**: on an existing
    /// database the index already carries that name, the creation is a
    /// silent no-op, and the defect would survive the update. The
    /// migration therefore reads its DEFINITION, not its name.
    ///
    /// Without this test, the rebuild branch is **never exercised**:
    /// every database born from a `Store::open` carries the up-to-date
    /// index straight from `SCHEMA`, and `migrate()` has nothing left to
    /// do. The index must therefore be downgraded by hand to exercise
    /// the field's code path.
    #[test]
    fn the_inherited_date_index_gains_uid_on_reopen() {
        let path =
            std::env::temp_dir().join(format!("wind-test-idx-date-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);

        let read_sql = |store: &Store| -> String {
            store
                .conn()
                .query_row(
                    "SELECT sql FROM sqlite_master WHERE name = 'idx_envelopes_date'",
                    [],
                    |row| row.get(0),
                )
                .unwrap()
        };

        {
            let store = Store::open(&path).unwrap();
            // Downgrades the index to its shape from before the job — the
            // exact state of any database in the field at update time.
            store
                .conn()
                .execute_batch(
                    "DROP INDEX idx_envelopes_date;
                     CREATE INDEX idx_envelopes_date
                         ON envelopes(mailbox_id, date_epoch DESC);",
                )
                .unwrap();
            assert!(
                !read_sql(&store).contains("uid"),
                "the fixture must start from the SHORT index, otherwise the test proves nothing"
            );
        }

        Store::forget_initialization(&path);
        let store = Store::open(&path).unwrap();
        let sql = read_sql(&store);
        assert!(
            sql.contains("uid"),
            "the inherited index was not rebuilt on open — the definition probe does nothing, and the field would keep the defect.
SQL: {sql}"
        );
        drop(store);
        let _ = std::fs::remove_file(&path);
    }

    /// PLAN-DEMARRAGE, defect 01 — the probe "how many bodies are
    /// missing?" held the GLOBAL LOCK on commands **8,870 ms at every
    /// startup** (20,839 ms in pure SQL cold), measured on 2026-08-26
    /// on the field database: 251,466 bodies, 11.4 GB.
    ///
    /// The cause was not the join. It was reading one COLUMN of
    /// `bodies`: absent from the primary key's auto-index, it forced
    /// SQLite to fetch the ROW — 56 KB on average — to read one bit.
    /// 251k random reads across 11.4 GB.
    ///
    /// The plan says it in one word: `COVERING`. As long as the
    /// subquery reads NO column of `bodies`, the existence of the row
    /// is decided from the index alone. Add a column to it one day,
    /// and the word disappears — that, and nothing else, is what this
    /// test guards.
    ///
    /// We query the plan rather than a stopwatch: a duration depends
    /// on the machine, an execution plan does not.
    #[test]
    fn missing_body_probes_never_fetch_the_fat_row() {
        let (mut store, inbox) = store_with_mailbox();
        let envelopes: Vec<Envelope> = (1..=40u32)
            .map(|uid| envelope(uid, "Subject", 1_600_000_000 + i64::from(uid), true))
            .collect();
        store.upsert_envelopes(inbox, &envelopes).unwrap();
        // Bodies for three quarters: the subquery must have both rows
        // to find AND rows not to find.
        for uid in 1..=30u32 {
            store.save_body(inbox, uid, "<p>body</p>", &[]).unwrap();
        }

        let mut count = store
            .0
            .prepare(&format!(
                "EXPLAIN QUERY PLAN {}",
                bodies_pending_count_sql()
            ))
            .unwrap();
        let count_plan: Vec<String> = count
            .query_map(params![1i64, "INBOX", 0i64], |row| row.get::<_, String>(3))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();

        let mut list = store
            .0
            .prepare(&format!("EXPLAIN QUERY PLAN {}", bodies_to_backfill_sql()))
            .unwrap();
        let list_plan: Vec<String> = list
            .query_map(params![1i64, "INBOX", 0i64, 10i64], |row| {
                row.get::<_, String>(3)
            })
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();

        for (what, plan) in [
            ("the count of missing", &count_plan),
            ("the backfill work list", &list_plan),
        ] {
            for (alias, table) in [(" e ", "envelopes"), (" b ", "bodies")] {
                let step = plan
                    .iter()
                    .find(|step| step.contains(alias))
                    .unwrap_or_else(|| {
                        panic!(
                            "{what}: no step touches `{table}`.\nPlan:\n{}",
                            plan.join("\n")
                        )
                    });
                assert!(
                    step.contains("COVERING"),
                    "{what}: access to `{table}` is NOT covered by its \
index — SQLite fetches the row to read a column the index does not \
carry. That is the PLAN-DEMARRAGE defect, on BOTH sides: 8,870 ms of \
lock held on the `bodies` side, 521.9 ms of probe on the `envelopes` \
side.\n\
Step: {step}\nPlan:\n{}",
                    plan.join("\n")
                );
            }
        }
    }

    /// R4 (PLAN-RETOURS-7): a pinned conversation is served SEPARATELY
    /// (`pinned_unified_scoped`) and LEAVES the paginated flow along
    /// with its count (decision D5: the list never shows the same
    /// message twice). Unpinning returns it to the flow. The pin is
    /// bounded to the account and follows the "Unread" tab like the
    /// page.
    #[test]
    fn a_pin_serves_its_conversation_separately_and_out_of_the_flow() {
        let (mut store, inbox) = store_with_mailbox();
        store
            .upsert_envelopes(
                inbox,
                &[
                    envelope(1, "old", 100, true),
                    envelope(2, "middle", 200, true),
                    envelope(3, "recent", 300, true),
                ],
            )
            .unwrap();
        assert!(
            store
                .pinned_unified_scoped(None, false, false)
                .unwrap()
                .is_empty()
        );

        assert!(store.toggle_pin(inbox, 1, 1_000).unwrap(), "pinned");
        let pinned = store.pinned_unified_scoped(None, false, false).unwrap();
        assert_eq!(pinned.len(), 1);
        assert_eq!(pinned[0].envelope.uid, 1);
        let flow = store.unified_recent_scoped(None, false, 0, 10).unwrap();
        assert!(
            flow.iter().all(|row| row.envelope.uid != 1),
            "the pinned conversation leaves the flow"
        );
        assert_eq!(flow.len(), 2);
        assert_eq!(store.unified_count_scoped(None, false).unwrap(), 2);
        // Scope bounds: an OTHER account does not have this pin, and
        // the "Unread" tab does not show it (everything is read here).
        assert!(
            store
                .pinned_unified_scoped(Some(999), false, false)
                .unwrap()
                .is_empty()
        );
        assert!(
            store
                .pinned_unified_scoped(None, true, false)
                .unwrap()
                .is_empty()
        );

        assert!(!store.toggle_pin(inbox, 1, 1_001).unwrap(), "unpinned");
        assert!(
            store
                .pinned_unified_scoped(None, false, false)
                .unwrap()
                .is_empty()
        );
        assert_eq!(store.unified_count_scoped(None, false).unwrap(), 3);
    }

    /// R1 (PLAN-RETOURS-11, D1-D2): the "Show images" choice is an
    /// EXPLICIT exception written to the database, per MESSAGE
    /// (envelope key, `pins` pattern) — reopening the message does not
    /// ask again, and the neighboring message inherits nothing.
    #[test]
    fn the_image_choice_per_message_persists_and_does_not_bleed_over() {
        let (mut store, inbox) = store_with_mailbox();
        store
            .upsert_envelopes(
                inbox,
                &[envelope(1, "a", 100, true), envelope(2, "b", 200, true)],
            )
            .unwrap();
        assert!(
            !store.images_allowed(inbox, 1).unwrap(),
            "blocked by default"
        );
        store.allow_images_message(inbox, 1, 1_000).unwrap();
        assert!(store.images_allowed(inbox, 1).unwrap());
        assert!(
            !store.images_allowed(inbox, 2).unwrap(),
            "the choice is PER message"
        );
    }

    /// R1 (D3-D4): the sender rule is set FROM a message — the address
    /// is read from the ENVELOPE (never from the UI), normalized to
    /// lowercase — covers all its messages, and can be listed and
    /// revoked.
    #[test]
    fn the_sender_rule_covers_its_messages_and_can_be_revoked() {
        let (mut store, inbox) = store_with_mailbox();
        let mut sender = envelope(1, "a", 100, true);
        sender.sender_address = Some("No-Reply@Registrar.FR".to_string());
        let mut same = envelope(2, "b", 200, true);
        same.sender_address = Some("no-reply@registrar.fr".to_string());
        let third_party = envelope(3, "c", 300, true); // alice@example.com
        store
            .upsert_envelopes(inbox, &[sender, same, third_party])
            .unwrap();

        let applied = store.allow_images_sender_of(inbox, 1, 1_000).unwrap();
        assert_eq!(
            applied.as_deref(),
            Some("no-reply@registrar.fr"),
            "the applied address is normalized"
        );
        assert!(store.images_allowed(inbox, 1).unwrap());
        assert!(
            store.images_allowed(inbox, 2).unwrap(),
            "all of the sender's messages, whatever the case"
        );
        assert!(
            !store.images_allowed(inbox, 3).unwrap(),
            "never a third party"
        );
        assert_eq!(
            store.images_senders().unwrap(),
            vec!["no-reply@registrar.fr".to_string()]
        );

        store.revoke_images_sender("no-reply@registrar.fr").unwrap();
        assert!(store.images_senders().unwrap().is_empty());
        assert!(
            !store.images_allowed(inbox, 1).unwrap(),
            "revoked — the guard returns"
        );
    }

    /// R1 (review 2026-08-28): the PER-MESSAGE image consent dies on a
    /// UIDVALIDITY change — a recycled UID must NEVER inherit a
    /// consent (a stranger's tracking pixel would fire with no banner
    /// and no gesture). Same contract as `invitations`/`attachments`
    /// in `reset_mailbox`.
    #[test]
    fn the_uidvalidity_reset_purges_the_per_message_image_memory() {
        let (mut store, inbox) = store_with_mailbox();
        store
            .upsert_envelopes(inbox, &[envelope(1, "a", 100, true)])
            .unwrap();
        store.allow_images_message(inbox, 1, 1_000).unwrap();
        assert!(store.images_allowed(inbox, 1).unwrap());

        store.reset_mailbox(inbox, 2).unwrap();
        store
            .upsert_envelopes(inbox, &[envelope(1, "something else entirely", 200, true)])
            .unwrap();
        assert!(
            !store.images_allowed(inbox, 1).unwrap(),
            "a recycled UID inherits no consent"
        );
    }

    /// R1: an envelope WITHOUT a sender address sets NOTHING — never
    /// an empty rule that would grant who-knows-what.
    #[test]
    fn no_sender_address_no_rule() {
        let (mut store, inbox) = store_with_mailbox();
        let mut without = envelope(1, "a", 100, true);
        without.sender_address = None;
        store.upsert_envelopes(inbox, &[without]).unwrap();
        assert_eq!(store.allow_images_sender_of(inbox, 1, 1_000).unwrap(), None);
        assert!(store.images_senders().unwrap().is_empty());
        assert!(!store.images_allowed(inbox, 1).unwrap());
    }

    /// R4: the pin follows the THREAD — set on a message, it holds
    /// when a reply moves the head of the conversation; `pin_state`
    /// answers per thread, and unpinning from the NEW head releases
    /// the whole thread.
    #[test]
    fn a_pin_follows_the_thread_and_its_new_head() {
        let (mut store, inbox) = store_with_mailbox();
        store
            .upsert_envelopes(inbox, &[envelope(1, "subject", 100, true)])
            .unwrap();
        assert!(store.toggle_pin(inbox, 1, 1_000).unwrap());

        let mut reply = envelope(2, "Re: subject", 400, true);
        reply.in_reply_to = Some("<m1@example.com>".to_string());
        store.upsert_envelopes(inbox, &[reply]).unwrap();

        let pinned = store.pinned_unified_scoped(None, false, false).unwrap();
        assert_eq!(pinned.len(), 1, "a pinned thread = ONE row");
        assert_eq!(
            pinned[0].envelope.uid, 2,
            "the row is the head of the thread"
        );
        assert_eq!(pinned[0].thread_size, 2);
        assert!(
            store.pin_state(inbox, 2).unwrap(),
            "the state is read per thread"
        );

        assert!(
            !store.toggle_pin(inbox, 2, 1_001).unwrap(),
            "unpinned from the new head"
        );
        assert!(
            store
                .pinned_unified_scoped(None, false, false)
                .unwrap()
                .is_empty()
        );
        assert!(!store.pin_state(inbox, 1).unwrap());
        assert_eq!(store.unified_count_scoped(None, false).unwrap(), 1);
    }

    /// PLAN-MODE-ORGANISE E1 (D1: routing is LOCAL only, `images_expediteurs`
    /// pattern). Setting it normalizes the address through THE SAME
    /// authority as the image guard, overwrites the previous decision
    /// (a single verdict per sender), and "Reinstate" = DELETE —
    /// whatever the case supplied by the caller.
    #[test]
    fn routing_set_normalizes_overwrites_and_can_be_removed() {
        let store = Store::open_in_memory().unwrap();
        store
            .route_sender("  Ada@Exemple.FR ", "kiosque", None, 1_700_000_000)
            .unwrap();
        let r = store.routing_of("ada@exemple.fr").unwrap().unwrap();
        assert_eq!(
            (r.destination.as_str(), r.rule.as_deref()),
            ("kiosque", None)
        );
        store
            .route_sender("ada@exemple.fr", "ecarte", Some("corbeille"), 1_700_000_100)
            .unwrap();
        let r = store.routing_of("ADA@EXEMPLE.FR").unwrap().unwrap();
        assert_eq!(
            (r.destination.as_str(), r.rule.as_deref()),
            ("ecarte", Some("corbeille"))
        );
        store.remove_routing(" ada@EXEMPLE.fr ").unwrap();
        assert!(store.routing_of("ada@exemple.fr").unwrap().is_none());
    }

    /// The vocabulary is CLOSED: a destination or a rule outside the
    /// table is refused BEFORE any write (a pure decision, never a
    /// SQLite CHECK as the first line of defense); a rule only makes
    /// sense on a screened-out sender; an empty address never writes a
    /// phantom rule.
    #[test]
    fn routing_refuses_outside_the_vocabulary() {
        let store = Store::open_in_memory().unwrap();
        assert!(store.route_sender("a@b.fr", "poubelle", None, 1).is_err());
        assert!(
            store
                .route_sender("a@b.fr", "ecarte", Some("suppression-definitive"), 1)
                .is_err()
        );
        assert!(
            store
                .route_sender("a@b.fr", "kiosque", Some("corbeille"), 1)
                .is_err(),
            "a No rule on a served destination makes no sense"
        );
        assert!(store.route_sender("   ", "kiosque", None, 1).is_err());
        assert!(store.routings().unwrap().is_empty(), "nothing was written");
    }

    /// PLAN-MODE-ORGANISE E1: a page of the Feed or the Paper trail —
    /// the Inbox's unified flow, bounded to threads whose HEAD comes
    /// from a sender routed to that destination. Same skeleton, same
    /// exclusions (pins), same sort as the Inbox; the probe is PK → PK
    /// (spike S2: 0.209 ms at 200k, never a scan).
    #[test]
    fn the_feed_only_serves_routed_senders() {
        let (mut store, inbox) = store_with_mailbox();
        let mut letter = envelope(1, "The letter", 100, true);
        letter.sender_address = Some("Lettre@infolettre.fr".to_string());
        letter.message_id = Some("<l1@infolettre.fr>".to_string());
        let ordinary = envelope(2, "Hello", 200, false);
        store.upsert_envelopes(inbox, &[letter, ordinary]).unwrap();
        store
            .route_sender("lettre@infolettre.fr", "kiosque", None, 300)
            .unwrap();

        let feed = store
            .routing_unified_scoped("kiosque", None, false, 0, 10)
            .unwrap();
        assert_eq!(feed.len(), 1);
        assert_eq!(feed[0].envelope.uid, 1);
        assert_eq!(
            store.routing_count_scoped("kiosque", None, false).unwrap(),
            1
        );
        // The Paper trail is empty: the destination really filters.
        assert!(
            store
                .routing_unified_scoped("registre", None, false, 0, 10)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            store.routing_count_scoped("registre", None, false).unwrap(),
            0
        );
        // The Inbox, meanwhile, ALWAYS shows everything (E1: taking
        // items out of the flow is the job of step E2 — Screener
        // retention).
        assert_eq!(store.unified_count_scoped(None, false).unwrap(), 2);
    }

    /// The plan guard for serving the Feed (`pins` lesson): the
    /// routing probe is played by KEYS (envelopes PK, routing PK) —
    /// never a scan of `envelopes`.
    #[test]
    fn the_feed_never_scans_the_envelopes() {
        let store = Store::open_in_memory().unwrap();
        let plan: Vec<String> = store
            .0
            .prepare(&format!(
                "EXPLAIN QUERY PLAN {}",
                routing_page_sql(false, false)
            ))
            .unwrap()
            .query_map(params![10, 0, "kiosque"], |row| row.get::<_, String>(3))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let scans: Vec<&String> = plan
            .iter()
            .filter(|l| l.starts_with("SCAN") && l.contains("envelopes"))
            .collect();
        assert!(scans.is_empty(), "plan with an envelopes scan: {plan:?}");
    }

    /// Review E1: the HEAD of a thread is the last message across ALL
    /// mailboxes — Sent included. The gesture and the filter must
    /// never anchor on it: (1) "Move to…" from a thread where the
    /// user replied last must route the CORRESPONDENT, never
    /// themselves; (2) a thread routed to the Feed does not leave it
    /// because we replied there; (3) a pinned routed thread stays
    /// visible in its destination (pins are only surfaced in the
    /// Inbox — excluding it here would make it disappear everywhere).
    #[test]
    fn routing_ignores_its_own_reply_and_keeps_pins() {
        let (mut store, inbox) = store_with_mailbox();
        // Sent items enter the grouping scope (ADR 0009) — without
        // which the reply would stay out of the thread and the
        // fixture would not replay the root (head = Sent).
        store
            .set_thread_scope(test_account(&store), Some("Envoyes"))
            .unwrap();
        let sent = store
            .create_mailbox(test_account(&store), "Envoyes", 1)
            .unwrap();
        let mut letter = envelope(1, "The letter", 100, true);
        letter.sender_address = Some("lettre@infolettre.fr".to_string());
        letter.message_id = Some("<l1@infolettre.fr>".to_string());
        store.upsert_envelopes(inbox, &[letter]).unwrap();
        // The user's reply, in Sent — it becomes the HEAD of the
        // thread (most recent date).
        let mut reply = envelope(1, "Re: The letter", 500, true);
        reply.sender_address = Some("test@exemple.fr".to_string());
        reply.message_id = Some("<r1@exemple.fr>".to_string());
        reply.in_reply_to = Some("<l1@infolettre.fr>".to_string());
        store.upsert_envelopes(sent, &[reply]).unwrap();

        // (1) The gesture from the head (the user's own reply) routes
        // the correspondent, never themselves.
        let address = store
            .route_sender_of(sent, 1, "kiosque", None, 600)
            .unwrap();
        assert_eq!(address.as_deref(), Some("lettre@infolettre.fr"));
        // (2) The thread is in the Feed despite its "Sent" head.
        let feed = store
            .routing_unified_scoped("kiosque", None, false, 0, 10)
            .unwrap();
        assert_eq!(feed.len(), 1);
        assert_eq!(
            store.routing_count_scoped("kiosque", None, false).unwrap(),
            1
        );
        // (3) Pinned, it stays visible in the Feed — page AND total.
        assert!(store.toggle_pin(inbox, 1, 700).unwrap());
        assert_eq!(
            store
                .routing_unified_scoped("kiosque", None, false, 0, 10)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            store.routing_count_scoped("kiosque", None, false).unwrap(),
            1
        );
    }

    /// "Move to…" (E1): the address is resolved from the ENVELOPE on
    /// the core side — the UI never parses an address
    /// (`allow_images_sender_of` pattern). Returns the routed
    /// address; None if the envelope has no address (never a phantom
    /// verdict).
    #[test]
    fn routing_from_the_envelope_resolves_the_address_in_the_core() {
        let (mut store, inbox) = store_with_mailbox();
        let mut env = envelope(1, "subject", 100, true);
        env.sender_address = Some("  ADA@Exemple.FR ".to_string());
        let mut without_address = envelope(2, "anonymous", 200, true);
        without_address.sender_address = None;
        store
            .upsert_envelopes(inbox, &[env, without_address])
            .unwrap();

        let address = store
            .route_sender_of(inbox, 1, "registre", None, 300)
            .unwrap();
        assert_eq!(address.as_deref(), Some("ada@exemple.fr"));
        assert_eq!(
            store
                .routing_of("ada@exemple.fr")
                .unwrap()
                .unwrap()
                .destination,
            "registre"
        );
        assert_eq!(
            store
                .route_sender_of(inbox, 2, "kiosque", None, 400)
                .unwrap(),
            None
        );
        assert_eq!(
            store.routings().unwrap().len(),
            1,
            "nothing written without an address"
        );
    }

    /// Organized mode lives in SQLite `prefs` (D2 amended: Rust must
    /// read the state — the No rules turn off with it) and the
    /// FIRST-ACTIVATION EPOCH NEVER moves (D3 "arrivals only": it is
    /// what bounds Screener retention; rewriting it on every toggle
    /// would silently dump or hold back mail). Off by default, the
    /// state and the epoch are written TOGETHER on first activation
    /// (never one without the other).
    #[test]
    fn organized_mode_keeps_the_first_activation_epoch() {
        let mut store = Store::open_in_memory().unwrap();
        assert!(!store.organized_mode().unwrap());
        assert_eq!(store.organized_mode_epoch().unwrap(), None);
        store.set_organized_mode(true, 100).unwrap();
        assert!(store.organized_mode().unwrap());
        assert_eq!(store.organized_mode_epoch().unwrap(), Some(100));
        store.set_organized_mode(false, 200).unwrap();
        assert!(!store.organized_mode().unwrap());
        store.set_organized_mode(true, 300).unwrap();
        assert_eq!(
            store.organized_mode_epoch().unwrap(),
            Some(100),
            "the FIRST activation epoch is set in stone"
        );
    }

    /// RETOURS-13 R10 — the Feed's "read" memory (`pins`/`mis_de_cote`
    /// pattern: envelope key, local to the workstation). A card read
    /// down to the bottom gets marked; the mark is idempotent, dies
    /// with its mailbox (`reset_mailbox`) and with its message
    /// (`remove_local`) — a recycled UID inherits no read state.
    #[test]
    fn feed_read_gets_marked_and_dies_with_its_mailbox_and_its_message() {
        let (mut store, inbox) = store_with_mailbox();
        store
            .upsert_envelopes(inbox, &[envelope(1, "letter", 1_000, false)])
            .unwrap();
        store
            .upsert_envelopes(inbox, &[envelope(2, "other", 1_100, false)])
            .unwrap();
        assert!(!store.feed_read(inbox, 1).unwrap());
        store.mark_feed_read(inbox, 1, 2_000).unwrap();
        store.mark_feed_read(inbox, 1, 2_100).unwrap(); // idempotent
        assert!(store.feed_read(inbox, 1).unwrap());
        store.mark_feed_read(inbox, 2, 2_200).unwrap();
        // The message leaves: its mark leaves too.
        store.remove_local(inbox, 1).unwrap();
        assert!(!store.feed_read(inbox, 1).unwrap());
        // The mailbox resets: no more marks at all.
        store.reset_mailbox(inbox, 2).unwrap();
        assert!(!store.feed_read(inbox, 2).unwrap());
    }

    /// RETOURS-14 R8 (field 2026-08-31) — a YES to the Screener means
    /// trust: the verdict ALSO sets the rule "always show this
    /// sender's images" (`images_expediteurs` table, revocable in
    /// Settings > Display like any rule). A No sets nothing and
    /// removes nothing — the image guard has its own exit door.
    #[test]
    fn a_yes_to_the_screener_allows_the_senders_images() {
        let (mut store, inbox) = store_with_mailbox();
        let mut welcome = envelope(1, "Hello", 100, false);
        welcome.sender_address = Some("Ami@exemple.fr".to_string());
        welcome.message_id = Some("<a1@exemple.fr>".to_string());
        let mut intruder = envelope(2, "Promo", 200, false);
        intruder.sender_address = Some("promo@exemple.fr".to_string());
        intruder.message_id = Some("<p1@exemple.fr>".to_string());
        store.upsert_envelopes(inbox, &[welcome, intruder]).unwrap();
        assert!(!store.images_allowed(inbox, 1).unwrap());

        // The Yes (any served destination) sets the rule — address
        // normalized by THE gate (images_address).
        store
            .route_sender("ami@exemple.fr", "reception", None, 300)
            .unwrap();
        assert!(store.images_allowed(inbox, 1).unwrap());
        // The No allows nothing.
        store
            .route_sender("promo@exemple.fr", "ecarte", Some("spam"), 300)
            .unwrap();
        assert!(!store.images_allowed(inbox, 2).unwrap());
        // The pre-existing exit door undoes the rule set by the Yes.
        store.revoke_images_sender("ami@exemple.fr").unwrap();
        assert!(!store.images_allowed(inbox, 1).unwrap());
    }

    /// RETOURS-14 R6 (D7) — the Paper trail groups by SENDER, groups
    /// sorted by the recency of the last message (Cleanup pattern),
    /// and a group's page returns the threads of that one sender, in
    /// the view's sort order.
    #[test]
    fn the_paper_trail_groups_by_sender_by_recency() {
        let (mut store, inbox) = store_with_mailbox();
        let mut old = envelope(1, "Receipt A", 100, true);
        old.sender_address = Some("recu@boutique.fr".to_string());
        old.message_id = Some("<r1@boutique.fr>".to_string());
        let mut recent = envelope(2, "Notice B", 300, true);
        recent.sender_address = Some("avis@banque.fr".to_string());
        recent.message_id = Some("<b1@banque.fr>".to_string());
        let mut second = envelope(3, "Receipt C", 200, true);
        second.sender_address = Some("recu@boutique.fr".to_string());
        second.message_id = Some("<r2@boutique.fr>".to_string());
        let outside = envelope(4, "Hello", 400, false);
        store
            .upsert_envelopes(inbox, &[old, recent, second, outside])
            .unwrap();
        store
            .route_sender("recu@boutique.fr", "registre", None, 500)
            .unwrap();
        store
            .route_sender("avis@banque.fr", "registre", None, 500)
            .unwrap();

        let groups = store.paper_trail_groups(None).unwrap();
        assert_eq!(groups.len(), 2, "one group per routed sender");
        // Recency first (D7): banque (300) before boutique (200).
        assert_eq!(groups[0].address, "avis@banque.fr");
        assert_eq!(groups[0].threads, 1);
        assert_eq!(groups[1].address, "recu@boutique.fr");
        assert_eq!(groups[1].threads, 2);
        assert_eq!(groups[1].last_epoch, 200);
        assert_eq!(groups[1].last_subject.as_deref(), Some("Receipt C"));

        // A group's page: the threads of THIS one sender, most recent
        // first.
        let page = store
            .paper_trail_group_scoped("recu@boutique.fr", None, 0, 10)
            .unwrap();
        assert_eq!(page.len(), 2);
        assert_eq!(page[0].envelope.uid, 3);
        assert_eq!(page[1].envelope.uid, 1);
        // The account filter bounds it like everywhere else.
        let other = store
            .paper_trail_group_scoped("recu@boutique.fr", Some(999), 0, 10)
            .unwrap();
        assert!(other.is_empty());
    }

    /// RETOURS-14 R7 (D8) — the Feed's nav badge counts cards NOT YET
    /// OPENED (`kiosque_lus` memory), never the IMAP `seen` flag: that
    /// is the semantics of the page itself (the Unread / Previously
    /// read sections). The fixture is seen server-side (`seen =
    /// true`): if the query counted `unseen`, it would return zero.
    #[test]
    fn the_feed_badge_counts_never_opened_cards() {
        let (mut store, inbox) = store_with_mailbox();
        let mut a = envelope(1, "Letter A", 100, true);
        a.sender_address = Some("lettre@infolettre.fr".to_string());
        a.message_id = Some("<a@infolettre.fr>".to_string());
        let mut b = envelope(2, "Letter B", 200, true);
        b.sender_address = Some("lettre@infolettre.fr".to_string());
        b.message_id = Some("<b@infolettre.fr>".to_string());
        let ordinary = envelope(3, "Hello", 300, false);
        store.upsert_envelopes(inbox, &[a, b, ordinary]).unwrap();
        store
            .route_sender("lettre@infolettre.fr", "kiosque", None, 400)
            .unwrap();

        // Two cards in the Feed, none opened — the IMAP seen flag
        // (true) does not count; neither does the unrouted message.
        assert_eq!(store.feed_unopened(None).unwrap(), 2);
        // The account filter is proven WHILE some unread remains
        // (review: at zero everywhere, an ignored filter would pass
        // green): the right account sees 2, a foreign account 0.
        let account = test_account(&store);
        assert_eq!(store.feed_unopened(Some(account)).unwrap(), 2);
        assert_eq!(store.feed_unopened(Some(account + 1)).unwrap(), 0);
        // Opening a card removes it from the count.
        store.mark_feed_read(inbox, 2, 500).unwrap();
        assert_eq!(store.feed_unopened(None).unwrap(), 1);
        store.mark_feed_read(inbox, 1, 600).unwrap();
        assert_eq!(store.feed_unopened(None).unwrap(), 0);
    }

    /// RETOURS-13 R5/R9 — the Screener buttons' DEFAULT actions:
    /// shipped as Yes → Inbox, No → Trash; configurable within a
    /// CLOSED vocabulary (the Yes destinations, the No rules plus
    /// "screen out without moving"); a corrupted pref falls back to
    /// the default — never a verdict with a broken vocabulary.
    #[test]
    fn screener_defaults_ship_then_configurable_within_the_closed_vocabulary() {
        let mut store = Store::open_in_memory().unwrap();
        assert_eq!(
            store.screener_defaults().unwrap(),
            ("reception".to_string(), "corbeille".to_string()),
            "the shipped defaults: Yes → Inbox, No → Trash"
        );
        store.set_screener_defaults("kiosque", "archive").unwrap();
        assert_eq!(
            store.screener_defaults().unwrap(),
            ("kiosque".to_string(), "archive".to_string())
        );
        store.set_screener_defaults("reception", "ecarte").unwrap();
        assert_eq!(store.screener_defaults().unwrap().1, "ecarte");
        // The vocabulary is closed: "ecarte" is not a Yes, a
        // destination is not a No rule.
        assert!(store.set_screener_defaults("ecarte", "corbeille").is_err());
        assert!(
            store
                .set_screener_defaults("reception", "registre")
                .is_err()
        );
        // A corrupted pref (written outside the gate) falls back to
        // the default.
        store
            .set_text_pref("portier_defaut_oui", "poubelle")
            .unwrap();
        assert_eq!(store.screener_defaults().unwrap().0, "reception");
    }

    /// PLAN-MODE-ORGANISE E2 — Screener retention (D3 "arrivals
    /// only"). A sender WITHOUT a routing row whose mail only exists
    /// AFTER the activation epoch waits at the Screener: its thread
    /// leaves the flow AND the totals of the organized Inbox (shared
    /// exclusion, `pins` lesson). A known sender's history stays in
    /// the Inbox, and CLASSIC mode does not move a single message.
    #[test]
    fn an_unknown_sender_after_the_epoch_waits_at_the_screener_out_of_the_flow_and_totals() {
        let (mut store, inbox) = store_with_mailbox();
        store.set_organized_mode(true, 1_000).unwrap();
        // The known one: mail before AND after the epoch.
        let mut before = envelope(1, "from yesterday", 500, true);
        before.sender_address = Some("ancien@exemple.fr".to_string());
        let mut after = envelope(2, "from today", 1_500, false);
        after.sender_address = Some("ancien@exemple.fr".to_string());
        // The unknown one: first message AFTER the epoch.
        let mut unknown = envelope(3, "first time", 1_600, false);
        unknown.sender = Some("New Arrival".to_string());
        unknown.sender_address = Some("Nouv@Exemple.FR".to_string());
        store
            .upsert_envelopes(inbox, &[before, after, unknown])
            .unwrap();

        let page = store.organized_inbox_scoped(None, false, 0, 10).unwrap();
        assert_eq!(
            page.iter().map(|r| r.envelope.uid).collect::<Vec<_>>(),
            vec![2, 1],
            "the organized Inbox only serves the known sender"
        );
        assert_eq!(
            store.organized_inbox_count_scoped(None, false).unwrap(),
            2,
            "the total follows the flow (shared exclusion)"
        );
        assert_eq!(
            store.unified_count_scoped(None, false).unwrap(),
            3,
            "classic mode ALWAYS shows everything"
        );
        let waiting = store.screener_waiting().unwrap();
        assert_eq!(waiting.len(), 1);
        assert_eq!(waiting[0].address, "nouv@exemple.fr");
        assert_eq!(
            waiting[0].row.envelope.uid, 3,
            "the rank carries its last message"
        );
        assert_eq!(store.screener_total().unwrap(), 1);
    }

    /// The Screener gate: a plain Yes returns the sender to the
    /// Inbox, a No with a rule screens it out — in BOTH cases it
    /// leaves the waiting list, and the history records the rule
    /// chosen.
    #[test]
    fn a_yes_releases_a_no_screens_out_and_the_waiting_list_empties() {
        let (mut store, inbox) = store_with_mailbox();
        store.set_organized_mode(true, 1_000).unwrap();
        let mut a = envelope(1, "hello", 1_500, false);
        a.sender_address = Some("a@exemple.fr".to_string());
        let mut b = envelope(2, "offer", 1_600, false);
        b.sender_address = Some("b@exemple.fr".to_string());
        store.upsert_envelopes(inbox, &[a, b]).unwrap();
        assert_eq!(store.screener_waiting().unwrap().len(), 2);
        assert_eq!(store.organized_inbox_count_scoped(None, false).unwrap(), 0);

        // Plain Yes → Inbox: the thread comes back, page AND total.
        store
            .route_sender("a@exemple.fr", "reception", None, 2_000)
            .unwrap();
        assert_eq!(
            store
                .screener_waiting()
                .unwrap()
                .iter()
                .map(|r| r.address.as_str())
                .collect::<Vec<_>>(),
            vec!["b@exemple.fr"]
        );
        let page = store.organized_inbox_scoped(None, false, 0, 10).unwrap();
        assert_eq!(page.len(), 1);
        assert_eq!(page[0].envelope.uid, 1);
        assert_eq!(store.organized_inbox_count_scoped(None, false).unwrap(), 1);

        // No with a rule → screened out: out of the Inbox, out of
        // every served view, and the history carries the rule.
        store
            .route_sender("b@exemple.fr", "ecarte", Some("archive"), 2_100)
            .unwrap();
        assert!(store.screener_waiting().unwrap().is_empty());
        assert_eq!(store.screener_total().unwrap(), 0);
        assert_eq!(
            store.organized_inbox_count_scoped(None, false).unwrap(),
            1,
            "the screened-out sender does not return to the Inbox"
        );
        assert!(
            store
                .routing_unified_scoped("kiosque", None, false, 0, 10)
                .unwrap()
                .is_empty(),
            "screened out is not a served view"
        );
        let verdict = store.routing_of("b@exemple.fr").unwrap().unwrap();
        assert_eq!(
            (verdict.destination.as_str(), verdict.rule.as_deref()),
            ("ecarte", Some("archive"))
        );
    }

    /// "Reinstate" from the history = DELETE of the row: a
    /// screened-out unknown sender RETURNS to the Screener (their
    /// messages reappear), a routed known sender simply returns to
    /// the Inbox — never to the Screener, their pre-epoch mail is
    /// proof enough.
    #[test]
    fn reinstating_returns_the_unknown_sender_to_the_screener_and_the_known_one_to_the_inbox() {
        let (mut store, inbox) = store_with_mailbox();
        store.set_organized_mode(true, 1_000).unwrap();
        let mut known = envelope(1, "from yesterday", 500, true);
        known.sender_address = Some("ancien@exemple.fr".to_string());
        let mut unknown = envelope(2, "first time", 1_500, false);
        unknown.sender_address = Some("nouv@exemple.fr".to_string());
        store.upsert_envelopes(inbox, &[known, unknown]).unwrap();
        store
            .route_sender("nouv@exemple.fr", "ecarte", Some("spam"), 2_000)
            .unwrap();
        store
            .route_sender("ancien@exemple.fr", "kiosque", None, 2_000)
            .unwrap();
        assert!(store.screener_waiting().unwrap().is_empty());
        assert_eq!(store.organized_inbox_count_scoped(None, false).unwrap(), 0);

        store.remove_routing("nouv@exemple.fr").unwrap();
        let waiting = store.screener_waiting().unwrap();
        assert_eq!(
            waiting.len(),
            1,
            "the reinstated unknown sender waits again at the Screener"
        );
        assert_eq!(waiting[0].address, "nouv@exemple.fr");

        store.remove_routing("ancien@exemple.fr").unwrap();
        assert_eq!(
            store.screener_waiting().unwrap().len(),
            1,
            "the known sender NEVER goes through the Screener: their pre-epoch mail is proof enough"
        );
        assert_eq!(
            store.organized_inbox_count_scoped(None, false).unwrap(),
            1,
            "the known sender is returned to the Inbox"
        );
    }

    /// Golden rule — never lose mail: a MIXED thread (an unknown
    /// sender replies in a known sender's thread) STAYS in the Inbox;
    /// the unknown sender still waits at the Screener. Retention only
    /// takes a thread if it belongs ENTIRELY to waiting senders.
    #[test]
    fn a_mixed_thread_stays_in_the_inbox_and_the_unknown_sender_still_waits() {
        let (mut store, inbox) = store_with_mailbox();
        store.set_organized_mode(true, 1_000).unwrap();
        let mut yesterday = envelope(1, "yesterday", 500, true);
        yesterday.sender_address = Some("connu@exemple.fr".to_string());
        let mut root = envelope(2, "project", 1_500, false);
        root.sender_address = Some("connu@exemple.fr".to_string());
        store.upsert_envelopes(inbox, &[yesterday, root]).unwrap();
        let mut intruder = envelope(3, "Re: project", 1_600, false);
        intruder.sender_address = Some("nouv@exemple.fr".to_string());
        intruder.in_reply_to = Some("<m2@example.com>".to_string());
        store.upsert_envelopes(inbox, &[intruder]).unwrap();

        assert_eq!(
            store.organized_inbox_count_scoped(None, false).unwrap(),
            2,
            "the mixed thread and yesterday's thread stay in the Inbox"
        );
        let waiting = store.screener_waiting().unwrap();
        assert_eq!(
            waiting
                .iter()
                .map(|r| r.address.as_str())
                .collect::<Vec<_>>(),
            vec!["nouv@exemple.fr"],
            "the unknown sender waits at the Screener even though their thread is mixed"
        );
    }

    /// Never yourself at the Screener (E1 lesson "never your own
    /// address"), and never a waiting entry without an address.
    #[test]
    fn never_yourself_or_without_an_address_at_the_screener() {
        let (mut store, inbox) = store_with_mailbox();
        store.set_organized_mode(true, 1_000).unwrap();
        let mut self_mail = envelope(1, "note to self", 1_500, false);
        self_mail.sender_address = Some("Test@Exemple.FR".to_string());
        let mut silent = envelope(2, "anonymous", 1_600, false);
        silent.sender_address = None;
        store.upsert_envelopes(inbox, &[self_mail, silent]).unwrap();
        assert!(store.screener_waiting().unwrap().is_empty());
        assert_eq!(
            store.organized_inbox_count_scoped(None, false).unwrap(),
            2,
            "nothing is held back: neither ourselves nor a message without an address"
        );
    }

    /// Sync does not arrive in order: if a sender's OLD mail
    /// (predating the epoch) arrives AFTER their new mail, the
    /// waiting entry wrongly set unwinds and the thread is released —
    /// the sender was known, the database just did not know it yet.
    #[test]
    fn old_mail_arriving_after_the_fact_undoes_the_waiting_entry() {
        let (mut store, inbox) = store_with_mailbox();
        store.set_organized_mode(true, 1_000).unwrap();
        let mut new_mail = envelope(1, "recent", 1_500, false);
        new_mail.sender_address = Some("connu@exemple.fr".to_string());
        store.upsert_envelopes(inbox, &[new_mail]).unwrap();
        assert_eq!(store.screener_waiting().unwrap().len(), 1);

        let mut old_mail = envelope(2, "history arrives", 500, true);
        old_mail.sender_address = Some("connu@exemple.fr".to_string());
        store.upsert_envelopes(inbox, &[old_mail]).unwrap();
        assert!(
            store.screener_waiting().unwrap().is_empty(),
            "pre-epoch mail proves the sender is known"
        );
        assert_eq!(
            store.organized_inbox_count_scoped(None, false).unwrap(),
            2,
            "their threads are released, page and totals"
        );
    }

    /// Waiting entries are DERIVED from mail: when the mailbox resets
    /// (UIDVALIDITY), the Screener ranks that no longer rest on
    /// anything die with it (A43/A89 lesson — a recycled UID must
    /// inherit no decision).
    #[test]
    fn the_waiting_entry_dies_with_the_mail_that_carried_it() {
        let (mut store, inbox) = store_with_mailbox();
        store.set_organized_mode(true, 1_000).unwrap();
        let mut unknown = envelope(1, "first time", 1_500, false);
        unknown.sender_address = Some("nouv@exemple.fr".to_string());
        store.upsert_envelopes(inbox, &[unknown]).unwrap();
        assert_eq!(store.screener_waiting().unwrap().len(), 1);

        store.reset_mailbox(inbox, 2).unwrap();
        assert!(
            store.screener_waiting().unwrap().is_empty(),
            "no more mail, no more waiting"
        );
        assert_eq!(store.screener_total().unwrap(), 0);
    }

    /// Review E2, golden rule — never lose mail: a No on an INTRUDER
    /// (a screened-out sender who replied in a known sender's thread)
    /// does not hide the known sender's thread. `ecarte` has NO
    /// served view: hiding the mixed thread would make it disappear
    /// everywhere. Only a thread ENTIRELY made of screened-out/waiting
    /// senders gets hidden.
    #[test]
    fn a_no_on_an_intruder_does_not_hide_the_known_senders_thread() {
        let (mut store, inbox) = store_with_mailbox();
        store.set_organized_mode(true, 1_000).unwrap();
        let mut yesterday = envelope(1, "yesterday", 500, true);
        yesterday.sender_address = Some("connu@exemple.fr".to_string());
        let mut root = envelope(2, "project", 1_500, false);
        root.sender_address = Some("connu@exemple.fr".to_string());
        let mut intruder = envelope(3, "Re: project", 1_600, false);
        intruder.sender_address = Some("spam@exemple.fr".to_string());
        intruder.in_reply_to = Some("<m2@example.com>".to_string());
        store
            .upsert_envelopes(inbox, &[yesterday, root, intruder])
            .unwrap();
        // An unknown sender ALONE, screened out too: their thread,
        // entirely theirs, gets hidden — the contrast that proves the
        // rule.
        let mut alone = envelope(4, "offer", 1_700, false);
        alone.sender_address = Some("promo@exemple.fr".to_string());
        store.upsert_envelopes(inbox, &[alone]).unwrap();

        store
            .route_sender("spam@exemple.fr", "ecarte", Some("spam"), 2_000)
            .unwrap();
        store
            .route_sender("promo@exemple.fr", "ecarte", None, 2_000)
            .unwrap();
        let page = store.organized_inbox_scoped(None, false, 0, 10).unwrap();
        assert_eq!(
            page.iter().map(|r| r.envelope.uid).collect::<Vec<_>>(),
            vec![3, 1],
            "the known sender's mixed thread STAYS (intruder head included), the promo-only thread gets hidden"
        );
        assert_eq!(store.organized_inbox_count_scoped(None, false).unwrap(), 2);
        assert!(
            store
                .routing_unified_scoped("kiosque", None, false, 0, 10)
                .unwrap()
                .is_empty(),
            "screened out is not a served view"
        );
    }

    /// A message WITHOUT a Date header NEVER proves the known status:
    /// treating it as predating the epoch would let it bypass the
    /// very gate that exists to sort those senders (spam without a
    /// Date is common) — and would undo a legitimate waiting entry.
    #[test]
    fn a_message_without_a_date_is_never_proof_of_a_known_sender() {
        let (mut store, inbox) = store_with_mailbox();
        store.set_organized_mode(true, 1_000).unwrap();
        let mut without_date = envelope(1, "no date", 0, false);
        without_date.sender_address = Some("nouv@exemple.fr".to_string());
        without_date.date = None;
        store.upsert_envelopes(inbox, &[without_date]).unwrap();
        assert_eq!(
            store.screener_waiting().unwrap().len(),
            1,
            "the dateless unknown sender waits at the gate — never a bypass"
        );

        let mut dated = envelope(2, "dated", 1_500, false);
        dated.sender_address = Some("autre@exemple.fr".to_string());
        let mut without_date2 = envelope(3, "re-no date", 0, false);
        without_date2.sender_address = Some("autre@exemple.fr".to_string());
        without_date2.date = None;
        store
            .upsert_envelopes(inbox, &[dated, without_date2])
            .unwrap();
        assert_eq!(
            store
                .screener_waiting()
                .unwrap()
                .iter()
                .filter(|r| r.address == "autre@exemple.fr")
                .count(),
            1,
            "a second dateless message does not undo the waiting entry"
        );
    }

    /// Reinstating follows the SAME rule as arrival (D3): only a
    /// sender with mail that ARRIVED (INBOX) after the epoch waits at
    /// the Screener again — a sender seen only in Archive or Junk
    /// never went through the gate, and does not enter through the
    /// exit door.
    #[test]
    fn reinstating_only_admits_arrivals() {
        let (mut store, inbox) = store_with_mailbox();
        store.set_organized_mode(true, 1_000).unwrap();
        let archive = store
            .create_mailbox(test_account(&store), "Archives", 1)
            .unwrap();
        let mut outside_the_gate = envelope(1, "seen in archive", 1_500, true);
        outside_the_gate.sender_address = Some("ailleurs@exemple.fr".to_string());
        store
            .upsert_envelopes(archive, &[outside_the_gate])
            .unwrap();
        let mut arrived = envelope(1, "arrived", 1_600, false);
        arrived.sender_address = Some("guichet@exemple.fr".to_string());
        store.upsert_envelopes(inbox, &[arrived]).unwrap();

        store
            .route_sender("ailleurs@exemple.fr", "ecarte", None, 2_000)
            .unwrap();
        store
            .route_sender("guichet@exemple.fr", "ecarte", None, 2_000)
            .unwrap();
        store.remove_routing("ailleurs@exemple.fr").unwrap();
        store.remove_routing("guichet@exemple.fr").unwrap();
        assert_eq!(
            store
                .screener_waiting()
                .unwrap()
                .iter()
                .map(|r| r.address.as_str())
                .collect::<Vec<_>>(),
            vec!["guichet@exemple.fr"],
            "only the arrival reinstates at the gate"
        );
    }

    /// The badge and the gate only report ARRIVALS: a message from
    /// the same sender living elsewhere (trash, archive) is neither
    /// counted nor served as a rank.
    #[test]
    fn the_gate_only_counts_arrivals() {
        let (mut store, inbox) = store_with_mailbox();
        store.set_organized_mode(true, 1_000).unwrap();
        let trash = store
            .create_mailbox(test_account(&store), "Corbeille", 1)
            .unwrap();
        let mut arrived = envelope(1, "arrived", 1_500, false);
        arrived.sender_address = Some("nouv@exemple.fr".to_string());
        store.upsert_envelopes(inbox, &[arrived]).unwrap();
        let mut thrown_away = envelope(1, "already thrown away", 1_600, false);
        thrown_away.sender_address = Some("nouv@exemple.fr".to_string());
        store.upsert_envelopes(trash, &[thrown_away]).unwrap();

        assert_eq!(
            store.screener_total().unwrap(),
            1,
            "the trash does not count"
        );
        let waiting = store.screener_waiting().unwrap();
        assert_eq!(waiting.len(), 1);
        assert_eq!(
            waiting[0].row.envelope.uid, 1,
            "the rank shows the arrival, never the discarded message"
        );
        assert_eq!(waiting[0].row.mailbox, "INBOX");
    }

    /// Shared exclusion extends to PINS and to the nav counter: in
    /// the organized Inbox, a pinned thread routed to the Feed no
    /// longer surfaces (it lives in its own view), and the unread
    /// count of a held-back sender does not inflate the Inbox badge —
    /// classic mode, meanwhile, does not move.
    #[test]
    fn pins_and_the_badge_follow_the_shared_exclusion() {
        let (mut store, inbox) = store_with_mailbox();
        store.set_organized_mode(true, 1_000).unwrap();
        let mut letter = envelope(1, "the letter", 500, false);
        letter.sender_address = Some("lettre@exemple.fr".to_string());
        let ordinary = envelope(2, "hello", 600, false);
        store.upsert_envelopes(inbox, &[letter, ordinary]).unwrap();
        assert!(store.toggle_pin(inbox, 1, 700).unwrap());
        store
            .route_sender("lettre@exemple.fr", "kiosque", None, 2_000)
            .unwrap();
        let mut held_back = envelope(3, "first time", 1_500, false);
        held_back.sender_address = Some("nouv@exemple.fr".to_string());
        store.upsert_envelopes(inbox, &[held_back]).unwrap();

        assert!(
            store
                .pinned_unified_scoped(None, false, true)
                .unwrap()
                .is_empty(),
            "a routed thread's pin no longer surfaces in the organized Inbox"
        );
        assert_eq!(
            store
                .pinned_unified_scoped(None, false, false)
                .unwrap()
                .len(),
            1,
            "classic mode keeps its pin"
        );
        let account = test_account(&store);
        let folders = store.canonical_folders(account).unwrap();
        let (organized, _) = store.nav_unread_counts(account, &folders, true).unwrap();
        assert_eq!(
            organized, 1,
            "only the ordinary unread message counts (the pinned routed one and the held-back one do not)"
        );
        let (classic, _) = store.nav_unread_counts(account, &folders, false).unwrap();
        assert_eq!(classic, 3);
    }

    /// E1 → E2 in the field: the mode may have been ACTIVATED before
    /// this version (E1 in the field, on the CE's workstations) —
    /// unknown senders who arrived between activation and the update
    /// get caught up by the migration, otherwise they would bypass
    /// the gate forever, silently. Fixture: an E2 database whose E2
    /// artifacts (column + waiting entries) are erased to replay the
    /// exact E1 state, then a reopen.
    #[test]
    fn the_migration_catches_up_the_waiting_list_of_a_pre_e2_database() {
        let path = std::env::temp_dir().join(format!(
            "wind-test-rattrapage-portier-{}.db",
            std::process::id()
        ));
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
        }
        {
            let mut store = Store::open(&path).unwrap();
            let account = store
                .adopt_or_create_account("test@exemple.fr", "gmail")
                .unwrap();
            let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
            store.set_organized_mode(true, 1_000).unwrap();
            let mut known = envelope(1, "from yesterday", 500, true);
            known.sender_address = Some("ancien@exemple.fr".to_string());
            let mut unknown = envelope(2, "first time", 1_500, false);
            unknown.sender_address = Some("nouv@exemple.fr".to_string());
            store.upsert_envelopes(inbox, &[known, unknown]).unwrap();
            // Replays E1 state: neither the flag column nor a waiting
            // entry.
            // Reconstruction (not DROP COLUMN: SQLite chokes on the
            // comments in the stored SQL — "incomplete input").
            store
                .0
                .execute_batch(
                    "DELETE FROM portier_attente;
                     PRAGMA foreign_keys = OFF;
                     CREATE TABLE threads_e1 AS
                       SELECT id, account_id, last_mailbox_id, last_uid,
                              last_epoch, size, unseen, inbox_size FROM threads;
                     DROP TABLE threads;
                     ALTER TABLE threads_e1 RENAME TO threads;
                     PRAGMA foreign_keys = ON;",
                )
                .unwrap();
        }
        Store::forget_initialization(&path);
        let store = Store::open(&path).unwrap();
        let waiting = store.screener_waiting().unwrap();
        assert_eq!(
            waiting
                .iter()
                .map(|r| r.address.as_str())
                .collect::<Vec<_>>(),
            vec!["nouv@exemple.fr"],
            "the pre-update unknown sender waits at the gate again"
        );
        assert_eq!(
            store.organized_inbox_count_scoped(None, false).unwrap(),
            1,
            "their thread is held back, the known sender's stays"
        );
        drop(store);
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
        }
    }

    /// PLAN-MODE-ORGANISE E3 — the No rules at sync time. A message
    /// that ARRIVES from a screened-out sender WITH a rule is handled
    /// as PLAN-HORIZON-NETTOYAGE panel B (D5-D8) — the cleanup
    /// session: a single one, persisted; starting freezes the bound
    /// and counts the groups; a GROUP verdict routes the future AND
    /// processes the stock WITHIN THE RANGE (never what precedes it);
    /// progress advances; finishing erases the session.
    #[test]
    fn cleanup_session_groups_verdicts_and_progress() {
        const DAY: i64 = 86_400;
        let now = 100 * DAY;
        let (mut store, inbox) = store_with_mailbox();
        store.set_organized_mode(true, 1_000).unwrap();

        let seed = |uid, subject: &str, epoch, address: &str| {
            let mut e = envelope(uid, subject, epoch, true);
            e.sender_address = Some(address.to_string());
            e
        };
        store
            .upsert_envelopes(
                inbox,
                &[
                    seed(1, "letter", now - 2 * DAY, "un@exemple.fr"),
                    seed(2, "follow-up", now - DAY, "un@exemple.fr"),
                    seed(3, "offer", now - 3 * DAY, "deux@exemple.fr"),
                    // The stock PREDATING the range from the same
                    // sender: never touched by the verdict.
                    seed(5, "very old offer", 500, "deux@exemple.fr"),
                    // A sender entirely outside the range: not a
                    // group.
                    seed(4, "archive", 1_000, "vieux@exemple.fr"),
                    // Already routed (D7): never asked again.
                    seed(6, "news", now - DAY, "route@exemple.fr"),
                    // Yourself: never a group.
                    seed(7, "note to self", now - DAY, "test@exemple.fr"),
                ],
            )
            .unwrap();
        store
            .route_sender("route@exemple.fr", "kiosque", None, 2_000)
            .unwrap();

        assert!(store.cleanup_state().unwrap().is_none());
        assert!(
            store.cleanup_start("un siecle", "reception", now).is_err(),
            "the range vocabulary is closed"
        );
        assert!(
            store.cleanup_start("3m", "le grenier", now).is_err(),
            "the scope vocabulary is closed"
        );

        let session = store.cleanup_start("3m", "reception", now).unwrap();
        assert_eq!((session.total, session.handled), (2, 0));
        let groups = store.cleanup_groups().unwrap();
        assert_eq!(
            groups
                .iter()
                .map(|g| (g.address.as_str(), g.messages))
                .collect::<Vec<_>>(),
            vec![("un@exemple.fr", 2), ("deux@exemple.fr", 1)],
            "the range's groups, most recent first — routed, self and out-of-range excluded"
        );

        // Group Yes: routing only, no server action.
        store
            .cleanup_verdict("un@exemple.fr", "reception", None, now)
            .unwrap();
        assert!(store.pending_actions(inbox).unwrap().is_empty());
        let state = store.cleanup_state().unwrap().unwrap();
        assert_eq!((state.total, state.handled), (2, 1));
        assert_eq!(store.cleanup_groups().unwrap().len(), 1);

        // Navigating into a group: ITS messages from the range,
        // never what precedes it — the reading the sort screen offers
        // on click.
        let inside = store.cleanup_messages("deux@exemple.fr").unwrap();
        assert_eq!(
            inside.iter().map(|r| r.envelope.uid).collect::<Vec<_>>(),
            vec![3],
            "the group shows only its mail from the range"
        );

        // No + trash: the stock WITHIN THE RANGE leaves (uid 3), never
        // what precedes it (uid 5); the action is the server's trash.
        store
            .cleanup_verdict("deux@exemple.fr", "ecarte", Some("corbeille"), now)
            .unwrap();
        let actions = store.pending_actions(inbox).unwrap();
        assert_eq!(
            actions
                .iter()
                .map(|a| (a.uid, a.action.clone()))
                .collect::<Vec<_>>(),
            vec![(3, Action::Delete)],
            "the range's stock only — D4: never a permanent delete"
        );
        let account = test_account(&store);
        assert!(
            store.envelope(account, "INBOX", 5).unwrap().is_some(),
            "what predates the range stays in the database"
        );
        assert!(
            store.envelope(account, "INBOX", 3).unwrap().is_none(),
            "the processed stock leaves the local copy"
        );
        let state = store.cleanup_state().unwrap().unwrap();
        assert_eq!((state.total, state.handled), (2, 2));

        store.cleanup_finish().unwrap();
        assert!(store.cleanup_state().unwrap().is_none());
        assert!(
            store
                .cleanup_verdict("vieux@exemple.fr", "reception", None, now)
                .is_err(),
            "a verdict with no session in progress is refused"
        );
    }

    /// D6 (CE, verbatim): the scope is chosen — "Inbox only" ignores
    /// user folders, "Inbox + Folders" covers them.
    #[test]
    fn cleanup_scope_inbox_or_folders() {
        const DAY: i64 = 86_400;
        let now = 100 * DAY;
        let (mut store, inbox) = store_with_mailbox();
        let account = test_account(&store);
        store.set_organized_mode(true, 1_000).unwrap();
        let projects = store.create_mailbox(account, "Projets", 1).unwrap();

        let mut inbox_msg = envelope(1, "hello", now - DAY, true);
        inbox_msg.sender_address = Some("un@exemple.fr".to_string());
        store.upsert_envelopes(inbox, &[inbox_msg]).unwrap();
        let mut filed = envelope(1, "filed", now - DAY, true);
        filed.sender_address = Some("proj@exemple.fr".to_string());
        store.upsert_envelopes(projects, &[filed]).unwrap();

        let session = store.cleanup_start("tout", "reception", now).unwrap();
        assert_eq!(session.total, 1, "Inbox only: the folder does not enter");
        store.cleanup_finish().unwrap();

        let session = store.cleanup_start("tout", "dossiers", now).unwrap();
        assert_eq!(session.total, 2, "Inbox + Folders: both groups");
        let addresses: Vec<_> = store
            .cleanup_groups()
            .unwrap()
            .into_iter()
            .map(|g| g.address)
            .collect();
        assert!(addresses.contains(&"proj@exemple.fr".to_string()));
    }

    /// Via the gesture path: a logged action (`pending_actions`,
    /// replayed at the head of every sync) + local disappearance — no
    /// echo (this is not a user gesture). `archive` → Archive,
    /// `trash` → Delete (the server's trash, NEVER a permanent
    /// delete — D4).
    #[test]
    fn the_no_rule_runs_on_arrival() {
        let (mut store, inbox) = store_with_mailbox();
        store.set_organized_mode(true, 1_000).unwrap();
        store
            .route_sender("promo@exemple.fr", "ecarte", Some("archive"), 2_000)
            .unwrap();
        store
            .route_sender("pub@exemple.fr", "ecarte", Some("corbeille"), 2_000)
            .unwrap();
        let mut offer = envelope(1, "offer", 2_500, false);
        offer.sender_address = Some("promo@exemple.fr".to_string());
        let mut follow_up = envelope(2, "follow-up", 2_600, false);
        follow_up.sender_address = Some("pub@exemple.fr".to_string());
        store.upsert_envelopes(inbox, &[offer, follow_up]).unwrap();

        assert_eq!(
            store.count(inbox).unwrap(),
            0,
            "both left the local mailbox"
        );
        let actions = store.pending_actions(inbox).unwrap();
        assert_eq!(
            actions
                .iter()
                .map(|a| (a.uid, a.action.clone()))
                .collect::<Vec<_>>(),
            vec![(1, Action::Archive), (2, Action::Delete)],
            "archive → Archive, corbeille → Delete (never permanent)"
        );
    }

    /// The `spam` rule goes to the account's RESOLVED junk folder
    /// (`canonical_folders`, like the gesture); with no recognized
    /// folder, we do NOTHING — never an invented destination (golden
    /// rule).
    #[test]
    fn the_spam_rule_goes_to_the_resolved_junk_folder() {
        let (mut store, inbox) = store_with_mailbox();
        let account = test_account(&store);
        store.set_organized_mode(true, 1_000).unwrap();
        store
            .route_sender("arnaque@exemple.fr", "ecarte", Some("spam"), 2_000)
            .unwrap();
        // With no recognized junk folder: the message STAYS.
        let mut before = envelope(1, "before", 2_500, false);
        before.sender_address = Some("arnaque@exemple.fr".to_string());
        store.upsert_envelopes(inbox, &[before]).unwrap();
        assert_eq!(
            store.count(inbox).unwrap(),
            1,
            "with no recognized folder, nothing moves"
        );
        assert!(store.pending_actions(inbox).unwrap().is_empty());

        store
            .replace_folders(
                account,
                &[crate::Folder {
                    wire: "Junk".to_string(),
                    display: "Junk".to_string(),
                    selectable: true,
                    special_use: None,
                }],
            )
            .unwrap();
        let mut after = envelope(2, "after", 2_600, false);
        after.sender_address = Some("arnaque@exemple.fr".to_string());
        store.upsert_envelopes(inbox, &[after]).unwrap();
        assert_eq!(
            store.count(inbox).unwrap(),
            1,
            "the new one left, the old one stays"
        );
        assert_eq!(
            store
                .pending_actions(inbox)
                .unwrap()
                .iter()
                .map(|a| (a.uid, a.action.clone()))
                .collect::<Vec<_>>(),
            vec![(2, Action::MoveTo("Junk".to_string()))]
        );
    }

    /// D2 — the No rules TURN OFF with the mode: mode disabled, a
    /// message from a screened-out sender with a rule arrives and
    /// STAYS. And a screened-out sender WITHOUT a rule never triggers
    /// anything (a plain No only hides).
    #[test]
    fn the_no_rules_turn_off_with_the_mode() {
        let (mut store, inbox) = store_with_mailbox();
        store.set_organized_mode(true, 1_000).unwrap();
        store
            .route_sender("promo@exemple.fr", "ecarte", Some("archive"), 2_000)
            .unwrap();
        store
            .route_sender("muet@exemple.fr", "ecarte", None, 2_000)
            .unwrap();
        store.set_organized_mode(false, 3_000).unwrap();
        let mut while_off = envelope(1, "while off", 3_500, false);
        while_off.sender_address = Some("promo@exemple.fr".to_string());
        store.upsert_envelopes(inbox, &[while_off]).unwrap();
        assert_eq!(store.count(inbox).unwrap(), 1, "mode off: the rule sleeps");
        assert!(store.pending_actions(inbox).unwrap().is_empty());

        store.set_organized_mode(true, 4_000).unwrap();
        let mut without_rule = envelope(2, "no rule", 4_500, false);
        without_rule.sender_address = Some("muet@exemple.fr".to_string());
        store.upsert_envelopes(inbox, &[without_rule]).unwrap();
        assert_eq!(
            store.count(inbox).unwrap(),
            2,
            "a plain No processes nothing"
        );
        assert!(store.pending_actions(inbox).unwrap().is_empty());
    }

    /// Re-delivery (review E3): a local removal pulls `max_uid` back
    /// — if the replay fails, the next sync re-presents the same uid.
    /// The rule removes it locally again but NEVER logs it twice: a
    /// second identical action on a uid already gone from the server
    /// would jam the whole replay queue behind a permanent failure.
    #[test]
    fn a_redelivery_never_logs_twice() {
        let (mut store, inbox) = store_with_mailbox();
        store.set_organized_mode(true, 1_000).unwrap();
        store
            .route_sender("promo@exemple.fr", "ecarte", Some("archive"), 2_000)
            .unwrap();
        let mut offer = envelope(1, "offer", 2_500, false);
        offer.sender_address = Some("promo@exemple.fr".to_string());
        store
            .upsert_envelopes(inbox, std::slice::from_ref(&offer))
            .unwrap();
        // The server re-presents the same uid (replay not yet run).
        store.upsert_envelopes(inbox, &[offer]).unwrap();
        assert_eq!(store.count(inbox).unwrap(), 0, "removed locally again");
        assert_eq!(
            store
                .pending_actions(inbox)
                .unwrap()
                .iter()
                .map(|a| (a.uid, a.action.clone()))
                .collect::<Vec<_>>(),
            vec![(1, Action::Archive)],
            "ONE action logged"
        );
    }

    /// "Their NEXT messages" (the gate's toasts): the rule only
    /// touches mail AFTER the verdict — a backfill of old mail
    /// (adding an account, sync disorder) never archives or discards
    /// the history. A message WITHOUT a date is treated as arriving
    /// today: the rule applies.
    #[test]
    fn the_rule_never_touches_mail_predating_the_verdict() {
        let (mut store, inbox) = store_with_mailbox();
        store.set_organized_mode(true, 1_000).unwrap();
        store
            .route_sender("promo@exemple.fr", "ecarte", Some("corbeille"), 2_000)
            .unwrap();
        let mut before = envelope(1, "before the verdict", 1_500, true);
        before.sender_address = Some("promo@exemple.fr".to_string());
        let mut without_date = envelope(2, "no date", 0, false);
        without_date.sender_address = Some("promo@exemple.fr".to_string());
        without_date.date = None;
        store
            .upsert_envelopes(inbox, &[before, without_date])
            .unwrap();
        assert_eq!(
            store.count(inbox).unwrap(),
            1,
            "what predates the verdict stays; the dateless one (today's arrival) is processed"
        );
        assert_eq!(
            store
                .pending_actions(inbox)
                .unwrap()
                .iter()
                .map(|a| (a.uid, a.action.clone()))
                .collect::<Vec<_>>(),
            vec![(2, Action::Delete)]
        );
    }

    /// PLAN-MODE-ORGANISE E4 — the organized Inbox's sections
    /// (verdict S1, variant A2): ONE ordered flow "unread first, then
    /// date" — "New for you" then "Already seen" are TWO bounds of
    /// the same paginated source, the seam is the unread COUNT.
    /// Classic mode, meanwhile, does not move a single rank.
    #[test]
    fn the_organized_inbox_serves_unread_first() {
        let (mut store, inbox) = store_with_mailbox();
        store.set_organized_mode(true, 1_000).unwrap();
        store
            .upsert_envelopes(
                inbox,
                &[
                    envelope(1, "read old", 100, true),
                    envelope(2, "unread recent", 200, false),
                    envelope(3, "read recent", 300, true),
                    envelope(4, "unread old", 150, false),
                ],
            )
            .unwrap();
        let organized = store.organized_inbox_scoped(None, false, 0, 10).unwrap();
        assert_eq!(
            organized.iter().map(|r| r.envelope.uid).collect::<Vec<_>>(),
            vec![2, 4, 3, 1],
            "unread first (by date), then read (by date)"
        );
        let account = test_account(&store);
        let bounded = store
            .organized_inbox_scoped(Some(account), false, 0, 10)
            .unwrap();
        assert_eq!(
            bounded.iter().map(|r| r.envelope.uid).collect::<Vec<_>>(),
            vec![2, 4, 3, 1],
            "same order bounded to an account"
        );
        assert_eq!(
            store.organized_inbox_count_scoped(None, true).unwrap(),
            2,
            "the seam: the unread COUNT says where the second section starts"
        );
        // Classic mode, UNTOUCHED: date only.
        let classic = store.unified_recent_scoped(None, false, 0, 10).unwrap();
        assert_eq!(
            classic.iter().map(|r| r.envelope.uid).collect::<Vec<_>>(),
            vec![3, 2, 4, 1]
        );
    }

    /// PLAN-MODE-ORGANISE E5 — Set aside (`pins` pattern: an ENVELOPE
    /// key that survives thread rebuilding, state per THREAD). A
    /// set-aside thread leaves ALL organized views — Inbox, its
    /// routing view, surfaced pins — and lives in the pile; "Done"
    /// returns it to where it came from. CLASSIC mode does not move a
    /// single message.
    #[test]
    fn a_set_aside_thread_lives_in_the_pile_and_returns_when_done() {
        let (mut store, inbox) = store_with_mailbox();
        store.set_organized_mode(true, 1_000).unwrap();
        let mut letter = envelope(1, "the letter", 100, false);
        letter.sender_address = Some("lettre@exemple.fr".to_string());
        let ordinary = envelope(2, "hello", 200, false);
        store.upsert_envelopes(inbox, &[letter, ordinary]).unwrap();
        store
            .route_sender("lettre@exemple.fr", "kiosque", None, 300)
            .unwrap();

        assert!(store.toggle_set_aside(inbox, 2, 1_000).unwrap());
        assert!(store.set_aside_state(inbox, 2).unwrap());
        assert!(
            store
                .organized_inbox_scoped(None, false, 0, 10)
                .unwrap()
                .is_empty(),
            "the set-aside thread leaves the organized Inbox"
        );
        assert_eq!(
            store.organized_inbox_count_scoped(None, false).unwrap(),
            0,
            "the total follows (shared exclusion)"
        );
        assert_eq!(
            store.unified_count_scoped(None, false).unwrap(),
            2,
            "classic mode ALWAYS shows everything"
        );
        // The pile: the thread's mini-card, most recent first.
        assert!(store.toggle_set_aside(inbox, 1, 1_100).unwrap());
        let pile = store.set_aside_pile().unwrap();
        assert_eq!(
            pile.iter().map(|r| r.envelope.uid).collect::<Vec<_>>(),
            vec![2, 1],
            "the pile, most recent to oldest"
        );
        assert!(
            store
                .routing_unified_scoped("kiosque", None, false, 0, 10)
                .unwrap()
                .is_empty(),
            "set aside, the letter ALSO leaves its routing view"
        );

        // "Done": the thread returns TO WHERE IT CAME FROM.
        assert!(!store.toggle_set_aside(inbox, 2, 1_200).unwrap());
        assert_eq!(
            store.organized_inbox_count_scoped(None, false).unwrap(),
            1,
            "the ordinary one returns to the Inbox"
        );
        assert!(!store.toggle_set_aside(inbox, 1, 1_300).unwrap());
        assert_eq!(
            store
                .routing_unified_scoped("kiosque", None, false, 0, 10)
                .unwrap()
                .len(),
            1,
            "the letter returns to the Feed"
        );
        assert!(store.set_aside_pile().unwrap().is_empty());

        // The nav badge follows the pile (E5 capture finding): a
        // set-aside unread no longer counts in organized mode.
        assert!(store.toggle_set_aside(inbox, 2, 1_400).unwrap());
        let account = test_account(&store);
        let folders = store.canonical_folders(account).unwrap();
        let (organized, _) = store.nav_unread_counts(account, &folders, true).unwrap();
        assert_eq!(organized, 0, "the set-aside unread leaves the badge");
        let (classic, _) = store.nav_unread_counts(account, &folders, false).unwrap();
        assert_eq!(classic, 2, "classic mode does not move");
    }

    /// Setting aside follows the THREAD (pins pattern): set on a
    /// message, it holds when a reply moves the head; a set-aside pin
    /// leaves the organized Inbox's surfaced section (classic mode
    /// keeps it).
    #[test]
    fn setting_aside_follows_the_thread_and_removes_the_surfaced_pin() {
        let (mut store, inbox) = store_with_mailbox();
        store.set_organized_mode(true, 1_000).unwrap();
        store
            .upsert_envelopes(inbox, &[envelope(1, "subject", 100, true)])
            .unwrap();
        assert!(store.toggle_pin(inbox, 1, 500).unwrap());
        assert!(store.toggle_set_aside(inbox, 1, 600).unwrap());
        let mut reply = envelope(2, "Re: subject", 700, true);
        reply.in_reply_to = Some("<m1@example.com>".to_string());
        store.upsert_envelopes(inbox, &[reply]).unwrap();

        assert!(
            store.set_aside_state(inbox, 2).unwrap(),
            "the state is read per thread, new head included"
        );
        assert!(
            store
                .pinned_unified_scoped(None, false, true)
                .unwrap()
                .is_empty(),
            "a set-aside thread's pin no longer surfaces in organized mode"
        );
        assert_eq!(
            store
                .pinned_unified_scoped(None, false, false)
                .unwrap()
                .len(),
            1,
            "classic mode keeps its pin"
        );
        // "Done" from the NEW head releases the whole thread.
        assert!(!store.toggle_set_aside(inbox, 2, 800).unwrap());
        assert!(!store.set_aside_state(inbox, 1).unwrap());
    }

    /// A43/A89: setting aside dies with its mail — a reset mailbox
    /// (UIDVALIDITY) and a local removal purge it, a recycled UID
    /// inherits nothing.
    #[test]
    fn setting_aside_dies_with_its_mail() {
        let (mut store, inbox) = store_with_mailbox();
        store
            .upsert_envelopes(
                inbox,
                &[envelope(1, "a", 100, true), envelope(2, "b", 200, true)],
            )
            .unwrap();
        assert!(store.toggle_set_aside(inbox, 1, 300).unwrap());
        store.remove_local(inbox, 1).unwrap();
        assert!(store.set_aside_pile().unwrap().is_empty());

        assert!(store.toggle_set_aside(inbox, 2, 400).unwrap());
        store.reset_mailbox(inbox, 2).unwrap();
        assert!(
            store.set_aside_pile().unwrap().is_empty(),
            "the fresh UIDVALIDITY leaves no phantom set-aside entry"
        );
    }

    /// The organized Inbox's plan guard (S2-bis lesson,
    /// spikes/routage-plan): the page follows the mirrored PARTIAL
    /// index (`idx_threads_date_organise`) — a stable offset by
    /// construction, never a probe per skipped row, never an
    /// envelopes scan.
    #[test]
    fn the_organized_inbox_follows_the_partial_index_never_a_scan() {
        let store = Store::open_in_memory().unwrap();
        let plan: Vec<String> = store
            .0
            .prepare(&format!(
                "EXPLAIN QUERY PLAN {}",
                unified_page_sql(false, false, true)
            ))
            .unwrap()
            .query_map(params![10, 0], |row| row.get::<_, String>(3))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(
            plan.iter().any(|l| l.contains("idx_threads_date_organise")),
            "the page does not follow the partial index: {plan:?}"
        );
        assert!(
            !plan
                .iter()
                .any(|l| l.starts_with("SCAN") && l.contains("envelopes")),
            "plan with an envelopes scan: {plan:?}"
        );
        // E4: the index CARRIES the sectioned sort INSIDE the
        // paginated skeleton — a materialized sort BEFORE the LIMIT
        // would be a sort of the whole mailbox (548 ms measured at
        // spike S1 without the expression index). The EXTERNAL
        // re-sort of the ≤200 retained rows (after "SCAN t") is
        // bounded and legitimate — the section expression is not
        // derived from the join.
        let join = plan
            .iter()
            .position(|l| l == "SCAN t")
            .expect("the plan lost its paginated co-routine");
        assert!(
            !plan[..join].iter().any(|l| l.contains("TEMP B-TREE")),
            "materialized sort INSIDE the paginated skeleton: {plan:?}"
        );
        // Review E4: the OTHER TWO organized paths carry the same
        // guard — the "Mailboxes" view (index prefixed by account)
        // and the Unread tab. Without it, a change of index key would
        // silently bring back S1's materialized sort (548 ms/page).
        for (name, sql, param_n) in [
            (
                "by account",
                unified_page_sql(true, false, true),
                params![10, 0, 1].to_vec(),
            ),
            (
                "unread",
                unified_page_sql(false, true, true),
                params![10, 0].to_vec(),
            ),
        ] {
            let plan: Vec<String> = store
                .0
                .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
                .unwrap()
                .query_map(rusqlite::params_from_iter(param_n), |row| {
                    row.get::<_, String>(3)
                })
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            assert!(
                plan.iter().any(|l| l.contains("idx_threads_date_organise")),
                "organized path \"{name}\" without the partial index: {plan:?}"
            );
            let join = plan
                .iter()
                .position(|l| l == "SCAN t")
                .expect("paginated co-routine missing");
            assert!(
                !plan[..join].iter().any(|l| l.contains("TEMP B-TREE")),
                "organized path \"{name}\": materialized sort in the skeleton: {plan:?}"
            );
        }
    }

    /// The Screener's history reads the list from most recently
    /// decided to oldest — the eye is looking for the latest
    /// decision.
    #[test]
    fn routings_list_from_the_most_recent() {
        let store = Store::open_in_memory().unwrap();
        store
            .route_sender("ancien@ex.fr", "registre", None, 100)
            .unwrap();
        store
            .route_sender("recent@ex.fr", "ecarte", Some("archive"), 200)
            .unwrap();
        let list = store.routings().unwrap();
        assert_eq!(
            list.iter().map(|r| r.address.as_str()).collect::<Vec<_>>(),
            vec!["recent@ex.fr", "ancien@ex.fr"]
        );
        assert_eq!(list[0].rule.as_deref(), Some("archive"));
    }
}

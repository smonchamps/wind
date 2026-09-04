//! The engine's "port" to the network: `mail-core`'s only abstract
//! boundary.
//!
//! The sync engine knows neither IMAP, nor OAuth, nor TLS — only this
//! trait. The real IMAP adapter will implement it (protocol module);
//! tests use a fake server that replays the field's oddities.

use crate::attachment::Attachment;
use crate::envelope::{Envelope, Uid};
use crate::error::Error;

/// What a fetched body reports: the HTML to display, and the
/// description of the files it carries.
///
/// The two travel TOGETHER because they are read from the same bytes.
/// Requesting the attachments separately would cost a second full
/// download of the message for information already seen by the adapter.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FetchedBody {
    pub html: String,
    pub attachments: Vec<Attachment>,
    /// The message's `text/calendar` part, raw — `None` for an ordinary
    /// message. Same logic as the attachments: it is read in the same
    /// bytes, requesting it again would cost a full download
    /// (PLAN-INVITATIONS).
    pub ics: Option<String>,
}

impl FetchedBody {
    /// Body without attachment — the common case, and all the engine's
    /// tests need.
    pub fn html(html: impl Into<String>) -> Self {
        Self {
            html: html.into(),
            attachments: Vec::new(),
            ics: None,
        }
    }
}

/// A message's recipients (To / Cc), raw addresses.
///
/// The stored envelope only carries the sender: "Reply all" therefore
/// re-reads these lists in the server's ENVELOPE at click time — an
/// on-demand round trip, not one extra byte in the database.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MessageRecipients {
    pub to: Vec<String>,
    pub cc: Vec<String>,
}

/// The headers that attach a message to its conversation.
///
/// `None` and `Some("")` do NOT say the same thing: the former means
/// "not yet read", the latter "read, and the message has none".
/// Confusing the two would make the same messages get requested
/// forever.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ThreadHeaders {
    pub in_reply_to: Option<String>,
    pub references: Option<String>,
}

/// A draft read from the server's Drafts folder.
///
/// The body arrives in the two forms MIME can carry, without choosing
/// here: converting HTML to text is a rendering job, and this type is a
/// network boundary. The layer that knows how to render decides.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RemoteDraft {
    /// "To" field as is: a draft is allowed to be incomplete, that is
    /// even its point.
    pub to_raw: String,
    pub subject: String,
    /// `text/plain` part, when there is one.
    pub text: Option<String>,
    /// `text/html` part — often the only one for a draft composed in a
    /// webmail.
    pub html: Option<String>,
}

/// State of a mailbox at the moment it is selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MailboxSnapshot {
    /// Changes when the server invalidates all known UIDs → full resync.
    pub uid_validity: u32,
    /// `Some` if the server supports CONDSTORE (frozen decision: PHASE0.md §2.2).
    pub highest_modseq: Option<u64>,
    /// How many messages the server announces in this mailbox (EXISTS).
    ///
    /// Free: the SELECT reply always carries it, we used to throw it
    /// away. It is the **denominator** of full synchronization progress
    /// ([ADR 0010](../../../docs/adr/0010-full-synchronization.md) §5)
    /// — without it, "12,000 messages fetched" does not say whether we
    /// are a tenth of the way in or at the end.
    pub exists: u32,
}

/// A server folder, under its TWO names.
///
/// `wire` is the protocol one (modified UTF-7): it is the one sent back
/// to the server, and the one logged. `display` is its readable form.
/// Confusing them breaks either the display or the SELECT — they
/// therefore coexist explicitly rather than by convention.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Folder {
    pub wire: String,
    pub display: String,
    /// Can the folder receive a moved message?
    ///
    /// False for containers that carry no mail (`\Noselect` attribute):
    /// offering them would produce a failure on click.
    pub selectable: bool,
    /// The RFC 6154 role announced by the server (`\Trash`, `\All`…) —
    /// `None` when it announces none. It takes precedence over the name
    /// for canonical folders (PLAN-AUDIT-V2 E5: `[Gmail]` was hardcoded,
    /// a "[Google Mail]/…" account lost Archive, Spam and Trash).
    pub special_use: Option<SpecialUse>,
    /// The hierarchy separator announced by LIST (`.`, `/`…) — `None`
    /// for a flat namespace or a server that leaves it unannounced.
    /// No consumer reads it yet (PLAN-AUDIT-V3 E6): it lands so the
    /// model stops silently dropping what the adapter already parses.
    pub delimiter: Option<String>,
}

/// The RFC 6154 roles a folder can carry — what the server KNOWS,
/// against what the name lets you guess.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialUse {
    All,
    Archive,
    Drafts,
    Junk,
    Sent,
    Trash,
}

impl SpecialUse {
    /// The code stored in the database (`folders.special_use`).
    pub fn code(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Archive => "archive",
            Self::Drafts => "drafts",
            Self::Junk => "junk",
            Self::Sent => "sent",
            Self::Trash => "trash",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        Some(match code {
            "all" => Self::All,
            "archive" => Self::Archive,
            "drafts" => Self::Drafts,
            "junk" => Self::Junk,
            "sent" => Self::Sent,
            "trash" => Self::Trash,
            _ => return None,
        })
    }
}

/// A folder's STATUS reading, without selecting it (ADR 0017).
///
/// `uid_next` and `uid_validity` are optional because RFC 3501 does not
/// force a server to serve them: their absence makes `must_poll`
/// conservative — it polls — never wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FolderStatus {
    /// Messages announced (EXISTS).
    pub messages: u32,
    pub uid_next: Option<u32>,
    pub uid_validity: Option<u32>,
    /// HIGHESTMODSEQ (RFC 7162), silent from servers without CONDSTORE.
    /// It is what betrays a flag-ONLY change (E2b, PLAN-SYNCHRO) —
    /// neither UIDNEXT nor MESSAGES move then.
    pub highest_modseq: Option<u64>,
}

/// A folder AND its reading, as LIST-STATUS (RFC 5819) returns them
/// PAIRED in one round trip. The reading is optional: the server may
/// omit it for a folder it stumbles on (RFC 5819 §2).
pub type FolderWithStatus = (Folder, Option<FolderStatus>);

/// A message's server-side flag state, as one `UID FETCH … (UID FLAGS)`
/// line serves it — the D-51 window's currency (PLAN-RETOURS-15 E3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlagState {
    pub uid: Uid,
    pub seen: bool,
    pub flagged: bool,
}

pub trait MailServer {
    /// Selects a mailbox and returns its current state.
    fn select(&mut self, mailbox: &str) -> Result<MailboxSnapshot, Error>;

    /// All UIDs present in the mailbox (any order).
    fn list_uids(&mut self, mailbox: &str) -> Result<Vec<Uid>, Error>;

    /// Envelopes of the requested messages; unknown UIDs are ignored.
    fn fetch_envelopes(&mut self, mailbox: &str, uids: &[Uid]) -> Result<Vec<Envelope>, Error>;

    /// New or modified (flags) messages since `modseq` — CONDSTORE.
    /// Returns `None` if the server does not support the extension; the
    /// engine then falls back to UID differential detection.
    fn changes_since(&mut self, mailbox: &str, modseq: u64)
    -> Result<Option<Vec<Envelope>>, Error>;

    /// The seen/flagged state of the requested messages, in ONE command
    /// (`UID FETCH … (UID FLAGS)` — one short line per message, no
    /// envelope bytes). Serves the D-51 window (PLAN-RETOURS-15 E3): a
    /// server without CONDSTORE offers no delta, so the engine re-reads
    /// a BOUNDED window of recent flags instead. UIDs the server no
    /// longer serves are simply absent from the result.
    ///
    /// Deliberately without a default implementation (the
    /// `fetch_bodies_html` rule): each adapter must state its cost.
    fn fetch_flags(&mut self, mailbox: &str, uids: &[Uid]) -> Result<Vec<FlagState>, Error>;

    /// Bodies of SEVERAL messages in a single command. UIDs the server
    /// no longer serves are simply absent from the result.
    ///
    /// Deliberately without a default implementation: a fallback that
    /// looped it one UID at a time would be silently ruinous. A
    /// per-message round trip costs ~192 ms on a real server
    /// (`spikes/body-backfill`) — catching up a whole mailbox is only
    /// tenable by batching, and each adapter must say so explicitly.
    fn fetch_bodies_html(
        &mut self,
        mailbox: &str,
        uids: &[Uid],
    ) -> Result<Vec<(Uid, FetchedBody)>, Error>;

    /// The thread headers of SEVERAL messages, in one command.
    ///
    /// Separate from the ENVELOPE **by a measurement**: the latter
    /// carries `In-Reply-To` but not `References` (RFC 3501 §7.4.2), and
    /// obtaining `References` requires reading the full header block —
    /// ten times bigger than an envelope. Adding it to synchronization
    /// would multiply tenfold the cost of "envelopes first"; these
    /// headers are therefore fetched AFTERWARD, in the background.
    ///
    /// Now `References` is not a refinement: in an inbox, an exchange's
    /// intermediate message is the one we sent ourselves, and it is not
    /// there. Without it, half the conversations stay split in two.
    fn fetch_thread_headers(
        &mut self,
        mailbox: &str,
        uids: &[Uid],
    ) -> Result<Vec<(Uid, ThreadHeaders)>, Error>;

    /// The BYTES of an attachment, designated by its rank in the
    /// message. `None` if the message or the attachment no longer
    /// exists.
    ///
    /// Deliberately separate from the body: metadata is free and
    /// stored, bytes are paid for on demand and never kept. This is
    /// what leaves ADR 0007's disk budget intact — adding files to it
    /// would blow it up.
    fn fetch_attachment(
        &mut self,
        mailbox: &str,
        uid: Uid,
        index: usize,
    ) -> Result<Option<Vec<u8>>, Error>;

    /// Applies (or removes) the `\Seen` flag server-side.
    fn set_seen(&mut self, mailbox: &str, uid: Uid, seen: bool) -> Result<(), Error>;

    /// Applies (or removes) the `\Flagged` flag — the star.
    fn set_flagged(&mut self, mailbox: &str, uid: Uid, flagged: bool) -> Result<(), Error>;

    /// Takes the message out of the mailbox without deleting it
    /// (archiving).
    fn archive(&mut self, mailbox: &str, uid: Uid) -> Result<(), Error>;

    /// Puts the message in the server's trash.
    fn delete(&mut self, mailbox: &str, uid: Uid) -> Result<(), Error>;

    /// The account's folders, as the user can choose them.
    fn folders(&mut self) -> Result<Vec<Folder>, Error>;

    /// The folders AND their reading, in ONE round trip (LIST-STATUS,
    /// RFC 5819) — what `folders()` + a `folder_status()` per folder do
    /// in ~51 sequential round trips.
    ///
    /// Field, 2026-08-13: the sober cycle held EVERYTHING EXCEPT the
    /// inventory, stuck at 66 s on the Gmail account — ~51 STATUS one by
    /// one. LIST-STATUS melts them into one command.
    ///
    /// `None` = capability absent (the server does not announce
    /// LIST-STATUS): the caller falls back to `folders()` +
    /// `folder_status()`, a complete and tested path. Each folder's
    /// reading is optional even within the reply — RFC 5819 §2 allows
    /// the server to omit it if it stumbles on it; the caller then
    /// treats that folder as unpolled.
    fn folders_with_status(&mut self) -> Result<Option<Vec<FolderWithStatus>>, Error> {
        Ok(None)
    }

    /// A folder's reading — WITHOUT selecting it.
    ///
    /// A single round trip (STATUS in IMAP, designed exactly to query an
    /// unselected mailbox) that serves TWO decisions: the disk-space
    /// guard ([ADR 0010](../../../docs/adr/0010-full-synchronization.md)
    /// §4) which sums `messages` BEFORE committing, and guarded polling
    /// ([ADR 0017](../../../docs/adr/0017-poll-guarded-by-status.md))
    /// — `must_poll` skips folders where nothing moved. `uid_next` and
    /// `uid_validity` are optional: a server that keeps silent on them
    /// makes the decision conservative (we poll), never wrong.
    fn folder_status(&mut self, mailbox: &str) -> Result<FolderStatus, Error>;

    /// Moves the message to `target`, designated by its NETWORK name.
    ///
    /// The operation must be **atomic from the message's point of
    /// view**: it must never be able to disappear from the source
    /// without having arrived at the destination. Same golden rule as
    /// the outbox, applied to sorting.
    fn move_to(&mut self, mailbox: &str, uid: Uid, target: &str) -> Result<(), Error>;
}

//! Business core of the email client.
//!
//! This crate holds the domain model and the sync engine, independent
//! of any UI and any network protocol: it knows neither Tauri, nor the
//! web, nor IMAP. Its only abstract boundary is the [`MailServer`]
//! trait; the real IMAP adapter lives outside the core.

mod action;
mod address;
mod attachment;
mod backfill;
mod body;
mod compose;
mod contacts;
mod crash;
pub mod cycle;
mod drafts;
mod echo;
mod envelope;
mod error;
mod imap_quoted;
mod invitation;
mod nav;
mod notify;
mod outbox;
mod remote;
mod search;
mod store;
mod sync;
#[cfg(test)]
mod test_support;
mod thread;
mod transport;

pub use action::{Action, PendingAction};
pub use address::EmailAddress;
pub use attachment::{Attachment, human_size};
pub use backfill::{
    BACKFILL_BATCH, BackfillReport, HORIZONS_IMPORT, NO_HORIZON, THREAD_HEADER_BATCH,
    backfill_bodies, backfill_percent, backfill_recipients, backfill_thread_headers, horizon_epoch,
};
pub use body::load_body;
pub use compose::{
    Draft, FORWARD_MARKER, ForwardSource, compose, forward_source, forward_subject, quote_forward,
    quote_forward_html, quote_reply, quote_reply_html, reply_all_split, reply_subject, reply_to,
    substitute_forward,
};
pub use contacts::Contact;
pub use crash::{CrashReport, RawPanic, redact};
pub use drafts::{
    DraftAttachmentFull, DraftAttachmentMeta, DraftAttachmentSaved, DraftContent, DraftPull,
    DraftSaved, MAX_ATTACHMENTS_BYTES, SavedDraft, plan_draft_pull,
};
pub use echo::{ECHO_DESTINATIONS, GestureTarget, GroupGesture};
pub use envelope::{Envelope, Uid};
pub use error::Error;
pub use imap_quoted::{unescape_imap_quoted, unescape_imap_quoted_str};
pub use invitation::{
    InvitationRow, StoredInvitation, extract_invitation, participation_de_stable,
};
pub use nav::{CanonicalFolders, NavCounts, PaperTrailGroup};
pub use notify::{Lang, Notification, arrivals_to_notify, notification_for};
pub use outbox::{OutboxAttachment, OutboxMessage, OutboxReport, OutboxState, flush_outbox};
pub use remote::{
    FetchedBody, FlagState, Folder, FolderStatus, FolderWithStatus, MailServer, MailboxSnapshot,
    MessageRecipients, RemoteDraft, SpecialUse, ThreadHeaders,
};
pub use search::WIDE_QUERY_THRESHOLD;
pub use store::{
    Account, AccountConfig, AdoptionProgress, CLEANUP_RANGES, CLEANUP_SCOPES, CleanupGroup,
    CleanupSession, InvitationRank, PREF_ARRIVAL_BUBBLES, PREF_LANG, PREF_LAST_SYNC, Store,
    SyncState, UnifiedRow,
};
pub use sync::{
    LocalMarker, SYNC_BYTES_PER_MESSAGE, SyncEngine, SyncMode, SyncReport, disk_shortfall,
    must_poll, sync_order, sync_percent,
};
pub use transport::{MailTransport, SendError};

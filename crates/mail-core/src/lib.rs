//! Business core of the email client.
//!
//! This crate holds the domain model and the sync engine, independent
//! of any UI and any network protocol: it knows neither Tauri, nor the
//! web, nor IMAP. Its only abstract boundary is the [`MailServer`]
//! trait; the real IMAP adapter lives outside the core.

/// SQLite integers are i64, and rusqlite 0.38 dropped the lossy u64
/// conversions. The domain's counts and sizes are u64 and always fit
/// (bytes, files, uids); the boundary CLAMPS instead of panicking —
/// never an `unwrap` for a value the schema cannot produce.
pub(crate) fn sql_u64(v: u64) -> i64 {
    i64::try_from(v).unwrap_or(i64::MAX)
}
/// The read direction: a negative integer (impossible for our counts
/// and sizes) reads as zero.
pub(crate) fn sql_read_u64(v: i64) -> u64 {
    u64::try_from(v).unwrap_or(0)
}

mod action;
mod address;
mod attachment;
mod backfill;
mod backfill_cursor;
mod disk;
mod operations;
mod work_budget;
pub use disk::BACKGROUND_DISK_RESERVE;
pub use operations::OperationIssue;
pub use work_budget::{FetchLimits, WorkBudget};
mod body;
mod compose;
mod contacts;
mod crash;
pub mod cycle;
mod draft_edit;
mod drafts;
mod echo;
mod envelope;
mod error;
mod flags;
mod images;
mod imap_quoted;
mod invitation;
mod mutations;
mod nav;
mod notify;
mod outbox;
mod remote;
mod remote_drafts;
mod search;
mod store;
mod sync;
#[cfg(test)]
mod test_support;
mod thread;
mod transport;

pub use action::{Action, ActionIncident, PendingAction, RemovalMethod, RemovalPlan, RemovalStep};
pub use address::EmailAddress;
pub use attachment::{Attachment, CALENDAR_ATTACHMENT_INDEX, human_size};
pub use backfill::{
    BACKFILL_BATCH, BackfillReport, HORIZONS_IMPORT, NO_HORIZON, THREAD_HEADER_BATCH,
    backfill_bodies, backfill_bodies_budgeted, backfill_percent, backfill_recipients,
    backfill_recipients_budgeted, backfill_thread_headers, backfill_thread_headers_budgeted,
    horizon_epoch,
};
pub use body::REMOTE_MESSAGE_BYTES;
pub use body::{load_body, load_body_version, refresh_invitation_version};
pub use compose::{
    ComposeMode, Draft, compose, forward_subject, is_own_message, quote_forward,
    quote_forward_html, quote_reply, quote_reply_html, reply_all_recipients, reply_all_split,
    reply_subject, reply_to, signature_applies,
};
pub use contacts::Contact;
pub use crash::{CrashReport, RawPanic, redact};
pub use draft_edit::DraftEdit;
pub use drafts::{
    DraftAttachmentFull, DraftAttachmentMeta, DraftAttachmentSaved, DraftContent, DraftPull,
    DraftSaved, MAX_ATTACHMENTS_BYTES, SavedDraft, plan_draft_pull,
};
pub use echo::{ECHO_DESTINATIONS, GestureTarget, GroupGesture};
pub use envelope::{Envelope, Uid};
pub use error::Error;
pub use imap_quoted::{unescape_imap_quoted, unescape_imap_quoted_str};
pub use invitation::{
    InvitationReplyTarget, InvitationRow, StoredInvitation, extract_invitation,
    participation_de_stable,
};
pub use nav::{CanonicalFolders, NavCounts, PaperTrailGroup};
pub use notify::{Lang, Notification, arrivals_to_notify, notification_for};
pub use outbox::{
    OutboxAttachment, OutboxMessage, OutboxReport, OutboxState, flush_outbox, flush_outbox_while,
};
pub use remote::{
    FetchedBody, FlagState, Folder, FolderStatus, FolderWithStatus, MailServer, MailboxSnapshot,
    MessageRecipients, RemoteDraft, SpecialUse, ThreadHeaders, fetch_attachment_checked,
    verify_mailbox_generation,
};
pub use search::WIDE_QUERY_THRESHOLD;
pub use store::{
    Account, AccountConfig, AdoptionProgress, CLEANUP_RANGES, CLEANUP_SCOPES, CleanupGroup,
    CleanupSession, InvitationRank, MailboxIdentity, PREF_ARRIVAL_BUBBLES, PREF_LANG,
    PREF_LAST_SYNC, Store, SyncState, UnifiedRow,
};
pub use sync::{
    LocalMarker, SYNC_BYTES_PER_MESSAGE, SyncEngine, SyncMode, SyncReport, disk_shortfall,
    must_poll, sync_order, sync_percent,
};
pub use transport::{MailTransport, SendError};

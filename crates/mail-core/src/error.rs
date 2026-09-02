/// Domain errors.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid email address: {0:?}")]
    InvalidEmailAddress(String),

    #[error("storage: {0}")]
    Storage(#[from] rusqlite::Error),

    /// Error raised by an implementation of [`crate::MailServer`]
    /// (network, protocol, authentication…).
    #[error("server: {0}")]
    Server(String),

    /// EXPLICIT refusal from the server (NO/BAD: folder gone,
    /// `[CANNOT]`, `[TRYCREATE]`) — retrying will not change anything.
    /// Everything else (`Server`) is deemed transient: network,
    /// throttling, timeout. This is the distinction the outbox has had
    /// since ADR 0003 (`SendError::{Transient, Permanent}`) and the
    /// action journal did not (2026-09-01 audit S1-7, PLAN-AUDIT-V1 E3).
    #[error("server refusal: {0}")]
    Refusal(String),

    /// Unexpected local data (database modified outside the app).
    #[error("invalid local data: {0}")]
    Corrupt(String),

    /// A closed vocabulary of Organized mode (routing destination, No
    /// rule) received a word outside the table — refused before any
    /// write (PLAN-MODE-ORGANISE E1).
    #[error("invalid routing: {0}")]
    InvalidRouting(String),

    /// The attachment would overflow a message's cap (PJ-D3): nothing is
    /// attached — the refusal happens at the gesture, never at send
    /// time. The sizes let the surface say the remaining room.
    #[error(
        "attachment too large: {name:?} ({size} bytes) exceeds the remaining room ({remaining} bytes)"
    )]
    AttachmentOverBudget {
        name: String,
        size: u64,
        remaining: u64,
    },

    /// The user cancelled the migration of a legacy database during the
    /// adoption pass. Everything was undone (`ROLLBACK`), `user_version`
    /// is unchanged: the whole pass will be replayed on the next launch.
    #[error("migration interrupted")]
    Interrupted,
}

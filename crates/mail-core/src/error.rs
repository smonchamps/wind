/// Domain errors.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(
        "insufficient disk space: {required} bytes required, {available} available; background downloads paused"
    )]
    InsufficientDisk { required: u64, available: u64 },
    #[error("draft editing session is stale or belongs to another composition")]
    StaleDraft,
    #[error("another draft editing session must be closed first")]
    DraftAlreadyOpen,
    #[error("mailbox identity changed; synchronize before retrying")]
    StaleMailbox,

    #[error("check the unresolved delivery before removing this account")]
    UnresolvedDelivery,

    #[error("this send is in progress; wait for its result before deciding")]
    DeliveryInProgress,

    #[error("this send changed; refresh its status before deciding")]
    StaleDelivery,

    #[error("invalid email address: {0:?}")]
    InvalidEmailAddress(String),

    #[error("storage: {0}")]
    Storage(#[from] rusqlite::Error),

    /// Error raised by an implementation of [`crate::MailServer`]
    /// (network, protocol, authentication…).
    #[error("server: {0}")]
    Server(String),

    /// The connection itself failed (DNS, TCP, TLS, greeting): the server
    /// was never reached. Typed since audit lot 4 (E11c / A05): the
    /// reconnect path used to inspect a "connection " string prefix to
    /// decide NOT to refresh an OAuth token during an outage.
    #[error("connection {0}")]
    Connection(String),

    #[error("message {uid} exceeds the {limit}-byte download limit; open it in webmail")]
    RemoteMessageTooLarge { uid: u32, limit: u64 },

    /// EXPLICIT refusal from the server (NO/BAD: folder gone,
    /// `[CANNOT]`, `[TRYCREATE]`) — retrying will not change anything.
    /// Everything else (`Server`) is deemed transient: network,
    /// throttling, timeout. This is the distinction the outbox has had
    /// since ADR 0003 (`SendError::{Transient, Permanent}`) and the
    /// action journal did not (2026-09-01 audit S1-7, PLAN-AUDIT-V1 E3).
    #[error("server refusal: {0}")]
    Refusal(String),

    /// The server says the mailbox does not exist or cannot be opened
    /// (`NO [NONEXISTENT]`, RFC 5530): a container listed without its
    /// `\Noselect`, a label the server no longer serves. Not a failure of
    /// the cycle — the folder leaves the sync scope until the next
    /// inventory says otherwise (field 2026-09-07, lot 4 STOP 2).
    #[error("no such mailbox: {0}")]
    NoSuchMailbox(String),

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

impl Error {
    /// The stable category a surface keys on across the IPC (audit lot 4,
    /// E11c / A05); the prose stays the detail.
    pub fn code(&self) -> &'static str {
        match self {
            Error::InsufficientDisk { .. } => "disk",
            Error::StaleDraft | Error::DraftAlreadyOpen => "draft_session",
            Error::StaleMailbox | Error::StaleDelivery => "stale",
            Error::UnresolvedDelivery | Error::DeliveryInProgress => "delivery_pending",
            Error::InvalidEmailAddress(_) | Error::InvalidRouting(_) => "invalid_input",
            Error::Storage(_) | Error::Corrupt(_) => "local",
            Error::Server(_) => "server",
            Error::Connection(_) => "connection",
            Error::RemoteMessageTooLarge { .. } => "remote_message_too_large",
            Error::Refusal(_) => "refusal",
            Error::NoSuchMailbox(_) => "no_such_mailbox",
            Error::AttachmentOverBudget { .. } => "attachment_budget",
            Error::Interrupted => "interrupted",
        }
    }

    /// Whether the same gesture can succeed later without a change: the
    /// network and the server may; a refusal, an input or local data won't.
    pub fn retryable(&self) -> bool {
        matches!(
            self,
            Error::InsufficientDisk { .. }
                | Error::StaleMailbox
                | Error::StaleDelivery
                | Error::Server(_)
                | Error::Connection(_)
                | Error::Interrupted
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_category_is_stable_and_refusals_never_retry() {
        assert_eq!(Error::Connection("x".into()).code(), "connection");
        assert!(Error::Connection("x".into()).retryable());
        assert!(!Error::Refusal("NO".into()).retryable());
        assert!(!Error::InvalidEmailAddress("a".into()).retryable());
        assert_eq!(Error::Interrupted.code(), "interrupted");
    }
}

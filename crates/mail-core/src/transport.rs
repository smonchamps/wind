//! The core's send port: the SMTP counterpart of [`crate::MailServer`].
//!
//! The transient/permanent distinction is THE decision the core
//! delegates to the adapter: on it depends the fate of a message in the
//! outbox (retry as is, or stop and let the user decide).

use crate::outbox::OutboxMessage;

pub trait MailTransport {
    /// Hands the message to the sending server. Only return `Ok` if the
    /// server ACCEPTED the message in full — it is this acknowledgment
    /// that authorizes the outbox to mark the send as done.
    fn send(&mut self, message: &OutboxMessage) -> Result<(), SendError>;
}

/// Send failure, classified by the conduct to follow.
#[derive(Debug, thiserror::Error)]
pub enum SendError {
    /// Network down, server unreachable or overloaded: the send will be
    /// retried as is at the outbox's next flush.
    #[error("transient failure: {0}")]
    Transient(String),

    /// Definitive refusal from the server (nonexistent recipient,
    /// message rejected): retrying would be pointless — the user
    /// decides.
    #[error("permanent refusal: {0}")]
    Permanent(String),
}

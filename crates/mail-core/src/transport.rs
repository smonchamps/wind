//! The core's send port: the SMTP counterpart of [`crate::MailServer`].
//!
//! The adapter distinguishes safe retries, refusals and uncertain delivery.

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
    /// Proven not accepted: retry at the next flush.
    #[error("transient failure: {0}")]
    Transient(String),

    /// Definitive refusal from the server (nonexistent recipient,
    /// message rejected): retrying would be pointless — the user
    /// decides.
    #[error("permanent refusal: {0}")]
    Permanent(String),

    /// Acceptance cannot be determined. Never retry without a user decision.
    #[error("delivery unknown: {0}")]
    Unknown(String),
}

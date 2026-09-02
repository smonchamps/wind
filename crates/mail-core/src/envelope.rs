use chrono::{DateTime, Utc};

/// IMAP identifier of a message within a mailbox (RFC 3501).
pub type Uid = u32;

/// A message's envelope: the metadata sufficient to display a list
/// without ever downloading the body (the "envelopes first" principle).
///
/// `sender` is a raw display string and not a validated
/// [`crate::EmailAddress`]: a mail client must display what exists,
/// including malformed real-world senders. Strict validation is
/// reserved for addresses WE produce (composition, Phase 2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Envelope {
    pub uid: Uid,
    pub subject: Option<String>,
    pub sender: Option<String>,
    /// Sender's raw address (`mailbox@host`) — to reply, where `sender`
    /// is the display string (decoded name).
    pub sender_address: Option<String>,
    /// Raw To / Cc recipients (`mailbox@host` each), taken from the SAME
    /// ENVELOPE as the sender — free, never an extra byte over the
    /// network (R4, PLAN-RETOURS-MAIL). They serve to display "to X" in
    /// a Sent folder (the sender there is SELF) and "Reply all" offline.
    /// Empty when the ENVELOPE does not carry any.
    pub to_addrs: Vec<String>,
    pub cc_addrs: Vec<String>,
    /// `Reply-To` (first address), taken from the same ENVELOPE: where
    /// "Reply" must write when the sender says so — lists, notifications
    /// (PLAN-AUDIT-V2 E5; dropped before). `None` = reply to the sender.
    pub reply_to: Option<String>,
    /// RFC 5322 `Message-ID` — to reply within the thread
    /// (`In-Reply-To`).
    pub message_id: Option<String>,
    /// `In-Reply-To`: the direct ancestor, as announced by the sender.
    ///
    /// It arrives **for free** with the IMAP ENVELOPE, in the same bytes
    /// as the subject and the sender. This is what makes the first level
    /// of grouping free of network cost; `References`, on the other
    /// hand, requires a separate pass over the full headers.
    pub in_reply_to: Option<String>,
    pub date: Option<DateTime<Utc>>,
    pub seen: bool,
    /// `\Flagged` — the star at Gmail.
    pub flagged: bool,
}

//! Attachments: what we know about a file BEFORE downloading it.
//!
//! The model follows the same rule as the rest of the product: metadata
//! is local and free, bytes are paid for on demand.
//!
//! They cost no extra network round trip: a message's body is already
//! fetched in full ([ADR 0007](../../../docs/adr/0007-rattrapage-des-corps.md)),
//! and this metadata is read in the same bytes. The attachment's
//! **bytes**, though, are never stored: at 62 KB per body the disk
//! budget holds, it would not hold with the files added in.

/// An attachment as it can be described without having downloaded it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attachment {
    /// Rank of the attachment in the message, in the order the MIME
    /// delivers it.
    ///
    /// This is the **re-download key**: replaying the same extraction on
    /// the same message gives back the same rank. Deliberately NOT the
    /// IMAP part number — the arithmetic of `BODY[2.1.3]` is a classic
    /// source of bugs, and it is not needed here.
    pub index: usize,
    /// File name, decoded (RFC 2047) and sanitized on save.
    pub name: String,
    pub mime: String,
    /// Size of the DECODED bytes — the one the user recognizes, not the
    /// base64 source's.
    pub size: u64,
}

impl Attachment {
    /// Human-readable size, for the UI's use.
    pub fn human_size(&self) -> String {
        human_size(self.size)
    }
}

/// Human-readable size — the same form for Reading and the composer
/// (attachment chips, remaining room on a cap refusal).
pub fn human_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    match bytes {
        0..=1023 => format!("{bytes} o"),                         // lang:fr
        n if n < MB => format!("{:.0} Ko", n as f64 / KB as f64), // lang:fr
        n => format!("{:.1} Mo", n as f64 / MB as f64),           // lang:fr
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sized(size: u64) -> Attachment {
        Attachment {
            index: 0,
            name: "f".to_string(),
            mime: "application/pdf".to_string(),
            size,
        }
    }

    #[test]
    fn human_size_changes_unit_where_it_becomes_readable() {
        assert_eq!(sized(0).human_size(), "0 o"); // lang:fr
        assert_eq!(sized(1023).human_size(), "1023 o"); // lang:fr
        assert_eq!(sized(1024).human_size(), "1 Ko"); // lang:fr
        assert_eq!(sized(1_048_576).human_size(), "1.0 Mo"); // lang:fr
        assert_eq!(sized(2_600_000).human_size(), "2.5 Mo"); // lang:fr
    }
}

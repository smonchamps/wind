//! Un-escaping of IMAP `quoted-string`s (RFC 3501 §4.3).
//!
//! `imap-proto` strips the outer quotes of a quoted string but **leaves
//! the raw content**, escapes included (proven by its own `core.rs`
//! tests: `quoted("Hello \" ")` renders `Hello \" `). Without this pass,
//! any subject, sender name or address containing a `"` or a `\` shows
//! up corrupted (R2, PLAN-RETOURS-MAIL).
//!
//! Lives in `mail-core` — not the IMAP adapter — because TWO paths need
//! it: decoding at sync time (`mail-imap`), and repairing envelopes
//! already stored with their escapes (`store.rs` migration, for messages
//! synchronized before the fix).

use std::borrow::Cow;

/// Removes the backslash escapes of an IMAP `quoted-string`: `\"` → `"`,
/// `\\` → `\`, the two ONLY valid sequences (RFC 3501).
///
/// Accepted trade-off: IMAP also transmits strings as a *literal*
/// (`{n}`), where the bytes are raw — and `imap-proto` does not tell us
/// which one it read. Un-escaping would corrupt a literal that genuinely
/// contains `\"` (a very rare case); we settle for the common case, like
/// any mature client. An input without `\` comes back borrowed, with no
/// allocation.
pub fn unescape_imap_quoted(raw: &[u8]) -> Cow<'_, [u8]> {
    if !raw.contains(&b'\\') {
        return Cow::Borrowed(raw);
    }
    let mut out = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        // A `\` is only consumed before `"` or `\`; before anything else
        // (malformed input) it is kept as is, nothing is lost.
        if raw[i] == b'\\' && matches!(raw.get(i + 1), Some(b'"' | b'\\')) {
            out.push(raw[i + 1]);
            i += 2;
        } else {
            out.push(raw[i]);
            i += 1;
        }
    }
    Cow::Owned(out)
}

/// String variant, to repair a value already stored (UTF-8 in the
/// database): un-escapes and renders a `String`. Borrowed without a copy
/// when nothing changes.
pub fn unescape_imap_quoted_str(value: &str) -> Cow<'_, str> {
    match unescape_imap_quoted(value.as_bytes()) {
        Cow::Borrowed(_) => Cow::Borrowed(value),
        // Un-escaping only removes ASCII bytes (`\`), never in the middle
        // of a multi-byte sequence: the result stays valid UTF-8.
        Cow::Owned(bytes) => Cow::Owned(String::from_utf8_lossy(&bytes).into_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_escaped_quotes() {
        assert_eq!(
            unescape_imap_quoted_str(r#"Test \"Sent\""#),
            r#"Test "Sent""#
        );
    }

    #[test]
    fn removes_the_double_backslash() {
        assert_eq!(unescape_imap_quoted_str(r"path C:\\temp"), r"path C:\temp");
    }

    #[test]
    fn a_string_without_backslash_is_borrowed() {
        assert!(matches!(
            unescape_imap_quoted_str("Meeting tomorrow"),
            Cow::Borrowed(_)
        ));
    }

    /// A malformed `\` (before anything but `"` or `\`) survives.
    #[test]
    fn a_lone_backslash_survives() {
        assert_eq!(unescape_imap_quoted_str(r"a\b"), r"a\b");
    }
}

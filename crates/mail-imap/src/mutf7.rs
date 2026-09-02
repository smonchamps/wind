//! Modified UTF-7 (RFC 3501 §5.1.3): the IMAP folder names.
//!
//! IMAP predates UTF-8. A folder "Actualité" travels encoded
//! `Actualit&AOk-`: `&` opens a sequence, `-` closes it, and the content is
//! base64 of UTF-16BE — with `,` in place of `/` in the alphabet. `&-` is
//! the way to write a literal `&`.
//!
//! **This module ONLY decodes for display and comparisons.** The encoded
//! name remains the one sent back to the server: sending "Actualité" where
//! the protocol expects `Actualit&AOk-` would fail the SELECT. Both names
//! must therefore coexist, never replace each other.

use base64::Engine;

/// Decodes an IMAP folder name for the human EYE.
///
/// Cannot fail: a malformed sequence is copied as is. A slightly ugly name
/// beats a vanished folder — the "never lose" rule also holds for what is
/// displayed.
pub(crate) fn decode(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = String::with_capacity(raw.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] != b'&' {
            // Bytes outside a sequence are printable ASCII; we advance
            // character by character to stay UTF-8 safe.
            let rest = &raw[index..];
            let ch = rest.chars().next().unwrap_or('&');
            out.push(ch);
            index += ch.len_utf8();
            continue;
        }
        match bytes[index + 1..].iter().position(|&b| b == b'-') {
            // `&-`: a literal ampersand.
            Some(0) => {
                out.push('&');
                index += 2;
            }
            Some(offset) => {
                let start = index + 1;
                let end = start + offset;
                match decode_segment(&raw[start..end]) {
                    Some(decoded) => out.push_str(&decoded),
                    // Unreadable: copy the raw sequence rather than invent
                    // or lose.
                    None => out.push_str(&raw[index..=end]),
                }
                index = end + 1;
            }
            // Sequence never closed: the rest is copied as is.
            None => {
                out.push_str(&raw[index..]);
                break;
            }
        }
    }
    out
}

/// A segment between `&` and `-`: modified base64 of UTF-16BE.
fn decode_segment(segment: &str) -> Option<String> {
    if segment.is_empty() {
        return None;
    }
    // IMAP's alphabet replaces `/` by `,` — otherwise it is standard
    // base64, without padding.
    let standard = segment.replace(',', "/");
    let bytes = base64::engine::general_purpose::STANDARD_NO_PAD
        .decode(standard)
        .ok()?;
    if bytes.len() % 2 != 0 {
        return None;
    }
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
        .collect();
    // `from_utf16` refuses orphan surrogates: intended, they signal a
    // broken encoding.
    String::from_utf16(&units).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_ascii_is_returned_unchanged() {
        assert_eq!(decode("INBOX"), "INBOX");
        assert_eq!(decode("[Gmail]/Sent Mail"), "[Gmail]/Sent Mail");
        assert_eq!(decode(""), "");
    }

    /// The exact case seen in the field, recorded in ADR 0006: an account
    /// displayed `Actualit&AOk-` instead of "Actualité".
    #[test]
    fn decodes_the_accented_name_seen_in_production() {
        assert_eq!(decode("Actualit&AOk-"), "Actualité");
    }

    #[test]
    fn decodes_several_sequences_in_one_name() {
        assert_eq!(decode("&AOk-t&AOk-"), "été");
        assert_eq!(decode("Dossier/&AOk-l&AOk-ments"), "Dossier/éléments");
    }

    /// `&-` is the only way to write an ampersand: without this case,
    /// "Ventes & Marketing" would become unreadable.
    #[test]
    fn an_escaped_ampersand_comes_back_as_itself() {
        assert_eq!(decode("&-"), "&");
        assert_eq!(decode("Ventes &- Marketing"), "Ventes & Marketing");
    }

    /// Non-Latin scripts: the `,` of the modified alphabet only appears on
    /// some contents, and that is exactly where hand-written decoders go
    /// wrong.
    #[test]
    fn decodes_non_latin_scripts() {
        assert_eq!(decode("&BBIEMAQ2BD0EPg-"), "Важно");
        assert_eq!(decode("&ZeVnLIqe-"), "日本語");
    }

    /// Outside the basic multilingual plane, UTF-16 uses two units.
    #[test]
    fn decodes_a_surrogate_pair() {
        assert_eq!(decode("&2D3es9g93qU-"), "🚳🚥");
    }

    /// A badly encoded name must neither panic nor disappear: it is
    /// displayed as is, and the user at least sees their folder.
    #[test]
    fn malformed_sequences_survive_verbatim() {
        assert_eq!(decode("Actualit&AOk"), "Actualit&AOk");
        assert_eq!(decode("&???-"), "&???-");
        assert_eq!(decode("&"), "&");
        assert_eq!(decode("&AO-"), "&AO-");
    }

    /// Decoding must never lose the wire name: the caller keeps both. This
    /// test documents the rule by showing that a decoded name is NOT
    /// re-encodable here.
    #[test]
    fn decoding_is_not_reversible_here_by_design() {
        let wire = "Actualit&AOk-";
        let display = decode(wire);
        assert_ne!(display, wire, "the display differs from the wire name");
        assert_eq!(
            decode(&display),
            display,
            "re-decoding an already decoded name must break nothing"
        );
    }
}

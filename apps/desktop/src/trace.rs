//! The field trace (PLAN-AUDIT-V1 E9, STANDARD §6.8): a dated line on
//! stderr AND appended to `wind.log` next to the database, bounded to
//! one meg (CE decision D4: truncated, the file starts over from
//! zero).
//!
//! Why a file: the shipped app is a *windows* subsystem, it has no
//! stderr — three updates (0.13.0 → 0.15.0) went by without any
//! measurement surviving, until the `maj.log` poka-yoke (`trace_update`).
//! The same pattern, generalized: poll, pass-after-gesture, drain,
//! watchers, unreadable horizon.
//!
//! What NEVER goes in here (§6.8): no subject, no sender, no body —
//! identifiers, durations, counts, errors. Since audit lot 4 (E11c /
//! S05) the promise no longer rests on every caller: [`sanitize`] runs
//! at the sink on every line — addresses masked, secret-shaped values
//! stripped, line breaks collapsed, length bounded.
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Beyond this, the file is truncated (D4).
pub(crate) const BYTE_LIMIT: u64 = 1_000_000;
const NAME: &str = "wind.log";

static FOLDER: OnceLock<PathBuf> = OnceLock::new();

/// To call ONCE at startup — before that, the trace only goes out on
/// stderr.
pub(crate) fn init(folder: PathBuf) {
    let _ = FOLDER.set(folder);
}

/// Beyond this, a line is cut (a remote error echoing a whole body would
/// otherwise land here).
pub(crate) const LINE_LIMIT: usize = 512;

/// A trace line: stderr (console of a `cargo run`) + `wind.log`. Any
/// write error is ignored — a trace must never make the gesture it
/// describes fail.
pub(crate) fn trace(line: &str) {
    let line = sanitize(line);
    eprintln!("{line}");
    if let Some(folder) = FOLDER.get() {
        write_to(folder, &line);
    }
}

fn is_address_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '.' | '_' | '%' | '+' | '-')
}

fn is_domain_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '.' | '-')
}

/// The sink's own promise (§6.8): whatever a caller formats, the file
/// receives no address, no secret-shaped value, no injected line and no
/// unbounded text. Pure; proven by the tests below.
pub(crate) fn sanitize(line: &str) -> String {
    // 1. Addresses: `local@domain.tld` → `<address>`.
    let chars: Vec<char> = line.chars().collect();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '@' {
            let mut start = i;
            while start > 0 && is_address_char(chars[start - 1]) {
                start -= 1;
            }
            let mut end = i + 1;
            while end < chars.len() && is_domain_char(chars[end]) {
                end += 1;
            }
            let domain: String = chars[i + 1..end].iter().collect();
            if start < i && domain.contains('.') && !domain.ends_with('.') {
                for _ in start..i {
                    out.pop();
                }
                out.push_str("<address>");
                i = end;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    // 2. Secret-shaped values: `key=value` for known keys, bearer tokens.
    let mut cleaned = String::with_capacity(out.len());
    for (index, word) in out.split(' ').enumerate() {
        if index > 0 {
            cleaned.push(' ');
        }
        let lower = word.to_ascii_lowercase();
        let secret_key = [
            "password",
            "passwd",
            "access_token",
            "refresh_token",
            "client_secret",
            "token",
            "secret",
        ]
        .iter()
        .any(|key| lower.starts_with(&format!("{key}=")) || lower.starts_with(&format!("{key}:")));
        if secret_key {
            let key_len = word.find(['=', ':']).map_or(word.len(), |p| p + 1);
            cleaned.push_str(&word[..key_len]);
            cleaned.push_str("<redacted>");
        } else {
            cleaned.push_str(word);
        }
    }
    for marker in ["Bearer ", "bearer "] {
        // Resume AFTER each replacement: the marker itself stays in the
        // line, and a search from the start would loop on it forever.
        let mut from = 0;
        while let Some(found) = cleaned[from..].find(marker) {
            let value_start = from + found + marker.len();
            let value_end = cleaned[value_start..]
                .find(|c: char| c.is_whitespace())
                .map_or(cleaned.len(), |p| value_start + p);
            // Both resume points are char boundaries: right after the
            // ASCII marker, or right after the ASCII replacement.
            from = if value_end > value_start {
                cleaned.replace_range(value_start..value_end, "<redacted>");
                value_start + "<redacted>".len()
            } else {
                value_start
            };
        }
    }
    // 3. No injected line: CR/LF become a visible marker on ONE line.
    let one_line: String = cleaned.replace("\r\n", "\\n").replace(['\r', '\n'], "\\n");
    // 4. Bounded.
    if one_line.len() > LINE_LIMIT {
        let mut cut = LINE_LIMIT;
        while !one_line.is_char_boundary(cut) {
            cut -= 1;
        }
        format!("{}… [cut]", &one_line[..cut])
    } else {
        one_line
    }
}

pub(crate) fn write_to(folder: &Path, line: &str) {
    let _ = std::fs::create_dir_all(folder);
    let path = folder.join(NAME);
    let too_big = std::fs::metadata(&path)
        .map(|m| m.len() >= BYTE_LIMIT)
        .unwrap_or(false);
    let dated = format!(
        "{} {line}\n",
        chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ")
    );
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(!too_big)
        .write(true)
        .truncate(too_big)
        .open(&path)
        .and_then(|mut file| file.write_all(dated.as_bytes()));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_folder(name: &str) -> PathBuf {
        let folder = std::env::temp_dir().join(format!("wind-trace-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        folder
    }

    /// D4: past one meg, the file starts over from zero — never a log
    /// that grows forever next to the database.
    #[test]
    fn the_trace_is_bounded_to_one_meg() {
        let folder = temp_folder("bound");
        let line = "x".repeat(10_000);
        for _ in 0..110 {
            write_to(&folder, &line);
        }
        let size = std::fs::metadata(folder.join(NAME)).unwrap().len();
        assert!(
            size < BYTE_LIMIT + 20_000,
            "truncated past one meg: {size} bytes"
        );
        assert!(size > 0);
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// S05 (audit lot 4): a synthetic address, a subject, a token and an
    /// injected line never reach the file, whatever the caller formatted.
    #[test]
    fn the_sink_masks_addresses_secrets_and_injected_lines() {
        let folder = temp_folder("sanitize");
        let hostile = "poll account 1: server: cannot open mailbox of paul.merand@atelier-nord.fr\r\nsubject: Salaires 2026 password=hunter2 Authorization: Bearer ya29.a0AfH6SMB\nsent";
        let line = sanitize(hostile);
        write_to(&folder, &line);
        let content = std::fs::read_to_string(folder.join(NAME)).unwrap();
        assert_eq!(
            content.lines().count(),
            1,
            "one line, injected breaks neutralized: {content}"
        );
        assert!(!content.contains("paul.merand"), "{content}");
        assert!(!content.contains("atelier-nord"), "{content}");
        assert!(content.contains("<address>"), "{content}");
        assert!(!content.contains("hunter2"), "{content}");
        assert!(!content.contains("ya29"), "{content}");
        assert!(content.contains("password=<redacted>"), "{content}");
        assert!(content.contains("Bearer <redacted>"), "{content}");
        // What the trace is for survives: identifiers, phases, errors.
        assert!(
            content.contains("poll account 1: server: cannot open mailbox"),
            "{content}"
        );
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn the_sink_bounds_a_line_and_keeps_ordinary_text_intact() {
        let long = "x".repeat(LINE_LIMIT * 3);
        let cut = sanitize(&long);
        assert!(cut.len() < LINE_LIMIT + 16, "{}", cut.len());
        assert!(cut.ends_with("[cut]"));
        assert_eq!(
            sanitize("scheduler: body pass: 200 scanned, 3 saved, 0 errors"),
            "scheduler: body pass: 200 scanned, 3 saved, 0 errors"
        );
        assert_eq!(
            sanitize("account 2 @ INBOX"),
            "account 2 @ INBOX",
            "a bare @ is not an address"
        );
        assert_eq!(
            sanitize("user@localhost"),
            "user@localhost",
            "no dot, no address"
        );
    }

    /// Review 2026-09-07: accented addresses are addresses too, and a
    /// bearer marker followed by a break must never cut a character.
    #[test]
    fn accented_addresses_are_masked_and_a_bare_bearer_marker_cannot_panic() {
        assert_eq!(
            sanitize("mailbox of josé@atelier-nord.fr"), // lang:fr
            "mailbox of <address>"
        );
        assert_eq!(sanitize("paul@société.fr said"), "<address> said"); // lang:fr
        assert_eq!(sanitize("françois.x@atelier-nord.fr"), "<address>"); // lang:fr
        let line = sanitize("Authorization: Bearer \nSSéééé"); // lang:fr
        assert!(line.contains("Bearer"), "{line}");
        assert!(!line.contains('\n'));
        assert_eq!(
            sanitize("Bearer a Bearer b"),
            "Bearer <redacted> Bearer <redacted>"
        );
    }

    /// Every line is dated in UTC ISO 8601 — readable afterwards,
    /// alignable with a gesture's timestamp.
    #[test]
    fn every_line_is_dated() {
        let folder = temp_folder("dated");
        write_to(&folder, "poll account 1: INBOX 0.4s");
        let content = std::fs::read_to_string(folder.join(NAME)).unwrap();
        assert!(
            content.starts_with("20")
                && content.contains("T")
                && content.contains("Z poll account 1"),
            "{content}"
        );
        let _ = std::fs::remove_dir_all(&folder);
    }
}

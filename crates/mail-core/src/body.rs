//! On-demand loading of a message body: SQLite cache first, server next,
//! then caching — the "envelopes first" principle applied to the end (the
//! body only arrives on click, then stays offline).

use crate::envelope::Uid;
use crate::error::Error;
use crate::remote::MailServer;
use crate::store::Store;
use rusqlite::{OptionalExtension, params};

pub const REMOTE_MESSAGE_BYTES: u64 = 32 * 1024 * 1024;

pub(crate) fn fetch_bodies_for_store(
    server: &mut dyn MailServer,
    store: &Store,
    identity: &crate::MailboxIdentity,
    uids: &[Uid],
) -> Result<Vec<(Uid, crate::FetchedBody)>, Error> {
    // Disk admission belongs to the background pumps, before their window:
    // this path also serves the user's click, which must read a message
    // whenever the server can deliver it.
    let result =
        crate::remote::fetch_bodies_checked(server, &identity.mailbox, identity.uid_validity, uids);
    if let Err(Error::RemoteMessageTooLarge { uid, limit }) = &result {
        if !uids.contains(uid) {
            return Err(Error::Server(
                "size refusal names an unrequested UID".into(),
            ));
        }
        crate::remote::verify_mailbox_generation(server, &identity.mailbox, identity.uid_validity)?;
        let tx = store.conn().unchecked_transaction()?;
        store.verify_mailbox_identity(identity)?;
        store.conn().execute(
            "INSERT INTO body_download_refusals (mailbox_id, uid, limit_bytes)
             SELECT mailbox_id, uid, ?3 FROM envelopes WHERE mailbox_id = ?1 AND uid = ?2
             ON CONFLICT(mailbox_id, uid) DO UPDATE SET limit_bytes = excluded.limit_bytes",
            params![identity.mailbox_id, uid, limit],
        )?;
        tx.commit()?;
    }
    result
}

/// Load the message selected by a displayed row, including its UID namespace.
pub fn load_body_version(
    server: &mut dyn MailServer,
    store: &mut Store,
    identity: &crate::MailboxIdentity,
    uid: Uid,
) -> Result<Option<String>, Error> {
    if let Some(cached) = store.body_version(identity, uid)? {
        return Ok(Some(cached));
    }
    if let Some(limit) = store.body_download_limit(identity, uid)? {
        return Err(Error::RemoteMessageTooLarge { uid, limit });
    }
    match fetch_bodies_for_store(server, store, identity, &[uid])?
        .into_iter()
        .next()
    {
        Some((_, fetched)) => {
            let invitation = invitation_from(store, identity.account_id, fetched.ics.as_deref())?;
            store.save_body_checked(
                identity,
                uid,
                &fetched.html,
                &fetched.attachments,
                invitation.as_ref(),
            )?;
            Ok(Some(fetched.html))
        }
        None => Ok(None),
    }
}

/// Recover one legacy invitation without making the cached message unavailable.
pub fn refresh_invitation_version(
    server: &mut dyn MailServer,
    store: &Store,
    identity: &crate::MailboxIdentity,
    uid: Uid,
) -> Result<Option<crate::StoredInvitation>, Error> {
    store.verify_mailbox_identity(identity)?;
    let before = store.invitation_version(identity, uid)?;
    if before.as_ref().is_none_or(|i| i.row.metadata_version != 0) {
        return Ok(before);
    }
    if let Some(limit) = store.body_download_limit(identity, uid)? {
        return Err(Error::RemoteMessageTooLarge { uid, limit });
    }
    let (_, fetched) = fetch_bodies_for_store(server, store, identity, &[uid])?
        .into_iter()
        .next()
        .ok_or_else(|| {
            Error::Server("invitation source unavailable; cached message retained".into())
        })?;
    let invitation = invitation_from(store, identity.account_id, fetched.ics.as_deref())?;
    store.save_invitation_metadata_checked(identity, uid, invitation.as_ref())?;
    store.invitation_version(identity, uid)
}

/// Background callers capture the current local namespace before loading.
pub fn load_body(
    server: &mut dyn MailServer,
    store: &mut Store,
    account_id: i64,
    mailbox: &str,
    uid: Uid,
) -> Result<Option<String>, Error> {
    let Some(identity) = store.mailbox_identity(account_id, mailbox)? else {
        return Ok(None);
    };
    load_body_version(server, store, &identity, uid)
}

impl Store {
    pub fn body_download_limit(
        &self,
        identity: &crate::MailboxIdentity,
        uid: Uid,
    ) -> Result<Option<u64>, Error> {
        let tx = self.conn().unchecked_transaction()?;
        self.verify_mailbox_identity(identity)?;
        let limit: Option<u64> = self.conn().query_row(
            "SELECT limit_bytes FROM body_download_refusals WHERE mailbox_id = ?1 AND uid = ?2 AND limit_bytes >= ?3",
            params![identity.mailbox_id, uid, REMOTE_MESSAGE_BYTES], |row| row.get(0),
        ).optional()?;
        tx.commit()?;
        Ok(limit)
    }

    pub fn body_version(
        &self,
        identity: &crate::MailboxIdentity,
        uid: Uid,
    ) -> Result<Option<String>, Error> {
        let tx = self.conn().unchecked_transaction()?;
        self.verify_mailbox_identity(identity)?;
        let body = self.body(identity.account_id, &identity.mailbox, uid)?;
        tx.commit()?;
        Ok(body)
    }
}

/// The invitation row of a calendar part carried with the body — our
/// PARTSTAT is looked up at the account's address (PLAN-INVITATIONS).
pub(crate) fn invitation_from(
    store: &Store,
    account_id: i64,
    ics: Option<&str>,
) -> Result<Option<crate::invitation::InvitationRow>, Error> {
    let Some(ics) = ics else { return Ok(None) };
    let Some(address) = store.account_email(account_id)? else {
        return Ok(None);
    };
    Ok(crate::invitation::extract_invitation(ics, &address))
}

/// Text preview of a body — the gray line under the subject (screen 02 of
/// the redesign). Computed ONCE, when the body is written (`save_body`) or
/// during the bounded backfill (`preview_catchup`) — never on scroll: the
/// list page stays within the P1 gate's cost.
///
/// Tolerant of RAW HTML (the body is stored pre-sanitization): the content
/// of `<style>`, `<script>`, `<title>` and comments is ignored, common
/// entities are decoded, whitespace is collapsed, and the whole thing is
/// truncated to 160 characters without splitting a character.
pub(crate) fn extract_preview(html: &str) -> String {
    const LIMIT: usize = 160;

    // ASCII-insensitive comparisons AT THE BYTE POSITION, never a lowercase
    // copy of the document: some characters change length when lowercased,
    // and an index taken from the copy would panic on the original. Tags
    // and entities are ASCII — that is sufficient.
    fn starts_with(rest: &str, pattern: &str) -> bool {
        rest.len() >= pattern.len()
            && rest
                .as_bytes()
                .iter()
                .zip(pattern.as_bytes())
                .all(|(a, b)| a.eq_ignore_ascii_case(b))
    }
    fn find(rest: &str, pattern: &str) -> Option<usize> {
        (0..=rest.len().saturating_sub(pattern.len()))
            .find(|&start| rest.is_char_boundary(start) && starts_with(&rest[start..], pattern))
    }

    let mut preview = String::new();
    let mut count = 0usize;
    let mut i = 0;
    let bytes = html.as_bytes();
    let mut last_blank = true;
    while i < bytes.len() && count < LIMIT {
        if bytes[i] == b'<' {
            if starts_with(&html[i..], "<!--") {
                i = find(&html[i..], "-->").map_or(html.len(), |end| i + end + 3);
                continue;
            }
            // The containers whose TEXT must never leak into the preview:
            // stylesheets, scripts, document title.
            let mut enclosing = false;
            for tag in ["style", "script", "title"] {
                if starts_with(&html[i + 1..], tag) {
                    let closing = format!("</{tag}");
                    let after = find(&html[i..], &closing)
                        .map_or(html.len(), |end| i + end + closing.len());
                    // Up to and INCLUDING the angle bracket: the whole
                    // `</style>`.
                    i = html[after..]
                        .find('>')
                        .map_or(html.len(), |end| after + end + 1);
                    enclosing = true;
                    break;
                }
            }
            if enclosing {
                continue;
            }
            i = html[i..].find('>').map_or(html.len(), |end| i + end + 1);
            // A tag counts as a blank: `</p><p>` does not glue two words
            // together.
            if !last_blank {
                preview.push(' ');
                last_blank = true;
            }
            continue;
        }
        if bytes[i] == b'&'
            && let Some((length, decoded)) = decode_entity(&html[i..])
        {
            i += length;
            match decoded {
                Some(c) if !c.is_whitespace() && !is_invisible(c) => {
                    preview.push(c);
                    count += 1;
                    last_blank = false;
                }
                // Blank, invisible character (pre-header pegs: &zwnj;,
                // &shy;, thin spaces…) or unknown entity: counts as ONE
                // blank, never a raw "&#8199;" leftover on screen.
                _ => {
                    if !last_blank {
                        preview.push(' ');
                        last_blank = true;
                    }
                }
            }
            continue;
        }
        // Advance by a whole CHARACTER, not a byte.
        let ch = html[i..].chars().next().unwrap_or(' ');
        i += ch.len_utf8();
        if ch.is_whitespace() || is_invisible(ch) {
            if !last_blank {
                preview.push(' ');
                last_blank = true;
            }
        } else {
            preview.push(ch);
            count += 1;
            last_blank = false;
        }
    }
    preview.trim().to_string()
}

/// Decodes ONE HTML entity at the start of `rest` (which starts with `&`).
/// Returns the consumed length and the character — `None` for an unknown
/// entity (still consumed: better a blank than a leftover). Returns a plain
/// `None` if this `&` does not open an entity: it then reads as an
/// ordinary character ("R&D").
fn decode_entity(rest: &str) -> Option<(usize, Option<char>)> {
    let bytes = rest.as_bytes();
    // Numeric: &#233; or &#xE9; — terminated by ";", otherwise it is not
    // an entity.
    if bytes.len() > 2 && bytes[1] == b'#' {
        let (base, start) = if bytes[2] == b'x' || bytes[2] == b'X' {
            (16u32, 3usize)
        } else {
            (10u32, 2usize)
        };
        let end = bytes[start..]
            .iter()
            .position(|o| !o.is_ascii_hexdigit())
            .map(|n| start + n)?;
        if end == start || end - start > 7 || bytes.get(end) != Some(&b';') {
            return None;
        }
        let value = u32::from_str_radix(&rest[start..end], base).ok()?;
        // An invalid or control code point counts as a blank.
        let c = char::from_u32(value).filter(|c| !c.is_control());
        return Some((end + 1, c));
    }
    // Named: &name; — ASCII name of 2 to 32 characters.
    let end = bytes[1..]
        .iter()
        .position(|o| !o.is_ascii_alphanumeric())
        .map(|n| 1 + n)?;
    if !(3..=33).contains(&end) || bytes.get(end) != Some(&b';') || !bytes[1].is_ascii_alphabetic()
    {
        return None;
    }
    let name = &rest[1..end];
    Some((end + 1, named_entity(name)))
}

/// The decoded named entities — the ones from real mail: HTML structure,
/// accented letters (Latin-1), typography. An entity missing here is
/// consumed and counts as a blank — the preview NEVER shows a raw
/// "&eacute;".
fn named_entity(name: &str) -> Option<char> {
    Some(match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "eacute" => 'é',
        "egrave" => 'è',
        "ecirc" => 'ê',
        "euml" => 'ë',
        "agrave" => 'à',
        "acirc" => 'â',
        "aacute" => 'á',
        "ccedil" => 'ç',
        "ocirc" => 'ô',
        "ouml" => 'ö',
        "oacute" => 'ó',
        "otilde" => 'õ',
        "ugrave" => 'ù',
        "ucirc" => 'û',
        "uuml" => 'ü',
        "uacute" => 'ú',
        "icirc" => 'î',
        "iuml" => 'ï',
        "iacute" => 'í',
        "ntilde" => 'ñ',
        "aelig" => 'æ',
        "oelig" => 'œ',
        "szlig" => 'ß',
        "aring" => 'å',
        "oslash" => 'ø',
        "yuml" => 'ÿ',
        "Eacute" => 'É',
        "Egrave" => 'È',
        "Ecirc" => 'Ê',
        "Agrave" => 'À',
        "Acirc" => 'Â',
        "Ccedil" => 'Ç',
        "Ocirc" => 'Ô',
        "AElig" => 'Æ',
        "OElig" => 'Œ',
        "rsquo" => '’',
        "lsquo" => '‘',
        "rdquo" => '”',
        "ldquo" => '“',
        "hellip" => '…',
        "ndash" => '–',
        "mdash" => '—',
        "laquo" => '«',
        "raquo" => '»',
        "middot" => '·',
        "bull" => '•',
        "deg" => '°',
        "euro" => '€',
        "copy" => '©',
        "reg" => '®',
        "trade" => '™',
        "times" => '×',
        "divide" => '÷',
        "plusmn" => '±',
        "sup2" => '²',
        "sup3" => '³',
        "frac12" => '½',
        "frac14" => '¼',
        "frac34" => '¾',
        "sect" => '§',
        "para" => '¶',
        "minus" => '−',
        // Blanks and invisible pegs: decoded to their character,
        // `is_invisible`/`is_whitespace` collapse them into a blank.
        "nbsp" => '\u{00A0}',
        "ensp" => '\u{2002}',
        "emsp" => '\u{2003}',
        "thinsp" => '\u{2009}',
        "zwnj" => '\u{200C}',
        "zwj" => '\u{200D}',
        "shy" => '\u{00AD}',
        "lrm" => '\u{200E}',
        "rlm" => '\u{200F}',
        _ => return None,
    })
}

/// True if the text still contains a well-formed HTML entity, OR ends with
/// a TRUNCATED entity ("…&#12852": the earlier decoder cut at 160 in the
/// middle of an entity) — the criterion for repairing previews (migrate).
/// Over-broad by a hair at the end of a text ("…R&D" re-matches): the
/// repair is ONE flagged pass, the only cost is a recompute.
pub(crate) fn contains_residual_entity(text: &str) -> bool {
    let whole = text
        .char_indices()
        .filter(|(_, c)| *c == '&')
        .any(|(i, _)| decode_entity(&text[i..]).is_some());
    let truncated_tail = text.rfind('&').is_some_and(|i| {
        let after = &text[i + 1..];
        !after.is_empty()
            && after
                .bytes()
                .all(|o| o.is_ascii_alphanumeric() || o == b'#')
    });
    whole || truncated_tail
}

/// The formatting characters with no glyph: pre-header pegs from
/// newsletters (&zwnj;, &shy;, U+034F…). In a one-line preview, they
/// count as a blank.
fn is_invisible(c: char) -> bool {
    matches!(
        c,
        '\u{00AD}'
            | '\u{034F}'
            | '\u{200B}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{2064}'
            | '\u{FEFF}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::FakeServer;

    #[test]
    fn legacy_invitation_refresh_keeps_cached_body_and_local_reply_across_failures() {
        let (mut server, store, account) = synced_setup();
        let identity = store.mailbox_identity(account, "INBOX").unwrap().unwrap();
        let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nMETHOD:REQUEST\r\nBEGIN:VEVENT\r\nUID:legacy\r\nORGANIZER:mailto:owner@example.fr\r\nRECURRENCE-ID:20260908T120000Z\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let mut legacy = crate::extract_invitation(ics, "test@exemple.fr").unwrap();
        legacy.metadata_version = 0;
        legacy.occurrence_key = None;
        legacy.occurrence_property = None;
        store
            .save_body_full(
                identity.mailbox_id,
                1,
                "<p>Available offline</p>",
                &[],
                Some(&legacy),
            )
            .unwrap();
        store
            .conn()
            .execute(
                "UPDATE invitations SET reponse='accepte', reponse_epoch=42",
                [],
            )
            .unwrap();
        server.bodies.clear();
        assert!(refresh_invitation_version(&mut server, &store, &identity, 1).is_err());
        assert_eq!(
            store.body(account, "INBOX", 1).unwrap().as_deref(),
            Some("<p>Available offline</p>")
        );
        assert_eq!(
            store
                .invitation(account, "INBOX", 1)
                .unwrap()
                .unwrap()
                .row
                .metadata_version,
            0
        );
        server.bodies.insert(1, "remote body".into());
        server.ics.insert(1, ics.into());
        let repaired = refresh_invitation_version(&mut server, &store, &identity, 1)
            .unwrap()
            .unwrap();
        assert!(repaired.row.can_reply());
        assert!(repaired.row.occurrence_property.is_some());
        assert_eq!(repaired.reply.as_deref(), Some("accepte"));
        assert_eq!(repaired.reply_epoch, Some(42));
        assert_eq!(
            store.body(account, "INBOX", 1).unwrap().as_deref(),
            Some("<p>Available offline</p>")
        );
        assert_eq!(server.body_batches, vec![vec![1], vec![1]]);
        refresh_invitation_version(&mut server, &store, &identity, 1).unwrap();
        assert_eq!(server.body_batches.len(), 2, "already verified: no network");
    }

    #[test]
    fn preview_ignores_styles_scripts_and_comments() {
        let html = "<html><head><title>Hidden title</title>\n<style>p { color: red; }</style></head>\
                    <body><!-- note --><p>Hello&nbsp;Paul,</p><p>it&#39;s the essential &amp; the rest.</p>\
                    <script>var x = 1;</script></body></html>";
        assert_eq!(
            extract_preview(html),
            "Hello Paul, it's the essential & the rest."
        );
    }

    #[test]
    fn preview_collapses_whitespace_and_passes_raw_text() {
        // "Bonjour, deux créneaux se chevauchent." (French, accented word) —
        // exercises whitespace collapsing over raw multi-byte UTF-8 text.
        assert_eq!(
            extract_preview("Bonjour,\n\n   deux  créneaux\tse chevauchent."), // lang:fr
            "Bonjour, deux créneaux se chevauchent."                           // lang:fr
        );
    }

    #[test]
    fn preview_decodes_numeric_and_named_entities() {
        // The REAL pattern from the field: accents as decimal, hex and
        // named entities — plus the typographic apostrophe. (French text,
        // accents are the point of the test.)
        assert_eq!(
            extract_preview(
                "Vos r&#233;f&#233;rences ont &#xE9;t&#xE9; re&ccedil;ues, merci d&rsquo;avoir voyag&eacute;." // lang:fr
            ),
            "Vos références ont été reçues, merci d’avoir voyagé." // lang:fr
        );
    }

    #[test]
    fn preview_collapses_invisible_pegs_into_a_blank() {
        // Newsletter pre-header pegs: zwnj, shy, thin spaces as entities —
        // never a raw "&#8199;" leftover on screen. (French text, accents
        // are the point of the test.)
        assert_eq!(
            extract_preview(
                "R&#233;compense&#847;&zwnj;&#8199;&shy;&zwnj; &#8202; d&eacute;bloqu&eacute;e" // lang:fr
            ),
            "Récompense débloquée" // lang:fr
        );
        // An UNKNOWN entity counts as a blank, not a leftover.
        assert_eq!(
            extract_preview("avant&inconnue;apr&egrave;s"), // lang:fr
            "avant après"                                   // lang:fr
        );
        // An ordinary "&" stays a character: R&D.
        assert_eq!(extract_preview("R&D et &#litige"), "R&D et &#litige"); // lang:fr
    }

    #[test]
    fn repair_criterion_catches_entities_and_truncated_tails() {
        // Well-formed entity in the middle — the bulk case from the field.
        // (French text, accents are the point of the test.)
        assert!(contains_residual_entity("Vos r&#233;f&#233;rences")); // lang:fr
        assert!(contains_residual_entity("voyag&eacute; loin")); // lang:fr
        // Entity TRUNCATED by the old decoder's cut at 160.
        assert!(contains_residual_entity("des journ es &#12852")); // lang:fr
        assert!(contains_residual_entity("fin coup&eacu")); // lang:fr
        // Clean text: nothing to repair.
        assert!(!contains_residual_entity(
            "références décodées, R&D comprise." // lang:fr
        ));
        assert!(!contains_residual_entity("aucune esperluette")); // lang:fr
    }

    #[test]
    fn preview_truncates_at_160_without_splitting_a_character() {
        // Accented character repeated 400 times: exercises truncation
        // exactly at a character boundary. (French letter, the accent is
        // the point of the test.)
        let long = "é".repeat(400); // lang:fr
        let preview = extract_preview(&long);
        assert_eq!(preview.chars().count(), 160);
        assert!(preview.chars().all(|c| c == 'é')); // lang:fr
    }

    fn synced_setup() -> (FakeServer, Store, i64) {
        let mut server = FakeServer::new(false);
        server.add_with_body(1, "subject", "<p>message body</p>");
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("test@exemple.fr", "gmail")
            .unwrap();
        crate::SyncEngine::default()
            .sync(&mut server, &mut store, account, "INBOX")
            .unwrap();
        (server, store, account)
    }

    #[test]
    fn size_refusal_survives_reopen_and_cached_content_still_wins() {
        let (mut server, mut store, account) = synced_setup();
        let identity = store.mailbox_identity(account, "INBOX").unwrap().unwrap();
        server.oversized_body = Some(1);
        assert!(load_body_version(&mut server, &mut store, &identity, 1).is_err());
        let path = std::env::temp_dir().join(format!(
            "wind-size-refusal-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        store
            .conn()
            .execute("VACUUM INTO ?1", [path.to_str().unwrap()])
            .unwrap();
        drop(store);
        let mut store = Store::open(&path).unwrap();
        assert_eq!(
            store.body_download_limit(&identity, 1).unwrap(),
            Some(REMOTE_MESSAGE_BYTES)
        );
        let calls = server.body_batches.len();
        assert!(matches!(
            load_body_version(&mut server, &mut store, &identity, 1),
            Err(Error::RemoteMessageTooLarge { .. })
        ));
        assert_eq!(server.body_batches.len(), calls);
        store
            .save_body(identity.mailbox_id, 1, "<p>previously cached</p>", &[])
            .unwrap();
        assert_eq!(
            load_body_version(&mut server, &mut store, &identity, 1)
                .unwrap()
                .as_deref(),
            Some("<p>previously cached</p>")
        );
        drop(store);
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
        }
    }

    #[test]
    fn a_failed_size_refusal_write_remains_retryable() {
        let (mut server, mut store, account) = synced_setup();
        let identity = store.mailbox_identity(account, "INBOX").unwrap().unwrap();
        server.oversized_body = Some(1);
        store.conn().execute_batch("CREATE TRIGGER refuse_size_write BEFORE INSERT ON body_download_refusals BEGIN SELECT RAISE(ABORT, 'synthetic refusal'); END;").unwrap();
        assert!(matches!(
            load_body_version(&mut server, &mut store, &identity, 1),
            Err(Error::Storage(_))
        ));
        assert_eq!(store.body_download_limit(&identity, 1).unwrap(), None);
        assert_eq!(
            store.bodies_to_backfill(account, "INBOX", 0, 10).unwrap(),
            [1]
        );
        store
            .conn()
            .execute_batch("DROP TRIGGER refuse_size_write;")
            .unwrap();
        assert!(matches!(
            load_body_version(&mut server, &mut store, &identity, 1),
            Err(Error::RemoteMessageTooLarge { .. })
        ));
    }

    #[test]
    fn displayed_message_version_cannot_read_a_reused_uid_from_cache() {
        let (mut server, mut store, account) = synced_setup();
        let identity = store.mailbox_identity(account, "INBOX").unwrap().unwrap();
        server.uid_validity += 1;
        crate::SyncEngine::default()
            .sync(&mut server, &mut store, account, "INBOX")
            .unwrap();
        load_body(&mut server, &mut store, account, "INBOX", 1).unwrap();
        assert!(matches!(
            load_body_version(&mut server, &mut store, &identity, 1),
            Err(Error::StaleMailbox)
        ));
    }

    #[test]
    fn refuses_a_body_from_a_reused_uid_generation() {
        let (mut server, mut store, account) = synced_setup();
        server.uid_validity += 1;
        server
            .bodies
            .insert(1, "<p>Unrelated replacement</p>".into());

        assert!(load_body(&mut server, &mut store, account, "INBOX", 1).is_err());
        assert_eq!(store.body(account, "INBOX", 1).unwrap(), None);
        assert!(server.body_batches.is_empty());
    }

    #[test]
    fn refuses_a_generation_change_during_body_fetch() {
        let (mut server, mut store, account) = synced_setup();
        server.reset_during_body_fetch = true;
        assert!(load_body(&mut server, &mut store, account, "INBOX", 1).is_err());
        assert_eq!(store.body(account, "INBOX", 1).unwrap(), None);
    }

    #[test]
    fn ignores_a_body_returned_for_another_uid() {
        let (mut server, mut store, account) = synced_setup();
        server.body_reply_uid = Some(2);
        assert_eq!(
            load_body(&mut server, &mut store, account, "INBOX", 1).unwrap(),
            None
        );
        assert_eq!(store.body(account, "INBOX", 1).unwrap(), None);
    }

    #[test]
    fn checked_cache_write_refuses_a_local_reset() {
        let (_, store, account) = synced_setup();
        let before = store.mailbox_identity(account, "INBOX").unwrap().unwrap();
        store
            .reset_mailbox(before.mailbox_id, before.uid_validity + 1)
            .unwrap();
        assert!(matches!(
            store.save_body_checked(&before, 1, "stale bytes", &[], None),
            Err(Error::StaleMailbox)
        ));
        assert_eq!(store.body(account, "INBOX", 1).unwrap(), None);
    }

    #[test]
    fn checked_cache_write_refuses_a_reassigned_mailbox_id() {
        let (_, store, account) = synced_setup();
        let before = store.mailbox_identity(account, "INBOX").unwrap().unwrap();
        let other = store
            .adopt_or_create_account("other@example.com", "gmail")
            .unwrap();
        store
            .conn()
            .execute(
                "UPDATE mailboxes SET account_id = ?1 WHERE id = ?2",
                rusqlite::params![other, before.mailbox_id],
            )
            .unwrap();
        assert!(matches!(
            store.save_body_checked(&before, 1, "old account body", &[], None),
            Err(Error::StaleMailbox)
        ));
        assert_eq!(store.body(other, "INBOX", 1).unwrap(), None);
    }

    #[test]
    fn attachment_fetch_refuses_a_reset_before_or_during_download() {
        for during in [false, true] {
            let (mut server, _, _) = synced_setup();
            server
                .attachment_bytes
                .insert((1, 0), b"unrelated bytes".to_vec());
            if during {
                server.reset_during_attachment_fetch = true;
            } else {
                server.uid_validity += 1;
            }
            assert!(matches!(
                crate::fetch_attachment_checked(&mut server, "INBOX", 1, 1, 0),
                Err(Error::StaleMailbox)
            ));
        }
        let (mut server, _, _) = synced_setup();
        server
            .attachment_bytes
            .insert((1, 0), b"requested bytes".to_vec());
        assert_eq!(
            crate::fetch_attachment_checked(&mut server, "INBOX", 1, 1, 0).unwrap(),
            Some(b"requested bytes".to_vec())
        );
    }

    #[test]
    fn fetches_then_serves_from_cache() {
        let (mut server, mut store, account) = synced_setup();

        let first = load_body(&mut server, &mut store, account, "INBOX", 1).unwrap();
        assert_eq!(first.as_deref(), Some("<p>message body</p>"));
        assert_eq!(server.body_batches.len(), 1);

        let second = load_body(&mut server, &mut store, account, "INBOX", 1).unwrap();
        assert_eq!(second.as_deref(), Some("<p>message body</p>"));
        assert_eq!(
            server.body_batches.len(),
            1,
            "the cache must avoid the server"
        );
    }

    /// PLAN-INVITATIONS: the calendar part travels with the body and ends
    /// up as an `invitations` row — our PARTSTAT looked up at the
    /// account's address.
    #[test]
    fn body_reports_its_invitation_and_stores_it() {
        let (mut server, mut store, account) = synced_setup();
        server.ics.insert(
            1,
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nMETHOD:REQUEST\r\n\
             BEGIN:VEVENT\r\nUID:r1@exemple.fr\r\nSUMMARY:Project sync\r\n\
             DTSTART:20260903T123000Z\r\n\
             ORGANIZER;CN=Claire Martin:mailto:claire@exemple.fr\r\n\
             ATTENDEE;PARTSTAT=NEEDS-ACTION:mailto:test@exemple.fr\r\n\
             END:VEVENT\r\nEND:VCALENDAR\r\n"
                .to_string(),
        );

        load_body(&mut server, &mut store, account, "INBOX", 1).unwrap();

        let stored = store
            .invitation(account, "INBOX", 1)
            .unwrap()
            .expect("the invitation row");
        assert_eq!(stored.row.method, "request");
        assert_eq!(stored.row.title, "Project sync");
        assert_eq!(stored.row.partstat.as_deref(), Some("sans_reponse"));
    }

    #[test]
    fn returns_none_for_vanished_message() {
        let (mut server, mut store, account) = synced_setup();
        assert_eq!(
            load_body(&mut server, &mut store, account, "INBOX", 99).unwrap(),
            None
        );
    }

    #[test]
    fn returns_none_before_first_sync_without_touching_server() {
        let mut server = FakeServer::new(false);
        server.add_with_body(1, "subject", "<p>x</p>");
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("test@exemple.fr", "gmail")
            .unwrap();

        assert_eq!(
            load_body(&mut server, &mut store, account, "INBOX", 1).unwrap(),
            None
        );
        assert_eq!(server.body_batches.len(), 0);
    }
}

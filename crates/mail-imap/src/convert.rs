//! Translation of the IMAP replies into domain types.
//!
//! Headers arrive RFC 2047-encoded (`=?UTF-8?Q?…?=`) and fragmented: the
//! decoding is delegated to `mail-parser` (frozen decision, PHASE0.md §2.3),
//! never rewritten by hand.

use chrono::Utc;
use imap_proto::types::{Address, Envelope as ProtoEnvelope};
use mail_core::{Envelope, Uid, unescape_imap_quoted};

/// Special role of a folder (RFC 6154), reduced to what decides archiving.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpecialUse {
    Archive,
    All,
    /// `\Sent` — where the server stores OUR sent messages.
    Sent,
    Other,
}

/// What "archive" means on THIS server.
///
/// Inferred from its announced capabilities, **never from the provider**:
/// the same discipline as the discovery of the trash and the drafts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ArchiveStrategy {
    /// The server exposes `\Archive`: copy the message there, then expunge.
    MoveTo(String),
    /// The server exposes `\All` (Gmail semantics): expunging from INBOX only
    /// removes the label, the message survives in "All Mail".
    ExpungeOnly,
    /// Neither: expunging would DESTROY the message. We refuse.
    Unsupported,
}

/// Fallback names, when the server announces no archive attribute.
///
/// Deliberate exception to the "never a hard-coded name" rule, justified by
/// measurement: Exchange Online announces `\Drafts`, `\Junk`, `\Sent` and
/// `\Trash`, but **not** `\Archive` — while the "Archive" folder exists and
/// serves (spikes/microsoft, real account). Without this fallback, archiving
/// would be unavailable on every Microsoft account. The list stays
/// deliberately short: an unknown name beats a wrong choice.
const ARCHIVE_FALLBACK_NAMES: [&str; 4] = ["archive", "archives", "archivé", "archivés"]; // lang:fr server names

/// Chooses the archiving strategy from the announced folders.
///
/// Priority order, from the safest to the least safe:
/// 1. `\Archive` announced — the server's intent, without ambiguity;
/// 2. `\All` announced — Gmail semantics, where expunging IS archiving;
/// 3. a folder named "Archive" — measured fallback (see above);
/// 4. otherwise: refusal. "Never lose a mail" (PLAN.md §1) wins over the
///    comfort of a feature.
pub(crate) fn archive_strategy<'a>(
    folders: impl IntoIterator<Item = (&'a str, SpecialUse)>,
) -> ArchiveStrategy {
    let mut has_all = false;
    let mut named: Option<String> = None;
    for (name, role) in folders {
        match role {
            SpecialUse::Archive => return ArchiveStrategy::MoveTo(name.to_string()),
            SpecialUse::All => has_all = true,
            // The sent folder is never an archiving destination — and it
            // must not fall into the name fallback below either.
            SpecialUse::Sent => {}
            SpecialUse::Other => {
                // The FULL name must match: "Archive/Achats" is a filing,
                // not the archiving destination. Comparison on the DECODED
                // name: a French server announces `Archiv&AOk-s`, which the
                // list would never recognize in its wire form. What is
                // memorized, however, remains the wire name — it is what we
                // will send back.
                if named.is_none()
                    && ARCHIVE_FALLBACK_NAMES
                        .contains(&crate::mutf7::decode(name).to_lowercase().as_str())
                {
                    named = Some(name.to_string());
                }
            }
        }
    }
    if has_all {
        return ArchiveStrategy::ExpungeOnly;
    }
    match named {
        Some(folder) => ArchiveStrategy::MoveTo(folder),
        None => ArchiveStrategy::Unsupported,
    }
}

/// Fallback names for the sent folder, when the server does not announce
/// `\Sent`.
///
/// Same deliberate exception as for archiving, and for the same reason: a
/// real server does not always announce what it owns. The list stays short
/// — a folder not found degrades cleanly (the threads only group the
/// received messages), a WRONG folder would sync foreign mail into the
/// conversations.
const SENT_FALLBACK_NAMES: [&str; 5] = [
    "sent",
    "sent items",
    "envoyé",           // lang:fr server name
    "envoyés",          // lang:fr server name
    "éléments envoyés", // lang:fr server name
];

/// Where the server stores OUR sent messages, if it says so or if its name
/// gives it away.
///
/// Priority order, from the safest to the least safe — the one of
/// [`archive_strategy`]:
/// 1. `\Sent` announced: the server's intent, without ambiguity;
/// 2. a folder whose full name is known (fallback);
/// 3. otherwise `None`, and the account works as before: the threads only
///    group the received messages. A local and silent degradation, never an
///    error ([ADR 0009] §7).
pub(crate) fn sent_folder<'a>(
    folders: impl IntoIterator<Item = (&'a str, SpecialUse)>,
) -> Option<String> {
    let mut named: Option<String> = None;
    for (name, role) in folders {
        match role {
            SpecialUse::Sent => return Some(name.to_string()),
            SpecialUse::Archive | SpecialUse::All => {}
            SpecialUse::Other => {
                // The FULL name, and decoded: a French server announces
                // "Envoy&AOk-s". What is memorized remains the wire name, it
                // is what we will send back to the server.
                if named.is_none()
                    && SENT_FALLBACK_NAMES
                        .contains(&crate::mutf7::decode(name).to_lowercase().as_str())
                {
                    named = Some(name.to_string());
                }
            }
        }
    }
    named
}

/// Compacts a list of UIDs into an IMAP set: `[1,2,3,5]` → `"1:3,5"`.
pub(crate) fn uid_set(uids: &[Uid]) -> String {
    let mut sorted = uids.to_vec();
    sorted.sort_unstable();
    sorted.dedup();

    let mut parts: Vec<String> = Vec::new();
    let mut run: Option<(Uid, Uid)> = None;
    for uid in sorted {
        run = match run {
            Some((start, end)) if uid == end + 1 => Some((start, uid)),
            Some((start, end)) => {
                parts.push(format_run(start, end));
                Some((uid, uid))
            }
            None => Some((uid, uid)),
        };
    }
    if let Some((start, end)) = run {
        parts.push(format_run(start, end));
    }
    parts.join(",")
}

fn format_run(start: Uid, end: Uid) -> String {
    if start == end {
        start.to_string()
    } else {
        format!("{start}:{end}")
    }
}

pub(crate) fn fetch_to_envelope(fetch: &imap::types::Fetch) -> Option<Envelope> {
    let uid = fetch.uid?;
    let seen = fetch
        .flags()
        .iter()
        .any(|flag| matches!(flag, imap::types::Flag::Seen));
    let flagged = fetch
        .flags()
        .iter()
        .any(|flag| matches!(flag, imap::types::Flag::Flagged));
    let date = fetch.internal_date().map(|d| d.with_timezone(&Utc));
    Some(envelope_from_parts(
        uid,
        fetch.envelope(),
        date,
        seen,
        flagged,
    ))
}

/// Heart of the mapping, separated from `Fetch` (not constructible) to be
/// testable.
pub(crate) fn envelope_from_parts(
    uid: Uid,
    proto: Option<&ProtoEnvelope<'_>>,
    date: Option<chrono::DateTime<Utc>>,
    seen: bool,
    flagged: bool,
) -> Envelope {
    let subject = proto
        .and_then(|envelope| envelope.subject.as_deref())
        .and_then(decode_header);
    let from = proto
        .and_then(|envelope| envelope.from.as_ref())
        .and_then(|from| from.first());
    let message_id = proto
        .and_then(|envelope| envelope.message_id.as_deref())
        .and_then(text_header);
    // The ENVELOPE carries `In-Reply-To` (RFC 3501 §7.4.2). Threading
    // therefore starts WITHOUT one more byte on the network: that is what
    // allowed not to weigh down the "envelopes first" sync. `References`,
    // absent from the ENVELOPE, requires a separate pass.
    let in_reply_to = proto
        .and_then(|envelope| envelope.in_reply_to.as_deref())
        .and_then(text_header);
    Envelope {
        // `Reply-To`: the first address, where the sender wants the reply
        // (PLAN-AUDIT-V2 E5 — thrown away before).
        reply_to: proto
            .and_then(|envelope| envelope.reply_to.as_ref())
            .and_then(|list| list.first())
            .and_then(address_literal),
        uid,
        subject,
        sender: from.and_then(sender_display),
        sender_address: from.and_then(address_literal),
        // To / Cc come from the SAME ENVELOPE (R4): stored at sync, they
        // avoid the server round trip of "Reply all" and give the sent
        // folder its real recipient.
        to_addrs: proto
            .map(|envelope| address_list(envelope.to.as_deref()))
            .unwrap_or_default(),
        cc_addrs: proto
            .map(|envelope| address_list(envelope.cc.as_deref()))
            .unwrap_or_default(),
        message_id,
        in_reply_to,
        date,
        seen,
        flagged,
    }
}

/// The recipients (To / Cc) of an ENVELOPE, raw addresses — what "Reply
/// all" re-reads at click time, the stored envelope only carrying the
/// sender.
pub(crate) fn envelope_recipients(proto: &ProtoEnvelope<'_>) -> mail_core::MessageRecipients {
    mail_core::MessageRecipients {
        to: address_list(proto.to.as_deref()),
        cc: address_list(proto.cc.as_deref()),
    }
}

/// The raw addresses of an ENVELOPE list; those without a complete
/// `mailbox@host` (RFC 5322 groups, empty entries) are silenced.
fn address_list(addresses: Option<&[Address<'_>]>) -> Vec<String> {
    addresses
        .into_iter()
        .flatten()
        .filter_map(address_literal)
        .collect()
}

/// Reads a raw draft: recipients, subject, and the two body forms MIME can
/// carry.
///
/// Nothing is validated — a draft is allowed to have neither recipient, nor
/// subject, nor body. That is exactly what distinguishes it from a message:
/// it is being written.
pub(crate) fn draft_from_raw(raw: &[u8]) -> Option<mail_core::RemoteDraft> {
    let message = mail_parser::MessageParser::new().parse(raw)?;
    Some(mail_core::RemoteDraft {
        to_raw: recipients(&message),
        subject: message.subject().unwrap_or_default().to_string(),
        text: message.body_text(0).map(|body| body.into_owned()),
        html: message.body_html(0).map(|body| body.into_owned()),
    })
}

/// The recipients in the form the composer expects: raw addresses
/// separated by commas.
///
/// We keep the ADDRESS and not the display name: it is what must survive
/// the round trip, and what the send validation will examine.
fn recipients(message: &mail_parser::Message<'_>) -> String {
    let Some(to) = message.to() else {
        return String::new();
    };
    to.iter()
        .filter_map(|addr| addr.address())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Reads the two threading headers in a raw header block.
///
/// Parsed by hand rather than by `mail-parser`: here we only want strings
/// of identifiers copied as is, without normalization. A complete MIME
/// parser would decide in our place what a valid identifier is; that
/// decision belongs to the core, which knows how to handle the non-standard
/// forms real life produces.
pub(crate) fn thread_headers(raw: &[u8]) -> mail_core::ThreadHeaders {
    let text = String::from_utf8_lossy(raw);
    mail_core::ThreadHeaders {
        in_reply_to: header_value(&text, "in-reply-to"),
        // Always `Some`: an empty string says "read, and there is none",
        // which is not the same as "not read yet".
        references: Some(header_value(&text, "references").unwrap_or_default()),
    }
}

/// The value of a header, folds included (RFC 5322 §2.2.3: a line starting
/// with a space or a tab continues the previous one).
fn header_value(text: &str, name: &str) -> Option<String> {
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        // Empty line = end of the headers; what follows is the body, and a
        // body may very well contain "References:" in plain text.
        if line.is_empty() {
            return None;
        }
        let Some((field, value)) = line.split_once(':') else {
            continue;
        };
        if !field.trim().eq_ignore_ascii_case(name) {
            continue;
        }
        let mut value = value.trim().to_string();
        for folded in lines.by_ref() {
            if !folded.starts_with([' ', '\t']) {
                break;
            }
            value.push(' ');
            value.push_str(folded.trim());
        }
        return Some(value);
    }
    None
}

/// Display name if it exists (decoded), otherwise `mailbox@host`.
fn sender_display(address: &Address<'_>) -> Option<String> {
    if let Some(name) = address.name.as_deref().and_then(decode_header) {
        return Some(name);
    }
    address_literal(address)
}

/// Raw `mailbox@host` address — the target of a reply (Phase 2).
fn address_literal(address: &Address<'_>) -> Option<String> {
    let mailbox = address.mailbox.as_deref()?;
    let host = address.host.as_deref()?;
    Some(format!(
        "{}@{}",
        String::from_utf8_lossy(&unescape_imap_quoted(mailbox)),
        String::from_utf8_lossy(&unescape_imap_quoted(host))
    ))
}

/// Raw textual header (Message-ID, In-Reply-To): ASCII in practice, never
/// RFC 2047-encoded — no decoding, just a cleanup. The IMAP `quoted-string`
/// escapes are still removed (like `decode_header`, R2): a server that
/// transmits a Message-ID as an escaped string would otherwise keep it with
/// its backslashes, and the same id received elsewhere as an atom would no
/// longer attach to it (broken thread). Extremely rare, but consistency
/// with the subject decoding costs nothing.
fn text_header(raw: &[u8]) -> Option<String> {
    let raw = unescape_imap_quoted(raw);
    let value = String::from_utf8_lossy(&raw);
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Extracts the HTML body of a raw message. `mail-parser` itself converts
/// text-only messages into safe HTML (Phase 0 lesson) — `None` only if the
/// message is unparseable. Embedded images (`cid:`) are inlined as `data:`
/// URIs: they are part of the message, displaying them triggers no network
/// load.
#[cfg(test)]
pub(crate) fn extract_html(raw: &[u8]) -> Option<String> {
    let message = mail_parser::MessageParser::new().parse(raw)?;
    html_of(&message, raw)
}

fn html_of(message: &mail_parser::Message<'_>, raw: &[u8]) -> Option<String> {
    let html = message.body_html(0)?.into_owned();
    let html = redecode_without_charset(html, message, raw);
    Some(inline_cid_images(html, message))
}

/// Everything a raw message gives the application — displayable body,
/// attachments, invitation — in ONE MIME parse (PLAN-AUDIT-V2 E3:
/// `extract_html`, `extract_attachments` and `extract_ics` each parsed the
/// same bytes; on a backfill of 200 k messages, every extra parse cost
/// ~60 s of CPU).
pub(crate) fn parse(raw: &[u8]) -> Option<mail_core::FetchedBody> {
    let message = mail_parser::MessageParser::new().parse(raw)?;
    let ics = contains_calendar_marker(raw)
        .then(|| ics_of(&message))
        .flatten();
    // A message whose ROOT is text/calendar has no HTML body: it stays
    // displayable — invitation card over an empty body. Before
    // PLAN-INVITATIONS it fell into "message not found" and stayed a
    // backfill candidate forever.
    let html = match html_of(&message, raw) {
        Some(html) => html,
        None if ics.is_some() => String::new(),
        None => return None,
    };
    Some(mail_core::FetchedBody {
        html,
        attachments: attachments_of(&message),
        ics,
    })
}

/// Repairs the body when `mail-parser` replaced bytes by U+FFFD.
///
/// Without a declared charset (or with a charset no decoder knows),
/// `mail-parser` reads the bytes as UTF-8 with replacement — and the Latin-1
/// accents of real mail become "�". The de facto default of the field is
/// windows-1252 (a superset of ISO-8859-1): if the bytes of the part are not
/// valid UTF-8, we re-decode them that way.
///
/// If the declared charset is known, or if the bytes are valid UTF-8 (the
/// U+FFFD then comes from the sender), the body is left as is.
fn redecode_without_charset(
    html: String,
    message: &mail_parser::Message<'_>,
    raw: &[u8],
) -> String {
    use mail_parser::MimeHeaders;

    if !html.contains('\u{FFFD}') {
        return html;
    }
    // The part `body_html(0)` took the body from: the HTML part if there is
    // one, otherwise the text part converted to HTML.
    let (part, was_text) = match message.html_part(0) {
        Some(part) => (part, false),
        None => match message.text_part(0) {
            Some(part) => (part, true),
            None => return html,
        },
    };
    let declared = part
        .content_type()
        .and_then(|content_type| content_type.attribute("charset"));
    if let Some(charset) = declared
        && mail_parser::decoders::charsets::map::charset_decoder(charset.as_bytes()).is_some()
    {
        return html;
    }
    let Some(bytes) = raw.get(part.offset_body as usize..part.offset_end as usize) else {
        return html;
    };
    let bytes = match part.encoding {
        mail_parser::Encoding::None => std::borrow::Cow::Borrowed(bytes),
        mail_parser::Encoding::QuotedPrintable => {
            match mail_parser::decoders::quoted_printable::quoted_printable_decode(bytes) {
                Some(decoded) => std::borrow::Cow::Owned(decoded),
                None => return html,
            }
        }
        mail_parser::Encoding::Base64 => {
            match mail_parser::decoders::base64::base64_decode(bytes) {
                Some(decoded) => std::borrow::Cow::Owned(decoded),
                None => return html,
            }
        }
    };
    if std::str::from_utf8(&bytes).is_ok() {
        return html;
    }
    let Some(decoder) = mail_parser::decoders::charsets::map::charset_decoder(b"windows-1252")
    else {
        return html;
    };
    let text = decoder(&bytes);
    if was_text {
        mail_parser::decoders::html::text_to_html(&text)
    } else {
        text
    }
}

/// MIME type of a part, `application/octet-stream` by default.
fn part_mime(part: &mail_parser::MessagePart<'_>) -> String {
    use mail_parser::MimeHeaders;

    match part.content_type() {
        Some(content_type) => format!(
            "{}/{}",
            content_type.ctype(),
            content_type.subtype().unwrap_or("octet-stream")
        ),
        None => "application/octet-stream".to_string(),
    }
}

/// Is this part an image embedded in the HTML by [`inline_cid_images`]?
///
/// **Shared predicate, and that is its whole point**: what is embedded in
/// the body must not be listed as an attachment, and conversely. Two rules
/// written separately would end up diverging — either the newsletter logo
/// would appear as an attachment, or a file would vanish on both sides.
fn is_inlined_image(part: &mail_parser::MessagePart<'_>) -> bool {
    use mail_parser::MimeHeaders;

    part.content_id().is_some() && part_mime(part).starts_with("image/")
}

/// A CALENDAR part — THE single predicate of what the invitation card
/// consumes ([`extract_ics`]): defining it twice had left a hole at the
/// review (an unnamed `application/ics` part was consumed AND listed as a
/// ghost chip).
fn is_calendar_part(part: &mail_parser::MessagePart<'_>) -> bool {
    use mail_parser::MimeHeaders;

    let mime = part_mime(part);
    mime.eq_ignore_ascii_case("text/calendar")
        || mime.eq_ignore_ascii_case("application/ics")
        || part
            .attachment_name()
            .is_some_and(|name| name.to_ascii_lowercase().ends_with(".ics"))
}

/// The INLINE calendar part of an invitation (D3, PLAN-INVITATIONS): a
/// calendar part WITHOUT a file name is not a file — it is the invitation
/// itself, rendered as a card. Listing it as an attachment showed a ghost
/// "attachment.calendar" chip (the field finding). A real named and
/// attached `.ics`, for its part, remains a savable attachment. Same
/// shared-predicate rule as [`is_inlined_image`]: what the card consumes is
/// not listed, and conversely.
fn is_inline_calendar(part: &mail_parser::MessagePart<'_>) -> bool {
    use mail_parser::MimeHeaders;

    part.attachment_name().is_none() && is_calendar_part(part)
}

fn inline_cid_images(html: String, message: &mail_parser::Message<'_>) -> String {
    use base64::Engine;
    use mail_parser::MimeHeaders;

    let mut result = html;
    for part in message.attachments().filter(|part| is_inlined_image(part)) {
        let Some(content_id) = part.content_id() else {
            continue;
        };
        let data_uri = format!(
            "data:{};base64,{}",
            part_mime(part),
            base64::engine::general_purpose::STANDARD.encode(part.contents())
        );
        let reference = format!("cid:{}", content_id.trim_matches(['<', '>']));
        result = result.replace(&reference, &data_uri);
    }
    result
}

/// The REAL attachments of a message: the files the user would recognize
/// as such.
///
/// Images already embedded in the body are excluded ([`is_inlined_image`]).
/// The index returned follows the KEPT attachments: it is what will serve
/// to find the bytes later, by replaying this same extraction.
#[cfg(test)]
pub(crate) fn extract_attachments(raw: &[u8]) -> Vec<mail_core::Attachment> {
    let Some(message) = mail_parser::MessageParser::new().parse(raw) else {
        return Vec::new();
    };
    attachments_of(&message)
}

fn attachments_of(message: &mail_parser::Message<'_>) -> Vec<mail_core::Attachment> {
    attachment_parts(message)
        .into_iter()
        .enumerate()
        .map(|(index, (name, mime, size))| mail_core::Attachment {
            index,
            name,
            mime,
            size,
        })
        .collect()
}

/// The bytes of ONE attachment, designated by its index.
///
/// Replays the extraction on the raw message: the index is therefore stable
/// by construction, without ever handling an IMAP part number.
pub(crate) fn attachment_bytes(raw: &[u8], index: usize) -> Option<Vec<u8>> {
    let message = mail_parser::MessageParser::new().parse(raw)?;
    message
        .attachments()
        .filter(|part| !is_inlined_image(part) && !is_inline_calendar(part))
        .nth(index)
        .map(|part| part.contents().to_vec())
}

/// Name, type and decoded size of each kept attachment.
fn attachment_parts(message: &mail_parser::Message<'_>) -> Vec<(String, String, u64)> {
    use mail_parser::MimeHeaders;

    message
        .attachments()
        .filter(|part| !is_inlined_image(part) && !is_inline_calendar(part))
        .map(|part| {
            let mime = part_mime(part);
            // `attachment_name` already decodes RFC 2047. Without a name, we
            // make one up: an anonymous file remains savable.
            let name = part
                .attachment_name()
                .map(str::to_string)
                .unwrap_or_else(|| fallback_name(&mime));
            (name, mime, part.contents().len() as u64)
        })
        .collect()
}

/// The `text/calendar` part of a message — the raw iTIP invitation
/// (PLAN-INVITATIONS). Looked for in ALL the parts: a Gmail/Outlook
/// invitation lives in the `multipart/alternative` (where mail-parser files
/// it as an attachment), a forwarded invitation arrives as an attached
/// `.ics` file, and some producers make it the root of the message.
#[cfg(test)]
pub(crate) fn extract_ics(raw: &[u8]) -> Option<String> {
    // BYTE guard before any parse: 99.9 % of the messages have no calendar
    // — making them pay one more complete MIME parse (the third of the
    // path) cost ~60 s of CPU on a backfill of 200 k messages (review). A
    // false positive ("text/calendar" written in a body) only costs one
    // parse for nothing.
    if !contains_calendar_marker(raw) {
        return None;
    }
    let message = mail_parser::MessageParser::new().parse(raw)?;
    ics_of(&message)
}

fn ics_of(message: &mail_parser::Message<'_>) -> Option<String> {
    for part in &message.parts {
        if !is_calendar_part(part) {
            continue;
        }
        // A text part arrives decoded by mail-parser; a binary attachment
        // (`application/ics`) is read as UTF-8 — the de facto charset of the
        // format (RFC 5545 §3.1.4).
        let text = match part.text_contents() {
            Some(text) => text.to_string(),
            None => String::from_utf8_lossy(part.contents()).into_owned(),
        };
        if !text.trim().is_empty() {
            return Some(text);
        }
    }
    None
}

fn contains_calendar_marker(raw: &[u8]) -> bool {
    ["text/calendar", "application/ics", ".ics"]
        .iter()
        .any(|pattern| {
            raw.windows(pattern.len())
                .any(|window| window.eq_ignore_ascii_case(pattern.as_bytes()))
        })
}

/// Fallback name for an attachment without `filename` — derived from the
/// subtype.
fn fallback_name(mime: &str) -> String {
    let extension = mime.rsplit('/').next().unwrap_or("bin");
    format!("attachment.{extension}")
}

/// Decodes an RFC 2047 header by presenting it to `mail-parser` as a
/// synthetic message. Returns `None` for an empty header. The escapes of the
/// IMAP `quoted-string` layer are removed BEFORE the RFC 2047 pass (a
/// subject may mix `\"` and encoded-words).
fn decode_header(raw: &[u8]) -> Option<String> {
    let raw = unescape_imap_quoted(raw);
    let raw = raw.as_ref();
    let synthetic = [b"Subject: ".as_slice(), raw, b"\r\n\r\n".as_slice()].concat();
    let decoded = mail_parser::MessageParser::new()
        .parse(&synthetic)
        .and_then(|message| message.subject().map(str::to_string))
        .unwrap_or_else(|| String::from_utf8_lossy(raw).into_owned());
    let trimmed = decoded.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use chrono::TimeZone;

    use super::*;

    fn address<'a>(
        name: Option<&'a [u8]>,
        mailbox: Option<&'a [u8]>,
        host: Option<&'a [u8]>,
    ) -> Address<'a> {
        Address {
            name: name.map(Cow::Borrowed),
            adl: None,
            mailbox: mailbox.map(Cow::Borrowed),
            host: host.map(Cow::Borrowed),
        }
    }

    fn proto_envelope<'a>(subject: &'a [u8], from: Address<'a>) -> ProtoEnvelope<'a> {
        ProtoEnvelope {
            date: None,
            subject: Some(Cow::Borrowed(subject)),
            from: Some(vec![from]),
            sender: None,
            reply_to: None,
            to: None,
            cc: None,
            bcc: None,
            in_reply_to: None,
            message_id: None,
        }
    }

    /// PLAN-AUDIT-V2 E5: the ENVELOPE carries `Reply-To` for free; it was
    /// never read.
    #[test]
    fn reply_to_is_read_from_the_envelope() {
        let mut proto = proto_envelope(
            b"Subject",
            address(Some(b"List"), Some(b"list"), Some(b"x.fr")),
        );
        proto.reply_to = Some(vec![address(None, Some(b"bob"), Some(b"y.fr"))]);
        let envelope = envelope_from_parts(1, Some(&proto), None, false, false);
        assert_eq!(envelope.reply_to.as_deref(), Some("bob@y.fr"));
        let without = envelope_from_parts(
            2,
            Some(&proto_envelope(
                b"S",
                address(None, Some(b"a"), Some(b"b.fr")),
            )),
            None,
            false,
            false,
        );
        assert_eq!(without.reply_to, None);
    }

    #[test]
    fn uid_set_compacts_consecutive_runs() {
        assert_eq!(uid_set(&[1, 2, 3, 5, 7, 8]), "1:3,5,7:8");
    }

    #[test]
    fn uid_set_handles_single_and_unordered_duplicates() {
        assert_eq!(uid_set(&[4]), "4");
        assert_eq!(uid_set(&[9, 7, 8, 8, 1]), "1,7:9");
    }

    #[test]
    fn envelope_recipients_reads_raw_to_and_cc_addresses() {
        let mut proto = proto_envelope(b"subject", address(None, Some(b"alice"), Some(b"a.fr")));
        proto.to = Some(vec![
            address(Some(b"Bob"), Some(b"bob"), Some(b"b.fr")),
            // RFC 5322 group entry (no mailbox@host): silenced.
            address(Some(b"the group"), None, None),
        ]);
        proto.cc = Some(vec![address(None, Some(b"carole"), Some(b"c.fr"))]);
        let recipients = envelope_recipients(&proto);
        assert_eq!(recipients.to, vec!["bob@b.fr"]);
        assert_eq!(recipients.cc, vec!["carole@c.fr"]);
    }

    const ICS_MINIMAL: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nMETHOD:REQUEST\r\n\
        BEGIN:VEVENT\r\nUID:r1@exemple.fr\r\nSUMMARY:Project sync\r\n\
        DTSTART:20260903T123000Z\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

    /// The massive case of the field: the Gmail/Outlook invitation, third
    /// part of the multipart/alternative (mail-parser files it as an
    /// attachment).
    #[test]
    fn the_ics_of_a_multipart_alternative_is_extracted() {
        let raw = format!(
            "From: claire@exemple.fr\r\nTo: nous@wind.example\r\n\
             Subject: Invitation\r\nMIME-Version: 1.0\r\n\
             Content-Type: multipart/alternative; boundary=\"XX\"\r\n\r\n\
             --XX\r\nContent-Type: text/plain; charset=utf-8\r\n\r\ntext body\r\n\
             --XX\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<p>body</p>\r\n\
             --XX\r\nContent-Type: text/calendar; method=REQUEST; charset=utf-8\r\n\r\n\
             {ICS_MINIMAL}\
             --XX--\r\n"
        );
        let ics = extract_ics(raw.as_bytes()).expect("calendar part");
        assert!(ics.contains("METHOD:REQUEST"));
        assert!(ics.contains("UID:r1@exemple.fr"));
        // The HTML body, for its part, remains the body: nothing changes for it.
        assert_eq!(extract_html(raw.as_bytes()).as_deref(), Some("<p>body</p>"));
        // D3: the INLINE calendar part (without file name) is not a file —
        // it appears neither as a chip nor in the count. Without this
        // filter, every invitation showed a ghost "attachment.calendar"
        // (the field finding).
        assert!(
            extract_attachments(raw.as_bytes()).is_empty(),
            "the inline calendar part must not be listed as an attachment"
        );
    }

    /// The forwarded invitation: an `.ics` file attached as
    /// `application/ics`, disposition attachment.
    #[test]
    fn the_ics_of_an_attachment_is_extracted() {
        let raw = format!(
            "From: claire@exemple.fr\r\nTo: nous@wind.example\r\n\
             Subject: Fwd: Invitation\r\nMIME-Version: 1.0\r\n\
             Content-Type: multipart/mixed; boundary=\"YY\"\r\n\r\n\
             --YY\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<p>see attached</p>\r\n\
             --YY\r\nContent-Type: application/ics; name=\"invite.ics\"\r\n\
             Content-Disposition: attachment; filename=\"invite.ics\"\r\n\r\n\
             {ICS_MINIMAL}\
             --YY--\r\n"
        );
        let ics = extract_ics(raw.as_bytes()).expect("calendar attachment");
        assert!(ics.contains("UID:r1@exemple.fr"));
        // D3: a REAL named and attached `.ics` file, for its part, REMAINS a
        // savable attachment.
        let attachments = extract_attachments(raw.as_bytes());
        assert_eq!(attachments.len(), 1);
        assert_eq!(attachments[0].name, "invite.ics");
    }

    #[test]
    fn an_ordinary_message_has_no_ics() {
        let raw = "From: a@b.fr\r\nTo: c@d.fr\r\nSubject: hello\r\n\
                   Content-Type: text/html; charset=utf-8\r\n\r\n<p>nothing</p>\r\n";
        assert_eq!(extract_ics(raw.as_bytes()), None);
    }

    #[test]
    fn envelope_recipients_tolerates_missing_lists() {
        let proto = proto_envelope(b"subject", address(None, Some(b"alice"), Some(b"a.fr")));
        let recipients = envelope_recipients(&proto);
        assert!(recipients.to.is_empty());
        assert!(recipients.cc.is_empty());
    }

    #[test]
    fn decodes_rfc2047_subject() {
        let proto = proto_envelope(
            b"=?UTF-8?Q?R=C3=A9union_de_demain?=",
            address(None, Some(b"seb"), Some(b"example.com")),
        );
        let envelope = envelope_from_parts(1, Some(&proto), None, false, false);
        assert_eq!(envelope.subject.as_deref(), Some("R\u{e9}union de demain"));
    }

    #[test]
    fn sender_prefers_decoded_display_name() {
        let proto = proto_envelope(
            b"subject",
            address(
                Some(b"=?UTF-8?Q?S=C3=A9bastien?="),
                Some(b"seb"),
                Some(b"example.com"),
            ),
        );
        let envelope = envelope_from_parts(1, Some(&proto), None, false, false);
        assert_eq!(envelope.sender.as_deref(), Some("S\u{e9}bastien"));
    }

    #[test]
    fn sender_falls_back_to_mailbox_at_host() {
        let proto = proto_envelope(
            b"subject",
            address(None, Some(b"seb"), Some(b"example.com")),
        );
        let envelope = envelope_from_parts(1, Some(&proto), None, false, false);
        assert_eq!(envelope.sender.as_deref(), Some("seb@example.com"));
    }

    /// R2 (PLAN-RETOURS-MAIL): `imap-proto` returns the content of an IMAP
    /// `quoted-string` keeping the backslash escapes (outer quotes removed,
    /// raw content — proven by its `core.rs` tests). A real subject
    /// `Test "Envoyés"` therefore arrives `Test \"Envoyés\"`: it must be
    /// unescaped before anything.
    #[test]
    fn unescapes_imap_quoted_quotes_in_subject() {
        let proto = proto_envelope(
            br#"Test \"Envoyes\""#,
            address(None, Some(b"seb"), Some(b"example.com")),
        );
        let envelope = envelope_from_parts(1, Some(&proto), None, false, false);
        assert_eq!(envelope.subject.as_deref(), Some(r#"Test "Envoyes""#));
    }

    /// `\\` is the second (and last) valid RFC 3501 sequence: a literal
    /// backslash of a subject arrives doubled.
    #[test]
    fn unescapes_imap_quoted_backslash_in_subject() {
        let proto = proto_envelope(
            br"path C:\\temp",
            address(None, Some(b"seb"), Some(b"example.com")),
        );
        let envelope = envelope_from_parts(1, Some(&proto), None, false, false);
        assert_eq!(envelope.subject.as_deref(), Some(r"path C:\temp"));
    }

    /// Unescaping precedes the RFC 2047 pass: the escaped `"` of the IMAP
    /// layer and the encoded-word coexist in the same subject.
    #[test]
    fn unescape_precedes_rfc2047_decoding() {
        let proto = proto_envelope(
            br#"\"quote\" =?UTF-8?Q?r=C3=A9pond?="#,
            address(None, Some(b"seb"), Some(b"example.com")),
        );
        let envelope = envelope_from_parts(1, Some(&proto), None, false, false);
        assert_eq!(envelope.subject.as_deref(), Some("\"quote\" r\u{e9}pond"));
    }

    /// Same defect on the sender's display name.
    #[test]
    fn unescapes_imap_quoted_quotes_in_sender_name() {
        let proto = proto_envelope(
            b"subject",
            address(
                Some(br#"Societe \"ACME\""#),
                Some(b"info"),
                Some(b"acme.fr"),
            ),
        );
        let envelope = envelope_from_parts(1, Some(&proto), None, false, false);
        assert_eq!(envelope.sender.as_deref(), Some(r#"Societe "ACME""#));
    }

    /* An ordinary subject, without escape, goes through intact — no regression. */
    #[test]
    fn plain_subject_without_escapes_is_unchanged() {
        let proto = proto_envelope(
            b"Reunion de demain",
            address(None, Some(b"seb"), Some(b"example.com")),
        );
        let envelope = envelope_from_parts(1, Some(&proto), None, false, false);
        assert_eq!(envelope.subject.as_deref(), Some("Reunion de demain"));
    }

    #[test]
    fn missing_envelope_yields_bare_fields() {
        let date = Utc.timestamp_opt(1_700_000_000, 0).unwrap();
        let envelope = envelope_from_parts(42, None, Some(date), true, true);
        assert_eq!(envelope.uid, 42);
        assert_eq!(envelope.subject, None);
        assert_eq!(envelope.sender, None);
        assert_eq!(envelope.sender_address, None);
        assert_eq!(envelope.message_id, None);
        assert_eq!(envelope.date, Some(date));
        assert!(envelope.seen);
        assert!(envelope.flagged, "the star follows the FETCH flags");
    }

    /// The raw address must stay available even when a display name
    /// exists: it is what goes into the "To" of a reply.
    #[test]
    fn keeps_raw_sender_address_alongside_display_name() {
        let proto = proto_envelope(
            b"subject",
            address(
                Some(b"=?UTF-8?Q?S=C3=A9bastien?="),
                Some(b"seb"),
                Some(b"example.com"),
            ),
        );
        let envelope = envelope_from_parts(1, Some(&proto), None, false, false);
        assert_eq!(envelope.sender.as_deref(), Some("S\u{e9}bastien"));
        assert_eq!(envelope.sender_address.as_deref(), Some("seb@example.com"));
    }

    #[test]
    fn extracts_message_id_for_threading() {
        let mut proto = proto_envelope(b"subject", address(None, Some(b"a"), Some(b"b.c")));
        proto.message_id = Some(Cow::Borrowed(b" <abc.123@mail.example.com> ".as_slice()));
        let envelope = envelope_from_parts(1, Some(&proto), None, false, false);
        assert_eq!(
            envelope.message_id.as_deref(),
            Some("<abc.123@mail.example.com>")
        );
    }

    #[test]
    fn blank_subject_becomes_none() {
        let proto = proto_envelope(b"   ", address(None, Some(b"a"), Some(b"b.c")));
        let envelope = envelope_from_parts(1, Some(&proto), None, false, false);
        assert_eq!(envelope.subject, None);
    }

    #[test]
    fn extracts_html_body_from_raw_message() {
        let raw = b"From: a@b.c\r\nSubject: t\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<p>Hello <b>world</b></p>";
        let html = extract_html(raw).expect("html body expected");
        assert!(html.contains("<b>world</b>"));
    }

    // --- Charsets: defects seen in the field -----------------------

    /// Absent charset + Latin-1 bytes: real mail is full of it. Without a
    /// fallback, every accent becomes U+FFFD from storage on — defect seen
    /// on 25 bodies of the measurement database.
    #[test]
    fn html_without_charset_in_latin1_is_redecoded_as_windows_1252() {
        let raw = b"From: a@b.c\r\nSubject: t\r\nContent-Type: text/html\r\n\r\n<p>journ\xe9es d'acc\xe8s r\xe9compens\xe9es</p>";
        let html = extract_html(raw).expect("html body expected");
        assert!(
            html.contains("journées d'accès récompensées"),
            "accents expected, got: {html}"
        );
        assert!(!html.contains('\u{FFFD}'));
    }

    /// Same fallback for a text-only message: the conversion to HTML must
    /// start again from the re-decoded text, not the mutilated one.
    #[test]
    fn text_only_without_charset_in_latin1_is_redecoded() {
        let raw = b"From: a@b.c\r\nSubject: t\r\nContent-Type: text/plain\r\n\r\nune journ\xe9e enti\xe8re";
        let html = extract_html(raw).expect("html body expected");
        assert!(
            html.contains("une journée entière"),
            "accents expected, got: {html}"
        );
    }

    /// Quoted-printable without charset: the fallback must re-decode the
    /// bytes AFTER lifting the transfer encoding, not the literal `=E9`.
    #[test]
    fn quoted_printable_without_charset_is_redecoded() {
        let raw = b"From: a@b.c\r\nSubject: t\r\nContent-Type: text/html\r\nContent-Transfer-Encoding: quoted-printable\r\n\r\n<p>journ=E9es</p>";
        let html = extract_html(raw).expect("html body expected");
        assert!(html.contains("journées"), "got: {html}");
    }

    /// A U+FFFD sent AS IS by the sender (valid UTF-8) is not a decoding
    /// error: the body stays intact, no fallback.
    #[test]
    fn a_genuine_fffd_in_valid_utf8_is_kept() {
        let raw = "From: a@b.c\r\nSubject: t\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<p>broken at the sender's: \u{FFFD}</p>".as_bytes();
        let html = extract_html(raw).expect("html body expected");
        assert!(html.contains("broken at the sender's: \u{FFFD}"));
    }

    /// gb2312 requires mail-parser's `full_encoding` feature: without it,
    /// the decoder falls back on UTF-8 with replacement (14 of the 25
    /// mutilated bodies of the measurement database). This test locks the
    /// feature.
    #[test]
    fn gb2312_is_decoded_thanks_to_full_encoding() {
        let raw = b"From: a@b.c\r\nSubject: t\r\nContent-Type: text/html; charset=gb2312\r\n\r\n<p>\xc4\xe3\xba\xc3</p>";
        let html = extract_html(raw).expect("html body expected");
        assert!(html.contains("你好"), "got: {html}");
        assert!(!html.contains('\u{FFFD}'));
    }

    // --- Attachments ----------------------------------------------

    /// A message carrying a real file: name, type and DECODED size.
    #[test]
    fn lists_a_real_attachment_with_its_name_type_and_decoded_size() {
        let raw = b"From: a@b.c
Subject: t
MIME-Version: 1.0
Content-Type: multipart/mixed; boundary=\"B\"

--B
Content-Type: text/html

<p>here</p>
--B
Content-Type: application/pdf; name=\"facture.pdf\"
Content-Disposition: attachment; filename=\"facture.pdf\"
Content-Transfer-Encoding: base64

SGVsbG8sIHdvcmxkIQ==
--B--
";
        let found = extract_attachments(raw);
        assert_eq!(found.len(), 1, "a single attachment expected: {found:?}");
        assert_eq!(found[0].name, "facture.pdf");
        assert_eq!(found[0].mime, "application/pdf");
        // "Hello, world!" = 13 bytes once the base64 is decoded.
        assert_eq!(
            found[0].size, 13,
            "the size must be that of the decoded bytes"
        );
        assert_eq!(found[0].index, 0);
    }

    /// THE trap of this feature. `mail_parser` files the images referenced
    /// by Content-ID among the `attachments()` — yet they are ALREADY
    /// embedded in the HTML by `inline_cid_images`. Without this filter, the
    /// logo of every newsletter would appear as an attachment: the paperclip
    /// would become permanent noise.
    #[test]
    fn an_inlined_cid_image_is_not_an_attachment() {
        let raw = b"From: a@b.c
Subject: t
MIME-Version: 1.0
Content-Type: multipart/related; boundary=\"B\"

--B
Content-Type: text/html; charset=utf-8

<p>logo: <img src=\"cid:logo123\"></p>
--B
Content-Type: image/png
Content-ID: <logo123>
Content-Transfer-Encoding: base64

iVBORw0KGgo=
--B--
";
        assert!(
            extract_attachments(raw).is_empty(),
            "an image already embedded in the HTML must not be listed"
        );
    }

    /// The real case: a newsletter with its logo AND a real attachment.
    /// Exactly one must come out.
    #[test]
    fn keeps_the_real_file_and_drops_the_logo() {
        let raw = b"From: a@b.c
Subject: t
MIME-Version: 1.0
Content-Type: multipart/mixed; boundary=\"B\"

--B
Content-Type: text/html

<img src=\"cid:logo\">
--B
Content-Type: image/png
Content-ID: <logo>
Content-Transfer-Encoding: base64

iVBORw0KGgo=
--B
Content-Type: application/pdf
Content-Disposition: attachment; filename=\"contrat.pdf\"

PDF
--B--
";
        let found = extract_attachments(raw);
        assert_eq!(found.len(), 1, "the logo must disappear: {found:?}");
        assert_eq!(found[0].name, "contrat.pdf");
    }

    /// Symmetric of the previous one, the other way round: a NON-image file
    /// carrying a Content-ID is not embedded in the HTML, so it stays an
    /// attachment. The filter must be exactly that of the embedding —
    /// neither wider nor narrower.
    #[test]
    fn a_non_image_with_a_content_id_stays_an_attachment() {
        let raw = b"From: a@b.c
Subject: t
MIME-Version: 1.0
Content-Type: multipart/related; boundary=\"B\"

--B
Content-Type: text/html

<p>x</p>
--B
Content-Type: application/pdf
Content-ID: <doc1>
Content-Disposition: attachment; filename=\"annexe.pdf\"

PDF
--B--
";
        let found = extract_attachments(raw);
        assert_eq!(found.len(), 1, "a PDF is never embedded in the HTML");
        assert_eq!(found[0].name, "annexe.pdf");
    }

    /// Non-ASCII names travel encoded (RFC 2047). Showing
    /// `=?UTF-8?B?...?=` to the user would be a visible regression — the
    /// same defect as undecoded UTF-7 folders.
    #[test]
    fn decodes_an_encoded_filename() {
        let raw = "From: a@b.c
Subject: t
MIME-Version: 1.0
Content-Type: multipart/mixed; boundary=\"B\"

--B
Content-Type: text/plain

body
--B
Content-Type: application/pdf
Content-Disposition: attachment; filename=\"=?UTF-8?B?csOpc3Vtw6kucGRm?=\"

PDF
--B--
"
        .as_bytes();
        let found = extract_attachments(raw);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "résumé.pdf", "RFC 2047 name to decode");
    }

    /// A simple message has nothing to show — and certainly not its own
    /// body disguised as an attachment.
    #[test]
    fn a_plain_message_has_no_attachments() {
        let raw = b"From: a@b.c
Subject: t

Just text.
";
        assert!(extract_attachments(raw).is_empty());
    }

    /// The indexes are contiguous and serve as re-download key: they must
    /// follow the KEPT attachments, not the MIME parts.
    #[test]
    fn indexes_are_contiguous_over_the_kept_attachments() {
        let raw = b"From: a@b.c
Subject: t
MIME-Version: 1.0
Content-Type: multipart/mixed; boundary=\"B\"

--B
Content-Type: text/html

<img src=\"cid:l\">
--B
Content-Type: image/png
Content-ID: <l>

PNG
--B
Content-Type: application/pdf
Content-Disposition: attachment; filename=\"un.pdf\"

A
--B
Content-Type: text/csv
Content-Disposition: attachment; filename=\"deux.csv\"

B
--B--
";
        let found = extract_attachments(raw);
        assert_eq!(found.len(), 2);
        assert_eq!(
            found[0].index, 0,
            "the discarded logo must not shift the indexes"
        );
        assert_eq!(found[1].index, 1);
        assert_eq!(found[1].name, "deux.csv");
    }

    #[test]
    fn inlines_embedded_cid_images_as_data_uris() {
        let raw = b"From: a@b.c\r\nSubject: t\r\nMIME-Version: 1.0\r\n\
Content-Type: multipart/related; boundary=\"B\"\r\n\r\n\
--B\r\nContent-Type: text/html; charset=utf-8\r\n\r\n\
<p>logo: <img src=\"cid:logo123\"></p>\r\n\
--B\r\nContent-Type: image/png\r\nContent-ID: <logo123>\r\n\
Content-Transfer-Encoding: base64\r\n\r\niVBORw0KGgo=\r\n--B--\r\n";
        let html = extract_html(raw).expect("html body expected");
        assert!(html.contains("data:image/png;base64,"));
        assert!(!html.contains("cid:logo123"));
    }

    #[test]
    fn converts_plain_text_message_to_html() {
        let raw = b"From: a@b.c\r\nSubject: t\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nHello <chevron>";
        let html = extract_html(raw).expect("text to html conversion expected");
        assert!(html.contains("Hello"));
        assert!(
            !html.contains("<chevron>"),
            "the text must be escaped, not interpreted"
        );
    }

    /// Gmail does not expose `\Archive` but exposes `\All`: expunging from
    /// INBOX only removes the label there, the message survives in "All
    /// Mail". That is the product's original semantics. The announced
    /// attribute counts, whatever the folder's name — Gmail names its own
    /// "[Gmail]/Messages envoyés".
    #[test]
    fn the_sent_folder_is_read_from_the_announced_attribute() {
        let folder = sent_folder([
            ("INBOX", SpecialUse::Other),
            ("[Gmail]/Messages envoy&AOk-s", SpecialUse::Sent),
            ("[Gmail]/Corbeille", SpecialUse::Other),
        ]);
        assert_eq!(folder.as_deref(), Some("[Gmail]/Messages envoy&AOk-s"));
    }

    /// Fallback by name, on the DECODED name: a French-speaking server
    /// announces "Envoy&AOk-s" in modified UTF-7. We still memorize the wire
    /// name, since it is what we will send back to the server.
    #[test]
    fn an_accented_sent_folder_is_recognized_under_its_encoded_form() {
        let folder = sent_folder([
            ("INBOX", SpecialUse::Other),
            ("Envoy&AOk-s", SpecialUse::Other),
        ]);
        assert_eq!(folder.as_deref(), Some("Envoy&AOk-s"));
    }

    /// The FULL name must match: "Sent/2024" is a filing, not the sent
    /// folder. Getting this wrong would let foreign mail into the
    /// conversations.
    #[test]
    fn a_subfolder_does_not_pass_for_the_sent_folder() {
        assert_eq!(
            sent_folder([
                ("INBOX", SpecialUse::Other),
                ("Sent/2024", SpecialUse::Other)
            ]),
            None
        );
    }

    /// No attribute, no known name: we do not guess. The account works as
    /// before — the threads only group the received messages.
    #[test]
    fn without_attribute_or_known_name_no_folder_is_invented() {
        assert_eq!(
            sent_folder([("INBOX", SpecialUse::Other), ("Bazar", SpecialUse::Other)]),
            None
        );
    }

    /// The attribute wins over the name, even if the name comes first: a
    /// personal "Sent" folder must not steal the place of the one the
    /// server designates.
    #[test]
    fn the_attribute_wins_over_a_homonym_met_before() {
        let folder = sent_folder([
            ("Sent", SpecialUse::Other),
            ("Elements envoyes", SpecialUse::Sent),
        ]);
        assert_eq!(folder.as_deref(), Some("Elements envoyes"));
    }

    #[test]
    fn gmail_archives_by_expunging_because_all_mail_catches_the_message() {
        let folders = [
            ("INBOX", SpecialUse::Other),
            ("[Gmail]/Tous les messages", SpecialUse::All),
            ("[Gmail]/Corbeille", SpecialUse::Other),
        ];
        assert_eq!(archive_strategy(folders), ArchiveStrategy::ExpungeOnly);
    }

    /// A generic server exposing `\Archive`: we MOVE the message there.
    /// UTF-7 debt settled. A French-speaking server announces its archive
    /// folder in modified UTF-7: `Archiv&AOk-s`. Without decoding, the name
    /// fallback did not recognize it, and archiving stayed unavailable on
    /// those accounts — exactly the Exchange case that motivated the
    /// fallback (ADR 0006).
    ///
    /// What is kept remains the WIRE name: it is what we will send back to
    /// the server, never its readable form.
    #[test]
    fn an_accented_archive_folder_is_recognised_through_its_encoded_name() {
        let strategy = archive_strategy([
            ("INBOX", SpecialUse::Other),
            ("Archiv&AOk-s", SpecialUse::Other),
        ]);
        assert_eq!(
            strategy,
            ArchiveStrategy::MoveTo("Archiv&AOk-s".to_string()),
            "the memorized name must remain the protocol's"
        );
    }

    #[test]
    fn generic_server_moves_to_its_archive_folder() {
        let folders = [
            ("INBOX", SpecialUse::Other),
            ("Archive", SpecialUse::Archive),
            ("Trash", SpecialUse::Other),
        ];
        assert_eq!(
            archive_strategy(folders),
            ArchiveStrategy::MoveTo("Archive".to_string())
        );
    }

    /// THE case that lost messages: neither `\Archive` nor `\All`. On a
    /// generic IMAP, expunging from INBOX DELETES for good — there is no
    /// safety net. We refuse rather than destroy.
    #[test]
    fn refuses_to_archive_when_expunging_would_destroy_the_message() {
        let folders = [("INBOX", SpecialUse::Other), ("Trash", SpecialUse::Other)];
        assert_eq!(archive_strategy(folders), ArchiveStrategy::Unsupported);
    }

    /// Exchange Online announces `\Drafts`, `\Junk`, `\Sent` and `\Trash`
    /// but NOT `\Archive` — while the "Archive" folder exists and serves
    /// (measured on a real account, spikes/microsoft). Without this
    /// fallback, archiving would be unavailable on every Microsoft account.
    #[test]
    fn falls_back_to_a_folder_named_archive_when_the_attribute_is_missing() {
        let exchange = [
            ("Archive", SpecialUse::Other),
            ("Archive/Achats", SpecialUse::Other),
            ("INBOX", SpecialUse::Other),
            ("Drafts", SpecialUse::Other),
            ("Deleted", SpecialUse::Other),
        ];
        assert_eq!(
            archive_strategy(exchange),
            ArchiveStrategy::MoveTo("Archive".to_string())
        );
    }

    #[test]
    fn named_archive_matches_whatever_the_case() {
        let folders = [
            ("INBOX", SpecialUse::Other),
            ("ARCHIVES", SpecialUse::Other),
        ];
        assert_eq!(
            archive_strategy(folders),
            ArchiveStrategy::MoveTo("ARCHIVES".to_string())
        );
    }

    /// An archive SUBfolder is not the archive folder: we would not pour
    /// the mail into "Archive/Achats".
    #[test]
    fn an_archive_subfolder_alone_does_not_count() {
        let folders = [
            ("INBOX", SpecialUse::Other),
            ("Archive/Achats", SpecialUse::Other),
        ];
        assert_eq!(archive_strategy(folders), ArchiveStrategy::Unsupported);
    }

    /// An announced attribute always counts against a mere name match: at
    /// Gmail, expunging IS archiving.
    #[test]
    fn announced_all_mail_wins_over_a_merely_named_folder() {
        let folders = [
            ("[Gmail]/Tous les messages", SpecialUse::All),
            ("Archive", SpecialUse::Other),
        ];
        assert_eq!(archive_strategy(folders), ArchiveStrategy::ExpungeOnly);
    }

    /// `\Archive` wins over `\All`: moving is always safer than expunging,
    /// whatever the order the folders are announced in.
    #[test]
    fn archive_folder_wins_over_all_mail_whatever_the_order() {
        let all_first = [("Tous", SpecialUse::All), ("Archives", SpecialUse::Archive)];
        let archive_first = [("Archives", SpecialUse::Archive), ("Tous", SpecialUse::All)];
        let expected = ArchiveStrategy::MoveTo("Archives".to_string());
        assert_eq!(archive_strategy(all_first), expected);
        assert_eq!(archive_strategy(archive_first), expected);
    }

    #[test]
    fn reads_both_threading_headers() {
        let raw = b"Subject: Devis\r\nIn-Reply-To: <a@b>\r\nReferences: <r@b> <a@b>\r\n\r\n";
        let headers = thread_headers(raw);
        assert_eq!(headers.in_reply_to.as_deref(), Some("<a@b>"));
        assert_eq!(headers.references.as_deref(), Some("<r@b> <a@b>"));
    }

    /// `References` is the header that folds most often: it grows at every
    /// turn of the conversation. Reading only the first line would lose
    /// precisely the root.
    #[test]
    fn a_folded_header_is_read_in_full() {
        let raw = b"References: <a@b>\r\n <c@d>\r\n\t<e@f>\r\nSubject: x\r\n\r\n";
        assert_eq!(
            thread_headers(raw).references.as_deref(),
            Some("<a@b> <c@d> <e@f>")
        );
    }

    #[test]
    fn the_header_name_is_case_insensitive() {
        let raw = b"REFERENCES: <a@b>\r\nin-reply-to: <c@d>\r\n\r\n";
        let headers = thread_headers(raw);
        assert_eq!(headers.references.as_deref(), Some("<a@b>"));
        assert_eq!(headers.in_reply_to.as_deref(), Some("<c@d>"));
    }

    /// Absent header: `Some("")`, not `None`. It is the mark "read, there is
    /// none" — without it, the pass would re-request this message
    /// indefinitely.
    #[test]
    fn the_absence_of_references_is_told_from_the_absence_of_a_read() {
        let headers = thread_headers(b"Subject: alone\r\n\r\n");
        assert_eq!(headers.references.as_deref(), Some(""));
        assert_eq!(headers.in_reply_to, None);
    }

    /// The empty line ends the headers. A body may contain "References:" in
    /// plain text — a quote, a code excerpt — and reading it there would
    /// give an invented attachment.
    #[test]
    fn a_header_quoted_in_the_body_is_ignored() {
        let raw = b"Subject: x\r\n\r\nReferences: <fake@b>\r\n";
        assert_eq!(thread_headers(raw).references.as_deref(), Some(""));
    }

    #[test]
    fn reads_a_text_draft_with_its_recipients() {
        let raw = b"To: Alice <alice@exemple.fr>, bob@exemple.fr\r\n\
                    Subject: Devis\r\n\
                    Content-Type: text/plain; charset=utf-8\r\n\
                    \r\n\
                    Hello Alice";
        let draft = draft_from_raw(raw).unwrap();
        assert_eq!(draft.to_raw, "alice@exemple.fr, bob@exemple.fr");
        assert_eq!(draft.subject, "Devis");
        assert_eq!(draft.text.as_deref().map(str::trim), Some("Hello Alice"));
    }

    /// A draft composed in a webmail often has ONLY HTML. Converting it is
    /// not the adapter's job: it returns both forms and lets the rendering
    /// layer decide.
    #[test]
    fn an_html_draft_returns_its_html_part() {
        let raw = b"To: alice@exemple.fr\r\n\
                    Subject: Devis\r\n\
                    Content-Type: text/html; charset=utf-8\r\n\
                    \r\n\
                    <p>Hello <b>Alice</b></p>";
        let draft = draft_from_raw(raw).unwrap();
        assert!(draft.html.unwrap().contains("<b>Alice</b>"));
    }

    /// A draft is allowed to be empty of everything: that is what
    /// distinguishes it from a message. Nothing must be rejected.
    #[test]
    fn a_draft_without_recipient_or_subject_stays_readable() {
        let draft = draft_from_raw(b"\r\nsome text alone").unwrap();
        assert_eq!(draft.to_raw, "");
        assert_eq!(draft.subject, "");
        assert_eq!(
            draft.text.as_deref().map(str::trim),
            Some("some text alone")
        );
    }

    /// RFC 2047-encoded headers are decoded as everywhere else.
    #[test]
    fn the_encoded_subject_is_decoded() {
        let raw = b"To: alice@exemple.fr\r\n\
                    Subject: =?UTF-8?Q?Devis_pour_l'=C3=A9t=C3=A9?=\r\n\
                    \r\n\
                    body";
        assert_eq!(draft_from_raw(raw).unwrap().subject, "Devis pour l'été");
    }
}

//! SMTP adapter: the real implementation of [`mail_core::MailTransport`].
//!
//! The core only knows the trait; this crate turns an [`OutboxMessage`]
//! into an RFC 5322 message (`lettre` crate) and hands it to the server in
//! XOAUTH2 — never a password, as for IMAP.
//!
//! Classification of failures (the port's contract):
//! - authentication happens at CONNECTION time (`test_connection`): an
//!   expired token fails the opening, never a send — otherwise a merely
//!   expired token would quarantine healthy messages;
//! - during the send, a 5xx response of the server is a refusal of the
//!   MESSAGE (`Permanent`), everything else (network, 4xx) is `Transient`.
//!
//! Gmail note: a message accepted over SMTP is added by Gmail itself to
//! the "Sent" folder — no IMAP APPEND to do. Other providers will require
//! it (Phase 3, multi-account).

use lettre::address::Envelope;
use lettre::message::header::{ContentType, Header, HeaderName, HeaderValue};
use lettre::message::{Attachment as FilePart, Mailbox, MultiPart, SinglePart};
use lettre::transport::smtp::SmtpTransportBuilder;
use lettre::transport::smtp::authentication::{Credentials, Mechanism};
use lettre::{Address, Message, SmtpTransport, Transport};
use mail_core::{DraftAttachmentFull, MailTransport, OutboxMessage, SendError};

pub struct SmtpMailer {
    transport: SmtpTransport,
}

/// TLS mode inferred from the SMTP submission port. 465 is the SMTPS port
/// (implicit TLS from the opening); 587 and the other submission ports
/// upgrade the encryption through STARTTLS. Never a cleartext fallback —
/// the "TLS everywhere" security rule holds (a server without STARTTLS
/// fails the opening, which is the wanted behavior).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SmtpTls {
    Implicit,
    StartTls,
}

fn smtp_tls_for_port(port: u16) -> SmtpTls {
    match port {
        465 => SmtpTls::Implicit,
        _ => SmtpTls::StartTls,
    }
}

/// Opens the transport to `host:port` according to the port's TLS policy.
///
/// SINGLE path of both authentication modes: that is what guarantees a fix
/// on the port policy can no longer benefit only one of the two. Bug #3
/// was born of that duplication.
fn transport_builder(host: &str, port: u16) -> Result<SmtpTransportBuilder, SendError> {
    match smtp_tls_for_port(port) {
        SmtpTls::Implicit => SmtpTransport::relay(host),
        SmtpTls::StartTls => SmtpTransport::starttls_relay(host),
    }
    .map(|builder| builder.port(port))
    .map_err(|err| SendError::Transient(err.to_string()))
}

impl SmtpMailer {
    /// TLS connection + XOAUTH2 authentication, verified immediately: a
    /// transport is only returned if it can send.
    ///
    /// The `port` is honored — Gmail listens on 465 (implicit TLS),
    /// `smtp.office365.com` only on 587 (STARTTLS).
    pub fn connect_xoauth2(
        host: &str,
        port: u16,
        user: &str,
        access_token: &str,
    ) -> Result<Self, SendError> {
        let transport = transport_builder(host, port)?
            .authentication(vec![Mechanism::Xoauth2])
            .credentials(Credentials::new(user.to_string(), access_token.to_string()))
            .build();
        Self::test_transport(transport)
    }

    /// TLS connection + password authentication (generic SMTP). Same port
    /// policy as XOAUTH2. Verified immediately.
    pub fn connect_password(
        host: &str,
        port: u16,
        user: &str,
        password: &str,
    ) -> Result<Self, SendError> {
        let transport = transport_builder(host, port)?
            .credentials(Credentials::new(user.to_string(), password.to_string()))
            .build();
        Self::test_transport(transport)
    }

    fn test_transport(transport: SmtpTransport) -> Result<Self, SendError> {
        match transport.test_connection() {
            Ok(true) => Ok(Self { transport }),
            Ok(false) => Err(SendError::Transient(
                "the SMTP server does not answer".to_string(),
            )),
            // Opening failure (network OR authentication): transient by
            // definition — the message was not even presented. The PREFIX
            // says which (E7): the shell only redoes the OAuth session on an
            // authentication refusal, never on a network failure (the P0
            // defect fixed on the IMAP side).
            Err(err) if err.status().is_none() => {
                Err(SendError::Transient(format!("connection: {err}")))
            }
            Err(err) => Err(SendError::Transient(format!("authentication: {err}"))),
        }
    }
}

/// The shell's discriminant (E7), twin of `mail_imap::is_connection_error`:
/// an opening failure WITHOUT a server response — network, TLS, timeout.
pub fn is_connection_error(err: &SendError) -> bool {
    matches!(err, SendError::Transient(msg) if msg.starts_with("connection"))
}

/// How to handle a send failure. Pure decision (STANDARD §4), tested
/// without network.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FailureClass {
    Transient,
    Permanent,
}

/// E7: `lettre` without `pool` reopens and re-authenticates at EACH send —
/// an OAuth token expired in the middle of a flush yields an AUTHENTICATION
/// 5xx (530/534/535/538) on a healthy message. It is the session that must
/// be redone, not the message: transient. The other 5xx (unknown
/// recipient, rejected message) stay definitive.
fn classify_failure(status: Option<u16>, permanent: bool) -> FailureClass {
    match status {
        Some(530 | 534 | 535 | 538) => FailureClass::Transient,
        _ if permanent => FailureClass::Permanent,
        _ => FailureClass::Transient,
    }
}

impl MailTransport for SmtpMailer {
    fn send(&mut self, message: &OutboxMessage) -> Result<(), SendError> {
        let email = build_message(message)?;
        // Bcc: the SMTP envelope carries ALL the recipients (To + Cc + Bcc),
        // but the served message (`build_message`) has NO Bcc header — that
        // is why we send through `send_raw` (explicit envelope) rather than
        // `send` (which would derive the envelope from the headers AND let
        // the Bcc leak into the body served to everyone).
        let envelope = build_envelope(message)?;
        let raw = email.formatted();
        match self.transport.send_raw(&envelope, &raw) {
            Ok(_) => Ok(()),
            Err(err) => {
                let status = err
                    .status()
                    .and_then(|code| code.to_string().parse::<u16>().ok());
                match classify_failure(status, err.is_permanent()) {
                    FailureClass::Permanent => Err(SendError::Permanent(err.to_string())),
                    FailureClass::Transient => Err(SendError::Transient(err.to_string())),
                }
            }
        }
    }
}

/// The SMTP envelope: sender + ALL the recipients (To, Cc, Bcc). It, and
/// not the message headers, drives the `RCPT TO` — the only place where a
/// Bcc address may appear.
fn build_envelope(message: &OutboxMessage) -> Result<Envelope, SendError> {
    let parse = |addr: &str| -> Result<Address, SendError> {
        addr.parse::<Address>()
            .map_err(|err| SendError::Permanent(format!("invalid address {addr:?}: {err}")))
    };
    let from = parse(&message.from)?;
    let recipients = message
        .to
        .iter()
        .chain(&message.cc)
        .chain(&message.bcc)
        .map(|addr| parse(addr))
        .collect::<Result<Vec<_>, _>>()?;
    Envelope::new(Some(from), recipients)
        .map_err(|err| SendError::Permanent(format!("SMTP envelope: {err}")))
}

/// `X-Priority: 1` header of a send marked important (R3, PLAN-RETOURS-6).
/// `lettre` has no built-in priority header; the X-Priority + Importance
/// pair is the one Outlook and Thunderbird set — and that Gmail and the
/// others read. Always "1": Wind knows one notch (important), not a scale.
#[derive(Debug, Clone)]
struct XPriority;

impl Header for XPriority {
    fn name() -> HeaderName {
        HeaderName::new_from_ascii_str("X-Priority")
    }

    fn parse(_s: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Self)
    }

    fn display(&self) -> HeaderValue {
        HeaderValue::new(Self::name(), "1".to_string())
    }
}

/// `Importance: high` header — the second of the pair (RFC 2156/4021).
#[derive(Debug, Clone)]
struct Importance;

impl Header for Importance {
    fn name() -> HeaderName {
        HeaderName::new_from_ascii_str("Importance")
    }

    fn parse(_s: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Self)
    }

    fn display(&self) -> HeaderValue {
        HeaderValue::new(Self::name(), "high".to_string())
    }
}

/// Turns an outbox message into an RFC 5322 message.
///
/// The Message-ID is THE journal's — never regenerated: it is what ties the
/// outbox entry to the message that really left (the "never a ghost send"
/// rule).
fn build_message(message: &OutboxMessage) -> Result<Message, SendError> {
    let mut builder = Message::builder()
        .from(parse_mailbox(&message.from)?)
        .subject(&message.subject)
        .message_id(Some(message.message_id.clone()))
        .date_now();
    for recipient in &message.to {
        builder = builder.to(parse_mailbox(recipient)?);
    }
    // Cc appears in the headers; Bcc NEVER (it lives in the SMTP envelope
    // alone, `build_envelope`) — that is its whole reason for being.
    for recipient in &message.cc {
        builder = builder.cc(parse_mailbox(recipient)?);
    }
    if let Some(parent) = &message.in_reply_to {
        // E7: the whole chain if the core knows it (References of the
        // parent + its Message-ID, RFC 5322 §3.6.4), otherwise the parent alone.
        let references = message.references.clone().unwrap_or_else(|| parent.clone());
        builder = builder.in_reply_to(parent.clone()).references(references);
    }
    // R3: the marking comes from the journal — it is what leaves, never a
    // screen state. Ordinary = no header (historical path intact).
    if message.important {
        builder = builder.header(XPriority).header(Importance);
    }
    // PLAN-INVITATIONS: the iTIP reply — text + `text/calendar;
    // method=REPLY` part as an alternative (the format Outlook emits, read
    // by Google/Exchange). By construction it has neither HTML nor
    // attachments; a journal carrying some is inconsistent — frank refusal,
    // never an ambiguous message.
    if let Some(ics) = &message.ics_reply {
        if !message.attachments.is_empty() || message.body_html.is_some() {
            return Err(SendError::Permanent(
                "iTIP reply with attachments or HTML: inconsistent journal, send refused"
                    .to_string(),
            ));
        }
        let calendar = SinglePart::builder()
            .header(
                ContentType::parse("text/calendar; method=REPLY; charset=utf-8")
                    .map_err(|err| SendError::Permanent(format!("calendar type: {err}")))?,
            )
            .body(ics.clone());
        return builder
            .multipart(
                MultiPart::alternative()
                    .singlepart(SinglePart::plain(message.body_text.clone()))
                    .singlepart(calendar),
            )
            .map_err(|err| SendError::Permanent(format!("message construction: {err}")));
    }
    if message.attachments.is_empty() {
        // Rich body: multipart/alternative — the text first (RFC 2046, from
        // the simplest to the most faithful), the HTML next. Without HTML,
        // the historical text path, byte for byte.
        return match &message.body_html {
            None => builder.body(message.body_text.clone()),
            Some(html) => builder.multipart(alternative_body(&message.body_text, html)),
        }
        .map_err(|err| SendError::Permanent(format!("message construction: {err}")));
    }
    // multipart/mixed: the body first (text alone, or nested text+HTML
    // alternative), then each attachment as the journal carries it (PJ-D2)
    // — the bytes come from the journal, never from a file re-read at send.
    let mut parts = match &message.body_html {
        None => MultiPart::mixed().singlepart(SinglePart::plain(message.body_text.clone())),
        Some(html) => MultiPart::mixed().multipart(alternative_body(&message.body_text, html)),
    };
    for attachment in &message.attachments {
        // Absent bytes sign a purged journal (PJ-D7): by construction, a
        // purged message is `sent`, hence never back in the queue. If the
        // invariant breaks, frank refusal — resending a message amputated
        // of its attachments would be worse.
        let bytes = attachment.bytes.clone().ok_or_else(|| {
            SendError::Permanent(format!(
                "attachment {:?} without bytes: purged journal, send refused",
                attachment.name
            ))
        })?;
        parts = parts.singlepart(file_part(&attachment.name, &attachment.mime, bytes)?);
    }
    builder
        .multipart(parts)
        .map_err(|err| SendError::Permanent(format!("message construction: {err}")))
}

/// The text+HTML alternative of a rich body — THE common constructor of
/// the four paths (send/draft × with/without attachments): the order of
/// the parts and the fallback are decided HERE, once.
fn alternative_body(text: &str, html: &str) -> MultiPart {
    MultiPart::alternative_plain_html(text.to_string(), html.to_string())
}

/// The MIME part of an attachment — THE common constructor of the send and
/// of the IMAP reflection: a fix (RFC 2231 name, type fallback) benefits
/// both, the lesson of bug #3.
///
/// An unknown type blocks nothing: the generic octet stream honestly says
/// "I do not know".
fn file_part(name: &str, mime: &str, bytes: Vec<u8>) -> Result<SinglePart, SendError> {
    let content_type = ContentType::parse(mime)
        .or_else(|_| ContentType::parse("application/octet-stream"))
        .map_err(|err| SendError::Permanent(format!("attachment type: {err}")))?;
    Ok(FilePart::new(name.to_string()).body(bytes, content_type))
}

/// RFC 5322 message of a draft, ready for an APPEND `\Draft` — the push to
/// the Gmail Drafts folder (Phase 2).
///
/// A draft carries raw text: invalid recipients are omitted (a half-typed
/// address stays local); if the message is not constructible as is, it is
/// simply not pushed — the local copy remains the reference, nothing is lost.
///
/// The attachments follow (PJ-D6): the remote reflection shows the WHOLE
/// draft, same part constructor as the send.
// The flat fields of a draft, all strings of the same register — the same
// compromise as `insert_draft` on the core side.
#[allow(clippy::too_many_arguments)]
pub fn draft_bytes(
    from: &str,
    to_raw: &str,
    cc_raw: &str,
    bcc_raw: &str,
    subject: &str,
    body: &str,
    body_html: Option<&str>,
    attachments: &[DraftAttachmentFull],
) -> Result<Vec<u8>, SendError> {
    let sender = parse_mailbox(from)?;
    let mut builder = Message::builder()
        .from(sender.clone())
        .subject(subject)
        .date_now();
    let mut recipients = 0usize;
    for candidate in to_raw.split([',', ';']) {
        if let Ok(mailbox) = candidate.trim().parse::<Mailbox>() {
            builder = builder.to(mailbox);
            recipients += 1;
        }
    }
    // A draft is the message as the user prepares it: it carries THEIR Cc
    // and Bcc (the Drafts folder is theirs alone, nothing is sent yet).
    // Tolerated addresses, like the To field.
    for candidate in cc_raw.split([',', ';']) {
        if let Ok(mailbox) = candidate.trim().parse::<Mailbox>() {
            builder = builder.cc(mailbox);
            recipients += 1;
        }
    }
    for candidate in bcc_raw.split([',', ';']) {
        if let Ok(mailbox) = candidate.trim().parse::<Mailbox>() {
            builder = builder.bcc(mailbox);
            recipients += 1;
        }
    }
    // No recipient typed YET: the draft is still a draft (field
    // 2026-09-04, PLAN-AUDIT-V3 STOP 2 — Gmail keeps recipient-less
    // drafts, the mirror must too). lettre derives its envelope from
    // the headers and refuses an empty destination — a draft is never
    // SENT, so we hand it an explicit envelope (the sender alone). The
    // envelope is transport-only: `formatted()` writes headers and
    // body, the pushed bytes carry no invented recipient.
    if recipients == 0 {
        let envelope =
            lettre::address::Envelope::new(Some(sender.email.clone()), vec![sender.email.clone()])
                .map_err(|err| SendError::Permanent(format!("draft construction: {err}")))?;
        builder = builder.envelope(envelope);
    }
    if attachments.is_empty() {
        // Same switch as the send: the reflection shows the rich draft as
        // multipart/alternative, the text draft stays single-part.
        return match body_html {
            None => builder.body(body.to_string()),
            Some(html) => builder.multipart(alternative_body(body, html)),
        }
        .map(|message| message.formatted())
        .map_err(|err| SendError::Permanent(format!("draft construction: {err}")));
    }
    let mut parts = match body_html {
        None => MultiPart::mixed().singlepart(SinglePart::plain(body.to_string())),
        Some(html) => MultiPart::mixed().multipart(alternative_body(body, html)),
    };
    for attachment in attachments {
        parts = parts.singlepart(file_part(
            &attachment.name,
            &attachment.mime,
            attachment.bytes.clone(),
        )?);
    }
    builder
        .multipart(parts)
        .map(|message| message.formatted())
        .map_err(|err| SendError::Permanent(format!("draft construction: {err}")))
}

fn parse_mailbox(address: &str) -> Result<Mailbox, SendError> {
    address
        .parse()
        .map_err(|err| SendError::Permanent(format!("invalid address {address:?}: {err}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mail_core::{OutboxAttachment, OutboxState};

    /// PLAN-AUDIT-V1 E7 (audit S2): `lettre` without `pool` reopens and
    /// re-authenticates at EACH send — an OAuth token expired in the middle
    /// of a long flush yields a 535 on a healthy message, and the code
    /// classified every 5xx as `Permanent`: message "refused", user gesture
    /// required. An AUTHENTICATION refusal is transient — it is the session
    /// that must be redone, not the message.
    #[test]
    fn a_535_in_the_middle_of_a_flush_is_transient() {
        assert!(matches!(
            classify_failure(Some(535), true),
            FailureClass::Transient
        ));
        assert!(matches!(
            classify_failure(Some(530), true),
            FailureClass::Transient
        ));
        assert!(matches!(
            classify_failure(Some(534), true),
            FailureClass::Transient
        ));
        assert!(matches!(
            classify_failure(Some(538), true),
            FailureClass::Transient
        ));
        assert!(
            matches!(classify_failure(Some(550), true), FailureClass::Permanent),
            "an unknown recipient remains a definitive refusal"
        );
        assert!(matches!(
            classify_failure(None, false),
            FailureClass::Transient
        ));
        assert!(matches!(
            classify_failure(Some(451), false),
            FailureClass::Transient
        ));
    }

    /// RFC 5322 §3.6.4: `References` = the parent's + its Message-ID. Before
    /// E7, the adapter only put the parent there: our own sends broke the
    /// thread at the recipient's from the 3rd message — and in our Sent
    /// folder re-read by `fetch_thread_headers`.
    #[test]
    fn references_carries_the_whole_chain() {
        let mut message = outbox_message(Some("<c@x>"));
        message.references = Some("<a@x> <b@x> <c@x>".to_string());
        let served = formatted(&message);
        assert!(served.contains("In-Reply-To: <c@x>"), "{served}");
        assert!(served.contains("References: <a@x> <b@x> <c@x>"), "{served}");
        // Without a known chain, the parent alone (the path before).
        let alone = formatted(&outbox_message(Some("<c@x>")));
        assert!(alone.contains("References: <c@x>"), "{alone}");
    }

    /// The shell did "any SMTP opening error ⇒ OAuth refresh": every network
    /// failure hammered the provider's endpoint (the P0 defect fixed on the
    /// IMAP side). Same discriminant, same prefix.
    #[test]
    fn an_smtp_network_failure_is_not_an_auth_refusal() {
        assert!(is_connection_error(&SendError::Transient(
            "connection smtp.exemple.fr:587: timed out".to_string()
        )));
        assert!(!is_connection_error(&SendError::Transient(
            "authentication: 535 5.7.8 Username and Password not accepted".to_string()
        )));
        assert!(!is_connection_error(&SendError::Permanent(
            "connection refused by the recipient".to_string()
        )));
    }
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    fn outbox_message(in_reply_to: Option<&str>) -> OutboxMessage {
        OutboxMessage {
            id: 1,
            account_id: 1,
            message_id: "<test.abc123@exemple.fr>".to_string(),
            from: "moi@exemple.fr".to_string(),
            to: vec!["a@exemple.fr".to_string(), "b@exemple.fr".to_string()],
            cc: vec![],
            bcc: vec![],
            subject: "Hello".to_string(),
            body_text: "First try.\nSecond line.".to_string(),
            body_html: None,
            in_reply_to: in_reply_to.map(str::to_string),
            references: None,
            important: false,
            send_at_epoch: None,
            ics_reply: None,
            attachments: vec![],
            state: OutboxState::Queued,
            attempts: 0,
            last_error: None,
            queued_epoch: 1_700_000_000,
        }
    }

    fn formatted(message: &OutboxMessage) -> String {
        let email = build_message(message).expect("constructible message");
        String::from_utf8(email.formatted()).expect("ASCII headers")
    }

    /// PLAN-INVITATIONS: the reply to an invitation leaves as
    /// `multipart/alternative` text + `text/calendar; method=REPLY` — it is
    /// the `method` parameter that Google/Exchange read to update the
    /// organizer's calendar.
    #[test]
    fn an_itip_reply_leaves_as_a_text_calendar_method_reply_part() {
        let mut message = outbox_message(None);
        message.body_text = "Reply sent from Wind.".to_string();
        message.ics_reply =
            Some("BEGIN:VCALENDAR\r\nMETHOD:REPLY\r\nEND:VCALENDAR\r\n".to_string());
        let raw = formatted(&message);
        assert!(
            raw.contains("multipart/alternative"),
            "text and calendar travel as an alternative:\n{raw}"
        );
        assert!(
            raw.contains("text/calendar") && raw.contains("method=REPLY"),
            "the calendar part must say method=REPLY:\n{raw}"
        );
        assert!(
            raw.contains("METHOD:REPLY"),
            "the VCALENDAR must travel whole:\n{raw}"
        );
        assert!(
            raw.contains("Reply sent from Wind."),
            "the text remains the readable fallback:\n{raw}"
        );
    }

    /// A journal that carried an iTIP reply WITH attachments or HTML is
    /// inconsistent by construction: frank refusal, never an ambiguous message.
    #[test]
    fn an_itip_reply_with_attachments_is_refused() {
        let mut message = outbox_message(None);
        message.ics_reply = Some("BEGIN:VCALENDAR\r\nEND:VCALENDAR\r\n".to_string());
        message.attachments = vec![OutboxAttachment {
            name: "trap.pdf".to_string(),
            mime: "application/pdf".to_string(),
            size: 4,
            bytes: Some(vec![1, 2, 3, 4]),
        }];
        assert!(build_message(&message).is_err());
    }

    #[test]
    fn builds_message_with_our_message_id_never_a_generated_one() {
        let raw = formatted(&outbox_message(None));
        assert!(
            raw.contains("Message-ID: <test.abc123@exemple.fr>"),
            "the journal's Message-ID must be the message's:\n{raw}"
        );
    }

    #[test]
    fn addresses_every_recipient() {
        let raw = formatted(&outbox_message(None));
        assert!(raw.contains("From: moi@exemple.fr"));
        assert!(raw.contains("a@exemple.fr"));
        assert!(raw.contains("b@exemple.fr"));
    }

    /// The heart of the Bcc correctness: the Cc appears in the headers of
    /// the SERVED message, the Bcc NEVER — it only lives in the SMTP
    /// envelope (the RCPT TO). A Bcc that leaks into the body served to
    /// everyone is no longer a Bcc. That is why the send goes through
    /// `send_raw` + `build_envelope`.
    #[test]
    fn cc_in_the_headers_bcc_in_the_envelope_only() {
        let mut message = outbox_message(None);
        message.cc = vec!["copie@exemple.fr".to_string()];
        message.bcc = vec!["invisible@exemple.fr".to_string()];

        let raw = formatted(&message);
        assert!(
            raw.contains("Cc: copie@exemple.fr"),
            "the Cc must appear:\n{raw}"
        );
        assert!(
            !raw.contains("invisible@exemple.fr"),
            "the Bcc must NEVER appear in the served message:\n{raw}"
        );
        assert!(
            !raw.to_lowercase().contains("bcc:"),
            "no Bcc header in the served message:\n{raw}"
        );

        // The envelope, for its part, carries ALL the recipients — Bcc
        // included: otherwise the Bcc would receive nothing.
        let envelope = build_envelope(&message).expect("constructible envelope");
        let rcpts: Vec<String> = envelope.to().iter().map(ToString::to_string).collect();
        assert!(rcpts.iter().any(|a| a == "a@exemple.fr"), "{rcpts:?}");
        assert!(rcpts.iter().any(|a| a == "copie@exemple.fr"), "{rcpts:?}");
        assert!(
            rcpts.iter().any(|a| a == "invisible@exemple.fr"),
            "the Bcc MUST be an envelope recipient: {rcpts:?}"
        );
    }

    /// R3 (PLAN-RETOURS-6): a send marked important carries THE pair of
    /// headers that mature clients read — `X-Priority: 1` and
    /// `Importance: high`. An ordinary send carries none.
    #[test]
    fn important_message_carries_priority_headers() {
        let mut message = outbox_message(None);
        message.important = true;
        let raw = formatted(&message);
        assert!(raw.contains("X-Priority: 1"), "{raw}");
        assert!(raw.contains("Importance: high"), "{raw}");
    }

    #[test]
    fn ordinary_message_has_no_priority_headers() {
        let raw = formatted(&outbox_message(None));
        assert!(!raw.contains("X-Priority"), "{raw}");
        assert!(!raw.contains("Importance"), "{raw}");
    }

    #[test]
    fn reply_carries_threading_headers() {
        let raw = formatted(&outbox_message(Some("<origine@exemple.fr>")));
        assert!(raw.contains("In-Reply-To: <origine@exemple.fr>"));
        assert!(raw.contains("References: <origine@exemple.fr>"));
    }

    #[test]
    fn fresh_message_has_no_threading_headers() {
        let raw = formatted(&outbox_message(None));
        assert!(!raw.contains("In-Reply-To"));
        assert!(!raw.contains("References"));
    }

    #[test]
    fn body_is_plain_text_with_preserved_lines() {
        let raw = formatted(&outbox_message(None));
        assert!(raw.contains("First try."));
        assert!(raw.contains("Second line."));
    }

    /// A message without attachment stays single-part: the historical path
    /// does not pay the multipart.
    #[test]
    fn message_without_attachments_stays_single_part() {
        let raw = formatted(&outbox_message(None));
        assert!(!raw.contains("multipart/mixed"));
    }

    /// PLAN-COMPOSITION-HTML E3: a rich body leaves as multipart/alternative
    /// — the text first (RFC 2046: from the simplest to the most faithful),
    /// the HTML next. Never HTML alone: the text fallback is systematic.
    #[test]
    fn html_body_travels_as_multipart_alternative_with_plain_fallback() {
        let mut message = outbox_message(None);
        message.body_html = Some("<b>First try.</b>".to_string());
        let raw = formatted(&message);

        assert!(raw.contains("multipart/alternative"), "{raw}");
        assert!(raw.contains("text/plain"), "{raw}");
        assert!(raw.contains("text/html"), "{raw}");
        assert!(raw.contains("<b>First try.</b>"), "{raw}");
        assert!(
            raw.contains("First try.\r\n") || raw.contains("First try.\n"),
            "the text fallback must leave too: {raw}"
        );
        let plain = raw.find("text/plain").unwrap();
        let html = raw.find("text/html").unwrap();
        assert!(plain < html, "the text precedes the HTML: {raw}");
        assert!(
            !raw.contains("multipart/mixed"),
            "without attachment: {raw}"
        );
    }

    /// With attachments, the alternative nests inside the mixed:
    /// mixed(alternative(text, html), attachment…) — the canonical form.
    #[test]
    fn html_body_with_attachments_nests_alternative_inside_mixed() {
        let mut message = outbox_message(None);
        message.body_html = Some("<b>body</b>".to_string());
        message.attachments = vec![attachment("report.pdf", Some(vec![0xFF, 0xD8, 0xFF, 0xE0]))];
        let raw = formatted(&message);

        assert!(raw.contains("multipart/mixed"), "{raw}");
        assert!(raw.contains("multipart/alternative"), "{raw}");
        assert!(raw.contains("<b>body</b>"), "{raw}");
        assert!(raw.contains("Content-Disposition: attachment"), "{raw}");
        assert!(raw.contains("report.pdf"), "{raw}");
        let mixed = raw.find("multipart/mixed").unwrap();
        let alternative = raw.find("multipart/alternative").unwrap();
        assert!(
            mixed < alternative,
            "the alternative lives INSIDE the mixed: {raw}"
        );
    }

    fn attachment(name: &str, bytes: Option<Vec<u8>>) -> OutboxAttachment {
        OutboxAttachment {
            name: name.to_string(),
            mime: "application/pdf".to_string(),
            size: bytes.as_ref().map_or(0, |b| b.len() as u64),
            bytes,
        }
    }

    /// PJ-D2 on the wire side: the message leaves as multipart/mixed — the
    /// text first, then each attachment as `attachment` with its name and
    /// its bytes. Binary bytes (high bit set) force base64: FF D8 FF E0
    /// encodes as "/9j/4A==".
    #[test]
    fn attachments_travel_as_multipart_mixed_attachments() {
        let mut message = outbox_message(None);
        message.attachments = vec![attachment("report.pdf", Some(vec![0xFF, 0xD8, 0xFF, 0xE0]))];
        let raw = formatted(&message);

        assert!(raw.contains("multipart/mixed"), "{raw}");
        assert!(raw.contains("Content-Disposition: attachment"), "{raw}");
        assert!(raw.contains("report.pdf"), "{raw}");
        assert!(raw.contains("/9j/4A=="), "the bytes must leave: {raw}");
        assert!(
            raw.contains("First try."),
            "the text remains the first part: {raw}"
        );
    }

    /// A non-ASCII name never leaves raw in the headers: `lettre` encodes it
    /// RFC 2231 (`filename*0*=utf-8''…`) — encoded, not lost.
    #[test]
    fn non_ascii_attachment_names_are_encoded_in_headers() {
        let mut message = outbox_message(None);
        message.attachments = vec![attachment("résumé années.pdf", Some(vec![1]))];
        let email = build_message(&message).expect("constructible message");
        let raw = String::from_utf8_lossy(&email.formatted()).to_string();

        assert!(
            !raw.contains("résumé"),
            "the name does not leave raw in the headers: {raw}"
        );
        assert!(
            raw.contains("filename*") || raw.contains("=?utf-8?"),
            "the name must be encoded, not lost: {raw}"
        );
        assert!(
            raw.contains("r%C3%A9sum%C3%A9") || raw.contains("=?utf-8?"),
            "the UTF-8 bytes of the name must be found: {raw}"
        );
    }

    /// The PJ-D7 invariant holds on the wire too: purged bytes (message
    /// already gone) NEVER build an amputated message — frank refusal.
    #[test]
    fn purged_attachment_is_a_permanent_refusal_never_an_amputated_message() {
        let mut message = outbox_message(None);
        message.attachments = vec![attachment("gone.pdf", None)];
        match build_message(&message) {
            Err(SendError::Permanent(reason)) => {
                assert!(reason.contains("gone.pdf"), "{reason}");
            }
            Err(other) => panic!("expected a permanent refusal, got {other:?}"),
            Ok(_) => panic!("expected a permanent refusal, got a built message"),
        }
    }

    /// An unreadable MIME type does not block the send: generic octet
    /// stream — the refusal at the gesture does not live here.
    #[test]
    fn unparseable_mime_falls_back_to_octet_stream() {
        let mut message = outbox_message(None);
        message.attachments = vec![OutboxAttachment {
            name: "raw.bin".to_string(),
            mime: "not a type".to_string(),
            size: 1,
            bytes: Some(vec![7]),
        }];
        let raw = formatted(&message);
        assert!(raw.contains("application/octet-stream"), "{raw}");
    }

    #[test]
    fn draft_bytes_keeps_valid_recipients_and_omits_the_rest() {
        let raw = draft_bytes(
            "moi@exemple.fr",
            "valide@exemple.fr, address-being-typ",
            "",
            "",
            "Draft",
            "body",
            None,
            &[],
        )
        .expect("constructible draft");
        let text = String::from_utf8_lossy(&raw);
        assert!(text.contains("valide@exemple.fr"));
        assert!(!text.contains("address-being-typ"));
        assert!(text.contains("Subject: Draft"));
    }

    /// A draft without any (yet) valid recipient IS pushable — the
    /// Chief Engineer's ruling at the PLAN-AUDIT-V3 field pass
    /// (2026-09-04) REVERSES the old documented limit ("stays local"):
    /// Gmail keeps recipient-less drafts, the mirror must too. The
    /// envelope handed to lettre is transport-only — the pushed bytes
    /// must carry no invented recipient.
    #[test]
    fn draft_without_any_valid_recipient_is_still_mirrored() {
        let raw = draft_bytes(
            "moi@exemple.fr",
            "no address yet",
            "",
            "",
            "s",
            "c",
            None,
            &[],
        )
        .expect("a recipient-less draft still builds");
        let text = String::from_utf8_lossy(&raw);
        assert!(!text.contains("To:"), "no invented To header");
        assert!(text.contains("Subject: s"));
        assert!(text.contains("From: moi@exemple.fr"));
    }

    /// PJ-D6: the remote reflection shows the WHOLE draft — attachments
    /// included, through the same part constructor as the send.
    #[test]
    fn draft_bytes_carries_attachments_as_multipart_mixed() {
        let raw = draft_bytes(
            "moi@exemple.fr",
            "valide@exemple.fr",
            "",
            "",
            "Draft",
            "body",
            None,
            &[DraftAttachmentFull {
                name: "quote.pdf".to_string(),
                mime: "application/pdf".to_string(),
                bytes: vec![0xFF, 0xD8, 0xFF, 0xE0],
            }],
        )
        .expect("constructible draft");
        let text = String::from_utf8_lossy(&raw);
        assert!(text.contains("multipart/mixed"), "{text}");
        assert!(text.contains("Content-Disposition: attachment"), "{text}");
        assert!(text.contains("quote.pdf"), "{text}");
        assert!(text.contains("/9j/4A=="), "the bytes must follow: {text}");
        assert!(
            text.contains("body"),
            "the text remains the first part: {text}"
        );
    }

    /// PLAN-COMPOSITION-HTML: the Drafts reflection shows the WHOLE rich
    /// draft — multipart/alternative, like the send.
    #[test]
    fn draft_bytes_with_html_is_multipart_alternative() {
        let raw = draft_bytes(
            "moi@exemple.fr",
            "valide@exemple.fr",
            "",
            "",
            "Draft",
            "body",
            Some("<b>body</b>"),
            &[],
        )
        .expect("constructible draft");
        let text = String::from_utf8_lossy(&raw);
        assert!(text.contains("multipart/alternative"), "{text}");
        assert!(text.contains("text/html"), "{text}");
        assert!(text.contains("<b>body</b>"), "{text}");
        assert!(
            text.contains("body"),
            "the text fallback must follow: {text}"
        );
    }

    /// A draft without attachment stays single-part — the historical path
    /// does not pay the multipart.
    #[test]
    fn draft_bytes_without_attachments_stays_single_part() {
        let raw = draft_bytes(
            "moi@exemple.fr",
            "valide@exemple.fr",
            "",
            "",
            "s",
            "c",
            None,
            &[],
        )
        .expect("constructible draft");
        assert!(!String::from_utf8_lossy(&raw).contains("multipart/mixed"));
    }

    /// Regression (bug #1): the SMTP submission port was ignored —
    /// `connect_password` wired `relay()` = implicit TLS 465 hard, and the
    /// port entered by the user was thrown away. The policy must
    /// distinguish 465 (SMTPS, implicit TLS) from 587 and the other
    /// submission ports (STARTTLS). Never a cleartext fallback.
    #[test]
    fn smtp_tls_policy_follows_the_submission_port() {
        assert_eq!(smtp_tls_for_port(465), SmtpTls::Implicit);
        assert_eq!(smtp_tls_for_port(587), SmtpTls::StartTls);
        assert_eq!(smtp_tls_for_port(25), SmtpTls::StartTls);
        assert_eq!(smtp_tls_for_port(2525), SmtpTls::StartTls);
    }

    /// Listens on an ephemeral port, accepts a connection then hangs up.
    /// Returns the port and a channel that signals the arrival: what is
    /// tested is the ARRIVAL of the connection, not the SMTP dialogue — a
    /// fake server that hangs up is enough, and keeps the test offline.
    fn fake_smtp_server() -> (u16, mpsc::Receiver<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("ephemeral port");
        let port = listener.local_addr().expect("local address").port();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            if listener.accept().is_ok() {
                let _ = tx.send(());
            }
        });
        (port, rx)
    }

    fn connection_arrived(rx: &mpsc::Receiver<()>) -> bool {
        rx.recv_timeout(Duration::from_secs(5)).is_ok()
    }

    /// Regression (bug #3): `connect_xoauth2` wired `relay()` — implicit TLS
    /// on 465 — and offered no port. The twin defect of the one fixed for
    /// passwords, invisible because a single provider was wired: Gmail does
    /// listen on 465, but `smtp.office365.com` only listens on 587/STARTTLS.
    #[test]
    fn xoauth2_connects_to_the_port_it_is_given() {
        let (port, arrived) = fake_smtp_server();
        // The connection necessarily fails (the fake server hangs up); only
        // its arrival on THE requested port is at stake.
        let _ = SmtpMailer::connect_xoauth2("127.0.0.1", port, "moi@exemple.fr", "token");
        assert!(
            connection_arrived(&arrived),
            "XOAUTH2 must reach the requested port, not a hard-wired 465"
        );
    }

    /// The counterpart for the password: keeps the fix of bug #1 from
    /// regressing when both paths are unified.
    #[test]
    fn password_connects_to_the_port_it_is_given() {
        let (port, arrived) = fake_smtp_server();
        let _ = SmtpMailer::connect_password("127.0.0.1", port, "moi@exemple.fr", "secret");
        assert!(
            connection_arrived(&arrived),
            "the password must reach the requested port"
        );
    }

    #[test]
    fn malformed_stored_address_is_a_permanent_error() {
        let mut message = outbox_message(None);
        message.to = vec!["not an address".to_string()];
        match build_message(&message) {
            Err(SendError::Permanent(_)) => {}
            Err(other) => panic!("expected a permanent refusal, got {other:?}"),
            Ok(_) => panic!("expected a permanent refusal, got a built message"),
        }
    }
}

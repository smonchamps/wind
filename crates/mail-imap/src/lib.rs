//! IMAP adapter: the first real implementation of [`mail_core::MailServer`].
//!
//! The core only knows the trait; this crate turns its operations into IMAP
//! commands (`imap` crate) and the server replies into domain types. One
//! crate per protocol: SMTP and Graph have their own.
//!
//! CONDSTORE (RFC 7162) is wired since E2b (PLAN-SYNCHRO): when the server
//! announces it, `changes_since` serves the delta (flags included) through
//! `UID FETCH … CHANGEDSINCE`, and HIGHESTMODSEQ is read at SELECT as at
//! STATUS. Without the announcement, `None` — the engine falls back on the
//! UID differential, a complete and tested path.

mod convert;
#[cfg(test)]
mod fake_server;
mod mutf7;
#[cfg(test)]
mod tests_e3;

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use imap_proto::NameAttribute;
use imap_proto::types::UidSetMember;
use mail_core::{
    Envelope, Error, FetchedBody, MailServer, MailboxSnapshot, MessageRecipients, RemoteDraft,
    ThreadHeaders, Uid,
};

/// The cycle's timeouts (P0, PLAN-SYNCHRO): without them, a network stalling
/// in the middle of a FETCH froze the poll without end or error — and the
/// UI's re-entrance guard skipped every following cycle, until restart.
/// Assumed as provisional: the IDLE spike (E4) re-investigates these values,
/// an IDLE read being long by nature.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// 120 s, the top of the plan's range (60–120 s), and not less: the timeout
/// runs per `read()`, not per command — it only fires if NOTHING more
/// arrives. A server chewing on a SEARCH over a big mailbox can stay silent
/// for tens of seconds before its first byte.
const IO_TIMEOUT: Duration = Duration::from_secs(120);

/// Opens the IMAP connection with the timeouts set BEFORE the first byte:
/// bounded TCP, read AND write timeouts on the socket, then direct TLS on
/// 993 and mandatory STARTTLS elsewhere — the exact behavior of the
/// `ClientBuilder` (AutoTls mode), which itself bounds nothing: that is the
/// whole reason to build by hand.
fn connect_client(
    host: &str,
    port: u16,
    connect_timeout: Duration,
    io_timeout: Duration,
) -> Result<imap::Client<imap::Connection>, Error> {
    let context = |err: String| Error::Server(format!("connection {host}:{port}: {err}"));
    // The resolution may return several addresses (IPv4/IPv6): each one
    // gets the same timeout, the first that answers wins.
    let mut tcp: Option<TcpStream> = None;
    let mut last: Option<std::io::Error> = None;
    for addr in (host, port)
        .to_socket_addrs()
        .map_err(|err| context(err.to_string()))?
    {
        match TcpStream::connect_timeout(&addr, connect_timeout) {
            Ok(stream) => {
                tcp = Some(stream);
                break;
            }
            Err(err) => last = Some(err),
        }
    }
    let mut tcp = match (tcp, last) {
        (Some(tcp), _) => tcp,
        (None, Some(err)) => return Err(context(err.to_string())),
        (None, None) => return Err(context("address not found".to_string())),
    };
    tcp.set_read_timeout(Some(io_timeout))
        .map_err(|err| context(err.to_string()))?;
    tcp.set_write_timeout(Some(io_timeout))
        .map_err(|err| context(err.to_string()))?;

    let connector = native_tls::TlsConnector::new().map_err(|err| context(err.to_string()))?;
    if port == 993 {
        let tls = connector
            .connect(host, tcp)
            .map_err(|err| context(err.to_string()))?;
        let mut client =
            imap::Client::new(Box::new(BoundedStream::new(tls, io_timeout)) as imap::Connection);
        client.read_greeting().map_err(server_err)?;
        Ok(client)
    } else {
        // STARTTLS: greeting then MANDATORY upgrade to TLS — a server that
        // refuses it is refused, never a cleartext session (same requirement
        // as the replaced AutoTls mode). Negotiated BY HAND on the socket:
        // the crate's public API cannot emit STARTTLS before authentication.
        // Two protocol lines, bounded by the same timeouts as everything else.
        read_line(&mut tcp).map_err(|err| context(err.to_string()))?; // greeting
        tcp.write_all(b"a1 STARTTLS\r\n")
            .map_err(|err| context(err.to_string()))?;
        loop {
            let line = read_line(&mut tcp).map_err(|err| context(err.to_string()))?;
            let Some(reply) = line.strip_prefix("a1 ") else {
                continue; // untagged line ("* …"): read on
            };
            if reply.starts_with("OK") {
                break;
            }
            return Err(context(format!("STARTTLS refused: {}", line.trim_end())));
        }
        let tls = connector
            .connect(host, tcp)
            .map_err(|err| context(err.to_string()))?;
        // No greeting to read: it was consumed in cleartext, the server
        // does not send another after STARTTLS (RFC 3501 §6.2.1).
        Ok(imap::Client::new(
            Box::new(BoundedStream::new(tls, io_timeout)) as imap::Connection,
        ))
    }
}

/// The TCP socket under a stream (bare, or under TLS): it is ON IT that the
/// read timeout is set — never on a clone (Windows: `SO_RCVTIMEO` is
/// specific to the handle, proven by a test that hung).
trait InnerSocket {
    fn socket(&self) -> &TcpStream;
}

impl InnerSocket for TcpStream {
    fn socket(&self) -> &TcpStream {
        self
    }
}

impl InnerSocket for native_tls::TlsStream<TcpStream> {
    fn socket(&self) -> &TcpStream {
        self.get_ref()
    }
}

/// The stream the `imap` crate wraps — with a read-timeout FLOOR
/// (PLAN-AUDIT-V1 E6, audit 2026-09-01 S1-5). The crate sets its own timeout
/// for the IDLE watch then REMOVES it on exit (`set_read_timeout(None)`,
/// `idle.rs`): the `+` of the next IDLE and the reply to the `DONE` were
/// read without bound — on a server or a NAT that acknowledges and goes
/// silent, the watcher froze without error or reconnection. Here, `None`
/// means the floor: the crate can no longer disarm the bound. The rest
/// (read, write) is delegated as is.
struct BoundedStream<S: Read + Write + Send + InnerSocket> {
    stream: S,
    floor: Duration,
}

impl<S: Read + Write + Send + InnerSocket> BoundedStream<S> {
    fn new(stream: S, floor: Duration) -> Self {
        Self { stream, floor }
    }
}

impl<S: Read + Write + Send + InnerSocket> Read for BoundedStream<S> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.stream.read(buf)
    }
}

impl<S: Read + Write + Send + InnerSocket> Write for BoundedStream<S> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.stream.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.stream.flush()
    }
}

impl<S: Read + Write + Send + InnerSocket> imap::extensions::idle::SetReadTimeout
    for BoundedStream<S>
{
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> imap::error::Result<()> {
        self.stream
            .socket()
            .set_read_timeout(Some(timeout.unwrap_or(self.floor)))
            .map_err(imap::Error::Io)
    }
}

/// Reads ONE `\n`-terminated line on the socket, bounded to 8 KiB: the
/// pre-TLS part of IMAP fits in two short lines, any overrun is suspect.
/// Byte by byte, deliberately — no buffer must swallow the first bytes of
/// the TLS handshake that follows.
fn read_line(tcp: &mut TcpStream) -> std::io::Result<String> {
    let mut line = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        if tcp.read(&mut byte)? == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "connection closed before the end of the line",
            ));
        }
        line.push(byte[0]);
        if byte[0] == b'\n' {
            return Ok(String::from_utf8_lossy(&line).into_owned());
        }
        if line.len() > 8 * 1024 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "oversized protocol line before TLS",
            ));
        }
    }
}

/// What one IDLE watch turn reports (ADR 0018).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Watch {
    /// An `EXISTS` arrived: mail is there, the account's light pass must go.
    Mail,
    /// The heartbeat delay elapsed without an event — heartbeat, we watch
    /// again.
    Timeout,
}

/// SASL XOAUTH2 string (Gmail, Microsoft): never a password.
struct XOAuth2 {
    user: String,
    access_token: String,
}

impl imap::Authenticator for XOAuth2 {
    type Response = String;

    fn process(&self, _challenge: &[u8]) -> Self::Response {
        format!(
            "user={}\x01auth=Bearer {}\x01\x01",
            self.user, self.access_token
        )
    }
}

pub struct ImapServer {
    session: imap::Session<Box<dyn imap::ImapConnection>>,
    selected: Option<(String, MailboxSnapshot)>,
    /// The special folders (RFC 6154), discovered by ONE `LIST` and memorized
    /// for the session (PLAN-AUDIT-V2 E3 — before, each of the four re-listed
    /// for itself alone). `None` = not looked up yet; inside, an absent
    /// folder is an absent capability, not a failure.
    special: Option<SpecialFolders>,
    /// The announced capabilities, read by ONE `CAPABILITY` and memorized:
    /// they do not change during a session. MOVE (RFC 6851), CONDSTORE
    /// (RFC 7162), LIST-STATUS (RFC 5819 — the one that melts ~51 STATUS into
    /// one round trip), UIDPLUS (RFC 4315) are read there.
    capabilities: Option<imap::types::Capabilities>,
}

/// What `LIST "" "*"` teaches at once about the special folders.
#[derive(Clone)]
struct SpecialFolders {
    trash: Option<String>,
    drafts: Option<String>,
    /// The sent folder: `None` = this server announces none — the
    /// conversations then only group received messages, exactly as before
    /// [ADR 0009]; synchronization goes on.
    sent: Option<String>,
    archive: convert::ArchiveStrategy,
}

/// The most bytes a batch of bodies may weigh (PLAN-AUDIT-V2 E3): beyond
/// that, the batch is cut — a message heavier than the bound travels alone.
/// Before, 50 whole messages left in one reply without looking at their
/// size (worst case beyond the gigabyte).
const BODY_BATCH_BYTES: u64 = 32 * 1024 * 1024;

/// How many envelopes a `changes_since` re-requests per round trip: after a
/// long offline period, the whole changed mailbox no longer fits in ONE
/// reply.
const CHANGES_BATCH: usize = 500;

impl ImapServer {
    fn new(session: imap::Session<Box<dyn imap::ImapConnection>>) -> Self {
        Self {
            session,
            selected: None,
            special: None,
            capabilities: None,
        }
    }

    /// An already open session, whatever its stream — for the tests' fake
    /// server.
    #[cfg(test)]
    pub(crate) fn for_test(session: imap::Session<Box<dyn imap::ImapConnection>>) -> Self {
        Self::new(session)
    }

    /// TLS connection + XOAUTH2 authentication with an OAuth2 access token.
    /// Timeouts set on the socket (P0): a stalling network is an error,
    /// never a freeze.
    pub fn connect_xoauth2(
        host: &str,
        port: u16,
        user: &str,
        access_token: &str,
    ) -> Result<Self, Error> {
        let client = connect_client(host, port, CONNECT_TIMEOUT, IO_TIMEOUT)?;
        let auth = XOAuth2 {
            user: user.to_string(),
            access_token: access_token.to_string(),
        };
        let session = client
            .authenticate("XOAUTH2", &auth)
            .map_err(|(err, _)| server_err(err))?;
        Ok(Self::new(session))
    }

    /// TLS connection + password authentication (generic IMAP). Same
    /// timeouts as the OAuth path (P0).
    pub fn connect_password(
        host: &str,
        port: u16,
        user: &str,
        password: &str,
    ) -> Result<Self, Error> {
        let client = connect_client(host, port, CONNECT_TIMEOUT, IO_TIMEOUT)?;
        let session = client
            .login(user, password)
            .map_err(|(err, _)| server_err(err))?;
        Ok(Self::new(session))
    }

    pub fn logout(mut self) {
        let _ = self.session.logout();
    }

    /// Selects the mailbox if it is not already selected (the engine calls
    /// `select` then chains the operations on the same mailbox).
    fn ensure_selected(&mut self, mailbox: &str) -> Result<MailboxSnapshot, Error> {
        if let Some((name, snapshot)) = &self.selected
            && name == mailbox
        {
            return Ok(*snapshot);
        }
        let selected = self.session.select(mailbox).map_err(server_err)?;
        let snapshot = MailboxSnapshot {
            uid_validity: selected
                .uid_validity
                .ok_or_else(|| Error::Server(format!("UIDVALIDITY absent for {mailbox}")))?,
            // OK [HIGHESTMODSEQ] comes with the SELECT on any CONDSTORE
            // server (RFC 7162 §3.1.2); absent = no CONDSTORE, the engine
            // keeps the UID differential.
            highest_modseq: selected.highest_mod_seq,
            exists: selected.exists,
        };
        self.selected = Some((mailbox.to_string(), snapshot));
        Ok(snapshot)
    }

    /// The server's special folders (RFC 6154) — never a hard-coded name:
    /// "[Gmail]/Corbeille" on a French account, "Trash" elsewhere. ONE
    /// `LIST "" "*"` per session, memorized.
    fn special_folders(&mut self) -> Result<&SpecialFolders, Error> {
        if self.special.is_none() {
            let names = self.session.list(None, Some("*")).map_err(server_err)?;
            let carries = |name: &imap::types::Name, wanted: NameAttribute| {
                name.attributes().contains(&wanted)
            };
            let first = |wanted: NameAttribute| {
                names
                    .iter()
                    .find(|name| carries(name, wanted.clone()))
                    .map(|name| name.name().to_string())
            };
            let roles = |role_of: &dyn Fn(&imap::types::Name) -> convert::SpecialUse| {
                names
                    .iter()
                    .map(|name| (name.name(), role_of(name)))
                    .collect::<Vec<_>>()
            };
            let archive = convert::archive_strategy(roles(&|name| {
                if carries(name, NameAttribute::Archive) {
                    convert::SpecialUse::Archive
                } else if carries(name, NameAttribute::All) {
                    convert::SpecialUse::All
                } else {
                    convert::SpecialUse::Other
                }
            }));
            let sent = convert::sent_folder(roles(&|name| {
                if carries(name, NameAttribute::Sent) {
                    convert::SpecialUse::Sent
                } else {
                    convert::SpecialUse::Other
                }
            }));
            self.special = Some(SpecialFolders {
                trash: first(NameAttribute::Trash),
                drafts: first(NameAttribute::Drafts),
                sent,
                archive,
            });
        }
        Ok(self
            .special
            .as_ref()
            .unwrap_or_else(|| unreachable!("set just above")))
    }

    /// The trash folder, or an error: the gestures that move there cannot
    /// do without it.
    fn trash_folder(&mut self) -> Result<String, Error> {
        self.special_folders()?
            .trash
            .clone()
            .ok_or_else(|| Error::Server("trash folder not found (RFC 6154)".to_string()))
    }

    /// The Drafts folder, if announced.
    ///
    /// `None` when the server does not announce the attribute: a generic
    /// IMAP may expose none. **It is not a failure, it is an absent
    /// capability** — and treating it as an error would repeat the same
    /// message at every sync, until the report means nothing any more.
    pub fn drafts_folder_name(&mut self) -> Result<Option<String>, Error> {
        Ok(self.special_folders()?.drafts.clone())
    }

    /// The Drafts folder, or an error — for the paths the user explicitly
    /// asked for (push, purge), where its absence must be stated.
    fn drafts_folder(&mut self) -> Result<String, Error> {
        self.drafts_folder_name()?
            .ok_or_else(|| Error::Server("drafts folder not found (RFC 6154)".to_string()))
    }

    /// What "archive" means on THIS server, inferred from its special
    /// folders (RFC 6154).
    fn archive_strategy(&mut self) -> Result<convert::ArchiveStrategy, Error> {
        Ok(self.special_folders()?.archive.clone())
    }

    /// The folder where THIS server stores our sent messages, if it has one.
    ///
    /// `None` is not a failure, it is an absent capability — same discipline
    /// as [`Self::drafts_folder_name`].
    pub fn sent_folder_name(&mut self) -> Result<Option<String>, Error> {
        Ok(self.special_folders()?.sent.clone())
    }

    /// UIDVALIDITY of the Drafts folder — the guard of the remote markers:
    /// if it changes, the recorded UIDs no longer mean anything.
    pub fn drafts_uidvalidity(&mut self) -> Result<u32, Error> {
        let folder = self.drafts_folder()?;
        Ok(self.ensure_selected(&folder)?.uid_validity)
    }

    /// The UIDs present in the server's Drafts folder.
    ///
    /// This is the "pull" half of the draft synchronization: until then we
    /// only pushed, and a draft started elsewhere stayed invisible here.
    pub fn draft_uids(&mut self) -> Result<Vec<Uid>, Error> {
        let folder = self.drafts_folder()?;
        self.ensure_selected(&folder)?;
        let uids = self.session.uid_search("ALL").map_err(server_err)?;
        Ok(uids.into_iter().collect())
    }

    /// Retrieves a draft from the server. `None` if it disappeared between
    /// the listing and the read — a mundane race, without consequence.
    ///
    /// `PEEK`: reading a draft must not mark it read.
    pub fn fetch_draft(&mut self, uid: Uid) -> Result<Option<RemoteDraft>, Error> {
        let folder = self.drafts_folder()?;
        self.ensure_selected(&folder)?;
        let fetches = self
            .session
            .uid_fetch(uid.to_string(), "(UID BODY.PEEK[])")
            .map_err(server_err)?;
        Ok(fetches
            .iter()
            .find_map(|fetch| convert::draft_from_raw(fetch.body()?)))
    }

    /// Pushes a draft copy (`\Draft`); returns its UID when the server
    /// announces it (APPENDUID/UIDPLUS — Gmail does). Without a UID, the
    /// copy cannot be replaced: a possible duplicate, assumed.
    pub fn append_draft(&mut self, message: &[u8]) -> Result<Option<Uid>, Error> {
        let folder = self.drafts_folder()?;
        let appended = self
            .session
            .append(&folder, message)
            .flag(imap::types::Flag::Draft)
            .finish()
            .map_err(server_err)?;
        let uid = appended.uids.and_then(|uids| {
            uids.into_iter().next().map(|member| match member {
                UidSetMember::Uid(uid) => uid,
                UidSetMember::UidRange(range) => *range.start(),
            })
        });
        Ok(uid)
    }

    /// Purges a remote draft copy — only UIDs the storage recorded itself
    /// (anti-wrong-deletion invariant).
    pub fn delete_draft_remote(&mut self, uid: Uid) -> Result<(), Error> {
        let folder = self.drafts_folder()?;
        self.ensure_selected(&folder)?;
        self.expunge_uid(uid)
    }

    /// One IDLE watch turn (RFC 2177) on `mailbox` — the ADAPTER's
    /// capability, outside the `MailServer` trait (ADR 0018): blocking by
    /// nature, it has no business in the engine's command flow. Returns
    /// [`Watch::Mail`] as soon as an `EXISTS` arrives (mail is there — the
    /// caller triggers the light pass), [`Watch::Timeout`] if `heartbeat`
    /// elapses without an event (the heartbeat: the DONE/re-IDLE of the next
    /// turn will prove the connection lives). An error = dead connection,
    /// the caller reconnects.
    ///
    /// `heartbeat` is ALSO the maximum detection delay of a dead connection
    /// (2nd field pass of the spike, 2026-08-14): a cut and a Windows sleep
    /// produce NO error, the read blocks silently until this deadline —
    /// 3 min, not the RFC's 29.
    ///
    /// Every read of this path is bounded: the `idle` handle sets
    /// `heartbeat` as the read timeout during the watch. (It resets the
    /// timeout to `None` on exit — acceptable HERE because a watch
    /// connection ONLY watches: the next turn re-sets its timeout. Never
    /// share this connection with a poll.)
    pub fn watch(&mut self, mailbox: &str, heartbeat: Duration) -> Result<Watch, Error> {
        self.ensure_selected(mailbox)?;
        let mut handle = self.session.idle();
        handle.timeout(heartbeat).keepalive(false);
        let outcome = handle.wait_while(|reply| {
            // `true` = keep waiting. Only EXISTS interrupts the watch: the
            // other replies (EXPUNGE, flag FETCH…) belong to the full cycle
            // and to CONDSTORE.
            !matches!(reply, imap::types::UnsolicitedResponse::Exists(_))
        });
        match outcome {
            Ok(imap::extensions::idle::WaitOutcome::MailboxChanged) => Ok(Watch::Mail),
            Ok(imap::extensions::idle::WaitOutcome::TimedOut) => Ok(Watch::Timeout),
            Err(err) => Err(server_err(err)),
        }
    }

    /// Does the server announce `name`? ONE `CAPABILITY` per session, read
    /// once: asking an extension of a server that does not announce it would
    /// be a BAD — hence the guards below.
    fn announces(&mut self, name: &str) -> Result<bool, Error> {
        if self.capabilities.is_none() {
            self.capabilities = Some(self.session.capabilities().map_err(server_err)?);
        }
        Ok(self
            .capabilities
            .as_ref()
            .is_some_and(|capabilities| capabilities.has_str(name)))
    }

    /// Can the server do MOVE (RFC 6851)?
    fn supports_move(&mut self) -> Result<bool, Error> {
        self.announces("MOVE")
    }

    /// Can the server do CONDSTORE (RFC 7162)? Decides the enriched STATUS
    /// (HIGHESTMODSEQ) and the `changes_since` delta.
    fn supports_condstore(&mut self) -> Result<bool, Error> {
        self.announces("CONDSTORE")
    }

    /// Can the server do LIST-STATUS (RFC 5819)? Decides the inventory in
    /// one round trip instead of ~51 STATUS.
    fn supports_list_status(&mut self) -> Result<bool, Error> {
        self.announces("LIST-STATUS")
    }

    /// Can the server do UIDPLUS (RFC 4315, `UID EXPUNGE`)? Without it, a
    /// `UID EXPUNGE` is a BAD: the copy succeeded then the original stayed —
    /// a duplicate at every cycle.
    fn supports_uidplus(&mut self) -> Result<bool, Error> {
        self.announces("UIDPLUS")
    }

    /// Marks `\Deleted` then expunges: the single targeted UID with UIDPLUS;
    /// without it, the RFC 3501 `EXPUNGE` (everything the mailbox carries as
    /// `\Deleted` — and those are messages meant to be deleted).
    fn expunge_uid(&mut self, uid: Uid) -> Result<(), Error> {
        self.session
            .uid_store(uid.to_string(), "+FLAGS.SILENT (\\Deleted)")
            .map_err(server_err)?;
        if self.supports_uidplus()? {
            self.session
                .uid_expunge(uid.to_string())
                .map_err(server_err)?;
        } else {
            self.session.expunge().map_err(server_err)?;
        }
        Ok(())
    }
}

impl MailServer for ImapServer {
    fn select(&mut self, mailbox: &str) -> Result<MailboxSnapshot, Error> {
        // Systematic re-selection: it is the refresh point of the snapshot
        // (UIDVALIDITY) at the start of a sync.
        self.selected = None;
        self.ensure_selected(mailbox)
    }

    fn list_uids(&mut self, mailbox: &str) -> Result<Vec<Uid>, Error> {
        self.ensure_selected(mailbox)?;
        let uids = self.session.uid_search("ALL").map_err(server_err)?;
        Ok(uids.into_iter().collect())
    }

    fn fetch_envelopes(&mut self, mailbox: &str, uids: &[Uid]) -> Result<Vec<Envelope>, Error> {
        self.ensure_selected(mailbox)?;
        if uids.is_empty() {
            return Ok(Vec::new());
        }
        let fetches = self
            .session
            .uid_fetch(convert::uid_set(uids), "(UID ENVELOPE INTERNALDATE FLAGS)")
            .map_err(server_err)?;
        Ok(fetches
            .iter()
            .filter_map(convert::fetch_to_envelope)
            .collect())
    }

    /// `BODY.PEEK[HEADER.FIELDS (…)]` — the three thread headers, and
    /// nothing else: twenty times fewer bytes than the whole block
    /// (PLAN-AUDIT-V2 E3; the old comment said the crate could not return
    /// this section — `imap-proto` files it under `MessageSection::Header`,
    /// so `fetch.header()` serves it).
    ///
    /// `PEEK`: reading headers must not set `\Seen` any more than reading a
    /// body.
    fn fetch_thread_headers(
        &mut self,
        mailbox: &str,
        uids: &[Uid],
    ) -> Result<Vec<(Uid, ThreadHeaders)>, Error> {
        if uids.is_empty() {
            return Ok(Vec::new());
        }
        self.ensure_selected(mailbox)?;
        let fetches = self
            .session
            .uid_fetch(
                convert::uid_set(uids),
                "(UID BODY.PEEK[HEADER.FIELDS (MESSAGE-ID IN-REPLY-TO REFERENCES)])",
            )
            .map_err(server_err)?;
        Ok(fetches
            .iter()
            .filter_map(|fetch| Some((fetch.uid?, convert::thread_headers(fetch.header()?))))
            .collect())
    }

    fn changes_since(
        &mut self,
        mailbox: &str,
        modseq: u64,
    ) -> Result<Option<Vec<Envelope>>, Error> {
        // Without the CONDSTORE announcement, `None`: the engine falls back
        // on the UID differential — the complete, tested path from before E2b.
        if !self.supports_condstore()? {
            return Ok(None);
        }
        self.ensure_selected(mailbox)?;
        // Everything that moved since the marker — new messages AND flags
        // (the reflection that was missing: a mail read on the phone stayed
        // unread here). CHANGEDSINCE is itself a "CONDSTORE enabling" command
        // (RFC 7162 §3.1): nothing to negotiate before.
        //
        // In TWO steps (PLAN-AUDIT-V2 E3): first the UIDs alone — one short
        // line per changed message, even after a month offline — then the
        // envelopes by bounded batches. Before, the whole changed mailbox
        // arrived in ONE reply.
        let changes = self
            .session
            .uid_fetch("1:*", format!("(UID FLAGS) (CHANGEDSINCE {modseq})"))
            .map_err(server_err)?;
        let uids: Vec<Uid> = changes.iter().filter_map(|fetch| fetch.uid).collect();
        let mut envelopes = Vec::with_capacity(uids.len());
        for batch in uids.chunks(CHANGES_BATCH) {
            let fetches = self
                .session
                .uid_fetch(convert::uid_set(batch), "(UID ENVELOPE INTERNALDATE FLAGS)")
                .map_err(server_err)?;
            envelopes.extend(fetches.iter().filter_map(convert::fetch_to_envelope));
        }
        Ok(Some(envelopes))
    }

    fn fetch_body_html(&mut self, mailbox: &str, uid: Uid) -> Result<Option<FetchedBody>, Error> {
        self.ensure_selected(mailbox)?;
        let fetches = self
            .session
            .uid_fetch(uid.to_string(), "(UID BODY.PEEK[])")
            .map_err(server_err)?;
        Ok(fetches
            .iter()
            .find_map(|fetch| body_from_raw(fetch.body()?)))
    }

    /// One `UID FETCH` command per batch — that is what makes the body
    /// backfill tenable (one round trip per message costs ~192 ms on a real
    /// server, cf. `spikes/body-backfill`). The sizes first (`RFC822.SIZE`,
    /// one line per message), then the bodies by batches bounded to
    /// [`BODY_BATCH_BYTES`]: a batch never weighs more than the bound, a
    /// heavier message travels alone (PLAN-AUDIT-V2 E3).
    ///
    /// `BODY.PEEK[]`: reading a body must never set `\Seen`. The UIDs the
    /// server no longer serves are simply absent from the result.
    fn fetch_bodies_html(
        &mut self,
        mailbox: &str,
        uids: &[Uid],
    ) -> Result<Vec<(Uid, FetchedBody)>, Error> {
        if uids.is_empty() {
            return Ok(Vec::new());
        }
        self.ensure_selected(mailbox)?;
        let sizes = self
            .session
            .uid_fetch(convert::uid_set(uids), "(UID RFC822.SIZE)")
            .map_err(server_err)?;
        let weighed: Vec<(Uid, u64)> = sizes
            .iter()
            .filter_map(|fetch| Some((fetch.uid?, u64::from(fetch.size.unwrap_or(0)))))
            .collect();
        let mut bodies = Vec::with_capacity(weighed.len());
        for batch in bounded_batches(&weighed, BODY_BATCH_BYTES) {
            let fetches = self
                .session
                .uid_fetch(convert::uid_set(&batch), "(UID BODY.PEEK[])")
                .map_err(server_err)?;
            bodies.extend(fetches.iter().filter_map(|fetch| {
                let uid = fetch.uid?;
                Some((uid, body_from_raw(fetch.body()?)?))
            }));
        }
        Ok(bodies)
    }

    /// Re-downloads the message to extract ONE attachment, rather than
    /// asking the server for the part (`BODY[2.1.3]`).
    ///
    /// It is a choice, and it costs: a full round trip (~192 ms measured)
    /// where a part FETCH would be lighter. In exchange, no MIME part number
    /// is ever computed — the arithmetic of nested parts is a classic source
    /// of bugs, and the index stays the one the local extraction produced,
    /// hence consistent with what is displayed. To revisit if big files
    /// hurt.
    fn fetch_attachment(
        &mut self,
        mailbox: &str,
        uid: Uid,
        index: usize,
    ) -> Result<Option<Vec<u8>>, Error> {
        self.ensure_selected(mailbox)?;
        let fetches = self
            .session
            .uid_fetch(uid.to_string(), "(UID BODY.PEEK[])")
            .map_err(server_err)?;
        Ok(fetches
            .iter()
            .find_map(|fetch| convert::attachment_bytes(fetch.body()?, index)))
    }

    /// Re-reads the message's ENVELOPE to extract To and Cc: the locally
    /// stored envelope does not carry them, "Reply all" asks for them at
    /// click time — an on-demand round trip, not a byte more in the
    /// database nor in the "envelopes first" sync.
    fn fetch_recipients(
        &mut self,
        mailbox: &str,
        uid: Uid,
    ) -> Result<Option<MessageRecipients>, Error> {
        self.ensure_selected(mailbox)?;
        let fetches = self
            .session
            .uid_fetch(uid.to_string(), "(UID ENVELOPE)")
            .map_err(server_err)?;
        Ok(fetches
            .iter()
            .find_map(|fetch| Some(convert::envelope_recipients(fetch.envelope()?))))
    }

    fn folders(&mut self) -> Result<Vec<mail_core::Folder>, Error> {
        let names = self.session.list(None, Some("*")).map_err(server_err)?;
        Ok(names.iter().map(name_to_folder).collect())
    }

    fn folders_with_status(&mut self) -> Result<Option<Vec<mail_core::FolderWithStatus>>, Error> {
        if !self.supports_list_status()? {
            return Ok(None);
        }
        // HIGHESTMODSEQ in the RETURN when CONDSTORE is there (E2b), like
        // `folder_status`: the same reading serves the guarded poll.
        let items = if self.supports_condstore()? {
            "(MESSAGES UIDNEXT UIDVALIDITY HIGHESTMODSEQ)"
        } else {
            "(MESSAGES UIDNEXT UIDVALIDITY)"
        };
        let names = self
            .session
            .list_status(None, Some("*"), items)
            .map_err(server_err)?;
        Ok(Some(
            names
                .iter()
                .map(|(name, mailbox)| {
                    // The STATUS stays OPTIONAL in the reply: RFC 5819 §2
                    // allows the server to omit it if it stumbles on a folder
                    // — the caller then treats that folder as unguarded (it
                    // will poll it, ADR 0017 caution).
                    let status = mailbox.as_ref().map(|mb| mail_core::FolderStatus {
                        messages: mb.exists,
                        uid_next: mb.uid_next,
                        uid_validity: mb.uid_validity,
                        highest_modseq: mb.highest_mod_seq,
                    });
                    (name_to_folder(name), status)
                })
                .collect(),
        ))
    }

    fn folder_status(&mut self, mailbox: &str) -> Result<mail_core::FolderStatus, Error> {
        // STATUS and not SELECT: the command is made to query a NON-selected
        // mailbox (RFC 3501 §6.3.10) — the engine's current selection is not
        // disturbed, and some servers charge a SELECT much more than a
        // STATUS. One round trip for the disk guard AND the guarded poll
        // (ADR 0017).
        // HIGHESTMODSEQ joins the reading when the server knows CONDSTORE
        // (E2b): it is what wakes a folder whose flags alone slipped.
        // Asking it of a server that does not announce it would be a BAD —
        // hence the guard.
        let items = if self.supports_condstore()? {
            "(MESSAGES UIDNEXT UIDVALIDITY HIGHESTMODSEQ)"
        } else {
            "(MESSAGES UIDNEXT UIDVALIDITY)"
        };
        let status = self.session.status(mailbox, items).map_err(server_err)?;
        Ok(mail_core::FolderStatus {
            messages: status.exists,
            uid_next: status.uid_next,
            uid_validity: status.uid_validity,
            highest_modseq: status.highest_mod_seq,
        })
    }

    /// MOVE if the server announces it, COPY + EXPUNGE otherwise.
    ///
    /// The fallback is not equivalent, and the gap deserves to be named:
    /// between the COPY and the EXPUNGE there is a window where a cut leaves
    /// the message in BOTH folders. It is a duplicate, not a loss — and the
    /// chosen order guarantees it will always be in that direction. Copy
    /// first, only remove next: "never lose a mail" (PLAN.md §1) wins over
    /// tidiness.
    fn move_to(&mut self, mailbox: &str, uid: Uid, target: &str) -> Result<(), Error> {
        self.ensure_selected(mailbox)?;
        if self.supports_move()? {
            return self
                .session
                .uid_mv(uid.to_string(), target)
                .map_err(server_err);
        }
        self.session
            .uid_copy(uid.to_string(), target)
            .map_err(server_err)?;
        self.expunge_uid(uid)
    }

    fn set_seen(&mut self, mailbox: &str, uid: Uid, seen: bool) -> Result<(), Error> {
        self.ensure_selected(mailbox)?;
        let query = if seen {
            "+FLAGS.SILENT (\\Seen)"
        } else {
            "-FLAGS.SILENT (\\Seen)"
        };
        self.session
            .uid_store(uid.to_string(), query)
            .map_err(server_err)?;
        Ok(())
    }

    fn set_flagged(&mut self, mailbox: &str, uid: Uid, flagged: bool) -> Result<(), Error> {
        self.ensure_selected(mailbox)?;
        let query = if flagged {
            "+FLAGS.SILENT (\\Flagged)"
        } else {
            "-FLAGS.SILENT (\\Flagged)"
        };
        self.session
            .uid_store(uid.to_string(), query)
            .map_err(server_err)?;
        Ok(())
    }

    /// Archiving depends on the server's capabilities, NEVER on the provider.
    ///
    /// At Gmail (`\All`), expunging from INBOX only removes the label: the
    /// message survives in "All Mail". On a generic IMAP, the same expunge
    /// would **destroy** the message — it must therefore be moved to
    /// `\Archive`. With neither, we refuse: "never lose a mail" (PLAN.md §1)
    /// wins over the availability of the feature.
    fn archive(&mut self, mailbox: &str, uid: Uid) -> Result<(), Error> {
        match self.archive_strategy()? {
            convert::ArchiveStrategy::MoveTo(folder) => {
                self.ensure_selected(mailbox)?;
                self.session
                    .uid_copy(uid.to_string(), &folder)
                    .map_err(server_err)?;
                self.expunge_uid(uid)
            }
            convert::ArchiveStrategy::ExpungeOnly => {
                self.ensure_selected(mailbox)?;
                self.expunge_uid(uid)
            }
            convert::ArchiveStrategy::Unsupported => Err(Error::Server(
                "this server exposes neither an Archive folder (\\Archive) nor \"all mail\" \
                 (\\All): archiving there would destroy the message"
                    .to_string(),
            )),
        }
    }

    fn delete(&mut self, mailbox: &str, uid: Uid) -> Result<(), Error> {
        let trash = self.trash_folder()?;
        self.ensure_selected(mailbox)?;
        self.session
            .uid_copy(uid.to_string(), &trash)
            .map_err(server_err)?;
        self.expunge_uid(uid)
    }
}

/// NO/BAD = the server understood and REFUSES (vanished folder, `[CANNOT]`,
/// `[TRYCREATE]`): `Error::Refusal`, definitive — the action journal
/// quarantines instead of retrying forever (E3). Everything else (I/O, TLS,
/// lost connection, unexpected reply) stays `Error::Server`, deemed
/// transient.
fn server_err(err: imap::Error) -> Error {
    match err {
        imap::Error::No(_) | imap::Error::Bad(_) => Error::Refusal(err.to_string()),
        other => Error::Server(other.to_string()),
    }
}

/// An IMAP `Name` (from LIST or LIST-STATUS) becomes a domain `Folder` —
/// single mapping, shared by `folders()` and `folders_with_status()` so
/// they cannot diverge.
fn name_to_folder(name: &imap::types::Name<'_>) -> mail_core::Folder {
    mail_core::Folder {
        wire: name.name().to_string(),
        // Decoded for the eye ONLY: `wire` remains what is sent back to the
        // server (RFC 3501 §5.1.3).
        display: mutf7::decode(name.name()),
        // `\Noselect` marks a container without mail: offering it as a
        // destination would produce a failure at the click.
        selectable: !name
            .attributes()
            .iter()
            .any(|attribute| matches!(attribute, NameAttribute::NoSelect)),
        // The RFC 6154 role the server announces — what it KNOWS about the
        // folder, where the name lets one guess (PLAN-AUDIT-V2 E5).
        special_use: name.attributes().iter().find_map(|attribute| {
            use mail_core::SpecialUse;
            Some(match attribute {
                NameAttribute::All => SpecialUse::All,
                NameAttribute::Archive => SpecialUse::Archive,
                NameAttribute::Drafts => SpecialUse::Drafts,
                NameAttribute::Junk => SpecialUse::Junk,
                NameAttribute::Sent => SpecialUse::Sent,
                NameAttribute::Trash => SpecialUse::Trash,
                _ => return None,
            })
        }),
    }
}

/// Does the failure come from the CONNECTION (resolution, TCP, TLS, P0
/// timeouts) rather than from what follows (authentication, protocol)?
///
/// The shell uses it to NOT refresh an OAuth token on a network failure: a
/// cut cable is not a dead token, and hammering the token endpoint at every
/// failed cycle is the best way to turn an IMAP throttling into an account
/// freeze.
///
/// The contract lives HERE, next to the format it inspects: every error of
/// [`connect_client`] is prefixed "connection host:port: …" — that prefix is
/// what counts.
pub fn is_connection_error(err: &Error) -> bool {
    matches!(err, Error::Server(msg) if msg.starts_with("connection "))
}

/// Splits weighed messages into batches whose sum does not exceed `bound`
/// — pure, in the order received; a single message heavier than the bound
/// makes a batch on its own (it has to be read anyway).
fn bounded_batches(weighed: &[(Uid, u64)], bound: u64) -> Vec<Vec<Uid>> {
    let mut batches: Vec<Vec<Uid>> = Vec::new();
    let mut current: Vec<Uid> = Vec::new();
    let mut weight = 0u64;
    for (uid, size) in weighed {
        if !current.is_empty() && weight + size > bound {
            batches.push(std::mem::take(&mut current));
            weight = 0;
        }
        current.push(*uid);
        weight += size;
    }
    if !current.is_empty() {
        batches.push(current);
    }
    batches
}

/// A raw message becomes a displayable body AND the description of its
/// attachments — both are read from the same bytes, in ONE MIME parse
/// (PLAN-AUDIT-V2 E3: before, three parses of the same message).
fn body_from_raw(raw: &[u8]) -> Option<FetchedBody> {
    convert::parse(raw)
}

#[cfg(test)]
mod body_from_raw_tests {
    use super::{BoundedStream, body_from_raw};

    /// E6: `None` never removes the bound — it is the gesture the crate makes
    /// on exiting the watch, and it is what left the `DONE` and the next IDLE
    /// without a timeout.
    #[test]
    fn a_bounded_stream_refuses_to_lose_its_bound() {
        use imap::extensions::idle::SetReadTimeout;
        use std::net::{TcpListener, TcpStream};
        use std::time::Duration;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let _server = listener.accept().unwrap();
        let mut stream = BoundedStream::new(origin.try_clone().unwrap(), Duration::from_secs(7));
        stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        assert_eq!(
            stream.stream.read_timeout().unwrap(),
            Some(Duration::from_secs(1)),
            "an explicit timeout passes as is"
        );
        stream.set_read_timeout(None).unwrap();
        assert_eq!(
            stream.stream.read_timeout().unwrap(),
            Some(Duration::from_secs(7)),
            "None means the floor, never \"unbounded\""
        );
    }

    /// And a read on a mute server does return at the floor — on the SAME
    /// handle as the one that reads (the lesson of the clones).
    #[test]
    fn a_read_on_a_mute_server_expires_at_the_floor() {
        use imap::extensions::idle::SetReadTimeout;
        use std::io::Read;
        use std::net::{TcpListener, TcpStream};
        use std::time::{Duration, Instant};
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let _mute_server = listener.accept().unwrap();
        let mut stream = BoundedStream::new(origin, Duration::from_millis(200));
        stream.set_read_timeout(None).unwrap(); // the crate's gesture
        let start = Instant::now();
        let mut byte = [0u8; 1];
        assert!(stream.read(&mut byte).is_err());
        assert!(start.elapsed() < Duration::from_secs(5));
    }

    /// Case C of the PLAN-INVITATIONS finding: the root IS the invitation.
    #[test]
    fn a_calendar_root_message_stays_displayable() {
        let raw = "From: claire@exemple.fr\r\nTo: nous@wind.example\r\n\
                   Subject: Invitation\r\nMIME-Version: 1.0\r\n\
                   Content-Type: text/calendar; method=REQUEST; charset=utf-8\r\n\r\n\
                   BEGIN:VCALENDAR\r\nVERSION:2.0\r\nMETHOD:REQUEST\r\n\
                   BEGIN:VEVENT\r\nUID:r1@exemple.fr\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let fetched = body_from_raw(raw.as_bytes()).expect("displayable");
        assert!(
            fetched
                .ics
                .as_deref()
                .is_some_and(|i| i.contains("METHOD:REQUEST"))
        );
    }

    #[test]
    fn an_unparseable_message_stays_none() {
        assert_eq!(body_from_raw(b"\xff\xfe not a message"), None);
    }
}

#[cfg(test)]
mod connect_timeout_tests {
    use super::connect_client;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::time::{Duration, Instant};

    /// THE P0 contract (PLAN-SYNCHRO): a server that accepts the connection
    /// then stays silent forever must become a bounded ERROR. Before P0,
    /// `ClientBuilder::connect()` read the greeting without a timeout: the
    /// whole cycle froze, silently, and the UI's re-entrance guard forbade
    /// any following cycle — no more mail until restart.
    #[test]
    fn a_mute_server_fails_instead_of_freezing() {
        // A real listener never served: the TCP handshake succeeds (kernel
        // backlog), then nothing — the stalling network.
        let silent = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = silent.local_addr().unwrap().port();

        let start = Instant::now();
        let outcome = connect_client(
            "127.0.0.1",
            port,
            Duration::from_secs(5),
            Duration::from_millis(200),
        );

        assert!(outcome.is_err(), "a mute server must not return a client");
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "the failure must come from the read timeout, not a freeze: {:?}",
            start.elapsed()
        );
    }

    /// The TLS upgrade is MANDATORY: a server that refuses STARTTLS is
    /// refused in turn — never a cleartext session (the requirement of the
    /// AutoTls mode the manual connection replaces). The interleaved
    /// untagged line checks that the read skips the "* …" without mistaking
    /// them for the reply.
    #[test]
    fn a_refused_starttls_is_a_frank_error() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            sock.write_all(b"* OK ready\r\n").unwrap();
            let mut buffer = [0u8; 64];
            let _ = sock.read(&mut buffer).unwrap();
            sock.write_all(b"* CAPABILITY IMAP4rev1\r\na1 NO not here\r\n")
                .unwrap();
        });

        let outcome = connect_client(
            "127.0.0.1",
            port,
            Duration::from_secs(5),
            Duration::from_secs(5),
        );
        server.join().unwrap();

        let error = outcome
            .expect_err("a STARTTLS refusal must be an error")
            .to_string();
        assert!(error.contains("STARTTLS refused"), "error: {error}");
    }

    /// The discriminant's contract: a CONNECTION error is recognized (the
    /// shell does not refresh an OAuth token on a network failure), an
    /// authentication or protocol error is not.
    #[test]
    fn a_network_failure_is_told_from_an_authentication_refusal() {
        let silent = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = silent.local_addr().unwrap().port();
        let failure = connect_client(
            "127.0.0.1",
            port,
            Duration::from_secs(5),
            Duration::from_millis(200),
        )
        .expect_err("mute server");
        assert!(super::is_connection_error(&failure), "failure: {failure}");

        let refusal =
            mail_core::Error::Server("AUTHENTICATIONFAILED Invalid credentials".to_string());
        assert!(!super::is_connection_error(&refusal));
    }
}

//! Fake server shared by the tests: replays field scenarios — additions,
//! deletions, flags, UIDVALIDITY switch, message bodies — and journals
//! calls to check the order and count of accesses.

use std::collections::BTreeMap;

use chrono::{TimeZone, Utc};

use crate::cycle::CycleConnection;
use crate::envelope::{Envelope, Uid};
use crate::error::Error;
use crate::remote::{FetchedBody, MailServer, MailboxSnapshot, ThreadHeaders};
use crate::store::Store;

pub(crate) struct FakeServer {
    pub(crate) uid_validity: u32,
    pub(crate) condstore: bool,
    pub(crate) modseq: u64,
    pub(crate) messages: BTreeMap<Uid, (Envelope, u64)>,
    pub(crate) bodies: BTreeMap<Uid, String>,
    /// The `text/calendar` part served with the body, by UID — the
    /// simulator's invitation (PLAN-INVITATIONS).
    pub(crate) ics: BTreeMap<Uid, String>,
    pub(crate) fetch_batches: Vec<Vec<Uid>>,
    /// Number of paid UID inventories (`UID SEARCH ALL` on the real
    /// server) — the proof that E2b only requests them when the count
    /// requires it.
    pub(crate) uid_list_calls: usize,
    pub(crate) body_fetches: usize,
    /// Batches of bodies requested, in order: this is what proves that
    /// backfill batches instead of chaining round trips.
    pub(crate) body_batches: Vec<Vec<Uid>>,
    /// `References` served by the fake server, by UID.
    pub(crate) references: BTreeMap<Uid, String>,
    /// Batches of headers requested: the proof that the pass batches.
    pub(crate) header_batches: Vec<Vec<Uid>>,
    pub(crate) folders: Vec<crate::remote::Folder>,
    /// Moves received: (uid, network target folder).
    pub(crate) moved: Vec<(Uid, String)>,
    /// Bytes served for (uid, rank) — the simulator's attachments.
    pub(crate) attachment_bytes: BTreeMap<(Uid, usize), Vec<u8>>,
    /// Log of actions received, in order (`seen:1:true`, `archive:2`…).
    pub(crate) action_calls: Vec<String>,
    /// Simulates a cut on actions ("zero loss" test).
    pub(crate) actions_fail: bool,
    /// E3: the server REFUSES moves (NO/BAD — folder gone), a
    /// definitive refusal, not a cut.
    pub(crate) refused_moves: bool,
    /// PLAN-AUDIT-V2 E5: the server cuts at the n-th `fetch_envelopes`
    /// (1 = the first) — the initial sync interrupted at batch k.
    pub(crate) envelope_batch_failure: Option<usize>,
    /// Flag windows requested (RETOURS-15 E3, D-51): the proof that the
    /// window is BOUNDED and asked in one batch.
    pub(crate) flag_batches: Vec<Vec<Uid>>,
}

impl FakeServer {
    pub(crate) fn new(condstore: bool) -> Self {
        Self {
            uid_validity: 1,
            condstore,
            modseq: 0,
            messages: BTreeMap::new(),
            bodies: BTreeMap::new(),
            ics: BTreeMap::new(),
            fetch_batches: Vec::new(),
            uid_list_calls: 0,
            body_fetches: 0,
            body_batches: Vec::new(),
            references: BTreeMap::new(),
            header_batches: Vec::new(),
            folders: Vec::new(),
            moved: Vec::new(),
            attachment_bytes: BTreeMap::new(),
            action_calls: Vec::new(),
            actions_fail: false,
            refused_moves: false,
            envelope_batch_failure: None,
            flag_batches: Vec::new(),
        }
    }

    /// Sets the `References` the server will serve for this message —
    /// the header the ENVELOPE does not carry.
    pub(crate) fn set_references(&mut self, uid: Uid, references: &str) {
        self.references.insert(uid, references.to_string());
    }

    /// Sets the To/Cc recipients the server's ENVELOPE will carry for
    /// this message — what the send backfill rereads (R4).
    pub(crate) fn set_envelope_recipients(&mut self, uid: Uid, to: &[&str], cc: &[&str]) {
        if let Some((envelope, _)) = self.messages.get_mut(&uid) {
            envelope.to_addrs = to.iter().map(|s| s.to_string()).collect();
            envelope.cc_addrs = cc.iter().map(|s| s.to_string()).collect();
        }
    }

    /// A bare envelope for tests that do not need the simulator.
    pub(crate) fn simple_envelope(uid: Uid, subject: &str) -> Envelope {
        Envelope {
            reply_to: None,
            uid,
            subject: Some(subject.to_string()),
            sender: Some("Alice".to_string()),
            sender_address: Some("alice@example.com".to_string()),
            message_id: Some(format!("<m{uid}@example.com>")),
            in_reply_to: None,
            date: Some(
                chrono::Utc
                    .timestamp_opt(1_700_000_000 + i64::from(uid), 0)
                    .unwrap(),
            ),
            seen: false,
            flagged: false,
            to_addrs: Vec::new(),
            cc_addrs: Vec::new(),
        }
    }

    pub(crate) fn add(&mut self, uid: Uid, subject: &str) {
        self.modseq += 1;
        let envelope = Envelope {
            reply_to: None,
            uid,
            subject: Some(subject.to_string()),
            sender: Some("alice@example.com".to_string()),
            sender_address: Some("alice@example.com".to_string()),
            message_id: Some(format!("<fake-{uid}@example.com>")),
            in_reply_to: None,
            // The date follows the UID: the bigger the UID, the more
            // recent it is.
            date: Some(
                Utc.timestamp_opt(1_700_000_000 + i64::from(uid), 0)
                    .unwrap(),
            ),
            seen: false,
            flagged: false,
            to_addrs: Vec::new(),
            cc_addrs: Vec::new(),
        };
        self.messages.insert(uid, (envelope, self.modseq));
    }

    pub(crate) fn add_with_body(&mut self, uid: Uid, subject: &str, html: &str) {
        self.add(uid, subject);
        self.bodies.insert(uid, html.to_string());
    }

    pub(crate) fn expunge(&mut self, uid: Uid) {
        self.messages.remove(&uid);
        self.bodies.remove(&uid);
        self.modseq += 1;
    }

    pub(crate) fn mark_seen(&mut self, uid: Uid) {
        self.modseq += 1;
        if let Some((envelope, modseq)) = self.messages.get_mut(&uid) {
            envelope.seen = true;
            *modseq = self.modseq;
        }
    }

    pub(crate) fn mark_flagged(&mut self, uid: Uid) {
        self.modseq += 1;
        if let Some((envelope, modseq)) = self.messages.get_mut(&uid) {
            envelope.flagged = true;
            *modseq = self.modseq;
        }
    }

    pub(crate) fn bump_uid_validity(&mut self) {
        self.uid_validity += 1;
    }
}

impl MailServer for FakeServer {
    fn select(&mut self, _mailbox: &str) -> Result<MailboxSnapshot, Error> {
        Ok(MailboxSnapshot {
            uid_validity: self.uid_validity,
            highest_modseq: self.condstore.then_some(self.modseq),
            // The REAL message count of the fixture, not a constant: a
            // fake server that announced anything else than what it
            // serves would make progress tests pass on a false model.
            exists: self.messages.len() as u32,
        })
    }

    fn list_uids(&mut self, _mailbox: &str) -> Result<Vec<Uid>, Error> {
        self.uid_list_calls += 1;
        Ok(self.messages.keys().copied().collect())
    }

    fn folder_status(&mut self, _mailbox: &str) -> Result<crate::remote::FolderStatus, Error> {
        // The REAL fixture state, same rule as `exists` in `select`:
        // announcing anything else than what is served would make disk
        // guard or guarded-polling tests pass on a false model.
        Ok(crate::remote::FolderStatus {
            messages: self.messages.len() as u32,
            uid_next: Some(self.messages.keys().max().copied().unwrap_or(0) + 1),
            uid_validity: Some(self.uid_validity),
            highest_modseq: self.condstore.then_some(self.modseq),
        })
    }

    fn fetch_envelopes(&mut self, _mailbox: &str, uids: &[Uid]) -> Result<Vec<Envelope>, Error> {
        if self.envelope_batch_failure == Some(self.fetch_batches.len() + 1) {
            return Err(Error::Server("simulated batch cut".to_string()));
        }
        self.fetch_batches.push(uids.to_vec());
        Ok(uids
            .iter()
            .filter_map(|uid| self.messages.get(uid))
            .map(|(envelope, _)| envelope.clone())
            .collect())
    }

    fn changes_since(
        &mut self,
        _mailbox: &str,
        modseq: u64,
    ) -> Result<Option<Vec<Envelope>>, Error> {
        if !self.condstore {
            return Ok(None);
        }
        Ok(Some(
            self.messages
                .values()
                .filter(|(_, m)| *m > modseq)
                .map(|(envelope, _)| envelope.clone())
                .collect(),
        ))
    }

    fn fetch_flags(
        &mut self,
        _mailbox: &str,
        uids: &[Uid],
    ) -> Result<Vec<crate::remote::FlagState>, Error> {
        self.flag_batches.push(uids.to_vec());
        Ok(uids
            .iter()
            .filter_map(|uid| {
                self.messages
                    .get(uid)
                    .map(|(e, _)| crate::remote::FlagState {
                        uid: *uid,
                        seen: e.seen,
                        flagged: e.flagged,
                    })
            })
            .collect())
    }

    fn fetch_bodies_html(
        &mut self,
        _mailbox: &str,
        uids: &[Uid],
    ) -> Result<Vec<(Uid, FetchedBody)>, Error> {
        self.body_batches.push(uids.to_vec());
        Ok(uids
            .iter()
            .filter_map(|uid| {
                self.bodies.get(uid).map(|html| {
                    let mut fetched = FetchedBody::html(html);
                    fetched.ics = self.ics.get(uid).cloned();
                    (*uid, fetched)
                })
            })
            .collect())
    }

    /// The fake server returns the headers set via
    /// [`Self::set_thread_headers`], and records the batches requested.
    fn fetch_thread_headers(
        &mut self,
        _mailbox: &str,
        uids: &[Uid],
    ) -> Result<Vec<(Uid, ThreadHeaders)>, Error> {
        self.header_batches.push(uids.to_vec());
        Ok(uids
            .iter()
            .filter_map(|uid| {
                self.messages.get(uid)?;
                Some((
                    *uid,
                    ThreadHeaders {
                        in_reply_to: None,
                        // Always `Some`: the server answered, even if
                        // the message has no `References`.
                        references: Some(self.references.get(uid).cloned().unwrap_or_default()),
                    },
                ))
            })
            .collect())
    }

    /// The fake server serves the bytes it was given — enough to prove
    /// the path without ever touching the network.
    fn fetch_attachment(
        &mut self,
        _mailbox: &str,
        uid: Uid,
        index: usize,
    ) -> Result<Option<Vec<u8>>, Error> {
        Ok(self
            .attachment_bytes
            .get(&(uid, index))
            .map(|bytes| bytes.to_vec()))
    }

    fn folders(&mut self) -> Result<Vec<crate::remote::Folder>, Error> {
        Ok(self.folders.clone())
    }

    /// The simulator records the requested moves: this is what allows
    /// proving REPLAY without a network.
    fn move_to(&mut self, _mailbox: &str, uid: Uid, target: &str) -> Result<(), Error> {
        if self.actions_fail {
            return Err(Error::Server("simulated cut".to_string()));
        }
        if self.refused_moves {
            return Err(Error::Refusal(format!(
                "[TRYCREATE] {target} does not exist"
            )));
        }
        self.moved.push((uid, target.to_string()));
        self.messages.remove(&uid);
        Ok(())
    }

    fn set_seen(&mut self, _mailbox: &str, uid: Uid, seen: bool) -> Result<(), Error> {
        if self.actions_fail {
            return Err(Error::Server("simulated cut".to_string()));
        }
        self.action_calls.push(format!("seen:{uid}:{seen}"));
        self.modseq += 1;
        if let Some((envelope, modseq)) = self.messages.get_mut(&uid) {
            envelope.seen = seen;
            *modseq = self.modseq;
        }
        Ok(())
    }

    fn set_flagged(&mut self, _mailbox: &str, uid: Uid, flagged: bool) -> Result<(), Error> {
        if self.actions_fail {
            return Err(Error::Server("simulated cut".to_string()));
        }
        self.action_calls.push(format!("flag:{uid}:{flagged}"));
        self.modseq += 1;
        if let Some((envelope, modseq)) = self.messages.get_mut(&uid) {
            envelope.flagged = flagged;
            *modseq = self.modseq;
        }
        Ok(())
    }

    fn archive(&mut self, _mailbox: &str, uid: Uid) -> Result<(), Error> {
        if self.actions_fail {
            return Err(Error::Server("simulated cut".to_string()));
        }
        self.action_calls.push(format!("archive:{uid}"));
        self.messages.remove(&uid);
        self.modseq += 1;
        Ok(())
    }

    fn delete(&mut self, _mailbox: &str, uid: Uid) -> Result<(), Error> {
        if self.actions_fail {
            return Err(Error::Server("simulated cut".to_string()));
        }
        self.action_calls.push(format!("delete:{uid}"));
        self.messages.remove(&uid);
        self.modseq += 1;
        Ok(())
    }
}

/// The fake server has neither a sent folder nor a Drafts folder to
/// pull from — the honest default (PLAN-AUDIT-V3 E4): no test needs
/// RFC 6154 heuristics or a draft round trip faked, `run_sync` treats
/// both as absent capabilities, exactly as a bare IMAP server would.
impl CycleConnection for FakeServer {
    fn sent_folder_name(&mut self) -> Result<Option<String>, String> {
        Ok(None)
    }

    fn pull_drafts(&mut self, _store: &Store, _account_id: i64) -> Result<(), String> {
        Ok(())
    }
}

use crate::MailServer;
use std::time::{Duration, Instant};

#[derive(Clone, Copy)]
pub struct FetchLimits {
    pub bytes: u64,
    pub timeout: Duration,
}

/// Shared by callers across folders and accounts, including failed attempts.
pub struct WorkBudget {
    remaining: usize,
    scanned: usize,
    saved: usize,
    bytes: u64,
    until: Instant,
}

impl WorkBudget {
    pub fn new(candidates: usize) -> Self {
        Self::with_limits(candidates, 64 * 1024 * 1024, Duration::from_secs(120))
    }
    pub fn with_limits(candidates: usize, bytes: u64, duration: Duration) -> Self {
        Self {
            remaining: candidates,
            scanned: 0,
            saved: 0,
            bytes,
            until: Instant::now() + duration,
        }
    }
    /// Candidates still admissible. Zero once fewer bytes remain than one
    /// maximal message: a window claimed against a scope too small for its
    /// content would be scanned without being served, and only return after
    /// a full wrap.
    pub fn remaining(&self) -> usize {
        if self.bytes < crate::REMOTE_MESSAGE_BYTES || Instant::now() >= self.until {
            0
        } else {
            self.remaining
        }
    }
    pub fn scanned(&self) -> usize {
        self.scanned
    }
    pub fn saved(&self) -> usize {
        self.saved
    }
    pub fn fetch<S: MailServer, T>(
        &mut self,
        server: &mut S,
        attempts: usize,
        work: impl FnOnce(&mut S) -> Result<T, crate::Error>,
    ) -> Result<T, crate::Error> {
        if attempts == 0 || attempts > self.remaining() {
            return Err(crate::Error::Server(
                "download allowance spent; continuing on the next pass".into(),
            ));
        }
        self.spend_candidates(attempts);
        let before = self.begin_fetch(server);
        let result = work(server);
        self.end_fetch(server, before);
        result
    }
    pub(crate) fn record_saved(&mut self) {
        self.saved += 1;
    }
    pub(crate) fn spend_candidates(&mut self, count: usize) {
        self.remaining = self.remaining.saturating_sub(count);
        self.scanned = self.scanned.saturating_add(count);
    }
    pub(crate) fn begin_fetch(&self, server: &mut dyn MailServer) -> u64 {
        server.set_fetch_limits(Some(FetchLimits {
            bytes: self.bytes,
            timeout: self.until.saturating_duration_since(Instant::now()),
        }));
        server.received_bytes()
    }
    pub(crate) fn end_fetch(&mut self, server: &mut dyn MailServer, before: u64) {
        self.bytes = self
            .bytes
            .saturating_sub(server.received_bytes().saturating_sub(before));
        server.set_fetch_limits(None);
    }
}

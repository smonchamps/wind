use std::io;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Default)]
pub(crate) struct ReadBudget(Arc<Mutex<State>>);

#[derive(Default)]
struct State {
    active: Option<(usize, Instant)>,
    scope: Option<(u64, Instant)>,
    received: u64,
    poisoned: bool,
}

fn exhausted() -> io::Error {
    io::Error::other("IMAP response budget exceeded; reconnect before retrying")
}

impl ReadBudget {
    pub(crate) fn set_scope(&self, limits: Option<mail_core::FetchLimits>) {
        if let Ok(mut state) = self.0.lock() {
            state.scope = limits.map(|limits| (limits.bytes, Instant::now() + limits.timeout));
        }
    }

    /// Bytes the shared pass scope still admits, if a scope is installed.
    pub(crate) fn scope_left(&self) -> Option<u64> {
        self.0
            .lock()
            .ok()
            .and_then(|state| state.scope.map(|(left, _)| left))
    }

    pub(crate) fn received(&self) -> u64 {
        self.0
            .lock()
            .map(|state| state.received)
            .unwrap_or(u64::MAX)
    }

    pub(crate) fn begin(&self, bytes: usize, duration: Duration) -> io::Result<()> {
        let mut state = self.0.lock().map_err(|_| exhausted())?;
        if state.poisoned || state.active.is_some() {
            return Err(exhausted());
        }
        state.active = Some((bytes, Instant::now() + duration));
        Ok(())
    }

    pub(crate) fn check(&self) -> io::Result<()> {
        let mut state = self.0.lock().map_err(|_| exhausted())?;
        if state
            .active
            .is_some_and(|(_, until)| Instant::now() >= until)
        {
            state.poisoned = true;
        }
        if state.poisoned {
            return Err(exhausted());
        }
        // The pass deadline refuses NEW commands without poisoning the
        // session; a response already flowing is bounded by its own command
        // deadline above.
        if state.active.is_none()
            && state
                .scope
                .is_some_and(|(_, until)| Instant::now() >= until)
        {
            return Err(exhausted());
        }
        Ok(())
    }

    pub(crate) fn allowance(&self, requested: usize) -> io::Result<(usize, Option<Duration>)> {
        self.check()?;
        let mut state = self.0.lock().map_err(|_| exhausted())?;
        let mut allowed = requested;
        let mut deadline = None;
        if let Some((left, until)) = state.active {
            allowed = allowed.min(left);
            deadline = Some(until);
        }
        if let Some((left, _)) = state.scope {
            allowed = allowed.min(usize::try_from(left).unwrap_or(usize::MAX));
        }
        if allowed == 0 && requested != 0 {
            state.poisoned = true;
            return Err(exhausted());
        }
        Ok((
            allowed,
            deadline.map(|until| until.saturating_duration_since(Instant::now())),
        ))
    }

    pub(crate) fn consumed(&self, bytes: usize) -> io::Result<()> {
        let mut state = self.0.lock().map_err(|_| exhausted())?;
        if let Some((left, _)) = &mut state.active {
            *left = left.saturating_sub(bytes);
        }
        if let Some((left, _)) = &mut state.scope {
            *left = left.saturating_sub(bytes as u64);
        }
        state.received = state.received.saturating_add(bytes as u64);
        drop(state);
        self.check()
    }

    /// An incomplete parser response cannot be reused for another command.
    pub(crate) fn finish(&self, success: bool) -> io::Result<()> {
        let checked = self.check();
        let mut state = self.0.lock().map_err(|_| exhausted())?;
        state.poisoned |= !success;
        state.active = None;
        checked
    }
}

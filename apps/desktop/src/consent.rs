use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use mail_auth::{AuthorizationLink, ConsentControl};

use crate::commands::recovered;

#[derive(Clone, Default)]
pub(crate) struct Registry(Arc<Mutex<State>>);

#[derive(Default)]
struct State {
    next: u64,
    flight: Option<Flight>,
}

struct Flight {
    id: String,
    control: ConsentControl,
    claimed: bool,
    created: Instant,
}

pub(crate) struct Guard {
    registry: Registry,
    id: String,
    pub(crate) control: ConsentControl,
}

impl Registry {
    pub(crate) fn begin(&self) -> Result<String, String> {
        let mut state = recovered(&self.0);
        if state.flight.as_ref().is_some_and(|flight| {
            !flight.claimed && flight.created.elapsed() >= Duration::from_secs(30)
        }) {
            state.flight = None;
        }
        if state.flight.is_some() {
            return Err("finish or cancel the current authorization first".into());
        }
        state.next = state
            .next
            .checked_add(1)
            .ok_or("authorization id exhausted")?;
        let id = state.next.to_string();
        state.flight = Some(Flight {
            id: id.clone(),
            control: ConsentControl::default(),
            claimed: false,
            created: Instant::now(),
        });
        Ok(id)
    }

    pub(crate) fn claim(&self, id: &str) -> Result<Guard, String> {
        let mut state = recovered(&self.0);
        let flight = state
            .flight
            .as_mut()
            .filter(|flight| flight.id == id)
            .ok_or("authorization is no longer active")?;
        if flight.claimed
            || flight.control.is_cancelled()
            || flight.created.elapsed() >= Duration::from_secs(30)
        {
            return Err("authorization is already used or expired".into());
        }
        flight.claimed = true;
        Ok(Guard {
            registry: self.clone(),
            id: id.into(),
            control: flight.control.clone(),
        })
    }

    pub(crate) fn status(&self, id: &str) -> Result<Option<AuthorizationLink>, String> {
        let control = recovered(&self.0)
            .flight
            .as_ref()
            .filter(|flight| flight.id == id)
            .map(|flight| flight.control.clone());
        control.map_or(Ok(None), |control| {
            control.status().map_err(|err| err.to_string())
        })
    }

    pub(crate) fn cancel(&self, id: &str) -> bool {
        let mut state = recovered(&self.0);
        let Some(flight) = state.flight.as_ref().filter(|flight| flight.id == id) else {
            return false;
        };
        let accepted = flight.control.cancel();
        if accepted && !flight.claimed {
            state.flight = None;
        }
        accepted
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        let mut state = recovered(&self.registry.0);
        if state
            .flight
            .as_ref()
            .is_some_and(|flight| flight.id == self.id)
        {
            state.flight = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancelled_start_cannot_be_claimed_or_cancel_a_later_flow() {
        let registry = Registry::default();
        let old = registry.begin().unwrap();
        assert!(registry.cancel(&old));
        let new = registry.begin().unwrap();
        assert_ne!(old, new);
        assert!(registry.claim(&old).is_err());
        assert!(!registry.cancel(&old));
        assert!(!registry.claim(&new).unwrap().control.is_cancelled());
    }

    #[test]
    fn one_worker_owns_the_listener_until_its_guard_drops() {
        let registry = Registry::default();
        let id = registry.begin().unwrap();
        let guard = registry.claim(&id).unwrap();
        assert!(registry.claim(&id).is_err());
        assert!(registry.begin().is_err());
        assert!(registry.cancel(&id));
        assert!(guard.control.is_cancelled());
        assert!(registry.begin().is_err());
        drop(guard);
        assert!(registry.begin().is_ok());
    }

    #[test]
    fn abandoned_preparation_expires_without_recycling_its_id() {
        let registry = Registry::default();
        let old = registry.begin().unwrap();
        recovered(&registry.0).flight.as_mut().unwrap().created =
            Instant::now() - Duration::from_secs(31);
        assert!(registry.claim(&old).is_err());
        let new = registry.begin().unwrap();
        assert_ne!(old, new);
        assert!(registry.claim(&old).is_err());
    }
}

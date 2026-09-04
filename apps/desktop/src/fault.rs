//! The shell's typed command error (PLAN-AUDIT-V3 E3, audit 3.4).
//!
//! One type at every Tauri boundary in place of bare `String`, so the
//! 336 `.map_err(|err| err.to_string())` sites become a `?`. The WIRE
//! does not change: `CommandError` serializes as its message, and the
//! UI treats error text as opaque display data (recon E3: no error
//! substring is matched by the UI or the e2e suite — the discriminant
//! and the translation key carry the behavior).

use std::fmt;

pub struct CommandError(String);

impl CommandError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl serde::Serialize for CommandError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl From<String> for CommandError {
    fn from(message: String) -> Self {
        Self(message)
    }
}

impl From<&str> for CommandError {
    fn from(message: &str) -> Self {
        Self(message.to_string())
    }
}

impl From<CommandError> for String {
    fn from(err: CommandError) -> Self {
        err.0
    }
}

/// The concrete error types the commands actually convert today — each
/// gains a `?` in place of a `map_err`. A new source type is added HERE,
/// never by a fresh `map_err` at the call site.
macro_rules! from_display {
    ($($source:ty),+ $(,)?) => {
        $(impl From<$source> for CommandError {
            fn from(err: $source) -> Self {
                Self(err.to_string())
            }
        })+
    };
}

from_display!(
    mail_core::Error,
    mail_core::SendError,
    mail_auth::AuthError,
    std::io::Error,
    tauri::Error,
);

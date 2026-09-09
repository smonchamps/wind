//! Command errors: the prose stays the detail; every domain error also
//! carries a stable `code` and `retryable` flag across the IPC (audit lot 4,
//! E11c / A05) so the UI branches on the code, never on the wording. An
//! error built from a bare string keeps its string wire shape.

use std::fmt;

pub struct CommandError {
    message: String,
    code: Option<&'static str>,
    retryable: bool,
    remote_limit: Option<u64>,
}

impl CommandError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: None,
            retryable: false,
            remote_limit: None,
        }
    }

    pub fn code(&self) -> Option<&'static str> {
        self.code
    }

    /// A shell-born typed error (the core's errors carry their own
    /// codes through `From`): the UI branches on the code and says the
    /// catalogued sentence; the message stays the English fallback.
    pub fn with_code(message: impl Into<String>, code: &'static str) -> Self {
        Self {
            message: message.into(),
            code: Some(code),
            retryable: false,
            remote_limit: None,
        }
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl fmt::Debug for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl serde::Serialize for CommandError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(serde::Serialize)]
        struct Typed<'a> {
            code: &'a str,
            message: &'a str,
            retryable: bool,
            #[serde(skip_serializing_if = "Option::is_none")]
            limit: Option<u64>,
        }
        match self.code {
            Some(code) => Typed {
                code,
                message: &self.message,
                retryable: self.retryable,
                limit: self.remote_limit,
            }
            .serialize(serializer),
            None => serializer.serialize_str(&self.message),
        }
    }
}

impl From<String> for CommandError {
    fn from(message: String) -> Self {
        Self::new(message)
    }
}

impl From<&str> for CommandError {
    fn from(message: &str) -> Self {
        Self::new(message)
    }
}

impl From<CommandError> for String {
    fn from(err: CommandError) -> Self {
        err.message
    }
}

impl From<mail_core::Error> for CommandError {
    fn from(err: mail_core::Error) -> Self {
        let remote_limit = match &err {
            mail_core::Error::RemoteMessageTooLarge { limit, .. } => Some(*limit),
            _ => None,
        };
        // The category and the retryability are the CORE's rule
        // (`Error::code`/`retryable`, A01); the shell only serializes.
        Self {
            message: err.to_string(),
            code: Some(err.code()),
            retryable: err.retryable(),
            remote_limit,
        }
    }
}

impl From<mail_core::SendError> for CommandError {
    fn from(err: mail_core::SendError) -> Self {
        Self {
            message: err.to_string(),
            code: Some(err.code()),
            retryable: err.retryable(),
            remote_limit: None,
        }
    }
}

impl From<mail_auth::AuthError> for CommandError {
    fn from(err: mail_auth::AuthError) -> Self {
        // Partial consent is the ONE auth failure the user can fix on
        // the next screen (backlog 86, E6): its own code, so the desk
        // says the catalogued sentence — the `app_location_readonly`
        // convention (A140), never a substring match on the prose.
        let code = match &err {
            mail_auth::AuthError::MissingMailScope(_, _) => "missing_mail_scope",
            _ => "auth",
        };
        Self {
            message: err.to_string(),
            code: Some(code),
            retryable: false,
            remote_limit: None,
        }
    }
}

/// The concrete error types the commands actually convert today — each
/// gains a `?` in place of a `map_err`. A new source type is added HERE,
/// never by a fresh `map_err` at the call site.
macro_rules! from_display {
    ($($source:ty),+ $(,)?) => {
        $(impl From<$source> for CommandError {
            fn from(err: $source) -> Self {
                Self::new(err.to_string())
            }
        })+
    };
}

from_display!(std::io::Error, tauri::Error);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_size_refusal_has_a_stable_code_and_ordinary_errors_keep_their_wire_shape() {
        let error = CommandError::from(mail_core::Error::RemoteMessageTooLarge {
            uid: 7,
            limit: 33_554_432,
        });
        let json = serde_json::to_value(&error).unwrap();
        assert_eq!(json["code"], "remote_message_too_large");
        assert_eq!(json["limit"], 33_554_432_u64);
        assert_eq!(json["retryable"], false);
        assert_eq!(
            serde_json::to_value(CommandError::new("ordinary")).unwrap(),
            "ordinary"
        );
    }

    /// A05 (audit lot 4): every domain error crosses the IPC with a stable
    /// code and a retryability the UI can key on, the prose kept as detail.
    #[test]
    fn every_domain_error_carries_a_code_and_a_retryability() {
        let connection = CommandError::from(mail_core::Error::Connection(
            "imap.example:993: refused".into(),
        ));
        let json = serde_json::to_value(&connection).unwrap();
        assert_eq!(json["code"], "connection");
        assert_eq!(json["retryable"], true);
        assert_eq!(json["message"], "connection imap.example:993: refused");
        assert!(json.get("limit").is_none());
        let refusal = CommandError::from(mail_core::Error::Refusal("NO [CANNOT]".into()));
        assert_eq!(serde_json::to_value(&refusal).unwrap()["retryable"], false);
        let uncertain =
            CommandError::from(mail_core::SendError::Unknown("timeout after DATA".into()));
        assert_eq!(
            serde_json::to_value(&uncertain).unwrap()["code"],
            "send_uncertain"
        );
        assert_eq!(
            CommandError::from(mail_core::Error::Interrupted).code(),
            Some("interrupted")
        );
    }
}

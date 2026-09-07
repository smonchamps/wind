//! Command errors keep ordinary messages as strings and size refusals typed.

use std::fmt;

pub struct CommandError {
    message: String,
    remote_limit: Option<u64>,
}

impl CommandError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
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
        if let Some(limit) = self.remote_limit {
            #[derive(serde::Serialize)]
            struct SizeRefusal<'a> {
                code: &'a str,
                message: &'a str,
                limit: u64,
            }
            SizeRefusal {
                code: "remote_message_too_large",
                message: &self.message,
                limit,
            }
            .serialize(serializer)
        } else {
            serializer.serialize_str(&self.message)
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
        Self {
            message: err.to_string(),
            remote_limit,
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

from_display!(
    mail_core::SendError,
    mail_auth::AuthError,
    std::io::Error,
    tauri::Error,
);

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
        assert_eq!(
            serde_json::to_value(CommandError::new("ordinary")).unwrap(),
            "ordinary"
        );
    }
}

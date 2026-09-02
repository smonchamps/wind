//! Crash report drafting — the PURE, proven part.
//!
//! A panic hook captures a panic, but its MESSAGE may carry personal
//! data: `format!("{err:?}")` on a [`crate::Error`], whose
//! `InvalidEmailAddress` contains an address. This module turns the raw
//! data of a panic into a report that is PROVEN to contain nothing
//! personal — the message is discarded, only CODE artifacts (location,
//! symbols) and environment ones are kept.
//!
//! Pure: no I/O, no dependency. Writing the file and getting consent
//! live in the app — a panic hook must not touch the database (it may be
//! the cause of the panic, or hold a poisoned lock), nor anything that
//! could panic in turn.

/// What a panic hook has at hand, before drafting.
#[derive(Debug, Clone)]
pub struct RawPanic {
    /// The panic's message. **Leak vector**: may contain an address, a
    /// subject, a body fragment… It is DISCARDED by [`redact`], never
    /// kept.
    pub message: String,
    /// `file:line` of the panic — a position in the CODE, fixed at
    /// compile time, with no user data.
    pub location: Option<String>,
    /// The call stack: code symbols (function names), not captured
    /// values. Free of personal data by construction.
    pub backtrace: Vec<String>,
    pub app_version: String,
    pub os: String,
    pub timestamp: String,
}

/// A crash report ready to write — **without the panic's message**.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrashReport {
    pub app_version: String,
    pub os: String,
    pub location: Option<String>,
    pub backtrace: Vec<String>,
    pub timestamp: String,
}

/// Discards everything that could carry personal data.
///
/// Redaction reduces to **dropping the message**: it is the only field
/// that can hold free text coming from an error, hence potentially an
/// address or a subject. Location and stack are compile-time artifacts,
/// kept as is — they identify the bug without disclosing anything.
///
/// The implementation is trivial (one field dropped); its value lies in
/// the INVARIANT, held by a test: no data from the message survives. If
/// someone "puts the message back to help debugging" one day, the test
/// `the_report_carries_no_data_from_the_message` turns red.
pub fn redact(raw: RawPanic) -> CrashReport {
    CrashReport {
        app_version: raw.app_version,
        os: raw.os,
        location: raw.location,
        backtrace: raw.backtrace,
        timestamp: raw.timestamp,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The job's central invariant: a crash report must NEVER contain
    /// personal data. A panic's message can carry some — this test puts
    /// both an address AND a subject in it, and requires them to be gone
    /// from the report.
    #[test]
    fn the_report_carries_no_data_from_the_message() {
        let raw = RawPanic {
            message: "invalid envelope: alice.martin@example.com \
                      — subject \"Confidential Q3 invoice\""
                .to_string(),
            location: Some("crates/mail-core/src/thread.rs:42".to_string()),
            backtrace: vec![
                "mail_core::thread::attach".to_string(),
                "mail_core::store::Store::upsert_envelopes".to_string(),
            ],
            app_version: "0.1.2".to_string(),
            os: "Windows 11".to_string(),
            timestamp: "2026-07-26T15:00:00Z".to_string(),
        };

        let report = redact(raw);

        // The Debug representation, not a hand-picked list of fields: it
        // AUTOMATICALLY includes any field added one day. If someone puts
        // a `message` field back into the report, it will show up here
        // and the test will turn red — that is what gives the invariant
        // its teeth.
        let haystack = format!("{report:?}");

        assert!(
            !haystack.contains("alice.martin@example.com"),
            "the address leaked into the report"
        );
        assert!(!haystack.contains('@'), "no at-sign should survive");
        assert!(
            !haystack.contains("Confidential"),
            "the subject leaked into the report"
        );
        assert!(
            !haystack.to_lowercase().contains("confidential"),
            "the subject leaked into the report"
        );
    }

    /// But it KEEPS what helps find the bug — otherwise the report is
    /// useless. Location, stack, versions: code artifacts, safe.
    #[test]
    fn the_report_keeps_enough_to_locate_the_bug() {
        let raw = RawPanic {
            message: "does not matter".to_string(),
            location: Some("crates/mail-core/src/thread.rs:42".to_string()),
            backtrace: vec!["mail_core::thread::attach".to_string()],
            app_version: "0.1.2".to_string(),
            os: "Windows 11".to_string(),
            timestamp: "2026-07-26T15:00:00Z".to_string(),
        };

        let report = redact(raw);

        assert_eq!(
            report.location.as_deref(),
            Some("crates/mail-core/src/thread.rs:42"),
            "the location situates the bug"
        );
        assert_eq!(
            report.backtrace,
            vec!["mail_core::thread::attach".to_string()],
            "the stack is kept"
        );
        assert_eq!(report.app_version, "0.1.2");
        assert_eq!(report.os, "Windows 11");
    }
}

//! What deserves to interrupt the user — and what does not.
//!
//! A notification is an interruption. The product promises *simple*
//! ([PLAN.md](../../../docs/PLAN.md) §1): better show none than one too
//! many. The rules are therefore written here, as pure functions, rather
//! than scattered across the application layer where they would be
//! unverifiable.

use crate::envelope::Envelope;
use crate::sync::SyncMode;

/// Beyond this, we summarize instead of enumerating: a list of ten
/// senders in a system bubble is no longer readable.
const MAX_SENDERS_LISTED: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    pub title: String,
    pub body: String,
}

/// The language of the notification texts (PLAN-LANGUES, E2). The shell
/// reads `prefs.lang` and passes it HERE, as a parameter — no global: the
/// functions stay pure, testable per language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    #[default]
    Fr,
    En,
}

impl Lang {
    /// The code set by the UI (`prefs.lang`). Absent or unknown = French
    /// — the same fallback as the interface.
    pub fn from_pref(code: Option<&str>) -> Self {
        match code {
            Some("en") => Self::En,
            _ => Self::Fr,
        }
    }
}

/// The arrivals of a sync that may give rise to a notification.
///
/// An **initial** sync gives none: it fetches the whole mailbox.
/// Notifying there would announce as "new" mail three years old,
/// thousands of times over.
pub fn arrivals_to_notify(mode: SyncMode, new_unread: Vec<Envelope>) -> Vec<Envelope> {
    match mode {
        SyncMode::Initial => Vec::new(),
        SyncMode::Incremental => new_unread,
    }
}

/// The notification to display for a batch of arrivals, across all
/// accounts — **a single one**, never one per message.
///
/// Three messages arriving together produce one bubble, not three:
/// stacked bubbles are the flaw that gets notifications turned off.
pub fn notification_for(arrivals: &[Envelope], lang: Lang) -> Option<Notification> {
    match arrivals {
        [] => None,
        [single] => Some(Notification {
            title: sender_of(single, lang),
            body: subject_of(single, lang),
        }),
        many => Some(Notification {
            title: match lang {
                Lang::Fr => format!("{} nouveaux messages", many.len()), // lang:fr
                Lang::En => format!("{} new messages", many.len()),
            },
            body: summarize_senders(many, lang),
        }),
    }
}

/// Distinct senders, in arrival order, cut off at
/// [`MAX_SENDERS_LISTED`].
fn summarize_senders(arrivals: &[Envelope], lang: Lang) -> String {
    let mut seen: Vec<String> = Vec::new();
    for arrival in arrivals {
        let sender = sender_of(arrival, lang);
        if !seen.contains(&sender) {
            seen.push(sender);
        }
        if seen.len() > MAX_SENDERS_LISTED {
            break;
        }
    }
    if seen.len() > MAX_SENDERS_LISTED {
        let listed = seen[..MAX_SENDERS_LISTED].join(", ");
        format!("{listed}…")
    } else {
        seen.join(", ")
    }
}

/// A missing sender must not produce an empty bubble: the fallback is
/// explicit, as in the list.
fn sender_of(envelope: &Envelope, lang: Lang) -> String {
    envelope
        .sender
        .clone()
        .or_else(|| envelope.sender_address.clone())
        .unwrap_or_else(|| {
            match lang {
                Lang::Fr => "(expéditeur inconnu)", // lang:fr
                Lang::En => "(unknown sender)",
            }
            .to_string()
        })
}

fn subject_of(envelope: &Envelope, lang: Lang) -> String {
    envelope.subject.clone().unwrap_or_else(|| {
        match lang {
            Lang::Fr => "(sans sujet)", // lang:fr
            Lang::En => "(no subject)",
        }
        .to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arrival(uid: u32, sender: Option<&str>, subject: Option<&str>) -> Envelope {
        Envelope {
            reply_to: None,
            uid,
            subject: subject.map(str::to_string),
            sender: sender.map(str::to_string),
            sender_address: None,
            message_id: None,
            in_reply_to: None,
            date: None,
            seen: false,
            flagged: false,
            to_addrs: Vec::new(),
            cc_addrs: Vec::new(),
        }
    }

    /// The rule that matters most. A first sync fetches the whole
    /// mailbox: notifying there would announce as "new" thousands of
    /// messages years old. This is the flaw that gets notifications
    /// disabled for good.
    #[test]
    fn an_initial_sync_never_notifies() {
        let arrivals = vec![arrival(1, Some("Alice"), Some("Bonjour"))]; // lang:fr
        assert!(arrivals_to_notify(SyncMode::Initial, arrivals).is_empty());
    }

    #[test]
    fn an_incremental_sync_keeps_its_arrivals() {
        let arrivals = vec![arrival(1, Some("Alice"), Some("Bonjour"))]; // lang:fr
        assert_eq!(arrivals_to_notify(SyncMode::Incremental, arrivals).len(), 1);
    }

    #[test]
    fn nothing_new_shows_nothing() {
        assert_eq!(notification_for(&[], Lang::Fr), None);
    }

    #[test]
    fn a_single_message_shows_its_sender_and_subject() {
        let notification =
            notification_for(&[arrival(1, Some("Alice"), Some("Facture mars"))], Lang::Fr) // lang:fr
                .unwrap();
        assert_eq!(notification.title, "Alice");
        assert_eq!(notification.body, "Facture mars"); // lang:fr
    }

    /// Three messages arrived together make ONE bubble, not three. The
    /// stacking is what pushes people to turn notifications off.
    #[test]
    fn several_messages_are_summarized_in_a_single_notification() {
        let notification = notification_for(
            &[
                arrival(1, Some("Alice"), Some("a")),
                arrival(2, Some("Bob"), Some("b")),
            ],
            Lang::Fr,
        )
        .unwrap();
        assert_eq!(notification.title, "2 nouveaux messages"); // lang:fr
        assert_eq!(notification.body, "Alice, Bob");
    }

    /// The same sender writing three times appears only once: repeating
    /// their name would waste the only available line.
    #[test]
    fn a_repeated_sender_is_listed_once() {
        let notification = notification_for(
            &[
                arrival(1, Some("Alice"), Some("a")),
                arrival(2, Some("Alice"), Some("b")),
                arrival(3, Some("Bob"), Some("c")),
            ],
            Lang::Fr,
        )
        .unwrap();
        assert_eq!(notification.title, "3 nouveaux messages"); // lang:fr
        assert_eq!(notification.body, "Alice, Bob");
    }

    #[test]
    fn beyond_three_senders_the_list_is_cut() {
        let notification = notification_for(
            &[
                arrival(1, Some("Alice"), None),
                arrival(2, Some("Bob"), None),
                arrival(3, Some("Carole"), None),
                arrival(4, Some("David"), None),
            ],
            Lang::Fr,
        )
        .unwrap();
        assert_eq!(notification.title, "4 nouveaux messages"); // lang:fr
        assert_eq!(notification.body, "Alice, Bob, Carole…");
    }

    /// An empty bubble would be worse than no bubble at all.
    #[test]
    fn a_message_without_sender_or_subject_still_reads() {
        let notification = notification_for(&[arrival(1, None, None)], Lang::Fr).unwrap();
        assert_eq!(notification.title, "(expéditeur inconnu)"); // lang:fr
        assert_eq!(notification.body, "(sans sujet)"); // lang:fr
    }

    /// The English transposition (PLAN-LANGUES, E2): the same rules, the
    /// texts of the language set — fallbacks included.
    #[test]
    fn english_notifications_carry_english_texts() {
        let notification = notification_for(
            &[
                arrival(1, Some("Alice"), Some("a")),
                arrival(2, Some("Bob"), Some("b")),
            ],
            Lang::En,
        )
        .unwrap();
        assert_eq!(notification.title, "2 new messages");
        assert_eq!(notification.body, "Alice, Bob");

        let fallback = notification_for(&[arrival(1, None, None)], Lang::En).unwrap();
        assert_eq!(fallback.title, "(unknown sender)");
        assert_eq!(fallback.body, "(no subject)");
    }

    /// `prefs.lang` absent or unknown = French — the UI's fallback.
    #[test]
    fn the_lang_pref_falls_back_to_french() {
        assert_eq!(Lang::from_pref(None), Lang::Fr);
        assert_eq!(Lang::from_pref(Some("fr")), Lang::Fr);
        assert_eq!(Lang::from_pref(Some("en")), Lang::En);
        assert_eq!(Lang::from_pref(Some("de")), Lang::Fr);
    }
}

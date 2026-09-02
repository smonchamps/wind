//! The pending user actions — the product's second core.
//!
//! Each intention is applied locally right away (UI optimism), journaled
//! in SQLite, then replayed to the server **at the head of the next
//! synchronization**: a network cut or a crash loses none of them, that
//! is the gate of Phase 2 (PLAN.md §4).

use crate::envelope::Uid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    MarkSeen,
    MarkUnseen,
    /// Set the star (`\Flagged`).
    MarkFlagged,
    /// Remove the star.
    MarkUnflagged,
    /// Take out of the mailbox without deleting (with Gmail: stays in
    /// "All Mail").
    Archive,
    /// Put in the server's trash.
    Delete,
    /// Move to a folder chosen by the user.
    ///
    /// Carries the folder's **NETWORK** name (modified UTF-7), never its
    /// readable form: this is the name that will be sent back to the
    /// server, and a journaled action can be replayed days later.
    MoveTo(String),
}

/// Prefix of actions carrying a destination. The name follows the first
/// `:` — everything that remains is part of it, including other `:`,
/// which folder names allow.
const MOVE_PREFIX: &str = "move_to:";

impl Action {
    pub(crate) fn to_kind(&self) -> String {
        match self {
            Action::MarkSeen => "mark_seen".to_string(),
            Action::MarkUnseen => "mark_unseen".to_string(),
            Action::MarkFlagged => "mark_flagged".to_string(),
            Action::MarkUnflagged => "mark_unflagged".to_string(),
            Action::Archive => "archive".to_string(),
            Action::Delete => "delete".to_string(),
            Action::MoveTo(folder) => format!("{MOVE_PREFIX}{folder}"),
        }
    }

    pub(crate) fn parse(kind: &str) -> Option<Self> {
        match kind {
            "mark_seen" => Some(Action::MarkSeen),
            "mark_unseen" => Some(Action::MarkUnseen),
            "mark_flagged" => Some(Action::MarkFlagged),
            "mark_unflagged" => Some(Action::MarkUnflagged),
            "archive" => Some(Action::Archive),
            "delete" => Some(Action::Delete),
            other => other
                .strip_prefix(MOVE_PREFIX)
                // An empty destination is not replayable: better to
                // ignore the action than move to nowhere.
                .filter(|folder| !folder.is_empty())
                .map(|folder| Action::MoveTo(folder.to_string())),
        }
    }

    /// Does the action make the message DISAPPEAR from the current mailbox?
    ///
    /// What disappears locally must disappear server-side, and vice versa
    /// — the three cases share the same handling in the list as in replay.
    pub fn removes_from_mailbox(&self) -> bool {
        matches!(self, Action::Archive | Action::Delete | Action::MoveTo(_))
    }
}

/// A journaled action, in emission order (increasing id).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingAction {
    pub id: i64,
    pub uid: Uid,
    pub action: Action,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn every_action() -> Vec<Action> {
        vec![
            Action::MarkSeen,
            Action::MarkUnseen,
            Action::MarkFlagged,
            Action::MarkUnflagged,
            Action::Archive,
            Action::Delete,
            Action::MoveTo("Archive".to_string()),
        ]
    }

    /// The invariant that matters most. A journaled action can be replayed
    /// days later, by a different build of the binary. The round trip is
    /// therefore this module's central invariant: losing it means losing
    /// user intentions already confirmed on screen.
    #[test]
    fn every_action_survives_a_round_trip_through_storage() {
        for action in every_action() {
            let kind = action.to_kind();
            assert_eq!(
                Action::parse(&kind).as_ref(),
                Some(&action),
                "round trip broken for {action:?} (encoded \"{kind}\")"
            );
        }
    }

    /// Folder names accept `:` — and IMAP's hierarchy separator is often
    /// `/` or `.`, but nothing mandates it. Splitting on the LAST `:`
    /// would break these names.
    #[test]
    fn a_destination_containing_a_colon_is_preserved() {
        let action = Action::MoveTo("Projets:2026/Clients".to_string());
        let kind = action.to_kind();
        assert_eq!(Action::parse(&kind), Some(action));
    }

    /// The NETWORK name travels as is: re-encoding or decoding it here
    /// would make replay fail on an accented folder.
    #[test]
    fn an_encoded_folder_name_is_journaled_verbatim() {
        let wire = "Archiv&AOk-s";
        let kind = Action::MoveTo(wire.to_string()).to_kind();
        assert_eq!(kind, "move_to:Archiv&AOk-s");
        assert_eq!(Action::parse(&kind), Some(Action::MoveTo(wire.to_string())));
    }

    #[test]
    fn an_unknown_or_incomplete_kind_is_ignored() {
        assert_eq!(Action::parse("teleporter"), None);
        assert_eq!(Action::parse(""), None);
        assert_eq!(
            Action::parse("move_to:"),
            None,
            "moving to nowhere is not replayable"
        );
    }

    /// Only these three take the message out of the mailbox: this is what
    /// decides optimistic disappearance in the list.
    #[test]
    fn only_removing_actions_take_the_message_out_of_the_mailbox() {
        assert!(Action::Archive.removes_from_mailbox());
        assert!(Action::Delete.removes_from_mailbox());
        assert!(Action::MoveTo("Factures".to_string()).removes_from_mailbox());

        assert!(!Action::MarkSeen.removes_from_mailbox());
        assert!(!Action::MarkFlagged.removes_from_mailbox());
    }
}

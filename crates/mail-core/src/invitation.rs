//! A message's meeting invitation — the cache's `invitations` row.
//!
//! The bridge between `mail-ical` (the pure parser) and storage: a
//! `text/calendar` part becomes a `StoredInvitation` ready to write, at
//! the moment the MIME passes under the engine's eyes
//! (`save_body_full`), or on opening for a message predating the
//! feature (adoption, invariant §6.7 — re-fetch on demand then
//! write-back, never a mass migration).
//!
//! The stored values are STABLE strings (`request`, `accepte`…),
//! decoupled from `mail-ical`'s enums: the database survives the
//! parser's overhauls.

use mail_ical::{Invitation, Method, Participation, When};

/// The `invitations` row: ONE message's invitation, ready to display.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InvitationRow {
    pub metadata_version: u32,
    pub dtstamp_epoch: Option<i64>,
    /// Empty identifies the series; NULL means identity has not been recovered.
    pub occurrence_key: Option<String>,
    pub occurrence_property: Option<String>,
    pub scheduling_supported: bool,
    pub scheduling_state: String,
    /// `request` | `cancel` | `reply`.
    pub method: String,
    /// The iCalendar UID of the meeting (shared by REQUEST/CANCEL/REPLY).
    pub event_uid: String,
    pub sequence: i64,
    pub title: String,
    pub location: Option<String>,
    pub organizer_address: Option<String>,
    pub organizer_name: Option<String>,
    /// Start resolved to UTC — `None` if all-day or unresolved TZID.
    pub start_epoch: Option<i64>,
    pub end_epoch: Option<i64>,
    /// The TEXT form when the epoch is missing: `YYYY-MM-DD` (all-day)
    /// or `YYYY-MM-DDTHH:MM` (floating time, displayed as is — guard D1,
    /// never a misleading conversion).
    pub start_text: Option<String>,
    pub end_text: Option<String>,
    pub all_day: bool,
    pub recurrent: bool,
    /// OUR status read from the REQUEST: `sans_reponse` | `accepte` |
    /// `provisoire` | `refuse`. `None`: we are not invited.
    pub partstat: Option<String>,
    /// The attendee of a received REPLY (we are the organizer).
    pub attendee_address: Option<String>,
    pub attendee_name: Option<String>,
    pub attendee_status: Option<String>,
    /// Derived from the known versions of this organizer's meeting and occurrence.
    pub cancelled: bool,
}

/// A stored invitation, reread with OUR local reply (D6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredInvitation {
    pub row: InvitationRow,
    pub revision: i64,
    /// `accepte` | `provisoire` | `refuse` — the last reply sent through
    /// the outbox. `None`: not yet answered from Wind.
    pub reply: Option<String>,
    pub reply_epoch: Option<i64>,
}

/// Snapshot selected by the user, including the mailbox namespace.
pub struct InvitationReplyTarget<'a> {
    pub identity: &'a crate::MailboxIdentity,
    pub uid: crate::envelope::Uid,
    pub revision: i64,
}

/// Pulls the invitation row from a `text/calendar` part.
///
/// `None` if the text is not a readable iTIP message — an export `.ics`
/// without METHOD is NOT an invitation, it stays a plain attachment.
pub fn extract_invitation(ics: &str, our_address: &str) -> Option<InvitationRow> {
    let invitation = mail_ical::parse(ics, our_address).ok()?;
    Some(row_from(invitation))
}

fn row_from(invitation: Invitation) -> InvitationRow {
    let (start_epoch, start_text, all_day) = decompose(invitation.start);
    let (end_epoch, end_text, _) = decompose(invitation.end);
    InvitationRow {
        metadata_version: 1,
        dtstamp_epoch: invitation.scheduling.dtstamp_epoch,
        occurrence_key: invitation.scheduling.supported.then(|| {
            invitation
                .scheduling
                .recurrence
                .as_ref()
                .map(|r| r.key().to_string())
                .unwrap_or_default()
        }),
        occurrence_property: invitation
            .scheduling
            .recurrence
            .as_ref()
            .map(|r| r.property().to_string()),
        scheduling_supported: invitation.scheduling.supported,
        scheduling_state: if !invitation.scheduling.supported {
            "unsupported"
        } else if invitation.method == Method::Cancel {
            "cancelled"
        } else {
            "active"
        }
        .into(),
        method: method_stable(invitation.method).to_string(),
        event_uid: invitation.uid,
        sequence: invitation.sequence,
        title: invitation.title,
        location: invitation.location,
        organizer_address: invitation.organizer.as_ref().map(|o| o.address.clone()),
        organizer_name: invitation.organizer.and_then(|o| o.name),
        start_epoch,
        end_epoch,
        start_text,
        end_text,
        all_day,
        recurrent: invitation.recurrent,
        partstat: invitation
            .our_participation
            .map(|p| status_stable(p).to_string()),
        attendee_address: invitation.attendee.as_ref().map(|r| r.address.clone()),
        attendee_name: invitation.attendee.and_then(|r| r.name),
        attendee_status: invitation
            .attendee_participation
            .map(|p| status_stable(p).to_string()),
        // A CANCEL is cancelled by nature; cross-setting it onto the
        // REQUEST of the same meeting belongs to storage.
        cancelled: matches!(invitation.method, Method::Cancel),
    }
}

impl InvitationRow {
    pub fn can_reply(&self) -> bool {
        self.method == "request"
            && self.metadata_version == 1
            && self.scheduling_supported
            && self.occurrence_key.is_some()
            && !self.cancelled
            && self.scheduling_state == "active"
            && self
                .organizer_address
                .as_ref()
                .is_some_and(|address| !address.trim().is_empty())
    }
}

// Peers are scoped to one account by storage. Arrival order is never a version.
pub(crate) fn scheduling_state<'a>(
    current: &InvitationRow,
    peers: impl Iterator<Item = &'a InvitationRow>,
) -> &'static str {
    if current.metadata_version == 0 {
        return "unverified";
    }
    if !current.scheduling_supported {
        return "unsupported";
    }
    if current.method == "cancel" {
        return "cancelled";
    }
    if current.method != "request" {
        return "active";
    }
    let Some(organizer) = current
        .organizer_address
        .as_deref()
        .filter(|s| !s.trim().is_empty())
    else {
        return "unverified";
    };
    let relevant: Vec<_> = peers
        .filter(|peer| {
            peer.event_uid == current.event_uid
                && peer
                    .organizer_address
                    .as_deref()
                    .is_some_and(|s| s.trim().eq_ignore_ascii_case(organizer.trim()))
                && matches!(peer.method.as_str(), "request" | "cancel")
                && (peer.occurrence_key == current.occurrence_key
                    || peer.occurrence_key.is_none()
                    || (peer.method == "cancel" && peer.occurrence_key.as_deref() == Some("")))
        })
        .collect();
    let sequence = relevant
        .iter()
        .map(|r| r.sequence)
        .max()
        .unwrap_or(current.sequence);
    let latest: Vec<_> = relevant
        .into_iter()
        .filter(|r| r.sequence == sequence)
        .collect();
    if latest
        .iter()
        .any(|r| r.metadata_version == 0 || !r.scheduling_supported)
    {
        return "unverified";
    }
    let missing_stamp = latest.iter().any(|r| r.dtstamp_epoch.is_none());
    let stamp = latest.iter().filter_map(|r| r.dtstamp_epoch).max();
    let candidates: Vec<_> = latest
        .into_iter()
        .filter(|r| missing_stamp || r.dtstamp_epoch == stamp)
        .collect();
    let Some(winner) = candidates.first() else {
        return "active";
    };
    if candidates
        .iter()
        .any(|r| !same_scheduling_content(winner, r))
    {
        return "unverified";
    }
    if winner.method == "cancel" {
        return "cancelled";
    }
    if sequence > current.sequence || (!missing_stamp && stamp > current.dtstamp_epoch) {
        return "superseded";
    }
    "active"
}

fn same_scheduling_content(a: &InvitationRow, b: &InvitationRow) -> bool {
    a.method == b.method
        && a.occurrence_key == b.occurrence_key
        && a.title == b.title
        && a.location == b.location
        && a.start_epoch == b.start_epoch
        && a.end_epoch == b.end_epoch
        && a.start_text == b.start_text
        && a.end_text == b.end_text
        && a.all_day == b.all_day
        && a.recurrent == b.recurrent
}

#[cfg(test)]
mod scheduling_tests {
    use super::*;

    fn row(method: &str, sequence: i64, stamp: Option<i64>, occurrence: &str) -> InvitationRow {
        InvitationRow {
            metadata_version: 1,
            scheduling_supported: true,
            occurrence_key: Some(occurrence.into()),
            method: method.into(),
            event_uid: "series".into(),
            sequence,
            dtstamp_epoch: stamp,
            organizer_address: Some("owner@example.fr".into()),
            title: "Meeting".into(),
            ..Default::default()
        }
    }

    #[test]
    fn cancellation_and_replacement_follow_version_not_arrival_order() {
        let rows = [
            row("request", 0, Some(1), ""),
            row("cancel", 1, Some(2), ""),
            row("request", 2, Some(3), ""),
        ];
        for order in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            let peers = order.map(|i| &rows[i]);
            assert_eq!(
                scheduling_state(&rows[0], peers.iter().copied()),
                "superseded"
            );
            assert_eq!(scheduling_state(&rows[2], peers.iter().copied()), "active");
        }
        assert_eq!(scheduling_state(&rows[0], rows[..2].iter()), "cancelled");
    }

    #[test]
    fn organizer_and_occurrence_isolate_updates_but_series_cancel_covers_occurrences() {
        let request = row("request", 0, Some(1), "utc:10");
        let mut cancel = row("cancel", 1, Some(2), "utc:20");
        assert_eq!(
            scheduling_state(&request, [&request, &cancel].into_iter()),
            "active"
        );
        cancel.occurrence_key = Some(String::new());
        assert_eq!(
            scheduling_state(&request, [&request, &cancel].into_iter()),
            "cancelled"
        );
        cancel.organizer_address = Some("other@example.fr".into());
        assert_eq!(
            scheduling_state(&request, [&request, &cancel].into_iter()),
            "active"
        );
    }

    #[test]
    fn equal_sequence_uses_timestamp_and_ambiguous_conflicts_cannot_be_answered() {
        let request = row("request", 2, Some(10), "");
        let mut cancel = row("cancel", 2, Some(9), "");
        assert_eq!(
            scheduling_state(&request, [&request, &cancel].into_iter()),
            "active"
        );
        cancel.dtstamp_epoch = Some(11);
        assert_eq!(
            scheduling_state(&request, [&request, &cancel].into_iter()),
            "cancelled"
        );
        for stamp in [None, Some(10)] {
            cancel.dtstamp_epoch = stamp;
            assert_eq!(
                scheduling_state(&request, [&request, &cancel].into_iter()),
                "unverified"
            );
        }
        let mut changed = request.clone();
        changed.title = "Different meeting".into();
        assert_eq!(
            scheduling_state(&request, [&request, &changed].into_iter()),
            "unverified"
        );
        assert_eq!(
            scheduling_state(&request, [&request, &request].into_iter()),
            "active"
        );
    }
}

fn decompose(when: Option<When>) -> (Option<i64>, Option<String>, bool) {
    match when {
        Some(When::Instant(epoch)) => (Some(epoch), None, false),
        Some(When::Day(day)) => (None, Some(day), true),
        Some(When::Floating(text)) => (None, Some(text), false),
        None => (None, None, false),
    }
}

fn method_stable(methode: Method) -> &'static str {
    match methode {
        Method::Request => "request",
        Method::Cancel => "cancel",
        Method::Reply => "reply",
    }
}

fn status_stable(participation: Participation) -> &'static str {
    match participation {
        Participation::NeedsAction => "sans_reponse",
        Participation::Accepted => "accepte",
        Participation::Tentative => "provisoire",
        Participation::Declined => "refuse",
    }
}

/// The `Participation` for a stable string from the database — to build
/// the iTIP reply from the stored row.
pub fn participation_de_stable(stable: &str) -> Option<Participation> {
    match stable {
        "accepte" => Some(Participation::Accepted),
        "provisoire" => Some(Participation::Tentative),
        "refuse" => Some(Participation::Declined),
        "sans_reponse" => Some(Participation::NeedsAction),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REQUEST: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nMETHOD:REQUEST\r\n\
        BEGIN:VEVENT\r\nUID:reunion-1@exemple.fr\r\nSEQUENCE:2\r\n\
        SUMMARY:Point projet\r\nLOCATION:Salle A\r\n\
        DTSTART:20260903T123000Z\r\nDTEND:20260903T130000Z\r\n\
        ORGANIZER;CN=Claire Martin:mailto:claire@exemple.fr\r\n\
        ATTENDEE;PARTSTAT=NEEDS-ACTION:mailto:nous@wind.example\r\n\
        END:VEVENT\r\nEND:VCALENDAR\r\n";

    #[test]
    fn a_request_becomes_a_complete_row() {
        let row = extract_invitation(REQUEST, "nous@wind.example").expect("invitation");
        assert_eq!(row.method, "request");
        assert_eq!(row.event_uid, "reunion-1@exemple.fr");
        assert_eq!(row.sequence, 2);
        assert_eq!(row.title, "Point projet"); // lang:fr
        assert_eq!(row.location.as_deref(), Some("Salle A")); // lang:fr
        assert_eq!(row.organizer_address.as_deref(), Some("claire@exemple.fr"));
        assert_eq!(row.organizer_name.as_deref(), Some("Claire Martin"));
        assert!(row.start_epoch.is_some());
        assert!(row.end_epoch.is_some());
        assert!(!row.all_day);
        assert_eq!(row.partstat.as_deref(), Some("sans_reponse"));
    }

    #[test]
    fn an_all_day_event_carries_the_text_not_the_epoch() {
        let ics = REQUEST
            .replace("DTSTART:20260903T123000Z", "DTSTART;VALUE=DATE:20260907")
            .replace("DTEND:20260903T130000Z", "DTEND;VALUE=DATE:20260908");
        let row = extract_invitation(&ics, "nous@wind.example").expect("invitation");
        assert_eq!(row.start_epoch, None);
        assert_eq!(row.start_text.as_deref(), Some("2026-09-07"));
        assert!(row.all_day);
    }

    #[test]
    fn an_export_without_method_is_not_an_invitation() {
        let ics = REQUEST.replace("METHOD:REQUEST\r\n", "");
        assert_eq!(extract_invitation(&ics, "nous@wind.example"), None);
    }

    #[test]
    fn a_received_reply_carries_the_attendee() {
        let ics = REQUEST.replace("METHOD:REQUEST", "METHOD:REPLY").replace(
            "ATTENDEE;PARTSTAT=NEEDS-ACTION:mailto:nous@wind.example",
            "ATTENDEE;PARTSTAT=ACCEPTED;CN=Paul Durand:mailto:paul@contoso.com",
        );
        let row = extract_invitation(&ics, "nous@wind.example").expect("invitation");
        assert_eq!(row.method, "reply");
        assert_eq!(row.attendee_address.as_deref(), Some("paul@contoso.com"));
        assert_eq!(row.attendee_name.as_deref(), Some("Paul Durand"));
        assert_eq!(row.attendee_status.as_deref(), Some("accepte"));
        // We are not in the list: no partstat for us.
        assert_eq!(row.partstat, None);
    }

    #[test]
    fn the_stable_strings_round_trip() {
        for (stable, participation) in [
            ("accepte", Participation::Accepted),
            ("provisoire", Participation::Tentative),
            ("refuse", Participation::Declined),
            ("sans_reponse", Participation::NeedsAction),
        ] {
            assert_eq!(participation_de_stable(stable), Some(participation));
        }
        assert_eq!(participation_de_stable("autre"), None);
    }
}

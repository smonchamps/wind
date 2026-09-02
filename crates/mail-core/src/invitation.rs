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
    /// The meeting is cancelled (field finding R6): true on a CANCEL,
    /// and CROSS-SET on the REQUEST of the same meeting (same
    /// `event_uid`, same account) — set by storage on write, whatever
    /// order the scans arrive in. A cancelled invitation no longer
    /// offers a reply.
    pub cancelled: bool,
}

/// A stored invitation, reread with OUR local reply (D6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredInvitation {
    pub row: InvitationRow,
    /// `accepte` | `provisoire` | `refuse` — the last reply sent through
    /// the outbox. `None`: not yet answered from Wind.
    pub reply: Option<String>,
    pub reply_epoch: Option<i64>,
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

//! The corpus of the PLAN-INVITATIONS spikes, committed as tests: six
//! realistic invitations (Google/IANA, Outlook/Windows TZID, bare UTC,
//! whole day, CANCEL, recurrence) + the generation trial of METHOD:REPLY.
//! The reference truth is the spikes' ATTENDU.md (2026-08-22), replayed
//! here at every gate.

use mail_ical::{
    IcalError, Invitation, Method, Participation, ReplyRequest, When, itip_reply, parse,
};

const US: &str = "nous@wind.example";

fn load(name: &str) -> String {
    let raw = match name {
        "google" => include_str!("fixtures/google-request.ics"),
        "outlook" => include_str!("fixtures/outlook-request.ics"),
        "utc" => include_str!("fixtures/utc-request.ics"),
        "allday" => include_str!("fixtures/allday-request.ics"),
        "cancel" => include_str!("fixtures/cancel.ics"),
        "recurrence" => include_str!("fixtures/recurrence-request.ics"),
        other => panic!("unknown fixture: {other}"),
    };
    // The repository's fixtures are LF; the real wire is CRLF.
    raw.replace("\r\n", "\n").replace('\n', "\r\n")
}

fn epoch(s: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(s)
        .unwrap_or_else(|e| panic!("invalid reference date {s}: {e}"))
        .timestamp()
}

#[test]
fn google_request_iana_tzid_summer() {
    let inv = parse(&load("google"), US).unwrap();
    assert_eq!(inv.method, Method::Request);
    assert_eq!(inv.uid, "7f3e9a2b1c4d5e6f@google.com");
    assert_eq!(inv.sequence, 0);
    assert_eq!(inv.title, "Revue budgétaire T3 — comité de pilotage"); // lang:fr fixture
    // UNESCAPED comma of LOCATION.
    assert_eq!(inv.location.as_deref(), Some("Salle Vosges, 3e étage")); // lang:fr fixture
    let org = inv.organizer.expect("organizer");
    assert_eq!(org.address, "claire.martin@exemple.fr");
    assert_eq!(org.name.as_deref(), Some("Claire Martin"));
    // Europe/Paris in summer: 14:30 local = 12:30Z.
    assert_eq!(
        inv.start,
        Some(When::Instant(epoch("2026-09-03T12:30:00Z")))
    );
    assert_eq!(inv.end, Some(When::Instant(epoch("2026-09-03T13:30:00Z"))));
    assert!(!inv.recurrent);
    // ATTENDEE line FOLDED in the middle of a parameter (RSVP=\r\n TRUE).
    assert_eq!(inv.our_participation, Some(Participation::NeedsAction));
}

#[test]
fn outlook_request_windows_tzid_winter() {
    let inv = parse(&load("outlook"), US).unwrap();
    assert_eq!(inv.method, Method::Request);
    assert_eq!(
        inv.uid,
        "040000008200E00074C5B7101A82E00800000000B0C3D4E5F6A7B8C9"
    );
    // LANGUAGE parameter ignored without breaking the value.
    assert_eq!(inv.title, "Entretien annuel"); // lang:fr fixture
    assert_eq!(inv.location.as_deref(), Some("Bureau 204"));
    let org = inv.organizer.expect("organizer");
    assert_eq!(org.address, "paul.durand@contoso.com");
    // "Romance Standard Time" (Windows TZID) in winter: 09:00 = 08:00Z.
    assert_eq!(
        inv.start,
        Some(When::Instant(epoch("2026-12-10T08:00:00Z")))
    );
    assert_eq!(inv.end, Some(When::Instant(epoch("2026-12-10T08:30:00Z"))));
    assert_eq!(inv.our_participation, Some(Participation::NeedsAction));
}

#[test]
fn utc_request_without_tzid() {
    let inv = parse(&load("utc"), US).unwrap();
    assert_eq!(inv.title, "Appel fournisseur"); // lang:fr fixture
    assert_eq!(inv.location, None);
    assert_eq!(
        inv.start,
        Some(When::Instant(epoch("2026-09-03T12:30:00Z")))
    );
    assert_eq!(inv.end, Some(When::Instant(epoch("2026-09-03T13:00:00Z"))));
}

#[test]
fn all_day_value_date() {
    let inv = parse(&load("allday"), US).unwrap();
    assert_eq!(inv.title, "Séminaire d'équipe"); // lang:fr fixture
    assert_eq!(inv.start, Some(When::Day("2026-09-07".into())));
    // Exclusive DTEND, returned as is — the UI decides the display.
    assert_eq!(inv.end, Some(When::Day("2026-09-08".into())));
}

#[test]
fn cancel_same_uid_sequence_1() {
    let inv = parse(&load("cancel"), US).unwrap();
    assert_eq!(inv.method, Method::Cancel);
    // Same UID as google-request: it is the same meeting.
    assert_eq!(inv.uid, "7f3e9a2b1c4d5e6f@google.com");
    assert_eq!(inv.sequence, 1);
}

#[test]
fn recurrence_presence_of_the_rrule() {
    let inv = parse(&load("recurrence"), US).unwrap();
    assert!(inv.recurrent);
    // First occurrence: Tuesday Sept. 8, 10:00 Paris = 08:00Z.
    assert_eq!(
        inv.start,
        Some(When::Instant(epoch("2026-09-08T08:00:00Z")))
    );
}

#[test]
fn unknown_tzid_renders_floating_never_a_wrong_time() {
    // The trap measured at the spike: a TZID outside the tables would fall
    // back to "floating treated as UTC" = SILENT one-hour offset. The D1
    // guard: we return When::Floating, the UI says "organizer's local time".
    let ics = load("outlook").replace("Romance Standard Time", "Zone Perso Wind");
    let inv = parse(&ics, US).unwrap();
    assert_eq!(inv.start, Some(When::Floating("2026-12-10T09:00".into())));
    assert_eq!(inv.end, Some(When::Floating("2026-12-10T09:30".into())));
}

#[test]
fn our_address_compares_case_insensitively() {
    let ics = load("utc").replace("mailto:nous@wind.example", "mailto:NOUS@Wind.Example");
    let inv = parse(&ics, US).unwrap();
    assert_eq!(inv.our_participation, Some(Participation::NeedsAction));
}

#[test]
fn a_received_reply_gives_the_attendee() {
    // We are the organizer: a REPLY arrives with the attendee's PARTSTAT
    // (D2 — the "X accepted" state).
    let request = ReplyRequest {
        uid: "7f3e9a2b1c4d5e6f@google.com",
        sequence: 0,
        organizer_address: US,
        our_address: "claire.martin@exemple.fr",
        participation: Participation::Accepted,
        dtstamp_epoch: epoch("2026-08-22T12:00:00Z"),
    };
    let reply = itip_reply(&request);
    let inv = parse(&reply, US).unwrap();
    assert_eq!(inv.method, Method::Reply);
    let attendee = inv.attendee.expect("attendee");
    assert_eq!(attendee.address, "claire.martin@exemple.fr");
    assert_eq!(inv.attendee_participation, Some(Participation::Accepted));
}

#[test]
fn the_attendee_of_a_reply_is_not_the_echoed_organizer() {
    // Exchange sometimes ECHOES the organizer at the head of the ATTENDEE
    // list of a REPLY (review): the attendee is the first ATTENDEE who is
    // NOT them — never "we replied" in our place.
    let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nMETHOD:REPLY\r\n\
        BEGIN:VEVENT\r\nUID:r1@exemple.fr\r\n\
        ORGANIZER;CN=Nous:mailto:nous@wind.example\r\n\
        ATTENDEE;PARTSTAT=NEEDS-ACTION:mailto:nous@wind.example\r\n\
        ATTENDEE;PARTSTAT=ACCEPTED;CN=Paul Durand:mailto:paul@contoso.com\r\n\
        END:VEVENT\r\nEND:VCALENDAR\r\n";
    let inv = parse(ics, US).unwrap();
    let attendee = inv.attendee.expect("attendee");
    assert_eq!(attendee.address, "paul@contoso.com");
    assert_eq!(inv.attendee_participation, Some(Participation::Accepted));
}

#[test]
fn generated_reply_is_conforming_and_reparsable() {
    let inv = parse(&load("google"), US).unwrap();
    let org = inv.organizer.expect("organizer");
    let request = ReplyRequest {
        uid: &inv.uid,
        sequence: inv.sequence,
        organizer_address: &org.address,
        our_address: US,
        participation: Participation::Declined,
        dtstamp_epoch: epoch("2026-08-22T12:00:00Z"),
    };
    let reply = itip_reply(&request);
    // Wire form: CRLF only, lines ≤ 75 bytes.
    assert!(
        !reply.replace("\r\n", "").contains('\n'),
        "bare LF in the output"
    );
    let longest = reply.split("\r\n").map(str::len).max().unwrap_or(0);
    assert!(longest <= 75, "line of {longest} bytes");
    assert!(reply.contains("METHOD:REPLY"));
    // Re-parse: the identity of the meeting and our status survive.
    let reread = parse(&reply, org.address.as_str()).unwrap();
    assert_eq!(reread.method, Method::Reply);
    assert_eq!(reread.uid, "7f3e9a2b1c4d5e6f@google.com");
    assert_eq!(reread.sequence, 0);
    let attendee = reread.attendee.expect("attendee");
    assert_eq!(attendee.address, US);
    assert_eq!(reread.attendee_participation, Some(Participation::Declined));
}

#[test]
fn unreadable_text_says_unreadable() {
    assert_eq!(parse("not a calendar", US), Err(IcalError::Unreadable));
}

#[test]
fn calendar_without_vevent_says_no_event() {
    let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nMETHOD:REQUEST\r\nEND:VCALENDAR\r\n";
    assert_eq!(parse(ics, US), Err(IcalError::NoEvent));
}

#[test]
fn absent_method_says_unknown_method() {
    // A pure export .ics (without METHOD) is not an invitation.
    let ics = load("utc").replace("METHOD:REQUEST\r\n", "");
    assert_eq!(
        parse(&ics, US),
        Err(IcalError::UnknownMethod),
        "a calendar without METHOD is not an iTIP message"
    );
}

#[test]
fn bare_lf_is_tolerated() {
    // Some producers deliver bare LF; the extraction is identical.
    let raw = include_str!("fixtures/google-request.ics").replace("\r\n", "\n");
    let via_lf = parse(&raw, US).unwrap();
    let via_crlf = parse(&load("google"), US).unwrap();
    assert_eq!(via_lf, via_crlf);
}

#[test]
fn invitation_is_clonable_and_comparable() {
    let inv: Invitation = parse(&load("google"), US).unwrap();
    assert_eq!(inv.clone(), inv);
}

use mail_ical::parse;

#[test]
fn reply_preserves_the_original_occurrence_and_timezone() {
    for property in [
        "RECURRENCE-ID:20260908T120059Z",
        "RECURRENCE-ID;VALUE=DATE:20260908",
        "RECURRENCE-ID;TZID=Europe/Paris:20260908T140059",
        "RECURRENCE-ID:20260908T140059",
    ] {
        let recurrence = mail_ical::RecurrenceId::from_property(property).unwrap();
        let reply = mail_ical::itip_reply(&mail_ical::ReplyRequest {
            recurrence_id: Some(&recurrence),
            uid: "series@example.fr",
            sequence: 2,
            organizer_address: "owner@example.fr",
            our_address: "guest@example.fr",
            participation: mail_ical::Participation::Accepted,
            dtstamp_epoch: 1788696000,
        });
        let parsed = parse(&reply, "owner@example.fr").unwrap();
        assert_eq!(parsed.scheduling.recurrence, Some(recurrence));
        assert_eq!(reply.matches("RECURRENCE-ID").count(), 1);
    }
}

#[test]
fn stored_recurrence_cannot_inject_another_calendar_property() {
    for property in [
        "RECURRENCE-ID:20260908T120000Z\r\nORGANIZER:mailto:other@example.fr",
        "RECURRENCE-ID:20260908T120000Z\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:injected",
        "RECURRENCE-ID;RANGE=THISANDFUTURE:20260908T120000Z",
    ] {
        assert!(mail_ical::RecurrenceId::from_property(property).is_none());
    }
}

fn invitation(extra: &str) -> String {
    format!(
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nMETHOD:REQUEST\r\nBEGIN:VEVENT\r\nUID:series@example.fr\r\nDTSTAMP:20260906T120000Z\r\nSEQUENCE:2\r\nORGANIZER:mailto:owner@example.fr\r\nDTSTART:20260910T120000Z\r\n{extra}END:VEVENT\r\nEND:VCALENDAR\r\n"
    )
}

#[test]
fn revisions_keep_the_source_timestamp() {
    let parsed = parse(&invitation(""), "guest@example.fr").unwrap();
    assert_eq!(parsed.scheduling.dtstamp_epoch, Some(1788696000));
    assert!(parsed.scheduling.supported);
    for stamp in ["not-a-time", "20260906T120000", "20260230T120000Z"] {
        let text = invitation("").replace("20260906T120000Z", stamp);
        assert_eq!(
            parse(&text, "guest@example.fr")
                .unwrap()
                .scheduling
                .dtstamp_epoch,
            None
        );
    }
}

#[test]
fn occurrence_identity_keeps_seconds_date_type_and_original_start() {
    let original = "RECURRENCE-ID:20260908T120059Z\r\n";
    let first = parse(&invitation(original), "guest@example.fr").unwrap();
    let moved = parse(
        &invitation(original).replace("20260910T120000Z", "20261010T140000Z"),
        "guest@example.fr",
    )
    .unwrap();
    assert_eq!(first.scheduling.recurrence, moved.scheduling.recurrence);
    let id = first.scheduling.recurrence.unwrap();
    assert!(id.property().contains("20260908T120059Z"));
    for other in [
        "RECURRENCE-ID:20260908T120058Z\r\n",
        "RECURRENCE-ID;VALUE=DATE:20260908\r\n",
        "RECURRENCE-ID:20260908T120059\r\n",
    ] {
        let parsed = parse(&invitation(other), "guest@example.fr").unwrap();
        assert!(parsed.scheduling.supported);
        assert_ne!(id.key(), parsed.scheduling.recurrence.unwrap().key());
    }
}

#[test]
fn known_timezone_and_utc_compare_as_the_same_occurrence() {
    let utc = parse(
        &invitation("RECURRENCE-ID:20260908T120059Z\r\n"),
        "guest@example.fr",
    )
    .unwrap();
    let local = parse(
        &invitation("RECURRENCE-ID;TZID=Europe/Paris:20260908T140059\r\n"),
        "guest@example.fr",
    )
    .unwrap();
    assert_eq!(
        utc.scheduling.recurrence.unwrap().key(),
        local.scheduling.recurrence.as_ref().unwrap().key()
    );
    assert!(
        local
            .scheduling
            .recurrence
            .unwrap()
            .property()
            .contains("TZID=Europe/Paris")
    );
}

#[test]
fn unsupported_shapes_stay_readable_without_becoming_series_actions() {
    for extra in [
        "RECURRENCE-ID;RANGE=THISANDFUTURE:20260908T120000Z\r\n",
        "RECURRENCE-ID;TZID=Unknown/Zone:20260908T120000\r\n",
        "RECURRENCE-ID:broken\r\n",
        "RECURRENCE-ID:20260908T120000Z\r\nRECURRENCE-ID:20260909T120000Z\r\n",
        "END:VEVENT\r\nBEGIN:VEVENT\r\nUID:another@example.fr\r\n",
        "SEQUENCE:3\r\n",
    ] {
        let parsed = parse(&invitation(extra), "guest@example.fr").unwrap();
        assert!(!parsed.scheduling.supported, "{extra}");
        assert_eq!(parsed.uid, "series@example.fr");
    }
    let negative = invitation("").replace("SEQUENCE:2", "SEQUENCE:-1");
    assert!(
        !parse(&negative, "guest@example.fr")
            .unwrap()
            .scheduling
            .supported
    );
}

#[test]
fn duplicate_calendar_method_and_event_dates_are_not_actionable() {
    for text in [
        invitation("").replace("METHOD:REQUEST", "METHOD:REQUEST\r\nMETHOD:CANCEL"),
        invitation("DTSTART:20260911T120000Z\r\n"),
        invitation("").replace("DTSTAMP:20260906T120000Z", "DTSTAMP:broken"),
        invitation("").replace("UID:series@example.fr", "UID: "),
    ] {
        assert!(
            parse(&text, "guest@example.fr").map_or(true, |p| !p.scheduling.supported),
            "{text}"
        );
    }
}

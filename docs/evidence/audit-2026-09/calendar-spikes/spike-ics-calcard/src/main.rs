// Spike jetable — option A : crate calcard 0.3.11 (Stalwart).
// Parse les 6 fixtures, compare a ATTENDU.md, genere un METHOD:REPLY.

use calcard::common::timezone::Tz;
use calcard::common::{IanaString, PartialDateTime};
use calcard::icalendar::{
    ICalendar, ICalendarComponent, ICalendarComponentType, ICalendarEntry, ICalendarMethod,
    ICalendarParameter, ICalendarParameterName, ICalendarParameterValue, ICalendarParticipationStatus,
    ICalendarProperty, ICalendarValue, ICalendarValueType,
};
use calcard::{Entry, Parser};
use chrono::Utc;
use std::time::Instant;

const NOUS: &str = "nous@wind.example";

#[derive(Debug, Default, PartialEq)]
struct Extraction {
    method: Option<String>,
    uid: Option<String>,
    sequence: Option<i64>,
    summary: Option<String>,
    location: Option<String>,
    org_email: Option<String>,
    org_cn: Option<String>,
    start_utc: Option<String>,
    end_utc: Option<String>,
    allday: bool,
    rrule: bool,
    partstat: Option<String>,
}

struct Expected {
    file: &'static str,
    method: &'static str,
    uid: &'static str,
    sequence: i64,
    summary: &'static str,
    location: Option<&'static str>,
    org_email: &'static str,
    org_cn: &'static str,
    start_utc: &'static str,
    end_utc: &'static str,
    allday: bool,
    rrule: bool,
    partstat: Option<&'static str>,
}

const EXPECTED: &[Expected] = &[
    Expected {
        file: "google-request.ics",
        method: "REQUEST",
        uid: "7f3e9a2b1c4d5e6f@google.com",
        sequence: 0,
        summary: "Revue budg\u{e9}taire T3 \u{2014} comit\u{e9} de pilotage",
        location: Some("Salle Vosges, 3e \u{e9}tage"),
        org_email: "claire.martin@exemple.fr",
        org_cn: "Claire Martin",
        start_utc: "2026-09-03T12:30:00Z",
        end_utc: "2026-09-03T13:30:00Z",
        allday: false,
        rrule: false,
        partstat: Some("NEEDS-ACTION"),
    },
    Expected {
        file: "outlook-request.ics",
        method: "REQUEST",
        uid: "040000008200E00074C5B7101A82E00800000000B0C3D4E5F6A7B8C9",
        sequence: 0,
        summary: "Entretien annuel",
        location: Some("Bureau 204"),
        org_email: "paul.durand@contoso.com",
        org_cn: "Paul Durand",
        start_utc: "2026-12-10T08:00:00Z",
        end_utc: "2026-12-10T08:30:00Z",
        allday: false,
        rrule: false,
        partstat: Some("NEEDS-ACTION"),
    },
    Expected {
        file: "utc-request.ics",
        method: "REQUEST",
        uid: "9a8b7c6d-5e4f-3a2b-1c0d-e9f8a7b6c5d4",
        sequence: 0,
        summary: "Appel fournisseur",
        location: None,
        org_email: "lena.berg@exemple.org",
        org_cn: "Lena Berg",
        start_utc: "2026-09-03T12:30:00Z",
        end_utc: "2026-09-03T13:00:00Z",
        allday: false,
        rrule: false,
        partstat: Some("NEEDS-ACTION"),
    },
    Expected {
        file: "allday-request.ics",
        method: "REQUEST",
        uid: "allday-2026-09-07@exemple.fr",
        sequence: 0,
        summary: "S\u{e9}minaire d'\u{e9}quipe",
        location: Some("Domaine des C\u{e8}dres"),
        org_email: "claire.martin@exemple.fr",
        org_cn: "Claire Martin",
        start_utc: "date 2026-09-07",
        end_utc: "date 2026-09-08",
        allday: true,
        rrule: false,
        partstat: Some("NEEDS-ACTION"),
    },
    Expected {
        file: "cancel.ics",
        method: "CANCEL",
        uid: "7f3e9a2b1c4d5e6f@google.com",
        sequence: 1,
        summary: "Revue budg\u{e9}taire T3 \u{2014} comit\u{e9} de pilotage",
        location: None,
        org_email: "claire.martin@exemple.fr",
        org_cn: "Claire Martin",
        start_utc: "2026-09-03T12:30:00Z",
        end_utc: "2026-09-03T13:30:00Z",
        allday: false,
        rrule: false,
        partstat: None, // « — » dans ATTENDU : non compare
    },
    Expected {
        file: "recurrence-request.ics",
        method: "REQUEST",
        uid: "hebdo-standup@exemple.fr",
        sequence: 0,
        summary: "Point hebdomadaire",
        location: None,
        org_email: "claire.martin@exemple.fr",
        org_cn: "Claire Martin",
        start_utc: "2026-09-08T08:00:00Z",
        end_utc: "2026-09-08T08:45:00Z",
        allday: false,
        rrule: true,
        partstat: Some("NEEDS-ACTION"),
    },
];

fn parse_ical(input: &str) -> Result<ICalendar, String> {
    match Parser::new(input).entry() {
        Entry::ICalendar(ical) => Ok(ical),
        other => Err(format!("{other:?}")),
    }
}

fn is_date_only(entry: &ICalendarEntry) -> bool {
    entry
        .parameter(&ICalendarParameterName::Value)
        .is_some_and(|v| matches!(v, ICalendarParameterValue::Value(ICalendarValueType::Date)))
}

fn instant_utc(entry: &ICalendarEntry, ical: &ICalendar) -> Option<String> {
    let pdt = entry.values.first()?.as_partial_date_time()?;
    if is_date_only(entry) {
        return Some(format!(
            "date {:04}-{:02}-{:02}",
            pdt.year?, pdt.month?, pdt.day?
        ));
    }
    let resolver = ical.build_tz_resolver();
    let tz: Tz = resolver.resolve_or_default(entry.tz_id());
    let dt = pdt.to_date_time_with_tz(tz)?;
    Some(dt.with_timezone(&Utc).format("%Y-%m-%dT%H:%M:%SZ").to_string())
}

fn extract(ical: &ICalendar) -> Extraction {
    let mut ex = Extraction::default();
    for comp in &ical.components {
        match comp.component_type {
            ICalendarComponentType::VCalendar => {
                if let Some(e) = comp.property(&ICalendarProperty::Method) {
                    if let Some(ICalendarValue::Method(m)) = e.values.first() {
                        ex.method = Some(m.as_str().to_string());
                    }
                }
            }
            ICalendarComponentType::VEvent => {
                ex.uid = comp.uid().map(str::to_string);
                ex.sequence = comp
                    .property(&ICalendarProperty::Sequence)
                    .and_then(|e| e.values.first())
                    .and_then(|v| v.as_integer());
                ex.summary = comp
                    .property(&ICalendarProperty::Summary)
                    .and_then(|e| e.values.first())
                    .and_then(|v| v.as_text())
                    .map(str::to_string);
                ex.location = comp
                    .property(&ICalendarProperty::Location)
                    .and_then(|e| e.values.first())
                    .and_then(|v| v.as_text())
                    .map(str::to_string);
                if let Some(org) = comp.property(&ICalendarProperty::Organizer) {
                    ex.org_email = org.calendar_address().map(str::to_string);
                    ex.org_cn = org
                        .parameter(&ICalendarParameterName::Cn)
                        .and_then(|v| v.as_text())
                        .map(str::to_string);
                }
                if let Some(dtstart) = comp.property(&ICalendarProperty::Dtstart) {
                    ex.allday = is_date_only(dtstart);
                    ex.start_utc = instant_utc(dtstart, ical);
                }
                if let Some(dtend) = comp.property(&ICalendarProperty::Dtend) {
                    ex.end_utc = instant_utc(dtend, ical);
                }
                ex.rrule = comp.has_property(&ICalendarProperty::Rrule);
                for att in comp.properties(&ICalendarProperty::Attendee) {
                    if att.calendar_address() == Some(NOUS) {
                        ex.partstat = att
                            .parameter(&ICalendarParameterName::Partstat)
                            .map(|v| match v {
                                ICalendarParameterValue::Partstat(p) => p.as_str().to_string(),
                                other => format!("{other:?}"),
                            });
                    }
                }
            }
            _ => {}
        }
    }
    ex
}

fn check(label: &str, got: &Option<String>, want: Option<&str>) -> bool {
    let ok = got.as_deref() == want;
    println!(
        "  [{}] {:<12} attendu={:?} obtenu={:?}",
        if ok { "PASS" } else { "FAIL" },
        label,
        want,
        got.as_deref()
    );
    ok
}

fn check_val<T: PartialEq + std::fmt::Debug>(label: &str, got: T, want: T) -> bool {
    let ok = got == want;
    println!(
        "  [{}] {:<12} attendu={:?} obtenu={:?}",
        if ok { "PASS" } else { "FAIL" },
        label,
        want,
        got
    );
    ok
}

fn build_reply(request: &ICalendar) -> ICalendar {
    let vevent_req = request
        .components
        .iter()
        .find(|c| c.component_type == ICalendarComponentType::VEvent)
        .expect("VEVENT absent");
    let uid = vevent_req.uid().expect("UID absent");
    let organizer = vevent_req
        .property(&ICalendarProperty::Organizer)
        .expect("ORGANIZER absent")
        .clone();

    let mut vcal = ICalendarComponent::new(ICalendarComponentType::VCalendar);
    vcal.add_property(ICalendarProperty::Version, "2.0");
    vcal.add_property(ICalendarProperty::Prodid, "-//Wind//Spike calcard//FR");
    vcal.add_property(
        ICalendarProperty::Method,
        ICalendarValue::Method(ICalendarMethod::Reply),
    );

    let mut vevent = ICalendarComponent::new(ICalendarComponentType::VEvent);
    vevent.add_uid(uid);
    vevent.add_sequence(0);
    vevent.add_dtstamp(PartialDateTime::from_utc_timestamp(Utc::now().timestamp()));
    vevent.entries.push(organizer);
    vevent.entries.push(
        ICalendarEntry::new(ICalendarProperty::Attendee)
            .with_param(ICalendarParameter::partstat(ICalendarParameterValue::Partstat(
                ICalendarParticipationStatus::Accepted,
            )))
            .with_value(format!("mailto:{NOUS}")),
    );

    vcal.component_ids = vec![1];
    ICalendar {
        components: vec![vcal, vevent],
    }
}

fn main() {
    let dir = std::env::args().nth(1).expect("usage: spike <dossier-fixtures>");
    let mut total_pass = 0u32;
    let mut total_fail = 0u32;

    for exp in EXPECTED {
        let raw = std::fs::read_to_string(format!("{dir}/{}", exp.file)).expect("lecture fixture");
        let crlf = raw.replace("\r\n", "\n").replace('\n', "\r\n");

        println!("=== {} ===", exp.file);
        let ical = match parse_ical(&crlf) {
            Ok(i) => i,
            Err(e) => {
                println!("  [FAIL] parse: {e}");
                total_fail += 12;
                continue;
            }
        };
        let ex = extract(&ical);

        let mut results = vec![
            check("method", &ex.method, Some(exp.method)),
            check("uid", &ex.uid, Some(exp.uid)),
            check_val("sequence", ex.sequence, Some(exp.sequence)),
            check("summary", &ex.summary, Some(exp.summary)),
            check("location", &ex.location, exp.location),
            check("org_email", &ex.org_email, Some(exp.org_email)),
            check("org_cn", &ex.org_cn, Some(exp.org_cn)),
            check("start_utc", &ex.start_utc, Some(exp.start_utc)),
            check("end_utc", &ex.end_utc, Some(exp.end_utc)),
            check_val("allday", ex.allday, exp.allday),
            check_val("rrule", ex.rrule, exp.rrule),
        ];
        if let Some(want_ps) = exp.partstat {
            results.push(check("partstat", &ex.partstat, Some(want_ps)));
        } else {
            println!("  [----] partstat     non compare (\u{2014})");
        }
        total_pass += results.iter().filter(|r| **r).count() as u32;
        total_fail += results.iter().filter(|r| !**r).count() as u32;

        // Tolerance LF nu : parse du texte original (LF) et comparaison
        match parse_ical(&raw) {
            Ok(ical_lf) => {
                let same = extract(&ical_lf) == ex;
                println!("  [info] LF nu tolere: parse OK, extraction identique = {same}");
            }
            Err(e) => println!("  [info] LF nu NON tolere: {e}"),
        }

        // Timing
        let n = 2000;
        let t0 = Instant::now();
        for _ in 0..n {
            let _ = std::hint::black_box(parse_ical(std::hint::black_box(&crlf)));
        }
        let per = t0.elapsed().as_nanos() / n as u128;
        println!("  [info] parse: {} ns/iteration ({n} iterations)", per);
    }

    // === Epreuve de generation : REPLY depuis google-request.ics ===
    println!("=== generation REPLY (depuis google-request.ics) ===");
    let raw = std::fs::read_to_string(format!("{dir}/google-request.ics")).expect("lecture");
    let crlf = raw.replace("\r\n", "\n").replace('\n', "\r\n");
    let request = parse_ical(&crlf).expect("parse request");
    let reply = build_reply(&request);
    let serialized = reply.to_string();
    println!("--- texte genere ---\n{serialized}--- fin ---");

    let all_crlf = serialized.lines().all(|_| true) && !serialized.replace("\r\n", "").contains('\n');
    let max_line = serialized.split("\r\n").map(|l| l.len()).max().unwrap_or(0);
    println!(
        "  [{}] fins de ligne CRLF uniquement",
        if all_crlf { "PASS" } else { "FAIL" }
    );
    println!(
        "  [{}] lignes <= 75 octets (max observe: {max_line})",
        if max_line <= 75 { "PASS" } else { "FAIL" }
    );

    let reparsed = parse_ical(&serialized).expect("re-parse REPLY");
    let rex = extract(&reparsed);
    let g1 = check("method", &rex.method, Some("REPLY"));
    let g2 = check("uid", &rex.uid, Some("7f3e9a2b1c4d5e6f@google.com"));
    let g3 = check("partstat", &rex.partstat, Some("ACCEPTED"));
    let gen_ok = [all_crlf, max_line <= 75, g1, g2, g3];
    total_pass += gen_ok.iter().filter(|r| **r).count() as u32;
    total_fail += gen_ok.iter().filter(|r| !**r).count() as u32;

    // === Sonde : TZID inconnu (VTIMEZONE embarque seulement, nom hors tables) ===
    // Copie en memoire de outlook-request.ics avec TZID renomme.
    println!("=== sonde TZID inconnu (regles VTIMEZONE seules) ===");
    let raw = std::fs::read_to_string(format!("{dir}/outlook-request.ics")).expect("lecture");
    let custom = raw
        .replace("\r\n", "\n")
        .replace('\n', "\r\n")
        .replace("Romance Standard Time", "Zone Perso Wind");
    let ical = parse_ical(&custom).expect("parse");
    let ex = extract(&ical);
    println!(
        "  TZID='Zone Perso Wind' -> start_utc obtenu={:?} (correct serait \"2026-12-10T08:00:00Z\")",
        ex.start_utc
    );
    let resolver = ical.build_tz_resolver();
    println!("  resolve(\"Zone Perso Wind\") = {:?}", resolver.resolve("Zone Perso Wind"));

    println!("=== TOTAL: {total_pass} PASS, {total_fail} FAIL ===");
}

//! Harnais de mesure du spike « parseur ICS maison » (option B).
//! Usage : spike-ics-maison <dossier-fixtures>
//! Sortie : tableau PASS/FAIL par fixture x champ, épreuve REPLY,
//! démonstration du repli TZID inconnu, temps de parsing.

mod parser;
mod reply;

use chrono::{TimeZone, Utc};
use parser::{parse_invitation, Invitation};
use std::time::Instant;

const NOUS: &str = "nous@wind.example";

struct Expected {
    fixture: &'static str,
    method: &'static str,
    uid: &'static str,
    sequence: u32,
    summary: &'static str,
    location: Option<&'static str>,
    org_email: &'static str,
    org_cn: &'static str,
    start: &'static str,
    end: &'static str,
    all_day: bool,
    rrule: bool,
    /// None = « — » dans ATTENDU.md (non exigé).
    partstat: Option<&'static str>,
}

const EXPECTED: &[Expected] = &[
    Expected {
        fixture: "google-request.ics",
        method: "REQUEST",
        uid: "7f3e9a2b1c4d5e6f@google.com",
        sequence: 0,
        summary: "Revue budgétaire T3 — comité de pilotage",
        location: Some("Salle Vosges, 3e étage"),
        org_email: "claire.martin@exemple.fr",
        org_cn: "Claire Martin",
        start: "2026-09-03T12:30:00Z",
        end: "2026-09-03T13:30:00Z",
        all_day: false,
        rrule: false,
        partstat: Some("NEEDS-ACTION"),
    },
    Expected {
        fixture: "outlook-request.ics",
        method: "REQUEST",
        uid: "040000008200E00074C5B7101A82E00800000000B0C3D4E5F6A7B8C9",
        sequence: 0,
        summary: "Entretien annuel",
        location: Some("Bureau 204"),
        org_email: "paul.durand@contoso.com",
        org_cn: "Paul Durand",
        start: "2026-12-10T08:00:00Z",
        end: "2026-12-10T08:30:00Z",
        all_day: false,
        rrule: false,
        partstat: Some("NEEDS-ACTION"),
    },
    Expected {
        fixture: "utc-request.ics",
        method: "REQUEST",
        uid: "9a8b7c6d-5e4f-3a2b-1c0d-e9f8a7b6c5d4",
        sequence: 0,
        summary: "Appel fournisseur",
        location: None,
        org_email: "lena.berg@exemple.org",
        org_cn: "Lena Berg",
        start: "2026-09-03T12:30:00Z",
        end: "2026-09-03T13:00:00Z",
        all_day: false,
        rrule: false,
        partstat: Some("NEEDS-ACTION"),
    },
    Expected {
        fixture: "allday-request.ics",
        method: "REQUEST",
        uid: "allday-2026-09-07@exemple.fr",
        sequence: 0,
        summary: "Séminaire d'équipe",
        location: Some("Domaine des Cèdres"),
        org_email: "claire.martin@exemple.fr",
        org_cn: "Claire Martin",
        start: "date 2026-09-07",
        end: "date 2026-09-08",
        all_day: true,
        rrule: false,
        partstat: Some("NEEDS-ACTION"),
    },
    Expected {
        fixture: "cancel.ics",
        method: "CANCEL",
        uid: "7f3e9a2b1c4d5e6f@google.com",
        sequence: 1,
        summary: "Revue budgétaire T3 — comité de pilotage",
        location: None,
        org_email: "claire.martin@exemple.fr",
        org_cn: "Claire Martin",
        start: "2026-09-03T12:30:00Z",
        end: "2026-09-03T13:30:00Z",
        all_day: false,
        rrule: false,
        partstat: None,
    },
    Expected {
        fixture: "recurrence-request.ics",
        method: "REQUEST",
        uid: "hebdo-standup@exemple.fr",
        sequence: 0,
        summary: "Point hebdomadaire",
        location: None,
        org_email: "claire.martin@exemple.fr",
        org_cn: "Claire Martin",
        start: "2026-09-08T08:00:00Z",
        end: "2026-09-08T08:45:00Z",
        all_day: false,
        rrule: true,
        partstat: Some("NEEDS-ACTION"),
    },
];

fn normalise_crlf(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\n', "\r\n")
}

fn check(
    fails: &mut u32,
    fixture: &str,
    champ: &str,
    attendu: &str,
    obtenu: &str,
) {
    let ok = attendu == obtenu;
    if !ok {
        *fails += 1;
    }
    println!(
        "| {} | {} | {} | {} | {} |",
        fixture,
        champ,
        attendu,
        obtenu,
        if ok { "PASS" } else { "FAIL" }
    );
}

fn opt(o: &Option<String>) -> String {
    o.clone().unwrap_or_else(|| "(absent)".into())
}

fn verify(exp: &Expected, inv: &Invitation, fails: &mut u32) {
    let f = exp.fixture;
    check(fails, f, "method", exp.method, &opt(&inv.method));
    check(fails, f, "uid", exp.uid, &opt(&inv.uid));
    check(fails, f, "sequence", &exp.sequence.to_string(), &inv.sequence.to_string());
    check(fails, f, "summary", exp.summary, &opt(&inv.summary));
    check(
        fails,
        f,
        "location",
        exp.location.unwrap_or("(absent)"),
        &opt(&inv.location),
    );
    check(fails, f, "organizer.email", exp.org_email, &opt(&inv.organizer_email));
    check(fails, f, "organizer.cn", exp.org_cn, &opt(&inv.organizer_cn));
    check(
        fails,
        f,
        "debut",
        exp.start,
        &inv.dtstart.as_ref().map(|w| w.to_display()).unwrap_or_else(|| "(absent)".into()),
    );
    check(
        fails,
        f,
        "fin",
        exp.end,
        &inv.dtend.as_ref().map(|w| w.to_display()).unwrap_or_else(|| "(absent)".into()),
    );
    check(
        fails,
        f,
        "journee_entiere",
        if exp.all_day { "oui" } else { "non" },
        if inv.all_day { "oui" } else { "non" },
    );
    check(
        fails,
        f,
        "rrule",
        if exp.rrule { "oui" } else { "non" },
        if inv.has_rrule { "oui" } else { "non" },
    );
    match exp.partstat {
        Some(p) => check(fails, f, "partstat(nous)", p, &opt(&inv.our_partstat)),
        None => println!(
            "| {} | partstat(nous) | — (non exigé) | {} | PASS |",
            f,
            opt(&inv.our_partstat)
        ),
    }
    if inv.tz_non_resolue {
        println!("| {} | (drapeau) | | tz_non_resolue=vrai | INFO |", f);
    }
}

fn main() {
    let dir = std::env::args().nth(1).expect("usage: spike-ics-maison <dossier-fixtures>");
    let mut fails = 0u32;

    println!("## 1. Tableau PASS/FAIL par fixture x champ\n");
    println!("| fixture | champ | attendu | obtenu | verdict |");
    println!("|---|---|---|---|---|");
    let mut contents: Vec<(String, String)> = Vec::new();
    for exp in EXPECTED {
        let path = format!("{}/{}", dir, exp.fixture);
        let raw = std::fs::read_to_string(&path).expect(&path);
        let norm = normalise_crlf(&raw);
        let inv = parse_invitation(&norm, NOUS);
        verify(exp, &inv, &mut fails);
        contents.push((exp.fixture.to_string(), norm));
    }

    println!("\n## 2. Épreuve de génération (METHOD:REPLY depuis google-request.ics)\n");
    let google = &contents.iter().find(|(n, _)| n == "google-request.ics").unwrap().1;
    let inv = parse_invitation(google, NOUS);
    let dtstamp = Utc.with_ymd_and_hms(2026, 8, 22, 12, 0, 0).unwrap();
    let rep = reply::build_reply(&inv, NOUS, "Nous Wind", "ACCEPTED", dtstamp);
    println!("```");
    print!("{}", rep.replace("\r\n", "\n"));
    println!("```");
    // Contraintes physiques : CRLF, lignes <= 75 octets.
    let crlf_ok = rep.ends_with("\r\n") && !rep.replace("\r\n", "").contains('\r');
    let max_line = rep.split("\r\n").map(|l| l.len()).max().unwrap_or(0);
    check(&mut fails, "reply", "fins CRLF", "oui", if crlf_ok { "oui" } else { "non" });
    check(
        &mut fails,
        "reply",
        "lignes <= 75 octets",
        "oui",
        if max_line <= 75 { "oui" } else { "non" },
    );
    println!("(ligne physique la plus longue : {} octets)", max_line);
    // Re-parse.
    let re = parse_invitation(&rep, NOUS);
    check(&mut fails, "reply(re-parse)", "method", "REPLY", &opt(&re.method));
    check(&mut fails, "reply(re-parse)", "uid", "7f3e9a2b1c4d5e6f@google.com", &opt(&re.uid));
    check(&mut fails, "reply(re-parse)", "partstat(nous)", "ACCEPTED", &opt(&re.our_partstat));
    check(&mut fails, "reply(re-parse)", "sequence", "0", &re.sequence.to_string());

    // Pliage forcé : la REPLY ci-dessus tient sous 75 octets, on force
    // donc le chemin de pliage avec un résumé long (accents = multi-octets).
    let mut longue = inv;
    longue.summary = Some(
        "Revue budgétaire T3 — comité de pilotage élargi aux directions régionales, préparation du séminaire annuel"
            .to_string(),
    );
    let rep2 = reply::build_reply(&longue, NOUS, "Nous Wind", "ACCEPTED", dtstamp);
    let max2 = rep2.split("\r\n").map(|l| l.len()).max().unwrap_or(0);
    let re2 = parse_invitation(&rep2, NOUS);
    check(
        &mut fails,
        "reply(pliage force)",
        "lignes <= 75 octets",
        "oui",
        if max2 <= 75 { "oui" } else { "non" },
    );
    check(
        &mut fails,
        "reply(pliage force)",
        "summary restitué au re-parse",
        longue.summary.as_deref().unwrap(),
        &opt(&re2.summary),
    );
    println!(
        "(pliage forcé : {} lignes physiques, la plus longue {} octets)",
        rep2.split("\r\n").count(),
        max2
    );

    println!("\n## 3. Repli TZID non résolu (hors table, VTIMEZONE ignoré)\n");
    let exotique = "BEGIN:VCALENDAR\r\nMETHOD:REQUEST\r\nBEGIN:VEVENT\r\nUID:x@y\r\nDTSTART;TZID=Magallanes Standard Time:20260903T143000\r\nSUMMARY:Test exotique\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    let ex = parse_invitation(exotique, NOUS);
    println!(
        "TZID « Magallanes Standard Time » (Windows, HORS table) : debut = {} ; tz_non_resolue = {}",
        ex.dtstart.as_ref().map(|w| w.to_display()).unwrap_or_default(),
        ex.tz_non_resolue
    );
    check(
        &mut fails,
        "tzid-exotique",
        "repli flottant + drapeau",
        "oui",
        if ex.tz_non_resolue && matches!(ex.dtstart, Some(parser::When::Floating { .. })) {
            "oui"
        } else {
            "non"
        },
    );

    println!("\n## 4. Temps de parsing (20 000 itérations par fixture)\n");
    println!("| fixture | octets | total (ms) | par parse (µs) |");
    println!("|---|---|---|---|");
    const N: u32 = 20_000;
    for (name, content) in &contents {
        // Échauffement.
        for _ in 0..1_000 {
            std::hint::black_box(parse_invitation(std::hint::black_box(content), NOUS));
        }
        let t0 = Instant::now();
        for _ in 0..N {
            std::hint::black_box(parse_invitation(std::hint::black_box(content), NOUS));
        }
        let dt = t0.elapsed();
        println!(
            "| {} | {} | {:.1} | {:.2} |",
            name,
            content.len(),
            dt.as_secs_f64() * 1000.0,
            dt.as_secs_f64() * 1e6 / N as f64
        );
    }

    println!("\n== Bilan : {} FAIL ==", fails);
    std::process::exit(if fails == 0 { 0 } else { 1 });
}

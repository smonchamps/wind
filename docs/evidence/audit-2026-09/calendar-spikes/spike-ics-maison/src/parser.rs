//! Parseur iCalendar minimal (spike, jetable).
//!
//! Couvre exactement ce que Wind doit lire d'un iTIP REQUEST/CANCEL :
//! dépliage RFC 5545 §3.1, propriétés + paramètres (avec valeurs entre
//! guillemets), déséchappement, METHOD/UID/SEQUENCE/SUMMARY/LOCATION/
//! ORGANIZER/DTSTART/DTEND/RRULE (présence)/PARTSTAT du compte local.
//! Les VTIMEZONE embarqués ne sont PAS interprétés : TZID résolu via
//! chrono-tz (IANA) ou table Windows→IANA embarquée ; sinon repli en
//! heure flottante + drapeau `tz_non_resolue`.

use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz;

/// Table Windows→IANA embarquée : les entrées les plus courantes côté
/// Exchange/Outlook (source : CLDR windowsZones, territoire 001).
/// La table CLDR complète fait ~140 entrées (voir rapport).
pub const WINDOWS_TO_IANA: &[(&str, &str)] = &[
    ("Romance Standard Time", "Europe/Paris"),
    ("W. Europe Standard Time", "Europe/Berlin"),
    ("Central Europe Standard Time", "Europe/Budapest"),
    ("Central European Standard Time", "Europe/Warsaw"),
    ("GMT Standard Time", "Europe/London"),
    ("Greenwich Standard Time", "Atlantic/Reykjavik"),
    ("GTB Standard Time", "Europe/Bucharest"),
    ("FLE Standard Time", "Europe/Kiev"),
    ("Russian Standard Time", "Europe/Moscow"),
    ("Eastern Standard Time", "America/New_York"),
    ("Central Standard Time", "America/Chicago"),
    ("Mountain Standard Time", "America/Denver"),
    ("Pacific Standard Time", "America/Los_Angeles"),
    ("Atlantic Standard Time", "America/Halifax"),
    ("SA Pacific Standard Time", "America/Bogota"),
    ("E. South America Standard Time", "America/Sao_Paulo"),
    ("China Standard Time", "Asia/Shanghai"),
    ("Tokyo Standard Time", "Asia/Tokyo"),
    ("Korea Standard Time", "Asia/Seoul"),
    ("India Standard Time", "Asia/Calcutta"),
    ("Singapore Standard Time", "Asia/Singapore"),
    ("AUS Eastern Standard Time", "Australia/Sydney"),
    ("New Zealand Standard Time", "Pacific/Auckland"),
    ("UTC", "Etc/UTC"),
];

/// Un instant d'événement, tel qu'on a pu le résoudre.
#[derive(Debug, Clone, PartialEq)]
pub enum When {
    /// Instant résolu en UTC (valeur en Z, ou TZID résolu).
    Utc(DateTime<Utc>),
    /// Journée entière (VALUE=DATE).
    Date(NaiveDate),
    /// Heure flottante : pas de TZID (légal, RFC 5545 §3.3.5) ou TZID
    /// non résolu (repli honnête : on affiche l'heure locale telle
    /// quelle, sans conversion, et on lève un drapeau).
    Floating {
        local: NaiveDateTime,
        tzid: Option<String>,
    },
}

impl When {
    pub fn to_display(&self) -> String {
        match self {
            When::Utc(dt) => dt.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            When::Date(d) => format!("date {}", d.format("%Y-%m-%d")),
            When::Floating { local, tzid } => match tzid {
                Some(t) => format!("flottant {} (TZID non résolu: {})", local.format("%Y-%m-%dT%H:%M:%S"), t),
                None => format!("flottant {}", local.format("%Y-%m-%dT%H:%M:%S")),
            },
        }
    }
}

/// Le sous-ensemble iTIP dont Wind a besoin.
#[derive(Debug, Default)]
pub struct Invitation {
    pub method: Option<String>,
    pub uid: Option<String>,
    pub sequence: u32,
    pub summary: Option<String>,
    pub location: Option<String>,
    pub organizer_email: Option<String>,
    pub organizer_cn: Option<String>,
    pub dtstart: Option<When>,
    pub dtend: Option<When>,
    pub all_day: bool,
    pub has_rrule: bool,
    /// PARTSTAT de l'ATTENDEE correspondant au compte local.
    pub our_partstat: Option<String>,
    /// Vrai si un TZID n'a pas pu être résolu (repli heure flottante).
    pub tz_non_resolue: bool,
}

/// Une ligne de contenu décomposée : NOM;P1=V1;P2="V2":VALEUR
pub struct ContentLine<'a> {
    pub name: String,
    pub params: Vec<(String, String)>,
    pub value: &'a str,
}

/// Dépliage RFC 5545 §3.1 : une ligne qui commence par SP ou HTAB
/// continue la précédente. Tolère les fins LF nues (on split sur '\n'
/// et on retire un '\r' final éventuel) — fait à noter au rapport.
pub fn unfold(input: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for raw in input.split('\n') {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        if line.is_empty() {
            continue;
        }
        if (line.starts_with(' ') || line.starts_with('\t')) && !out.is_empty() {
            out.last_mut().unwrap().push_str(&line[1..]);
        } else {
            out.push(line.to_string());
        }
    }
    out
}

/// Découpe une ligne de contenu. Respecte les valeurs de paramètre
/// entre guillemets (qui peuvent contenir : ; ,).
pub fn parse_content_line(line: &str) -> Option<ContentLine<'_>> {
    let mut in_quotes = false;
    let mut colon = None;
    let mut semis: Vec<usize> = Vec::new();
    for (i, c) in line.char_indices() {
        match c {
            '"' => in_quotes = !in_quotes,
            ':' if !in_quotes => {
                colon = Some(i);
                break;
            }
            ';' if !in_quotes => semis.push(i),
            _ => {}
        }
    }
    let colon = colon?;
    let name_end = semis.first().copied().unwrap_or(colon);
    let name = line[..name_end].trim().to_ascii_uppercase();
    let mut params = Vec::new();
    let mut bounds = semis;
    bounds.push(colon);
    for w in bounds.windows(2) {
        let seg = &line[w[0] + 1..w[1]];
        if let Some(eq) = seg.find('=') {
            let pname = seg[..eq].trim().to_ascii_uppercase();
            let pval = seg[eq + 1..].trim().trim_matches('"').to_string();
            params.push((pname, pval));
        }
    }
    Some(ContentLine {
        name,
        params,
        value: &line[colon + 1..],
    })
}

fn param<'a>(params: &'a [(String, String)], name: &str) -> Option<&'a str> {
    params
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, v)| v.as_str())
}

/// Déséchappement RFC 5545 §3.3.11 : \\ \; \, \n \N
pub fn unescape(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    let mut chars = v.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') | Some('N') => out.push('\n'),
                Some(other) => out.push(other),
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn resolve_tzid(tzid: &str) -> Option<Tz> {
    if let Ok(tz) = tzid.parse::<Tz>() {
        return Some(tz);
    }
    WINDOWS_TO_IANA
        .iter()
        .find(|(win, _)| win.eq_ignore_ascii_case(tzid))
        .and_then(|(_, iana)| iana.parse::<Tz>().ok())
}

/// Parse une valeur DATE-TIME ou DATE. Retourne (When, tz_non_resolue).
fn parse_when(value: &str, params: &[(String, String)]) -> Option<(When, bool)> {
    let value = value.trim();
    if param(params, "VALUE") == Some("DATE") || (value.len() == 8 && !value.contains('T')) {
        let d = NaiveDate::parse_from_str(value, "%Y%m%d").ok()?;
        return Some((When::Date(d), false));
    }
    if let Some(stripped) = value.strip_suffix('Z') {
        let naive = NaiveDateTime::parse_from_str(stripped, "%Y%m%dT%H%M%S").ok()?;
        return Some((When::Utc(Utc.from_utc_datetime(&naive)), false));
    }
    let naive = NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%S").ok()?;
    match param(params, "TZID") {
        Some(tzid) => match resolve_tzid(tzid) {
            Some(tz) => match tz.from_local_datetime(&naive).earliest() {
                Some(dt) => Some((When::Utc(dt.with_timezone(&Utc)), false)),
                // Trou du passage à l'heure d'été : repli flottant + drapeau.
                None => Some((
                    When::Floating {
                        local: naive,
                        tzid: Some(tzid.to_string()),
                    },
                    true,
                )),
            },
            None => Some((
                When::Floating {
                    local: naive,
                    tzid: Some(tzid.to_string()),
                },
                true,
            )),
        },
        None => Some((When::Floating { local: naive, tzid: None }, false)),
    }
}

/// Parse un VCALENDAR iTIP complet (premier VEVENT rencontré).
/// `our_email` : l'adresse du compte local, pour extraire notre PARTSTAT.
pub fn parse_invitation(input: &str, our_email: &str) -> Invitation {
    let mut inv = Invitation::default();
    let mut stack: Vec<String> = Vec::new();
    let mut vevent_count = 0u32;
    for line in unfold(input) {
        let Some(cl) = parse_content_line(&line) else {
            continue;
        };
        match cl.name.as_str() {
            "BEGIN" => {
                let comp = cl.value.trim().to_ascii_uppercase();
                if comp == "VEVENT" {
                    vevent_count += 1;
                }
                stack.push(comp);
                continue;
            }
            "END" => {
                stack.pop();
                continue;
            }
            _ => {}
        }
        let depth = stack.last().map(String::as_str);
        // METHOD au niveau VCALENDAR.
        if depth == Some("VCALENDAR") && cl.name == "METHOD" {
            inv.method = Some(cl.value.trim().to_string());
            continue;
        }
        // Propriétés du PREMIER VEVENT seulement (les VALARM/VTIMEZONE
        // sont naturellement exclus par la pile).
        if depth != Some("VEVENT") || vevent_count != 1 {
            continue;
        }
        match cl.name.as_str() {
            "UID" => inv.uid = Some(cl.value.trim().to_string()),
            "SEQUENCE" => inv.sequence = cl.value.trim().parse().unwrap_or(0),
            "SUMMARY" => inv.summary = Some(unescape(cl.value)),
            "LOCATION" => inv.location = Some(unescape(cl.value)),
            "RRULE" => inv.has_rrule = true,
            "ORGANIZER" => {
                let mail = cl.value.trim();
                let mail = mail
                    .strip_prefix("mailto:")
                    .or_else(|| mail.strip_prefix("MAILTO:"))
                    .unwrap_or(mail);
                inv.organizer_email = Some(mail.to_string());
                inv.organizer_cn = param(&cl.params, "CN").map(unescape);
            }
            "ATTENDEE" => {
                let mail = cl.value.trim();
                let mail = mail
                    .strip_prefix("mailto:")
                    .or_else(|| mail.strip_prefix("MAILTO:"))
                    .unwrap_or(mail);
                if mail.eq_ignore_ascii_case(our_email) {
                    // PARTSTAT absent => NEEDS-ACTION par défaut (RFC 5545 §3.2.12).
                    inv.our_partstat = Some(
                        param(&cl.params, "PARTSTAT")
                            .unwrap_or("NEEDS-ACTION")
                            .to_string(),
                    );
                }
            }
            "DTSTART" => {
                if let Some((w, fallback)) = parse_when(cl.value, &cl.params) {
                    inv.all_day = matches!(w, When::Date(_));
                    inv.dtstart = Some(w);
                    inv.tz_non_resolue |= fallback;
                }
            }
            "DTEND" => {
                if let Some((w, fallback)) = parse_when(cl.value, &cl.params) {
                    inv.dtend = Some(w);
                    inv.tz_non_resolue |= fallback;
                }
            }
            _ => {}
        }
    }
    inv
}

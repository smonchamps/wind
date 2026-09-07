//! Génération d'un iTIP METHOD:REPLY (spike, jetable).
//! Pliage RFC 5545 §3.1 (lignes > 75 octets), fins CRLF.

use crate::parser::Invitation;
use chrono::{DateTime, Utc};

/// Échappement RFC 5545 §3.3.11 pour une valeur TEXT.
fn escape_text(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    for c in v.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            ';' => out.push_str("\\;"),
            ',' => out.push_str("\\,"),
            '\n' => out.push_str("\\n"),
            _ => out.push(c),
        }
    }
    out
}

/// Valeur de paramètre : guillemets si elle contient : ; ou ,
fn param_value(v: &str) -> String {
    if v.contains([':', ';', ',']) {
        format!("\"{}\"", v)
    } else {
        v.to_string()
    }
}

/// Plie une ligne logique en lignes physiques de 75 octets max,
/// continuation par CRLF + un espace, sans couper un caractère UTF-8.
fn fold(line: &str) -> String {
    if line.len() <= 75 {
        return line.to_string();
    }
    let mut out = String::new();
    let mut rest = line;
    let mut first = true;
    while !rest.is_empty() {
        // Ligne de continuation : l'espace de tête compte dans les 75 octets.
        let budget = if first { 75 } else { 74 };
        if rest.len() <= budget {
            if !first {
                out.push_str("\r\n ");
            }
            out.push_str(rest);
            break;
        }
        let mut i = budget;
        while !rest.is_char_boundary(i) {
            i -= 1;
        }
        if !first {
            out.push_str("\r\n ");
        }
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        first = false;
    }
    out
}

/// Construit le VCALENDAR METHOD:REPLY : même UID, SEQUENCE conservée,
/// DTSTAMP fourni, ORGANIZER conservé, UN SEUL ATTENDEE (nous).
pub fn build_reply(
    inv: &Invitation,
    our_email: &str,
    our_cn: &str,
    partstat: &str,
    dtstamp: DateTime<Utc>,
) -> String {
    let mut lines: Vec<String> = vec![
        "BEGIN:VCALENDAR".into(),
        "PRODID:-//Wind//Spike ICS Maison 0.1//FR".into(),
        "VERSION:2.0".into(),
        "METHOD:REPLY".into(),
        "BEGIN:VEVENT".into(),
    ];
    if let Some(uid) = &inv.uid {
        lines.push(format!("UID:{}", uid));
    }
    lines.push(format!("SEQUENCE:{}", inv.sequence));
    lines.push(format!("DTSTAMP:{}", dtstamp.format("%Y%m%dT%H%M%SZ")));
    if let Some(org) = &inv.organizer_email {
        match &inv.organizer_cn {
            Some(cn) => lines.push(format!(
                "ORGANIZER;CN={}:mailto:{}",
                param_value(cn),
                org
            )),
            None => lines.push(format!("ORGANIZER:mailto:{}", org)),
        }
    }
    lines.push(format!(
        "ATTENDEE;PARTSTAT={};CN={}:mailto:{}",
        partstat,
        param_value(our_cn),
        our_email
    ));
    if let Some(summary) = &inv.summary {
        lines.push(format!("SUMMARY:{}", escape_text(summary)));
    }
    lines.push("END:VEVENT".into());
    lines.push("END:VCALENDAR".into());
    let mut out = String::new();
    for l in lines {
        out.push_str(&fold(&l));
        out.push_str("\r\n");
    }
    out
}

//! The wire vocabularies (PLAN-BASCULE-ANGLAIS E5a, decision D16).
//!
//! Four closed vocabularies cross the IPC as strings and are persisted
//! in the database: the category ids (with the routing destinations and
//! rules, and the `tout` of the horizons and ranges), the marker hues,
//! the cleanup scopes, the invitation replies. The database keeps the
//! French value (decision D3, debt D-54); the UI, the catalogue keys,
//! the CSS selectors and the test ids carry the English one. This
//! module is the ONLY place where the two meet: every command that
//! reads or writes such a value maps it here, at the boundary. The sync
//! phases are transient (never persisted) and were renamed in place.
//!
//! An unknown value passes through unchanged in both directions — the
//! core rejects what it does not know (closed vocabulary), the UI never
//! sees a value the core did not produce. The period codes of the
//! horizons and ranges (`1m`, `6m`, `1a`, `5a`) are such pass-through
//! values: codes, not words, the same on both sides.
//!
//! The tables below are tied to the core's own vocabularies by the
//! tests at the bottom: a value added to the core without its wire
//! word turns them red.

/// The category ids of the nav and the routing destinations (the
/// routing `destination` column, the `list_category` parameter, the
/// screener defaults and the horizon `tout`).
const CATEGORIES: &[(&str, &str)] = &[
    ("reception", "inbox"),
    ("envoyes", "sent"),
    ("brouillons", "drafts"),
    ("indesirables", "junk"),
    ("archives", "archive"),
    ("corbeille", "trash"),
    ("kiosque", "feed"),
    ("registre", "paper_trail"),
    ("ecarte", "screened_out"),
    ("tout", "all"),
];

/// The 12 marker hues (`prefs.repere_teinte.N`).
const HUES: &[(&str, &str)] = &[
    ("rouge", "red"),
    ("orange", "orange"),
    ("ocre", "ochre"),
    ("olive", "olive"),
    ("vert", "green"),
    ("sapin", "pine"),
    ("bleu", "blue"),
    ("indigo", "indigo"),
    ("violet", "violet"),
    ("magenta", "magenta"),
    ("rose", "pink"),
    ("brun", "brown"),
];

/// The routing rules of a "No" (`ROUTING_RULES` of the core: what
/// happens to the next messages of a screened-out sender) — `archive`
/// is a rule here and `archives` a category above: two vocabularies.
const RULES: &[(&str, &str)] = &[
    ("spam", "spam"),
    ("archive", "archive"),
    ("corbeille", "trash"),
];

/// The cleanup scopes (`CLEANUP_SCOPES` of the core).
const SCOPES: &[(&str, &str)] = &[
    ("reception", "inbox"),
    ("dossiers", "folders"),
    ("dossiersArchives", "foldersArchive"),
    ("archives", "archive"),
];

/// The invitation replies (`attendee_status`, `reply` of the stored
/// invitation, the `Participation` stable strings of the core).
const REPLIES: &[(&str, &str)] = &[
    ("accepte", "accepted"),
    ("provisoire", "tentative"),
    ("refuse", "declined"),
    ("sans_reponse", "no_reply"),
];

fn to_wire(table: &[(&str, &str)], db: &str) -> String {
    table
        .iter()
        .find(|(fr, _)| *fr == db)
        .map(|(_, en)| (*en).to_string())
        .unwrap_or_else(|| db.to_string())
}

fn from_wire(table: &[(&str, &str)], wire: &str) -> String {
    table
        .iter()
        .find(|(_, en)| *en == wire)
        .map(|(fr, _)| (*fr).to_string())
        .unwrap_or_else(|| wire.to_string())
}

pub fn category_to_wire(db: &str) -> String {
    to_wire(CATEGORIES, db)
}
pub fn category_from_wire(wire: &str) -> String {
    from_wire(CATEGORIES, wire)
}
pub fn hue_to_wire(db: &str) -> String {
    to_wire(HUES, db)
}
pub fn hue_from_wire(wire: &str) -> String {
    from_wire(HUES, wire)
}
pub fn scope_to_wire(db: &str) -> String {
    to_wire(SCOPES, db)
}
pub fn scope_from_wire(wire: &str) -> String {
    from_wire(SCOPES, wire)
}
pub fn reply_to_wire(db: &str) -> String {
    to_wire(REPLIES, db)
}
pub fn reply_from_wire(wire: &str) -> String {
    from_wire(REPLIES, wire)
}

pub fn rule_to_wire(db: &str) -> String {
    to_wire(RULES, db)
}
pub fn rule_from_wire(wire: &str) -> String {
    from_wire(RULES, wire)
}

/// A routing verdict as the UI sends it — a destination (category
/// table) and its optional rule (rule table).
pub fn destination_rule_from_wire(
    destination: &str,
    rule: Option<&str>,
) -> (String, Option<String>) {
    (category_from_wire(destination), rule.map(rule_from_wire))
}

/// The Screener's "No" default: a rule, or `ecarte` (screened out
/// without moving) — the rule table first, the category table for the
/// rest.
pub fn no_default_to_wire(db: &str) -> String {
    if RULES.iter().any(|(fr, _)| *fr == db) {
        rule_to_wire(db)
    } else {
        category_to_wire(db)
    }
}
pub fn no_default_from_wire(wire: &str) -> String {
    if RULES.iter().any(|(_, en)| *en == wire) {
        rule_from_wire(wire)
    } else {
        category_from_wire(wire)
    }
}

/// The English hue names the UI may send — the allowlist of
/// `marker_set`, compared by the System coherence net with the
/// `--mk-<hue>` tokens of `system.css`.
pub const WIRE_HUES: [&str; 12] = [
    "red", "orange", "ochre", "olive", "green", "pine", "blue", "indigo", "violet", "magenta",
    "pink", "brown",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_vocabulary_round_trips() {
        for table in [CATEGORIES, HUES, SCOPES, REPLIES] {
            for (fr, en) in table {
                assert_eq!(to_wire(table, fr), *en);
                assert_eq!(from_wire(table, en), *fr);
            }
        }
    }

    #[test]
    fn the_twelve_hues_are_the_wire_allowlist() {
        let wired: Vec<String> = HUES.iter().map(|(fr, _)| hue_to_wire(fr)).collect();
        assert_eq!(wired, WIRE_HUES);
        for (fr, en) in HUES {
            assert_eq!(hue_from_wire(en), *fr);
        }
    }

    #[test]
    fn an_unknown_value_passes_through_both_ways() {
        assert_eq!(category_to_wire("nettoyage"), "nettoyage");
        assert_eq!(category_from_wire("cleanup"), "cleanup");
        assert_eq!(scope_from_wire("1m"), "1m");
        assert_eq!(reply_to_wire(""), "");
    }

    fn french<'a>(table: &[(&'a str, &str)]) -> Vec<&'a str> {
        table.iter().map(|(fr, _)| *fr).collect()
    }

    #[test]
    fn the_tables_cover_the_core_vocabularies() {
        for scope in mail_core::CLEANUP_SCOPES {
            assert!(
                french(SCOPES).contains(scope),
                "cleanup scope {scope} has no wire word"
            );
        }
        for destination in mail_core::ECHO_DESTINATIONS {
            assert!(
                french(CATEGORIES).contains(destination),
                "{destination} has no wire word"
            );
        }
        for word in mail_core::HORIZONS_IMPORT
            .iter()
            .chain(mail_core::CLEANUP_RANGES)
        {
            // A period code passes through; only the word `tout` is mapped.
            assert!(*word == "tout" || category_to_wire(word) == *word);
        }
        assert_eq!(french(HUES), crate::commands::MARKER_HUES);
        for (fr, _) in REPLIES {
            assert!(
                mail_core::participation_de_stable(fr).is_some(),
                "{fr} is not a Participation"
            );
        }
    }

    #[test]
    fn a_rule_is_not_a_category() {
        // `archive` the rule stays `archive`; `archive` the category is
        // `archives` — the table decides, never a shared word.
        assert_eq!(rule_from_wire("archive"), "archive");
        assert_eq!(category_from_wire("archive"), "archives");
        assert_eq!(rule_from_wire("trash"), "corbeille");
        assert_eq!(
            destination_rule_from_wire("screened_out", Some("archive")),
            ("ecarte".into(), Some("archive".into()))
        );
        assert_eq!(no_default_from_wire("screened_out"), "ecarte");
        assert_eq!(no_default_from_wire("trash"), "corbeille");
        assert_eq!(no_default_to_wire("corbeille"), "trash");
        assert_eq!(no_default_to_wire("ecarte"), "screened_out");
        assert_eq!(no_default_to_wire("archive"), "archive");
    }
}

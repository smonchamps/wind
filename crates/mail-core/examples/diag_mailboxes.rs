//! Diagnostic P2 (UI redesign): do the six canonical folders of the
//! prototype — inbox, sent, drafts, junk, archive, trash — turn up in
//! the REAL mailboxes of each account?
//!
//! The nav of screen 02 shows only these six categories: this diagnostic
//! classifies the folders of the cache (`folders`) by canonical patterns
//! and flags what is missing or stays ambiguous. "Sent" is not guessed:
//! `accounts.sent_mailbox` is authoritative (ADR 0009 §7).
//!
//! House rule: nothing personal. Only names RECOGNIZED as canonical are
//! shown in clear; the others are counted and rendered as shape only
//! (initial + length). Addresses are masked.
//!
//! ```powershell
//! cargo run -p mail-core --example diag_mailboxes --release -- <path.db>
//! ```

use rusqlite::Connection;

// Lesson of the first pass on the real database: a plain `contains()`
// blew up — a Gmail account carrying a PST migration gave 26 "archive"
// candidates (`.../Archive/Sport`, etc.). The rule becomes POSITIONAL:
// only the LAST segment counts, and the folder must live at the root or
// under the sole provider prefix (`[Gmail]/x`) — never deeper. With
// multiple candidates, the provider prefix wins over the root homonym.
const CATEGORIES: &[(&str, &[&str])] = &[
    ("inbox", &["inbox"]),
    ("drafts", &["drafts", "brouillons"]), // lang:fr — "brouillons" is a real French server folder name
    (
        "junk",
        &[
            "spam",
            "junk",
            "junk e-mail",
            "courrier ind\u{e9}sirable", // lang:fr — real French server folder name
            "ind\u{e9}sirables",         // lang:fr — real French server folder name
        ],
    ),
    (
        "trash",
        &[
            "trash",
            "corbeille", // lang:fr — real French server folder name
            "deleted",
            "deleted items",
            "\u{e9}l\u{e9}ments supprim\u{e9}s", // lang:fr — real French server folder name
        ],
    ),
    (
        "archive",
        &["archive", "archives", "all mail", "tous les messages"], // lang:fr — "tous les messages" is a real French server folder name
    ),
];

/// Root, or exactly one level under `[Gmail]` — nothing deeper.
fn segments(display: &str) -> Option<(bool, String)> {
    let parts: Vec<&str> = display.split('/').collect();
    match parts.as_slice() {
        [only] => Some((false, only.to_lowercase())),
        [prefix, leaf] if prefix.eq_ignore_ascii_case("[Gmail]") => {
            Some((true, leaf.to_lowercase()))
        }
        _ => None,
    }
}

fn mask(name: &str) -> String {
    let initial = name.chars().next().unwrap_or('?');
    format!("{initial}···({} chars)", name.chars().count())
}

/// Wire paths CAN CONTAIN an address (folders migrated from another
/// account): redacted before any display — "diagnostics disclose
/// nothing", identifiers included.
fn without_address(name: &str) -> String {
    name.split_whitespace()
        .map(|word| {
            if word.contains('@') {
                "\u{2039}address\u{203a}".to_string()
            } else {
                word.split('/')
                    .map(|seg| {
                        if seg.contains('@') {
                            "\u{2039}address\u{203a}"
                        } else {
                            seg
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("/")
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: diag_mailboxes <path.db>")?;
    let conn = Connection::open(&path)?;

    let accounts: Vec<(i64, Option<String>)> = conn
        .prepare("SELECT id, sent_mailbox FROM accounts ORDER BY id")?
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_, _>>()?;
    println!("{} account(s)\n", accounts.len());

    for (account_id, sent) in accounts {
        let folders: Vec<(String, String, bool)> = conn
            .prepare(
                "SELECT wire, display, selectable FROM folders
                 WHERE account_id = ?1 ORDER BY display",
            )?
            .query_map([account_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<Result<_, _>>()?;
        println!("account #{account_id} — {} folder(s) cached", folders.len());

        match &sent {
            Some(name) => println!(
                "  sent         : {}  (accounts.sent_mailbox, authoritative)",
                without_address(name)
            ),
            None => println!("  sent         : ABSENT — sent_mailbox not declared"),
        }

        let mut classified: Vec<&str> = Vec::new();
        for (category, patterns) in CATEGORIES {
            let found: Vec<&(String, String, bool)> = folders
                .iter()
                .filter(|(_, display, _)| {
                    segments(display).is_some_and(|(_, leaf)| patterns.contains(&leaf.as_str()))
                })
                .collect();
            // Priority to the provider prefix: `[Gmail]/Trash` beats
            // the root homonym `Trash`.
            let chosen = found
                .iter()
                .find(|(_, display, _)| segments(display).is_some_and(|(gmail, _)| gmail))
                .or_else(|| found.first());
            match chosen {
                None => println!("  {category:<12} : NO folder recognized"),
                Some((wire, display, selectable)) => {
                    for (w, _, _) in &found {
                        classified.push(w);
                    }
                    let duplicates = found.len().saturating_sub(1);
                    println!(
                        "  {category:<12} : {} (wire {}{}{})",
                        without_address(display),
                        without_address(wire),
                        if *selectable { "" } else { ", NOT selectable" },
                        if duplicates > 0 {
                            format!(", {duplicates} homonym(s) discarded")
                        } else {
                            String::new()
                        }
                    );
                }
            }
        }

        let others: Vec<&(String, String, bool)> = folders
            .iter()
            .filter(|(wire, _, _)| {
                !classified.contains(&wire.as_str()) && Some(wire.as_str()) != sent.as_deref()
            })
            .collect();
        println!(
            "  unclassified : {} — {}",
            others.len(),
            others
                .iter()
                .map(|(_, display, _)| mask(display))
                .collect::<Vec<_>>()
                .join(", ")
        );
        println!();
    }
    Ok(())
}

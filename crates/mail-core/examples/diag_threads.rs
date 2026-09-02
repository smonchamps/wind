//! Diagnostic of grouping into conversations.
//!
//! Answers two questions only the real mailbox can settle:
//!
//! 1. has the header pass run, and what did it find?
//! 2. **which identifier** ties together the messages of an abnormally
//!    large thread?
//!
//! Same discipline as [`diag_index`]: no subject, no sender, no
//! content is read or shown. Technical identifiers are **masked** — only
//! their shape is shown (brackets, length, domain), which is enough to
//! point at the defect.
//!
//! ```powershell
//! cargo run -p mail-core --example diag_threads -- "$env:APPDATA\dev.elements.wind\wind.db"
//! ```

use rusqlite::{Connection, OptionalExtension};

/// Shows only the SHAPE of an identifier: brackets present or not,
/// length of the local part, domain. Enough to recognize a reused,
/// empty, or non-standard `Message-ID` without disclosing a single one.
fn shape(raw: &str) -> String {
    form(raw, true)
}

/// Shape of a DIRECTORY token.
///
/// Unlike [`shape`], this says nothing about brackets: the directory
/// only stores the canonical form, which has already stripped them.
/// Mentioning them would make "WITHOUT BRACKETS" show up on perfectly
/// normal identifiers — a false alarm, exactly what a diagnostic must
/// not produce.
fn shape_canonical(raw: &str) -> String {
    form(raw, false)
}

fn form(raw: &str, show_brackets: bool) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "(empty)".to_string();
    }
    // How many identifiers does this value carry? A `References` header
    // holds a whole chain of them, and `In-Reply-To` can hold several.
    let count = trimmed
        .matches('<')
        .count()
        .max(trimmed.split_whitespace().count());
    if count > 1 {
        // Do NOT describe it as a single identifier.
        //
        // Splitting on the FIRST "@" would make the rest of the string
        // pass for a domain — and show it IN CLEAR, while this module
        // promises to disclose none. Found on the real database: five
        // readable Message-IDs in a diagnostic's output.
        return format!(
            "chain of {count} identifiers, the first: {}",
            simple_form(first_identifier(trimmed), show_brackets)
        );
    }
    simple_form(trimmed, show_brackets)
}

/// The first identifier of a chain, brackets included if it has any.
fn first_identifier(raw: &str) -> &str {
    match raw.split_once('>') {
        // `+ 1`: keep the closing bracket, or the shape would say
        // "WITHOUT BRACKETS" of a perfectly normal identifier.
        Some((head, _)) if raw.starts_with('<') => &raw[..head.len() + 1],
        _ => raw.split_whitespace().next().unwrap_or(raw),
    }
}

/// The shape of ONE identifier, and only one. No recursion: the caller
/// has already isolated a single token.
fn simple_form(trimmed: &str, show_brackets: bool) -> String {
    let bracketed = trimmed.starts_with('<') && trimmed.ends_with('>');
    let inner = trimmed.trim_start_matches('<').trim_end_matches('>');
    let (local, domain) = match inner.split_once('@') {
        Some((local, domain)) => (local.chars().count(), domain.to_string()),
        None => (inner.chars().count(), "(no @)".to_string()),
    };
    let brackets = match (show_brackets, bracketed) {
        (false, _) => String::new(),
        (true, true) => "<…> ".to_string(),
        (true, false) => "WITHOUT BRACKETS ".to_string(),
    };
    format!("{brackets}local part {local} chars, domain \"{domain}\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The defect found on the real database: a whole `References`
    /// header described as a single identifier displayed everything
    /// after the first "@" — hence four Message-IDs in clear.
    #[test]
    fn a_references_chain_discloses_no_identifier() {
        let reference = "<a1b2@Spark> <c3d4@AM8P190.OUTLOOK.COM> <e5f6@mail.gmail.com>";
        let output = form(reference, true);

        assert!(output.contains("chain of 3 identifiers"));
        assert!(
            !output.contains("AM8P190.OUTLOOK.COM"),
            "an identifier of the chain leaked: {output}"
        );
        assert!(
            !output.contains("mail.gmail.com"),
            "an identifier of the chain leaked: {output}"
        );
        // The first stays described, masked: it is the one pointing at the defect.
        assert!(output.contains("domain \"Spark\""), "{output}");
    }

    /// A single identifier is described as before — the fix must not
    /// degrade the common case.
    #[test]
    fn a_single_identifier_keeps_its_shape() {
        let output = form("<abcdef@example.com>", true);
        assert_eq!(output, "<…> local part 6 chars, domain \"example.com\"");
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: diag_threads <path.db>")?;
    let opened = std::time::Instant::now();
    let conn = Connection::open(&path)?;
    println!("database: {path}");
    println!("open: {} ms\n", opened.elapsed().as_millis());

    let one = |sql: &str| -> rusqlite::Result<i64> { conn.query_row(sql, [], |row| row.get(0)) };

    let messages = one("SELECT COUNT(*) FROM envelopes")?;
    let threads = one("SELECT COUNT(*) FROM threads")?;
    let links = one("SELECT COUNT(*) FROM thread_links")?;
    println!("messages     : {messages}");
    println!("conversations: {threads}");
    println!("directory    : {links} identifiers\n");

    // 1. Has the header pass run?
    //
    // NULL = never read; '' = read, the message has no References;
    // non-empty = read, and it has some. The three are told apart,
    // otherwise we cannot tell whether the silence comes from the
    // server or from us.
    //
    // BROKEN DOWN BY SCOPE since ADR 0010. The pass only reads the
    // grouping mailboxes (INBOX + Sent); on a full database, a global
    // "never read" would mix real pending work with the hundreds of
    // thousands of out-of-scope messages it DELIBERATELY ignores. Found
    // on the first field run: 250,864 "never read", the overwhelming
    // majority of which would rightly never be read — a figure that
    // points at nothing sends the diagnostic back for nothing.
    println!("--- header pass (grouping scope) ---");
    for (state, sql) in [
        ("never read", "e.refs IS NULL"),
        ("read, no References", "e.refs = ''"),
        (
            "read, with References",
            "e.refs IS NOT NULL AND e.refs != ''",
        ),
    ] {
        let count = one(&format!(
            "SELECT COUNT(*) FROM envelopes e
             JOIN mailboxes m ON m.id = e.mailbox_id
             WHERE m.threaded = 1 AND {sql}"
        ))?;
        println!("{state:<24}: {count}");
    }
    let in_reply = one("SELECT COUNT(*) FROM envelopes e
         JOIN mailboxes m ON m.id = e.mailbox_id
         WHERE m.threaded = 1 AND e.in_reply_to IS NOT NULL")?;
    println!("{:<24}: {in_reply}", "with In-Reply-To");
    // Out of scope in ONE line, so the total cross-checks against
    // "messages" at the top of the output — without it, the breakdown
    // would seem to lose messages.
    let out_of_scope = one("SELECT COUNT(*) FROM envelopes e
         JOIN mailboxes m ON m.id = e.mailbox_id
         WHERE m.threaded = 0")?;
    println!("{:<24}: {out_of_scope}\n", "out of scope (ignored)");

    // 2. Size distribution — a giant thread is spotted at a glance.
    println!("--- conversation sizes ---");
    for (label, sql) in [
        ("1 message", "size <= 1"),
        ("2 to 5", "size BETWEEN 2 AND 5"),
        ("6 to 20", "size BETWEEN 6 AND 20"),
        ("more than 20", "size > 20"),
    ] {
        let count = one(&format!("SELECT COUNT(*) FROM threads WHERE {sql}"))?;
        println!("{label:<12}: {count}");
    }

    // 3. The biggest threads, and above all WHAT TIES THEM TOGETHER.
    //
    // If the 17 messages of a thread have only one distinct
    // `Message-ID`, the culprit is a sender reusing its own. If they
    // have only one `In-Reply-To` or one `References`, it is a shared
    // anchor — a campaign identifier, for instance. These three counts
    // point at the defect without showing a single value.
    println!("\n--- the biggest threads, and what ties them together ---");
    let mut stmt = conn.prepare(
        "SELECT t.id, t.size,
                (SELECT COUNT(DISTINCT message_id) FROM envelopes WHERE thread_id = t.id),
                (SELECT COUNT(DISTINCT in_reply_to) FROM envelopes WHERE thread_id = t.id),
                (SELECT COUNT(DISTINCT refs) FROM envelopes WHERE thread_id = t.id),
                (SELECT COUNT(*) FROM thread_links WHERE thread_id = t.id)
         FROM threads t ORDER BY t.size DESC LIMIT 5",
    )?;
    let biggest: Vec<(i64, i64, i64, i64, i64, i64)> = stmt
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
            ))
        })?
        .collect::<Result<_, _>>()?;

    for (id, size, ids, parents, refs, links) in biggest {
        println!(
            "\nthread #{id} — {size} messages | {ids} distinct Message-IDs \
             | {parents} distinct In-Reply-To | {refs} distinct References \
             | {links} directory entries"
        );
        // A single distinct identifier shared by the whole thread: that
        // is the tie. Only its shape is shown, never the value.
        for (label, column) in [
            ("Message-ID", "message_id"),
            ("In-Reply-To", "in_reply_to"),
            ("References", "refs"),
        ] {
            if size < 2 {
                continue;
            }
            let common: Option<String> = conn
                .query_row(
                    &format!(
                        "SELECT {column} FROM envelopes
                         WHERE thread_id = ?1 AND {column} IS NOT NULL AND {column} != ''
                         GROUP BY {column} HAVING COUNT(*) > 1
                         ORDER BY COUNT(*) DESC LIMIT 1"
                    ),
                    [id],
                    |row| row.get(0),
                )
                .optional()?;
            if let Some(value) = common {
                let shared: i64 = conn.query_row(
                    &format!(
                        "SELECT COUNT(*) FROM envelopes WHERE thread_id = ?1 AND {column} = ?2"
                    ),
                    rusqlite::params![id, value],
                    |row| row.get(0),
                )?;
                println!("  {label} shared by {shared} messages: {}", shape(&value));
            }
        }
    }

    // 3 bis. THE ANCHORS — the real question.
    //
    // Comparing whole headers is not enough: two messages whose
    // `References` differ end to end can still cite the same ancestor.
    // It is THAT token that ties them together, and a single wrong
    // anchor collapses everything it touches, step by step.
    //
    // So we start over from the directory, which holds the tokens as
    // the grouping kept them.
    println!("\n--- anchors of the two biggest threads ---");
    let mut stmt = conn.prepare("SELECT id FROM threads ORDER BY size DESC LIMIT 2")?;
    let tops: Vec<i64> = stmt
        .query_map([], |row| row.get(0))?
        .collect::<Result<_, _>>()?;

    for thread in tops {
        let mut stmt = conn.prepare("SELECT message_id FROM thread_links WHERE thread_id = ?1")?;
        let tokens: Vec<String> = stmt
            .query_map([thread], |row| row.get(0))?
            .collect::<Result<_, _>>()?;

        // `instr` and not `LIKE`: an identifier readily contains `_`,
        // which `LIKE` would read as a wildcard.
        let mut scope: Vec<(i64, bool, String)> = Vec::new();
        for token in &tokens {
            let cited: i64 = conn.query_row(
                "SELECT COUNT(*) FROM envelopes
                 WHERE thread_id = ?1
                   AND (instr(COALESCE(message_id, ''), ?2) > 0
                     OR instr(COALESCE(in_reply_to, ''), ?2) > 0
                     OR instr(COALESCE(refs, ''), ?2) > 0)",
                rusqlite::params![thread, token],
                |row| row.get(0),
            )?;
            // An anchor that NOBODY owns is a phantom: no message in
            // the database is named that way. Legitimate when the
            // ancestor is elsewhere (in "Sent"), suspect when dozens of
            // unrelated messages latch onto it.
            let owned: i64 = conn.query_row(
                "SELECT COUNT(*) FROM envelopes WHERE instr(COALESCE(message_id, ''), ?1) > 0",
                [token],
                |row| row.get(0),
            )?;
            scope.push((cited, owned > 0, token.clone()));
        }
        scope.sort_by_key(|entry| std::cmp::Reverse(entry.0));

        println!("\nthread #{thread} — {} directory tokens", tokens.len());
        for (cited, owned, token) in scope.iter().take(5) {
            let nature = if *owned {
                "owned by a message"
            } else {
                "PHANTOM (nobody carries it)"
            };
            println!(
                "  cited by {cited} messages — {nature} — {}",
                shape_canonical(token)
            );
        }
    }

    // 4. The classic trap: a sender reusing its Message-ID.
    println!("\n--- reused Message-IDs (whole database) ---");
    let mut stmt = conn.prepare(
        "SELECT message_id, COUNT(*) FROM envelopes
         WHERE message_id IS NOT NULL AND message_id != ''
         GROUP BY message_id HAVING COUNT(*) > 1
         ORDER BY COUNT(*) DESC LIMIT 5",
    )?;
    let duplicates: Vec<(String, i64)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_, _>>()?;
    if duplicates.is_empty() {
        println!("none — every message has its own");
    }
    for (value, count) in duplicates {
        println!("{count} messages share a Message-ID: {}", shape(&value));
    }

    Ok(())
}

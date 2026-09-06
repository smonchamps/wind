//! PLAN-AUDIT-V2 E3 — what the adapter SENDS, proven on the scripted fake
//! server ([`crate::fake_server`]). Every test was played RED against the
//! adapter of before (see the PLAN).

use mail_core::MailServer;

use crate::fake_server::{FakeImap, Script, literal, uids_of};

fn tiny_mime(uid: u32) -> String {
    format!(
        "From: alice@ex.fr\r\nSubject: message {uid}\r\nContent-Type: text/plain\r\n\r\nhello {uid}\r\n"
    )
}

fn envelope(uid: u32) -> String {
    format!(
        "* {uid} FETCH (UID {uid} FLAGS (\\Seen) INTERNALDATE \"01-Jan-2026 00:00:00 +0000\" \
         ENVELOPE (\"Thu, 1 Jan 2026 00:00:00 +0000\" \"Subject {uid}\" \
         ((\"Alice\" NIL \"alice\" \"ex.fr\")) NIL NIL NIL NIL NIL NIL \"<m{uid}@ex.fr>\"))"
    )
}

/// The backfill bench: 50 multipart bodies of ~56 KB (text, HTML, a 30 KB
/// base64 attachment) through the parse — the dominant CPU post of the
/// backfill. `cargo test -p mail-imap bench_parse -- --ignored --nocapture`.
#[test]
#[ignore = "bench: a measurement, not a net"]
fn bench_parse_50_bodies() {
    let attachment = "QUJDRA==".repeat(30 * 1024 / 8);
    let html = "<p>Hello everyone, here is the letter of the month.</p>".repeat(400);
    let raw = format!(
        "From: alice@ex.fr\r\nTo: bob@ex.fr\r\nSubject: letter\r\nMIME-Version: 1.0\r\n\
         Content-Type: multipart/mixed; boundary=\"b1\"\r\n\r\n\
         --b1\r\nContent-Type: multipart/alternative; boundary=\"b2\"\r\n\r\n\
         --b2\r\nContent-Type: text/plain\r\n\r\nHello everyone\r\n\
         --b2\r\nContent-Type: text/html\r\n\r\n{html}\r\n--b2--\r\n\
         --b1\r\nContent-Type: application/pdf; name=\"doc.pdf\"\r\n\
         Content-Disposition: attachment; filename=\"doc.pdf\"\r\n\
         Content-Transfer-Encoding: base64\r\n\r\n{attachment}\r\n--b1--\r\n"
    );
    println!("body: {} KB", raw.len() / 1024);
    let start = std::time::Instant::now();
    let mut attachments = 0;
    for _ in 0..50 {
        let body = crate::body_from_raw(raw.as_bytes()).expect("parseable");
        attachments += body.attachments.len();
    }
    println!(
        "50 bodies parsed in {:?} ({attachments} attachments seen)",
        start.elapsed()
    );
}

#[test]
fn thread_headers_ask_for_three_fields_only() {
    let mut script = Script::simple();
    script.fetch = Box::new(|command| {
        let text = "Message-ID: <m1@ex.fr>\r\nReferences: <a@ex.fr> <b@ex.fr>\r\n\r\n";
        uids_of(command)
            .into_iter()
            .map(|uid| {
                format!(
                    "* {uid} FETCH (UID {uid} BODY[HEADER.FIELDS (MESSAGE-ID IN-REPLY-TO REFERENCES)] {})",
                    literal(text)
                )
            })
            .collect()
    });
    let fake = FakeImap::start(script);
    let mut server = fake.connect();

    let read = server.fetch_thread_headers("INBOX", &[1]).unwrap();
    assert_eq!(read.len(), 1);
    assert_eq!(read[0].1.references.as_deref(), Some("<a@ex.fr> <b@ex.fr>"));

    let fetch = fake
        .commands()
        .into_iter()
        .find(|c| c.starts_with("UID FETCH"))
        .expect("a FETCH went out");
    assert!(
        fetch.contains("BODY.PEEK[HEADER.FIELDS (MESSAGE-ID IN-REPLY-TO REFERENCES)]"),
        "the whole header block is requested: {fetch}"
    );
}

#[test]
fn a_body_batch_is_bounded_to_32_mb() {
    let mut script = Script::simple();
    script.fetch = Box::new(|command| {
        let uids = uids_of(command);
        if command.contains("RFC822.SIZE") {
            // 20 MB, 20 MB, 1 KB: the first two do not fit together under
            // 32 MB, the third follows the second.
            uids.into_iter()
                .map(|uid| {
                    let size = if uid == 3 { 1024 } else { 20 * 1024 * 1024 };
                    format!("* {uid} FETCH (UID {uid} RFC822.SIZE {size})")
                })
                .collect()
        } else {
            uids.into_iter()
                .map(|uid| {
                    format!(
                        "* {uid} FETCH (UID {uid} BODY[] {})",
                        literal(&tiny_mime(uid))
                    )
                })
                .collect()
        }
    });
    let fake = FakeImap::start(script);
    let mut server = fake.connect();

    let bodies = server.fetch_bodies_html("INBOX", &[1, 2, 3]).unwrap();
    assert_eq!(bodies.len(), 3, "the three bodies arrive");

    let batches: Vec<String> = fake
        .commands()
        .into_iter()
        .filter(|c| c.contains("BODY.PEEK[]"))
        .collect();
    assert_eq!(batches.len(), 2, "two batches expected, seen: {batches:?}");
    assert!(batches[0].starts_with("UID FETCH 1 "), "{}", batches[0]);
    assert!(batches[1].starts_with("UID FETCH 2:3 "), "{}", batches[1]);
}

#[test]
fn an_unsupported_move_is_refused_before_any_mutation() {
    let mut script = Script::simple();
    script.capabilities = "IMAP4rev1".to_string();
    let fake = FakeImap::start(script);
    let mut server = fake.connect();

    let outcome = server.move_to("INBOX", 1, "Archive");

    let commands = fake.commands();
    assert!(
        matches!(outcome, Err(mail_core::Error::Refusal(_))),
        "{outcome:?}"
    );
    assert_no_mutations(&commands);
}

fn assert_no_mutations(commands: &[String]) {
    assert!(
        !commands.iter().any(|c| c.starts_with("UID COPY")
            || c.starts_with("UID MOVE")
            || c.starts_with("UID STORE")
            || c.contains("EXPUNGE")),
        "unexpected mutation: {commands:?}"
    );
}

#[test]
fn move_only_servers_share_one_safe_path_for_archive_and_trash() {
    let mut script = Script::simple();
    script.capabilities = "IMAP4rev1 MOVE".to_string();
    let fake = FakeImap::start(script);
    let mut server = fake.connect();
    server.archive("INBOX", 1).unwrap();
    server.delete("INBOX", 2).unwrap();
    let commands = fake.commands();
    assert!(
        commands.contains(&"UID MOVE 1 \"Archive\"".to_string()),
        "{commands:?}"
    );
    assert!(
        commands.contains(&"UID MOVE 2 \"Corbeille\"".to_string()),
        "{commands:?}"
    );
    assert!(
        !commands
            .iter()
            .any(|c| c.starts_with("UID COPY") || c.contains("EXPUNGE")),
        "{commands:?}"
    );
}

#[test]
fn draft_purge_without_uidplus_never_sets_deleted() {
    let mut script = Script::simple();
    script.capabilities = "IMAP4rev1 MOVE".to_string();
    let fake = FakeImap::start(script);
    let mut server = fake.connect();
    let outcome = server.delete_draft_remote(1);
    assert!(
        matches!(outcome, Err(mail_core::Error::Refusal(_))),
        "{outcome:?}"
    );
    assert_no_mutations(&fake.commands());
}

#[test]
fn copy_fallback_quotes_the_entire_destination_and_scopes_expunge() {
    let mut script = Script::simple();
    script.capabilities = "IMAP4rev1 UIDPLUS".to_string();
    let fake = FakeImap::start(script);
    let mut server = fake.connect();
    server.move_to("INBOX", 1, "Client \"A\"\\2026").unwrap();
    let commands = fake.commands();
    assert!(
        commands.contains(&"UID COPY 1 \"Client \\\"A\\\"\\\\2026\"".to_string()),
        "{commands:?}"
    );
    assert!(
        commands.contains(&"UID EXPUNGE 1".to_string()),
        "{commands:?}"
    );
    assert!(!commands.iter().any(|c| c == "EXPUNGE"), "{commands:?}");
}

#[test]
fn unsafe_destination_bytes_are_refused_without_a_command() {
    for target in ["Archive\r\na99 EXPUNGE", "Archive\n", "Archive\0"] {
        let mut script = Script::simple();
        script.capabilities = "IMAP4rev1 UIDPLUS".to_string();
        let fake = FakeImap::start(script);
        let mut server = fake.connect();
        assert!(server.move_to("INBOX", 1, target).is_err());
        assert_no_mutations(&fake.commands());
    }
}

#[test]
fn generic_all_mail_is_not_evidence_that_expunge_preserves_content() {
    let mut script = Script::simple();
    script.list = vec!["* LIST (\\All) \"/\" \"Everywhere\"".to_string()];
    let fake = FakeImap::start(script);
    let mut server = fake.connect();
    let outcome = server.archive("INBOX", 1);
    assert!(
        matches!(outcome, Err(mail_core::Error::Refusal(_))),
        "{outcome:?}"
    );
    assert_no_mutations(&fake.commands());
}

#[test]
fn a_generic_all_role_does_not_hide_a_usable_archive_destination() {
    let mut script = Script::simple();
    script
        .list
        .push("* LIST (\\All) \"/\" \"Everywhere\"".to_string());
    let fake = FakeImap::start(script);
    let mut server = fake.connect();
    server.archive("INBOX", 1).unwrap();
    assert!(
        fake.commands()
            .contains(&"UID MOVE 1 \"Archive\"".to_string())
    );
}

#[test]
fn gmail_label_archive_requires_inbox_and_targeted_expunge() {
    for (capabilities, mailbox, allowed) in [
        ("IMAP4rev1 UIDPLUS X-GM-EXT-1", "INBOX", true),
        ("IMAP4rev1 UIDPLUS X-GM-EXT-1", "Trash", false),
        ("IMAP4rev1 X-GM-EXT-1", "INBOX", false),
    ] {
        let mut script = Script::simple();
        script.capabilities = capabilities.to_string();
        script.list = vec!["* LIST (\\All) \"/\" \"All Mail\"".to_string()];
        let fake = FakeImap::start(script);
        let mut server = fake.connect();
        assert_eq!(server.archive(mailbox, 1).is_ok(), allowed);
        if allowed {
            let commands = fake.commands();
            assert!(commands.contains(&"UID EXPUNGE 1".to_string()));
            assert!(!commands.iter().any(|c| c == "EXPUNGE"));
        } else {
            assert_no_mutations(&fake.commands());
        }
    }
}

#[test]
fn a_session_lists_only_once_for_the_special_folders() {
    let fake = FakeImap::start(Script::simple());
    let mut server = fake.connect();

    assert_eq!(
        server.drafts_folder_name().unwrap().as_deref(),
        Some("Brouillons")
    );
    assert_eq!(
        server.sent_folder_name().unwrap().as_deref(),
        Some("Envoyes")
    );
    server.delete("INBOX", 1).unwrap(); // the trash, third reader

    let lists = fake
        .commands()
        .iter()
        .filter(|c| c.starts_with("LIST"))
        .count();
    assert_eq!(lists, 1, "one LIST per session: {:?}", fake.commands());
}

#[test]
fn a_session_queries_capability_only_once() {
    let fake = FakeImap::start(Script::simple());
    let mut server = fake.connect();

    let _ = server.changes_since("INBOX", 5).unwrap(); // CONDSTORE
    server.move_to("INBOX", 1, "Archive").unwrap(); // MOVE

    let capabilities = fake
        .commands()
        .iter()
        .filter(|c| c.starts_with("CAPABILITY"))
        .count();
    assert_eq!(
        capabilities,
        1,
        "one CAPABILITY per session: {:?}",
        fake.commands()
    );
}

#[test]
fn changes_are_requested_as_flags_then_as_envelopes_by_batches() {
    let mut script = Script::simple();
    script.fetch = Box::new(|command| {
        if command.contains("CHANGEDSINCE") {
            (1..=501)
                .map(|uid| format!("* {uid} FETCH (UID {uid} FLAGS (\\Seen) MODSEQ (6))"))
                .collect()
        } else {
            uids_of(command).into_iter().map(envelope).collect()
        }
    });
    let fake = FakeImap::start(script);
    let mut server = fake.connect();

    let envelopes = server.changes_since("INBOX", 5).unwrap().unwrap();
    assert_eq!(envelopes.len(), 501);
    assert_eq!(envelopes[0].subject.as_deref(), Some("Subject 1"));

    let fetches: Vec<String> = fake
        .commands()
        .into_iter()
        .filter(|c| c.starts_with("UID FETCH"))
        .collect();
    assert_eq!(
        fetches[0], "UID FETCH 1:* (UID FLAGS) (CHANGEDSINCE 5)",
        "the flags first, without envelope"
    );
    assert_eq!(
        fetches.len(),
        3,
        "then two batches of envelopes: {fetches:?}"
    );
    assert!(
        fetches[1].starts_with("UID FETCH 1:500 (UID ENVELOPE"),
        "{}",
        fetches[1]
    );
    assert!(
        fetches[2].starts_with("UID FETCH 501 (UID ENVELOPE"),
        "{}",
        fetches[2]
    );
}

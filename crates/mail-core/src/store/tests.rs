use chrono::{TimeZone, Utc};

use super::*;

fn envelope(uid: Uid, subject: &str, epoch: i64, seen: bool) -> Envelope {
    Envelope {
        reply_to: None,
        uid,
        subject: Some(subject.to_string()),
        sender: Some("Alice Martin".to_string()),
        sender_address: Some("alice@example.com".to_string()),
        message_id: Some(format!("<m{uid}@example.com>")),
        in_reply_to: None,
        date: Some(Utc.timestamp_opt(epoch, 0).unwrap()),
        seen,
        flagged: uid.is_multiple_of(2),
        to_addrs: Vec::new(),
        cc_addrs: Vec::new(),
    }
}

fn test_account(store: &Store) -> i64 {
    store
        .adopt_or_create_account("test@exemple.fr", "gmail")
        .unwrap()
}

fn store_with_mailbox() -> (Store, i64) {
    let store = Store::open_in_memory().unwrap();
    let account = test_account(&store);
    let id = store.create_mailbox(account, "INBOX", 1).unwrap();
    (store, id)
}

/// Every "per message" table filled for a UID: what every purge must
/// carry away (PLAN-AUDIT-V1 E4).
fn fill_message(store: &mut Store, inbox: i64, uid: Uid) {
    store
        .upsert_envelopes(inbox, &[envelope(uid, "subject", 100, false)])
        .unwrap();
    store.save_body(inbox, uid, "<p>body</p>", &[]).unwrap();
    let conn = store.conn();
    conn.execute(
            "INSERT INTO attachments (mailbox_id, uid, idx, name, mime, size) VALUES (?1, ?2, 0, 'a.pdf', 'application/pdf', 1)",
            params![inbox, uid],
        )
        .unwrap();
    conn.execute(
            "INSERT INTO invitations (mailbox_id, uid, methode, event_uid) VALUES (?1, ?2, 'REQUEST', 'evt')",
            params![inbox, uid],
        )
        .unwrap();
    for table in ["images_messages", "mis_de_cote", "kiosque_lus"] {
        conn.execute(
            &format!("INSERT INTO {table} (mailbox_id, uid, epoch) VALUES (?1, ?2, 1)"),
            params![inbox, uid],
        )
        .unwrap();
    }
}

/// How many rows, across every per-message table, still carry this
/// UID.
fn message_rows(store: &Store, inbox: i64, uid: Uid) -> Vec<(&'static str, i64)> {
    [
        "envelopes",
        "bodies",
        "attachments",
        "invitations",
        "images_messages",
        "mis_de_cote",
        "kiosque_lus",
    ]
    .into_iter()
    .map(|table| {
        let n: i64 = store
            .conn()
            .query_row(
                &format!("SELECT COUNT(*) FROM {table} WHERE mailbox_id = ?1 AND uid = ?2"),
                params![inbox, uid],
                |row| row.get(0),
            )
            .unwrap();
        (table, n)
    })
    .filter(|(_, n)| *n > 0)
    .collect()
}

/// Audit 2026-09-01 S2 (E4): `remove_absent` only purged 3 tables out
/// of 7 — a message gone from the server left attachments, invitation,
/// image memory, set-aside and Feed "read" orphaned (no foreign key on
/// `envelopes`). ONE list, the same for all three purges.
#[test]
fn a_message_gone_from_the_server_leaves_no_orphan() {
    let (mut store, inbox) = store_with_mailbox();
    fill_message(&mut store, inbox, 1);
    assert_eq!(message_rows(&store, inbox, 1).len(), 7, "fixture filled");

    let removed = store.remove_absent(inbox, &HashSet::new()).unwrap();

    assert_eq!(removed, 1);
    assert_eq!(
        message_rows(&store, inbox, 1),
        Vec::<(&str, i64)>::new(),
        "no row must survive the message"
    );
}

/// A SQLite trigger that refuses envelope deletion simulates a failure
/// in the middle of the purge: everything that came before it (body,
/// actions…) must be ROLLED BACK. Before E4, `reset_mailbox` chained
/// nine autocommit writes — a crash between two of them left threads
/// without envelopes (the "badge in front of an empty list" already
/// paid for at organized mode's E5).
fn block_envelope_deletions(store: &Store) {
    store
        .conn()
        .execute_batch(
            "CREATE TEMP TRIGGER panne BEFORE DELETE ON envelopes
                 BEGIN SELECT RAISE(ABORT, 'panne simulee'); END;",
        )
        .unwrap();
}

#[test]
fn reset_mailbox_is_atomic() {
    let (mut store, inbox) = store_with_mailbox();
    fill_message(&mut store, inbox, 1);
    store.enqueue_action(inbox, 1, Action::MarkSeen).unwrap();
    block_envelope_deletions(&store);

    assert!(
        store.reset_mailbox(inbox, 2).is_err(),
        "the failure must propagate"
    );

    assert_eq!(
        message_rows(&store, inbox, 1).len(),
        7,
        "nothing was erased before the failure: a single transaction"
    );
    assert_eq!(store.pending_actions(inbox).unwrap().len(), 1);
    assert_eq!(
        store
            .sync_state(test_account(&store), "INBOX")
            .unwrap()
            .unwrap()
            .uid_validity,
        1,
        "the UIDVALIDITY did not move either"
    );
}

#[test]
fn remove_local_is_atomic() {
    let (mut store, inbox) = store_with_mailbox();
    fill_message(&mut store, inbox, 1);
    block_envelope_deletions(&store);

    assert!(store.remove_local(inbox, 1).is_err());

    assert_eq!(
        message_rows(&store, inbox, 1).len(),
        7,
        "body, attachments, invitation… all still there: rolled back with the envelope"
    );
}

/// PLAN-AUDIT-V1 review: a refused action is not eternal — a fresh
/// gesture from the user on the same message replaces it, and the
/// screener-waiting row falls back down.
#[test]
fn a_new_gesture_replaces_the_old_refused_action() {
    let (store, id) = store_with_mailbox();
    store
        .enqueue_action(id, 1, Action::MoveTo("Gone".to_string()))
        .unwrap();
    let refused = store.pending_actions(id).unwrap().remove(0).id;
    store.refuse_action(refused, "[TRYCREATE]").unwrap();
    assert_eq!(store.refused_actions().unwrap(), 1);

    store.enqueue_action(id, 1, Action::MarkSeen).unwrap();

    assert_eq!(store.refused_actions().unwrap(), 0, "replaced");
    let queue = store.pending_actions(id).unwrap();
    assert_eq!(queue.len(), 1);
    assert_eq!(queue[0].action, Action::MarkSeen);
}

/// Audit 2026-09-01 (PLAN-AUDIT-V1 E3): a `pending_actions` row with an
/// unreadable `kind` (future version, corruption) made the WHOLE
/// `pending_actions(mailbox_id)` fail — the entire queue jammed by one
/// row. It is quarantined with its reason, the queue goes on.
#[test]
fn an_unreadable_row_does_not_fail_the_whole_queue() {
    let (store, id) = store_with_mailbox();
    store.enqueue_action(id, 1, Action::MarkSeen).unwrap();
    store
        .conn()
        .execute(
            "INSERT INTO pending_actions (mailbox_id, uid, kind) VALUES (?1, 2, 'teleporter')",
            [id],
        )
        .unwrap();
    store.enqueue_action(id, 3, Action::Archive).unwrap();

    let queue = store.pending_actions(id).unwrap();
    assert_eq!(
        queue.iter().map(|p| p.uid).collect::<Vec<_>>(),
        vec![1, 3],
        "the readable ones pass, the unreadable one is set aside"
    );
    assert_eq!(store.refused_actions().unwrap(), 1);
    // Idempotent: a second read does not recount it.
    store.pending_actions(id).unwrap();
    assert_eq!(store.refused_actions().unwrap(), 1);
}

/// D-36 (closed at the 2026-09-01 audit): a `\n` inside a `--` comment
/// of the `SCHEMA` literal became a real newline, SQLite swallowed the
/// rest of the comment as a COLUMN, and every FRESH database was born
/// with a phantom column in `echos`. The missing net: every column of
/// every table of a fresh database carries a sane name — an
/// identifier, never a scrap of sentence.
#[test]
fn a_fresh_database_has_no_phantom_column() {
    let store = Store::open_in_memory().unwrap();
    let conn = store.conn();
    let mut tables = conn
        .prepare(
            "SELECT name FROM sqlite_master \
                 WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
        )
        .unwrap();
    let names: Vec<String> = tables
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(
        names.iter().any(|t| t == "echos"),
        "the echos table is missing"
    );
    for table in names {
        let mut columns = conn
            .prepare(&format!("PRAGMA table_info(\"{table}\")"))
            .unwrap();
        let column_names: Vec<String> = columns
            .query_map([], |row| row.get(1))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        for column in &column_names {
            assert!(
                column
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
                "phantom column \"{column}\" in {table}: {column_names:?}"
            );
        }
    }
}

fn recent(store: &Store, offset: usize, limit: usize) -> Vec<Envelope> {
    store
        .recent(test_account(store), "INBOX", offset, limit)
        .unwrap()
}

/// R4: the To/Cc recipients written at sync read back exactly as
/// written — it is what the Sent folder displays (the sender there is
/// SELF) and what "Reply all" reads back offline. The "Test Attachment
/// 3" case: a send to a third-party address.
#[test]
fn upsert_persists_the_recipients() {
    let (mut store, id) = store_with_mailbox();
    let mut env = envelope(1, "Test Attachment 3", 1_700_000_000, true);
    env.to_addrs = vec!["sebastien.monchamps@gmail.com".to_string()];
    env.cc_addrs = vec![
        "copie1@exemple.fr".to_string(),
        "copie2@exemple.fr".to_string(),
    ];
    store
        .upsert_envelopes(id, std::slice::from_ref(&env))
        .unwrap();
    assert_eq!(recent(&store, 0, 10), vec![env]);
}

/// A preference never set answers the requested default; set, it
/// reads back exactly as written and overwrites without duplicating.
#[test]
fn bool_pref_default_then_roundtrip() {
    let store = Store::open_in_memory().unwrap();
    assert!(store.bool_pref("arrival_bubbles", true).unwrap());
    assert!(!store.bool_pref("arrival_bubbles", false).unwrap());
    store.set_bool_pref("arrival_bubbles", false).unwrap();
    assert!(!store.bool_pref("arrival_bubbles", true).unwrap());
    store.set_bool_pref("arrival_bubbles", true).unwrap();
    assert!(store.bool_pref("arrival_bubbles", false).unwrap());
}

/// The marker of the guarded poll (ADR 0017): never set -> `None` (a
/// legacy database polls everything on its first cycle), set -> read
/// back.
#[test]
fn remote_uidnext_absent_then_set() {
    let store = Store::open_in_memory().unwrap();
    let account = store
        .adopt_or_create_account("a@exemple.fr", "gmail")
        .unwrap();
    let mailbox = store.create_mailbox(account, "INBOX", 1).unwrap();
    // NULL as long as no guarded poll has happened: a legacy database
    // polls everything on its first cycle (ADR 0017).
    assert_eq!(store.remote_uidnext(mailbox).unwrap(), None);
    store.set_remote_uidnext(mailbox, 101).unwrap();
    assert_eq!(store.remote_uidnext(mailbox).unwrap(), Some(101));
    assert_eq!(store.envelope_count(mailbox).unwrap(), 0);
    assert!(!store.has_pending_actions(mailbox).unwrap());
}

/// A departure pending replay (archive, deletion, move) no longer
/// counts in the progress denominator: the gesture removes the local
/// row immediately (echo, PLAN-REACTIVITE E3) but `remote_total` dates
/// from the last SELECT — without the adjustment, a SINGLE triage was
/// enough to freeze progress at 99% and the status bar's hitofude
/// stroke with it (field 2026-08-15, PLAN-GELS: 5 archives + 1 pending
/// deletion = 99% for the whole duration of the replay). The real
/// gesture path is called (`gesture_with_echo`), never a simulation.
#[test]
fn a_departure_pending_replay_no_longer_counts_in_the_denominator() {
    let (mut store, id) = store_with_mailbox();
    store
        .upsert_envelopes(
            id,
            &[
                envelope(1, "stays", 100, true),
                envelope(2, "leaves for archive", 200, true),
                envelope(3, "stays too", 300, false),
            ],
        )
        .unwrap();
    store.record_remote_total(id, 3).unwrap();
    assert_eq!(store.sync_progress().unwrap(), (3, 3));
    // The triage: the echo removes the row, the action awaits its replay.
    store
        .gesture_with_echo(id, 2, Action::Archive, Some("archives"))
        .unwrap();
    assert_eq!(
        store.sync_progress().unwrap(),
        (2, 2),
        "the locally archived message must no longer be awaited"
    );
    // Marking as pending removes nothing from the mailbox: it does not
    // touch the denominator.
    store.enqueue_action(id, 1, Action::MarkSeen).unwrap();
    assert_eq!(store.sync_progress().unwrap(), (2, 2));
    // A move also removes; and the denominator never drops below zero
    // even when `remote_total` is behind.
    store
        .gesture_with_echo(id, 3, Action::MoveTo("Invoices".into()), None)
        .unwrap();
    store.record_remote_total(id, 1).unwrap();
    assert_eq!(store.sync_progress().unwrap(), (1, 0));
}

/// The text counterpart: never set -> `None` (the default belongs to
/// the caller), set -> read back exactly as written, overwritten
/// without duplicating.
#[test]
fn text_pref_none_then_roundtrip() {
    let store = Store::open_in_memory().unwrap();
    assert_eq!(store.text_pref("lang").unwrap(), None);
    store.set_text_pref("lang", "en").unwrap();
    assert_eq!(store.text_pref("lang").unwrap(), Some("en".to_string()));
    store.set_text_pref("lang", "fr").unwrap();
    assert_eq!(store.text_pref("lang").unwrap(), Some("fr".to_string()));
}

/// The transactional batch: everything written, everything read
/// back — the multi-key counterpart of `text_pref_none_then_roundtrip`.
#[test]
fn set_text_prefs_writes_the_whole_batch() {
    let mut store = Store::open_in_memory().unwrap();
    store
        .set_text_prefs(&[("repere_icone.1", "home"), ("repere_teinte.1", "bleu")])
        .unwrap();
    assert_eq!(
        store.text_pref("repere_icone.1").unwrap(),
        Some("home".to_string())
    );
    assert_eq!(
        store.text_pref("repere_teinte.1").unwrap(),
        Some("bleu".to_string())
    );
}

#[test]
fn roundtrips_all_envelope_fields() {
    let (mut store, id) = store_with_mailbox();
    let original = envelope(7, "Sujet accentué : été", 1_700_000_000, true); // lang:fr
    store
        .upsert_envelopes(id, std::slice::from_ref(&original))
        .unwrap();
    assert_eq!(recent(&store, 0, 10), vec![original]);
}

#[test]
fn roundtrips_envelope_without_optional_fields() {
    let (mut store, id) = store_with_mailbox();
    let bare = Envelope {
        reply_to: None,
        uid: 1,
        subject: None,
        sender: None,
        sender_address: None,
        message_id: None,
        in_reply_to: None,
        date: None,
        seen: false,
        flagged: false,
        to_addrs: Vec::new(),
        cc_addrs: Vec::new(),
    };
    store
        .upsert_envelopes(id, std::slice::from_ref(&bare))
        .unwrap();
    assert_eq!(recent(&store, 0, 10), vec![bare]);
}

/// The backfill order is a PRODUCT choice, not an accident of SQL
/// sort: INBOX first, Sent next, the rest by name. A server that
/// lists "Archive" before INBOX must not backfill 80,000 archive
/// bodies before the mail the list displays.
#[test]
fn mailboxes_backfill_inbox_first() {
    let store = Store::open_in_memory().unwrap();
    let account = store
        .adopt_or_create_account("moi@exemple.fr", "gmail")
        .unwrap();
    store.create_mailbox(account, "Archive", 1).unwrap();
    store.create_mailbox(account, "Corbeille", 1).unwrap(); // lang:fr
    store.create_mailbox(account, "INBOX", 1).unwrap();
    store
        .create_mailbox(account, "Messages envoyés", 1) // lang:fr
        .unwrap();
    store
        .set_thread_scope(account, Some("Messages envoyés")) // lang:fr
        .unwrap();

    assert_eq!(
        store.mailbox_names(account).unwrap(),
        vec!["INBOX", "Messages envoyés", "Archive", "Corbeille"] // lang:fr
    );
}

/// The import horizon (PLAN-HORIZON-NETTOYAGE, D1-D4): a per-account
/// pref with a CLOSED vocabulary; no pref -> "tout" (all) — an
/// account from before the setting keeps the full import (D4); the
/// value dies with the account, and a reused rowid does not inherit
/// it (PREFS_PAR_COMPTE).
#[test]
fn horizon_import_defaults_to_all_closed_vocabulary_purged_on_removal() {
    let mut store = Store::open_in_memory().unwrap();
    let id = store
        .adopt_or_create_account("h@exemple.fr", "gmail")
        .unwrap();

    assert_eq!(store.horizon_import(id).unwrap(), "tout");
    store.set_horizon_import(id, "1a").unwrap();
    assert_eq!(store.horizon_import(id).unwrap(), "1a");
    assert!(store.set_horizon_import(id, "42 jours").is_err());
    assert_eq!(store.horizon_import(id).unwrap(), "1a");

    store.delete_account(id).unwrap();
    let heir = store
        .adopt_or_create_account("h2@exemple.fr", "gmail")
        .unwrap();
    assert_eq!(heir, id, "fixture: the rowid must be reused");
    assert_eq!(store.horizon_import(heir).unwrap(), "tout");
}

/// Removing an account leaves NOTHING behind: neither the cascading
/// rows (mailboxes, envelopes, bodies), nor those without a foreign
/// key (drafts, outbox, search index) — and the neighboring account
/// keeps everything, search included.
#[test]
fn delete_account_erases_everything_and_does_not_touch_the_neighbor() {
    let mut store = Store::open_in_memory().unwrap();
    let departed = store
        .adopt_or_create_account("part@exemple.fr", "gmail")
        .unwrap();
    let neighbor = store
        .adopt_or_create_account("reste@exemple.fr", "gmail")
        .unwrap();
    for (account, subject) in [
        (departed, "Invoice for the departure"),
        (neighbor, "Quote that stays"),
    ] {
        let mailbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        store
            .upsert_envelopes(mailbox, &[envelope(1, subject, 100, false)])
            .unwrap();
        store.save_body(mailbox, 1, "<p>body</p>", &[]).unwrap();
        store
            .save_draft(
                account,
                None,
                None,
                crate::DraftContent {
                    to_raw: "a@b.fr",
                    cc_raw: "",
                    bcc_raw: "",
                    body_html: None,
                    subject,
                    body: "draft",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                    important: false,
                },
            )
            .unwrap();
        store
            .enqueue_outbox(
                account,
                &crate::compose::Draft {
                    message_id: format!("<outbound-{account}@exemple.fr>"),
                    from: "moi@exemple.fr".to_string(),
                    to: vec!["a@b.fr".to_string()],
                    cc: Vec::new(),
                    bcc: Vec::new(),
                    subject: subject.to_string(),
                    body_text: "body".to_string(),
                    body_html: None,
                    in_reply_to: None,
                    references: None,
                    important: false,
                    ics_reply: None,
                },
            )
            .unwrap();
    }

    // The preferences suffixed by the id (signature, marker, name): an
    // SQLite id reused after removal would otherwise make the next
    // account inherit the old one's identity (PLAN-RETOURS-8 review;
    // custom name: PLAN-RETOURS-9).
    for (account, hue) in [(departed, "rouge"), (neighbor, "bleu")] {
        store
            .set_text_pref(&format!("signature.{account}"), "<p>sig</p>")
            .unwrap();
        store
            .set_text_pref(&format!("repere_icone.{account}"), "home")
            .unwrap();
        store
            .set_text_pref(&format!("repere_teinte.{account}"), hue)
            .unwrap();
        store
            .set_text_pref(&format!("nom_compte.{account}"), "Perso")
            .unwrap();
    }

    store.delete_account(departed).unwrap();

    let accounts = store.accounts().unwrap();
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0].email, "reste@exemple.fr");
    for key in ["signature", "repere_icone", "repere_teinte", "nom_compte"] {
        assert_eq!(
            store.text_pref(&format!("{key}.{departed}")).unwrap(),
            None,
            "{key} of the departed account: the pref must die with it"
        );
        assert!(
            store
                .text_pref(&format!("{key}.{neighbor}"))
                .unwrap()
                .is_some(),
            "{key} of the neighbor: intact"
        );
    }
    for table in [
        "mailboxes",
        "envelopes",
        "bodies",
        "drafts",
        "outbox",
        "search_docs",
    ] {
        let total: i64 = store
            .0
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(total, 1, "{table}: only the neighbor's row must remain");
    }
    assert!(
        store.search("departure", 10).unwrap().is_empty(),
        "the departed account's mail must no longer come up in search"
    );
    assert_eq!(
        store.search("stays", 10).unwrap().len(),
        1,
        "the neighbor's search must survive the removal"
    );
}

/// ADR 0010: a message WITHOUT a date stays eligible for backfill,
/// even under a bounded horizon. The old rule excluded it ("not
/// placeable within the horizon") — a silent hole: never a body, so
/// never search, and nothing on screen to flag it. The doubt now
/// only costs its rank: the NULLs close the sort.
#[test]
fn a_message_without_a_date_stays_to_be_backfilled() {
    let (mut store, id) = store_with_mailbox();
    let without_date = Envelope {
        reply_to: None,
        uid: 9,
        subject: None,
        sender: None,
        sender_address: None,
        message_id: None,
        in_reply_to: None,
        date: None,
        seen: false,
        flagged: false,
        to_addrs: Vec::new(),
        cc_addrs: Vec::new(),
    };
    store
        .upsert_envelopes(id, std::slice::from_ref(&without_date))
        .unwrap();

    let account = test_account(&store);
    let uids = store
        .bodies_to_backfill(account, "INBOX", 1_000_000, 10)
        .unwrap();
    assert_eq!(
        uids,
        vec![9],
        "the bounded horizon no longer excludes the dateless"
    );
    assert_eq!(
        store
            .bodies_pending_count(account, "INBOX", 1_000_000)
            .unwrap(),
        1,
        "the progress counter sees it too — otherwise the bar would lie"
    );
}

#[test]
fn upsert_replaces_existing_envelope() {
    let (mut store, id) = store_with_mailbox();
    store
        .upsert_envelopes(id, &[envelope(1, "before", 100, false)])
        .unwrap();
    store
        .upsert_envelopes(id, &[envelope(1, "after", 100, true)])
        .unwrap();
    let rows = recent(&store, 0, 10);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].subject.as_deref(), Some("after"));
    assert!(rows[0].seen);
}

#[test]
fn recent_orders_by_date_then_uid_descending() {
    let (mut store, id) = store_with_mailbox();
    store
        .upsert_envelopes(
            id,
            &[
                envelope(1, "old", 100, false),
                envelope(3, "recent", 300, false),
                envelope(2, "middle", 200, false),
            ],
        )
        .unwrap();
    let uids: Vec<Uid> = recent(&store, 0, 2).iter().map(|e| e.uid).collect();
    assert_eq!(uids, vec![3, 2]);
}

#[test]
fn remove_absent_deletes_only_missing_uids() {
    let (mut store, id) = store_with_mailbox();
    store
        .upsert_envelopes(
            id,
            &[
                envelope(1, "a", 100, false),
                envelope(2, "b", 200, false),
                envelope(3, "c", 300, false),
            ],
        )
        .unwrap();
    let present: HashSet<Uid> = [1, 3].into_iter().collect();
    assert_eq!(store.remove_absent(id, &present).unwrap(), 1);
    assert_eq!(store.count(id).unwrap(), 2);
}

#[test]
fn sync_state_roundtrips_including_modseq() {
    let (store, id) = store_with_mailbox();
    assert_eq!(
        store.sync_state(test_account(&store), "INBOX").unwrap(),
        Some(SyncState {
            mailbox_id: id,
            uid_validity: 1,
            last_uid: 0,
            highest_modseq: None,
            initialized: false,
        })
    );
    store.update_state(id, 42, Some(9000)).unwrap();
    let state = store
        .sync_state(test_account(&store), "INBOX")
        .unwrap()
        .unwrap();
    assert_eq!(state.last_uid, 42);
    assert_eq!(state.highest_modseq, Some(9000));
}

#[test]
fn sync_state_is_none_for_unknown_mailbox() {
    let store = Store::open_in_memory().unwrap();
    assert_eq!(
        store.sync_state(test_account(&store), "INBOX").unwrap(),
        None
    );
}

#[test]
fn reset_mailbox_clears_envelopes_and_state() {
    let (mut store, id) = store_with_mailbox();
    store
        .upsert_envelopes(id, &[envelope(1, "a", 100, false)])
        .unwrap();
    store.update_state(id, 1, Some(5)).unwrap();
    store.reset_mailbox(id, 2).unwrap();
    assert_eq!(store.count(id).unwrap(), 0);
    let state = store
        .sync_state(test_account(&store), "INBOX")
        .unwrap()
        .unwrap();
    assert_eq!(state.uid_validity, 2);
    assert_eq!(state.last_uid, 0);
    assert_eq!(state.highest_modseq, None);
}

#[test]
fn max_uid_is_zero_for_empty_mailbox() {
    let (store, id) = store_with_mailbox();
    assert_eq!(store.max_uid(id).unwrap(), 0);
}

#[test]
fn recent_pages_with_offset() {
    let (mut store, id) = store_with_mailbox();
    store
        .upsert_envelopes(
            id,
            &(1..=5)
                .map(|uid| envelope(uid, "subject", 100 * i64::from(uid), false))
                .collect::<Vec<_>>(),
        )
        .unwrap();
    let page: Vec<Uid> = recent(&store, 2, 2).iter().map(|e| e.uid).collect();
    assert_eq!(page, vec![3, 2], "offset 2 skips the two most recent");
    assert!(recent(&store, 10, 5).is_empty());
}

#[test]
fn action_queue_roundtrips_in_emission_order() {
    let (store, id) = store_with_mailbox();
    store.enqueue_action(id, 5, Action::MarkSeen).unwrap();
    store.enqueue_action(id, 3, Action::MarkUnseen).unwrap();

    let queued = store.pending_actions(id).unwrap();
    assert_eq!(queued.len(), 2);
    assert_eq!(
        (queued[0].uid, queued[0].action.clone()),
        (5, Action::MarkSeen)
    );
    assert_eq!(
        (queued[1].uid, queued[1].action.clone()),
        (3, Action::MarkUnseen)
    );

    store.remove_action(queued[0].id).unwrap();
    assert_eq!(store.pending_actions(id).unwrap().len(), 1);
}

#[test]
fn set_seen_local_updates_and_reports_actual_change() {
    let (mut store, id) = store_with_mailbox();
    store
        .upsert_envelopes(id, &[envelope(1, "a", 100, false)])
        .unwrap();

    assert!(store.set_seen_local(id, 1, true).unwrap());
    assert!(recent(&store, 0, 1)[0].seen);
    assert!(
        !store.set_seen_local(id, 1, true).unwrap(),
        "already seen: nothing to log"
    );
}

#[test]
fn set_flagged_local_updates_and_reports_actual_change() {
    let (mut store, id) = store_with_mailbox();
    store
        .upsert_envelopes(id, &[envelope(1, "a", 100, false)])
        .unwrap();

    assert!(store.set_flagged_local(id, 1, true).unwrap());
    assert!(recent(&store, 0, 1)[0].flagged);
    assert!(
        !store.set_flagged_local(id, 1, true).unwrap(),
        "already flagged: nothing to log"
    );
}

/// E4 (PLAN-REACTIVITE, 1st field): ARRIVALS are counted by UID,
/// never by the report's `fetched` — a CONDSTORE delta mixes in
/// every shuffled flag (Gmail on every label), and the body limit
/// "overflowed" on every arrival.
#[test]
fn arrivals_are_counted_by_uid() {
    let (mut store, id) = store_with_mailbox();
    let account = test_account(&store);
    store
        .upsert_envelopes(
            id,
            &[
                envelope(1, "old", 100, true),
                envelope(2, "old too", 200, true),
            ],
        )
        .unwrap();
    assert_eq!(store.arrivals_since(account, "INBOX", 2).unwrap(), 0);

    // Two arrivals + one old flag retouched (upsert of the same
    // uid 1): the count only moves for the new UIDs.
    store
        .upsert_envelopes(
            id,
            &[
                envelope(1, "old", 100, false),
                envelope(3, "new", 300, false),
                envelope(4, "new too", 400, false),
            ],
        )
        .unwrap();
    assert_eq!(store.arrivals_since(account, "INBOX", 2).unwrap(), 2);
    // Unknown mailbox: zero, never an error — the poll of an account
    // never synced must not break on this account.
    assert_eq!(store.arrivals_since(account, "Elsewhere", 0).unwrap(), 0);
}

#[test]
fn remove_local_drops_envelope_and_body() {
    let (mut store, id) = store_with_mailbox();
    store
        .upsert_envelopes(id, &[envelope(1, "a", 100, false)])
        .unwrap();
    store.save_body(id, 1, "<p>x</p>", &[]).unwrap();

    store.remove_local(id, 1).unwrap();

    assert!(recent(&store, 0, 10).is_empty());
    assert_eq!(store.body(test_account(&store), "INBOX", 1).unwrap(), None);
}

#[test]
fn reset_mailbox_clears_pending_actions() {
    let (store, id) = store_with_mailbox();
    store.enqueue_action(id, 1, Action::MarkSeen).unwrap();
    store.reset_mailbox(id, 2).unwrap();
    assert!(store.pending_actions(id).unwrap().is_empty());
}

#[test]
fn body_roundtrips_and_is_none_when_absent() {
    let (store, id) = store_with_mailbox();
    assert_eq!(store.body(test_account(&store), "INBOX", 1).unwrap(), None);
    store.save_body(id, 1, "<p>hello</p>", &[]).unwrap();
    assert_eq!(
        store
            .body(test_account(&store), "INBOX", 1)
            .unwrap()
            .as_deref(),
        Some("<p>hello</p>")
    );
}

fn pdf(index: usize, name: &str) -> Attachment {
    Attachment {
        index,
        name: name.to_string(),
        mime: "application/pdf".to_string(),
        size: 2048,
    }
}

/// What the backfill has searched for since 2026-08-26: **ABSENT
/// bodies**, and nothing else.
///
/// It long also searched for bodies fetched BEFORE attachments
/// existed — `bodies.scanned = 0`, a MIME never inspected, not
/// recoverable from the stored HTML. This criterion is **removed**
/// (PLAN-DEMARRAGE, CE decision D8): it forced SQLite to recall the
/// body row to read one bit, which held the global lock **8,870 ms
/// on every startup** on the field database.
///
/// The three facts that allowed it, all measured on 2026-08-26:
/// production **never** writes `scanned = 0` ([`Store::save_body_full`]
/// hardcodes a `1`); **both** workstations of the fleet carry **zero**
/// rows at `scanned = 0`; and the legacy pass that produced them is
/// closed everywhere. The criterion protected zero rows.
///
/// What this test therefore keeps: a present body takes the message
/// out of the backfill, and **nothing brings it back**. Plus the
/// write invariant that made the removal safe — if something were to
/// write `scanned = 0` one day, the decision would need reopening,
/// and this test would say so.
#[test]
fn a_present_body_takes_the_message_out_of_the_backfill_and_nothing_brings_it_back() {
    let (mut store, id) = store_with_mailbox();
    let account = test_account(&store);
    store
        .upsert_envelopes(id, &[envelope(1, "subject", 100, false)])
        .unwrap();

    // Without a body: the message waits.
    assert_eq!(
        store.bodies_to_backfill(account, "INBOX", 0, 10).unwrap(),
        vec![1]
    );
    assert_eq!(store.bodies_pending_count(account, "INBOX", 0).unwrap(), 1);

    store.save_body(id, 1, "<p>body</p>", &[]).unwrap();

    // Body present: nothing left to do, definitively.
    assert!(
        store
            .bodies_to_backfill(account, "INBOX", 0, 10)
            .unwrap()
            .is_empty()
    );
    assert_eq!(store.bodies_pending_count(account, "INBOX", 0).unwrap(), 0);

    // The INVARIANT that made removing the criterion safe: production
    // always writes `scanned = 1`. The column is no longer read by
    // the backfill — if it had to become so again, it would still
    // tell the truth.
    let scanned: i64 = store
        .conn()
        .query_row("SELECT scanned FROM bodies", [], |row| row.get(0))
        .unwrap();
    assert_eq!(
        scanned, 1,
        "production must always write scanned = 1 — otherwise PLAN-DEMARRAGE's decision D8 needs reopening"
    );
}

/// R1 (PLAN-RETOURS-3): the percentage's denominator. The total does
/// NOT move when a body arrives — only the missing count decreases;
/// `total - pending` gives the present bodies, the basis of the
/// displayed percentage.
#[test]
fn the_corpus_total_counts_messages_not_bodies() {
    let (mut store, id) = store_with_mailbox();
    let account = test_account(&store);
    store
        .upsert_envelopes(
            id,
            &[
                envelope(1, "one", 100, false),
                envelope(2, "two", 200, false),
                envelope(3, "three", 300, false),
            ],
        )
        .unwrap();

    // Three messages in scope, no body read yet.
    assert_eq!(store.bodies_total_count(account, "INBOX", 0).unwrap(), 3);
    assert_eq!(store.bodies_pending_count(account, "INBOX", 0).unwrap(), 3);

    // A body arrives: the total holds, the rest drops by one.
    store.save_body(id, 2, "<p>body</p>", &[]).unwrap();
    assert_eq!(
        store.bodies_total_count(account, "INBOX", 0).unwrap(),
        3,
        "the total is the corpus, not the fetched bodies"
    );
    assert_eq!(store.bodies_pending_count(account, "INBOX", 0).unwrap(), 2);
}

/// A message already read elsewhere — phone, webmail — must not
/// trigger a notification bubble: it is pure noise, and it is what
/// gets notifications turned off.
#[test]
fn only_genuinely_new_and_unread_messages_are_notifiable() {
    let (mut store, id) = store_with_mailbox();
    let account = test_account(&store);
    store
        .upsert_envelopes(
            id,
            &[
                envelope(10, "old", 100, false),
                envelope(11, "already read", 200, true),
                envelope(12, "truly new", 300, false),
            ],
        )
        .unwrap();

    let arrivals = store.new_unread_after(account, "INBOX", 10, 20).unwrap();
    let subjects: Vec<_> = arrivals
        .iter()
        .map(|e| e.subject.clone().unwrap_or_default())
        .collect();
    assert_eq!(subjects, vec!["truly new".to_string()]);
}

fn folder(wire: &str, display: &str) -> Folder {
    Folder {
        wire: wire.to_string(),
        display: display.to_string(),
        selectable: true,
        special_use: None,
    }
}

/// Choosing a destination must work OFFLINE: the list is therefore
/// read locally, like the envelopes. Both the wire name and the
/// readable name are kept — losing the first would make the move
/// unplayable at replay time.
#[test]
fn folders_are_cached_locally_with_both_names() {
    let (store, _) = store_with_mailbox();
    let account = test_account(&store);
    assert!(store.folders(account).unwrap().is_empty());

    store
        .replace_folders(account, &[folder("Archiv&AOk-s", "Archivés")]) // lang:fr
        .unwrap();

    let cached = store.folders(account).unwrap();
    assert_eq!(cached.len(), 1);
    assert_eq!(cached[0].wire, "Archiv&AOk-s");
    assert_eq!(cached[0].display, "Archivés"); // lang:fr
}

/// A folder deleted server-side must no longer be offered: the move
/// would fail at replay time, long after the click — and the user
/// would no longer see the connection.
#[test]
fn refreshing_folders_drops_the_ones_that_disappeared() {
    let (store, _) = store_with_mailbox();
    let account = test_account(&store);
    store
        .replace_folders(account, &[folder("Old", "Old"), folder("Stays", "Stays")])
        .unwrap();

    store
        .replace_folders(account, &[folder("Stays", "Stays")])
        .unwrap();

    let cached = store.folders(account).unwrap();
    assert_eq!(cached.len(), 1);
    assert_eq!(cached[0].wire, "Stays");
}

#[test]
fn attachments_are_saved_with_the_body_and_read_back_in_order() {
    let (store, id) = store_with_mailbox();
    let account = test_account(&store);
    assert!(
        store.attachments(account, "INBOX", 1).unwrap().is_empty(),
        "nothing as long as the body has not been fetched"
    );

    store
        .save_body(
            id,
            1,
            "<p>attached</p>",
            &[pdf(0, "one.pdf"), pdf(1, "two.pdf")],
        )
        .unwrap();

    let found = store.attachments(account, "INBOX", 1).unwrap();
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].name, "one.pdf");
    assert_eq!(found[1].name, "two.pdf");
    assert_eq!(found[1].size, 2048);
}

/// A re-downloaded message whose attachment has disappeared must not
/// keep the old row: the user would click a file the server no
/// longer serves, and the failure would only surface at download
/// time — far from the cause.
#[test]
fn re_saving_replaces_the_attachment_list_instead_of_accumulating() {
    let (store, id) = store_with_mailbox();
    let account = test_account(&store);
    store
        .save_body(id, 1, "<p>x</p>", &[pdf(0, "one.pdf"), pdf(1, "two.pdf")])
        .unwrap();

    store
        .save_body(id, 1, "<p>x</p>", &[pdf(0, "one.pdf")])
        .unwrap();

    let found = store.attachments(account, "INBOX", 1).unwrap();
    assert_eq!(
        found.len(),
        1,
        "the vanished attachment must be gone here too"
    );
    assert_eq!(found[0].name, "one.pdf");
}

/// Attachments belong to a message of an ACCOUNT: the same (mailbox,
/// uid) pair on another account must see nothing.
#[test]
fn attachments_never_leak_across_accounts() {
    let (store, id) = store_with_mailbox();
    store
        .save_body(id, 1, "<p>x</p>", &[pdf(0, "private.pdf")])
        .unwrap();

    let other = store
        .adopt_or_create_account("autre@exemple.fr", "gmail")
        .unwrap();
    store.create_mailbox(other, "INBOX", 1).unwrap();

    assert!(store.attachments(other, "INBOX", 1).unwrap().is_empty());
}

fn project_invitation() -> crate::InvitationRow {
    crate::InvitationRow {
        method: "request".into(),
        event_uid: "reunion-1@exemple.fr".into(),
        sequence: 2,
        title: "Project sync".into(),
        location: Some("Room A".into()),
        organizer_address: Some("claire@exemple.fr".into()),
        organizer_name: Some("Claire Martin".into()),
        start_epoch: Some(1_788_400_200),
        end_epoch: Some(1_788_402_000),
        partstat: Some("sans_reponse".into()),
        ..Default::default()
    }
}

#[test]
fn an_invitation_is_written_with_the_body_and_reads_back() {
    let (store, id) = store_with_mailbox();
    let account = test_account(&store);
    store
        .save_body_full(id, 1, "<p>x</p>", &[], Some(&project_invitation()))
        .unwrap();

    let stored = store.invitation(account, "INBOX", 1).unwrap().expect("row");
    assert_eq!(stored.row, project_invitation());
    assert_eq!(stored.reply, None, "not answered yet");
}

/// Same rule as attachments: a re-downloaded message WITHOUT a
/// calendar part does not keep a phantom card.
#[test]
fn a_rescan_without_a_calendar_erases_the_row() {
    let (store, id) = store_with_mailbox();
    let account = test_account(&store);
    store
        .save_body_full(id, 1, "<p>x</p>", &[], Some(&project_invitation()))
        .unwrap();
    store.save_body(id, 1, "<p>x</p>", &[]).unwrap();
    assert_eq!(store.invitation(account, "INBOX", 1).unwrap(), None);
}

fn reply_draft() -> crate::compose::Draft {
    let mut draft = crate::compose(
        "moi@exemple.fr",
        "claire@exemple.fr",
        "",
        "",
        "Accepted: Project sync",
        "Accepted: Project sync",
        None,
    )
    .unwrap();
    draft.ics_reply = Some("BEGIN:VCALENDAR\r\nMETHOD:REPLY\r\nEND:VCALENDAR\r\n".into());
    draft
}

/// D6: the iTIP email gets logged AND the reply gets recorded — ONE
/// transaction; the reply survives the body's rescan (two distinct
/// truths — the PARTSTAT read from the message does not overwrite
/// it).
#[test]
fn the_reply_is_logged_with_its_email_and_survives_the_rescan() {
    let (store, id) = store_with_mailbox();
    let account = test_account(&store);
    store
        .save_body_full(id, 1, "<p>x</p>", &[], Some(&project_invitation()))
        .unwrap();

    let outbox_id = store
        .enqueue_invitation_reply(
            account,
            &reply_draft(),
            "INBOX",
            1,
            "accepte",
            1_755_900_000,
        )
        .unwrap();
    assert!(outbox_id.is_some(), "email logged");
    store
        .save_body_full(id, 1, "<p>x</p>", &[], Some(&project_invitation()))
        .unwrap();

    let stored = store.invitation(account, "INBOX", 1).unwrap().expect("row");
    assert_eq!(stored.reply.as_deref(), Some("accepte"));
    assert_eq!(stored.reply_epoch, Some(1_755_900_000));
    assert_eq!(store.outbox_to_send(account).unwrap().len(), 1);
}

/// The row disappeared between display and click (purged, mailbox
/// reset): NOTHING is sent — an email queued in front of a "not
/// answered" card would invite a double send (review).
#[test]
fn a_reply_without_a_row_logs_nothing() {
    let (store, _id) = store_with_mailbox();
    let account = test_account(&store);
    assert_eq!(
        store
            .enqueue_invitation_reply(account, &reply_draft(), "INBOX", 9, "accepte", 1)
            .unwrap(),
        None
    );
    assert!(
        store.outbox_to_send(account).unwrap().is_empty(),
        "the transaction rolled back: no email in queue"
    );
}

/// The PLAN-INVITATIONS review: after a UIDVALIDITY change, the UIDs
/// no longer mean anything — a card (and its reply!) that survived
/// would stick to an unrelated message.
#[test]
fn reset_mailbox_erases_invitations_and_attachments() {
    let (store, id) = store_with_mailbox();
    let account = test_account(&store);
    store
        .save_body_full(
            id,
            1,
            "<p>x</p>",
            &[pdf(0, "one.pdf")],
            Some(&project_invitation()),
        )
        .unwrap();

    store.reset_mailbox(id, 2).unwrap();

    assert_eq!(store.invitation(account, "INBOX", 1).unwrap(), None);
    assert!(store.attachments(account, "INBOX", 1).unwrap().is_empty());
}

/// The `pieces-calendrier` repair: a message scanned BEFORE
/// PLAN-INVITATIONS with a calendar part has SHIFTED attachment
/// indices (the old numbering counted it) and no card. At the
/// database's next opening, the body and attachments of these
/// messages are dropped: the backfill will reread them with the new
/// numbering — and the card will be born of the same scan (adoption,
/// invariant §6.7). On a FILE database: it is the reopening that
/// repairs it. Messages without a calendar do not move.
#[test]
fn the_calendar_attachments_repair_rereads_the_affected_messages() {
    let path = std::env::temp_dir().join(format!("wind-test-repair-cal-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    {
        let mut store = Store::open(&path).unwrap();
        let account = store
            .adopt_or_create_account("moi@exemple.fr", "gmail")
            .unwrap();
        let id = store.create_mailbox(account, "INBOX", 1).unwrap();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "invitation", 100, true),
                    envelope(2, "simple", 90, true),
                ],
            )
            .unwrap();
        // The BEFORE state: the calendar part counted as attachment 0.
        store
            .save_body(
                id,
                1,
                "<p>invitation</p>",
                &[
                    Attachment {
                        index: 0,
                        name: "attachment.calendar".into(),
                        mime: "text/calendar".into(),
                        size: 2048,
                    },
                    pdf(1, "contract.pdf"),
                ],
            )
            .unwrap();
        store
            .save_body(id, 2, "<p>simple</p>", &[pdf(0, "note.pdf")])
            .unwrap();
        // Removes the marker set at opening (database born repaired):
        // we replay the arrival of a database from BEFORE the repair.
        store
            .conn()
            .execute(
                "DELETE FROM reparations WHERE nom = 'pieces-calendrier'",
                [],
            )
            .unwrap();
    }

    Store::forget_initialization(&path);
    let store = Store::open(&path).unwrap();
    let account = store
        .adopt_or_create_account("moi@exemple.fr", "gmail")
        .unwrap();
    assert_eq!(
        store.body(account, "INBOX", 1).unwrap(),
        None,
        "the message with a calendar will be reread"
    );
    assert!(store.attachments(account, "INBOX", 1).unwrap().is_empty());
    assert_eq!(
        store.body(account, "INBOX", 2).unwrap().as_deref(),
        Some("<p>simple</p>"),
        "the ordinary message does not move"
    );
    assert_eq!(store.attachments(account, "INBOX", 2).unwrap().len(), 1);
    drop(store);
    let _ = std::fs::remove_file(&path);
}

/// Field R6: a CANCEL extinguishes the REQUEST of the same meeting
/// (same event_uid, same account), in BOTH arrival orders — the
/// cancellation often arrives in a fresh conversation, it is the
/// ORIGINAL card that must say so.
#[test]
fn a_cancel_extinguishes_the_request_of_the_same_meeting_in_both_arrival_orders() {
    let mut cancel = project_invitation();
    cancel.method = "cancel".to_string();
    cancel.cancelled = true;

    // Order 1: the REQUEST first, the CANCEL next.
    let (store, id) = store_with_mailbox();
    let account = test_account(&store);
    store
        .save_body_full(id, 1, "<p>i</p>", &[], Some(&project_invitation()))
        .unwrap();
    store
        .save_body_full(id, 2, "<p>a</p>", &[], Some(&cancel))
        .unwrap();
    assert!(
        store
            .invitation(account, "INBOX", 1)
            .unwrap()
            .expect("row")
            .row
            .cancelled,
        "the REQUEST is extinguished by the CANCEL"
    );

    // ANOTHER meeting of the same account does not move.
    let mut other = project_invitation();
    other.event_uid = "autre-reunion@exemple.fr".to_string();
    store
        .save_body_full(id, 3, "<p>x</p>", &[], Some(&other))
        .unwrap();
    assert!(
        !store
            .invitation(account, "INBOX", 3)
            .unwrap()
            .expect("row")
            .row
            .cancelled
    );

    // Order 2: the CANCEL scanned BEFORE (out-of-order backfill) —
    // the REQUEST is born cancelled.
    let (store, id) = store_with_mailbox();
    let account = test_account(&store);
    store
        .save_body_full(id, 2, "<p>a</p>", &[], Some(&cancel))
        .unwrap();
    store
        .save_body_full(id, 1, "<p>i</p>", &[], Some(&project_invitation()))
        .unwrap();
    assert!(
        store
            .invitation(account, "INBOX", 1)
            .unwrap()
            .expect("row")
            .row
            .cancelled
    );
}

#[test]
fn an_invitation_does_not_leak_across_accounts() {
    let (store, id) = store_with_mailbox();
    store
        .save_body_full(id, 1, "<p>x</p>", &[], Some(&project_invitation()))
        .unwrap();

    let other = store
        .adopt_or_create_account("autre@exemple.fr", "gmail")
        .unwrap();
    store.create_mailbox(other, "INBOX", 1).unwrap();

    assert_eq!(store.invitation(other, "INBOX", 1).unwrap(), None);
}

#[test]
fn reset_mailbox_clears_bodies_too() {
    let (store, id) = store_with_mailbox();
    store.save_body(id, 1, "<p>x</p>", &[]).unwrap();
    store.reset_mailbox(id, 2).unwrap();
    assert_eq!(store.body(test_account(&store), "INBOX", 1).unwrap(), None);
}

#[test]
fn envelope_returns_reply_context_fields() {
    let (mut store, id) = store_with_mailbox();
    let original = envelope(7, "subject", 100, false);
    store
        .upsert_envelopes(id, std::slice::from_ref(&original))
        .unwrap();

    assert_eq!(
        store.envelope(test_account(&store), "INBOX", 7).unwrap(),
        Some(original)
    );
    assert_eq!(
        store.envelope(test_account(&store), "INBOX", 99).unwrap(),
        None
    );
}

/// ADR 0011: on a FILE database, opening switches to WAL — and the
/// mode persists, a legacy database in rollback mode is converted.
/// This is what prevents "database is locked" when the progress
/// gauge reads while a full synchronization writes — the first
/// defect the field returned on ADR 0010.
///
/// On a file database and not in memory, like the field: an
/// in-memory database answers "memory" to this PRAGMA, and the test
/// would validate a false model.
#[test]
fn a_file_database_opens_in_wal() {
    let path = std::env::temp_dir().join(format!("wind-test-wal-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);

    // A legacy database, born BEFORE WAL: rollback mode (delete).
    {
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch("CREATE TABLE heritage (id INTEGER)")
            .unwrap();
    }

    {
        let _store = Store::open(&path).unwrap();
        let conn = Connection::open(&path).unwrap();
        let mode: String = conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        assert_eq!(
            mode.to_lowercase(),
            "wal",
            "the legacy database is converted"
        );
    }
    let _ = std::fs::remove_file(&path);
}

/// Field STOP 2 PLAN-AUDIT-V2 (2026-09-02): on the real database,
/// "table envelopes has no column named reply_to" on every pass of
/// the watcher — the column lived in the CREATE TABLE, never in the
/// list of migrated columns; the e2e fixtures, freshly seeded, could
/// not see it. A database from before wave 2 receives the column at
/// reopening, and a poll writes to it.
#[test]
fn a_database_from_before_wave_2_receives_the_reply_to_column() {
    let path =
        std::env::temp_dir().join(format!("wind-test-reply-to-migr-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    drop(Store::open(&path).unwrap());
    {
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch("ALTER TABLE envelopes DROP COLUMN reply_to")
            .unwrap();
    }
    Store::forget_initialization(&path);

    let mut store = Store::open(&path).unwrap();
    let account = test_account(&store);
    let mailbox = store.create_mailbox(account, "INBOX", 1).unwrap();
    let mut list = envelope(1, "List", 100, false);
    list.reply_to = Some("liste@exemple.fr".to_string());
    store.upsert_envelopes(mailbox, &[list]).unwrap();
    assert_eq!(
        store.reply_to_of(account, "INBOX", 1).unwrap(),
        Some("liste@exemple.fr".to_string())
    );
    let _ = std::fs::remove_file(&path);
}

/// PLAN-AUDIT-V2 E1: every shell command opens ITS OWN connection —
/// 103 sites — and each one replayed the schema, some twenty
/// `table_xinfo` calls and the migrations (36 ms on 200k envelopes,
/// on EVERY command). Once the full initialization has SUCCEEDED on
/// a path, subsequent openings of the same process do not replay it.
/// Proof without a spy in production code: an index is removed
/// behind the Store's back; if the schema were replayed,
/// `CREATE INDEX IF NOT EXISTS` would recreate it.
#[test]
fn a_second_opening_of_the_same_path_does_not_replay_the_schema() {
    let path =
        std::env::temp_dir().join(format!("wind-test-porte-rapide-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    drop(Store::open(&path).unwrap());

    {
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch("DROP INDEX idx_pending_actions_message")
            .unwrap();
    }
    drop(Store::open(&path).unwrap());

    let conn = Connection::open(&path).unwrap();
    let recreated: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_pending_actions_message'",
                [],
                |row| row.get(0),
            )
            .unwrap();
    assert_eq!(recreated, 0, "the second opening replayed the schema");
    let _ = std::fs::remove_file(&path);
}

/// Rebuilding the search index must make the migration screen show
/// (ADR 0012) even on a database ALREADY up to date on the thread
/// side: without this detection in `pending_adoption`, it would
/// freeze the startup in silence (field finding 2026-08-17). On a
/// file database, because the probe opens read-only — an in-memory
/// database has no path.
#[test]
fn pending_adoption_sees_an_old_search_index() {
    let path =
        std::env::temp_dir().join(format!("wind-test-search-migr-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    {
        let mut store = Store::open(&path).unwrap();
        let account = test_account(&store);
        let mailbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        store
            .upsert_envelopes(mailbox, &[envelope(1, "Subject", 100, false)])
            .unwrap();
        // Downgrades the index to the old three-column schema: the
        // threads stay adopted (`user_version` unchanged), only the
        // index predates this job — exactly the field's state.
        store
            .conn()
            .execute_batch(
                "DROP TABLE search_fts;
                     DROP TABLE search_docs;
                     CREATE TABLE search_docs (
                        docid      INTEGER PRIMARY KEY,
                        mailbox_id INTEGER NOT NULL,
                        uid        INTEGER NOT NULL,
                        UNIQUE (mailbox_id, uid)
                     );
                     CREATE VIRTUAL TABLE search_fts USING fts5(
                        subject, sender, body,
                        content='', contentless_delete=1,
                        tokenize='unicode61 remove_diacritics 2'
                     );",
            )
            .unwrap();
    } // clean close -> WAL checkpoint, the read-only probe reads.

    assert_eq!(
        Store::pending_adoption(&path).unwrap(),
        Some(1),
        "the old FTS schema makes the screen show, threads already adopted"
    );

    // A full opening rebuilds it; after that, nothing left to report.
    Store::forget_initialization(&path);
    {
        Store::open(&path).unwrap();
    }
    assert_eq!(
        Store::pending_adoption(&path).unwrap(),
        None,
        "rebuilt -> the screen does not show again"
    );
    let _ = std::fs::remove_file(&path);
}

/// A Phase 1 database (without the reply columns) must open and
/// enrich itself without losing the already-synced envelopes.
#[test]
fn opens_and_migrates_a_phase1_database() {
    let path = std::env::temp_dir().join(format!("wind-test-migration-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    {
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE mailboxes (
                    id             INTEGER PRIMARY KEY,
                    name           TEXT NOT NULL UNIQUE,
                    uid_validity   INTEGER NOT NULL,
                    last_uid       INTEGER NOT NULL DEFAULT 0,
                    highest_modseq INTEGER
                );
                CREATE TABLE envelopes (
                    mailbox_id INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
                    uid        INTEGER NOT NULL,
                    subject    TEXT,
                    sender     TEXT,
                    date_epoch INTEGER,
                    seen       INTEGER NOT NULL DEFAULT 0,
                    PRIMARY KEY (mailbox_id, uid)
                );
                INSERT INTO mailboxes (id, name, uid_validity) VALUES (1, 'INBOX', 1);
                INSERT INTO envelopes (mailbox_id, uid, subject, sender, date_epoch, seen)
                VALUES (1, 42, 'inherited from phase 1', 'Alice', 100, 1);",
        )
        .unwrap();
    }

    let store = Store::open(&path).unwrap();
    let rows = recent(&store, 0, 10);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].uid, 42);
    assert_eq!(rows[0].subject.as_deref(), Some("inherited from phase 1"));
    assert_eq!(
        rows[0].sender_address, None,
        "column added by migration: value unknown for the existing row"
    );
    assert!(!rows[0].flagged, "star absent by default after migration");

    drop(store);
    let _ = std::fs::remove_file(&path);
}

/// R2 (PLAN-RETOURS-MAIL): an envelope synced BEFORE the fix carries
/// IMAP backslash-escapes in its subject and its sender name; the
/// migration removes them once. The field case "Test \"Sent\"".
#[test]
fn migration_removes_the_imap_escapes_from_existing_subjects() {
    let path = std::env::temp_dir().join(format!("wind-test-escapes-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    {
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE mailboxes (
                    id             INTEGER PRIMARY KEY,
                    name           TEXT NOT NULL UNIQUE,
                    uid_validity   INTEGER NOT NULL,
                    last_uid       INTEGER NOT NULL DEFAULT 0,
                    highest_modseq INTEGER
                );
                CREATE TABLE envelopes (
                    mailbox_id     INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
                    uid            INTEGER NOT NULL,
                    subject        TEXT,
                    sender         TEXT,
                    sender_address TEXT,
                    date_epoch     INTEGER,
                    seen           INTEGER NOT NULL DEFAULT 0,
                    PRIMARY KEY (mailbox_id, uid)
                );
                INSERT INTO mailboxes (id, name, uid_validity) VALUES (1, 'INBOX', 1);",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO envelopes
                    (mailbox_id, uid, subject, sender, sender_address, date_epoch, seen)
                 VALUES (1, 7, ?1, ?2, ?3, 100, 1)",
            params![r#"Test \"Sent\""#, r#"Company \"ACME\""#, "info@acme.fr"],
        )
        .unwrap();
        // A clean subject, without escapes: it must pass through intact.
        conn.execute(
            "INSERT INTO envelopes (mailbox_id, uid, subject, sender, date_epoch, seen)
                 VALUES (1, 8, 'Meeting tomorrow', 'Alice', 90, 1)",
            [],
        )
        .unwrap();
    }

    let store = Store::open(&path).unwrap();
    let rows = recent(&store, 0, 10);
    let seven = rows.iter().find(|e| e.uid == 7).unwrap();
    assert_eq!(seven.subject.as_deref(), Some(r#"Test "Sent""#));
    assert_eq!(seven.sender.as_deref(), Some(r#"Company "ACME""#));
    let eight = rows.iter().find(|e| e.uid == 8).unwrap();
    assert_eq!(eight.subject.as_deref(), Some("Meeting tomorrow"));

    drop(store);
    let _ = std::fs::remove_file(&path);
}

/// Phase 2 → 3 migration on a full database: all the data
/// (envelopes, bodies, actions, drafts, tombstones, outbox)
/// are adopted by the pending account — zero loss, and the first
/// connection claims everything.
#[test]
fn migrates_a_full_phase2_database_and_adopts_everything() {
    let path =
        std::env::temp_dir().join(format!("wind-test-migration-p2-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    {
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
                "CREATE TABLE mailboxes (
                    id             INTEGER PRIMARY KEY,
                    name           TEXT NOT NULL UNIQUE,
                    uid_validity   INTEGER NOT NULL,
                    last_uid       INTEGER NOT NULL DEFAULT 0,
                    highest_modseq INTEGER
                );
                CREATE TABLE envelopes (
                    mailbox_id     INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
                    uid            INTEGER NOT NULL,
                    subject        TEXT,
                    sender         TEXT,
                    sender_address TEXT,
                    message_id     TEXT,
                    date_epoch     INTEGER,
                    seen           INTEGER NOT NULL DEFAULT 0,
                    flagged        INTEGER NOT NULL DEFAULT 0,
                    PRIMARY KEY (mailbox_id, uid)
                );
                CREATE TABLE bodies (
                    mailbox_id INTEGER NOT NULL,
                    uid        INTEGER NOT NULL,
                    html       TEXT NOT NULL,
                    PRIMARY KEY (mailbox_id, uid)
                );
                CREATE TABLE pending_actions (
                    id INTEGER PRIMARY KEY, mailbox_id INTEGER NOT NULL,
                    uid INTEGER NOT NULL, kind TEXT NOT NULL
                );
                CREATE TABLE drafts (
                    id INTEGER PRIMARY KEY, to_raw TEXT NOT NULL,
                    subject TEXT NOT NULL, body TEXT NOT NULL,
                    reply_to_uid INTEGER, updated_epoch INTEGER NOT NULL,
                    remote_uid INTEGER, pushed_epoch INTEGER
                );
                CREATE TABLE draft_tombstones (remote_uid INTEGER PRIMARY KEY);
                CREATE TABLE drafts_remote (
                    id INTEGER PRIMARY KEY CHECK (id = 1),
                    uid_validity INTEGER NOT NULL
                );
                CREATE TABLE outbox (
                    id INTEGER PRIMARY KEY, message_id TEXT NOT NULL,
                    sender TEXT NOT NULL, recipients TEXT NOT NULL,
                    subject TEXT NOT NULL, body_text TEXT NOT NULL,
                    in_reply_to TEXT, state TEXT NOT NULL DEFAULT 'queued',
                    attempts INTEGER NOT NULL DEFAULT 0, last_error TEXT,
                    queued_epoch INTEGER NOT NULL
                );
                INSERT INTO mailboxes (id, name, uid_validity) VALUES (1, 'INBOX', 7);
                INSERT INTO envelopes (mailbox_id, uid, subject, seen, flagged)
                    VALUES (1, 42, 'legacy', 1, 1);
                INSERT INTO bodies VALUES (1, 42, '<p>body</p>');
                INSERT INTO pending_actions (mailbox_id, uid, kind) VALUES (1, 42, 'mark_seen');
                INSERT INTO drafts (to_raw, subject, body, updated_epoch, remote_uid, pushed_epoch)
                    VALUES ('x@y.fr', 'precious', 'text', 10, 77, 10);
                INSERT INTO draft_tombstones VALUES (99);
                INSERT INTO drafts_remote VALUES (1, 1234);
                INSERT INTO outbox (message_id, sender, recipients, subject, body_text, queued_epoch)
                    VALUES ('<m@x>', 'me@y.fr', 'you@y.fr', 's', 'b', 20);",
            )
            .unwrap();
    }

    let store = Store::open(&path).unwrap();
    let account = store
        .adopt_or_create_account("legacy@example.fr", "gmail")
        .unwrap();
    assert_eq!(account, 1, "claiming takes over the pending account");

    assert_eq!(store.recent(account, "INBOX", 0, 10).unwrap()[0].uid, 42);
    assert_eq!(
        store.body(1, "INBOX", 42).unwrap().as_deref(),
        Some("<p>body</p>")
    );
    let drafts = store.drafts().unwrap();
    assert_eq!(drafts[0].account_id, 1);
    assert_eq!(drafts[0].remote_uid, Some(77));
    assert_eq!(store.draft_tombstones(1).unwrap(), vec![99]);
    assert!(
        !store.align_drafts_uidvalidity(1, 1234).unwrap(),
        "the drafts' UIDVALIDITY survived: no reset"
    );
    assert_eq!(store.outbox_to_send(1).unwrap().len(), 1);
    assert_eq!(store.accounts().unwrap().len(), 1);

    let second = store
        .adopt_or_create_account("two@example.fr", "gmail")
        .unwrap();
    assert_ne!(second, 1, "the placeholder is claimed only once");

    drop(store);
    let _ = std::fs::remove_file(&path);
}

/// PLAN-COMPOSITION-HTML E1: a legacy database (from before HTML
/// bodies) gains the `body_html` columns of `drafts` and `outbox` on
/// open — NULL on existing rows, the text path untouched.
/// On a FILE database: it is the real migration pass that is proved,
/// not a fresh schema (invariant #7).
#[test]
fn legacy_database_gains_body_html_columns_with_null_on_existing_rows() {
    let path = std::env::temp_dir().join(format!("wind-test-body-html-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    {
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
                "CREATE TABLE drafts (
                    id INTEGER PRIMARY KEY, to_raw TEXT NOT NULL,
                    subject TEXT NOT NULL, body TEXT NOT NULL,
                    reply_to_uid INTEGER, updated_epoch INTEGER NOT NULL,
                    remote_uid INTEGER, pushed_epoch INTEGER
                );
                CREATE TABLE outbox (
                    id INTEGER PRIMARY KEY, message_id TEXT NOT NULL,
                    sender TEXT NOT NULL, recipients TEXT NOT NULL,
                    subject TEXT NOT NULL, body_text TEXT NOT NULL,
                    in_reply_to TEXT, state TEXT NOT NULL DEFAULT 'queued',
                    attempts INTEGER NOT NULL DEFAULT 0, last_error TEXT,
                    queued_epoch INTEGER NOT NULL
                );
                INSERT INTO drafts (to_raw, subject, body, updated_epoch)
                    VALUES ('x@y.fr', 's', 'plain text', 10);
                INSERT INTO outbox (message_id, sender, recipients, subject, body_text, queued_epoch)
                    VALUES ('<m@x>', 'me@y.fr', 'you@y.fr', 's', 'b', 20);",
            )
            .unwrap();
    }

    let store = Store::open(&path).unwrap();
    for table in ["drafts", "outbox"] {
        assert!(
            table_columns(store.conn(), table)
                .unwrap()
                .contains("body_html"),
            "{table} must gain body_html on open"
        );
    }
    let old: Option<String> = store
        .conn()
        .query_row("SELECT body_html FROM drafts", [], |row| row.get(0))
        .unwrap();
    assert_eq!(old, None, "existing rows stay NULL: text path untouched");
    let old: Option<String> = store
        .conn()
        .query_row("SELECT body_html FROM outbox", [], |row| row.get(0))
        .unwrap();
    assert_eq!(old, None);

    drop(store);
    let _ = std::fs::remove_file(&path);
}

/// The core deliverable of multi-account: the same mailbox for every
/// account, merged by date — each row knows its own account.
#[test]
fn unified_recent_merges_accounts_by_date() {
    let store = Store::open_in_memory().unwrap();
    let first = store
        .adopt_or_create_account("a@example.fr", "gmail")
        .unwrap();
    let second = store
        .adopt_or_create_account("b@example.fr", "gmail")
        .unwrap();
    let inbox_a = store.create_mailbox(first, "INBOX", 1).unwrap();
    let inbox_b = store.create_mailbox(second, "INBOX", 1).unwrap();

    let mut store = store;
    store
        .upsert_envelopes(
            inbox_a,
            &[
                envelope(1, "a-old", 100, false),
                envelope(2, "a-recent", 300, false),
            ],
        )
        .unwrap();
    store
        .upsert_envelopes(
            inbox_b,
            &[
                envelope(1, "b-middle", 200, false),
                envelope(2, "b-last", 400, false),
            ],
        )
        .unwrap();

    let rows = store.unified_recent(0, 10).unwrap();
    let order: Vec<(&str, &str)> = rows
        .iter()
        .map(|row| {
            (
                row.account_email.as_str(),
                row.envelope.subject.as_deref().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        order,
        vec![
            ("b@example.fr", "b-last"),
            ("a@example.fr", "a-recent"),
            ("b@example.fr", "b-middle"),
            ("a@example.fr", "a-old"),
        ],
        "merged by date, each row carries its account"
    );
    assert_eq!(store.unified_count().unwrap(), 4);
    // Same UID in two accounts: two distinct messages.
    assert!(store.envelope(first, "INBOX", 1).unwrap().is_some());
    assert!(store.envelope(second, "INBOX", 1).unwrap().is_some());
}

#[test]
fn remove_absent_drops_orphaned_bodies() {
    let (mut store, id) = store_with_mailbox();
    store
        .upsert_envelopes(id, &[envelope(1, "a", 100, false)])
        .unwrap();
    store.save_body(id, 1, "<p>x</p>", &[]).unwrap();
    assert_eq!(store.remove_absent(id, &HashSet::new()).unwrap(), 1);
    assert_eq!(store.body(test_account(&store), "INBOX", 1).unwrap(), None);
}

/// `corps-fffd` repair: a body mutilated at decoding time (U+FFFD) is
/// purged so that the backfill redownloads it with the fixed decoder;
/// a healthy body is left in place.
#[test]
fn the_corps_fffd_repair_purges_mutilated_bodies() {
    let (mut store, id) = store_with_mailbox();
    store
        .upsert_envelopes(
            id,
            &[envelope(1, "a", 100, false), envelope(2, "b", 100, false)],
        )
        .unwrap();
    store.save_body(id, 1, "<p>tod\u{FFFD}ay</p>", &[]).unwrap();
    store.save_body(id, 2, "<p>healthy</p>", &[]).unwrap();
    // Simulates a database from before the repair: the marker
    // disappears, and the migration replays as on the next open.
    store
        .conn()
        .execute("DELETE FROM reparations WHERE nom = 'corps-fffd'", [])
        .unwrap();
    migrate(store.conn(), &mut |_| ControlFlow::Continue(())).unwrap();
    let account = test_account(&store);
    assert_eq!(
        store.body(account, "INBOX", 1).unwrap(),
        None,
        "mutilated body purged"
    );
    assert!(
        store.body(account, "INBOX", 2).unwrap().is_some(),
        "healthy body kept"
    );
    // The purged message becomes a backfill target again.
    assert_eq!(
        store.bodies_to_backfill(account, "INBOX", 0, 10).unwrap(),
        vec![1]
    );
}

/// Regression (bug #2): re-adding an already-known generic account
/// must return the SAME id and apply the new configuration.
/// On the UPDATE path of the upsert, `last_insert_rowid()` used to
/// return 0 — a phantom id that the UI picked up for the badge and
/// the selection. Each command opens ITS OWN connection: so the
/// re-add is modeled with two distinct `Store`s on the same file
/// database, because it is the fresh connection (with no prior
/// INSERT) that takes the UPDATE path and exhibits the 0.
#[test]
fn re_adding_a_generic_account_returns_the_same_id_and_updates_config() {
    let path = std::env::temp_dir().join(format!(
        "wind-test-generic-{}-{}.sqlite",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    let first = {
        let store = Store::open(&path).unwrap();
        store
            .create_generic_account(
                "account@example.fr",
                "account",
                "imap.a.fr",
                993,
                "smtp.a.fr",
                465,
            )
            .unwrap()
    };
    let second = {
        let store = Store::open(&path).unwrap();
        store
            .create_generic_account(
                "account@example.fr",
                "login",
                "imap.b.fr",
                143,
                "smtp.b.fr",
                587,
            )
            .unwrap()
    };
    let (count, config) = {
        let store = Store::open(&path).unwrap();
        (
            store.accounts().unwrap().len(),
            store.account_config(first).unwrap(),
        )
    };
    // Cleanup before the assertions: a failure must not leave a
    // temporary file behind.
    let _ = std::fs::remove_file(&path);

    assert!(first > 0, "the first creation must return a real id");
    assert_eq!(
        second, first,
        "re-adding must return the existing id, never 0"
    );
    assert_eq!(count, 1, "a single account, no duplicate");
    assert_eq!(config.username.as_deref(), Some("login"));
    assert_eq!(config.imap_host.as_deref(), Some("imap.b.fr"));
    assert_eq!(config.imap_port, Some(143));
    assert_eq!(config.smtp_host.as_deref(), Some("smtp.b.fr"));
    assert_eq!(config.smtp_port, Some(587));
}

/// The backfill targets RECENT bodyless messages, newest first: this
/// is the order in which search gains the most value, and the one
/// that makes resuming after an interruption feel natural.
#[test]
fn backfill_lists_recent_bodyless_messages_newest_first() {
    let (mut store, id) = store_with_mailbox();
    store
        .upsert_envelopes(
            id,
            &[
                envelope(1, "old", 1_000, false),
                envelope(2, "middle", 2_000, false),
                envelope(3, "recent", 3_000, false),
            ],
        )
        .unwrap();
    let account = test_account(&store);

    let todo = store.bodies_to_backfill(account, "INBOX", 0, 10).unwrap();
    assert_eq!(todo, vec![3, 2, 1]);
}

#[test]
fn backfill_skips_messages_that_already_have_a_body() {
    let (mut store, id) = store_with_mailbox();
    store
        .upsert_envelopes(
            id,
            &[
                envelope(1, "without body", 1_000, false),
                envelope(2, "with body", 2_000, false),
            ],
        )
        .unwrap();
    store.save_body(id, 2, "<p>already there</p>", &[]).unwrap();
    let account = test_account(&store);

    assert_eq!(
        store.bodies_to_backfill(account, "INBOX", 0, 10).unwrap(),
        vec![1]
    );
}

/// The recency horizon is what BOUNDS the cost (ADR 0007): beyond it,
/// nothing is fetched back.
#[test]
fn backfill_respects_the_recency_horizon() {
    let (mut store, id) = store_with_mailbox();
    store
        .upsert_envelopes(
            id,
            &[
                envelope(1, "outside the horizon", 1_000, false),
                envelope(2, "inside the horizon", 5_000, false),
            ],
        )
        .unwrap();
    let account = test_account(&store);

    assert_eq!(
        store
            .bodies_to_backfill(account, "INBOX", 4_000, 10)
            .unwrap(),
        vec![2]
    );
}

#[test]
fn backfill_honours_the_batch_limit() {
    let (mut store, id) = store_with_mailbox();
    let envelopes: Vec<Envelope> = (1..=10)
        .map(|uid| envelope(uid, "message", uid as i64 * 100, false))
        .collect();
    store.upsert_envelopes(id, &envelopes).unwrap();
    let account = test_account(&store);

    assert_eq!(
        store
            .bodies_to_backfill(account, "INBOX", 0, 3)
            .unwrap()
            .len(),
        3
    );
}

#[test]
fn backfill_never_leaks_another_accounts_messages() {
    let (mut store, mine) = store_with_mailbox();
    let other = store
        .adopt_or_create_account("other@example.fr", "gmail")
        .unwrap();
    let theirs = store.create_mailbox(other, "INBOX", 1).unwrap();
    store
        .upsert_envelopes(mine, &[envelope(1, "mine", 1_000, false)])
        .unwrap();
    store
        .upsert_envelopes(theirs, &[envelope(1, "someone else's", 2_000, false)])
        .unwrap();
    let account = test_account(&store);

    assert_eq!(
        store.bodies_to_backfill(account, "INBOX", 0, 10).unwrap(),
        vec![1],
        "a single message: the one belonging to the requested account"
    );
    assert_eq!(
        store.bodies_to_backfill(other, "INBOX", 0, 10).unwrap(),
        vec![1]
    );
}

// -----------------------------------------------------------------
// Grouping into conversations
// -----------------------------------------------------------------

/// A reply to `parent`, in the format of [`envelope`] — whose
/// `Message-ID` is `<m{uid}@example.com>`.
fn reply(uid: Uid, subject: &str, epoch: i64, seen: bool, parent: Uid) -> Envelope {
    Envelope {
        in_reply_to: Some(format!("<m{parent}@example.com>")),
        ..envelope(uid, subject, epoch, seen)
    }
}

fn unified(store: &Store) -> Vec<UnifiedRow> {
    store.unified_recent(0, 50).unwrap()
}

fn uids(rows: &[UnifiedRow]) -> Vec<Uid> {
    rows.iter().map(|row| row.envelope.uid).collect()
}

/// The heart of the job: two messages, a single row.
#[test]
fn the_list_shows_one_row_per_conversation() {
    let (mut store, id) = store_with_mailbox();
    store
        .upsert_envelopes(
            id,
            &[
                envelope(1, "Quote", 100, true),
                reply(2, "Re: Quote", 200, true, 1),
            ],
        )
        .unwrap();

    let rows = unified(&store);
    assert_eq!(rows.len(), 1, "one thread, not two messages");
    assert_eq!(rows[0].thread_size, 2);
    assert_eq!(rows[0].envelope.uid, 2, "the row shows the LAST message");
    assert_eq!(
        store.unified_count().unwrap(),
        1,
        "scrolling counts conversations, otherwise it scrolls into thin air"
    );
}

#[test]
fn a_reply_brings_the_whole_thread_back_up() {
    let (mut store, id) = store_with_mailbox();
    store
        .upsert_envelopes(
            id,
            &[
                envelope(1, "Quote", 100, true),
                envelope(2, "Invoice", 200, true),
            ],
        )
        .unwrap();
    assert_eq!(uids(&unified(&store)), vec![2, 1]);

    store
        .upsert_envelopes(id, &[reply(3, "Re: Quote", 300, true, 1)])
        .unwrap();

    let rows = unified(&store);
    assert_eq!(
        uids(&rows),
        vec![3, 2],
        "the quote moves back ahead of the invoice"
    );
    assert_eq!(rows[0].thread_size, 2);
}

/// A thread whose last message is read, but which still holds an
/// unread message higher up, must stay bold. Reading the state of
/// only the displayed message would give the opposite answer.
#[test]
fn a_thread_stays_unread_while_any_of_its_messages_is() {
    let (mut store, id) = store_with_mailbox();
    store
        .upsert_envelopes(
            id,
            &[
                envelope(1, "Quote", 100, false),
                reply(2, "Re: Quote", 200, true, 1),
            ],
        )
        .unwrap();

    let rows = unified(&store);
    assert!(rows[0].envelope.seen, "the last message is read…");
    assert_eq!(
        rows[0].thread_unseen, 1,
        "…but the thread still holds an unread one"
    );

    store.set_seen_local(id, 1, true).unwrap();
    assert_eq!(
        unified(&store)[0].thread_unseen,
        0,
        "reading the missing message clears the thread"
    );
}

/// The case that justifies the pass over full headers: in an inbox,
/// the middle message of an exchange is the one WE sent — it isn't
/// there. `In-Reply-To` alone therefore leaves two threads;
/// `References`, which also carries the root, glues them back
/// together.
#[test]
fn references_glue_two_thread_halves_back_together() {
    let (mut store, id) = store_with_mailbox();
    store
        .upsert_envelopes(
            id,
            &[
                envelope(1, "Quote", 100, true),
                // Replies to <m2@…>: our own reply, absent.
                reply(3, "Re: Quote", 300, true, 2),
            ],
        )
        .unwrap();
    assert_eq!(
        unified(&store).len(),
        2,
        "two threads, for lack of the missing link"
    );

    assert!(
        store
            .set_thread_headers(id, 3, None, "<m1@example.com> <m2@example.com>")
            .unwrap(),
        "the attachment changed"
    );

    let rows = unified(&store);
    assert_eq!(rows.len(), 1, "the two halves join back together");
    assert_eq!(rows[0].thread_size, 2);
    assert_eq!(rows[0].envelope.uid, 3);
}

/// A resync rewrites the envelope. If it overwrote the `References`
/// already acquired, it would UNGROUP a glued thread: the grouping
/// would silently come undone, with nothing to signal it. This is
/// the trap that had cost us the attachments.
#[test]
fn a_resync_does_not_ungroup_a_glued_thread() {
    let (mut store, id) = store_with_mailbox();
    let arrival = [
        envelope(1, "Quote", 100, true),
        reply(3, "Re: Quote", 300, true, 2),
    ];
    store.upsert_envelopes(id, &arrival).unwrap();
    store
        .set_thread_headers(id, 3, None, "<m1@example.com> <m2@example.com>")
        .unwrap();
    assert_eq!(unified(&store).len(), 1);

    store.upsert_envelopes(id, &arrival).unwrap();

    let rows = unified(&store);
    assert_eq!(rows.len(), 1, "the thread holds through the resync");
    assert_eq!(rows[0].thread_size, 2);
}

/// The attachments trap applied to threads: a database from before
/// grouping has `thread_id` NULL everywhere. The list starts from
/// `threads` — without adoption, it would be EMPTY on the first
/// open, and forever.
#[test]
fn a_legacy_database_sees_all_its_messages_adopted() {
    let (mut store, id) = store_with_mailbox();
    store
        .upsert_envelopes(
            id,
            &[
                envelope(1, "Quote", 100, true),
                envelope(2, "Invoice", 200, true),
            ],
        )
        .unwrap();

    // Rewind to the state of a database from before threads.
    store
        .conn()
        .execute_batch(
            "UPDATE envelopes SET thread_id = NULL;
                 DELETE FROM thread_links;
                 DELETE FROM threads;",
        )
        .unwrap();
    assert!(
        unified(&store).is_empty(),
        "without adoption, the entire mailbox disappears from the screen"
    );

    crate::thread::migrate_threads(store.conn()).unwrap();

    assert_eq!(uids(&unified(&store)), vec![2, 1]);
    assert_eq!(store.unified_count().unwrap(), 2);
}

/// Arrival order must change nothing: here the reply precedes its
/// parent in the same batch.
#[test]
fn a_thread_reads_from_oldest_to_newest() {
    let (mut store, id) = store_with_mailbox();
    store
        .upsert_envelopes(
            id,
            &[
                reply(2, "Re: Quote", 200, true, 1),
                envelope(1, "Quote", 100, true),
            ],
        )
        .unwrap();

    let rows = unified(&store);
    assert_eq!(rows.len(), 1, "arrival order does not break the thread");
    let thread = rows[0].thread_id.unwrap();
    let messages = store.thread_messages(thread).unwrap();
    assert_eq!(uids(&messages), vec![1, 2]);
    // Each message comes back knowing the size of ITS thread:
    // otherwise the screen that reopens it would conclude it's alone.
    assert!(messages.iter().all(|m| m.thread_size == 2));
}

#[test]
fn removing_a_threads_messages_makes_it_disappear() {
    let (mut store, id) = store_with_mailbox();
    store
        .upsert_envelopes(
            id,
            &[
                envelope(1, "Quote", 100, true),
                reply(2, "Re: Quote", 200, true, 1),
            ],
        )
        .unwrap();

    store.remove_local(id, 2).unwrap();
    let rows = unified(&store);
    assert_eq!(
        uids(&rows),
        vec![1],
        "the thread falls back on what remains"
    );
    assert_eq!(rows[0].thread_size, 1);

    store.remove_local(id, 1).unwrap();
    assert!(unified(&store).is_empty());
    assert_eq!(store.unified_count().unwrap(), 0);
}

/// The field's own finding, end to end: two unrelated messages whose
/// `In-Reply-To` is a SENTENCE — not an identifier — must remain two
/// conversations.
///
/// Before the fix, every word of the sentence became a shared anchor
/// and merged them together. On a real mailbox this produced a
/// 43-message thread with no relation between its messages.
#[test]
fn two_messages_whose_header_is_prose_do_not_merge() {
    let (mut store, id) = store_with_mailbox();
    let prose = "Your message of January 3rd";
    store
        .upsert_envelopes(
            id,
            &[
                Envelope {
                    in_reply_to: Some(prose.to_string()),
                    ..envelope(1, "Promotion", 100, true)
                },
                Envelope {
                    in_reply_to: Some(prose.to_string()),
                    ..envelope(2, "Another promotion", 200, true)
                },
            ],
        )
        .unwrap();

    assert_eq!(unified(&store).len(), 2, "no link between these two");
}

/// A database grouped by the old rule carries FALSE threads, and
/// fixing the code does not repair them on its own. The version
/// marker makes them redone on open — without a network, since the
/// raw headers are intact in the database.
#[test]
fn a_badly_grouped_database_is_redone_on_open() {
    let (mut store, id) = store_with_mailbox();
    store
        .upsert_envelopes(
            id,
            &[
                envelope(1, "Promotion", 100, true),
                envelope(2, "Another promotion", 200, true),
            ],
        )
        .unwrap();
    assert_eq!(unified(&store).len(), 2);

    // Replays the state that the permissive rule used to produce: a
    // single thread for two unrelated messages, and the old version.
    store
        .conn()
        .execute_batch(
            "DELETE FROM thread_links WHERE thread_id = (SELECT MAX(id) FROM threads);
                 UPDATE envelopes SET thread_id = (SELECT MIN(id) FROM threads);
                 DELETE FROM threads WHERE id = (SELECT MAX(id) FROM threads);
                 UPDATE threads SET size = 2, last_uid = 2, last_epoch = 200;
                 PRAGMA user_version = 0;",
        )
        .unwrap();
    assert_eq!(
        unified(&store).len(),
        1,
        "the faulty state is correctly reproduced"
    );

    crate::thread::migrate_threads(store.conn()).unwrap();

    assert_eq!(unified(&store).len(), 2, "the threads are redone");
    let version: i64 = store
        .conn()
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    // Against the CONSTANT, never against a literal: every change to
    // the grouping rule increments it, and a hardcoded "1" would fail
    // this test for a reason that isn't its own.
    assert_eq!(
        version,
        crate::thread::THREADING_VERSION,
        "and the rebuild does not replay again"
    );
}

/// UIDVALIDITY invalidated: threads go with the rest, and the
/// directory must not prevent a clean repopulation.
#[test]
fn reset_mailbox_also_clears_threads() {
    let (mut store, id) = store_with_mailbox();
    store
        .upsert_envelopes(
            id,
            &[
                envelope(1, "Quote", 100, true),
                reply(2, "Re: Quote", 200, true, 1),
            ],
        )
        .unwrap();
    store.reset_mailbox(id, 2).unwrap();
    assert!(unified(&store).is_empty());

    store
        .upsert_envelopes(id, &[envelope(1, "Quote", 100, true)])
        .unwrap();
    assert_eq!(
        unified(&store).len(),
        1,
        "the mailbox repopulates without a stop"
    );
}

/// Replays on `path` the tables as version 1 of threads created
/// them — the only fixture where the adoption pass has real work to
/// do. Shared by the open test below and by the rewind tests
/// (Phase 5 job).
fn rewind_to_schema_v1(path: &Path) {
    // A database rewound by hand is a database from BEFORE: the fast
    // path registry (E1) must no longer know about it.
    Store::forget_initialization(path);
    let conn = Connection::open(path).unwrap();
    conn.execute_batch(
        "DROP TABLE thread_links;
             DROP TABLE threads;
             CREATE TABLE threads (
                 id         INTEGER PRIMARY KEY,
                 mailbox_id INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
                 last_uid   INTEGER NOT NULL DEFAULT 0,
                 last_epoch INTEGER,
                 size       INTEGER NOT NULL DEFAULT 0,
                 unseen     INTEGER NOT NULL DEFAULT 0
             );
             CREATE INDEX idx_threads_date
                 ON threads(mailbox_id, last_epoch DESC, last_uid DESC);
             CREATE TABLE thread_links (
                 mailbox_id INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
                 message_id TEXT NOT NULL,
                 thread_id  INTEGER NOT NULL REFERENCES threads(id) ON DELETE CASCADE,
                 PRIMARY KEY (mailbox_id, message_id)
             );
             CREATE INDEX idx_thread_links_thread ON thread_links(thread_id);
             UPDATE envelopes SET thread_id = NULL;
             PRAGMA user_version = 1;",
    )
    .unwrap();
}

/// Finding from the FIELD, not here: a database created by the
/// previous version carries a `threads` table with no `inbox_size`.
/// `CREATE TABLE IF NOT EXISTS` does not touch it — but the partial
/// index does not exist yet, so SQLite really tries to create it:
/// it fails on a missing column, and **the entire open is refused**
/// ("no such column: inbox_size"). The app would no longer start.
///
/// No test could catch it: they all create a fresh database, already
/// on the current schema. This one REWINDS a real database to the
/// previous schema — the only fixture where the defect exists.
#[test]
fn a_database_on_the_previous_threads_schema_opens_and_migrates() {
    let path = std::env::temp_dir().join(format!("wind-test-threads-v1-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);

    {
        let mut store = Store::open(&path).unwrap();
        let account = store
            .adopt_or_create_account("me@example.fr", "gmail")
            .unwrap();
        let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        let mut first = envelope(1, "Quote", 100, true);
        first.message_id = Some("<a@example.fr>".to_string());
        let mut second = envelope(2, "Re: Quote", 200, true);
        second.message_id = Some("<b@example.fr>".to_string());
        second.in_reply_to = Some("<a@example.fr>".to_string());
        store.upsert_envelopes(inbox, &[first, second]).unwrap();
        assert_eq!(
            unified(&store).len(),
            1,
            "fixture: a thread of two messages"
        );
    }

    // Rewind: the tables as version 1 created them.
    rewind_to_schema_v1(&path);

    // This is the open that used to be refused.
    let store = Store::open(&path).unwrap();
    let rows = unified(&store);
    assert_eq!(rows.len(), 1, "the thread is redone, and the list shows it");
    assert_eq!(rows[0].thread_size, 2, "with its counter");

    drop(store);
    let _ = std::fs::remove_file(&path);
}

/// THE test of the Phase 5 job (handover §8): adoption is NOT
/// splittable — the list starts from `threads`, a partially persisted
/// adoption would be a half-empty mailbox. "Interruptible" therefore
/// means: cancelling IN THE MIDDLE of the pass undoes EVERYTHING and
/// leaves `user_version` unchanged, so the entire pass replays at the
/// next launch — where the list is complete.
#[test]
fn cancelling_adoption_undoes_everything_and_leaves_user_version_unchanged() {
    let path = std::env::temp_dir().join(format!("wind-test-rewind-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);

    // Enough messages for the cancellation to land IN THE MIDDLE of
    // the pass: progress is reported in stages, one must be crossed.
    const MESSAGES: u32 = 1_200;
    {
        let mut store = Store::open(&path).unwrap();
        let account = store
            .adopt_or_create_account("me@example.fr", "gmail")
            .unwrap();
        let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        let fixture: Vec<Envelope> = (1..=MESSAGES)
            .map(|uid| envelope(uid, "Subject", 100 + i64::from(uid), true))
            .collect();
        store.upsert_envelopes(inbox, &fixture).unwrap();
    }
    rewind_to_schema_v1(&path);

    // Cancel as soon as 1,000 messages have gone through — in the
    // middle, not at the gate's threshold: the rewind must undo real
    // work.
    let mut highest_done = 0;
    let result = Store::open_with_progress(&path, |p| {
        highest_done = highest_done.max(p.done);
        if p.done >= 1_000 {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    });
    assert!(
        matches!(result, Err(Error::Interrupted)),
        "cancelling must return Error::Interrupted, not a Store"
    );
    assert!(
        highest_done >= 1_000,
        "the fixture must exercise a cancellation IN PROGRESS \
             (highest reading: {highest_done})"
    );

    // Everything is undone: the database is back to the state
    // BEFORE the cancelled open.
    {
        let conn = Connection::open(&path).unwrap();
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, 1, "user_version unchanged: the pass will replay");
        let new_shape: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('threads')
                     WHERE name = 'inbox_size'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            new_shape, 0,
            "the v1 table is intact: the DROP is rewound too"
        );
        let envelopes: i64 = conn
            .query_row("SELECT COUNT(*) FROM envelopes", [], |row| row.get(0))
            .unwrap();
        assert_eq!(envelopes, i64::from(MESSAGES), "no message lost");
    }

    // The next launch replays the WHOLE pass: complete list.
    {
        let store = Store::open(&path).unwrap();
        let threadless: i64 = store
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM envelopes WHERE thread_id IS NULL",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(threadless, 0, "every legacy message is adopted");
        let version: i64 = store
            .conn()
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, crate::thread::THREADING_VERSION);
    }
    let _ = std::fs::remove_file(&path);
}

/// Progress is OBSERVABLE (lesson §9): the total is announced up
/// front and never moves again, progress never goes backwards, and
/// "done" is only said at the end — never before.
#[test]
fn adoption_reports_its_progress_from_start_to_finish() {
    let path = std::env::temp_dir().join(format!(
        "wind-test-adoption-progress-{}.db",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    {
        let mut store = Store::open(&path).unwrap();
        let account = store
            .adopt_or_create_account("me@example.fr", "gmail")
            .unwrap();
        let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        store
            .upsert_envelopes(
                inbox,
                &[
                    envelope(1, "Quote", 100, true),
                    reply(2, "Re: Quote", 200, true, 1),
                ],
            )
            .unwrap();
    }
    rewind_to_schema_v1(&path);

    let mut reports: Vec<AdoptionProgress> = Vec::new();
    let store = Store::open_with_progress(&path, |p| {
        reports.push(p);
        ControlFlow::Continue(())
    })
    .unwrap();

    assert!(!reports.is_empty(), "a silent adoption is not observable");
    assert_eq!(reports[0].done, 0, "the start is announced right away");
    assert!(reports[0].total > 0, "the total is announced up front");
    for pair in reports.windows(2) {
        assert!(
            pair[1].done >= pair[0].done,
            "progress does not go backwards"
        );
        assert_eq!(
            pair[1].total, pair[0].total,
            "the total does not move mid-flight — a bar that goes \
                 backwards is worse than an imprecise bar"
        );
    }
    let last = reports.last().unwrap();
    assert_eq!(last.done, last.total, "the last report says \"done\"");
    assert!(
        reports[..reports.len() - 1]
            .iter()
            .all(|p| p.done < p.total),
        "and it is the ONLY one: never \"100%\" before the end"
    );

    let rows = unified(&store);
    assert_eq!(rows.len(), 1, "the thread is redone");
    assert_eq!(rows[0].thread_size, 2, "with its counter");
    drop(store);
    let _ = std::fs::remove_file(&path);
}

/// The probe answers without triggering anything: the desktop calls
/// it BEFORE the first real open to decide whether to show the
/// migration screen — if it migrated on its own, the screen would
/// arrive after the fact.
#[test]
fn the_probe_says_when_an_adoption_is_pending_without_triggering_it() {
    let path = std::env::temp_dir().join(format!(
        "wind-test-probe-adoption-{}.db",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);

    // File absent: first install, nothing legacy — and the probe
    // must NOT create the file.
    assert_eq!(Store::pending_adoption(&path).unwrap(), None);
    assert!(!path.exists(), "a probe leaves no trace");

    {
        let mut store = Store::open(&path).unwrap();
        let account = store
            .adopt_or_create_account("me@example.fr", "gmail")
            .unwrap();
        let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        store
            .upsert_envelopes(
                inbox,
                &[
                    envelope(1, "Quote", 100, true),
                    reply(2, "Re: Quote", 200, true, 1),
                ],
            )
            .unwrap();
        // A message OUT OF SCOPE (ADR 0010 §3): the pass will never
        // adopt it, the probe must not announce it.
        let spam = store.create_mailbox(account, "Spam", 1).unwrap();
        store
            .upsert_envelopes(spam, &[envelope(1, "You won!", 300, true)])
            .unwrap();
    }
    // Up-to-date database: nothing to announce.
    assert_eq!(Store::pending_adoption(&path).unwrap(), None);

    rewind_to_schema_v1(&path);
    assert_eq!(
        Store::pending_adoption(&path).unwrap(),
        Some(2),
        "a legacy database announces its messages to adopt — the SCOPE, \
             not the whole database: a figure must name what it says"
    );
    // And NOTHING was triggered: the version has not moved.
    {
        let conn = Connection::open(&path).unwrap();
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, 1, "the probe did not migrate on our behalf");
    }
    let _ = std::fs::remove_file(&path);
}

/// The language is restored BEFORE the first render, so BEFORE the
/// migration screen (field finding 2026-08-15): reading it must be a
/// read-only probe — with a full open, adopting a legacy database
/// used to be paid for silently while loading the language, with no
/// modal, no progress, no cancellation — everything ADR 0012
/// forbids. The fixture REWINDS a real file database (invariant
/// §6.7): the only one where the defect exists.
#[test]
fn the_language_reads_without_adopting_the_database() {
    let path = std::env::temp_dir().join(format!(
        "wind-test-language-probe-{}.db",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);

    // File absent: first install — and the probe must NOT create the
    // file.
    assert_eq!(Store::text_pref_readonly(&path, "lang").unwrap(), None);
    assert!(!path.exists(), "a probe leaves no trace");

    {
        let mut store = Store::open(&path).unwrap();
        let account = store
            .adopt_or_create_account("me@example.fr", "gmail")
            .unwrap();
        let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        store
            .upsert_envelopes(
                inbox,
                &[
                    envelope(1, "Quote", 100, true),
                    reply(2, "Re: Quote", 200, true, 1),
                ],
            )
            .unwrap();
        store.set_text_pref("lang", "en").unwrap();
    }
    rewind_to_schema_v1(&path);

    // The preference reads back…
    assert_eq!(
        Store::text_pref_readonly(&path, "lang").unwrap(),
        Some("en".to_string())
    );
    // …and NOTHING was triggered: the version has not moved, the
    // modal will still find the adoption pending.
    {
        let conn = Connection::open(&path).unwrap();
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(
            version, 1,
            "reading the language did not migrate on our behalf"
        );
    }
    assert_eq!(
        Store::pending_adoption(&path).unwrap(),
        Some(2),
        "the migration screen still has a reason to exist"
    );

    // A legacy database from before WAL lives in rollback (delete)
    // mode — the real shape found in the field, not the one
    // `Store::open` leaves behind: the probe must answer there too.
    Connection::open(&path)
        .unwrap()
        .query_row("PRAGMA journal_mode = delete", [], |row| {
            row.get::<_, String>(0)
        })
        .unwrap();
    assert_eq!(
        Store::text_pref_readonly(&path, "lang").unwrap(),
        Some("en".to_string()),
        "the probe also answers on a database in rollback mode"
    );

    // A database from before preferences (no `prefs` table): the
    // probe answers "no preference", it does not fail.
    Connection::open(&path)
        .unwrap()
        .execute_batch("DROP TABLE prefs")
        .unwrap();
    assert_eq!(Store::text_pref_readonly(&path, "lang").unwrap(), None);
    let _ = std::fs::remove_file(&path);
}

/// On an up-to-date database there is NOTHING to adopt — and so
/// nothing to say. A migration banner on every launch would be a
/// false signal, and every desktop command opens its own connection.
#[test]
fn an_up_to_date_database_opens_without_announcing_a_migration() {
    let path = std::env::temp_dir().join(format!(
        "wind-test-silent-adoption-{}.db",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    {
        let mut store = Store::open(&path).unwrap();
        let account = store
            .adopt_or_create_account("me@example.fr", "gmail")
            .unwrap();
        let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        store
            .upsert_envelopes(
                inbox,
                &[
                    envelope(1, "Quote", 100, true),
                    reply(2, "Re: Quote", 200, true, 1),
                ],
            )
            .unwrap();
    }

    let mut calls = 0;
    let store = Store::open_with_progress(&path, |_| {
        calls += 1;
        ControlFlow::Continue(())
    })
    .unwrap();
    assert_eq!(calls, 0, "nothing to adopt, nothing to report");
    assert_eq!(unified(&store).len(), 1, "and the list is there");
    drop(store);
    let _ = std::fs::remove_file(&path);
}

/// THE point of the [ADR 0009] job: a received message and the reply
/// we made to it belong to the same exchange, so the same thread —
/// even though they live in **two different mailboxes**.
///
/// Before, threads were siloed by mailbox: this reply would have
/// formed its own thread in its own id space, and syncing "Sent"
/// would have cost without paying anything back.
///
/// The fixture deliberately gives the same UID (1) to both messages:
/// a message's identity is `(account, mailbox, UID)`, and any
/// grouping that confused two equal UIDs would show up here.
#[test]
fn a_reply_in_sent_joins_the_received_messages_thread() {
    let mut store = Store::open_in_memory().unwrap();
    let account = store
        .adopt_or_create_account("me@example.fr", "gmail")
        .unwrap();
    let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
    let sent = store.create_mailbox(account, "Sent", 1).unwrap();
    // The fixture must DECLARE the scope it exercises: since ADR
    // 0010, a mailbox only groups if it has been told to, and the
    // name of the sent folder varies from one server to the next.
    store.set_thread_scope(account, Some("Sent")).unwrap();

    // Alice writes.
    let mut received = envelope(1, "Quote", 100, true);
    received.message_id = Some("<alice-1@example.fr>".to_string());
    store.upsert_envelopes(inbox, &[received]).unwrap();

    // I reply: the message goes into "Sent" and quotes the first one.
    let mut reply = envelope(1, "Re: Quote", 200, true);
    reply.message_id = Some("<me-1@example.fr>".to_string());
    reply.in_reply_to = Some("<alice-1@example.fr>".to_string());
    store.upsert_envelopes(sent, &[reply]).unwrap();

    let rows = unified(&store);
    assert_eq!(rows.len(), 1, "a single thread, not two");
    assert_eq!(
        rows[0].thread_size, 2,
        "the counter covers the whole exchange, sent items included"
    );
    assert_eq!(
        rows[0].envelope.subject.as_deref(),
        Some("Re: Quote"),
        "the thread is represented by its most recent message, \
             even when it is our own reply"
    );
}

/// Two messages from the SAME account can carry the SAME UID as soon
/// as they live in two mailboxes — this is the rule, not the
/// exception, since UIDs are assigned per mailbox and restart at 1.
///
/// Each row must therefore say **where it lives**. Without this,
/// opening our reply from the conversation banner would display the
/// received message in its place, and mark it read — invariant §6.2
/// of the handover, amended here for two mailboxes.
#[test]
fn each_row_says_which_mailbox_it_lives_in() {
    let mut store = Store::open_in_memory().unwrap();
    let account = store
        .adopt_or_create_account("me@example.fr", "gmail")
        .unwrap();
    let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
    let sent = store.create_mailbox(account, "Sent", 1).unwrap();
    // The fixture must DECLARE the scope it exercises: since ADR
    // 0010, a mailbox only groups if it has been told to, and the
    // name of the sent folder varies from one server to the next.
    store.set_thread_scope(account, Some("Sent")).unwrap();

    let mut received = envelope(1, "Quote", 100, true);
    received.message_id = Some("<alice-9@example.fr>".to_string());
    store.upsert_envelopes(inbox, &[received]).unwrap();
    let mut reply = envelope(1, "Re: Quote", 200, true);
    reply.message_id = Some("<me-9@example.fr>".to_string());
    reply.in_reply_to = Some("<alice-9@example.fr>".to_string());
    store.upsert_envelopes(sent, &[reply]).unwrap();

    let thread = unified(&store)[0].thread_id.unwrap();
    let messages = store.thread_messages(thread).unwrap();

    assert_eq!(messages.len(), 2);
    assert!(
        messages.iter().all(|row| row.envelope.uid == 1),
        "the fixture does have two messages sharing the same UID: that's the whole point"
    );
    let mailboxes: Vec<&str> = messages.iter().map(|l| l.mailbox.as_str()).collect();
    assert!(
        mailboxes.contains(&"INBOX"),
        "mailboxes seen: {mailboxes:?}"
    );
    assert!(mailboxes.contains(&"Sent"), "mailboxes seen: {mailboxes:?}");
}

/// The other side of the same rule: writing to someone who never
/// replies does NOT create a conversation in the inbox. This is what
/// the `inbox_size` counter protects, and it is also what makes the
/// partial index possible (ADR 0009 §2 and §4).
#[test]
fn a_purely_outgoing_thread_has_no_row() {
    let mut store = Store::open_in_memory().unwrap();
    let account = store
        .adopt_or_create_account("me@example.fr", "gmail")
        .unwrap();
    store.create_mailbox(account, "INBOX", 1).unwrap();
    let sent = store.create_mailbox(account, "Sent", 1).unwrap();
    // The fixture must DECLARE the scope it exercises: since ADR
    // 0010, a mailbox only groups if it has been told to, and the
    // name of the sent folder varies from one server to the next.
    store.set_thread_scope(account, Some("Sent")).unwrap();

    let mut outgoing = envelope(1, "My proposal", 100, true);
    outgoing.message_id = Some("<me-2@example.fr>".to_string());
    store.upsert_envelopes(sent, &[outgoing]).unwrap();

    assert!(
        unified(&store).is_empty(),
        "nothing was received: the inbox stays empty"
    );
    assert_eq!(store.unified_count().unwrap(), 0);
}

/// [ADR 0010] §3 — we STORE everything, we only GROUP within scope.
///
/// Since [ADR 0009] a thread belongs to the ACCOUNT. As soon as full
/// sync pours Archive, Trash and Spam into that same account, their
/// messages would join threads **on their own** — and three
/// aggregates would silently get corrupted, with no test to see it:
///
/// - `size`: "12 messages" on a thread that shows 3;
/// - `unseen`: a thread perpetually unread because of a spam message;
/// - `last_epoch`: **the conversation jumps to the top of the list
///   because a spam message latched onto it**.
///
/// The third is a CORRECTNESS defect: the list would lie about the
/// order of exchanges, with no recourse for the user. Same reason
/// for refusal as grouping by subject (ADR 0008 §2).
///
/// The compiler protects nothing here — a mailbox is a string like
/// any other (handover §6.2). It's this test that holds the
/// invariant.
#[test]
fn a_message_out_of_scope_does_not_join_the_thread() {
    let mut store = Store::open_in_memory().unwrap();
    let account = store
        .adopt_or_create_account("me@example.fr", "gmail")
        .unwrap();
    let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
    let spam = store.create_mailbox(account, "Spam", 1).unwrap();

    let mut received = envelope(1, "Quote", 100, true);
    received.message_id = Some("<alice-10@example.fr>".to_string());
    store.upsert_envelopes(inbox, &[received]).unwrap();

    // The spam message quotes the received message — exactly what
    // would make it join the thread. It is MORE RECENT and UNREAD:
    // if it got in, all three aggregates would move at once.
    let mut junk = envelope(1, "WIN 1000 EUROS", 300, false);
    junk.message_id = Some("<spam-1@elsewhere.example>".to_string());
    junk.in_reply_to = Some("<alice-10@example.fr>".to_string());
    store.upsert_envelopes(spam, &[junk]).unwrap();

    let rows = unified(&store);
    assert_eq!(rows.len(), 1, "a single thread");
    assert_eq!(
        rows[0].thread_size, 1,
        "the spam message does not count in the exchange"
    );
    assert_eq!(
        rows[0].envelope.subject.as_deref(),
        Some("Quote"),
        "the thread stays represented by the received message, not by \
             the spam that latched onto it"
    );
    assert_eq!(
        rows[0].thread_unseen, 0,
        "a spam message never opened does not make the conversation unread"
    );

    // The other half of ADR 0010: out of scope does not mean absent.
    // The message is stored — so it is searchable.
    assert!(
        store.envelope(account, "Spam", 1).unwrap().is_some(),
        "the spam message is indeed in the database: we store everything, we don't group everything"
    );
}

/// A scope declared BEFORE the mailbox exists must still count —
/// this is the normal case, not the edge case.
///
/// The [ADR 0010] sync loop **creates** the sent folder: at the
/// moment the scope is declared, there is no row yet to update. If
/// the scope only lived on `mailboxes`, this declaration would be
/// lost, the mailbox would be born out of scope, and its messages
/// would stay threadless until the next startup — the list would
/// show an exchange amputated of our replies, with nothing to signal
/// it.
///
/// Hence the memory carried by the ACCOUNT, which this test guards.
#[test]
fn a_scope_declared_before_the_mailbox_is_created_still_counts() {
    let mut store = Store::open_in_memory().unwrap();
    let account = store
        .adopt_or_create_account("me@example.fr", "gmail")
        .unwrap();
    let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();

    // We declare "Sent" BEFORE creating it — the real order.
    store.set_thread_scope(account, Some("Sent")).unwrap();
    let sent = store.create_mailbox(account, "Sent", 1).unwrap();

    let mut received = envelope(1, "Quote", 100, true);
    received.message_id = Some("<alice-11@example.fr>".to_string());
    store.upsert_envelopes(inbox, &[received]).unwrap();
    let mut reply = envelope(1, "Re: Quote", 200, true);
    reply.message_id = Some("<me-11@example.fr>".to_string());
    reply.in_reply_to = Some("<alice-11@example.fr>".to_string());
    store.upsert_envelopes(sent, &[reply]).unwrap();

    let rows = unified(&store);
    assert_eq!(rows.len(), 1, "a single thread");
    assert_eq!(
        rows[0].thread_size, 2,
        "the reply joined the thread as soon as it was written, without \
             waiting for a restart"
    );
}

/// The promise of [ADR 0008] §4 — "the cost of a page no longer
/// depends on the size of the mailbox" — rests ENTIRELY on an index
/// that carries the sort order. If SQLite materializes the order in
/// a temporary B-tree, the promise is broken: silently, and only at
/// scale, exactly where no functional test looks anymore.
///
/// It happened. Gate 3 measured **987 ms** for a page over 160,000
/// conversations, against 0.66 ms once the index was in place. The
/// original index was prefixed by `mailbox_id`: it served a single
/// mailbox, but not the **unified mailbox**, which covers all of
/// them and is the product's default view. Two accounts are enough
/// to reproduce it — hence this fixture.
///
/// We interrogate the query plan rather than a stopwatch: a duration
/// depends on the machine, an execution plan does not.
#[test]
fn the_unified_mailbox_does_not_materialize_its_sort() {
    let mut store = Store::open_in_memory().unwrap();
    for (email, uids) in [("one@example.fr", 1..60u32), ("two@example.fr", 60..120)] {
        let account = store.adopt_or_create_account(email, "gmail").unwrap();
        let mailbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        let envelopes: Vec<Envelope> = uids
            .map(|uid| envelope(uid, "Subject", 1_600_000_000 + i64::from(uid), true))
            .collect();
        store.upsert_envelopes(mailbox, &envelopes).unwrap();
    }

    let mut stmt = store
        .0
        .prepare(&format!(
            "EXPLAIN QUERY PLAN {}",
            unified_page_sql(false, false, false)
        ))
        .unwrap();
    let plan: Vec<String> = stmt
        .query_map(params![200i64, 0i64], |row| row.get::<_, String>(3))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();

    // "FOR LAST TERM OF ORDER BY" is acceptable: that sort only
    // breaks ties on date AND UID. It's the FULL sort that costs,
    // and only that one is forbidden here.
    assert!(
        !plan
            .iter()
            .any(|step| step.contains("TEMP B-TREE FOR ORDER BY")),
        "the unified mailbox page materializes its sort — the cost \
             becomes proportional to the mailbox size again.\nPlan:\n{}",
        plan.join("\n")
    );
    // R4: the pinned-threads subquery (PINNED_THREADS) must start
    // from `pins` (lowercase) and PROBE `envelopes` by its key —
    // without the directive CROSS JOIN, SQLite (without ANALYZE, the
    // production case) scans `envelopes` ENTIRELY on every page:
    // ~24 ms measured at 200k, on the hottest path (review
    // 2026-08-21).
    assert!(
        !plan.iter().any(|step| step.contains("SCAN pe")),
        "the pinned-threads subquery scans `envelopes` — the join \
             order has lost its directive.\nPlan:\n{}",
        plan.join("\n")
    );
    assert!(
        plan.iter().any(|step| step.contains("SCAN p")),
        "the pinned-threads subquery no longer starts from `pins`.\nPlan:\n{}",
        plan.join("\n")
    );
}

/// PLAN-AUDIT-V2 E4: the cleanup groups (one sender × their mail)
/// cost 380 ms over 200k envelopes and 5,000 senders — a scan
/// through the DATE index followed by a temporary grouping B-tree.
/// The senders index, extended to the mailbox, COVERS the aggregate:
/// the plan must go through it, never through the date index (a
/// query-plan test, STANDARD §9 lesson).
#[test]
fn cleanup_groups_are_read_via_the_senders_index() {
    let store = Store::open_in_memory().unwrap();
    let account = store
        .adopt_or_create_account("me@example.fr", "gmail")
        .unwrap();
    let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
    let sql = Store::cleanup_groups_sql(&[inbox]);
    let plan: Vec<String> = store
        .conn()
        .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
        .unwrap()
        .query_map(params![0i64], |row| row.get::<_, String>(3))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(
        plan.iter().any(|row| row.contains("idx_envelopes_sender")),
        "the aggregate does not go through the senders index: {plan:?}"
    );
    assert!(
        !plan.iter().any(|row| row.contains("idx_envelopes_date")),
        "the aggregate scans the date index: {plan:?}"
    );
    // A group's mail, same requirement (116 ms at 200k otherwise).
    let sql = Store::cleanup_messages_sql(&[inbox]);
    let plan: Vec<String> = store
        .conn()
        .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
        .unwrap()
        .query_map(params![0i64, "x@y.fr"], |row| row.get::<_, String>(3))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(
        plan.iter()
            .any(|row| row.contains("idx_envelopes_sender (sender_norm=?)")),
        "a group's mail is not looked up by sender: {plan:?}"
    );
}

/// Wave 2 review: `PRAGMA foreign_keys = ON` lives in `SCHEMA` and
/// holds PER CONNECTION — the fast path does not replay the schema.
/// This test stayed green BEFORE the line was added to `init_with`:
/// rusqlite's `bundled` enables foreign keys by default at compile
/// time. It keeps the belt anyway: on a FILE database (an in-memory
/// database never enters the registry), the second open still clears
/// the mailboxes of a deleted account, whatever the compile flag.
#[test]
fn the_fast_path_keeps_foreign_keys_enabled() {
    let path =
        std::env::temp_dir().join(format!("wind-test-fast-path-fk-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    drop(Store::open(&path).unwrap());

    let mut store = Store::open(&path).unwrap();
    let enabled: i64 = store
        .conn()
        .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
        .unwrap();
    assert_eq!(enabled, 1, "foreign keys off on the second connection");
    let account = store
        .adopt_or_create_account("me@example.fr", "gmail")
        .unwrap();
    store.create_mailbox(account, "INBOX", 1).unwrap();
    store.delete_account(account).unwrap();
    let mailboxes: i64 = store
        .conn()
        .query_row("SELECT COUNT(*) FROM mailboxes", [], |row| row.get(0))
        .unwrap();
    assert_eq!(
        mailboxes, 0,
        "the cascade of the deleted account did not fire"
    );
    drop(store);
    let _ = std::fs::remove_file(&path);
}

/// A database from the field carries the senders index with TWO
/// columns; on reopen it gains the mailbox (same pattern as the date
/// index below).
#[test]
fn the_inherited_senders_index_gains_the_mailbox_on_reopen() {
    let path = std::env::temp_dir().join(format!("wind-test-idx-sender-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let read_sql = |conn: &Connection| -> String {
        conn.query_row(
            "SELECT sql FROM sqlite_master WHERE name = 'idx_envelopes_sender'",
            [],
            |row| row.get(0),
        )
        .unwrap()
    };
    {
        let store = Store::open(&path).unwrap();
        store
            .conn()
            .execute_batch(
                "DROP INDEX idx_envelopes_sender;
                     CREATE INDEX idx_envelopes_sender
                         ON envelopes(sender_norm, date_epoch);",
            )
            .unwrap();
        assert!(!read_sql(store.conn()).contains("mailbox_id"));
    }
    Store::forget_initialization(&path);
    let store = Store::open(&path).unwrap();
    assert!(
        read_sql(store.conn()).contains("mailbox_id"),
        "the inherited index was not rebuilt"
    );
    drop(store);
    let _ = std::fs::remove_file(&path);
}

/// PLAN-DEMARRAGE, E1-bis — the envelopes date index gains `uid`,
/// and **`CREATE INDEX IF NOT EXISTS` is NOT enough**: on an existing
/// database the index already carries that name, the creation is a
/// silent no-op, and the defect would survive the update. The
/// migration therefore reads its DEFINITION, not its name.
///
/// Without this test, the rebuild branch is **never exercised**:
/// every database born from a `Store::open` carries the up-to-date
/// index straight from `SCHEMA`, and `migrate()` has nothing left to
/// do. The index must therefore be downgraded by hand to exercise
/// the field's code path.
#[test]
fn the_inherited_date_index_gains_uid_on_reopen() {
    let path = std::env::temp_dir().join(format!("wind-test-idx-date-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);

    let read_sql = |store: &Store| -> String {
        store
            .conn()
            .query_row(
                "SELECT sql FROM sqlite_master WHERE name = 'idx_envelopes_date'",
                [],
                |row| row.get(0),
            )
            .unwrap()
    };

    {
        let store = Store::open(&path).unwrap();
        // Downgrades the index to its shape from before the job — the
        // exact state of any database in the field at update time.
        store
            .conn()
            .execute_batch(
                "DROP INDEX idx_envelopes_date;
                     CREATE INDEX idx_envelopes_date
                         ON envelopes(mailbox_id, date_epoch DESC);",
            )
            .unwrap();
        assert!(
            !read_sql(&store).contains("uid"),
            "the fixture must start from the SHORT index, otherwise the test proves nothing"
        );
    }

    Store::forget_initialization(&path);
    let store = Store::open(&path).unwrap();
    let sql = read_sql(&store);
    assert!(
            sql.contains("uid"),
            "the inherited index was not rebuilt on open — the definition probe does nothing, and the field would keep the defect.
SQL: {sql}"
        );
    drop(store);
    let _ = std::fs::remove_file(&path);
}

/// PLAN-DEMARRAGE, defect 01 — the probe "how many bodies are
/// missing?" held the GLOBAL LOCK on commands **8,870 ms at every
/// startup** (20,839 ms in pure SQL cold), measured on 2026-08-26
/// on the field database: 251,466 bodies, 11.4 GB.
///
/// The cause was not the join. It was reading one COLUMN of
/// `bodies`: absent from the primary key's auto-index, it forced
/// SQLite to fetch the ROW — 56 KB on average — to read one bit.
/// 251k random reads across 11.4 GB.
///
/// The plan says it in one word: `COVERING`. As long as the
/// subquery reads NO column of `bodies`, the existence of the row
/// is decided from the index alone. Add a column to it one day,
/// and the word disappears — that, and nothing else, is what this
/// test guards.
///
/// We query the plan rather than a stopwatch: a duration depends
/// on the machine, an execution plan does not.
#[test]
fn missing_body_probes_never_fetch_the_fat_row() {
    let (mut store, inbox) = store_with_mailbox();
    let envelopes: Vec<Envelope> = (1..=40u32)
        .map(|uid| envelope(uid, "Subject", 1_600_000_000 + i64::from(uid), true))
        .collect();
    store.upsert_envelopes(inbox, &envelopes).unwrap();
    // Bodies for three quarters: the subquery must have both rows
    // to find AND rows not to find.
    for uid in 1..=30u32 {
        store.save_body(inbox, uid, "<p>body</p>", &[]).unwrap();
    }

    let mut count = store
        .0
        .prepare(&format!(
            "EXPLAIN QUERY PLAN {}",
            bodies_pending_count_sql()
        ))
        .unwrap();
    let count_plan: Vec<String> = count
        .query_map(params![1i64, "INBOX", 0i64], |row| row.get::<_, String>(3))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();

    let mut list = store
        .0
        .prepare(&format!("EXPLAIN QUERY PLAN {}", bodies_to_backfill_sql()))
        .unwrap();
    let list_plan: Vec<String> = list
        .query_map(params![1i64, "INBOX", 0i64, 10i64], |row| {
            row.get::<_, String>(3)
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();

    for (what, plan) in [
        ("the count of missing", &count_plan),
        ("the backfill work list", &list_plan),
    ] {
        for (alias, table) in [(" e ", "envelopes"), (" b ", "bodies")] {
            let step = plan
                .iter()
                .find(|step| step.contains(alias))
                .unwrap_or_else(|| {
                    panic!(
                        "{what}: no step touches `{table}`.\nPlan:\n{}",
                        plan.join("\n")
                    )
                });
            assert!(
                step.contains("COVERING"),
                "{what}: access to `{table}` is NOT covered by its \
index — SQLite fetches the row to read a column the index does not \
carry. That is the PLAN-DEMARRAGE defect, on BOTH sides: 8,870 ms of \
lock held on the `bodies` side, 521.9 ms of probe on the `envelopes` \
side.\n\
Step: {step}\nPlan:\n{}",
                plan.join("\n")
            );
        }
    }
}

/// R4 (PLAN-RETOURS-7): a pinned conversation is served SEPARATELY
/// (`pinned_unified_scoped`) and LEAVES the paginated flow along
/// with its count (decision D5: the list never shows the same
/// message twice). Unpinning returns it to the flow. The pin is
/// bounded to the account and follows the "Unread" tab like the
/// page.
#[test]
fn a_pin_serves_its_conversation_separately_and_out_of_the_flow() {
    let (mut store, inbox) = store_with_mailbox();
    store
        .upsert_envelopes(
            inbox,
            &[
                envelope(1, "old", 100, true),
                envelope(2, "middle", 200, true),
                envelope(3, "recent", 300, true),
            ],
        )
        .unwrap();
    assert!(
        store
            .pinned_unified_scoped(None, false, false)
            .unwrap()
            .is_empty()
    );

    assert!(store.toggle_pin(inbox, 1, 1_000).unwrap(), "pinned");
    let pinned = store.pinned_unified_scoped(None, false, false).unwrap();
    assert_eq!(pinned.len(), 1);
    assert_eq!(pinned[0].envelope.uid, 1);
    let flow = store.unified_recent_scoped(None, false, 0, 10).unwrap();
    assert!(
        flow.iter().all(|row| row.envelope.uid != 1),
        "the pinned conversation leaves the flow"
    );
    assert_eq!(flow.len(), 2);
    assert_eq!(store.unified_count_scoped(None, false).unwrap(), 2);
    // Scope bounds: an OTHER account does not have this pin, and
    // the "Unread" tab does not show it (everything is read here).
    assert!(
        store
            .pinned_unified_scoped(Some(999), false, false)
            .unwrap()
            .is_empty()
    );
    assert!(
        store
            .pinned_unified_scoped(None, true, false)
            .unwrap()
            .is_empty()
    );

    assert!(!store.toggle_pin(inbox, 1, 1_001).unwrap(), "unpinned");
    assert!(
        store
            .pinned_unified_scoped(None, false, false)
            .unwrap()
            .is_empty()
    );
    assert_eq!(store.unified_count_scoped(None, false).unwrap(), 3);
}

/// R1 (PLAN-RETOURS-11, D1-D2): the "Show images" choice is an
/// EXPLICIT exception written to the database, per MESSAGE
/// (envelope key, `pins` pattern) — reopening the message does not
/// ask again, and the neighboring message inherits nothing.
#[test]
fn the_image_choice_per_message_persists_and_does_not_bleed_over() {
    let (mut store, inbox) = store_with_mailbox();
    store
        .upsert_envelopes(
            inbox,
            &[envelope(1, "a", 100, true), envelope(2, "b", 200, true)],
        )
        .unwrap();
    assert!(
        !store.images_allowed(inbox, 1).unwrap(),
        "blocked by default"
    );
    store.allow_images_message(inbox, 1, 1_000).unwrap();
    assert!(store.images_allowed(inbox, 1).unwrap());
    assert!(
        !store.images_allowed(inbox, 2).unwrap(),
        "the choice is PER message"
    );
}

/// R1 (D3-D4): the sender rule is set FROM a message — the address
/// is read from the ENVELOPE (never from the UI), normalized to
/// lowercase — covers all its messages, and can be listed and
/// revoked.
#[test]
fn the_sender_rule_covers_its_messages_and_can_be_revoked() {
    let (mut store, inbox) = store_with_mailbox();
    let mut sender = envelope(1, "a", 100, true);
    sender.sender_address = Some("No-Reply@Registrar.FR".to_string());
    let mut same = envelope(2, "b", 200, true);
    same.sender_address = Some("no-reply@registrar.fr".to_string());
    let third_party = envelope(3, "c", 300, true); // alice@example.com
    store
        .upsert_envelopes(inbox, &[sender, same, third_party])
        .unwrap();

    let applied = store.allow_images_sender_of(inbox, 1, 1_000).unwrap();
    assert_eq!(
        applied.as_deref(),
        Some("no-reply@registrar.fr"),
        "the applied address is normalized"
    );
    assert!(store.images_allowed(inbox, 1).unwrap());
    assert!(
        store.images_allowed(inbox, 2).unwrap(),
        "all of the sender's messages, whatever the case"
    );
    assert!(
        !store.images_allowed(inbox, 3).unwrap(),
        "never a third party"
    );
    assert_eq!(
        store.images_senders().unwrap(),
        vec!["no-reply@registrar.fr".to_string()]
    );

    store.revoke_images_sender("no-reply@registrar.fr").unwrap();
    assert!(store.images_senders().unwrap().is_empty());
    assert!(
        !store.images_allowed(inbox, 1).unwrap(),
        "revoked — the guard returns"
    );
}

/// R1 (review 2026-08-28): the PER-MESSAGE image consent dies on a
/// UIDVALIDITY change — a recycled UID must NEVER inherit a
/// consent (a stranger's tracking pixel would fire with no banner
/// and no gesture). Same contract as `invitations`/`attachments`
/// in `reset_mailbox`.
#[test]
fn the_uidvalidity_reset_purges_the_per_message_image_memory() {
    let (mut store, inbox) = store_with_mailbox();
    store
        .upsert_envelopes(inbox, &[envelope(1, "a", 100, true)])
        .unwrap();
    store.allow_images_message(inbox, 1, 1_000).unwrap();
    assert!(store.images_allowed(inbox, 1).unwrap());

    store.reset_mailbox(inbox, 2).unwrap();
    store
        .upsert_envelopes(inbox, &[envelope(1, "something else entirely", 200, true)])
        .unwrap();
    assert!(
        !store.images_allowed(inbox, 1).unwrap(),
        "a recycled UID inherits no consent"
    );
}

/// R1: an envelope WITHOUT a sender address sets NOTHING — never
/// an empty rule that would grant who-knows-what.
#[test]
fn no_sender_address_no_rule() {
    let (mut store, inbox) = store_with_mailbox();
    let mut without = envelope(1, "a", 100, true);
    without.sender_address = None;
    store.upsert_envelopes(inbox, &[without]).unwrap();
    assert_eq!(store.allow_images_sender_of(inbox, 1, 1_000).unwrap(), None);
    assert!(store.images_senders().unwrap().is_empty());
    assert!(!store.images_allowed(inbox, 1).unwrap());
}

/// R4: the pin follows the THREAD — set on a message, it holds
/// when a reply moves the head of the conversation; `pin_state`
/// answers per thread, and unpinning from the NEW head releases
/// the whole thread.
#[test]
fn a_pin_follows_the_thread_and_its_new_head() {
    let (mut store, inbox) = store_with_mailbox();
    store
        .upsert_envelopes(inbox, &[envelope(1, "subject", 100, true)])
        .unwrap();
    assert!(store.toggle_pin(inbox, 1, 1_000).unwrap());

    let mut reply = envelope(2, "Re: subject", 400, true);
    reply.in_reply_to = Some("<m1@example.com>".to_string());
    store.upsert_envelopes(inbox, &[reply]).unwrap();

    let pinned = store.pinned_unified_scoped(None, false, false).unwrap();
    assert_eq!(pinned.len(), 1, "a pinned thread = ONE row");
    assert_eq!(
        pinned[0].envelope.uid, 2,
        "the row is the head of the thread"
    );
    assert_eq!(pinned[0].thread_size, 2);
    assert!(
        store.pin_state(inbox, 2).unwrap(),
        "the state is read per thread"
    );

    assert!(
        !store.toggle_pin(inbox, 2, 1_001).unwrap(),
        "unpinned from the new head"
    );
    assert!(
        store
            .pinned_unified_scoped(None, false, false)
            .unwrap()
            .is_empty()
    );
    assert!(!store.pin_state(inbox, 1).unwrap());
    assert_eq!(store.unified_count_scoped(None, false).unwrap(), 1);
}

/// PLAN-MODE-ORGANISE E1 (D1: routing is LOCAL only, `images_expediteurs`
/// pattern). Setting it normalizes the address through THE SAME
/// authority as the image guard, overwrites the previous decision
/// (a single verdict per sender), and "Reinstate" = DELETE —
/// whatever the case supplied by the caller.
#[test]
fn routing_set_normalizes_overwrites_and_can_be_removed() {
    let store = Store::open_in_memory().unwrap();
    store
        .route_sender("  Ada@Exemple.FR ", "kiosque", None, 1_700_000_000)
        .unwrap();
    let r = store.routing_of("ada@exemple.fr").unwrap().unwrap();
    assert_eq!(
        (r.destination.as_str(), r.rule.as_deref()),
        ("kiosque", None)
    );
    store
        .route_sender("ada@exemple.fr", "ecarte", Some("corbeille"), 1_700_000_100)
        .unwrap();
    let r = store.routing_of("ADA@EXEMPLE.FR").unwrap().unwrap();
    assert_eq!(
        (r.destination.as_str(), r.rule.as_deref()),
        ("ecarte", Some("corbeille"))
    );
    store.remove_routing(" ada@EXEMPLE.fr ").unwrap();
    assert!(store.routing_of("ada@exemple.fr").unwrap().is_none());
}

/// The vocabulary is CLOSED: a destination or a rule outside the
/// table is refused BEFORE any write (a pure decision, never a
/// SQLite CHECK as the first line of defense); a rule only makes
/// sense on a screened-out sender; an empty address never writes a
/// phantom rule.
#[test]
fn routing_refuses_outside_the_vocabulary() {
    let store = Store::open_in_memory().unwrap();
    assert!(store.route_sender("a@b.fr", "poubelle", None, 1).is_err());
    assert!(
        store
            .route_sender("a@b.fr", "ecarte", Some("suppression-definitive"), 1)
            .is_err()
    );
    assert!(
        store
            .route_sender("a@b.fr", "kiosque", Some("corbeille"), 1)
            .is_err(),
        "a No rule on a served destination makes no sense"
    );
    assert!(store.route_sender("   ", "kiosque", None, 1).is_err());
    assert!(store.routings().unwrap().is_empty(), "nothing was written");
}

/// PLAN-MODE-ORGANISE E1: a page of the Feed or the Paper trail —
/// the Inbox's unified flow, bounded to threads whose HEAD comes
/// from a sender routed to that destination. Same skeleton, same
/// exclusions (pins), same sort as the Inbox; the probe is PK → PK
/// (spike S2: 0.209 ms at 200k, never a scan).
#[test]
fn the_feed_only_serves_routed_senders() {
    let (mut store, inbox) = store_with_mailbox();
    let mut letter = envelope(1, "The letter", 100, true);
    letter.sender_address = Some("Lettre@infolettre.fr".to_string());
    letter.message_id = Some("<l1@infolettre.fr>".to_string());
    let ordinary = envelope(2, "Hello", 200, false);
    store.upsert_envelopes(inbox, &[letter, ordinary]).unwrap();
    store
        .route_sender("lettre@infolettre.fr", "kiosque", None, 300)
        .unwrap();

    let feed = store
        .routing_unified_scoped("kiosque", None, false, 0, 10)
        .unwrap();
    assert_eq!(feed.len(), 1);
    assert_eq!(feed[0].envelope.uid, 1);
    assert_eq!(
        store.routing_count_scoped("kiosque", None, false).unwrap(),
        1
    );
    // The Paper trail is empty: the destination really filters.
    assert!(
        store
            .routing_unified_scoped("registre", None, false, 0, 10)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        store.routing_count_scoped("registre", None, false).unwrap(),
        0
    );
    // The Inbox, meanwhile, ALWAYS shows everything (E1: taking
    // items out of the flow is the job of step E2 — Screener
    // retention).
    assert_eq!(store.unified_count_scoped(None, false).unwrap(), 2);
}

/// The plan guard for serving the Feed (`pins` lesson): the
/// routing probe is played by KEYS (envelopes PK, routing PK) —
/// never a scan of `envelopes`.
#[test]
fn the_feed_never_scans_the_envelopes() {
    let store = Store::open_in_memory().unwrap();
    let plan: Vec<String> = store
        .0
        .prepare(&format!(
            "EXPLAIN QUERY PLAN {}",
            routing_page_sql(false, false)
        ))
        .unwrap()
        .query_map(params![10, 0, "kiosque"], |row| row.get::<_, String>(3))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let scans: Vec<&String> = plan
        .iter()
        .filter(|l| l.starts_with("SCAN") && l.contains("envelopes"))
        .collect();
    assert!(scans.is_empty(), "plan with an envelopes scan: {plan:?}");
}

/// Review E1: the HEAD of a thread is the last message across ALL
/// mailboxes — Sent included. The gesture and the filter must
/// never anchor on it: (1) "Move to…" from a thread where the
/// user replied last must route the CORRESPONDENT, never
/// themselves; (2) a thread routed to the Feed does not leave it
/// because we replied there; (3) a pinned routed thread stays
/// visible in its destination (pins are only surfaced in the
/// Inbox — excluding it here would make it disappear everywhere).
#[test]
fn routing_ignores_its_own_reply_and_keeps_pins() {
    let (mut store, inbox) = store_with_mailbox();
    // Sent items enter the grouping scope (ADR 0009) — without
    // which the reply would stay out of the thread and the
    // fixture would not replay the root (head = Sent).
    store
        .set_thread_scope(test_account(&store), Some("Envoyes"))
        .unwrap();
    let sent = store
        .create_mailbox(test_account(&store), "Envoyes", 1)
        .unwrap();
    let mut letter = envelope(1, "The letter", 100, true);
    letter.sender_address = Some("lettre@infolettre.fr".to_string());
    letter.message_id = Some("<l1@infolettre.fr>".to_string());
    store.upsert_envelopes(inbox, &[letter]).unwrap();
    // The user's reply, in Sent — it becomes the HEAD of the
    // thread (most recent date).
    let mut reply = envelope(1, "Re: The letter", 500, true);
    reply.sender_address = Some("test@exemple.fr".to_string());
    reply.message_id = Some("<r1@exemple.fr>".to_string());
    reply.in_reply_to = Some("<l1@infolettre.fr>".to_string());
    store.upsert_envelopes(sent, &[reply]).unwrap();

    // (1) The gesture from the head (the user's own reply) routes
    // the correspondent, never themselves.
    let address = store
        .route_sender_of(sent, 1, "kiosque", None, 600)
        .unwrap();
    assert_eq!(address.as_deref(), Some("lettre@infolettre.fr"));
    // (2) The thread is in the Feed despite its "Sent" head.
    let feed = store
        .routing_unified_scoped("kiosque", None, false, 0, 10)
        .unwrap();
    assert_eq!(feed.len(), 1);
    assert_eq!(
        store.routing_count_scoped("kiosque", None, false).unwrap(),
        1
    );
    // (3) Pinned, it stays visible in the Feed — page AND total.
    assert!(store.toggle_pin(inbox, 1, 700).unwrap());
    assert_eq!(
        store
            .routing_unified_scoped("kiosque", None, false, 0, 10)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        store.routing_count_scoped("kiosque", None, false).unwrap(),
        1
    );
}

/// "Move to…" (E1): the address is resolved from the ENVELOPE on
/// the core side — the UI never parses an address
/// (`allow_images_sender_of` pattern). Returns the routed
/// address; None if the envelope has no address (never a phantom
/// verdict).
#[test]
fn routing_from_the_envelope_resolves_the_address_in_the_core() {
    let (mut store, inbox) = store_with_mailbox();
    let mut env = envelope(1, "subject", 100, true);
    env.sender_address = Some("  ADA@Exemple.FR ".to_string());
    let mut without_address = envelope(2, "anonymous", 200, true);
    without_address.sender_address = None;
    store
        .upsert_envelopes(inbox, &[env, without_address])
        .unwrap();

    let address = store
        .route_sender_of(inbox, 1, "registre", None, 300)
        .unwrap();
    assert_eq!(address.as_deref(), Some("ada@exemple.fr"));
    assert_eq!(
        store
            .routing_of("ada@exemple.fr")
            .unwrap()
            .unwrap()
            .destination,
        "registre"
    );
    assert_eq!(
        store
            .route_sender_of(inbox, 2, "kiosque", None, 400)
            .unwrap(),
        None
    );
    assert_eq!(
        store.routings().unwrap().len(),
        1,
        "nothing written without an address"
    );
}

/// Organized mode lives in SQLite `prefs` (D2 amended: Rust must
/// read the state — the No rules turn off with it) and the
/// FIRST-ACTIVATION EPOCH NEVER moves (D3 "arrivals only": it is
/// what bounds Screener retention; rewriting it on every toggle
/// would silently dump or hold back mail). Off by default, the
/// state and the epoch are written TOGETHER on first activation
/// (never one without the other).
#[test]
fn organized_mode_keeps_the_first_activation_epoch() {
    let mut store = Store::open_in_memory().unwrap();
    assert!(!store.organized_mode().unwrap());
    assert_eq!(store.organized_mode_epoch().unwrap(), None);
    store.set_organized_mode(true, 100).unwrap();
    assert!(store.organized_mode().unwrap());
    assert_eq!(store.organized_mode_epoch().unwrap(), Some(100));
    store.set_organized_mode(false, 200).unwrap();
    assert!(!store.organized_mode().unwrap());
    store.set_organized_mode(true, 300).unwrap();
    assert_eq!(
        store.organized_mode_epoch().unwrap(),
        Some(100),
        "the FIRST activation epoch is set in stone"
    );
}

/// RETOURS-13 R10 — the Feed's "read" memory (`pins`/`mis_de_cote`
/// pattern: envelope key, local to the workstation). A card read
/// down to the bottom gets marked; the mark is idempotent, dies
/// with its mailbox (`reset_mailbox`) and with its message
/// (`remove_local`) — a recycled UID inherits no read state.
#[test]
fn feed_read_gets_marked_and_dies_with_its_mailbox_and_its_message() {
    let (mut store, inbox) = store_with_mailbox();
    store
        .upsert_envelopes(inbox, &[envelope(1, "letter", 1_000, false)])
        .unwrap();
    store
        .upsert_envelopes(inbox, &[envelope(2, "other", 1_100, false)])
        .unwrap();
    assert!(!store.feed_read(inbox, 1).unwrap());
    store.mark_feed_read(inbox, 1, 2_000).unwrap();
    store.mark_feed_read(inbox, 1, 2_100).unwrap(); // idempotent
    assert!(store.feed_read(inbox, 1).unwrap());
    store.mark_feed_read(inbox, 2, 2_200).unwrap();
    // The message leaves: its mark leaves too.
    store.remove_local(inbox, 1).unwrap();
    assert!(!store.feed_read(inbox, 1).unwrap());
    // The mailbox resets: no more marks at all.
    store.reset_mailbox(inbox, 2).unwrap();
    assert!(!store.feed_read(inbox, 2).unwrap());
}

/// RETOURS-14 R8 (field 2026-08-31) — a YES to the Screener means
/// trust: the verdict ALSO sets the rule "always show this
/// sender's images" (`images_expediteurs` table, revocable in
/// Settings > Display like any rule). A No sets nothing and
/// removes nothing — the image guard has its own exit door.
#[test]
fn a_yes_to_the_screener_allows_the_senders_images() {
    let (mut store, inbox) = store_with_mailbox();
    let mut welcome = envelope(1, "Hello", 100, false);
    welcome.sender_address = Some("Ami@exemple.fr".to_string());
    welcome.message_id = Some("<a1@exemple.fr>".to_string());
    let mut intruder = envelope(2, "Promo", 200, false);
    intruder.sender_address = Some("promo@exemple.fr".to_string());
    intruder.message_id = Some("<p1@exemple.fr>".to_string());
    store.upsert_envelopes(inbox, &[welcome, intruder]).unwrap();
    assert!(!store.images_allowed(inbox, 1).unwrap());

    // The Yes (any served destination) sets the rule — address
    // normalized by THE gate (images_address).
    store
        .route_sender("ami@exemple.fr", "reception", None, 300)
        .unwrap();
    assert!(store.images_allowed(inbox, 1).unwrap());
    // The No allows nothing.
    store
        .route_sender("promo@exemple.fr", "ecarte", Some("spam"), 300)
        .unwrap();
    assert!(!store.images_allowed(inbox, 2).unwrap());
    // The pre-existing exit door undoes the rule set by the Yes.
    store.revoke_images_sender("ami@exemple.fr").unwrap();
    assert!(!store.images_allowed(inbox, 1).unwrap());
}

/// RETOURS-14 R6 (D7) — the Paper trail groups by SENDER, groups
/// sorted by the recency of the last message (Cleanup pattern),
/// and a group's page returns the threads of that one sender, in
/// the view's sort order.
#[test]
fn the_paper_trail_groups_by_sender_by_recency() {
    let (mut store, inbox) = store_with_mailbox();
    let mut old = envelope(1, "Receipt A", 100, true);
    old.sender_address = Some("recu@boutique.fr".to_string());
    old.message_id = Some("<r1@boutique.fr>".to_string());
    let mut recent = envelope(2, "Notice B", 300, true);
    recent.sender_address = Some("avis@banque.fr".to_string());
    recent.message_id = Some("<b1@banque.fr>".to_string());
    let mut second = envelope(3, "Receipt C", 200, true);
    second.sender_address = Some("recu@boutique.fr".to_string());
    second.message_id = Some("<r2@boutique.fr>".to_string());
    let outside = envelope(4, "Hello", 400, false);
    store
        .upsert_envelopes(inbox, &[old, recent, second, outside])
        .unwrap();
    store
        .route_sender("recu@boutique.fr", "registre", None, 500)
        .unwrap();
    store
        .route_sender("avis@banque.fr", "registre", None, 500)
        .unwrap();

    let groups = store.paper_trail_groups(None).unwrap();
    assert_eq!(groups.len(), 2, "one group per routed sender");
    // Recency first (D7): banque (300) before boutique (200).
    assert_eq!(groups[0].address, "avis@banque.fr");
    assert_eq!(groups[0].threads, 1);
    assert_eq!(groups[1].address, "recu@boutique.fr");
    assert_eq!(groups[1].threads, 2);
    assert_eq!(groups[1].last_epoch, 200);
    assert_eq!(groups[1].last_subject.as_deref(), Some("Receipt C"));

    // A group's page: the threads of THIS one sender, most recent
    // first.
    let page = store
        .paper_trail_group_scoped("recu@boutique.fr", None, 0, 10)
        .unwrap();
    assert_eq!(page.len(), 2);
    assert_eq!(page[0].envelope.uid, 3);
    assert_eq!(page[1].envelope.uid, 1);
    // The account filter bounds it like everywhere else.
    let other = store
        .paper_trail_group_scoped("recu@boutique.fr", Some(999), 0, 10)
        .unwrap();
    assert!(other.is_empty());
}

/// RETOURS-14 R7 (D8) — the Feed's nav badge counts cards NOT YET
/// OPENED (`kiosque_lus` memory), never the IMAP `seen` flag: that
/// is the semantics of the page itself (the Unread / Previously
/// read sections). The fixture is seen server-side (`seen =
/// true`): if the query counted `unseen`, it would return zero.
#[test]
fn the_feed_badge_counts_never_opened_cards() {
    let (mut store, inbox) = store_with_mailbox();
    let mut a = envelope(1, "Letter A", 100, true);
    a.sender_address = Some("lettre@infolettre.fr".to_string());
    a.message_id = Some("<a@infolettre.fr>".to_string());
    let mut b = envelope(2, "Letter B", 200, true);
    b.sender_address = Some("lettre@infolettre.fr".to_string());
    b.message_id = Some("<b@infolettre.fr>".to_string());
    let ordinary = envelope(3, "Hello", 300, false);
    store.upsert_envelopes(inbox, &[a, b, ordinary]).unwrap();
    store
        .route_sender("lettre@infolettre.fr", "kiosque", None, 400)
        .unwrap();

    // Two cards in the Feed, none opened — the IMAP seen flag
    // (true) does not count; neither does the unrouted message.
    assert_eq!(store.feed_unopened(None).unwrap(), 2);
    // The account filter is proven WHILE some unread remains
    // (review: at zero everywhere, an ignored filter would pass
    // green): the right account sees 2, a foreign account 0.
    let account = test_account(&store);
    assert_eq!(store.feed_unopened(Some(account)).unwrap(), 2);
    assert_eq!(store.feed_unopened(Some(account + 1)).unwrap(), 0);
    // Opening a card removes it from the count.
    store.mark_feed_read(inbox, 2, 500).unwrap();
    assert_eq!(store.feed_unopened(None).unwrap(), 1);
    store.mark_feed_read(inbox, 1, 600).unwrap();
    assert_eq!(store.feed_unopened(None).unwrap(), 0);
}

/// RETOURS-13 R5/R9 — the Screener buttons' DEFAULT actions:
/// shipped as Yes → Inbox, No → Trash; configurable within a
/// CLOSED vocabulary (the Yes destinations, the No rules plus
/// "screen out without moving"); a corrupted pref falls back to
/// the default — never a verdict with a broken vocabulary.
#[test]
fn screener_defaults_ship_then_configurable_within_the_closed_vocabulary() {
    let mut store = Store::open_in_memory().unwrap();
    assert_eq!(
        store.screener_defaults().unwrap(),
        ("reception".to_string(), "corbeille".to_string()),
        "the shipped defaults: Yes → Inbox, No → Trash"
    );
    store.set_screener_defaults("kiosque", "archive").unwrap();
    assert_eq!(
        store.screener_defaults().unwrap(),
        ("kiosque".to_string(), "archive".to_string())
    );
    store.set_screener_defaults("reception", "ecarte").unwrap();
    assert_eq!(store.screener_defaults().unwrap().1, "ecarte");
    // The vocabulary is closed: "ecarte" is not a Yes, a
    // destination is not a No rule.
    assert!(store.set_screener_defaults("ecarte", "corbeille").is_err());
    assert!(
        store
            .set_screener_defaults("reception", "registre")
            .is_err()
    );
    // A corrupted pref (written outside the gate) falls back to
    // the default.
    store
        .set_text_pref("portier_defaut_oui", "poubelle")
        .unwrap();
    assert_eq!(store.screener_defaults().unwrap().0, "reception");
}

/// PLAN-MODE-ORGANISE E2 — Screener retention (D3 "arrivals
/// only"). A sender WITHOUT a routing row whose mail only exists
/// AFTER the activation epoch waits at the Screener: its thread
/// leaves the flow AND the totals of the organized Inbox (shared
/// exclusion, `pins` lesson). A known sender's history stays in
/// the Inbox, and CLASSIC mode does not move a single message.
#[test]
fn an_unknown_sender_after_the_epoch_waits_at_the_screener_out_of_the_flow_and_totals() {
    let (mut store, inbox) = store_with_mailbox();
    store.set_organized_mode(true, 1_000).unwrap();
    // The known one: mail before AND after the epoch.
    let mut before = envelope(1, "from yesterday", 500, true);
    before.sender_address = Some("ancien@exemple.fr".to_string());
    let mut after = envelope(2, "from today", 1_500, false);
    after.sender_address = Some("ancien@exemple.fr".to_string());
    // The unknown one: first message AFTER the epoch.
    let mut unknown = envelope(3, "first time", 1_600, false);
    unknown.sender = Some("New Arrival".to_string());
    unknown.sender_address = Some("Nouv@Exemple.FR".to_string());
    store
        .upsert_envelopes(inbox, &[before, after, unknown])
        .unwrap();

    let page = store.organized_inbox_scoped(None, false, 0, 10).unwrap();
    assert_eq!(
        page.iter().map(|r| r.envelope.uid).collect::<Vec<_>>(),
        vec![2, 1],
        "the organized Inbox only serves the known sender"
    );
    assert_eq!(
        store.organized_inbox_count_scoped(None, false).unwrap(),
        2,
        "the total follows the flow (shared exclusion)"
    );
    assert_eq!(
        store.unified_count_scoped(None, false).unwrap(),
        3,
        "classic mode ALWAYS shows everything"
    );
    let waiting = store.screener_waiting().unwrap();
    assert_eq!(waiting.len(), 1);
    assert_eq!(waiting[0].address, "nouv@exemple.fr");
    assert_eq!(
        waiting[0].row.envelope.uid, 3,
        "the rank carries its last message"
    );
    assert_eq!(store.screener_total().unwrap(), 1);
}

/// The Screener gate: a plain Yes returns the sender to the
/// Inbox, a No with a rule screens it out — in BOTH cases it
/// leaves the waiting list, and the history records the rule
/// chosen.
#[test]
fn a_yes_releases_a_no_screens_out_and_the_waiting_list_empties() {
    let (mut store, inbox) = store_with_mailbox();
    store.set_organized_mode(true, 1_000).unwrap();
    let mut a = envelope(1, "hello", 1_500, false);
    a.sender_address = Some("a@exemple.fr".to_string());
    let mut b = envelope(2, "offer", 1_600, false);
    b.sender_address = Some("b@exemple.fr".to_string());
    store.upsert_envelopes(inbox, &[a, b]).unwrap();
    assert_eq!(store.screener_waiting().unwrap().len(), 2);
    assert_eq!(store.organized_inbox_count_scoped(None, false).unwrap(), 0);

    // Plain Yes → Inbox: the thread comes back, page AND total.
    store
        .route_sender("a@exemple.fr", "reception", None, 2_000)
        .unwrap();
    assert_eq!(
        store
            .screener_waiting()
            .unwrap()
            .iter()
            .map(|r| r.address.as_str())
            .collect::<Vec<_>>(),
        vec!["b@exemple.fr"]
    );
    let page = store.organized_inbox_scoped(None, false, 0, 10).unwrap();
    assert_eq!(page.len(), 1);
    assert_eq!(page[0].envelope.uid, 1);
    assert_eq!(store.organized_inbox_count_scoped(None, false).unwrap(), 1);

    // No with a rule → screened out: out of the Inbox, out of
    // every served view, and the history carries the rule.
    store
        .route_sender("b@exemple.fr", "ecarte", Some("archive"), 2_100)
        .unwrap();
    assert!(store.screener_waiting().unwrap().is_empty());
    assert_eq!(store.screener_total().unwrap(), 0);
    assert_eq!(
        store.organized_inbox_count_scoped(None, false).unwrap(),
        1,
        "the screened-out sender does not return to the Inbox"
    );
    assert!(
        store
            .routing_unified_scoped("kiosque", None, false, 0, 10)
            .unwrap()
            .is_empty(),
        "screened out is not a served view"
    );
    let verdict = store.routing_of("b@exemple.fr").unwrap().unwrap();
    assert_eq!(
        (verdict.destination.as_str(), verdict.rule.as_deref()),
        ("ecarte", Some("archive"))
    );
}

/// "Reinstate" from the history = DELETE of the row: a
/// screened-out unknown sender RETURNS to the Screener (their
/// messages reappear), a routed known sender simply returns to
/// the Inbox — never to the Screener, their pre-epoch mail is
/// proof enough.
#[test]
fn reinstating_returns_the_unknown_sender_to_the_screener_and_the_known_one_to_the_inbox() {
    let (mut store, inbox) = store_with_mailbox();
    store.set_organized_mode(true, 1_000).unwrap();
    let mut known = envelope(1, "from yesterday", 500, true);
    known.sender_address = Some("ancien@exemple.fr".to_string());
    let mut unknown = envelope(2, "first time", 1_500, false);
    unknown.sender_address = Some("nouv@exemple.fr".to_string());
    store.upsert_envelopes(inbox, &[known, unknown]).unwrap();
    store
        .route_sender("nouv@exemple.fr", "ecarte", Some("spam"), 2_000)
        .unwrap();
    store
        .route_sender("ancien@exemple.fr", "kiosque", None, 2_000)
        .unwrap();
    assert!(store.screener_waiting().unwrap().is_empty());
    assert_eq!(store.organized_inbox_count_scoped(None, false).unwrap(), 0);

    store.remove_routing("nouv@exemple.fr").unwrap();
    let waiting = store.screener_waiting().unwrap();
    assert_eq!(
        waiting.len(),
        1,
        "the reinstated unknown sender waits again at the Screener"
    );
    assert_eq!(waiting[0].address, "nouv@exemple.fr");

    store.remove_routing("ancien@exemple.fr").unwrap();
    assert_eq!(
        store.screener_waiting().unwrap().len(),
        1,
        "the known sender NEVER goes through the Screener: their pre-epoch mail is proof enough"
    );
    assert_eq!(
        store.organized_inbox_count_scoped(None, false).unwrap(),
        1,
        "the known sender is returned to the Inbox"
    );
}

/// Golden rule — never lose mail: a MIXED thread (an unknown
/// sender replies in a known sender's thread) STAYS in the Inbox;
/// the unknown sender still waits at the Screener. Retention only
/// takes a thread if it belongs ENTIRELY to waiting senders.
#[test]
fn a_mixed_thread_stays_in_the_inbox_and_the_unknown_sender_still_waits() {
    let (mut store, inbox) = store_with_mailbox();
    store.set_organized_mode(true, 1_000).unwrap();
    let mut yesterday = envelope(1, "yesterday", 500, true);
    yesterday.sender_address = Some("connu@exemple.fr".to_string());
    let mut root = envelope(2, "project", 1_500, false);
    root.sender_address = Some("connu@exemple.fr".to_string());
    store.upsert_envelopes(inbox, &[yesterday, root]).unwrap();
    let mut intruder = envelope(3, "Re: project", 1_600, false);
    intruder.sender_address = Some("nouv@exemple.fr".to_string());
    intruder.in_reply_to = Some("<m2@example.com>".to_string());
    store.upsert_envelopes(inbox, &[intruder]).unwrap();

    assert_eq!(
        store.organized_inbox_count_scoped(None, false).unwrap(),
        2,
        "the mixed thread and yesterday's thread stay in the Inbox"
    );
    let waiting = store.screener_waiting().unwrap();
    assert_eq!(
        waiting
            .iter()
            .map(|r| r.address.as_str())
            .collect::<Vec<_>>(),
        vec!["nouv@exemple.fr"],
        "the unknown sender waits at the Screener even though their thread is mixed"
    );
}

/// Never yourself at the Screener (E1 lesson "never your own
/// address"), and never a waiting entry without an address.
#[test]
fn never_yourself_or_without_an_address_at_the_screener() {
    let (mut store, inbox) = store_with_mailbox();
    store.set_organized_mode(true, 1_000).unwrap();
    let mut self_mail = envelope(1, "note to self", 1_500, false);
    self_mail.sender_address = Some("Test@Exemple.FR".to_string());
    let mut silent = envelope(2, "anonymous", 1_600, false);
    silent.sender_address = None;
    store.upsert_envelopes(inbox, &[self_mail, silent]).unwrap();
    assert!(store.screener_waiting().unwrap().is_empty());
    assert_eq!(
        store.organized_inbox_count_scoped(None, false).unwrap(),
        2,
        "nothing is held back: neither ourselves nor a message without an address"
    );
}

/// Sync does not arrive in order: if a sender's OLD mail
/// (predating the epoch) arrives AFTER their new mail, the
/// waiting entry wrongly set unwinds and the thread is released —
/// the sender was known, the database just did not know it yet.
#[test]
fn old_mail_arriving_after_the_fact_undoes_the_waiting_entry() {
    let (mut store, inbox) = store_with_mailbox();
    store.set_organized_mode(true, 1_000).unwrap();
    let mut new_mail = envelope(1, "recent", 1_500, false);
    new_mail.sender_address = Some("connu@exemple.fr".to_string());
    store.upsert_envelopes(inbox, &[new_mail]).unwrap();
    assert_eq!(store.screener_waiting().unwrap().len(), 1);

    let mut old_mail = envelope(2, "history arrives", 500, true);
    old_mail.sender_address = Some("connu@exemple.fr".to_string());
    store.upsert_envelopes(inbox, &[old_mail]).unwrap();
    assert!(
        store.screener_waiting().unwrap().is_empty(),
        "pre-epoch mail proves the sender is known"
    );
    assert_eq!(
        store.organized_inbox_count_scoped(None, false).unwrap(),
        2,
        "their threads are released, page and totals"
    );
}

/// Waiting entries are DERIVED from mail: when the mailbox resets
/// (UIDVALIDITY), the Screener ranks that no longer rest on
/// anything die with it (A43/A89 lesson — a recycled UID must
/// inherit no decision).
#[test]
fn the_waiting_entry_dies_with_the_mail_that_carried_it() {
    let (mut store, inbox) = store_with_mailbox();
    store.set_organized_mode(true, 1_000).unwrap();
    let mut unknown = envelope(1, "first time", 1_500, false);
    unknown.sender_address = Some("nouv@exemple.fr".to_string());
    store.upsert_envelopes(inbox, &[unknown]).unwrap();
    assert_eq!(store.screener_waiting().unwrap().len(), 1);

    store.reset_mailbox(inbox, 2).unwrap();
    assert!(
        store.screener_waiting().unwrap().is_empty(),
        "no more mail, no more waiting"
    );
    assert_eq!(store.screener_total().unwrap(), 0);
}

/// Review E2, golden rule — never lose mail: a No on an INTRUDER
/// (a screened-out sender who replied in a known sender's thread)
/// does not hide the known sender's thread. `ecarte` has NO
/// served view: hiding the mixed thread would make it disappear
/// everywhere. Only a thread ENTIRELY made of screened-out/waiting
/// senders gets hidden.
#[test]
fn a_no_on_an_intruder_does_not_hide_the_known_senders_thread() {
    let (mut store, inbox) = store_with_mailbox();
    store.set_organized_mode(true, 1_000).unwrap();
    let mut yesterday = envelope(1, "yesterday", 500, true);
    yesterday.sender_address = Some("connu@exemple.fr".to_string());
    let mut root = envelope(2, "project", 1_500, false);
    root.sender_address = Some("connu@exemple.fr".to_string());
    let mut intruder = envelope(3, "Re: project", 1_600, false);
    intruder.sender_address = Some("spam@exemple.fr".to_string());
    intruder.in_reply_to = Some("<m2@example.com>".to_string());
    store
        .upsert_envelopes(inbox, &[yesterday, root, intruder])
        .unwrap();
    // An unknown sender ALONE, screened out too: their thread,
    // entirely theirs, gets hidden — the contrast that proves the
    // rule.
    let mut alone = envelope(4, "offer", 1_700, false);
    alone.sender_address = Some("promo@exemple.fr".to_string());
    store.upsert_envelopes(inbox, &[alone]).unwrap();

    store
        .route_sender("spam@exemple.fr", "ecarte", Some("spam"), 2_000)
        .unwrap();
    store
        .route_sender("promo@exemple.fr", "ecarte", None, 2_000)
        .unwrap();
    let page = store.organized_inbox_scoped(None, false, 0, 10).unwrap();
    assert_eq!(
        page.iter().map(|r| r.envelope.uid).collect::<Vec<_>>(),
        vec![3, 1],
        "the known sender's mixed thread STAYS (intruder head included), the promo-only thread gets hidden"
    );
    assert_eq!(store.organized_inbox_count_scoped(None, false).unwrap(), 2);
    assert!(
        store
            .routing_unified_scoped("kiosque", None, false, 0, 10)
            .unwrap()
            .is_empty(),
        "screened out is not a served view"
    );
}

/// A message WITHOUT a Date header NEVER proves the known status:
/// treating it as predating the epoch would let it bypass the
/// very gate that exists to sort those senders (spam without a
/// Date is common) — and would undo a legitimate waiting entry.
#[test]
fn a_message_without_a_date_is_never_proof_of_a_known_sender() {
    let (mut store, inbox) = store_with_mailbox();
    store.set_organized_mode(true, 1_000).unwrap();
    let mut without_date = envelope(1, "no date", 0, false);
    without_date.sender_address = Some("nouv@exemple.fr".to_string());
    without_date.date = None;
    store.upsert_envelopes(inbox, &[without_date]).unwrap();
    assert_eq!(
        store.screener_waiting().unwrap().len(),
        1,
        "the dateless unknown sender waits at the gate — never a bypass"
    );

    let mut dated = envelope(2, "dated", 1_500, false);
    dated.sender_address = Some("autre@exemple.fr".to_string());
    let mut without_date2 = envelope(3, "re-no date", 0, false);
    without_date2.sender_address = Some("autre@exemple.fr".to_string());
    without_date2.date = None;
    store
        .upsert_envelopes(inbox, &[dated, without_date2])
        .unwrap();
    assert_eq!(
        store
            .screener_waiting()
            .unwrap()
            .iter()
            .filter(|r| r.address == "autre@exemple.fr")
            .count(),
        1,
        "a second dateless message does not undo the waiting entry"
    );
}

/// Reinstating follows the SAME rule as arrival (D3): only a
/// sender with mail that ARRIVED (INBOX) after the epoch waits at
/// the Screener again — a sender seen only in Archive or Junk
/// never went through the gate, and does not enter through the
/// exit door.
#[test]
fn reinstating_only_admits_arrivals() {
    let (mut store, inbox) = store_with_mailbox();
    store.set_organized_mode(true, 1_000).unwrap();
    let archive = store
        .create_mailbox(test_account(&store), "Archives", 1)
        .unwrap();
    let mut outside_the_gate = envelope(1, "seen in archive", 1_500, true);
    outside_the_gate.sender_address = Some("ailleurs@exemple.fr".to_string());
    store
        .upsert_envelopes(archive, &[outside_the_gate])
        .unwrap();
    let mut arrived = envelope(1, "arrived", 1_600, false);
    arrived.sender_address = Some("guichet@exemple.fr".to_string());
    store.upsert_envelopes(inbox, &[arrived]).unwrap();

    store
        .route_sender("ailleurs@exemple.fr", "ecarte", None, 2_000)
        .unwrap();
    store
        .route_sender("guichet@exemple.fr", "ecarte", None, 2_000)
        .unwrap();
    store.remove_routing("ailleurs@exemple.fr").unwrap();
    store.remove_routing("guichet@exemple.fr").unwrap();
    assert_eq!(
        store
            .screener_waiting()
            .unwrap()
            .iter()
            .map(|r| r.address.as_str())
            .collect::<Vec<_>>(),
        vec!["guichet@exemple.fr"],
        "only the arrival reinstates at the gate"
    );
}

/// The badge and the gate only report ARRIVALS: a message from
/// the same sender living elsewhere (trash, archive) is neither
/// counted nor served as a rank.
#[test]
fn the_gate_only_counts_arrivals() {
    let (mut store, inbox) = store_with_mailbox();
    store.set_organized_mode(true, 1_000).unwrap();
    let trash = store
        .create_mailbox(test_account(&store), "Corbeille", 1)
        .unwrap();
    let mut arrived = envelope(1, "arrived", 1_500, false);
    arrived.sender_address = Some("nouv@exemple.fr".to_string());
    store.upsert_envelopes(inbox, &[arrived]).unwrap();
    let mut thrown_away = envelope(1, "already thrown away", 1_600, false);
    thrown_away.sender_address = Some("nouv@exemple.fr".to_string());
    store.upsert_envelopes(trash, &[thrown_away]).unwrap();

    assert_eq!(
        store.screener_total().unwrap(),
        1,
        "the trash does not count"
    );
    let waiting = store.screener_waiting().unwrap();
    assert_eq!(waiting.len(), 1);
    assert_eq!(
        waiting[0].row.envelope.uid, 1,
        "the rank shows the arrival, never the discarded message"
    );
    assert_eq!(waiting[0].row.mailbox, "INBOX");
}

/// Shared exclusion extends to PINS and to the nav counter: in
/// the organized Inbox, a pinned thread routed to the Feed no
/// longer surfaces (it lives in its own view), and the unread
/// count of a held-back sender does not inflate the Inbox badge —
/// classic mode, meanwhile, does not move.
#[test]
fn pins_and_the_badge_follow_the_shared_exclusion() {
    let (mut store, inbox) = store_with_mailbox();
    store.set_organized_mode(true, 1_000).unwrap();
    let mut letter = envelope(1, "the letter", 500, false);
    letter.sender_address = Some("lettre@exemple.fr".to_string());
    let ordinary = envelope(2, "hello", 600, false);
    store.upsert_envelopes(inbox, &[letter, ordinary]).unwrap();
    assert!(store.toggle_pin(inbox, 1, 700).unwrap());
    store
        .route_sender("lettre@exemple.fr", "kiosque", None, 2_000)
        .unwrap();
    let mut held_back = envelope(3, "first time", 1_500, false);
    held_back.sender_address = Some("nouv@exemple.fr".to_string());
    store.upsert_envelopes(inbox, &[held_back]).unwrap();

    assert!(
        store
            .pinned_unified_scoped(None, false, true)
            .unwrap()
            .is_empty(),
        "a routed thread's pin no longer surfaces in the organized Inbox"
    );
    assert_eq!(
        store
            .pinned_unified_scoped(None, false, false)
            .unwrap()
            .len(),
        1,
        "classic mode keeps its pin"
    );
    let account = test_account(&store);
    let folders = store.canonical_folders(account).unwrap();
    let (organized, _) = store.nav_unread_counts(account, &folders, true).unwrap();
    assert_eq!(
        organized, 1,
        "only the ordinary unread message counts (the pinned routed one and the held-back one do not)"
    );
    let (classic, _) = store.nav_unread_counts(account, &folders, false).unwrap();
    assert_eq!(classic, 3);
}

/// E1 → E2 in the field: the mode may have been ACTIVATED before
/// this version (E1 in the field, on the CE's workstations) —
/// unknown senders who arrived between activation and the update
/// get caught up by the migration, otherwise they would bypass
/// the gate forever, silently. Fixture: an E2 database whose E2
/// artifacts (column + waiting entries) are erased to replay the
/// exact E1 state, then a reopen.
#[test]
fn the_migration_catches_up_the_waiting_list_of_a_pre_e2_database() {
    let path = std::env::temp_dir().join(format!(
        "wind-test-rattrapage-portier-{}.db",
        std::process::id()
    ));
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
    }
    {
        let mut store = Store::open(&path).unwrap();
        let account = store
            .adopt_or_create_account("test@exemple.fr", "gmail")
            .unwrap();
        let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        store.set_organized_mode(true, 1_000).unwrap();
        let mut known = envelope(1, "from yesterday", 500, true);
        known.sender_address = Some("ancien@exemple.fr".to_string());
        let mut unknown = envelope(2, "first time", 1_500, false);
        unknown.sender_address = Some("nouv@exemple.fr".to_string());
        store.upsert_envelopes(inbox, &[known, unknown]).unwrap();
        // Replays E1 state: neither the flag column nor a waiting
        // entry.
        // Reconstruction (not DROP COLUMN: SQLite chokes on the
        // comments in the stored SQL — "incomplete input").
        store
            .0
            .execute_batch(
                "DELETE FROM portier_attente;
                     PRAGMA foreign_keys = OFF;
                     CREATE TABLE threads_e1 AS
                       SELECT id, account_id, last_mailbox_id, last_uid,
                              last_epoch, size, unseen, inbox_size FROM threads;
                     DROP TABLE threads;
                     ALTER TABLE threads_e1 RENAME TO threads;
                     PRAGMA foreign_keys = ON;",
            )
            .unwrap();
    }
    Store::forget_initialization(&path);
    let store = Store::open(&path).unwrap();
    let waiting = store.screener_waiting().unwrap();
    assert_eq!(
        waiting
            .iter()
            .map(|r| r.address.as_str())
            .collect::<Vec<_>>(),
        vec!["nouv@exemple.fr"],
        "the pre-update unknown sender waits at the gate again"
    );
    assert_eq!(
        store.organized_inbox_count_scoped(None, false).unwrap(),
        1,
        "their thread is held back, the known sender's stays"
    );
    drop(store);
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
    }
}

/// PLAN-MODE-ORGANISE E3 — the No rules at sync time. A message
/// that ARRIVES from a screened-out sender WITH a rule is handled
/// as PLAN-HORIZON-NETTOYAGE panel B (D5-D8) — the cleanup
/// session: a single one, persisted; starting freezes the bound
/// and counts the groups; a GROUP verdict routes the future AND
/// processes the stock WITHIN THE RANGE (never what precedes it);
/// progress advances; finishing erases the session.
#[test]
fn cleanup_session_groups_verdicts_and_progress() {
    const DAY: i64 = 86_400;
    let now = 100 * DAY;
    let (mut store, inbox) = store_with_mailbox();
    store.set_organized_mode(true, 1_000).unwrap();

    let seed = |uid, subject: &str, epoch, address: &str| {
        let mut e = envelope(uid, subject, epoch, true);
        e.sender_address = Some(address.to_string());
        e
    };
    store
        .upsert_envelopes(
            inbox,
            &[
                seed(1, "letter", now - 2 * DAY, "un@exemple.fr"),
                seed(2, "follow-up", now - DAY, "un@exemple.fr"),
                seed(3, "offer", now - 3 * DAY, "deux@exemple.fr"),
                // The stock PREDATING the range from the same
                // sender: never touched by the verdict.
                seed(5, "very old offer", 500, "deux@exemple.fr"),
                // A sender entirely outside the range: not a
                // group.
                seed(4, "archive", 1_000, "vieux@exemple.fr"),
                // Already routed (D7): never asked again.
                seed(6, "news", now - DAY, "route@exemple.fr"),
                // Yourself: never a group.
                seed(7, "note to self", now - DAY, "test@exemple.fr"),
            ],
        )
        .unwrap();
    store
        .route_sender("route@exemple.fr", "kiosque", None, 2_000)
        .unwrap();

    assert!(store.cleanup_state().unwrap().is_none());
    assert!(
        store.cleanup_start("un siecle", "reception", now).is_err(),
        "the range vocabulary is closed"
    );
    assert!(
        store.cleanup_start("3m", "le grenier", now).is_err(),
        "the scope vocabulary is closed"
    );

    let session = store.cleanup_start("3m", "reception", now).unwrap();
    assert_eq!((session.total, session.handled), (2, 0));
    let groups = store.cleanup_groups().unwrap();
    assert_eq!(
        groups
            .iter()
            .map(|g| (g.address.as_str(), g.messages))
            .collect::<Vec<_>>(),
        vec![("un@exemple.fr", 2), ("deux@exemple.fr", 1)],
        "the range's groups, most recent first — routed, self and out-of-range excluded"
    );

    // Group Yes: routing only, no server action.
    store
        .cleanup_verdict("un@exemple.fr", "reception", None, now)
        .unwrap();
    assert!(store.pending_actions(inbox).unwrap().is_empty());
    let state = store.cleanup_state().unwrap().unwrap();
    assert_eq!((state.total, state.handled), (2, 1));
    assert_eq!(store.cleanup_groups().unwrap().len(), 1);

    // Navigating into a group: ITS messages from the range,
    // never what precedes it — the reading the sort screen offers
    // on click.
    let inside = store.cleanup_messages("deux@exemple.fr").unwrap();
    assert_eq!(
        inside.iter().map(|r| r.envelope.uid).collect::<Vec<_>>(),
        vec![3],
        "the group shows only its mail from the range"
    );

    // No + trash: the stock WITHIN THE RANGE leaves (uid 3), never
    // what precedes it (uid 5); the action is the server's trash.
    store
        .cleanup_verdict("deux@exemple.fr", "ecarte", Some("corbeille"), now)
        .unwrap();
    let actions = store.pending_actions(inbox).unwrap();
    assert_eq!(
        actions
            .iter()
            .map(|a| (a.uid, a.action.clone()))
            .collect::<Vec<_>>(),
        vec![(3, Action::Delete)],
        "the range's stock only — D4: never a permanent delete"
    );
    let account = test_account(&store);
    assert!(
        store.envelope(account, "INBOX", 5).unwrap().is_some(),
        "what predates the range stays in the database"
    );
    assert!(
        store.envelope(account, "INBOX", 3).unwrap().is_none(),
        "the processed stock leaves the local copy"
    );
    let state = store.cleanup_state().unwrap().unwrap();
    assert_eq!((state.total, state.handled), (2, 2));

    store.cleanup_finish().unwrap();
    assert!(store.cleanup_state().unwrap().is_none());
    assert!(
        store
            .cleanup_verdict("vieux@exemple.fr", "reception", None, now)
            .is_err(),
        "a verdict with no session in progress is refused"
    );
}

/// D6 (CE, verbatim): the scope is chosen — "Inbox only" ignores
/// user folders, "Inbox + Folders" covers them.
#[test]
fn cleanup_scope_inbox_or_folders() {
    const DAY: i64 = 86_400;
    let now = 100 * DAY;
    let (mut store, inbox) = store_with_mailbox();
    let account = test_account(&store);
    store.set_organized_mode(true, 1_000).unwrap();
    let projects = store.create_mailbox(account, "Projets", 1).unwrap();

    let mut inbox_msg = envelope(1, "hello", now - DAY, true);
    inbox_msg.sender_address = Some("un@exemple.fr".to_string());
    store.upsert_envelopes(inbox, &[inbox_msg]).unwrap();
    let mut filed = envelope(1, "filed", now - DAY, true);
    filed.sender_address = Some("proj@exemple.fr".to_string());
    store.upsert_envelopes(projects, &[filed]).unwrap();

    let session = store.cleanup_start("tout", "reception", now).unwrap();
    assert_eq!(session.total, 1, "Inbox only: the folder does not enter");
    store.cleanup_finish().unwrap();

    let session = store.cleanup_start("tout", "dossiers", now).unwrap();
    assert_eq!(session.total, 2, "Inbox + Folders: both groups");
    let addresses: Vec<_> = store
        .cleanup_groups()
        .unwrap()
        .into_iter()
        .map(|g| g.address)
        .collect();
    assert!(addresses.contains(&"proj@exemple.fr".to_string()));
}

/// Via the gesture path: a logged action (`pending_actions`,
/// replayed at the head of every sync) + local disappearance — no
/// echo (this is not a user gesture). `archive` → Archive,
/// `trash` → Delete (the server's trash, NEVER a permanent
/// delete — D4).
#[test]
fn the_no_rule_runs_on_arrival() {
    let (mut store, inbox) = store_with_mailbox();
    store.set_organized_mode(true, 1_000).unwrap();
    store
        .route_sender("promo@exemple.fr", "ecarte", Some("archive"), 2_000)
        .unwrap();
    store
        .route_sender("pub@exemple.fr", "ecarte", Some("corbeille"), 2_000)
        .unwrap();
    let mut offer = envelope(1, "offer", 2_500, false);
    offer.sender_address = Some("promo@exemple.fr".to_string());
    let mut follow_up = envelope(2, "follow-up", 2_600, false);
    follow_up.sender_address = Some("pub@exemple.fr".to_string());
    store.upsert_envelopes(inbox, &[offer, follow_up]).unwrap();

    assert_eq!(
        store.count(inbox).unwrap(),
        0,
        "both left the local mailbox"
    );
    let actions = store.pending_actions(inbox).unwrap();
    assert_eq!(
        actions
            .iter()
            .map(|a| (a.uid, a.action.clone()))
            .collect::<Vec<_>>(),
        vec![(1, Action::Archive), (2, Action::Delete)],
        "archive → Archive, corbeille → Delete (never permanent)"
    );
}

/// The `spam` rule goes to the account's RESOLVED junk folder
/// (`canonical_folders`, like the gesture); with no recognized
/// folder, we do NOTHING — never an invented destination (golden
/// rule).
#[test]
fn the_spam_rule_goes_to_the_resolved_junk_folder() {
    let (mut store, inbox) = store_with_mailbox();
    let account = test_account(&store);
    store.set_organized_mode(true, 1_000).unwrap();
    store
        .route_sender("arnaque@exemple.fr", "ecarte", Some("spam"), 2_000)
        .unwrap();
    // With no recognized junk folder: the message STAYS.
    let mut before = envelope(1, "before", 2_500, false);
    before.sender_address = Some("arnaque@exemple.fr".to_string());
    store.upsert_envelopes(inbox, &[before]).unwrap();
    assert_eq!(
        store.count(inbox).unwrap(),
        1,
        "with no recognized folder, nothing moves"
    );
    assert!(store.pending_actions(inbox).unwrap().is_empty());

    store
        .replace_folders(
            account,
            &[crate::Folder {
                wire: "Junk".to_string(),
                display: "Junk".to_string(),
                selectable: true,
                special_use: None,
            }],
        )
        .unwrap();
    let mut after = envelope(2, "after", 2_600, false);
    after.sender_address = Some("arnaque@exemple.fr".to_string());
    store.upsert_envelopes(inbox, &[after]).unwrap();
    assert_eq!(
        store.count(inbox).unwrap(),
        1,
        "the new one left, the old one stays"
    );
    assert_eq!(
        store
            .pending_actions(inbox)
            .unwrap()
            .iter()
            .map(|a| (a.uid, a.action.clone()))
            .collect::<Vec<_>>(),
        vec![(2, Action::MoveTo("Junk".to_string()))]
    );
}

/// D2 — the No rules TURN OFF with the mode: mode disabled, a
/// message from a screened-out sender with a rule arrives and
/// STAYS. And a screened-out sender WITHOUT a rule never triggers
/// anything (a plain No only hides).
#[test]
fn the_no_rules_turn_off_with_the_mode() {
    let (mut store, inbox) = store_with_mailbox();
    store.set_organized_mode(true, 1_000).unwrap();
    store
        .route_sender("promo@exemple.fr", "ecarte", Some("archive"), 2_000)
        .unwrap();
    store
        .route_sender("muet@exemple.fr", "ecarte", None, 2_000)
        .unwrap();
    store.set_organized_mode(false, 3_000).unwrap();
    let mut while_off = envelope(1, "while off", 3_500, false);
    while_off.sender_address = Some("promo@exemple.fr".to_string());
    store.upsert_envelopes(inbox, &[while_off]).unwrap();
    assert_eq!(store.count(inbox).unwrap(), 1, "mode off: the rule sleeps");
    assert!(store.pending_actions(inbox).unwrap().is_empty());

    store.set_organized_mode(true, 4_000).unwrap();
    let mut without_rule = envelope(2, "no rule", 4_500, false);
    without_rule.sender_address = Some("muet@exemple.fr".to_string());
    store.upsert_envelopes(inbox, &[without_rule]).unwrap();
    assert_eq!(
        store.count(inbox).unwrap(),
        2,
        "a plain No processes nothing"
    );
    assert!(store.pending_actions(inbox).unwrap().is_empty());
}

/// Re-delivery (review E3): a local removal pulls `max_uid` back
/// — if the replay fails, the next sync re-presents the same uid.
/// The rule removes it locally again but NEVER logs it twice: a
/// second identical action on a uid already gone from the server
/// would jam the whole replay queue behind a permanent failure.
#[test]
fn a_redelivery_never_logs_twice() {
    let (mut store, inbox) = store_with_mailbox();
    store.set_organized_mode(true, 1_000).unwrap();
    store
        .route_sender("promo@exemple.fr", "ecarte", Some("archive"), 2_000)
        .unwrap();
    let mut offer = envelope(1, "offer", 2_500, false);
    offer.sender_address = Some("promo@exemple.fr".to_string());
    store
        .upsert_envelopes(inbox, std::slice::from_ref(&offer))
        .unwrap();
    // The server re-presents the same uid (replay not yet run).
    store.upsert_envelopes(inbox, &[offer]).unwrap();
    assert_eq!(store.count(inbox).unwrap(), 0, "removed locally again");
    assert_eq!(
        store
            .pending_actions(inbox)
            .unwrap()
            .iter()
            .map(|a| (a.uid, a.action.clone()))
            .collect::<Vec<_>>(),
        vec![(1, Action::Archive)],
        "ONE action logged"
    );
}

/// "Their NEXT messages" (the gate's toasts): the rule only
/// touches mail AFTER the verdict — a backfill of old mail
/// (adding an account, sync disorder) never archives or discards
/// the history. A message WITHOUT a date is treated as arriving
/// today: the rule applies.
#[test]
fn the_rule_never_touches_mail_predating_the_verdict() {
    let (mut store, inbox) = store_with_mailbox();
    store.set_organized_mode(true, 1_000).unwrap();
    store
        .route_sender("promo@exemple.fr", "ecarte", Some("corbeille"), 2_000)
        .unwrap();
    let mut before = envelope(1, "before the verdict", 1_500, true);
    before.sender_address = Some("promo@exemple.fr".to_string());
    let mut without_date = envelope(2, "no date", 0, false);
    without_date.sender_address = Some("promo@exemple.fr".to_string());
    without_date.date = None;
    store
        .upsert_envelopes(inbox, &[before, without_date])
        .unwrap();
    assert_eq!(
        store.count(inbox).unwrap(),
        1,
        "what predates the verdict stays; the dateless one (today's arrival) is processed"
    );
    assert_eq!(
        store
            .pending_actions(inbox)
            .unwrap()
            .iter()
            .map(|a| (a.uid, a.action.clone()))
            .collect::<Vec<_>>(),
        vec![(2, Action::Delete)]
    );
}

/// PLAN-MODE-ORGANISE E4 — the organized Inbox's sections
/// (verdict S1, variant A2): ONE ordered flow "unread first, then
/// date" — "New for you" then "Already seen" are TWO bounds of
/// the same paginated source, the seam is the unread COUNT.
/// Classic mode, meanwhile, does not move a single rank.
#[test]
fn the_organized_inbox_serves_unread_first() {
    let (mut store, inbox) = store_with_mailbox();
    store.set_organized_mode(true, 1_000).unwrap();
    store
        .upsert_envelopes(
            inbox,
            &[
                envelope(1, "read old", 100, true),
                envelope(2, "unread recent", 200, false),
                envelope(3, "read recent", 300, true),
                envelope(4, "unread old", 150, false),
            ],
        )
        .unwrap();
    let organized = store.organized_inbox_scoped(None, false, 0, 10).unwrap();
    assert_eq!(
        organized.iter().map(|r| r.envelope.uid).collect::<Vec<_>>(),
        vec![2, 4, 3, 1],
        "unread first (by date), then read (by date)"
    );
    let account = test_account(&store);
    let bounded = store
        .organized_inbox_scoped(Some(account), false, 0, 10)
        .unwrap();
    assert_eq!(
        bounded.iter().map(|r| r.envelope.uid).collect::<Vec<_>>(),
        vec![2, 4, 3, 1],
        "same order bounded to an account"
    );
    assert_eq!(
        store.organized_inbox_count_scoped(None, true).unwrap(),
        2,
        "the seam: the unread COUNT says where the second section starts"
    );
    // Classic mode, UNTOUCHED: date only.
    let classic = store.unified_recent_scoped(None, false, 0, 10).unwrap();
    assert_eq!(
        classic.iter().map(|r| r.envelope.uid).collect::<Vec<_>>(),
        vec![3, 2, 4, 1]
    );
}

/// PLAN-MODE-ORGANISE E5 — Set aside (`pins` pattern: an ENVELOPE
/// key that survives thread rebuilding, state per THREAD). A
/// set-aside thread leaves ALL organized views — Inbox, its
/// routing view, surfaced pins — and lives in the pile; "Done"
/// returns it to where it came from. CLASSIC mode does not move a
/// single message.
#[test]
fn a_set_aside_thread_lives_in_the_pile_and_returns_when_done() {
    let (mut store, inbox) = store_with_mailbox();
    store.set_organized_mode(true, 1_000).unwrap();
    let mut letter = envelope(1, "the letter", 100, false);
    letter.sender_address = Some("lettre@exemple.fr".to_string());
    let ordinary = envelope(2, "hello", 200, false);
    store.upsert_envelopes(inbox, &[letter, ordinary]).unwrap();
    store
        .route_sender("lettre@exemple.fr", "kiosque", None, 300)
        .unwrap();

    assert!(store.toggle_set_aside(inbox, 2, 1_000).unwrap());
    assert!(store.set_aside_state(inbox, 2).unwrap());
    assert!(
        store
            .organized_inbox_scoped(None, false, 0, 10)
            .unwrap()
            .is_empty(),
        "the set-aside thread leaves the organized Inbox"
    );
    assert_eq!(
        store.organized_inbox_count_scoped(None, false).unwrap(),
        0,
        "the total follows (shared exclusion)"
    );
    assert_eq!(
        store.unified_count_scoped(None, false).unwrap(),
        2,
        "classic mode ALWAYS shows everything"
    );
    // The pile: the thread's mini-card, most recent first.
    assert!(store.toggle_set_aside(inbox, 1, 1_100).unwrap());
    let pile = store.set_aside_pile().unwrap();
    assert_eq!(
        pile.iter().map(|r| r.envelope.uid).collect::<Vec<_>>(),
        vec![2, 1],
        "the pile, most recent to oldest"
    );
    assert!(
        store
            .routing_unified_scoped("kiosque", None, false, 0, 10)
            .unwrap()
            .is_empty(),
        "set aside, the letter ALSO leaves its routing view"
    );

    // "Done": the thread returns TO WHERE IT CAME FROM.
    assert!(!store.toggle_set_aside(inbox, 2, 1_200).unwrap());
    assert_eq!(
        store.organized_inbox_count_scoped(None, false).unwrap(),
        1,
        "the ordinary one returns to the Inbox"
    );
    assert!(!store.toggle_set_aside(inbox, 1, 1_300).unwrap());
    assert_eq!(
        store
            .routing_unified_scoped("kiosque", None, false, 0, 10)
            .unwrap()
            .len(),
        1,
        "the letter returns to the Feed"
    );
    assert!(store.set_aside_pile().unwrap().is_empty());

    // The nav badge follows the pile (E5 capture finding): a
    // set-aside unread no longer counts in organized mode.
    assert!(store.toggle_set_aside(inbox, 2, 1_400).unwrap());
    let account = test_account(&store);
    let folders = store.canonical_folders(account).unwrap();
    let (organized, _) = store.nav_unread_counts(account, &folders, true).unwrap();
    assert_eq!(organized, 0, "the set-aside unread leaves the badge");
    let (classic, _) = store.nav_unread_counts(account, &folders, false).unwrap();
    assert_eq!(classic, 2, "classic mode does not move");
}

/// Setting aside follows the THREAD (pins pattern): set on a
/// message, it holds when a reply moves the head; a set-aside pin
/// leaves the organized Inbox's surfaced section (classic mode
/// keeps it).
#[test]
fn setting_aside_follows_the_thread_and_removes_the_surfaced_pin() {
    let (mut store, inbox) = store_with_mailbox();
    store.set_organized_mode(true, 1_000).unwrap();
    store
        .upsert_envelopes(inbox, &[envelope(1, "subject", 100, true)])
        .unwrap();
    assert!(store.toggle_pin(inbox, 1, 500).unwrap());
    assert!(store.toggle_set_aside(inbox, 1, 600).unwrap());
    let mut reply = envelope(2, "Re: subject", 700, true);
    reply.in_reply_to = Some("<m1@example.com>".to_string());
    store.upsert_envelopes(inbox, &[reply]).unwrap();

    assert!(
        store.set_aside_state(inbox, 2).unwrap(),
        "the state is read per thread, new head included"
    );
    assert!(
        store
            .pinned_unified_scoped(None, false, true)
            .unwrap()
            .is_empty(),
        "a set-aside thread's pin no longer surfaces in organized mode"
    );
    assert_eq!(
        store
            .pinned_unified_scoped(None, false, false)
            .unwrap()
            .len(),
        1,
        "classic mode keeps its pin"
    );
    // "Done" from the NEW head releases the whole thread.
    assert!(!store.toggle_set_aside(inbox, 2, 800).unwrap());
    assert!(!store.set_aside_state(inbox, 1).unwrap());
}

/// A43/A89: setting aside dies with its mail — a reset mailbox
/// (UIDVALIDITY) and a local removal purge it, a recycled UID
/// inherits nothing.
#[test]
fn setting_aside_dies_with_its_mail() {
    let (mut store, inbox) = store_with_mailbox();
    store
        .upsert_envelopes(
            inbox,
            &[envelope(1, "a", 100, true), envelope(2, "b", 200, true)],
        )
        .unwrap();
    assert!(store.toggle_set_aside(inbox, 1, 300).unwrap());
    store.remove_local(inbox, 1).unwrap();
    assert!(store.set_aside_pile().unwrap().is_empty());

    assert!(store.toggle_set_aside(inbox, 2, 400).unwrap());
    store.reset_mailbox(inbox, 2).unwrap();
    assert!(
        store.set_aside_pile().unwrap().is_empty(),
        "the fresh UIDVALIDITY leaves no phantom set-aside entry"
    );
}

/// The organized Inbox's plan guard (S2-bis lesson,
/// spikes/routage-plan): the page follows the mirrored PARTIAL
/// index (`idx_threads_date_organise`) — a stable offset by
/// construction, never a probe per skipped row, never an
/// envelopes scan.
#[test]
fn the_organized_inbox_follows_the_partial_index_never_a_scan() {
    let store = Store::open_in_memory().unwrap();
    let plan: Vec<String> = store
        .0
        .prepare(&format!(
            "EXPLAIN QUERY PLAN {}",
            unified_page_sql(false, false, true)
        ))
        .unwrap()
        .query_map(params![10, 0], |row| row.get::<_, String>(3))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert!(
        plan.iter().any(|l| l.contains("idx_threads_date_organise")),
        "the page does not follow the partial index: {plan:?}"
    );
    assert!(
        !plan
            .iter()
            .any(|l| l.starts_with("SCAN") && l.contains("envelopes")),
        "plan with an envelopes scan: {plan:?}"
    );
    // E4: the index CARRIES the sectioned sort INSIDE the
    // paginated skeleton — a materialized sort BEFORE the LIMIT
    // would be a sort of the whole mailbox (548 ms measured at
    // spike S1 without the expression index). The EXTERNAL
    // re-sort of the ≤200 retained rows (after "SCAN t") is
    // bounded and legitimate — the section expression is not
    // derived from the join.
    let join = plan
        .iter()
        .position(|l| l == "SCAN t")
        .expect("the plan lost its paginated co-routine");
    assert!(
        !plan[..join].iter().any(|l| l.contains("TEMP B-TREE")),
        "materialized sort INSIDE the paginated skeleton: {plan:?}"
    );
    // Review E4: the OTHER TWO organized paths carry the same
    // guard — the "Mailboxes" view (index prefixed by account)
    // and the Unread tab. Without it, a change of index key would
    // silently bring back S1's materialized sort (548 ms/page).
    for (name, sql, param_n) in [
        (
            "by account",
            unified_page_sql(true, false, true),
            params![10, 0, 1].to_vec(),
        ),
        (
            "unread",
            unified_page_sql(false, true, true),
            params![10, 0].to_vec(),
        ),
    ] {
        let plan: Vec<String> = store
            .0
            .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
            .unwrap()
            .query_map(rusqlite::params_from_iter(param_n), |row| {
                row.get::<_, String>(3)
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(
            plan.iter().any(|l| l.contains("idx_threads_date_organise")),
            "organized path \"{name}\" without the partial index: {plan:?}"
        );
        let join = plan
            .iter()
            .position(|l| l == "SCAN t")
            .expect("paginated co-routine missing");
        assert!(
            !plan[..join].iter().any(|l| l.contains("TEMP B-TREE")),
            "organized path \"{name}\": materialized sort in the skeleton: {plan:?}"
        );
    }
}

/// The Screener's history reads the list from most recently
/// decided to oldest — the eye is looking for the latest
/// decision.
#[test]
fn routings_list_from_the_most_recent() {
    let store = Store::open_in_memory().unwrap();
    store
        .route_sender("ancien@ex.fr", "registre", None, 100)
        .unwrap();
    store
        .route_sender("recent@ex.fr", "ecarte", Some("archive"), 200)
        .unwrap();
    let list = store.routings().unwrap();
    assert_eq!(
        list.iter().map(|r| r.address.as_str()).collect::<Vec<_>>(),
        vec!["recent@ex.fr", "ancien@ex.fr"]
    );
    assert_eq!(list[0].rule.as_deref(), Some("archive"));
}

// The pure decisions of `upsert_envelopes`, extracted at PLAN-AUDIT-V3 E2:
// each one is a rule the comments used to carry alone — now a named
// function the compiler holds.
#[test]
fn reindexing_is_skipped_only_when_all_five_indexed_fields_match() {
    let existing = (
        Some("subject".to_string()),
        Some("Sender".to_string()),
        Some("s@x.io".to_string()),
        Some("to@x.io".to_string()),
        None,
    );
    // A brand-new envelope always indexes.
    assert!(needs_reindex(
        None,
        Some("subject"),
        Some("Sender"),
        Some("s@x.io"),
        Some("to@x.io"),
        None
    ));
    // Identical five fields: the index is left alone.
    assert!(!needs_reindex(
        Some(&existing),
        Some("subject"),
        Some("Sender"),
        Some("s@x.io"),
        Some("to@x.io"),
        None
    ));
    // Any single drift re-indexes — the subject here.
    assert!(needs_reindex(
        Some(&existing),
        Some("edited"),
        Some("Sender"),
        Some("s@x.io"),
        Some("to@x.io"),
        None
    ));
    // A field appearing where none was stored re-indexes — the cc here.
    assert!(needs_reindex(
        Some(&existing),
        Some("subject"),
        Some("Sender"),
        Some("s@x.io"),
        Some("to@x.io"),
        Some("cc@x.io")
    ));
}

#[test]
fn a_no_rule_maps_to_its_action_and_spam_needs_a_resolved_junk_folder() {
    assert_eq!(no_rule_action("archive", None), Some(Action::Archive));
    // "corbeille": the server's trash, never a permanent deletion (D4).
    assert_eq!(no_rule_action("corbeille", None), Some(Action::Delete));
    assert_eq!(
        no_rule_action("spam", Some("Junk")),
        Some(Action::MoveTo("Junk".to_string()))
    );
    // Junk with no resolved folder: no action — the stated limit holds.
    assert_eq!(no_rule_action("spam", None), None);
    // An unknown rule value acts on nothing.
    assert_eq!(no_rule_action("autre", Some("Junk")), None);
}

#[test]
fn a_message_without_a_date_is_treated_as_arriving_today() {
    // No Date header: the rule applies (spam without a Date would
    // otherwise dodge the very gate).
    assert!(arrived_after_verdict(None, i64::MAX));
    // Dated after the verdict: "their next messages" — the rule applies.
    assert!(arrived_after_verdict(Some(100), 50));
    // Dated at or before the verdict: history, never touched.
    assert!(!arrived_after_verdict(Some(50), 50));
    assert!(!arrived_after_verdict(Some(10), 50));
}

use crate::fake_server::{FakeImap, Script, literal, uids_of};
use mail_core::MailServer;

const CAP: usize = 32 * 1024 * 1024;
const MESSAGE: &str = "From: a@example.test\r\nContent-Type: text/plain\r\n\r\nhello";

#[test]
fn a_shared_allowance_bounds_header_responses_and_counts_failed_reads() {
    let mut script = Script::simple();
    script.fetch = Box::new(|_| {
        vec![format!(
            "* 1 FETCH (UID 1 BODY[HEADER.FIELDS (REFERENCES IN-REPLY-TO)] {})",
            literal(&"x".repeat(16000))
        )]
    });
    let fixture = FakeImap::start(script);
    let mut server = fixture.connect();
    let before = server.received_bytes();
    server.set_fetch_limits(Some(mail_core::FetchLimits {
        bytes: 4096,
        timeout: std::time::Duration::from_secs(2),
    }));
    assert!(server.fetch_thread_headers("INBOX", &[1]).is_err());
    let received = server.received_bytes() - before;
    assert!(received > 0 && received <= 4096, "received {received}");
    server.set_fetch_limits(None);
    assert!(
        server.select("INBOX").is_err(),
        "clearing a scope must not revive a truncated session"
    );
}

#[test]
fn an_expired_shared_allowance_refuses_the_next_command() {
    let fixture = FakeImap::start(Script::simple());
    let mut server = fixture.connect();
    server.set_fetch_limits(Some(mail_core::FetchLimits {
        bytes: 4096,
        timeout: std::time::Duration::ZERO,
    }));
    assert!(server.select("INBOX").is_err());
}

#[test]
fn successive_commands_share_the_body_calls_raw_budget() {
    let mut script = Script::simple();
    script.fetch = Box::new(|command| {
        uids_of(command)
            .into_iter()
            .map(|uid| {
                let size = if uid == 1 { CAP - 1000 } else { 2000 };
                if command.contains("RFC822.SIZE") {
                    format!("* {uid} FETCH (UID {uid} RFC822.SIZE {size})")
                } else {
                    let raw = format!("{MESSAGE}{}", "x".repeat(size - MESSAGE.len()));
                    format!("* {uid} FETCH (UID {uid} BODY[] {})", literal(&raw))
                }
            })
            .collect()
    });
    let fixture = FakeImap::start(script);
    let mut server = fixture.connect();
    let fetched = server.fetch_bodies_html("INBOX", &[1, 2]).unwrap();
    assert_eq!(fetched.iter().map(|(uid, _)| *uid).collect::<Vec<_>>(), [1]);
    drop(fetched);
    assert_eq!(
        fixture
            .commands()
            .iter()
            .filter(|c| c.contains("BODY.PEEK[]"))
            .count(),
        1
    );
    assert_eq!(server.fetch_bodies_html("INBOX", &[2]).unwrap()[0].0, 2);
}

#[test]
fn oversized_advertised_messages_are_not_downloaded_on_any_raw_path() {
    for path in 0..3 {
        let mut script = Script::simple();
        script.fetch = Box::new(|command| {
            if command.contains("RFC822.SIZE") {
                vec![format!("* 1 FETCH (UID 1 RFC822.SIZE {})", CAP + 1)]
            } else {
                vec![format!("* 1 FETCH (UID 1 BODY[] {})", literal(MESSAGE))]
            }
        });
        let fixture = FakeImap::start(script);
        let mut server = fixture.connect();
        let refused = match path {
            0 => server.fetch_bodies_html("INBOX", &[1]).is_err(),
            1 => server.fetch_attachment("INBOX", 1, 0).is_err(),
            _ => server.fetch_draft(1).is_err(),
        };
        assert!(refused, "raw path {path} accepted an oversized message");
        assert!(!fixture.commands().iter().any(|c| c.contains("BODY.PEEK")));
    }
}

#[test]
fn missing_size_still_fetches_the_requested_message() {
    let mut script = Script::simple();
    script.fetch = Box::new(|command| {
        if command.contains("RFC822.SIZE") {
            Vec::new()
        } else {
            vec![format!("* 1 FETCH (UID 1 BODY[] {})", literal(MESSAGE))]
        }
    });
    let fixture = FakeImap::start(script);
    let mut server = fixture.connect();
    let bodies = server.fetch_bodies_html("INBOX", &[1]).unwrap();
    assert_eq!(bodies.len(), 1);
    assert_eq!(bodies[0].0, 1);
}

#[test]
fn unrelated_flag_updates_during_size_queries_do_not_block_body_loading() {
    let mut script = Script::simple();
    script.fetch = Box::new(|command| {
        if command.contains("RFC822.SIZE") {
            vec![
                "* 2 FETCH (UID 2 FLAGS (\\Seen))".into(),
                format!("* 1 FETCH (UID 1 RFC822.SIZE {})", MESSAGE.len()),
            ]
        } else {
            vec![format!("* 1 FETCH (UID 1 BODY[] {})", literal(MESSAGE))]
        }
    });
    let fixture = FakeImap::start(script);
    assert_eq!(
        fixture
            .connect()
            .fetch_bodies_html("INBOX", &[1])
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn lying_size_cannot_admit_one_byte_over_the_raw_limit() {
    let mut script = Script::simple();
    script.fetch = Box::new(|command| {
        if command.contains("RFC822.SIZE") {
            vec!["* 1 FETCH (UID 1 RFC822.SIZE 1)".into()]
        } else {
            let raw = format!("{MESSAGE}{}", "x".repeat(CAP + 1 - MESSAGE.len()));
            vec![format!("* 1 FETCH (UID 1 BODY[] {})", literal(&raw))]
        }
    });
    let fixture = FakeImap::start(script);
    let mut server = fixture.connect();
    assert!(server.fetch_attachment("INBOX", 1, 0).is_err());
    assert!(
        server.select("INBOX").is_ok(),
        "a complete size refusal must permit its namespace fence"
    );
}

#[test]
fn duplicate_or_unrequested_bodies_are_rejected() {
    for duplicate in [false, true] {
        let mut script = Script::simple();
        script.fetch = Box::new(move |command| {
            if command.contains("RFC822.SIZE") {
                return uids_of(command)
                    .into_iter()
                    .map(|uid| format!("* {uid} FETCH (UID {uid} RFC822.SIZE 100)"))
                    .collect();
            }
            let uid = if duplicate { 1 } else { 2 };
            vec![
                format!("* 1 FETCH (UID 1 BODY[] {})", literal(MESSAGE)),
                format!("* 2 FETCH (UID {uid} BODY[] {})", literal(MESSAGE)),
            ]
        });
        let fixture = FakeImap::start(script);
        assert!(fixture.connect().fetch_bodies_html("INBOX", &[1]).is_err());
    }
}

#[test]
fn the_exact_raw_limit_is_accepted_and_honest_refusal_keeps_the_session_usable() {
    let mut script = Script::simple();
    script.fetch = Box::new(|command| {
        let uid = uids_of(command)[0];
        if command.contains("RFC822.SIZE") {
            vec![format!(
                "* 1 FETCH (UID {uid} RFC822.SIZE {})",
                CAP + usize::from(uid == 1)
            )]
        } else {
            let raw = format!("{MESSAGE}{}", "x".repeat(CAP - MESSAGE.len()));
            vec![format!("* 1 FETCH (UID {uid} BODY[] {})", literal(&raw))]
        }
    });
    let fixture = FakeImap::start(script);
    let mut server = fixture.connect();
    assert!(matches!(
        server.raw_sizes(&[1]),
        Err(mail_core::Error::RemoteMessageTooLarge { uid: 1, .. })
    ));
    server.raw_sizes(&[2]).unwrap();
    let bodies = server.raw_fetch(&[2]).unwrap();
    assert_eq!(bodies.iter().next().unwrap().body().unwrap().len(), CAP);
}

#[test]
fn nested_attachments_keep_their_indices_on_all_raw_paths() {
    let raw = concat!(
        "From: a@example.test\r\nTo: b@example.test\r\n",
        "Content-Type: multipart/mixed; boundary=outer\r\n\r\n",
        "--outer\r\nContent-Type: multipart/alternative; boundary=inner\r\n\r\n",
        "--inner\r\nContent-Type: text/plain\r\n\r\nhello\r\n",
        "--inner\r\nContent-Type: text/html\r\n\r\n<p>hello</p>\r\n--inner--\r\n",
        "--outer\r\nContent-Type: application/octet-stream\r\n",
        "Content-Disposition: attachment; filename=first.bin\r\n",
        "Content-Transfer-Encoding: base64\r\n\r\nAAECA/8=\r\n",
        "--outer\r\nContent-Type: application/octet-stream\r\n",
        "Content-Disposition: attachment; filename=second.bin\r\n",
        "Content-Transfer-Encoding: base64\r\n\r\nBAUG\r\n--outer--\r\n"
    );
    let mut script = Script::simple();
    script.fetch = Box::new(move |command| {
        if command.contains("RFC822.SIZE") {
            vec![format!("* 1 FETCH (UID 1 RFC822.SIZE {})", raw.len())]
        } else {
            vec![format!("* 1 FETCH (UID 1 BODY[] {})", literal(raw))]
        }
    });
    let fixture = FakeImap::start(script);
    let mut server = fixture.connect();
    let body = server.fetch_bodies_html("INBOX", &[1]).unwrap().remove(0).1;
    assert_eq!(body.attachments.len(), 2);
    for (index, expected) in [vec![0, 1, 2, 3, 255], vec![4, 5, 6]].iter().enumerate() {
        assert_eq!(
            &server.fetch_attachment("INBOX", 1, index).unwrap().unwrap(),
            expected
        );
    }
    assert!(server.fetch_draft(1).unwrap().is_some());
    assert!(
        fixture
            .commands()
            .iter()
            .filter(|c| c.contains("BODY"))
            .all(|c| c.contains("BODY.PEEK[]"))
    );
}

use std::collections::VecDeque;
use std::io::{self, Cursor, Read, Write};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

enum Piece {
    Bytes(Cursor<Vec<u8>>),
    Repeat(usize),
}

// Synthesizes abusive wire data without allocating the advertised message.
struct AbusiveStream {
    pieces: VecDeque<Piece>,
    command: Vec<u8>,
    admitted: Arc<AtomicUsize>,
    long_line: bool,
}

impl Read for AbusiveStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        loop {
            let Some(piece) = self.pieces.front_mut() else {
                return Ok(0);
            };
            let count = match piece {
                Piece::Bytes(bytes) => bytes.read(buf)?,
                Piece::Repeat(left) => {
                    let count = (*left).min(buf.len());
                    buf[..count].fill(b'x');
                    *left -= count;
                    count
                }
            };
            if count > 0 {
                self.admitted.fetch_add(count, Ordering::Relaxed);
                return Ok(count);
            }
            self.pieces.pop_front();
        }
    }
}

impl Write for AbusiveStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.command.extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        if self.command.is_empty() {
            return Ok(());
        }
        let command = String::from_utf8(std::mem::take(&mut self.command)).unwrap();
        let tag = command.split_whitespace().next().unwrap();
        if command.contains("LOGIN") {
            self.pieces.push_back(Piece::Bytes(Cursor::new(
                format!("{tag} OK logged in\r\n").into_bytes(),
            )));
        } else {
            let prefix = if self.long_line {
                "* OK ".into()
            } else {
                format!("* 1 FETCH (UID 1 BODY[] {{{}}}\r\n", 2 * CAP)
            };
            self.pieces
                .push_back(Piece::Bytes(Cursor::new(prefix.into_bytes())));
            self.pieces.push_back(Piece::Repeat(2 * CAP));
            self.pieces.push_back(Piece::Bytes(Cursor::new(
                format!(")\r\n{tag} OK done\r\n").into_bytes(),
            )));
        }
        Ok(())
    }
}

impl crate::InnerSocket for AbusiveStream {
    fn set_timeout(&mut self, _: Duration) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn abusive_responses_stop_before_the_parser_can_read_past_its_allowance() {
    for (long_line, metadata) in [(false, false), (true, false), (true, true)] {
        let admitted = Arc::new(AtomicUsize::new(0));
        let stream = AbusiveStream {
            pieces: VecDeque::new(),
            command: Vec::new(),
            admitted: admitted.clone(),
            long_line,
        };
        let budget = crate::ReadBudget::default();
        let client = imap::Client::new(Box::new(crate::BoundedStream::with_budget(
            stream,
            Duration::from_secs(1),
            budget.clone(),
        )) as imap::Connection);
        let session = client
            .login("fixture", "synthetic")
            .map_err(|(err, _)| err)
            .unwrap();
        let mut server = crate::ImapServer::for_test(session, budget);
        admitted.store(0, Ordering::Relaxed);
        let failed = if metadata {
            server.raw_sizes(&[1]).is_err()
        } else {
            server.raw_fetch(&[1]).is_err()
        };
        assert!(failed);
        let allowance = crate::RESPONSE_ALLOWANCE + if metadata { 128 } else { CAP };
        assert_eq!(admitted.load(Ordering::Relaxed), allowance);
        assert!(
            server.session.noop().is_err(),
            "incomplete parser session was reused"
        );
        assert!(server.check_operation().is_err());
        assert_eq!(admitted.load(Ordering::Relaxed), allowance);
    }
}

#[test]
fn slow_drip_cannot_restart_the_absolute_read_deadline() {
    struct Drip;
    impl Read for Drip {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            std::thread::sleep(Duration::from_millis(5));
            buf[0] = b'x';
            Ok(1)
        }
    }
    impl Write for Drip {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    impl crate::InnerSocket for Drip {
        fn set_timeout(&mut self, _: Duration) -> io::Result<()> {
            Ok(())
        }
    }
    let budget = crate::ReadBudget::default();
    let mut stream =
        crate::BoundedStream::with_budget(Drip, Duration::from_secs(2), budget.clone());
    budget.begin(1000, Duration::from_millis(20)).unwrap();
    let started = std::time::Instant::now();
    let mut successful = 0;
    while stream.read(&mut [0; 1]).is_ok() {
        successful += 1;
        assert!(successful < 100, "each read restarted the command deadline");
    }
    assert!(started.elapsed() < Duration::from_secs(1));
    assert!(stream.write(b"next command").is_err());
    assert!(budget.begin(1000, Duration::from_secs(2)).is_err());
}

#[test]
fn several_small_messages_cannot_exceed_the_aggregate_raw_budget() {
    let mut script = Script::simple();
    script.fetch = Box::new(|command| {
        if command.contains("RFC822.SIZE") {
            return vec![
                "* 1 FETCH (UID 1 RFC822.SIZE 1)".into(),
                "* 2 FETCH (UID 2 RFC822.SIZE 1)".into(),
            ];
        }
        let raw = "x".repeat(CAP / 2 + 1);
        (1..=2)
            .map(|uid| format!("* {uid} FETCH (UID {uid} BODY[] {})", literal(&raw)))
            .collect()
    });
    let fixture = FakeImap::start(script);
    let mut server = fixture.connect();
    assert!(server.fetch_bodies_html("INBOX", &[1, 2]).is_err());
    assert!(server.check_operation().is_err());
}

#[test]
fn a_scope_smaller_than_the_batch_defers_the_rest_without_poisoning() {
    let mut script = Script::simple();
    script.fetch = Box::new(|command| {
        uids_of(command)
            .into_iter()
            .map(|uid| {
                if command.contains("RFC822.SIZE") {
                    format!("* {uid} FETCH (UID {uid} RFC822.SIZE 2000)")
                } else {
                    let raw = format!("{MESSAGE}{}", "x".repeat(2000 - MESSAGE.len()));
                    format!("* {uid} FETCH (UID {uid} BODY[] {})", literal(&raw))
                }
            })
            .collect()
    });
    let fixture = FakeImap::start(script);
    let mut server = fixture.connect();
    server.set_fetch_limits(Some(mail_core::FetchLimits {
        bytes: 3000,
        timeout: std::time::Duration::from_secs(5),
    }));
    let fetched = server.fetch_bodies_html("INBOX", &[1, 2]).unwrap();
    assert_eq!(fetched.iter().map(|(uid, _)| *uid).collect::<Vec<_>>(), [1]);
    drop(fetched);
    server.set_fetch_limits(None);
    assert!(
        server.select("INBOX").is_ok(),
        "planning against the scope must leave the session usable"
    );
    assert_eq!(server.fetch_bodies_html("INBOX", &[2]).unwrap()[0].0, 2);
}

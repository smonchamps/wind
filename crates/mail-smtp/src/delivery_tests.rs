//! Synthetic SMTP fault injection through the production adapter and outbox.
use super::*;
use mail_core::{DraftContent, OutboxState, Store, compose, flush_outbox};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug)]
enum Scenario {
    GreetingEof,
    Greeting421,
    Auth535,
    MailEof,
    RcptEof,
    SecondRcpt450,
    SecondRcpt550,
    DataEof,
    Final451,
    Final550,
    Final250QuitEof,
    Final354,
}

fn reply(stream: &mut TcpStream, value: &str) {
    stream
        .write_all(value.as_bytes())
        .expect("fixture response");
    stream.flush().expect("fixture flush");
}

fn serve(listener: TcpListener, scenario: Scenario) -> (usize, Vec<String>) {
    listener
        .set_nonblocking(true)
        .expect("nonblocking listener");
    let deadline = Instant::now() + Duration::from_secs(5);
    let (mut stream, _) = loop {
        match listener.accept() {
            Ok(connection) => break connection,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(Instant::now() < deadline, "fixture accept deadline");
                thread::sleep(Duration::from_millis(2));
            }
            Err(error) => panic!("fixture accept: {error}"),
        }
    };
    stream
        .set_nonblocking(false)
        .expect("blocking accepted stream");
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("read timeout");
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .expect("write timeout");
    if matches!(scenario, Scenario::GreetingEof) {
        return (0, vec![]);
    }
    if matches!(scenario, Scenario::Greeting421) {
        reply(&mut stream, "421 service unavailable\r\n");
        return (0, vec![]);
    }
    reply(&mut stream, "220 localhost fixture\r\n");
    let mut reader = BufReader::new(stream.try_clone().expect("reader clone"));
    let mut transcript = Vec::new();
    let mut rcpts = 0;
    let mut data_mode = false;
    let mut complete = 0;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).expect("fixture read") == 0 {
            break;
        }
        if data_mode {
            if line == ".\r\n" {
                data_mode = false;
                complete += 1;
                transcript.push("<complete DATA>".into());
                match scenario {
                    Scenario::DataEof => break,
                    Scenario::Final451 => reply(&mut stream, "451 temporarily rejected\r\n"),
                    Scenario::Final550 => reply(&mut stream, "550 permanently rejected\r\n"),
                    Scenario::Final354 => reply(&mut stream, "354 unexpected intermediate\r\n"),
                    _ => reply(&mut stream, "250 accepted\r\n"),
                }
            }
            continue;
        }
        transcript.push(line.trim_end().to_owned());
        if line.starts_with("EHLO ") {
            if matches!(scenario, Scenario::Auth535) {
                reply(&mut stream, "250-localhost\r\n250 AUTH PLAIN\r\n");
            } else {
                reply(&mut stream, "250 localhost\r\n");
            }
        } else if line.starts_with("AUTH ") {
            reply(&mut stream, "535 credentials refused\r\n");
        } else if line == "NOOP\r\n" {
            reply(&mut stream, "250 alive\r\n");
        } else if line.starts_with("MAIL FROM:") {
            if matches!(scenario, Scenario::MailEof) {
                break;
            }
            reply(&mut stream, "250 sender accepted\r\n");
        } else if line.starts_with("RCPT TO:") {
            rcpts += 1;
            if matches!(scenario, Scenario::RcptEof) {
                break;
            }
            match (rcpts, scenario) {
                (2, Scenario::SecondRcpt450) => reply(&mut stream, "450 recipient refused\r\n"),
                (2, Scenario::SecondRcpt550) => reply(&mut stream, "550 recipient refused\r\n"),
                _ => reply(&mut stream, "250 recipient accepted\r\n"),
            }
        } else if line == "DATA\r\n" {
            data_mode = true;
            reply(&mut stream, "354 send data\r\n");
        } else if line == "QUIT\r\n" {
            if !matches!(scenario, Scenario::Final250QuitEof) {
                reply(&mut stream, "221 bye\r\n");
            }
            break;
        } else {
            panic!("unexpected fixture command {line:?}");
        }
    }
    (complete, transcript)
}

fn mailer(scenario: Scenario) -> (SmtpMailer, thread::JoinHandle<(usize, Vec<String>)>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = thread::spawn(move || serve(listener, scenario));
    let builder = SmtpTransport::builder_dangerous("127.0.0.1")
        .port(port)
        .timeout(Some(Duration::from_secs(2)));
    let transport = if matches!(scenario, Scenario::Auth535) {
        builder
            .authentication(vec![Mechanism::Plain])
            .credentials(Credentials::new("fixture".into(), "fixture".into()))
            .build()
    } else {
        builder.build()
    };
    (SmtpMailer { transport }, server)
}

fn queue(store: &Store) -> (i64, String) {
    let account = store
        .adopt_or_create_account("from@example.invalid", "gmail")
        .unwrap();
    let draft = compose(
        "from@example.invalid",
        "one@example.invalid, two@example.invalid",
        "",
        "",
        "Synthetic",
        "payload",
        None,
    )
    .unwrap();
    let draft_id = store
        .save_draft(
            account,
            None,
            None,
            DraftContent {
                to_raw: "one@example.invalid, two@example.invalid",
                cc_raw: "",
                bcc_raw: "",
                subject: "Synthetic",
                body: "payload",
                body_html: None,
                reply_to_uid: None,
                reply_to_mailbox: None,
                important: false,
            },
        )
        .unwrap()
        .id;
    store
        .add_draft_attachment(
            draft_id,
            "fixture.bin",
            "application/octet-stream",
            &[1, 2, 3],
        )
        .unwrap();
    store
        .enqueue_outbox_from_draft(account, &draft, draft_id)
        .unwrap();
    (account, draft.message_id)
}

fn check(scenario: Scenario, expected: OutboxState, payloads: usize) {
    let mut store = Store::open_in_memory().unwrap();
    let (account, message_id) = queue(&store);
    let (mut smtp, server) = mailer(scenario);
    let report = flush_outbox(&mut smtp, &mut store, account).unwrap();
    let (count, transcript) = server.join().unwrap();
    assert_eq!(count, payloads, "{scenario:?}: {transcript:?}");
    let rows = store.outbox_in_state(expected).unwrap();
    assert_eq!(rows.len(), 1, "{scenario:?}: {report:?}");
    assert_eq!(rows[0].message_id, message_id);
    if expected == OutboxState::Interrupted {
        assert_eq!(report.quarantined, 1);
        assert_eq!(rows[0].attempts, 1);
        assert!(
            rows[0]
                .last_error
                .as_ref()
                .is_some_and(|reason| !reason.is_empty())
        );
    }
    assert_eq!(
        rows[0].attachments[0].bytes,
        if expected == OutboxState::Sent {
            None
        } else {
            Some(vec![1, 2, 3])
        }
    );
    if matches!(scenario, Scenario::SecondRcpt450 | Scenario::SecondRcpt550) {
        assert!(!transcript.iter().any(|line| line == "DATA"));
    }
}

macro_rules! scenario_test {
    ($name:ident, $scenario:ident, $state:ident, $payloads:expr) => {
        #[test]
        fn $name() {
            check(Scenario::$scenario, OutboxState::$state, $payloads);
        }
    };
}
scenario_test!(
    greeting_eof_is_conservatively_interrupted,
    GreetingEof,
    Interrupted,
    0
);
scenario_test!(
    mail_eof_is_conservatively_interrupted,
    MailEof,
    Interrupted,
    0
);
scenario_test!(
    rcpt_eof_is_conservatively_interrupted,
    RcptEof,
    Interrupted,
    0
);
scenario_test!(
    second_recipient_temporary_refusal_prevents_data,
    SecondRcpt450,
    Queued,
    0
);
scenario_test!(
    second_recipient_permanent_refusal_prevents_data,
    SecondRcpt550,
    Rejected,
    0
);
scenario_test!(
    lost_final_ack_preserves_uncertainty,
    DataEof,
    Interrupted,
    1
);
scenario_test!(temporary_data_refusal_is_retryable, Final451, Queued, 1);
scenario_test!(permanent_data_refusal_is_rejected, Final550, Rejected, 1);
scenario_test!(accepted_data_survives_quit_eof, Final250QuitEof, Sent, 1);
scenario_test!(
    intermediate_final_reply_is_not_acceptance,
    Final354,
    Interrupted,
    1
);

#[test]
fn lost_ack_survives_reopening_without_automatic_resubmission() {
    let folder = std::env::temp_dir().join(format!(
        "wind-smtp-recovery-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&folder).unwrap();
    let path = folder.join("wind.db");
    let mut store = Store::open(&path).unwrap();
    let (account, message_id) = queue(&store);
    let (mut smtp, server) = mailer(Scenario::DataEof);
    flush_outbox(&mut smtp, &mut store, account).unwrap();
    assert_eq!(server.join().unwrap().0, 1);
    drop(store);
    struct NoSubmission;
    impl MailTransport for NoSubmission {
        fn send(&mut self, _: &OutboxMessage) -> Result<(), SendError> {
            panic!("uncertain delivery must never be submitted automatically");
        }
    }
    let mut store = Store::open(&path).unwrap();
    for _ in 0..3 {
        let report = flush_outbox(&mut NoSubmission, &mut store, account).unwrap();
        assert_eq!(report.sent, 0);
    }
    let rows = store.outbox_in_state(OutboxState::Interrupted).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].message_id, message_id);
    assert_eq!(rows[0].attachments[0].bytes, Some(vec![1, 2, 3]));
    store.requeue_outbox(rows[0].id).unwrap();
    let (mut smtp, server) = mailer(Scenario::Final250QuitEof);
    assert_eq!(
        flush_outbox(&mut smtp, &mut store, account).unwrap().sent,
        1
    );
    assert_eq!(
        server.join().unwrap().0,
        1,
        "only the explicit requeue permits a second submission"
    );
    drop(store);
    std::fs::remove_dir_all(&folder).unwrap();
}

scenario_test!(
    authentication_refusal_during_send_is_retryable,
    Auth535,
    Queued,
    0
);
scenario_test!(
    temporary_greeting_refusal_is_retryable,
    Greeting421,
    Queued,
    0
);

#[test]
fn setup_only_refreshes_credentials_on_an_authentication_refusal() {
    for scenario in [
        Scenario::GreetingEof,
        Scenario::Greeting421,
        Scenario::Auth535,
    ] {
        let (smtp, server) = mailer(scenario);
        let result = SmtpMailer::test_transport(smtp.transport);
        assert_eq!(server.join().unwrap().0, 0);
        match scenario {
            Scenario::Auth535 => assert!(matches!(result, Err(ConnectError::Authentication(_)))),
            _ => assert!(matches!(result, Err(ConnectError::Connection(_)))),
        }
    }
}

#[test]
fn missing_smtputf8_is_a_safe_retry_before_mail() {
    let store = Store::open_in_memory().unwrap();
    let (account, _) = queue(&store);
    let mut message = store.outbox_to_send(account).unwrap().remove(0);
    message.from = "\u{e9}@example.invalid".into();
    let (mut smtp, server) = mailer(Scenario::Final250QuitEof);
    assert!(matches!(smtp.send(&message), Err(SendError::Transient(_))));
    let (payloads, transcript) = server.join().unwrap();
    assert_eq!(payloads, 0);
    assert!(!transcript.iter().any(|line| line.starts_with("MAIL FROM:")));
}

#[test]
fn required_starttls_without_server_support_never_sends_cleartext() {
    use lettre::transport::smtp::client::{Tls, TlsParameters};
    let store = Store::open_in_memory().unwrap();
    let (account, _) = queue(&store);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = thread::spawn(move || serve(listener, Scenario::Final250QuitEof));
    let transport = SmtpTransport::builder_dangerous("127.0.0.1")
        .port(port)
        .timeout(Some(Duration::from_secs(2)))
        .tls(Tls::Required(
            TlsParameters::new("localhost".into()).unwrap(),
        ))
        .build();
    let mut smtp = SmtpMailer { transport };
    assert!(matches!(
        smtp.send(&store.outbox_to_send(account).unwrap()[0]),
        Err(SendError::Transient(_))
    ));
    let (payloads, transcript) = server.join().unwrap();
    assert_eq!(payloads, 0);
    assert!(!transcript.iter().any(|line| line.starts_with("MAIL FROM:")));
}

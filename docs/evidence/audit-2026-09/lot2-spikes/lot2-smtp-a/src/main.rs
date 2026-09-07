use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

use lettre::address::Envelope;
use lettre::transport::smtp::{Error, response::Response};
use lettre::{SmtpTransport, Transport};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Class {
    Sent,
    Transient,
    Permanent,
    Unknown,
}

fn baseline(result: &Result<Response, Error>) -> Class {
    match result {
        Ok(_) => Class::Sent,
        Err(error) if matches!(error.status().map(u16::from), Some(530 | 534 | 535 | 538)) => {
            Class::Transient
        }
        Err(error) if error.is_permanent() => Class::Permanent,
        Err(_) => Class::Transient,
    }
}

// Candidate A: keep SmtpTransport; only use public evidence pinned to lettre 0.11.22.
fn candidate(result: &Result<Response, Error>) -> Class {
    match result {
        Ok(response) if response.has_code(250) => Class::Sent,
        Ok(_) => Class::Unknown,
        Err(error) if matches!(error.status().map(u16::from), Some(530 | 534 | 535 | 538)) => {
            Class::Transient
        }
        Err(error) if error.is_permanent() => Class::Permanent,
        Err(error)
            if error.is_transient()
                || error.is_client()
                || error.is_tls()
                || error.is_transport_shutdown() =>
        {
            Class::Transient
        }
        Err(_) => Class::Unknown,
    }
}

#[derive(Clone, Copy, Debug)]
enum Scenario {
    GreetingEof,
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
            reply(&mut stream, "250 localhost\r\n");
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

fn attempt(scenario: Scenario) -> (Class, Class, usize, String, Vec<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("loopback bind");
    let port = listener.local_addr().expect("loopback address").port();
    let fixture = thread::spawn(move || serve(listener, scenario));
    let transport = SmtpTransport::builder_dangerous("127.0.0.1")
        .port(port)
        .timeout(Some(Duration::from_secs(2)))
        .build();
    let envelope = Envelope::new(
        Some("from@example.invalid".parse().unwrap()),
        vec![
            "one@example.invalid".parse().unwrap(),
            "two@example.invalid".parse().unwrap(),
        ],
    )
    .unwrap();
    let result = transport.send_raw(&envelope,
        b"From: from@example.invalid\r\nTo: one@example.invalid\r\nMessage-ID: <spike-a@example.invalid>\r\nSubject: synthetic fixture\r\n\r\nfixture payload\r\n");
    let evidence = match &result {
        Ok(response) => format!("ok={}", u16::from(response.code())),
        Err(error) => format!(
            "status={:?};response={};client={};tls={};timeout={};error={error}",
            error.status().map(u16::from),
            error.is_response(),
            error.is_client(),
            error.is_tls(),
            error.is_timeout()
        ),
    };
    let (complete, transcript) = fixture.join().expect("joined fixture");
    (
        baseline(&result),
        candidate(&result),
        complete,
        evidence,
        transcript,
    )
}

fn main() {
    let start = Instant::now();
    println!(
        "option=A; lettre=0.11.22; transport=SmtpTransport; pool=false; tls=false(loopback-only); repetitions=3"
    );
    let scenarios = [
        Scenario::GreetingEof,
        Scenario::MailEof,
        Scenario::RcptEof,
        Scenario::SecondRcpt450,
        Scenario::SecondRcpt550,
        Scenario::DataEof,
        Scenario::Final451,
        Scenario::Final550,
        Scenario::Final250QuitEof,
        Scenario::Final354,
    ];
    for scenario in scenarios {
        let mut previous = None;
        for repetition in 1..=3 {
            let (old, new, complete, evidence, transcript) = attempt(scenario);
            let signature = (old, new, complete);
            if let Some(value) = previous {
                assert_eq!(value, signature, "stable repetitions");
            }
            previous = Some(signature);
            if matches!(scenario, Scenario::SecondRcpt450 | Scenario::SecondRcpt550) {
                assert!(!transcript.iter().any(|command| command == "DATA"));
                assert_eq!(complete, 0);
            }
            println!(
                "{scenario:?};rep={repetition};baseline={old:?};candidate={new:?};complete_data={complete};{evidence}"
            );
            if repetition == 1 {
                println!("transcript={transcript:?}");
            }
        }
    }
    for use_candidate in [false, true] {
        let mut attempts = 0;
        let mut payloads = 0;
        for _ in 0..2 {
            let (old, new, complete, _, _) = attempt(Scenario::DataEof);
            attempts += 1;
            payloads += complete;
            let class = if use_candidate { new } else { old };
            if class != Class::Transient {
                break;
            }
        }
        println!(
            "replay;candidate={use_candidate};attempts={attempts};complete_data={payloads};duplicate_payloads={}",
            payloads.saturating_sub(1)
        );
        assert_eq!(payloads, if use_candidate { 1 } else { 2 });
    }
    println!("elapsed_ms={}", start.elapsed().as_millis());
}

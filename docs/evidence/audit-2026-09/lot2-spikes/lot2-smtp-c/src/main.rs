use lettre::transport::smtp::{client::SmtpConnection, commands::{Data, Mail, Rcpt}, extension::ClientId, Error};
use std::{io::{BufRead, BufReader, Write}, net::{SocketAddr, TcpListener, TcpStream}, thread, time::{Duration, Instant}};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome { Retry, Refused, Unknown, Sent }

fn failure(error: Error, payload_started: bool) -> Outcome {
    if error.is_transient() { Outcome::Retry }
    else if error.is_permanent() { Outcome::Refused }
    else if payload_started { Outcome::Unknown }
    else { Outcome::Retry }
}

// Measured transaction: no TLS/authentication or capability negotiation here.
fn transaction(address: SocketAddr) -> Outcome {
    let mut connection = match SmtpConnection::connect(address, Some(Duration::from_millis(500)), &ClientId::default(), None, None) {
        Ok(connection) => connection,
        Err(error) => return failure(error, false),
    };
    match connection.command(Mail::new(Some("sender@example.test".parse().unwrap()), vec![])) {
        Ok(response) if response.has_code(250) => (),
        Ok(_) => return Outcome::Retry,
        Err(error) => return failure(error, false),
    }
    for recipient in ["first@example.test", "second@example.test"] {
        match connection.command(Rcpt::new(recipient.parse().unwrap(), vec![])) {
            Ok(response) if response.has_code(250) || response.has_code(251) || response.has_code(252) => (),
            Ok(_) => return Outcome::Retry,
            Err(error) => return failure(error, false),
        }
    }
    match connection.command(Data) {
        Ok(response) if response.has_code(354) => (),
        Ok(_) => return Outcome::Retry,
        Err(error) => return failure(error, false),
    }
    match connection.message(b"From: sender@example.test\r\nTo: first@example.test, second@example.test\r\nSubject: synthetic spike\r\n\r\nNo private data.\r\n") {
        Ok(response) if response.has_code(250) => {
            let _ = connection.quit();
            Outcome::Sent
        },
        Ok(_) => Outcome::Unknown,
        Err(error) => failure(error, true),
    }
}

#[derive(Debug, Clone, Copy)]
enum Scenario { GreetingEof, MailEof, RcptEof, SecondRcpt450, SecondRcpt550, FinalEof, Final451, Final550, Final250QuitEof, Final354 }

fn reply(stream: &mut BufReader<TcpStream>, text: &str) { stream.get_mut().write_all(text.as_bytes()).unwrap(); }
fn read(stream: &mut BufReader<TcpStream>) -> String {
    let mut line = String::new();
    stream.read_line(&mut line).unwrap();
    line
}
fn expect(stream: &mut BufReader<TcpStream>, prefix: &str) { assert!(read(stream).starts_with(prefix)); }
fn fixture(listener: TcpListener, scenario: Scenario) -> usize {
    let (stream, _) = listener.accept().unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    stream.set_write_timeout(Some(Duration::from_secs(2))).unwrap();
    let mut stream = BufReader::new(stream);
    if matches!(scenario, Scenario::GreetingEof) { return 0; }
    reply(&mut stream, "220 fixture ready\r\n");
    expect(&mut stream, "EHLO ");
    reply(&mut stream, "250 fixture\r\n");
    expect(&mut stream, "MAIL FROM:");
    if matches!(scenario, Scenario::MailEof) { return 0; }
    reply(&mut stream, "250 sender accepted\r\n");
    expect(&mut stream, "RCPT TO:");
    if matches!(scenario, Scenario::RcptEof) { return 0; }
    reply(&mut stream, "250 first accepted\r\n");
    expect(&mut stream, "RCPT TO:");
    if matches!(scenario, Scenario::SecondRcpt450 | Scenario::SecondRcpt550) {
        reply(&mut stream, if matches!(scenario, Scenario::SecondRcpt450) { "450 second rejected\r\n" } else { "550 second rejected\r\n" });
        assert!(read(&mut stream).is_empty(), "client must close before sending any payload");
        return 0;
    }
    reply(&mut stream, "250 second accepted\r\n");
    expect(&mut stream, "DATA");
    reply(&mut stream, "354 send data\r\n");
    let mut payload = Vec::new();
    loop {
        let line = read(&mut stream);
        assert!(!line.is_empty(), "payload incomplete");
        if line == ".\r\n" { break; }
        payload.push(line);
    }
    assert!(payload.iter().any(|line| line == "No private data.\r\n"));
    match scenario {
        Scenario::FinalEof => (),
        Scenario::Final451 => reply(&mut stream, "451 temporary rejection\r\n"),
        Scenario::Final550 => reply(&mut stream, "550 permanent rejection\r\n"),
        Scenario::Final354 => reply(&mut stream, "354 invalid final response\r\n"),
        Scenario::Final250QuitEof => {
            reply(&mut stream, "250 queued\r\n");
            expect(&mut stream, "QUIT");
        },
        _ => unreachable!(),
    }
    1
}

fn main() {
    let cases = [
        (Scenario::GreetingEof, Outcome::Retry, 0), (Scenario::MailEof, Outcome::Retry, 0),
        (Scenario::RcptEof, Outcome::Retry, 0), (Scenario::SecondRcpt450, Outcome::Retry, 0),
        (Scenario::SecondRcpt550, Outcome::Refused, 0), (Scenario::FinalEof, Outcome::Unknown, 1),
        (Scenario::Final451, Outcome::Retry, 1), (Scenario::Final550, Outcome::Refused, 1),
        (Scenario::Final250QuitEof, Outcome::Sent, 1), (Scenario::Final354, Outcome::Unknown, 1),
    ];
    println!("scenario,repeat,outcome,complete_payloads,elapsed_us");
    for (scenario, expected, expected_payloads) in cases {
        for repeat in 1..=3 {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let server = thread::spawn(move || fixture(listener, scenario));
            let begin = Instant::now();
            let outcome = transaction(address);
            let payloads = server.join().unwrap();
            let elapsed = begin.elapsed().as_micros();
            println!("{scenario:?},{repeat},{outcome:?},{payloads},{elapsed}");
            assert_eq!(outcome, expected);
            assert_eq!(payloads, expected_payloads);
        }
    }
}

//! Synthetic TLS regression fixture; its CA is trusted only by this test client.
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::Duration;

use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::{
    ClientConfig, ClientConnection, RootCertStore, ServerConfig, ServerConnection, StreamOwned,
};

use crate::{BoundedStream, cancel::CancelableTcp};

#[derive(Debug)]
struct FixtureTime;
impl rustls::time_provider::TimeProvider for FixtureTime {
    fn current_time(&self) -> Option<UnixTime> {
        // A date within the synthetic certificate's validity, independent of CI's clock.
        Some(UnixTime::since_unix_epoch(Duration::from_secs(1788793168)))
    }
}

#[derive(Clone, Copy)]
enum Phase {
    Quiet,
    Done,
    Partial,
}

fn trial(phase: Phase, cancel: bool) {
    let mut roots = RootCertStore::empty();
    roots
        .add(CertificateDer::from(
            include_bytes!("fixtures/idle-root.der").to_vec(),
        ))
        .unwrap();
    let mut client = ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    client.time_provider = Arc::new(FixtureTime);
    let server = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(
            vec![CertificateDer::from(
                include_bytes!("fixtures/idle-leaf.der").to_vec(),
            )],
            PrivatePkcs8KeyDer::from(include_bytes!("fixtures/idle-key.der").to_vec()).into(),
        )
        .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let alive = Arc::new(AtomicBool::new(true));
    let (ready_tx, ready_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let peer = std::thread::spawn(move || {
        let (tcp, _) = listener.accept().unwrap();
        tcp.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        tcp.set_write_timeout(Some(Duration::from_secs(3))).unwrap();
        let mut tls = StreamOwned::new(ServerConnection::new(Arc::new(server)).unwrap(), tcp);
        tls.write_all(b"* OK synthetic\r\n").unwrap();
        tls.flush().unwrap();
        let mut reader = BufReader::new(tls);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let tag = line.split_whitespace().next().unwrap();
        reader
            .get_mut()
            .write_all(format!("{tag} OK login\r\n").as_bytes())
            .unwrap();
        reader.get_mut().flush().unwrap();
        line.clear();
        reader.read_line(&mut line).unwrap();
        assert!(line.ends_with(" IDLE\r\n"));
        let tag = line.split_whitespace().next().unwrap().to_string();
        reader.get_mut().write_all(b"+ idling\r\n").unwrap();
        reader.get_mut().flush().unwrap();
        let mut tail = None;
        match phase {
            Phase::Quiet => {}
            Phase::Done => {
                reader.get_mut().write_all(b"* 1 EXISTS\r\n").unwrap();
                reader.get_mut().flush().unwrap();
                line.clear();
                reader.read_line(&mut line).unwrap();
                assert_eq!(line, "DONE\r\n");
            }
            Phase::Partial => {
                let tls = reader.get_mut();
                tls.conn.writer().write_all(b"* 1 EXISTS\r\n").unwrap();
                let mut record = Vec::new();
                while tls.conn.wants_write() {
                    tls.conn.write_tls(&mut record).unwrap();
                }
                tls.sock.write_all(&record[..7]).unwrap();
                tail = Some(record[7..].to_vec());
            }
        }
        ready_tx.send(()).unwrap();
        let _ = release_rx.recv_timeout(Duration::from_secs(3));
        if cancel {
            return;
        }
        if let Some(tail) = tail {
            reader.get_mut().sock.write_all(&tail).unwrap();
        } else {
            reader.get_mut().write_all(b"* 1 EXISTS\r\n").unwrap();
            reader.get_mut().flush().unwrap();
        }
        line.clear();
        reader.read_line(&mut line).unwrap();
        assert_eq!(line, "DONE\r\n");
        reader
            .get_mut()
            .write_all(format!("{tag} OK done\r\n").as_bytes())
            .unwrap();
        reader.get_mut().flush().unwrap();
    });
    let tcp = TcpStream::connect(addr).unwrap();
    tcp.set_write_timeout(Some(Duration::from_secs(3))).unwrap();
    let tcp = CancelableTcp::new(tcp, Duration::from_secs(3), Some(alive.clone())).unwrap();
    let tls = StreamOwned::new(
        ClientConnection::new(Arc::new(client), ServerName::try_from("localhost").unwrap())
            .unwrap(),
        tcp,
    );
    let transport = Box::new(BoundedStream::new(tls, Duration::from_secs(3))) as imap::Connection;
    let (done_tx, done_rx) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let mut client = imap::Client::new(transport);
        client.read_greeting().unwrap();
        let mut session = client.login("fixture", "fixture").map_err(|e| e.0).unwrap();
        let result = {
            let mut idle = session.idle();
            idle.timeout(Duration::from_secs(180)).keepalive(false);
            idle.wait_while(|r| !matches!(r, imap::types::UnsolicitedResponse::Exists(_)))
        };
        done_tx.send(result).unwrap();
    });
    ready_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    assert!(
        done_rx.recv_timeout(Duration::from_millis(350)).is_err(),
        "local slices must not end IDLE"
    );
    if cancel {
        alive.store(false, Ordering::Release);
    } else {
        release_tx.send(()).unwrap();
    }
    let result = done_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    if cancel && !matches!(phase, Phase::Done) {
        assert!(result.is_err());
    }
    if !cancel {
        assert!(matches!(
            result,
            Ok(imap::extensions::idle::WaitOutcome::MailboxChanged)
        ));
    }
    let _ = release_tx.send(());
    worker.join().unwrap();
    peer.join().unwrap();
}

#[test]
fn cancellation_closes_tls_idle_including_partial_records_and_done() {
    for phase in [Phase::Quiet, Phase::Partial, Phase::Done] {
        trial(phase, true);
    }
}

#[test]
fn silent_and_fragmented_tls_watches_survive_local_timeouts() {
    for phase in [Phase::Quiet, Phase::Partial] {
        trial(phase, false);
    }
}

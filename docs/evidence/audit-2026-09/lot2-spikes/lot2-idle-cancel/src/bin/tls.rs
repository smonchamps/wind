use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer, ServerName};
use rustls::{
    ClientConfig, ClientConnection, RootCertStore, ServerConfig, ServerConnection, StreamOwned,
};
use std::{
    io::{self, BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
const SLICE: Duration = Duration::from_millis(100);
#[derive(Default)]
struct Counts {
    reads: AtomicU64,
    timeouts: AtomicU64,
    bytes: AtomicU64,
    tls_calls: AtomicU64,
    done: AtomicU64,
}
struct Control {
    alive: AtomicBool,
    logical_ms: AtomicU64,
    counts: Counts,
}
impl Control {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            alive: AtomicBool::new(true),
            logical_ms: AtomicU64::new(120_000),
            counts: Counts::default(),
        })
    }
}
fn check(c: &Control) -> io::Result<()> {
    if c.alive.load(Ordering::Relaxed) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::ConnectionAborted,
            "terminal watch cancellation",
        ))
    }
}
fn retry(c: &Control, mut f: impl FnMut() -> io::Result<usize>) -> io::Result<usize> {
    let deadline = Instant::now() + Duration::from_millis(c.logical_ms.load(Ordering::Relaxed));
    loop {
        check(c)?;
        match f() {
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                ) && Instant::now() < deadline => {}
            result => return result,
        }
    }
}
struct CountSocket {
    tcp: TcpStream,
    control: Arc<Control>,
}
impl Read for CountSocket {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        self.control.counts.reads.fetch_add(1, Ordering::Relaxed);
        let r = self.tcp.read(b);
        match &r {
            Ok(n) => {
                self.control
                    .counts
                    .bytes
                    .fetch_add(*n as u64, Ordering::Relaxed);
            }
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                ) =>
            {
                self.control.counts.timeouts.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
        r
    }
}
impl Write for CountSocket {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        check(&self.control)?;
        self.tcp.write(b)
    }
    fn flush(&mut self) -> io::Result<()> {
        check(&self.control)?;
        self.tcp.flush()
    }
}
struct BelowSocket(CountSocket);
impl Read for BelowSocket {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        let c = self.0.control.clone();
        retry(&c, || self.0.read(b))
    }
}
impl Write for BelowSocket {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.0.write(b)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}
struct Above {
    tls: StreamOwned<ClientConnection, CountSocket>,
    control: Arc<Control>,
}
impl Read for Above {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        let c = self.control.clone();
        retry(&c, || {
            c.counts.tls_calls.fetch_add(1, Ordering::Relaxed);
            self.tls.read(b)
        })
    }
}
impl Write for Above {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        check(&self.control)?;
        self.tls.write(b)
    }
    fn flush(&mut self) -> io::Result<()> {
        check(&self.control)?;
        self.tls.flush()
    }
}
impl imap::extensions::idle::SetReadTimeout for Above {
    fn set_read_timeout(&mut self, t: Option<Duration>) -> imap::error::Result<()> {
        self.control.logical_ms.store(
            t.unwrap_or(Duration::from_secs(120)).as_millis() as u64,
            Ordering::Relaxed,
        );
        self.tls
            .sock
            .tcp
            .set_read_timeout(Some(SLICE))
            .map_err(imap::Error::Io)
    }
}
struct Below {
    tls: StreamOwned<ClientConnection, BelowSocket>,
    control: Arc<Control>,
}
impl Read for Below {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        self.control
            .counts
            .tls_calls
            .fetch_add(1, Ordering::Relaxed);
        check(&self.control)?;
        self.tls.read(b)
    }
}
impl Write for Below {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        check(&self.control)?;
        self.tls.write(b)
    }
    fn flush(&mut self) -> io::Result<()> {
        check(&self.control)?;
        self.tls.flush()
    }
}
impl imap::extensions::idle::SetReadTimeout for Below {
    fn set_read_timeout(&mut self, t: Option<Duration>) -> imap::error::Result<()> {
        self.control.logical_ms.store(
            t.unwrap_or(Duration::from_secs(120)).as_millis() as u64,
            Ordering::Relaxed,
        );
        self.tls
            .sock
            .0
            .tcp
            .set_read_timeout(Some(SLICE))
            .map_err(imap::Error::Io)
    }
}
fn configs() -> (Arc<ClientConfig>, Arc<ServerConfig>) {
    let mut roots = RootCertStore::empty();
    roots
        .add(CertificateDer::from(
            include_bytes!("../../certs/root.der").to_vec(),
        ))
        .unwrap();
    let client = ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let cert = CertificateDer::from(include_bytes!("../../certs/leaf.der").to_vec());
    let key = PrivatePkcs8KeyDer::from(include_bytes!("../../certs/leaf-key.der").to_vec());
    let server = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert], key.into())
        .unwrap();
    (Arc::new(client), Arc::new(server))
}
fn trial(placement: &str, phase: &str, run: u32, cc: Arc<ClientConfig>, sc: Arc<ServerConfig>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let c = Control::new();
    let scount = c.clone();
    let (ready_tx, ready_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let phase_owned = phase.to_string();
    let server = thread::spawn(move || {
        let (tcp, _) = listener.accept().unwrap();
        tcp.set_read_timeout(Some(Duration::from_secs(15))).unwrap();
        tcp.set_write_timeout(Some(Duration::from_secs(3))).unwrap();
        let mut s = StreamOwned::new(ServerConnection::new(sc).unwrap(), tcp);
        s.write_all(b"* OK synthetic TLS only\r\n").unwrap();
        s.flush().unwrap();
        let mut reader = BufReader::new(s);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let tag = line.split(' ').next().unwrap();
        reader
            .get_mut()
            .write_all(format!("{tag} OK login\r\n").as_bytes())
            .unwrap();
        reader.get_mut().flush().unwrap();
        line.clear();
        reader.read_line(&mut line).unwrap();
        assert!(line.ends_with(" IDLE\r\n"));
        let idle_tag = line.split(' ').next().unwrap().to_string();
        reader.get_mut().write_all(b"+ idling\r\n").unwrap();
        reader.get_mut().flush().unwrap();
        match phase_owned.as_str() {
            "partial" | "partial_resume" => {
                let s = reader.get_mut();
                s.conn.writer().write_all(b"* 1 EXISTS\r\n").unwrap();
                let mut record = Vec::new();
                while s.conn.wants_write() {
                    s.conn.write_tls(&mut record).unwrap();
                }
                assert!(record.len() > 7);
                s.sock.write_all(&record[..7]).unwrap();
                ready_tx.send(()).unwrap();
                let _ = release_rx.recv_timeout(Duration::from_secs(12));
                if s.sock.write_all(&record[7..]).is_err() {
                    return;
                }
            }
            "done" => {
                reader.get_mut().write_all(b"* 1 EXISTS\r\n").unwrap();
                reader.get_mut().flush().unwrap();
                line.clear();
                reader.read_line(&mut line).unwrap();
                assert_eq!(line, "DONE\r\n");
                scount.counts.done.fetch_add(1, Ordering::Relaxed);
                ready_tx.send(()).unwrap();
                let _ = release_rx.recv_timeout(Duration::from_secs(12));
                let _ = reader
                    .get_mut()
                    .write_all(format!("{idle_tag} OK done\r\n").as_bytes());
                let _ = reader.get_mut().flush();
                return;
            }
            _ => {
                ready_tx.send(()).unwrap();
                let _ = release_rx.recv_timeout(Duration::from_secs(12));
                if reader.get_mut().write_all(b"* 1 EXISTS\r\n").is_err() {
                    return;
                }
                if reader.get_mut().flush().is_err() {
                    return;
                }
            }
        }
        line.clear();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            return;
        }
        if line == "DONE\r\n" {
            scount.counts.done.fetch_add(1, Ordering::Relaxed);
            let _ = reader
                .get_mut()
                .write_all(format!("{idle_tag} OK done\r\n").as_bytes());
            let _ = reader.get_mut().flush();
        }
    });
    let tcp = TcpStream::connect(addr).unwrap();
    tcp.set_read_timeout(Some(SLICE)).unwrap();
    tcp.set_write_timeout(Some(Duration::from_secs(3))).unwrap();
    let conn = ClientConnection::new(cc, ServerName::try_from("localhost").unwrap()).unwrap();
    let socket = CountSocket {
        tcp,
        control: c.clone(),
    };
    let transport: Box<dyn imap::ImapConnection> = if placement == "above" {
        Box::new(Above {
            tls: StreamOwned::new(conn, socket),
            control: c.clone(),
        })
    } else {
        Box::new(Below {
            tls: StreamOwned::new(conn, BelowSocket(socket)),
            control: c.clone(),
        })
    };
    let (done_tx, done_rx) = mpsc::channel();
    let wc = c.clone();
    let worker = thread::spawn(move || {
        let mut client = imap::Client::new(transport);
        client.read_greeting().unwrap();
        let mut session = client
            .login("synthetic", "synthetic")
            .map_err(|e| e.0)
            .unwrap();
        let result = {
            let mut h = session.idle();
            h.timeout(Duration::from_secs(180)).keepalive(false);
            h.wait_while(|reply| !matches!(reply, imap::types::UnsolicitedResponse::Exists(_)))
        };
        // The cancellation is terminal: deliberately never re-use this IMAP/TLS session.
        done_tx
            .send((format!("{result:?}"), wc.alive.load(Ordering::Relaxed)))
            .unwrap();
    });
    ready_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    thread::sleep(Duration::from_millis(25));
    let before_reads = c.counts.reads.load(Ordering::Relaxed);
    let before_timeouts = c.counts.timeouts.load(Ordering::Relaxed);
    let before_tls = c.counts.tls_calls.load(Ordering::Relaxed);
    let start = Instant::now();
    let mut reads_in_window = 0;
    let mut timeouts_in_window = 0;
    let mut tls_in_window = 0;
    let mut done_in_window = 0;
    if phase == "quiet" || phase == "partial_resume" {
        let duration = if phase == "quiet" {
            Duration::from_secs(10)
        } else {
            Duration::from_millis(350)
        };
        assert!(matches!(
            done_rx.recv_timeout(duration),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
        reads_in_window = c.counts.reads.load(Ordering::Relaxed) - before_reads;
        timeouts_in_window = c.counts.timeouts.load(Ordering::Relaxed) - before_timeouts;
        tls_in_window = c.counts.tls_calls.load(Ordering::Relaxed) - before_tls;
        done_in_window = c.counts.done.load(Ordering::Relaxed);
        assert_eq!(done_in_window, 0);
        release_tx.send(()).unwrap();
    } else {
        c.alive.store(false, Ordering::Relaxed);
    }
    let (outcome, still_alive) = done_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    let us = start.elapsed().as_micros();
    let _ = release_tx.send(());
    worker.join().unwrap();
    server.join().unwrap();
    println!(
        "{placement},{phase},{run},{us},{reads_in_window},{timeouts_in_window},{tls_in_window},{done_in_window},{still_alive},{outcome:?}"
    );
}
fn main() {
    let (cc, sc) = configs();
    println!(
        "placement,phase,run,completion_us,native_reads_window,native_timeouts_window,tls_read_calls_window,done_before_release,still_alive,outcome"
    );
    for placement in ["above", "below"] {
        for phase in ["idle", "done", "partial", "partial_resume"] {
            for run in 1..=10 {
                trial(placement, phase, run, cc.clone(), sc.clone());
            }
        }
        trial(placement, "quiet", 1, cc.clone(), sc.clone());
    }
}

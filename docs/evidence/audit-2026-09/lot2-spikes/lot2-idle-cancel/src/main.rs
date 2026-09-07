use std::{
    io::{self, BufRead, BufReader, Read, Write},
    net::{Shutdown, TcpListener, TcpStream},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

// Same timeout wrapper behavior as production, deliberately over bare loopback TCP.
struct Bounded {
    socket: Arc<TcpStream>,
    cancelled: Option<Arc<AtomicBool>>,
    logical_timeout: Duration,
}
impl Read for Bounded {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        let start = Instant::now();
        loop {
            if self
                .cancelled
                .as_ref()
                .is_some_and(|alive| !alive.load(Ordering::Relaxed))
            {
                return Err(io::Error::new(
                    io::ErrorKind::ConnectionAborted,
                    "watch cancelled",
                ));
            }
            match (&*self.socket).read(b) {
                Err(err)
                    if self.cancelled.is_some()
                        && matches!(
                            err.kind(),
                            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                        )
                        && start.elapsed() < self.logical_timeout =>
                {
                    continue;
                }
                result => return result,
            }
        }
    }
}
impl Write for Bounded {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        (&*self.socket).write(b)
    }
    fn flush(&mut self) -> io::Result<()> {
        (&*self.socket).flush()
    }
}
impl imap::extensions::idle::SetReadTimeout for Bounded {
    fn set_read_timeout(&mut self, t: Option<Duration>) -> imap::error::Result<()> {
        self.logical_timeout = t.unwrap_or(Duration::from_secs(120));
        self.socket
            .set_read_timeout(Some(if self.cancelled.is_some() {
                self.logical_timeout.min(Duration::from_millis(100))
            } else {
                self.logical_timeout
            }))
            .map_err(imap::Error::Io)
    }
}

fn trial(mode: &str, run: usize) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let (idle_tx, idle_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let server = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        socket.write_all(b"* OK loopback only\r\n").unwrap();
        let mut reader = BufReader::new(socket.try_clone().unwrap());
        let mut line = String::new();
        loop {
            line.clear();
            if reader.read_line(&mut line).unwrap_or(0) == 0 {
                break;
            }
            let (tag, cmd) = line.trim_end().split_once(' ').unwrap();
            if cmd.starts_with("LOGIN") {
                socket
                    .write_all(format!("{tag} OK logged in\r\n").as_bytes())
                    .unwrap();
            } else if cmd == "IDLE" {
                socket.write_all(b"+ idling\r\n").unwrap();
                idle_tx.send(()).unwrap();
                let _ = release_rx.recv_timeout(Duration::from_secs(3));
                // For the witness, a server event releases the unchanged 180 s heartbeat early.
                if socket.write_all(b"* 1 EXISTS\r\n").is_err() {
                    break;
                }
                line.clear();
                if reader.read_line(&mut line).unwrap_or(0) == 0 {
                    break;
                }
                if line.trim_end() == "DONE" {
                    let _ = socket.write_all(b"a2 OK idle done\r\n");
                }
                break;
            }
        }
    });
    let tcp = TcpStream::connect(addr).unwrap();
    tcp.set_read_timeout(Some(Duration::from_secs(120)))
        .unwrap();
    tcp.set_write_timeout(Some(Duration::from_secs(120)))
        .unwrap();
    let tcp = Arc::new(tcp);
    let cancel = if mode == "same_handle_shutdown_both" {
        tcp.clone()
    } else {
        Arc::new(tcp.try_clone().unwrap())
    };
    let alive = Arc::new(AtomicBool::new(true));
    let worker_alive = alive.clone();
    let read_alive = if mode.starts_with("token_") {
        Some(alive.clone())
    } else {
        None
    };
    let (done_tx, done_rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        let mut client = imap::Client::new(Bounded {
            socket: tcp,
            cancelled: read_alive,
            logical_timeout: Duration::from_secs(120),
        });
        client.read_greeting().unwrap();
        let mut session = client
            .login("synthetic", "synthetic")
            .map_err(|e| e.0)
            .unwrap();
        let result = {
            let mut handle = session.idle();
            handle.timeout(Duration::from_secs(180)).keepalive(false);
            handle.wait_while(|reply| {
                worker_alive.load(Ordering::Relaxed)
                    && !matches!(reply, imap::types::UnsolicitedResponse::Exists(_))
            })
        }; // Include Handle::drop (DONE + final response) in completion time.
        done_tx.send(format!("{result:?}")).unwrap();
    });
    idle_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    thread::sleep(Duration::from_millis(25));
    let start = Instant::now();
    if mode != "token_quiet_witness" {
        alive.store(false, Ordering::Relaxed);
    }
    let mut blocked_at_300ms = false;
    if mode == "token_sliced_read" {
        // The token itself interrupts local Read; heartbeat remains 180 seconds.
    } else if mode != "atomic_flag_only" && mode != "token_quiet_witness" {
        cancel.shutdown(Shutdown::Both).unwrap();
    } else {
        blocked_at_300ms = matches!(
            done_rx.recv_timeout(Duration::from_millis(300)),
            Err(mpsc::RecvTimeoutError::Timeout)
        );
        assert!(
            blocked_at_300ms,
            "atomic flag unexpectedly woke a quiet IDLE"
        );
        release_tx.send(()).unwrap();
    }
    let result = done_rx
        .recv_timeout(Duration::from_secs(3))
        .expect("worker must exit including IDLE drop");
    let elapsed = start.elapsed().as_micros();
    let _ = release_tx.send(());
    worker.join().unwrap();
    server.join().unwrap();
    println!("{mode},{run},{elapsed},{blocked_at_300ms},{result:?}");
}

fn main() {
    println!("mode,run,completion_us,still_blocked_at_300ms,outcome");
    if std::env::args().any(|arg| arg == "quiet") {
        for run in 1..=10 {
            trial("token_quiet_witness", run);
        }
        return;
    }
    for run in 1..=10 {
        trial("atomic_flag_only", run);
        trial("clone_shutdown_both", run);
        trial("same_handle_shutdown_both", run);
        trial("token_sliced_read", run);
    }
}

use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use super::InnerSocket;

const SLICE: Duration = Duration::from_millis(100);

/// Watcher-only read cancellation below TLS. Socket timeouts stay local;
/// the IMAP reader sees its original logical timeout or terminal cancellation.
pub(crate) struct CancelableTcp {
    tcp: TcpStream,
    alive: Option<Arc<AtomicBool>>,
    timeout: Duration,
    stopped: bool,
}

impl CancelableTcp {
    pub(crate) fn new(
        tcp: TcpStream,
        timeout: Duration,
        alive: Option<Arc<AtomicBool>>,
    ) -> io::Result<Self> {
        let mut stream = Self {
            tcp,
            alive,
            timeout,
            stopped: false,
        };
        stream.check()?;
        stream.set_timeout(timeout)?;
        Ok(stream)
    }

    fn check(&mut self) -> io::Result<()> {
        self.stopped |= self
            .alive
            .as_ref()
            .is_some_and(|alive| !alive.load(Ordering::Acquire));
        if self.stopped {
            Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "watcher stopped",
            ))
        } else {
            Ok(())
        }
    }
}

impl InnerSocket for CancelableTcp {
    fn set_timeout(&mut self, timeout: Duration) -> io::Result<()> {
        self.timeout = timeout;
        self.tcp.set_read_timeout(Some(if self.alive.is_some() {
            timeout.min(SLICE)
        } else {
            timeout
        }))
    }
}

impl Read for CancelableTcp {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let started = Instant::now();
        loop {
            self.check()?;
            let result = self.tcp.read(bytes);
            self.check()?;
            match result {
                Err(ref err)
                    if self.alive.is_some()
                        && matches!(
                            err.kind(),
                            io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                        )
                        && started.elapsed() < self.timeout => {}
                result => return result,
            }
        }
    }
}

impl Write for CancelableTcp {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.check()?;
        self.tcp.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.check()?;
        self.tcp.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::sync::mpsc;

    #[test]
    fn cancellation_interrupts_a_silent_read_and_cannot_be_reversed() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let tcp = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (_peer, _) = listener.accept().unwrap();
        let alive = Arc::new(AtomicBool::new(true));
        let mut stream =
            CancelableTcp::new(tcp, Duration::from_secs(180), Some(alive.clone())).unwrap();
        let (tx, rx) = mpsc::channel();
        let worker_alive = alive.clone();
        let worker = std::thread::spawn(move || {
            let error = stream.read(&mut [0]).unwrap_err();
            worker_alive.store(true, Ordering::Release);
            assert_eq!(
                stream.write(b"DONE\r\n").unwrap_err().kind(),
                io::ErrorKind::ConnectionAborted
            );
            tx.send(error.kind()).unwrap();
        });
        assert!(rx.recv_timeout(Duration::from_millis(250)).is_err());
        alive.store(false, Ordering::Release);
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(1)).unwrap(),
            io::ErrorKind::ConnectionAborted
        );
        worker.join().unwrap();
    }

    #[test]
    fn local_slices_do_not_replace_the_logical_read_timeout() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let tcp = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (_peer, _) = listener.accept().unwrap();
        let mut stream = CancelableTcp::new(
            tcp,
            Duration::from_millis(400),
            Some(Arc::new(AtomicBool::new(true))),
        )
        .unwrap();
        let started = Instant::now();
        let error = stream.read(&mut [0]).unwrap_err();
        assert!(matches!(
            error.kind(),
            io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
        ));
        assert!(started.elapsed() >= Duration::from_millis(400));
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn an_ordinary_poll_has_no_sliced_socket_timeout() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let tcp = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (_peer, _) = listener.accept().unwrap();
        let mut stream = CancelableTcp::new(tcp, Duration::from_secs(120), None).unwrap();
        assert_eq!(
            stream.tcp.read_timeout().unwrap(),
            Some(Duration::from_secs(120))
        );
        stream.set_timeout(Duration::from_secs(180)).unwrap();
        assert_eq!(
            stream.tcp.read_timeout().unwrap(),
            Some(Duration::from_secs(180))
        );
    }
}

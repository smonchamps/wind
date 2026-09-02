//! A FAKE, scripted IMAP server, in cleartext on 127.0.0.1, to prove what
//! the adapter SENDS (PLAN-AUDIT-V2 E3): which header fields it requests,
//! how many `LIST` and `CAPABILITY` per session, whether it dares `UID
//! EXPUNGE` without UIDPLUS, how it cuts a batch of bodies. It records every
//! command received and answers according to a [`Script`]; it understands
//! nothing beyond what these tests exercise — never a server, a witness.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::{BoundedStream, ImapServer};

/// What the fake server answers: the announced capabilities, the `* LIST …`
/// lines, and for each `UID FETCH` the reply lines (without the final `OK`),
/// computed from the command's text.
pub(crate) struct Script {
    pub capabilities: String,
    pub list: Vec<String>,
    pub fetch: Responder,
}

/// The reply lines to a `UID FETCH`, computed from the command.
pub(crate) type Responder = Box<dyn Fn(&str) -> Vec<String> + Send>;

impl Script {
    pub(crate) fn simple() -> Self {
        Self {
            capabilities: "IMAP4rev1 UIDPLUS MOVE CONDSTORE LIST-STATUS".to_string(),
            list: vec![
                "* LIST (\\HasNoChildren) \"/\" \"INBOX\"".to_string(),
                "* LIST (\\HasNoChildren \\Trash) \"/\" \"Corbeille\"".to_string(),
                "* LIST (\\HasNoChildren \\Drafts) \"/\" \"Brouillons\"".to_string(),
                "* LIST (\\HasNoChildren \\Sent) \"/\" \"Envoyes\"".to_string(),
                "* LIST (\\HasNoChildren) \"/\" \"Archive\"".to_string(),
            ],
            fetch: Box::new(|_| Vec::new()),
        }
    }
}

pub(crate) struct FakeImap {
    port: u16,
    commands: Arc<Mutex<Vec<String>>>,
}

/// An IMAP literal: `{n}` then the bytes.
pub(crate) fn literal(text: &str) -> String {
    format!("{{{}}}\r\n{text}", text.len())
}

/// The UIDs a `UID FETCH 1:3,7 (…)` command designates — `1:*` means
/// `1..=3`, the fake mailbox announcing 3 messages.
pub(crate) fn uids_of(command: &str) -> Vec<u32> {
    let Some(rest) = command
        .strip_prefix("UID FETCH ")
        .or_else(|| command.strip_prefix("uid fetch "))
    else {
        return Vec::new();
    };
    let set = rest.split(' ').next().unwrap_or("");
    let mut uids = Vec::new();
    for piece in set.split(',') {
        match piece.split_once(':') {
            Some((a, b)) => {
                let a: u32 = a.parse().unwrap_or(1);
                let b: u32 = if b == "*" { 3 } else { b.parse().unwrap_or(a) };
                uids.extend(a..=b);
            }
            None => {
                if let Ok(single) = piece.parse::<u32>() {
                    uids.push(single);
                }
            }
        }
    }
    uids
}

impl FakeImap {
    /// Starts the server for ONE connection; the thread dies with it.
    pub(crate) fn start(script: Script) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let commands: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&commands);
        std::thread::spawn(move || {
            let (sock, _) = listener.accept().unwrap();
            serve(sock, &script, &log);
        });
        Self { port, commands }
    }

    /// An authenticated session on this server, in cleartext, bounded to 2 s.
    pub(crate) fn connect(&self) -> ImapServer {
        let tcp = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        tcp.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let mut client = imap::Client::new(
            Box::new(BoundedStream::new(tcp, Duration::from_secs(2))) as imap::Connection,
        );
        client.read_greeting().unwrap();
        let session = client
            .login("me", "secret")
            .map_err(|(err, _)| err)
            .unwrap();
        ImapServer::for_test(session)
    }

    /// The commands received, tag removed, in order.
    pub(crate) fn commands(&self) -> Vec<String> {
        self.commands.lock().unwrap().clone()
    }
}

fn serve(mut sock: TcpStream, script: &Script, log: &Mutex<Vec<String>>) {
    sock.write_all(b"* OK fake server ready\r\n").unwrap();
    let mut reader = BufReader::new(sock.try_clone().unwrap());
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            return;
        }
        let text = line.trim_end();
        let Some((tag, command)) = text.split_once(' ') else {
            continue;
        };
        log.lock().unwrap().push(command.to_string());
        let upper = command.to_ascii_uppercase();
        let mut reply = String::new();
        if upper.starts_with("CAPABILITY") {
            reply.push_str(&format!("* CAPABILITY {}\r\n", script.capabilities));
        } else if upper.starts_with("LIST") {
            for l in &script.list {
                reply.push_str(l);
                reply.push_str("\r\n");
            }
        } else if upper.starts_with("SELECT") || upper.starts_with("EXAMINE") {
            reply
                .push_str("* 3 EXISTS\r\n* OK [UIDVALIDITY 1] ok\r\n* OK [HIGHESTMODSEQ 5] ok\r\n");
        } else if upper.starts_with("UID FETCH") {
            for l in (script.fetch)(command) {
                reply.push_str(&l);
                reply.push_str("\r\n");
            }
        } else if upper.starts_with("LOGOUT") {
            reply.push_str("* BYE\r\n");
        }
        reply.push_str(&format!("{tag} OK done\r\n"));
        if sock.write_all(reply.as_bytes()).is_err() {
            return;
        }
        if upper.starts_with("LOGOUT") {
            return;
        }
    }
}

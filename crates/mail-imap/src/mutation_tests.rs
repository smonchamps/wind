//! Stateful loopback witness: mailbox contents survive client reconnections.
use crate::{BoundedStream, ImapServer};
use mail_core::{Action, Store, SyncEngine};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

#[derive(Clone, Copy, PartialEq)]
enum Cut {
    None,
    BeforeCopy,
    CopyAck,
    MoveNo,
    Store,
    Expunge,
}
struct Remote {
    source: bool,
    unrelated_deleted: bool,
    copies: usize,
    cut: Cut,
    capabilities: String,
    source_generation: u32,
    destination_generation: u32,
    copyuid: Option<String>,
    commands: Vec<String>,
}
struct Witness {
    port: u16,
    remote: Arc<Mutex<Remote>>,
    stop: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Witness {
    fn start(cut: Cut, capabilities: &str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        listener.set_nonblocking(true).unwrap();
        let remote = Arc::new(Mutex::new(Remote {
            source: true,
            unrelated_deleted: true,
            copies: 0,
            cut,
            capabilities: capabilities.to_string(),
            source_generation: 1,
            destination_generation: 7,
            copyuid: None,
            commands: Vec::new(),
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let worker_remote = remote.clone();
        let worker_stop = stop.clone();
        let worker = std::thread::spawn(move || {
            while !worker_stop.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((socket, _)) => serve(socket, &worker_remote),
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(5))
                    }
                    Err(err) => panic!("loopback accept: {err}"),
                }
            }
        });
        Self {
            port,
            remote,
            stop,
            worker: Some(worker),
        }
    }
    fn connect(&self) -> ImapServer {
        let tcp = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        let mut client = imap::Client::new(
            Box::new(BoundedStream::new(tcp, Duration::from_secs(2))) as imap::Connection,
        );
        client.read_greeting().unwrap();
        ImapServer::for_test(
            client
                .login("fixture", "synthetic")
                .map_err(|(err, _)| err)
                .unwrap(),
        )
    }
    fn poll(&self, store: &mut Store, account: i64) {
        let mut server = self.connect();
        let _ = SyncEngine::default().sync(&mut server, store, account, "INBOX");
        server.logout();
    }
}
impl Drop for Witness {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            worker.join().unwrap();
        }
    }
}
fn serve(mut socket: TcpStream, remote: &Mutex<Remote>) {
    socket.set_nonblocking(false).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    socket
        .write_all(b"* OK synthetic mutation witness\r\n")
        .unwrap();
    let mut input = BufReader::new(socket.try_clone().unwrap());
    let mut line = String::new();
    loop {
        line.clear();
        if input.read_line(&mut line).unwrap_or(0) == 0 {
            return;
        }
        let Some((tag, command)) = line.trim_end().split_once(' ') else {
            continue;
        };
        let mut state = remote.lock().unwrap();
        state.commands.push(command.to_string());
        let mut reply = String::new();
        let mut code = String::new();
        if command.starts_with("CAPABILITY") {
            reply = format!("* CAPABILITY {}\r\n", state.capabilities);
        } else if command.starts_with("SELECT") {
            reply = format!(
                "* {} EXISTS\r\n* OK [UIDVALIDITY {}] generation\r\n",
                if state.source { 2 } else { 1 },
                state.source_generation
            );
        } else if command.starts_with("STATUS") {
            reply = format!(
                "* STATUS \"Archive\" (MESSAGES {} UIDVALIDITY {} UIDNEXT {})\r\n",
                state.copies,
                state.destination_generation,
                100 + state.copies
            );
        } else if command.starts_with("LIST") {
            reply = "* LIST (\\Archive) \"/\" \"Archive\"\r\n".to_string();
        } else if command.starts_with("UID SEARCH") {
            reply = if state.source {
                "* SEARCH 1 99\r\n"
            } else {
                "* SEARCH 99\r\n"
            }
            .to_string();
        } else if command.starts_with("UID COPY") || command.starts_with("UID MOVE") {
            if state.cut == Cut::BeforeCopy {
                state.cut = Cut::None;
                return;
            }
            if state.source {
                state.copies += 1;
            }
            if state.cut == Cut::MoveNo {
                state.cut = Cut::None;
                let reply = format!("* OK [COPYUID 7 1 101] copied\r\n{tag} NO partial move\r\n");
                socket.write_all(reply.as_bytes()).unwrap();
                continue;
            }
            if command.starts_with("UID MOVE") {
                state.source = false;
            }
            if state.cut == Cut::CopyAck {
                state.cut = Cut::None;
                return;
            }
            if let Some(value) = &state.copyuid {
                code = format!("[{value}] ");
            }
        } else if command.starts_with("UID STORE") && state.cut == Cut::Store {
            state.cut = Cut::None;
            return;
        } else if command.starts_with("UID EXPUNGE 1") {
            state.source = false;
            if state.cut == Cut::Expunge {
                state.cut = Cut::None;
                return;
            }
        } else if command == "EXPUNGE" {
            state.source = false;
            state.unrelated_deleted = false;
        } else if command == "LOGOUT" {
            reply = "* BYE\r\n".to_string();
        }
        reply.push_str(&format!("{tag} OK {code}done\r\n"));
        if socket.write_all(reply.as_bytes()).is_err() || command == "LOGOUT" {
            return;
        }
    }
}
fn queued_store() -> (Store, i64) {
    let store = Store::open_in_memory().unwrap();
    let account = store
        .adopt_or_create_account("journal@example.invalid", "generic")
        .unwrap();
    let mailbox = store.create_mailbox(account, "INBOX", 1).unwrap();
    store
        .enqueue_action(mailbox, 1, Action::MoveTo("Archive".to_string()))
        .unwrap();
    (store, account)
}
#[test]
fn copy_ack_lost_never_repeats_the_copy() {
    let witness = Witness::start(Cut::CopyAck, "IMAP4rev1 UIDPLUS");
    let (mut store, account) = queued_store();
    for _ in 0..4 {
        witness.poll(&mut store, account);
    }
    let state = witness.remote.lock().unwrap();
    assert_eq!(
        state.copies, 1,
        "lost ACK must not create another copy: {:?}",
        state.commands
    );
    assert!(state.source);
    assert!(state.unrelated_deleted);
}
#[test]
fn confirmed_copy_resumes_only_source_removal() {
    let witness = Witness::start(Cut::Store, "IMAP4rev1 UIDPLUS");
    let (mut store, account) = queued_store();
    for _ in 0..4 {
        witness.poll(&mut store, account);
    }
    let state = witness.remote.lock().unwrap();
    assert_eq!(
        state.copies, 1,
        "confirmed COPY is durable: {:?}",
        state.commands
    );
    assert!(!state.source);
    assert!(state.unrelated_deleted);
}

#[test]
fn mutation_cuts_preserve_unrelated_deleted_messages() {
    for (cut, caps, expected_copies, expected_source, expected_incidents) in [
        (Cut::BeforeCopy, "IMAP4rev1 UIDPLUS", 0, true, 1),
        (Cut::Expunge, "IMAP4rev1 UIDPLUS", 1, false, 0),
        (Cut::CopyAck, "IMAP4rev1 MOVE", 1, false, 1),
        (Cut::None, "IMAP4rev1 MOVE", 1, false, 0),
        (Cut::None, "IMAP4rev1", 0, true, 0),
    ] {
        let witness = Witness::start(cut, caps);
        let (mut store, account) = queued_store();
        for _ in 0..4 {
            witness.poll(&mut store, account);
        }
        let state = witness.remote.lock().unwrap();
        assert_eq!(state.copies, expected_copies, "{:?}", state.commands);
        assert_eq!(state.source, expected_source, "{:?}", state.commands);
        assert!(state.unrelated_deleted);
        assert_eq!(
            store.action_incidents().unwrap().len(),
            expected_incidents,
            "{:?}",
            state.commands
        );
        if caps == "IMAP4rev1" {
            assert!(
                !state
                    .commands
                    .iter()
                    .any(|command| command.starts_with("UID COPY")
                        || command.starts_with("UID STORE")
                        || command.contains("EXPUNGE"))
            );
        }
    }
}

#[test]
fn a_copied_transfer_never_deletes_a_new_source_or_a_changed_destination() {
    for source_reset in [true, false] {
        let witness = Witness::start(Cut::Store, "IMAP4rev1 UIDPLUS");
        let (mut store, account) = queued_store();
        witness.poll(&mut store, account);
        {
            let mut state = witness.remote.lock().unwrap();
            if source_reset {
                state.source_generation += 1;
            } else {
                state.destination_generation += 1;
            }
        }
        for _ in 0..3 {
            witness.poll(&mut store, account);
        }
        let state = witness.remote.lock().unwrap();
        assert_eq!(state.copies, 1);
        assert!(state.source);
        assert!(state.unrelated_deleted);
        assert!(
            !state
                .commands
                .iter()
                .any(|command| command.starts_with("UID EXPUNGE"))
        );
        assert_eq!(store.action_incidents().unwrap().len(), 1);
    }
}

#[test]
fn copyuid_disagreement_keeps_the_source_and_does_not_repeat_copy() {
    for code in ["COPYUID 8 1 101", "COPYUID 7 2 101", "COPYUID 7 1 101:102"] {
        let witness = Witness::start(Cut::None, "IMAP4rev1 UIDPLUS");
        witness.remote.lock().unwrap().copyuid = Some(code.to_string());
        let (mut store, account) = queued_store();
        for _ in 0..3 {
            witness.poll(&mut store, account);
        }
        let state = witness.remote.lock().unwrap();
        assert_eq!(state.copies, 1, "{code}");
        assert!(state.source, "{code}");
        assert_eq!(store.action_incidents().unwrap().len(), 1, "{code}");
    }
}

#[test]
fn tagged_and_untagged_copyuid_use_the_existing_parser_and_match_one_identity() {
    for reply in [
        "a1 OK [COPYUID 7 1 101] done\r\n",
        "* OK [COPYUID 7 1 101] copied\r\na1 OK done\r\n",
        "* OK [COPYUID 7 1:1 101:101] copied\r\na1 OK [COPYUID 7 1 101] done\r\n",
        "a1 OK done\r\n",
    ] {
        super::validate_copy_uid(reply.as_bytes(), 1, Some(7)).unwrap();
    }
    for reply in [
        "* OK [COPYUID 7 1 101] copied\r\na1 OK [COPYUID 7 1 102] done\r\n",
        "a1 OK [COPYUID 7 0 101] done\r\n",
        "a1 OK [COPYUID 7 1 0] done\r\n",
        "a1 OK [COPYUID 7 1:4294967295 101] done\r\n",
        "a1 OK [COPYUID 7 1 101:1] done\r\n",
    ] {
        assert!(
            super::validate_copy_uid(reply.as_bytes(), 1, Some(7)).is_err(),
            "{reply}"
        );
    }
}

#[test]
fn a_move_effect_followed_by_no_keeps_an_uncertain_incident() {
    let witness = Witness::start(Cut::MoveNo, "IMAP4rev1 UIDPLUS MOVE");
    let (mut store, account) = queued_store();
    for _ in 0..4 {
        witness.poll(&mut store, account);
    }
    assert_eq!(witness.remote.lock().unwrap().copies, 1);
    assert!(witness.remote.lock().unwrap().source);
    assert_eq!(store.action_incidents().unwrap().len(), 1);
    assert_eq!(store.refused_actions().unwrap(), 0);
}

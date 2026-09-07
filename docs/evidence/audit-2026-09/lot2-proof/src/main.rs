use mail_core::{Store, MailTransport, OutboxMessage, SendError, OutboxState, DraftContent};
use std::sync::mpsc;
use std::time::Duration;

mod relocation { pub const APP_ID: &str = "unused-synthetic-proof"; }
#[path = "../../../apps/desktop/src/instance.rs"]
mod instance;

fn draft() -> mail_core::Draft {
    mail_core::compose("sender@example.com", "recipient@example.com", "", "", "Synthetic lot 2", "Synthetic payload", None).unwrap()
}

struct Ambiguous(usize);
impl MailTransport for Ambiguous {
    fn send(&mut self, _: &OutboxMessage) -> Result<(), SendError> {
        self.0 += 1;
        Err(SendError::Transient("synthetic lost acknowledgement".into()))
    }
}

struct Held { entered: mpsc::Sender<()>, release: mpsc::Receiver<()> }
impl MailTransport for Held {
    fn send(&mut self, _: &OutboxMessage) -> Result<(), SendError> {
        self.entered.send(()).unwrap();
        self.release.recv_timeout(Duration::from_secs(10)).unwrap();
        Ok(())
    }
}

fn main() {
    for n in 0..3 {
        let mut store = Store::open_in_memory().unwrap();
        let account = store.adopt_or_create_account("sender@example.com", "gmail").unwrap();
        store.enqueue_outbox(account, &draft()).unwrap();
        let mut transport = Ambiguous(0);
        mail_core::flush_outbox(&mut transport, &mut store, account).unwrap();
        mail_core::flush_outbox(&mut transport, &mut store, account).unwrap();
        println!("AMBIGUOUS run={n} deliveries={} queued={}", transport.0, store.outbox_in_state(OutboxState::Queued).unwrap().len());

        let dir = std::env::current_dir().unwrap().join(format!("fixture-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("wind.db");
        let mut local = Store::open(&path).unwrap();
        let account = local.adopt_or_create_account("sender@example.com", "gmail").unwrap();
        local.enqueue_outbox(account, &draft()).unwrap();
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let mut writer = Store::open(&path).unwrap();
            let mut transport = Held { entered: entered_tx, release: release_rx };
            mail_core::flush_outbox(&mut transport, &mut writer, account).unwrap()
        });
        entered_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        local.delete_account(account).unwrap();
        let account_gone = local.accounts().unwrap().is_empty();
        release_tx.send(()).unwrap();
        let report = worker.join().unwrap();
        println!("REMOVAL run={n} removed_before_send_completed={account_gone} accepted_after_removal={} outbox_rows={}", report.sent, local.outbox_in_state(OutboxState::Sent).unwrap().len());

        let local = Store::open_in_memory().unwrap();
        let account = local.adopt_or_create_account("sender@example.com", "gmail").unwrap();
        local.begin_draft_edit("lost-ack", account, None, None, None).unwrap();
        let first = local.enqueue_draft_edit("lost-ack", account, &draft(), None).unwrap();
        let retried = local.enqueue_draft_edit("lost-ack", account, &draft(), None).unwrap();
        let content = DraftContent { to_raw:"recipient@example.com", cc_raw:"", bcc_raw:"", subject:"Synthetic lot 2", body:"Synthetic payload", body_html:None, reply_to_uid:None, reply_to_mailbox:None, important:false };
        let save = local.save_draft_edit("lost-ack", account, content);
        println!("LOST_IPC_ACK run={n} direct_retry_same_id={} compose_presave_failed={} queued={}", first==retried, save.is_err(), local.outbox_in_state(OutboxState::Queued).unwrap().len());

        let invalid_folder = dir.join("file-not-directory");
        std::fs::write(&invalid_folder, b"synthetic").unwrap();
        println!("INSTANCE run={n} lock_error_on_invalid_folder={}", instance::lock_patiently(&invalid_folder).is_err());
    }
}

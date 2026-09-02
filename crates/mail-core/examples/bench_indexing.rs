//! Bench of the INDEXING of a heavy body (PLAN-AUDIT-V2 E2): what does
//! `save_body` — hence `indexable_text` and the FTS5 index — cost on a
//! 28 MB HTML body (the biggest known in the field, D-1)? Duration here;
//! the memory peak is read from the outside, without `unsafe` (the
//! workspace forbids it):
//!
//! ```powershell
//! $p = Start-Process target\release\examples\bench_indexing.exe -PassThru -Wait -NoNewWindow
//! "{0:n0} MB peak" -f ($p.PeakWorkingSet64 / 1MB)
//! ```
//!
//! In-memory database, synthetic body: no real content. The body is
//! built BEFORE the stopwatch; the peak outside the body = peak − 28 MB
//! − ~10 MB of base (a run with `0` MB gives the base).
//!
//! ```powershell
//! cargo run -p mail-core --example bench_indexing --release -- [mb]
//! ```

use std::time::Instant;

use mail_core::Store;

fn body(mb: usize) -> String {
    let paragraph = "<p style=\"margin:0\">Hello everyone, here is the monthly <b>newsletter</b> \
        &mdash; coffee, tea &amp; chocolate.</p>\n";
    let mut html = String::with_capacity(mb * 1024 * 1024 + 1024);
    html.push_str("<html><head><style>p { color: red }</style></head><body>");
    while html.len() < mb * 1024 * 1024 {
        html.push_str(paragraph);
    }
    html.push_str("</body></html>");
    html
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mb: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(28);
    let mut store = Store::open_in_memory()?;
    let account = store.adopt_or_create_account("bench@example.com", "gmail")?;
    let inbox = store.create_mailbox(account, "INBOX", 1)?;
    // The body is indexed with ITS envelope: without it, there is nothing to index.
    let envelope = mail_core::Envelope {
        reply_to: None,
        uid: 1,
        subject: Some("Newsletter".to_string()),
        sender: Some("The Gazette".to_string()),
        sender_address: Some("gazette@example.com".to_string()),
        to_addrs: vec!["bench@example.com".to_string()],
        cc_addrs: Vec::new(),
        message_id: Some("<bench-1@example.com>".to_string()),
        in_reply_to: None,
        date: None,
        seen: false,
        flagged: false,
    };
    store.upsert_envelopes(inbox, &[envelope])?;
    let html = body(mb);
    println!("body: {} MB", html.len() / (1024 * 1024));

    let start = Instant::now();
    store.save_body(inbox, 1, &html, &[])?;
    println!("save_body (indexing included): {:?}", start.elapsed());
    Ok(())
}

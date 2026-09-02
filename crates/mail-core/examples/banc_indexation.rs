//! Banc de l'INDEXATION d'un corps lourd (PLAN-AUDIT-V2 E2) : que coûte
//! `save_body` — donc `indexable_text` et l'index FTS5 — sur un corps
//! HTML de 28 Mo (le plus gros connu au terrain, D-1) ? Durée ici ; le
//! pic mémoire se lit de l'extérieur, sans `unsafe` (le workspace
//! l'interdit) :
//!
//! ```powershell
//! $p = Start-Process target\release\examples\banc_indexation.exe -PassThru -Wait -NoNewWindow
//! "{0:n0} Mo de pic" -f ($p.PeakWorkingSet64 / 1MB)
//! ```
//!
//! Base en mémoire, corps synthétique : aucun contenu réel. Le corps est
//! construit AVANT le chrono ; le pic hors corps = pic − 28 Mo − ~10 Mo de
//! socle (un lancement avec `0` Mo donne le socle).
//!
//! ```powershell
//! cargo run -p mail-core --example banc_indexation --release -- [mo]
//! ```

use std::time::Instant;

use mail_core::Store;

fn corps(mo: usize) -> String {
    let paragraphe = "<p style=\"margin:0\">Bonjour &agrave; tous, voici la <b>lettre</b> \
        d&rsquo;information du mois &mdash; caf&eacute;, th&eacute; &amp; chocolat.</p>\n";
    let mut html = String::with_capacity(mo * 1024 * 1024 + 1024);
    html.push_str("<html><head><style>p { color: red }</style></head><body>");
    while html.len() < mo * 1024 * 1024 {
        html.push_str(paragraphe);
    }
    html.push_str("</body></html>");
    html
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mo: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(28);
    let mut store = Store::open_in_memory()?;
    let account = store.adopt_or_create_account("banc@exemple.fr", "gmail")?;
    let inbox = store.create_mailbox(account, "INBOX", 1)?;
    // Le corps s'indexe avec SON enveloppe : sans elle, rien à indexer.
    let enveloppe = mail_core::Envelope {
        uid: 1,
        subject: Some("Lettre d'information".to_string()),
        sender: Some("La Gazette".to_string()),
        sender_address: Some("gazette@exemple.fr".to_string()),
        to_addrs: vec!["banc@exemple.fr".to_string()],
        cc_addrs: Vec::new(),
        message_id: Some("<banc-1@exemple.fr>".to_string()),
        in_reply_to: None,
        date: None,
        seen: false,
        flagged: false,
    };
    store.upsert_envelopes(inbox, &[enveloppe])?;
    let html = corps(mo);
    println!("corps : {} Mo", html.len() / (1024 * 1024));

    let depart = Instant::now();
    store.save_body(inbox, 1, &html, &[])?;
    println!("save_body (indexation comprise) : {:?}", depart.elapsed());
    Ok(())
}

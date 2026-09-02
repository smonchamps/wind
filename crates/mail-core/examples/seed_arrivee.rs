//! Outil de décor E2E (PLAN-MODE-ORGANISE E2) : simule l'ARRIVÉE de
//! courrier — des enveloppes neuves, datées de MAINTENANT, ajoutées à
//! l'INBOX d'un compte existant par LE chemin de production
//! (`upsert_envelopes`, où vit la décision d'arrivée du Portier).
//! Contrairement à `seed_inbox`, la boîte n'est PAS réinitialisée :
//! l'outil s'exécute pendant que l'application tourne (WAL), comme une
//! synchronisation le ferait.
//!
//! ```powershell
//! cargo run -p mail-core --example seed_arrivee -- <chemin.db> <email-compte> <adresse-expediteur> <n> [nom] [sujet] [reponse-a]
//! ```
//!
//! `reponse-a` (RETOURS-14 R4) : un Message-ID existant — l'arrivée
//! REJOINT ce fil (In-Reply-To), le décor du « fil mêlé » : un inconnu
//! répond dans le fil d'un connu.

use chrono::Utc;
use mail_core::{Envelope, Store};

fn main() -> Result<(), mail_core::Error> {
    let args: Vec<String> = std::env::args().collect();
    // Zéro unwrap/expect, même dans un outil (§2.4) : un argument
    // manquant dit l'usage et sort — jamais une panique.
    let [Some(path), Some(email), Some(expediteur)] = [args.get(1), args.get(2), args.get(3)]
    else {
        eprintln!(
            "usage : seed_arrivee <chemin.db> <email-compte> <adresse-expediteur> [n] [nom] [sujet]"
        );
        std::process::exit(2);
    };
    let n: u32 = args
        .get(4)
        .and_then(|value| value.parse().ok())
        .unwrap_or(1);
    let nom = args.get(5).cloned().unwrap_or_else(|| expediteur.clone());
    let sujet = args
        .get(6)
        .cloned()
        .unwrap_or_else(|| "Premier contact".to_string());
    let reponse_a = args.get(7).cloned();

    let mut store = Store::open(std::path::Path::new(path))?;
    let account = store.adopt_or_create_account(email, "gmail")?;
    let Some(state) = store.sync_state(account, "INBOX")? else {
        eprintln!("seed_arrivee : l'INBOX du compte {email} n'existe pas (seed_inbox d'abord)");
        std::process::exit(2);
    };
    let depart = store.max_uid(state.mailbox_id)? + 1;

    let lot: Vec<Envelope> = (0..n)
        .map(|i| {
            let uid = depart + i;
            Envelope {
                reply_to: None,
                uid,
                subject: Some(format!("{sujet} n°{uid}")),
                sender: Some(nom.clone()),
                sender_address: Some(expediteur.clone()),
                message_id: Some(format!("<arrivee-{uid}@{expediteur}>")),
                in_reply_to: reponse_a.clone(),
                date: Some(Utc::now()),
                seen: false,
                flagged: false,
                to_addrs: vec![email.clone()],
                cc_addrs: Vec::new(),
            }
        })
        .collect();
    store.upsert_envelopes(state.mailbox_id, &lot)?;
    println!("arrivee : {n} message(s) de {expediteur} (uid {depart}..)");
    Ok(())
}

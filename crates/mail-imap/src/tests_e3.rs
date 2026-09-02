//! PLAN-AUDIT-V2 E3 — ce que l'adaptateur ENVOIE, prouvé sur le faux
//! serveur scripté ([`crate::faux_serveur`]). Chaque test a été joué RED
//! contre l'adaptateur d'avant (voir le PLAN).

use mail_core::MailServer;

use crate::faux_serveur::{FauxImap, Script, litteral, uids_de};

fn mime_minuscule(uid: u32) -> String {
    format!(
        "From: alice@ex.fr\r\nSubject: message {uid}\r\nContent-Type: text/plain\r\n\r\nbonjour {uid}\r\n"
    )
}

fn enveloppe(uid: u32) -> String {
    format!(
        "* {uid} FETCH (UID {uid} FLAGS (\\Seen) INTERNALDATE \"01-Jan-2026 00:00:00 +0000\" \
         ENVELOPE (\"Thu, 1 Jan 2026 00:00:00 +0000\" \"Sujet {uid}\" \
         ((\"Alice\" NIL \"alice\" \"ex.fr\")) NIL NIL NIL NIL NIL NIL \"<m{uid}@ex.fr>\"))"
    )
}

/// Le banc du rattrapage : 50 corps multipart de ~56 ko (texte, HTML,
/// une pièce de 30 ko en base64) passés par l'analyse — le poste CPU
/// dominant du rattrapage. `cargo test -p mail-imap banc_analyse --
/// --ignored --nocapture`.
#[test]
#[ignore = "banc : mesure, pas un filet"]
fn banc_analyse_de_50_corps() {
    let piece = "QUJDRA==".repeat(30 * 1024 / 8);
    let html = "<p>Bonjour &agrave; tous, voici la lettre du mois.</p>".repeat(400);
    let brut = format!(
        "From: alice@ex.fr\r\nTo: bob@ex.fr\r\nSubject: lettre\r\nMIME-Version: 1.0\r\n\
         Content-Type: multipart/mixed; boundary=\"b1\"\r\n\r\n\
         --b1\r\nContent-Type: multipart/alternative; boundary=\"b2\"\r\n\r\n\
         --b2\r\nContent-Type: text/plain\r\n\r\nBonjour a tous\r\n\
         --b2\r\nContent-Type: text/html\r\n\r\n{html}\r\n--b2--\r\n\
         --b1\r\nContent-Type: application/pdf; name=\"doc.pdf\"\r\n\
         Content-Disposition: attachment; filename=\"doc.pdf\"\r\n\
         Content-Transfer-Encoding: base64\r\n\r\n{piece}\r\n--b1--\r\n"
    );
    println!("corps : {} ko", brut.len() / 1024);
    let depart = std::time::Instant::now();
    let mut pieces = 0;
    for _ in 0..50 {
        let corps = crate::body_from_raw(brut.as_bytes()).expect("analysable");
        pieces += corps.attachments.len();
    }
    println!(
        "50 corps analysés en {:?} ({pieces} pièces vues)",
        depart.elapsed()
    );
}

#[test]
fn les_en_tetes_de_fil_ne_demandent_que_trois_champs() {
    let mut script = Script::simple();
    script.fetch = Box::new(|commande| {
        let texte = "Message-ID: <m1@ex.fr>\r\nReferences: <a@ex.fr> <b@ex.fr>\r\n\r\n";
        uids_de(commande)
            .into_iter()
            .map(|uid| {
                format!(
                    "* {uid} FETCH (UID {uid} BODY[HEADER.FIELDS (MESSAGE-ID IN-REPLY-TO REFERENCES)] {})",
                    litteral(texte)
                )
            })
            .collect()
    });
    let faux = FauxImap::lancer(script);
    let mut serveur = faux.connecter();

    let lus = serveur.fetch_thread_headers("INBOX", &[1]).unwrap();
    assert_eq!(lus.len(), 1);
    assert_eq!(lus[0].1.references.as_deref(), Some("<a@ex.fr> <b@ex.fr>"));

    let fetch = faux
        .commandes()
        .into_iter()
        .find(|c| c.starts_with("UID FETCH"))
        .expect("un FETCH est parti");
    assert!(
        fetch.contains("BODY.PEEK[HEADER.FIELDS (MESSAGE-ID IN-REPLY-TO REFERENCES)]"),
        "le bloc d'en-têtes entier est demandé : {fetch}"
    );
}

#[test]
fn un_lot_de_corps_est_borne_a_32_mo() {
    let mut script = Script::simple();
    script.fetch = Box::new(|commande| {
        let uids = uids_de(commande);
        if commande.contains("RFC822.SIZE") {
            // 20 Mo, 20 Mo, 1 ko : les deux premiers ne tiennent pas
            // ensemble sous 32 Mo, le troisième suit le deuxième.
            uids.into_iter()
                .map(|uid| {
                    let taille = if uid == 3 { 1024 } else { 20 * 1024 * 1024 };
                    format!("* {uid} FETCH (UID {uid} RFC822.SIZE {taille})")
                })
                .collect()
        } else {
            uids.into_iter()
                .map(|uid| {
                    format!(
                        "* {uid} FETCH (UID {uid} BODY[] {})",
                        litteral(&mime_minuscule(uid))
                    )
                })
                .collect()
        }
    });
    let faux = FauxImap::lancer(script);
    let mut serveur = faux.connecter();

    let corps = serveur.fetch_bodies_html("INBOX", &[1, 2, 3]).unwrap();
    assert_eq!(corps.len(), 3, "les trois corps arrivent");

    let lots: Vec<String> = faux
        .commandes()
        .into_iter()
        .filter(|c| c.contains("BODY.PEEK[]"))
        .collect();
    assert_eq!(lots.len(), 2, "deux lots attendus, vus : {lots:?}");
    assert!(lots[0].starts_with("UID FETCH 1 "), "{}", lots[0]);
    assert!(lots[1].starts_with("UID FETCH 2:3 "), "{}", lots[1]);
}

#[test]
fn un_serveur_sans_uidplus_n_envoie_jamais_uid_expunge() {
    let mut script = Script::simple();
    script.capacites = "IMAP4rev1".to_string();
    let faux = FauxImap::lancer(script);
    let mut serveur = faux.connecter();

    serveur.move_to("INBOX", 1, "Archive").unwrap();

    let commandes = faux.commandes();
    assert!(
        commandes.iter().any(|c| c.starts_with("UID COPY 1 ")),
        "sans MOVE, une copie : {commandes:?}"
    );
    assert!(
        !commandes.iter().any(|c| c.starts_with("UID EXPUNGE")),
        "UID EXPUNGE sans UIDPLUS : {commandes:?}"
    );
    assert!(
        commandes.iter().any(|c| c == "EXPUNGE"),
        "l'EXPUNGE de RFC 3501 manque : {commandes:?}"
    );
}

#[test]
fn une_session_ne_liste_qu_une_fois_pour_les_dossiers_speciaux() {
    let faux = FauxImap::lancer(Script::simple());
    let mut serveur = faux.connecter();

    assert_eq!(
        serveur.drafts_folder_name().unwrap().as_deref(),
        Some("Brouillons")
    );
    assert_eq!(
        serveur.sent_folder_name().unwrap().as_deref(),
        Some("Envoyes")
    );
    serveur.delete("INBOX", 1).unwrap(); // la corbeille, troisième lecteur

    let listes = faux
        .commandes()
        .iter()
        .filter(|c| c.starts_with("LIST"))
        .count();
    assert_eq!(listes, 1, "une LIST par session : {:?}", faux.commandes());
}

#[test]
fn une_session_n_interroge_capability_qu_une_fois() {
    let faux = FauxImap::lancer(Script::simple());
    let mut serveur = faux.connecter();

    let _ = serveur.changes_since("INBOX", 5).unwrap(); // CONDSTORE
    serveur.move_to("INBOX", 1, "Archive").unwrap(); // MOVE

    let capabilites = faux
        .commandes()
        .iter()
        .filter(|c| c.starts_with("CAPABILITY"))
        .count();
    assert_eq!(
        capabilites,
        1,
        "une CAPABILITY par session : {:?}",
        faux.commandes()
    );
}

#[test]
fn les_changements_sont_demandes_en_drapeaux_puis_en_enveloppes_par_lots() {
    let mut script = Script::simple();
    script.fetch = Box::new(|commande| {
        if commande.contains("CHANGEDSINCE") {
            (1..=501)
                .map(|uid| format!("* {uid} FETCH (UID {uid} FLAGS (\\Seen) MODSEQ (6))"))
                .collect()
        } else {
            uids_de(commande).into_iter().map(enveloppe).collect()
        }
    });
    let faux = FauxImap::lancer(script);
    let mut serveur = faux.connecter();

    let enveloppes = serveur.changes_since("INBOX", 5).unwrap().unwrap();
    assert_eq!(enveloppes.len(), 501);
    assert_eq!(enveloppes[0].subject.as_deref(), Some("Sujet 1"));

    let fetches: Vec<String> = faux
        .commandes()
        .into_iter()
        .filter(|c| c.starts_with("UID FETCH"))
        .collect();
    assert_eq!(
        fetches[0], "UID FETCH 1:* (UID FLAGS) (CHANGEDSINCE 5)",
        "les drapeaux d'abord, sans enveloppe"
    );
    assert_eq!(
        fetches.len(),
        3,
        "puis deux lots d'enveloppes : {fetches:?}"
    );
    assert!(
        fetches[1].starts_with("UID FETCH 1:500 (UID ENVELOPE"),
        "{}",
        fetches[1]
    );
    assert!(
        fetches[2].starts_with("UID FETCH 501 (UID ENVELOPE"),
        "{}",
        fetches[2]
    );
}

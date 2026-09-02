//! Un serveur IMAP FACTICE et scripté, en clair sur 127.0.0.1, pour
//! prouver ce que l'adaptateur ENVOIE (PLAN-AUDIT-V2 E3) : quels champs
//! d'en-tête il demande, combien de `LIST` et de `CAPABILITY` par
//! session, s'il ose `UID EXPUNGE` sans UIDPLUS, comment il découpe un
//! lot de corps. Il enregistre chaque commande reçue et répond selon un
//! [`Script`] ; il ne comprend rien au-delà de ce que ces tests
//! exercent — jamais un serveur, un témoin.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::{FluxBorne, ImapServer};

/// Ce que le faux serveur répond : les capacités annoncées, les lignes
/// `* LIST …`, et pour chaque `UID FETCH` les lignes de réponse (sans le
/// `OK` final), calculées depuis le texte de la commande.
pub(crate) struct Script {
    pub capacites: String,
    pub liste: Vec<String>,
    pub fetch: Repondeur,
}

/// Les lignes de réponse à un `UID FETCH`, calculées depuis la commande.
pub(crate) type Repondeur = Box<dyn Fn(&str) -> Vec<String> + Send>;

impl Script {
    pub(crate) fn simple() -> Self {
        Self {
            capacites: "IMAP4rev1 UIDPLUS MOVE CONDSTORE LIST-STATUS".to_string(),
            liste: vec![
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

pub(crate) struct FauxImap {
    port: u16,
    commandes: Arc<Mutex<Vec<String>>>,
}

/// Un littéral IMAP : `{n}` puis les octets.
pub(crate) fn litteral(texte: &str) -> String {
    format!("{{{}}}\r\n{texte}", texte.len())
}

/// Les UID qu'une commande `UID FETCH 1:3,7 (…)` désigne — `1:*` vaut
/// `1..=3`, la boîte factice annonçant 3 messages.
pub(crate) fn uids_de(commande: &str) -> Vec<u32> {
    let Some(reste) = commande
        .strip_prefix("UID FETCH ")
        .or_else(|| commande.strip_prefix("uid fetch "))
    else {
        return Vec::new();
    };
    let jeu = reste.split(' ').next().unwrap_or("");
    let mut uids = Vec::new();
    for morceau in jeu.split(',') {
        match morceau.split_once(':') {
            Some((a, b)) => {
                let a: u32 = a.parse().unwrap_or(1);
                let b: u32 = if b == "*" { 3 } else { b.parse().unwrap_or(a) };
                uids.extend(a..=b);
            }
            None => {
                if let Ok(seul) = morceau.parse::<u32>() {
                    uids.push(seul);
                }
            }
        }
    }
    uids
}

impl FauxImap {
    /// Lance le serveur pour UNE connexion ; le thread meurt avec elle.
    pub(crate) fn lancer(script: Script) -> Self {
        let ecoute = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = ecoute.local_addr().unwrap().port();
        let commandes: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let journal = Arc::clone(&commandes);
        std::thread::spawn(move || {
            let (sock, _) = ecoute.accept().unwrap();
            servir(sock, &script, &journal);
        });
        Self { port, commandes }
    }

    /// Une session authentifiée sur ce serveur, en clair, bornée à 2 s.
    pub(crate) fn connecter(&self) -> ImapServer {
        let tcp = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        tcp.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let mut client = imap::Client::new(
            Box::new(FluxBorne::new(tcp, Duration::from_secs(2))) as imap::Connection
        );
        client.read_greeting().unwrap();
        let session = client
            .login("moi", "secret")
            .map_err(|(err, _)| err)
            .unwrap();
        ImapServer::pour_test(session)
    }

    /// Les commandes reçues, étiquette retirée, dans l'ordre.
    pub(crate) fn commandes(&self) -> Vec<String> {
        self.commandes.lock().unwrap().clone()
    }
}

fn servir(mut sock: TcpStream, script: &Script, journal: &Mutex<Vec<String>>) {
    sock.write_all(b"* OK faux serveur pret\r\n").unwrap();
    let mut lecteur = BufReader::new(sock.try_clone().unwrap());
    let mut ligne = String::new();
    loop {
        ligne.clear();
        if lecteur.read_line(&mut ligne).unwrap_or(0) == 0 {
            return;
        }
        let texte = ligne.trim_end();
        let Some((tag, commande)) = texte.split_once(' ') else {
            continue;
        };
        journal.lock().unwrap().push(commande.to_string());
        let majuscule = commande.to_ascii_uppercase();
        let mut reponse = String::new();
        if majuscule.starts_with("CAPABILITY") {
            reponse.push_str(&format!("* CAPABILITY {}\r\n", script.capacites));
        } else if majuscule.starts_with("LIST") {
            for l in &script.liste {
                reponse.push_str(l);
                reponse.push_str("\r\n");
            }
        } else if majuscule.starts_with("SELECT") || majuscule.starts_with("EXAMINE") {
            reponse
                .push_str("* 3 EXISTS\r\n* OK [UIDVALIDITY 1] ok\r\n* OK [HIGHESTMODSEQ 5] ok\r\n");
        } else if majuscule.starts_with("UID FETCH") {
            for l in (script.fetch)(commande) {
                reponse.push_str(&l);
                reponse.push_str("\r\n");
            }
        } else if majuscule.starts_with("LOGOUT") {
            reponse.push_str("* BYE\r\n");
        }
        reponse.push_str(&format!("{tag} OK fait\r\n"));
        if sock.write_all(reponse.as_bytes()).is_err() {
            return;
        }
        if majuscule.starts_with("LOGOUT") {
            return;
        }
    }
}

//! Banc de mesure du CHEMIN D'OUVERTURE de Wind (spike jetable).
//!
//! Le constat à instruire : freeze et lenteurs APRÈS l'ouverture de la
//! fenêtre, jusqu'à l'état stable. Le soupçon : chaque commande Tauri
//! ouvre SA connexion (`apps/desktop/src/commands.rs` — 76 `Store::open`,
//! toutes derrière le verrou global de `hors_pompe`, ligne 4768), et
//! `Store::open` refait toute la séquence d'initialisation.
//!
//! Sous-commandes :
//!   seed <db> <n>        fabrique un décor (recopie de `seed_inbox`)
//!   open <db> <n>        N `Store::open` sur une base à jour — p50/p95
//!   ventilation <db> <n> décompose la séquence de `Store::init_with`
//!   rafale <db> <n>      12 « commandes » en série, comme au démarrage
//!   requetes <db> <n>    nav_unread_counts / page 50 / total exact
//!   colonnes <db>        contrôle : PRAGMA table_info(echos)

use std::path::Path;
use std::time::{Duration, Instant};

use chrono::{TimeZone, Utc};
use mail_core::{Envelope, Store};
use rusqlite::{Connection, OptionalExtension, params};

// ---------------------------------------------------------------------
// Le SCHEMA de production, EXTRAIT du source — jamais recopié à la main
// (une transcription approximative mesurerait autre chose que la
// production). Seul échappement présent dans les deux littéraux : \n.
// ---------------------------------------------------------------------
const STORE_RS: &str = include_str!("../../../crates/mail-core/src/store.rs");
const THREAD_RS: &str = include_str!("../../../crates/mail-core/src/thread.rs");

fn litteral(source: &str, entete: &str) -> String {
    let debut = source.find(entete).expect("littéral introuvable") + entete.len();
    let reste = &source[debut..];
    let fin = reste.find("\n\";").expect("fin de littéral introuvable");
    reste[..fin].replace("\\n", "\n")
}

fn schema_store() -> String {
    litteral(STORE_RS, "const SCHEMA: &str = \"")
}

fn schema_thread() -> String {
    litteral(THREAD_RS, "pub(crate) const SCHEMA: &str = \"")
}

/// `thread::orphans`, BASE + l'ordre — recopie littérale (thread.rs:576).
const ORPHANS: &str = "SELECT m.account_id, e.mailbox_id, e.uid,
                e.message_id, e.in_reply_to, e.refs
         FROM mailboxes m CROSS JOIN envelopes e ON e.mailbox_id = m.id
         WHERE m.threaded = 1 AND e.thread_id IS NULL ORDER BY e.mailbox_id, e.uid";

/// `thread::THREADING_VERSION` — lu au source pour ne pas dériver.
fn threading_version() -> i64 {
    let entete = "const THREADING_VERSION: i64 = ";
    let debut = THREAD_RS.find(entete).expect("THREADING_VERSION") + entete.len();
    let reste = &THREAD_RS[debut..];
    let fin = reste.find(';').expect("fin");
    reste[..fin].trim().replace('_', "").parse().expect("nombre")
}

// ---------------------------------------------------------------------
// Statistiques : p50 et p95, jamais une moyenne seule (STANDARD §9).
// ---------------------------------------------------------------------
struct Stat {
    n: usize,
    p50: f64,
    p95: f64,
    min: f64,
    max: f64,
    total: f64,
}

fn stat(mut ms: Vec<f64>) -> Stat {
    ms.sort_by(|a, b| a.partial_cmp(b).expect("pas de NaN"));
    let n = ms.len();
    let total: f64 = ms.iter().sum();
    let idx = |q: f64| -> f64 {
        if n == 0 {
            return 0.0;
        }
        let i = ((n as f64 - 1.0) * q).round() as usize;
        ms[i]
    };
    Stat {
        n,
        p50: idx(0.50),
        p95: idx(0.95),
        min: *ms.first().unwrap_or(&0.0),
        max: *ms.last().unwrap_or(&0.0),
        total,
    }
}

fn ligne(quoi: &str, s: &Stat) {
    println!(
        "{quoi:<42} n={:<4} p50={:>9.3} ms  p95={:>9.3} ms  min={:>8.3}  max={:>9.3}",
        s.n, s.p50, s.p95, s.min, s.max
    );
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

// ---------------------------------------------------------------------
// Décor : recopie de `crates/mail-core/examples/seed_inbox.rs`.
// (Recopié ici, et non lancé via `cargo run -p mail-core --example`,
// pour ne PAS toucher le target/ ni le verrou cargo du workspace —
// un autre agent y mesure au même moment.)
// ---------------------------------------------------------------------
const SENDERS: [&str; 8] = [
    "Alice Martin",
    "La Gazette",
    "GitHub",
    "Bob Dupont",
    "Newsletter Cuisine",
    "Service Client",
    "Équipe Produit",
    "Charlotte Bernard",
];
const TOPICS: [&str; 6] = [
    "Les nouveautés de la semaine",
    "Votre facture est disponible",
    "Réunion de suivi — compte rendu",
    "Promotion d'été : derniers jours",
    "Rapport hebdomadaire d'activité",
    "Confirmation de votre commande",
];
const SEED_UID_VALIDITY: u32 = 424_242;
const BATCH: usize = 1_000;

fn seed(path: &str, count: u32) -> Result<(), mail_core::Error> {
    let email = "seed@exemple.fr";
    let boite = "INBOX";
    let corps: u32 = 500;
    let timer = Instant::now();
    let mut store = Store::open(Path::new(path))?;
    let account = store.adopt_or_create_account(email, "gmail")?;
    let mailbox_id = match store.sync_state(account, boite)? {
        Some(state) => {
            store.reset_mailbox(state.mailbox_id, SEED_UID_VALIDITY)?;
            state.mailbox_id
        }
        None => store.create_mailbox(account, boite, SEED_UID_VALIDITY)?,
    };
    let mut batch = Vec::with_capacity(BATCH);
    for uid in 1..=count {
        let index = uid as usize;
        batch.push(Envelope {
            uid,
            subject: Some(format!("{} n°{uid}", TOPICS[index % TOPICS.len()])),
            sender: Some(SENDERS[(index * 7) % SENDERS.len()].to_string()),
            sender_address: Some(format!(
                "expediteur{}@exemple.fr",
                (index * 7) % SENDERS.len()
            )),
            message_id: Some(format!("<seed-{boite}-{uid}@exemple.fr>")),
            in_reply_to: (uid % 5 == 0 && uid > 1)
                .then(|| format!("<seed-{boite}-{}@exemple.fr>", uid - 1)),
            date: Utc
                .timestamp_opt(1_600_000_000 + i64::from(uid) * 60, 0)
                .single(),
            seen: uid % 3 != 0,
            flagged: uid % 7 == 0,
            to_addrs: Vec::new(),
            cc_addrs: Vec::new(),
        });
        if batch.len() == BATCH {
            store.upsert_envelopes(mailbox_id, &batch)?;
            batch.clear();
        }
    }
    if !batch.is_empty() {
        store.upsert_envelopes(mailbox_id, &batch)?;
    }
    let body_from = count.saturating_sub(corps) + 1;
    for uid in body_from..=count {
        let attachments: Vec<mail_core::Attachment> = if uid % 10 == 0 {
            vec![mail_core::Attachment {
                index: 0,
                name: format!("facture-{uid}.pdf"),
                mime: "application/pdf".to_string(),
                size: 20_480,
            }]
        } else {
            Vec::new()
        };
        let html = format!("<p>Corps du message n°{uid} : contenu de démonstration.</p>");
        store.save_body(mailbox_id, uid, &html, &attachments)?;
    }
    let dossiers = vec![
        mail_core::Folder {
            wire: "Archiv&AOk-s".to_string(),
            display: "Archivés".to_string(),
            selectable: true,
        },
        mail_core::Folder {
            wire: "Factures".to_string(),
            display: "Factures".to_string(),
            selectable: true,
        },
    ];
    store.replace_folders(account, &dossiers)?;
    store.update_state(mailbox_id, count, None)?;
    println!(
        "{count} enveloppes écrites dans {path} en {:?}",
        timer.elapsed()
    );
    Ok(())
}

// ---------------------------------------------------------------------
// 1. Le coût brut d'un Store::open sur une base À JOUR.
// ---------------------------------------------------------------------
fn mesure_open(path: &str, n: usize) -> Result<(), mail_core::Error> {
    let mut chauffe = Vec::new();
    for _ in 0..20 {
        let t = Instant::now();
        let store = Store::open(Path::new(path))?;
        chauffe.push(ms(t.elapsed()));
        drop(store);
    }
    println!(
        "  (chauffe : 1re ouverture {:.3} ms, 20e {:.3} ms)",
        chauffe[0], chauffe[19]
    );
    let mut v = Vec::with_capacity(n);
    for _ in 0..n {
        let t = Instant::now();
        let store = Store::open(Path::new(path))?;
        v.push(ms(t.elapsed()));
        drop(store);
    }
    let s = stat(v);
    ligne("Store::open (total)", &s);
    println!("  cumul {n} ouvertures : {:.1} ms", s.total);
    Ok(())
}

// ---------------------------------------------------------------------
// 2. La ventilation : la séquence de `Store::init_with` (store.rs:735),
//    refaite à la main avec rusqlite, étape par étape.
// ---------------------------------------------------------------------
struct Vent {
    open: Vec<f64>,
    busy: Vec<f64>,
    wal: Vec<f64>,
    schema: Vec<f64>,
    migrate: Vec<f64>,
    fils: Vec<f64>,
    corresp: Vec<f64>,
    somme: Vec<f64>,
}

fn etape_migrate(conn: &Connection) -> Result<(), rusqlite::Error> {
    // `migrate` (store.rs:2532) sur une base À JOUR : aucun ALTER ne part,
    // le coût est celui des SONDES. Recopie fidèle de la liste d'appels.
    let colonnes = |table: &str| -> Result<(), rusqlite::Error> {
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let _: Vec<String> = stmt
            .query_map([], |row| row.get(1))?
            .collect::<Result<_, _>>()?;
        Ok(())
    };
    colonnes("mailboxes")?; // migrate_multi_account
    for t in [
        "drafts",
        "mailboxes",
        "accounts",
        "mailboxes",
        "mailboxes",
        "outbox",
        "envelopes",
        "drafts",
        "outbox",
        "bodies",
        "echos",
        "drafts",
        "outbox",
        "outbox",
        "invitations",
        "bodies",
    ] {
        colonnes(t)?;
    }
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_bodies_apercu_manquant
             ON bodies(mailbox_id, uid) WHERE preview IS NULL;",
    )?;
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_envelopes_message
             ON envelopes(message_id) WHERE message_id IS NOT NULL;",
    )?;
    conn.execute_batch("CREATE TABLE IF NOT EXISTS reparations (nom TEXT PRIMARY KEY);")?;
    for marque in [
        "apercus-entites",
        "corps-fffd",
        "pieces-calendrier",
        "objets-escapes",
    ] {
        let _: bool = conn
            .prepare(&format!("SELECT 1 FROM reparations WHERE nom = '{marque}'"))?
            .exists([])?;
    }
    colonnes("accounts")?;
    // search::migrate_search — la sonde du marqueur `recipients`.
    let _: Option<String> = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'search_fts'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_envelopes_thread
             ON envelopes(thread_id, date_epoch DESC);",
    )?;
    Ok(())
}

fn etape_fils(
    conn: &Connection,
    schema: &str,
    version_cible: i64,
) -> Result<usize, rusqlite::Error> {
    // Le bloc BEGIN..COMMIT de `init_with` (store.rs, autour de la ligne 800).
    conn.execute_batch("BEGIN")?;
    let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version < version_cible {
        conn.execute_batch("DROP TABLE IF EXISTS thread_links; DROP TABLE IF EXISTS threads;")?;
    }
    conn.execute_batch(schema)?;
    let _: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let orphelins: usize = {
        let mut stmt = conn.prepare(ORPHANS)?;
        let rows: Vec<i64> = stmt
            .query_map([], |row| row.get(1))?
            .collect::<Result<_, _>>()?;
        rows.len()
    };
    conn.execute_batch("COMMIT")?;
    Ok(orphelins)
}

fn ventilation(path: &str, n: usize) -> Result<(), rusqlite::Error> {
    let schema = schema_store();
    let sfils = schema_thread();
    let cible = threading_version();
    let mut v = Vent {
        open: Vec::new(),
        busy: Vec::new(),
        wal: Vec::new(),
        schema: Vec::new(),
        migrate: Vec::new(),
        fils: Vec::new(),
        corresp: Vec::new(),
        somme: Vec::new(),
    };
    let mut orphelins = 0usize;
    for _ in 0..10 {
        let conn = Connection::open(path)?;
        conn.busy_timeout(Duration::from_secs(30))?;
        conn.query_row("PRAGMA journal_mode = wal", [], |row| {
            row.get::<_, String>(0)
        })?;
        conn.execute_batch(&schema)?;
        etape_migrate(&conn)?;
        etape_fils(&conn, &sfils, cible)?;
    }
    for _ in 0..n {
        let t0 = Instant::now();
        let conn = Connection::open(path)?;
        let t1 = Instant::now();
        conn.busy_timeout(Duration::from_secs(30))?;
        let t2 = Instant::now();
        conn.query_row("PRAGMA journal_mode = wal", [], |row| {
            row.get::<_, String>(0)
        })?;
        let t3 = Instant::now();
        conn.execute_batch(&schema)?;
        let t4 = Instant::now();
        etape_migrate(&conn)?;
        let t5 = Instant::now();
        orphelins = etape_fils(&conn, &sfils, cible)?;
        let t6 = Instant::now();
        let _: Option<String> = conn
            .query_row(
                "SELECT value FROM prefs WHERE key = ?1",
                params!["annuaire_correspondants_v1"],
                |row| row.get(0),
            )
            .optional()?;
        let t7 = Instant::now();
        v.open.push(ms(t1 - t0));
        v.busy.push(ms(t2 - t1));
        v.wal.push(ms(t3 - t2));
        v.schema.push(ms(t4 - t3));
        v.migrate.push(ms(t5 - t4));
        v.fils.push(ms(t6 - t5));
        v.corresp.push(ms(t7 - t6));
        v.somme.push(ms(t7 - t0));
    }
    println!("  (orphelins rendus par la requête d'adoption : {orphelins})");
    ligne("1. Connection::open", &stat(v.open));
    ligne("2. busy_timeout(30 s)", &stat(v.busy));
    ligne("3. PRAGMA journal_mode = wal", &stat(v.wal));
    ligne("4. execute_batch(SCHEMA)", &stat(v.schema));
    ligne("5. migrate() [sondes, base a jour]", &stat(v.migrate));
    ligne("6. BEGIN..fils..COMMIT", &stat(v.fils));
    ligne("7. rattraper_correspondants()", &stat(v.corresp));
    ligne("SOMME des 7 etapes", &stat(v.somme));
    Ok(())
}

// ---------------------------------------------------------------------
// 4. La rafale sérialisée : ce que le verrou global impose au démarrage.
// ---------------------------------------------------------------------
fn rafale(path: &str, tours: usize) -> Result<(), mail_core::Error> {
    for _ in 0..3 {
        let _ = un_tour(path)?;
    }
    let mut totaux = Vec::new();
    let mut detail: Vec<(String, Vec<f64>)> = Vec::new();
    for tour in 0..tours {
        let (total, pas) = un_tour(path)?;
        totaux.push(total);
        if tour == 0 {
            for (nom, _) in &pas {
                detail.push((nom.clone(), Vec::new()));
            }
        }
        for (i, (_, cout)) in pas.iter().enumerate() {
            detail[i].1.push(*cout);
        }
    }
    for (nom, couts) in detail {
        ligne(&format!("  commande {nom}"), &stat(couts));
    }
    let s = stat(totaux);
    ligne("RAFALE COMPLETE (12 commandes en serie)", &s);
    Ok(())
}

/// Douze « commandes » qui ouvrent chacune leur `Store` puis font leur
/// requête — le squelette de `hors_pompe` + `Store::open` de
/// `apps/desktop/src/commands.rs`, sans Tauri.
fn un_tour(path: &str) -> Result<(f64, Vec<(String, f64)>), mail_core::Error> {
    let p = Path::new(path);
    let mut pas: Vec<(String, f64)> = Vec::new();
    let t0 = Instant::now();

    macro_rules! chrono {
        ($nom:expr, $corps:block) => {{
            let t = Instant::now();
            $corps
            pas.push(($nom.to_string(), ms(t.elapsed())));
        }};
    }

    chrono!("accounts", {
        let s = Store::open(p)?;
        let _ = s.accounts()?;
    });
    chrono!("text_pref(langue)", {
        let s = Store::open(p)?;
        let _ = s.text_pref("langue")?;
    });
    chrono!("canonical_folders", {
        let s = Store::open(p)?;
        let _ = s.canonical_folders(1)?;
    });
    chrono!("nav_unread_counts", {
        let s = Store::open(p)?;
        let d = s.canonical_folders(1)?;
        let _ = s.nav_unread_counts(1, &d)?;
    });
    chrono!("unified_count_scoped", {
        let s = Store::open(p)?;
        let _ = s.unified_count_scoped(None, false)?;
    });
    chrono!("unified_recent_scoped(50)", {
        let s = Store::open(p)?;
        let _ = s.unified_recent_scoped(None, false, 0, 50)?;
    });
    for i in 0..6 {
        chrono!(format!("pref legere #{i}"), {
            let s = Store::open(p)?;
            let _ = s.bool_pref("volet_lecture", false)?;
        });
    }
    let total = ms(t0.elapsed());
    Ok((total, pas))
}

// ---------------------------------------------------------------------
// 5. Le coût des requêtes elles-mêmes, connexion DÉJÀ ouverte.
// ---------------------------------------------------------------------
fn requetes(path: &str, n: usize) -> Result<(), mail_core::Error> {
    let store = Store::open(Path::new(path))?;
    let dossiers = store.canonical_folders(1)?;
    let mut a = Vec::new();
    let mut b = Vec::new();
    let mut c = Vec::new();
    for _ in 0..5 {
        let _ = store.nav_unread_counts(1, &dossiers)?;
        let _ = store.unified_recent_scoped(None, false, 0, 50)?;
        let _ = store.unified_count_scoped(None, false)?;
    }
    let mut total = 0u64;
    let mut page = 0usize;
    for _ in 0..n {
        let t = Instant::now();
        let _ = store.nav_unread_counts(1, &dossiers)?;
        a.push(ms(t.elapsed()));
        let t = Instant::now();
        page = store.unified_recent_scoped(None, false, 0, 50)?.len();
        b.push(ms(t.elapsed()));
        let t = Instant::now();
        total = store.unified_count_scoped(None, false)?;
        c.push(ms(t.elapsed()));
    }
    println!("  (page rendue : {page} lignes ; total exact : {total} fils)");
    ligne("nav_unread_counts (hors ouverture)", &stat(a));
    ligne("unified_recent_scoped(0,50)", &stat(b));
    ligne("unified_count_scoped (total exact)", &stat(c));
    Ok(())
}

/// Le PLAFOND du gain : les MÊMES douze requêtes, sur UN `Store` ouvert
/// une seule fois. Ce n'est pas une proposition d'architecture — c'est la
/// borne qui dit combien des millisecondes de la rafale sont de
/// l'ouverture et combien sont du travail utile.
fn rafale_une_connexion(path: &str, tours: usize) -> Result<(), mail_core::Error> {
    let store = Store::open(Path::new(path))?;
    let travail = |s: &Store| -> Result<(), mail_core::Error> {
        let _ = s.accounts()?;
        let _ = s.text_pref("langue")?;
        let d = s.canonical_folders(1)?;
        let _ = s.canonical_folders(1)?;
        let _ = s.nav_unread_counts(1, &d)?;
        let _ = s.unified_count_scoped(None, false)?;
        let _ = s.unified_recent_scoped(None, false, 0, 50)?;
        for _ in 0..6 {
            let _ = s.bool_pref("volet_lecture", false)?;
        }
        Ok(())
    };
    for _ in 0..3 {
        travail(&store)?;
    }
    let mut v = Vec::new();
    for _ in 0..tours {
        let t = Instant::now();
        travail(&store)?;
        v.push(ms(t.elapsed()));
    }
    ligne("RAFALE sur UNE connexion (plafond)", &stat(v));
    Ok(())
}

/// Le détail DANS l'étape 6 : qui, du schéma des fils ou de la requête
/// d'adoption, paie les millisecondes ? Plus le plan de la requête.
fn fils_detail(path: &str, n: usize) -> Result<(), rusqlite::Error> {
    let sfils = schema_thread();
    let cible = threading_version();
    let conn = Connection::open(path)?;
    conn.busy_timeout(Duration::from_secs(30))?;
    conn.query_row("PRAGMA journal_mode = wal", [], |row| {
        row.get::<_, String>(0)
    })?;
    println!("  plan de la requête d'adoption (thread::orphans) :");
    {
        let mut stmt = conn.prepare(&format!("EXPLAIN QUERY PLAN {ORPHANS}"))?;
        let lignes: Vec<String> = stmt
            .query_map([], |row| row.get(3))?
            .collect::<Result<_, _>>()?;
        for l in lignes {
            println!("    {l}");
        }
    }
    let (mut a, mut b, mut c, mut d, mut e) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for tour in 0..(n + 10) {
        let t0 = Instant::now();
        conn.execute_batch("BEGIN")?;
        let t1 = Instant::now();
        let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version < cible {
            conn.execute_batch(
                "DROP TABLE IF EXISTS thread_links; DROP TABLE IF EXISTS threads;",
            )?;
        }
        let t2 = Instant::now();
        conn.execute_batch(&sfils)?;
        let t3 = Instant::now();
        {
            let mut stmt = conn.prepare(ORPHANS)?;
            let rows: Vec<i64> = stmt
                .query_map([], |row| row.get(1))?
                .collect::<Result<_, _>>()?;
            let _ = rows.len();
        }
        let t4 = Instant::now();
        conn.execute_batch("COMMIT")?;
        let t5 = Instant::now();
        if tour >= 10 {
            a.push(ms(t1 - t0));
            b.push(ms(t2 - t1));
            c.push(ms(t3 - t2));
            d.push(ms(t4 - t3));
            e.push(ms(t5 - t4));
        }
    }
    ligne("6a. BEGIN", &stat(a));
    ligne("6b. PRAGMA user_version (drop_if_outdated)", &stat(b));
    ligne("6c. execute_batch(thread::SCHEMA)", &stat(c));
    ligne("6d. requete d'adoption (orphans)", &stat(d));
    ligne("6e. COMMIT", &stat(e));
    Ok(())
}

fn colonnes_echos(path: &str) -> Result<(), rusqlite::Error> {
    let conn = Connection::open(path)?;
    let mut stmt = conn.prepare("PRAGMA table_info(echos)")?;
    let cols: Vec<(String, String)> = stmt
        .query_map([], |row| Ok((row.get(1)?, row.get(2)?)))?
        .collect::<Result<_, _>>()?;
    for (nom, typ) in cols {
        println!("  colonne {nom:?} type {typ:?}");
    }
    Ok(())
}

/// Le CHEMIN RÉEL de `backfill_status` : `pending_total`
/// (`commands.rs`) fait `accounts()`, puis `mailbox_names()` par compte,
/// puis `bodies_pending_count()` par boîte — soit 64 allers-retours sur
/// la base du terrain.
///
/// Mesuré ICI, hors de l'application et machine au repos, pour trancher
/// une question que le banc en situation ne peut pas trancher : les
/// 527-670 ms relevés dans l'application (jalons `mesure::pending_total`,
/// 2026-08-26) contre 0,113 s pour la même boucle en SQL direct — est-ce
/// le CODE, ou la CONTENTION de la synchro qui tourne en même temps ?
fn pending(path: &str, n: usize) -> Result<(), mail_core::Error> {
    let store = Store::open(Path::new(path))?;
    for _ in 0..3 {
        for a in store.accounts()? {
            for b in store.mailbox_names(a.id)? {
                let _ = store.bodies_pending_count(a.id, &b, mail_core::NO_HORIZON)?;
            }
        }
    }
    let (mut tout, mut comptes, mut noms, mut sondes) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let (mut boites, mut total) = (0usize, 0u64);
    for _ in 0..n {
        let t0 = Instant::now();
        let t = Instant::now();
        let acs = store.accounts()?;
        comptes.push(ms(t.elapsed()));
        let (mut somme, mut nb) = (0u64, 0usize);
        for a in &acs {
            let t = Instant::now();
            let bs = store.mailbox_names(a.id)?;
            noms.push(ms(t.elapsed()));
            for b in &bs {
                let t = Instant::now();
                somme += store.bodies_pending_count(a.id, b, mail_core::NO_HORIZON)?;
                sondes.push(ms(t.elapsed()));
                nb += 1;
            }
        }
        tout.push(ms(t0.elapsed()));
        boites = nb;
        total = somme;
    }
    println!("  ({boites} boites parcourues, {total} corps manquants)");
    ligne("pending_total ENTIER (chemin de prod)", &stat(tout));
    ligne("  dont accounts()", &stat(comptes));
    ligne("  dont mailbox_names() (par compte)", &stat(noms));
    ligne("  dont bodies_pending_count (par boite)", &stat(sondes));
    Ok(())
}

/// Le CANDIDAT : le même nombre, en UNE requête au lieu de 64. Ce n'est
/// pas une proposition d'architecture — c'est la borne qui dit ce que le
/// regroupement rapporterait, AVANT de l'écrire en production.
fn pending_une_requete(path: &str, n: usize) -> Result<(), mail_core::Error> {
    let conn = rusqlite::Connection::open_with_flags(
        Path::new(path),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    // Même portée que la boucle : les comptes d'`accounts()` (email non
    // vide), toutes leurs boîtes, aucun horizon.
    const SQL: &str = "SELECT COUNT(*)
         FROM envelopes e
         JOIN mailboxes m ON m.id = e.mailbox_id
         JOIN accounts a ON a.id = m.account_id
         WHERE a.email != ''
           AND NOT EXISTS (
               SELECT 1 FROM bodies b
                WHERE b.mailbox_id = e.mailbox_id AND b.uid = e.uid
           )";
    for _ in 0..3 {
        let _: i64 = conn.query_row(SQL, [], |r| r.get(0))?;
    }
    let mut v = Vec::new();
    let mut total = 0i64;
    for _ in 0..n {
        let t = Instant::now();
        total = conn.query_row(SQL, [], |r| r.get(0))?;
        v.push(ms(t.elapsed()));
    }
    println!("  ({total} corps manquants — doit EGALER la boucle, sinon la portee differe)");
    ligne("pending en UNE requete", &stat(v));
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let sous = args.get(1).map(String::as_str).unwrap_or("");
    let path = args.get(2).map(String::as_str).unwrap_or("base.db");
    let n: usize = args.get(3).and_then(|v| v.parse().ok()).unwrap_or(200);
    let r: Result<(), String> = match sous {
        "seed" => seed(path, n as u32).map_err(|e| e.to_string()),
        "open" => mesure_open(path, n).map_err(|e| e.to_string()),
        "ventilation" => ventilation(path, n).map_err(|e| e.to_string()),
        "rafale" => rafale(path, n).map_err(|e| e.to_string()),
        "requetes" => requetes(path, n).map_err(|e| e.to_string()),
        "rafale1" => rafale_une_connexion(path, n).map_err(|e| e.to_string()),
        "fils" => fils_detail(path, n).map_err(|e| e.to_string()),
        "colonnes" => colonnes_echos(path).map_err(|e| e.to_string()),
        "pending" => pending(path, n).map_err(|e| e.to_string()),
        "pending1" => pending_une_requete(path, n).map_err(|e| e.to_string()),
        autre => Err(format!("sous-commande inconnue : {autre:?}")),
    };
    if let Err(err) = r {
        eprintln!("ECHEC : {err}");
        std::process::exit(1);
    }
}

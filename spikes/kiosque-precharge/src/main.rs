//! Banc S3 — coût du préchargement des corps du Kiosque, borné à la
//! page servie (décision CE D5, PLAN-MODE-ORGANISE).
//!
//! Base synthétique au schéma de prod (store.rs / thread.rs) :
//! 200 000 enveloppes, dont 2 000 « lettres d'information » à corps
//! HTML 30-150 Ko, et ~20 % des messages ordinaires avec un petit
//! corps (2-8 Ko) pour que la base ait la géométrie d'une vraie.
//!
//! Mesures (20 itérations, médiane/p95, froid vs chaud) :
//!   1. lot par (mailbox_id, uid) IN (VALUES ...) — page 20 et 50 ;
//!   2. lot par e.thread_id IN (...) JOIN bodies — patron enrichir_lignes ;
//!   3. corps unitaire par le chemin de Store::body (jointure mailboxes) ;
//!   4. EXPLAIN QUERY PLAN de chaque lecture.
//!
//! « Froid » = purge du cache fichier Windows (ouverture
//! FILE_FLAG_NO_BUFFERING puis fermeture) + connexion neuve à chaque
//! itération. Ce n'est PAS un froid post-redémarrage (STANDARD §9) :
//! c'est un froid « pages du fichier évincées », le pire cas d'une
//! session déjà lancée. La validité de la purge se lit dans l'écart
//! froid/chaud : s'ils se confondent, la purge a échoué.

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::Instant;

const N_ENVELOPES: i64 = 200_000;
const N_NEWSLETTERS: i64 = 2_000;
const N_MAILBOXES: i64 = 4;
const ITERATIONS: usize = 20;

// ---------------------------------------------------------------- PRNG
struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed.max(1))
    }
    fn next(&mut self) -> u64 {
        // xorshift64*
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }
    fn range(&mut self, lo: u64, hi: u64) -> u64 {
        lo + self.next() % (hi - lo)
    }
}

const WORDS: &[&str] = &[
    "offre", "semaine", "nouveau", "produit", "lecture", "article", "detail", "prix",
    "livraison", "gratuite", "decouvrir", "collection", "edition", "limitee", "abonnement",
    "exclusif", "reduction", "valable", "aujourd", "demain", "boutique", "catalogue",
    "selection", "conseil", "recette", "voyage", "photo", "reportage", "analyse", "marche",
];

fn paragraphe(rng: &mut Rng, mots: usize) -> String {
    let mut s = String::with_capacity(mots * 8);
    for i in 0..mots {
        if i > 0 {
            s.push(' ');
        }
        s.push_str(WORDS[(rng.next() as usize) % WORDS.len()]);
    }
    s
}

/// HTML de lettre d'information réaliste : tables imbriquées, styles
/// inline, images distantes — la géométrie du courrier marchand.
fn html_newsletter(rng: &mut Rng, cible: usize) -> String {
    let mut h = String::with_capacity(cible + 4096);
    h.push_str("<!DOCTYPE html><html><head><meta charset=\"utf-8\"><style>.b{font-family:Arial,sans-serif;color:#333}.t{width:600px;margin:0 auto}</style></head><body class=\"b\"><table class=\"t\" cellpadding=\"0\" cellspacing=\"0\">");
    let mut bloc = 0u64;
    while h.len() < cible {
        bloc += 1;
        let pad = rng.range(8, 32);
        let couleur = rng.next() & 0xFFFFFF;
        let img = rng.next();
        let titre = paragraphe(rng, 6);
        let mots = rng.range(40, 160) as usize;
        let texte = paragraphe(rng, mots);
        let bouton = paragraphe(rng, 3);
        h.push_str(&format!(
            "<tr><td style=\"padding:{}px 24px;background:#{:06x}\"><img src=\"https://img.example.com/{}.png\" width=\"552\" alt=\"\"><h2 style=\"font-size:20px;margin:12px 0\">{}</h2><p style=\"line-height:1.5\">{}</p><a href=\"https://example.com/c/{}\" style=\"display:inline-block;padding:10px 18px;background:#AD204C;color:#fff;text-decoration:none\">{}</a></td></tr>",
            pad, couleur, img, titre, texte, bloc, bouton,
        ));
    }
    h.push_str("</table></body></html>");
    h
}

fn html_ordinaire(rng: &mut Rng, cible: usize) -> String {
    let mut h = String::with_capacity(cible + 512);
    h.push_str("<html><body><div dir=\"ltr\">");
    while h.len() < cible {
        let mots = rng.range(20, 80) as usize;
        h.push_str(&format!("<p>{}</p>", paragraphe(rng, mots)));
    }
    h.push_str("</div></body></html>");
    h
}

// ---------------------------------------------------------------- seed
/// Schéma copié de crates/mail-core/src/store.rs et thread.rs —
/// colonnes et index à l'identique pour les tables traversées.
const SCHEMA: &str = "
PRAGMA foreign_keys = ON;
CREATE TABLE accounts (
    id INTEGER PRIMARY KEY, email TEXT NOT NULL UNIQUE
);
CREATE TABLE mailboxes (
    id INTEGER PRIMARY KEY,
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    uidvalidity INTEGER NOT NULL DEFAULT 0,
    threaded INTEGER NOT NULL DEFAULT 1,
    remote_total INTEGER NOT NULL DEFAULT 0,
    UNIQUE (account_id, name)
);
CREATE TABLE envelopes (
    mailbox_id INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    uid INTEGER NOT NULL,
    subject TEXT, sender TEXT, sender_address TEXT,
    to_addrs TEXT, cc_addrs TEXT,
    message_id TEXT, in_reply_to TEXT, refs TEXT,
    thread_id INTEGER, date_epoch INTEGER,
    seen INTEGER NOT NULL DEFAULT 0,
    flagged INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (mailbox_id, uid)
);
CREATE INDEX idx_envelopes_date ON envelopes(mailbox_id, date_epoch DESC, uid);
CREATE INDEX idx_envelopes_thread ON envelopes(thread_id, date_epoch DESC);
CREATE TABLE bodies (
    mailbox_id INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    uid INTEGER NOT NULL,
    html TEXT NOT NULL,
    scanned INTEGER NOT NULL DEFAULT 0,
    preview TEXT,
    PRIMARY KEY (mailbox_id, uid)
);
CREATE TABLE threads (
    id INTEGER PRIMARY KEY,
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    last_mailbox_id INTEGER,
    last_uid INTEGER NOT NULL DEFAULT 0,
    last_epoch INTEGER,
    size INTEGER NOT NULL DEFAULT 0,
    unseen INTEGER NOT NULL DEFAULT 0,
    inbox_size INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_threads_date ON threads(account_id, last_epoch DESC, last_uid DESC);
CREATE INDEX idx_threads_date_globale ON threads(last_epoch DESC, last_uid DESC, account_id)
    WHERE inbox_size > 0;
";

fn seed(path: &Path) -> Result<()> {
    let mut conn = Connection::open(path)?;
    conn.query_row("PRAGMA journal_mode = wal", [], |_| Ok(()))?;
    conn.execute_batch(SCHEMA)?;
    let mut rng = Rng::new(0xC0FFEE);
    let t0 = Instant::now();
    let tx = conn.transaction()?;
    tx.execute("INSERT INTO accounts (id, email) VALUES (1, 'banc@example.com')", [])?;
    for m in 1..=N_MAILBOXES {
        tx.execute(
            "INSERT INTO mailboxes (id, account_id, name) VALUES (?1, 1, ?2)",
            params![m, format!("BOITE-{m}")],
        )?;
    }
    {
        let mut env = tx.prepare(
            "INSERT INTO envelopes (mailbox_id, uid, subject, sender, sender_address,
                 message_id, thread_id, date_epoch, seen)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1)",
        )?;
        let mut fil = tx.prepare(
            "INSERT INTO threads (id, account_id, last_mailbox_id, last_uid, last_epoch,
                 size, unseen, inbox_size)
             VALUES (?1, 1, ?2, ?3, ?4, 1, 0, 1)",
        )?;
        let mut corps = tx.prepare(
            "INSERT INTO bodies (mailbox_id, uid, html, scanned, preview)
             VALUES (?1, ?2, ?3, 1, ?4)",
        )?;
        // Époques décroissantes sur ~2 ans ; 1 lettre toutes les 100
        // enveloppes → les lettres sont dispersées dans tout le fichier.
        let base_epoch: i64 = 1_756_000_000;
        for i in 0..N_ENVELOPES {
            let mailbox_id = 1 + (rng.next() as i64).rem_euclid(N_MAILBOXES);
            let uid = i + 1; // unique par boîte a fortiori
            let epoch = base_epoch - i * 300 - (rng.range(0, 200) as i64);
            let thread_id = i + 1;
            let lettre = i % 100 == 0 && i / 100 < N_NEWSLETTERS;
            let (sender, addr) = if lettre {
                let n = rng.range(0, 40);
                (format!("Lettre {n}"), format!("news{n}@lettres.example.com"))
            } else {
                let n = rng.range(0, 5000);
                (format!("Contact {n}"), format!("c{n}@example.com"))
            };
            env.execute(params![
                mailbox_id,
                uid,
                format!("Sujet {i} {}", paragraphe(&mut rng, 4)),
                sender,
                addr,
                format!("<m{i}@example.com>"),
                thread_id,
                epoch,
            ])?;
            fil.execute(params![thread_id, mailbox_id, uid, epoch])?;
            if lettre {
                let cible = rng.range(30_000, 150_000) as usize;
                let html = html_newsletter(&mut rng, cible);
                corps.execute(params![mailbox_id, uid, html, &paragraphe(&mut rng, 20)])?;
            } else if rng.range(0, 100) < 20 {
                let cible = rng.range(2_000, 8_000) as usize;
                let html = html_ordinaire(&mut rng, cible);
                corps.execute(params![mailbox_id, uid, html, &paragraphe(&mut rng, 20)])?;
            }
            if i % 20_000 == 0 {
                eprint!("\rseed {i}/{N_ENVELOPES}");
            }
        }
    }
    tx.commit()?;
    conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))?;
    eprintln!("\rseed terminé en {:.1} s", t0.elapsed().as_secs_f64());
    Ok(())
}

// ------------------------------------------------------------- froid
#[cfg(windows)]
fn purge_cache_fichier(path: &Path) {
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_FLAG_NO_BUFFERING: u32 = 0x2000_0000;
    for suffixe in ["", "-wal", "-shm"] {
        let p = PathBuf::from(format!("{}{}", path.display(), suffixe));
        if p.exists() {
            let _ = std::fs::OpenOptions::new()
                .read(true)
                .custom_flags(FILE_FLAG_NO_BUFFERING)
                .open(&p);
        }
    }
}

// ------------------------------------------------------------- stats
fn stats(mut ms: Vec<f64>) -> (f64, f64, f64, f64) {
    ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = ms.len();
    let med = if n % 2 == 1 {
        ms[n / 2]
    } else {
        (ms[n / 2 - 1] + ms[n / 2]) / 2.0
    };
    let p95 = ms[((n as f64 * 0.95).ceil() as usize - 1).min(n - 1)];
    (med, p95, ms[0], ms[n - 1])
}

/// La page du Kiosque : les N lettres les plus récentes —
/// (thread_id, mailbox_id, uid), triées par date décroissante.
fn page_kiosque(conn: &Connection, n: usize) -> Result<Vec<(i64, i64, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT e.thread_id, e.mailbox_id, e.uid
           FROM envelopes e
          WHERE e.sender_address LIKE '%@lettres.example.com'
          ORDER BY e.date_epoch DESC LIMIT ?1",
    )?;
    let rows = stmt
        .query_map([n as i64], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn sql_lot_pk(n: usize) -> String {
    let vals = vec!["(?,?)"; n].join(",");
    format!(
        "SELECT mailbox_id, uid, html FROM bodies WHERE (mailbox_id, uid) IN (VALUES {vals})"
    )
}

fn sql_lot_thread(n: usize) -> String {
    let trous = vec!["?"; n].join(",");
    format!(
        "SELECT b.mailbox_id, b.uid, b.html
           FROM envelopes e
           JOIN bodies b ON b.mailbox_id = e.mailbox_id AND b.uid = e.uid
          WHERE e.thread_id IN ({trous})"
    )
}

const SQL_UNITAIRE: &str = "SELECT b.html FROM bodies b JOIN mailboxes m ON m.id = b.mailbox_id
     WHERE m.account_id = ?1 AND m.name = ?2 AND b.uid = ?3";

fn run_lot_pk(conn: &Connection, page: &[(i64, i64, i64)]) -> Result<(f64, u64)> {
    let sql = sql_lot_pk(page.len());
    let mut stmt = conn.prepare(&sql)?;
    let mut flat: Vec<i64> = Vec::with_capacity(page.len() * 2);
    for &(_, m, u) in page {
        flat.push(m);
        flat.push(u);
    }
    let t = Instant::now();
    let mut octets = 0u64;
    let mut rows = stmt.query(rusqlite::params_from_iter(flat.iter()))?;
    let mut n = 0;
    while let Some(row) = rows.next()? {
        let html: String = row.get(2)?;
        octets += html.len() as u64;
        n += 1;
    }
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    anyhow::ensure!(n == page.len(), "lot pk : {n} corps pour {} attendus", page.len());
    Ok((ms, octets))
}

fn run_lot_thread(conn: &Connection, page: &[(i64, i64, i64)]) -> Result<(f64, u64)> {
    let sql = sql_lot_thread(page.len());
    let mut stmt = conn.prepare(&sql)?;
    let fils: Vec<i64> = page.iter().map(|&(t, _, _)| t).collect();
    let t = Instant::now();
    let mut octets = 0u64;
    let mut rows = stmt.query(rusqlite::params_from_iter(fils.iter()))?;
    let mut n = 0;
    while let Some(row) = rows.next()? {
        let html: String = row.get(2)?;
        octets += html.len() as u64;
        n += 1;
    }
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    anyhow::ensure!(n == page.len(), "lot fil : {n} corps pour {} attendus", page.len());
    Ok((ms, octets))
}

fn run_unitaire(conn: &Connection, mailbox: &str, uid: i64) -> Result<(f64, u64)> {
    let mut stmt = conn.prepare(SQL_UNITAIRE)?;
    let t = Instant::now();
    let html: String = stmt.query_row(params![1i64, mailbox, uid], |r| r.get(0))?;
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    Ok((ms, html.len() as u64))
}

fn explain(conn: &Connection, titre: &str, sql: &str, n_params: usize) -> Result<()> {
    println!("\nEXPLAIN QUERY PLAN — {titre}");
    let mut stmt = conn.prepare(&format!("EXPLAIN QUERY PLAN {sql}"))?;
    let params: Vec<i64> = (0..n_params as i64).collect();
    let mut rows = stmt.query(rusqlite::params_from_iter(params.iter()))?;
    while let Some(row) = rows.next()? {
        let detail: String = row.get(3)?;
        println!("  {detail}");
    }
    Ok(())
}

// -------------------------------------------------------------- main
fn main() -> Result<()> {
    let db: PathBuf = std::env::args()
        .nth(1)
        .context("usage: spike-kiosque-precharge <chemin_base.db>")?
        .into();
    if !db.exists() {
        eprintln!("base absente — seed de {N_ENVELOPES} enveloppes…");
        seed(&db)?;
    }
    let taille = std::fs::metadata(&db)?.len();
    println!("base : {} — {:.1} Mo", db.display(), taille as f64 / 1e6);

    let conn = Connection::open(&db)?;
    let page50 = page_kiosque(&conn, 50)?;
    let page20: Vec<_> = page50.iter().take(20).cloned().collect();
    println!("page Kiosque : {} lettres (top 50 par date)", page50.len());

    // EXPLAIN QUERY PLAN (une fois, connexion chaude — le plan ne
    // dépend pas du cache).
    explain(&conn, "lot (mailbox_id, uid) IN (VALUES ...), page 20", &sql_lot_pk(20), 40)?;
    explain(&conn, "lot e.thread_id IN (...) JOIN bodies, page 20", &sql_lot_thread(20), 20)?;
    println!("\nEXPLAIN QUERY PLAN — unitaire (chemin Store::body)");
    {
        let mut stmt = conn.prepare(&format!("EXPLAIN QUERY PLAN {SQL_UNITAIRE}"))?;
        let mut rows = stmt.query(params![1i64, "BOITE-1", 1i64])?;
        while let Some(row) = rows.next()? {
            let detail: String = row.get(3)?;
            println!("  {detail}");
        }
    }
    drop(conn);

    // Corps unitaires témoins : 20 lettres distinctes (viewport).
    let conn = Connection::open(&db)?;
    let unitaires: Vec<(String, i64)> = page50
        .iter()
        .take(20)
        .map(|&(_, m, u)| (format!("BOITE-{m}"), u))
        .collect();
    drop(conn);

    struct Banc<'a> {
        nom: &'a str,
        run: Box<dyn Fn(&Connection) -> Result<(f64, u64)> + 'a>,
    }
    let bancs: Vec<Banc> = vec![
        Banc { nom: "lot PK page 20", run: Box::new(|c| run_lot_pk(c, &page20)) },
        Banc { nom: "lot PK page 50", run: Box::new(|c| run_lot_pk(c, &page50)) },
        Banc { nom: "lot fils page 20", run: Box::new(|c| run_lot_thread(c, &page20)) },
        Banc { nom: "lot fils page 50", run: Box::new(|c| run_lot_thread(c, &page50)) },
    ];

    println!("\n== LOTS ({} itérations) ==", ITERATIONS);
    println!("{:<20} {:>6} {:>9} {:>9} {:>9} {:>9} {:>10}",
        "banc", "mode", "méd ms", "p95 ms", "min ms", "max ms", "Ko/page");
    for banc in &bancs {
        // FROID : purge + connexion neuve à chaque itération.
        let mut froid = Vec::new();
        let mut octets = 0u64;
        for _ in 0..ITERATIONS {
            #[cfg(windows)]
            purge_cache_fichier(&db);
            let c = Connection::open(&db)?;
            let (ms, o) = (banc.run)(&c)?;
            froid.push(ms);
            octets = o;
        }
        let (med, p95, min, max) = stats(froid);
        println!("{:<20} {:>6} {:>9.2} {:>9.2} {:>9.2} {:>9.2} {:>10.1}",
            banc.nom, "froid", med, p95, min, max, octets as f64 / 1024.0);
        // CHAUD : même connexion, 1 échauffement puis 20 itérations.
        let c = Connection::open(&db)?;
        (banc.run)(&c)?;
        let mut chaud = Vec::new();
        for _ in 0..ITERATIONS {
            let (ms, _) = (banc.run)(&c)?;
            chaud.push(ms);
        }
        let (med, p95, min, max) = stats(chaud);
        println!("{:<20} {:>6} {:>9.2} {:>9.2} {:>9.2} {:>9.2} {:>10.1}",
            banc.nom, "chaud", med, p95, min, max, octets as f64 / 1024.0);
    }

    println!("\n== UNITAIRE (chemin Store::body, 20 corps distincts) ==");
    // FROID : purge avant chaque corps.
    let mut froid = Vec::new();
    let mut tailles = Vec::new();
    for (mb, uid) in &unitaires {
        #[cfg(windows)]
        purge_cache_fichier(&db);
        let c = Connection::open(&db)?;
        let (ms, o) = run_unitaire(&c, mb, *uid)?;
        froid.push(ms);
        tailles.push(o);
    }
    let (med, p95, min, max) = stats(froid);
    println!("froid : méd {med:.2} ms  p95 {p95:.2} ms  min {min:.2}  max {max:.2}");
    // CHAUD : même connexion, corps déjà lus une fois.
    let c = Connection::open(&db)?;
    for (mb, uid) in &unitaires {
        run_unitaire(&c, mb, *uid)?;
    }
    let mut chaud = Vec::new();
    for (mb, uid) in &unitaires {
        let (ms, _) = run_unitaire(&c, mb, *uid)?;
        chaud.push(ms);
    }
    let (med, p95, min, max) = stats(chaud);
    println!("chaud : méd {med:.2} ms  p95 {p95:.2} ms  min {min:.2}  max {max:.2}");
    let somme: u64 = tailles.iter().sum();
    println!(
        "tailles des 20 corps : {:.1} Ko méd, {:.1}-{:.1} Ko, {:.1} Ko cumulés",
        {
            let mut t = tailles.clone();
            t.sort();
            t[t.len() / 2] as f64 / 1024.0
        },
        *tailles.iter().min().unwrap() as f64 / 1024.0,
        *tailles.iter().max().unwrap() as f64 / 1024.0,
        somme as f64 / 1024.0
    );

    let _ = std::io::stdout().flush();
    Ok(())
}

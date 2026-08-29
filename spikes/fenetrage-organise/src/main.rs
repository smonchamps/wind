//! Spike S1 — PLAN-MODE-ORGANISE : la Réception organisée
//! (sections « Nouveau pour vous » / « Déjà consulté » + repli
//! « un expéditeur groupé = UNE rangée »), AU SERVICE (SQL) contre
//! À L'AFFICHAGE (post-traitement des lignes servies).
//!
//! Base synthétique : ~200 000 enveloppes, ~2 000 expéditeurs, 5
//! « bavards » à ~600 messages marqués groupés (dont un en RAFALE sur
//! 12 h — le cas défavorable du repli à l'affichage). Le schéma, les
//! index et la requête de page reproduisent ceux de production
//! (`crates/mail-core/src/store.rs`, `unified_page_sql`) — les colonnes
//! `sender_address`/`groupe` ajoutées à `threads` ne servent qu'aux
//! variantes « industrialisées » et ne changent pas les plans des
//! autres (vérifié par les EXPLAIN imprimés).
//!
//! Usage : cargo run --release -- <chemin_db> <chemin_rows_json>

use anyhow::Result;
use rusqlite::{Connection, params, params_from_iter};
use std::time::Instant;

const N: i64 = 200_000;
const SENDERS_NORMAUX: i64 = 1_995;
const PAGE: i64 = 200;
const WARMUP: usize = 3;
const ITERS: usize = 20;

// ~2 ans de courrier, borne haute proche d'aujourd'hui.
const FIN: i64 = 1_756_400_000;
const DEBUT: i64 = FIN - 730 * 86_400;
// La rafale du bavard 4 : 600 messages en 12 h, il y a ~30 jours.
const RAFALE_DEBUT: i64 = FIN - 30 * 86_400;
const RAFALE_PAS: i64 = 43_200 / 600;
const RAFALE_UID0: i64 = 100_000;
const RAFALE_NB: i64 = 600;

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }
}

// ---------------------------------------------------------------- schéma

const SCHEMA: &str = "
CREATE TABLE accounts (id INTEGER PRIMARY KEY, email TEXT NOT NULL UNIQUE);
CREATE TABLE mailboxes (
    id INTEGER PRIMARY KEY,
    account_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    UNIQUE (account_id, name)
);
CREATE TABLE envelopes (
    mailbox_id INTEGER NOT NULL,
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
CREATE TABLE bodies (
    mailbox_id INTEGER NOT NULL, uid INTEGER NOT NULL, preview TEXT,
    PRIMARY KEY (mailbox_id, uid)
);
CREATE TABLE attachments (
    mailbox_id INTEGER NOT NULL, uid INTEGER NOT NULL, idx INTEGER NOT NULL,
    name TEXT NOT NULL, mime TEXT NOT NULL, size INTEGER NOT NULL,
    PRIMARY KEY (mailbox_id, uid, idx)
);
CREATE TABLE pins (mailbox_id INTEGER NOT NULL, uid INTEGER NOT NULL,
    PRIMARY KEY (mailbox_id, uid));
CREATE TABLE threads (
    id INTEGER PRIMARY KEY,
    account_id INTEGER NOT NULL,
    last_mailbox_id INTEGER,
    last_uid INTEGER NOT NULL DEFAULT 0,
    last_epoch INTEGER,
    size INTEGER NOT NULL DEFAULT 0,
    unseen INTEGER NOT NULL DEFAULT 0,
    inbox_size INTEGER NOT NULL DEFAULT 0,
    -- Colonnes SPIKE (variantes industrialisées B'') : l'expéditeur du
    -- dernier message, et « ce fil appartient à un expéditeur groupé ».
    sender_address TEXT,
    groupe INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_threads_date
    ON threads(account_id, last_epoch DESC, last_uid DESC);
CREATE INDEX idx_threads_date_globale
    ON threads(last_epoch DESC, last_uid DESC, account_id)
    WHERE inbox_size > 0;
CREATE TABLE groupes (address TEXT PRIMARY KEY);
-- Nécessaire au GROUP BY par expéditeur de la variante B (coût
-- d'industrialisation : cet index n'existe pas en production).
CREATE INDEX idx_env_sender ON envelopes(sender_address, date_epoch DESC, uid);
";

// ------------------------------------------------- requêtes (prod reproduite)

const PINNED_THREADS: &str = "SELECT pe.thread_id FROM pins p CROSS JOIN envelopes pe ON pe.mailbox_id = p.mailbox_id AND pe.uid = p.uid WHERE pe.thread_id IS NOT NULL";
const SELECT_UNIFIED: &str = "SELECT a.id, a.email, e.uid, e.subject, e.sender, e.sender_address, e.message_id, e.date_epoch, e.seen, e.flagged, (SELECT COUNT(*) FROM attachments att WHERE att.mailbox_id = e.mailbox_id AND att.uid = e.uid), e.thread_id, e.in_reply_to, m.name, b.preview, e.to_addrs, e.cc_addrs, t.size, t.unseen";
const JOIN_TAIL: &str = "
         JOIN envelopes e ON e.mailbox_id = t.last_mailbox_id AND e.uid = t.last_uid
         JOIN mailboxes m ON m.id = e.mailbox_id
         JOIN accounts a ON a.id = t.account_id
         LEFT JOIN bodies b ON b.mailbox_id = e.mailbox_id AND b.uid = e.uid
         ORDER BY t.last_epoch DESC, t.last_uid DESC, a.id";

/// La page de production (`unified_page_sql(false, false)`), avec un
/// filtre supplémentaire optionnel sur la sous-requête `threads`.
fn page_sql(filtre: &str, ordre: &str) -> String {
    format!(
        "{SELECT_UNIFIED}
         FROM (SELECT account_id, last_mailbox_id, last_uid, last_epoch, size, unseen
                 FROM threads
                WHERE inbox_size > 0 AND id NOT IN ({PINNED_THREADS}){filtre}
                ORDER BY {ordre}
                LIMIT ?1 OFFSET ?2) t{JOIN_TAIL}"
    )
}

const ORDRE_PROD: &str = "last_epoch DESC, last_uid DESC, account_id";

// ---------------------------------------------------------------- mesure

fn percentiles(mut v: Vec<f64>) -> (f64, f64) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let med = v[v.len() / 2];
    let p95 = v[((v.len() as f64 * 0.95).ceil() as usize).min(v.len()) - 1];
    (med, p95)
}

/// Prépare + exécute + consomme toutes les lignes, comme le fait la
/// production (`prepare` à chaque appel). Chaud : 3 échauffements.
fn bench(conn: &Connection, nom: &str, sql: &str, ps: &[i64]) -> Result<usize> {
    let mut lignes = 0usize;
    let mut temps = Vec::with_capacity(ITERS);
    for i in 0..(WARMUP + ITERS) {
        let t0 = Instant::now();
        let mut stmt = conn.prepare(sql)?;
        let mut rows = stmt.query(params_from_iter(ps.iter()))?;
        let mut n = 0usize;
        while let Some(r) = rows.next()? {
            // Toucher une colonne texte : la ligne se décode vraiment.
            let _: Option<String> = r.get(3).ok().flatten();
            n += 1;
        }
        let dt = t0.elapsed().as_secs_f64() * 1000.0;
        if i >= WARMUP {
            temps.push(dt);
        }
        lignes = n;
    }
    let (med, p95) = percentiles(temps);
    println!("{nom} | {lignes} lignes | méd {med:.2} ms | p95 {p95:.2} ms");
    Ok(lignes)
}

fn eqp(conn: &Connection, nom: &str, sql: &str, ps: &[i64]) -> Result<()> {
    println!("--- EXPLAIN QUERY PLAN : {nom}");
    let mut stmt = conn.prepare(&format!("EXPLAIN QUERY PLAN {sql}"))?;
    let mut rows = stmt.query(params_from_iter(ps.iter()))?;
    while let Some(r) = rows.next()? {
        let detail: String = r.get(3)?;
        println!("    {detail}");
    }
    Ok(())
}

// ------------------------------------------------------------- génération

fn expediteur(i: i64, rng: &mut Lcg) -> (i64, bool) {
    // (id, rafale) — ids 0..4 : bavards groupés ; 5.. : normaux.
    if (RAFALE_UID0..RAFALE_UID0 + RAFALE_NB).contains(&i) {
        return (4, true);
    }
    match i % 333 {
        0 => (0, false),
        1 => (1, false),
        2 => (2, false),
        3 => (3, false),
        _ => (5 + (rng.next() as i64 % SENDERS_NORMAUX), false),
    }
}

fn adresse(sid: i64) -> String {
    if sid < 5 {
        format!("bavard{sid}@exemple.fr")
    } else {
        format!("exp{sid:04}@exemple.fr")
    }
}

fn build(conn: &Connection) -> Result<()> {
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;
    conn.execute_batch(SCHEMA)?;
    conn.execute("INSERT INTO accounts (id, email) VALUES (1, 'moi@exemple.fr')", [])?;
    conn.execute("INSERT INTO mailboxes (id, account_id, name) VALUES (1, 1, 'INBOX')", [])?;
    conn.execute("INSERT INTO mailboxes (id, account_id, name) VALUES (2, 1, 'Envoyes')", [])?;
    for b in 0..5 {
        conn.execute("INSERT INTO groupes (address) VALUES (?1)", params![adresse(b)])?;
    }
    let mut rng = Lcg(0x5EED_2026_0829);
    let tx = conn.unchecked_transaction()?;
    {
        let mut ins_e = tx.prepare(
            "INSERT INTO envelopes (mailbox_id, uid, subject, sender, sender_address,
                 to_addrs, message_id, thread_id, date_epoch, seen)
             VALUES (1, ?1, ?2, ?3, ?4, 'moi@exemple.fr', ?5, ?6, ?7, ?8)",
        )?;
        let mut ins_t = tx.prepare(
            "INSERT INTO threads (id, account_id, last_mailbox_id, last_uid, last_epoch,
                 size, unseen, inbox_size, sender_address, groupe)
             VALUES (?1, 1, 1, ?2, ?3, 1, ?4, 1, ?5, ?6)",
        )?;
        for i in 0..N {
            let uid = i + 1;
            let (sid, rafale) = expediteur(i, &mut rng);
            let epoch = if rafale {
                RAFALE_DEBUT + (i - RAFALE_UID0) * RAFALE_PAS
            } else {
                DEBUT + (rng.next() as i64) % (FIN - DEBUT)
            };
            let seen = i64::from(rng.next() % 100 >= 12);
            let addr = adresse(sid);
            ins_e.execute(params![
                uid,
                format!("Objet du message {i} — quelques mots de plus pour la taille"),
                format!("Expediteur {sid}"),
                addr,
                format!("<m{i}@exemple.fr>"),
                uid,
                epoch,
                seen,
            ])?;
            ins_t.execute(params![uid, uid, epoch, 1 - seen, addr, i64::from(sid < 5)])?;
        }
    }
    tx.commit()?;
    // PAS d'ANALYZE : la production ne l'exécute jamais (voir le
    // commentaire de PINNED_THREADS) — le planificateur doit être jugé
    // dans les mêmes conditions.
    Ok(())
}

// ------------------------------------------------------------------ main

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let db = args.get(1).cloned().unwrap_or_else(|| "fenetrage.db".into());
    let json = args.get(2).cloned().unwrap_or_else(|| "rows.json".into());
    let _ = std::fs::remove_file(&db);

    let conn = Connection::open(&db)?;
    println!("SQLite {}", rusqlite::version());
    let t0 = Instant::now();
    build(&conn)?;
    println!("Base construite : {N} enveloppes en {:.1} s\n", t0.elapsed().as_secs_f64());

    let nonlus: i64 = conn.query_row(
        "SELECT COUNT(*) FROM threads WHERE inbox_size > 0 AND unseen > 0", [], |r| r.get(0))?;
    println!("Conversations non lues : {nonlus} / {N}\n");

    // ---------------- V0 : la page de production, reproduite (témoin)
    println!("== V0 — page de production reproduite (PAGE={PAGE}) ==");
    let v0 = page_sql("", ORDRE_PROD);
    eqp(&conn, "V0", &v0, &[PAGE, 0])?;
    for off in [0, 1_000, 100_000] {
        bench(&conn, &format!("V0 offset {off}"), &v0, &[PAGE, off])?;
    }

    // ---------------- A1 : sections au service, DEUX requêtes bornées
    println!("\n== A1 — deux requêtes bornées (index de prod, filtre évalué ligne à ligne) ==");
    let a_nonlus = page_sql(" AND unseen > 0", ORDRE_PROD);
    let a_lus = page_sql(" AND unseen = 0", ORDRE_PROD);
    eqp(&conn, "A1 non-lus", &a_nonlus, &[PAGE, 0])?;
    bench(&conn, "A1 non-lus offset 0", &a_nonlus, &[PAGE, 0])?;
    bench(&conn, &format!("A1 non-lus offset {}", nonlus - 250), &a_nonlus, &[PAGE, nonlus - 250])?;
    bench(&conn, "A1 lus offset 0", &a_lus, &[PAGE, 0])?;
    bench(&conn, "A1 lus offset 100000", &a_lus, &[PAGE, 100_000])?;

    println!("\n== A1' — mêmes requêtes, DEUX index partiels dédiés ==");
    conn.execute_batch(
        "CREATE INDEX idx_org_nonlus ON threads(last_epoch DESC, last_uid DESC, account_id)
             WHERE inbox_size > 0 AND unseen > 0;
         CREATE INDEX idx_org_lus ON threads(last_epoch DESC, last_uid DESC, account_id)
             WHERE inbox_size > 0 AND unseen = 0;",
    )?;
    eqp(&conn, "A1' non-lus", &a_nonlus, &[PAGE, 0])?;
    eqp(&conn, "A1' lus", &a_lus, &[PAGE, 0])?;
    bench(&conn, "A1' non-lus offset 0", &a_nonlus, &[PAGE, 0])?;
    bench(&conn, &format!("A1' non-lus offset {}", nonlus - 250), &a_nonlus, &[PAGE, nonlus - 250])?;
    bench(&conn, "A1' lus offset 0", &a_lus, &[PAGE, 0])?;
    bench(&conn, "A1' lus offset 100000", &a_lus, &[PAGE, 100_000])?;
    // Le COMPTE des non-lus : la couture entre les deux sections (à quel
    // offset bascule-t-on ?) — hors chemin d'affichage, mais nécessaire.
    bench(&conn, "A1' COUNT non-lus",
        "SELECT COUNT(*) FROM threads WHERE inbox_size > 0 AND unseen > 0", &[])?;
    conn.execute_batch("DROP INDEX idx_org_nonlus; DROP INDEX idx_org_lus;")?;

    // ---------------- A2 : sections au service, UNE requête, tri à deux clés
    println!("\n== A2 — une requête, ORDER BY (unseen>0) DESC, date DESC ==");
    let ordre_sections = "(unseen > 0) DESC, last_epoch DESC, last_uid DESC, account_id";
    let a2 = page_sql("", ordre_sections);
    eqp(&conn, "A2 sans index", &a2, &[PAGE, 0])?;
    bench(&conn, "A2 sans index offset 0", &a2, &[PAGE, 0])?;
    bench(&conn, "A2 sans index offset 100000", &a2, &[PAGE, 100_000])?;
    conn.execute_batch(
        "CREATE INDEX idx_org_sections ON threads((unseen > 0) DESC, last_epoch DESC, last_uid DESC, account_id)
             WHERE inbox_size > 0;",
    )?;
    eqp(&conn, "A2 index expression", &a2, &[PAGE, 0])?;
    bench(&conn, "A2 index expr offset 0", &a2, &[PAGE, 0])?;
    bench(&conn, "A2 index expr offset 100000", &a2, &[PAGE, 100_000])?;
    conn.execute_batch("DROP INDEX idx_org_sections;")?;

    // ---------------- B : repli de groupe AU SERVICE, sans dénormalisation
    println!("\n== B — repli au service : UNION ALL (flot non groupé + agrégat par expéditeur) ==");
    let b = format!(
        "SELECT est_groupe, cle, last_epoch, last_uid, n FROM (
            SELECT 0 AS est_groupe, t.id AS cle, t.last_epoch, t.last_uid, 1 AS n
              FROM threads t
              JOIN envelopes e ON e.mailbox_id = t.last_mailbox_id AND e.uid = t.last_uid
             WHERE t.inbox_size > 0
               AND e.sender_address NOT IN (SELECT address FROM groupes)
            UNION ALL
            SELECT 1, g.rowid, MAX(e.date_epoch), MAX(e.uid), COUNT(*)
              FROM groupes g JOIN envelopes e ON e.sender_address = g.address
             GROUP BY g.address
         ) ORDER BY last_epoch DESC, last_uid DESC LIMIT ?1 OFFSET ?2"
    );
    eqp(&conn, "B", &b, &[PAGE, 0])?;
    for off in [0, 5_000, 100_000] {
        bench(&conn, &format!("B offset {off}"), &b, &[PAGE, off])?;
    }
    // Le TOTAL affichable (l'offset stable exige de le connaître).
    let b_total = "SELECT
        (SELECT COUNT(*) FROM threads t
           JOIN envelopes e ON e.mailbox_id = t.last_mailbox_id AND e.uid = t.last_uid
          WHERE t.inbox_size > 0 AND e.sender_address NOT IN (SELECT address FROM groupes))
      + (SELECT COUNT(DISTINCT e.sender_address) FROM groupes g
           JOIN envelopes e ON e.sender_address = g.address)";
    bench(&conn, "B COUNT total organisé", b_total, &[])?;

    // ---------------- B'' : repli au service, industrialisé (dénormalisé)
    println!("\n== B'' — industrialisé : threads.groupe précalculé, groupes servis À PART (comme les épingles) ==");
    conn.execute_batch(
        "CREATE INDEX idx_flot_hors_groupe ON threads(last_epoch DESC, last_uid DESC, account_id)
             WHERE inbox_size > 0 AND groupe = 0;
         CREATE TABLE groupes_agg AS
             SELECT e.sender_address AS address, COUNT(*) AS n,
                    MAX(e.date_epoch) AS last_epoch, MAX(e.uid) AS last_uid
               FROM groupes g JOIN envelopes e ON e.sender_address = g.address
              GROUP BY e.sender_address;",
    )?;
    let b2 = page_sql(" AND groupe = 0", ORDRE_PROD);
    eqp(&conn, "B'' flot", &b2, &[PAGE, 0])?;
    bench(&conn, "B'' flot offset 0", &b2, &[PAGE, 0])?;
    bench(&conn, "B'' flot offset 100000", &b2, &[PAGE, 100_000])?;
    bench(&conn, "B'' rangées de groupe (matérialisées)",
        "SELECT address, n, last_epoch, last_uid FROM groupes_agg ORDER BY last_epoch DESC", &[])?;
    bench(&conn, "B'' agrégat recalculé à la volée (borne haute)",
        "SELECT e.sender_address, COUNT(*), MAX(e.date_epoch), MAX(e.uid)
           FROM groupes g JOIN envelopes e ON e.sender_address = g.address
          GROUP BY e.sender_address", &[])?;

    // ---------------- Faits pour l'AFFICHAGE : profondeur des non-lus
    println!("\n== Faits — sections à l'affichage ==");
    let rang_200e: i64 = conn.query_row(
        "WITH nl AS (SELECT last_epoch, last_uid FROM threads
                      WHERE inbox_size > 0 AND unseen > 0
                      ORDER BY last_epoch DESC, last_uid DESC LIMIT 1 OFFSET 199)
         SELECT COUNT(*) FROM threads t, nl
          WHERE t.inbox_size > 0
            AND (t.last_epoch > nl.last_epoch
                 OR (t.last_epoch = nl.last_epoch AND t.last_uid >= nl.last_uid))",
        [], |r| r.get(0))?;
    println!("Rang (dans le flot servi) du 200e fil non lu : {rang_200e} → {} vols de page pour remplir la 1re page de la section non-lus à l'affichage",
        ((rang_200e + PAGE - 1) / PAGE));
    let rang_dernier: i64 = conn.query_row(
        "WITH nl AS (SELECT last_epoch, last_uid FROM threads
                      WHERE inbox_size > 0 AND unseen > 0
                      ORDER BY last_epoch, last_uid LIMIT 1)
         SELECT COUNT(*) FROM threads t, nl
          WHERE t.inbox_size > 0
            AND (t.last_epoch > nl.last_epoch
                 OR (t.last_epoch = nl.last_epoch AND t.last_uid >= nl.last_uid))",
        [], |r| r.get(0))?;
    println!("Rang du DERNIER fil non lu : {rang_dernier} → la section complète exige {} vols",
        ((rang_dernier + PAGE - 1) / PAGE));

    // ---------------- Export pour la simulation Node (repli à l'affichage)
    let mut stmt = conn.prepare(
        "SELECT t.sender_address, t.unseen, t.groupe FROM threads t
          WHERE t.inbox_size > 0
          ORDER BY t.last_epoch DESC, t.last_uid DESC, t.account_id")?;
    let mut rows = stmt.query([])?;
    let mut out = String::with_capacity(6_000_000);
    out.push('[');
    let mut premier = true;
    while let Some(r) = rows.next()? {
        let s: String = r.get(0)?;
        let u: i64 = r.get(1)?;
        let g: i64 = r.get(2)?;
        if !premier { out.push(','); }
        premier = false;
        out.push_str(&format!("[\"{s}\",{u},{g}]"));
    }
    out.push(']');
    std::fs::write(&json, out)?;
    println!("\nExport {json} écrit (ordre servi, {N} lignes).");
    Ok(())
}

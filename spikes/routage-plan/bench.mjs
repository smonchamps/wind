// Spike S2 (PLAN-MODE-ORGANISE) — plan SQLite du routage d'expéditeurs
// dans les requêtes chaudes. JETABLE. Usage :
//   node spikes/routage-plan/bench.mjs spikes/routage-plan/banc.db
//
// La base vient de `cargo run -p mail-core --example seed_inbox --release
// -- <db> 200000 seed@exemple.fr 0 0 INBOX`, puis ce script :
//   1. réécrit sender_address → 2000 adresses distinctes (le seed n'en
//      produit que 8) ;
//   2. crée `routage_expediteurs` (schéma proposé au PLAN) et y route 50
//      adresses (25 'kiosque', 25 'registres'), decide_epoch au milieu de
//      la plage des dates ;
//   3. mesure chaque requête : 5 échauffements, 20 itérations chronométrées,
//      médiane + p95, et le EXPLAIN QUERY PLAN brut.
import { DatabaseSync } from "node:sqlite";

const dbPath = process.argv[2] ?? "spikes/routage-plan/banc.db";
const db = new DatabaseSync(dbPath);
db.exec("PRAGMA journal_mode = WAL;");

// ---------------------------------------------------------------- décor
const N_SENDERS = 2000;
const N_ROUTED = 50;

const deja = db
  .prepare("SELECT COUNT(DISTINCT sender_address) AS n FROM envelopes")
  .get().n;
if (deja < N_SENDERS) {
  // Déterministe : l'expéditeur dépend de l'uid. Distribution zipf-ienne
  // grossière : les 20 premières adresses portent la moitié du courrier
  // (les newsletters dominent une vraie boîte).
  db.exec(`
    UPDATE envelopes SET sender_address =
      CASE WHEN uid % 2 = 0 THEN 'gros' || (uid % 20) || '@exemple.fr'
           ELSE 'exp' || (uid % ${N_SENDERS}) || '@exemple.fr' END`);
}
db.exec(`
  CREATE TABLE IF NOT EXISTS routage_expediteurs (
    adresse      TEXT PRIMARY KEY,
    destination  TEXT NOT NULL,
    regle        TEXT,
    decide_epoch INTEGER NOT NULL
  );
  DELETE FROM routage_expediteurs;`);
// L'époque médiane du courrier : la moitié des messages routés sont
// arrivés APRÈS la décision — le cas de la variante « époque ».
const { epoque } = db
  .prepare(
    "SELECT date_epoch AS epoque FROM envelopes ORDER BY date_epoch LIMIT 1 OFFSET (SELECT COUNT(*)/2 FROM envelopes)"
  )
  .get();
const ins = db.prepare(
  "INSERT INTO routage_expediteurs (adresse, destination, regle, decide_epoch) VALUES (?, ?, NULL, ?)"
);
for (let i = 0; i < N_ROUTED; i++) {
  // 10 grosses adresses + 40 moyennes : le Kiosque reçoit du volume.
  const adresse =
    i < 10 ? `gros${i}@exemple.fr` : `exp${i * 7}@exemple.fr`;
  ins.run(adresse, i % 2 === 0 ? "kiosque" : "registres", epoque);
}
const couverts = db
  .prepare(
    "SELECT COUNT(*) AS n FROM envelopes e WHERE e.sender_address IN (SELECT adresse FROM routage_expediteurs)"
  )
  .get().n;
const total = db.prepare("SELECT COUNT(*) AS n FROM envelopes").get().n;
const inboxId = db
  .prepare("SELECT id FROM mailboxes WHERE name = 'INBOX'")
  .get().id;
console.log(
  `decor : ${total} enveloppes, ${N_SENDERS + 20} adresses distinctes, ${N_ROUTED} routees couvrant ${couverts} messages, epoque mediane ${epoque}, inbox=${inboxId}`
);

// ------------------------------------------------- requêtes reproduites
// Copie CONFORME de unified_page_sql(false, false) — store.rs:2674,
// SELECT_UNIFIED l.555, THREAD_AGGREGATE l.562, PINNED_THREADS l.575,
// UNIFIED_JOIN_TAIL l.582.
const SELECT_UNIFIED =
  "SELECT a.id, a.email, e.uid, e.subject, e.sender, e.sender_address, e.message_id, e.date_epoch, e.seen, e.flagged, (SELECT COUNT(*) FROM attachments att WHERE att.mailbox_id = e.mailbox_id AND att.uid = e.uid), e.thread_id, e.in_reply_to, m.name, b.preview, e.to_addrs, e.cc_addrs";
const THREAD_AGGREGATE = ", t.size, t.unseen";
const PINNED_THREADS =
  "SELECT pe.thread_id FROM pins p CROSS JOIN envelopes pe ON pe.mailbox_id = p.mailbox_id AND pe.uid = p.uid WHERE pe.thread_id IS NOT NULL";
const TAIL = `
         JOIN envelopes e ON e.mailbox_id = t.last_mailbox_id AND e.uid = t.last_uid
         JOIN mailboxes m ON m.id = e.mailbox_id
         JOIN accounts a ON a.id = t.account_id
         LEFT JOIN bodies b ON b.mailbox_id = e.mailbox_id AND b.uid = e.uid`;
const ORDER = "\n         ORDER BY t.last_epoch DESC, t.last_uid DESC, a.id";

const squelette = (surcroit_where = "", surcroit_join = "") =>
  `${SELECT_UNIFIED}${THREAD_AGGREGATE}
         FROM (SELECT account_id, last_mailbox_id, last_uid, last_epoch, size, unseen
                 FROM threads
                WHERE inbox_size > 0 AND id NOT IN (${PINNED_THREADS})
                ORDER BY last_epoch DESC, last_uid DESC, account_id
                LIMIT ?1 OFFSET ?2) t${TAIL}${surcroit_join}${surcroit_where}${ORDER}`;

const EXISTE_ROUTAGE =
  "EXISTS (SELECT 1 FROM routage_expediteurs r WHERE r.adresse = e.sender_address)";

const cas = [
  {
    nom: "U0  page unifiee EXISTANTE (temoin), offset 0",
    sql: squelette(),
    params: [50, 0],
  },
  {
    nom: "U0d page unifiee EXISTANTE (temoin), offset 100000",
    sql: squelette(),
    params: [50, 100000],
  },
  {
    nom: "U1  + exclusion NOT EXISTS (sans epoque)",
    sql: squelette(`\n         WHERE NOT ${EXISTE_ROUTAGE}`),
    params: [50, 0],
  },
  {
    nom: "U2  + exclusion LEFT JOIN r ... WHERE r.adresse IS NULL",
    sql: squelette(
      "\n         WHERE r.adresse IS NULL",
      "\n         LEFT JOIN routage_expediteurs r ON r.adresse = e.sender_address"
    ),
    params: [50, 0],
  },
  {
    nom: "U3  + exclusion NOT EXISTS avec epoque (r pose, message ARRIVE APRES)",
    sql: squelette(
      "\n         WHERE NOT EXISTS (SELECT 1 FROM routage_expediteurs r WHERE r.adresse = e.sender_address AND e.date_epoch > r.decide_epoch)"
    ),
    params: [50, 0],
  },
  {
    nom: "U4  + exclusion PAR FILS a la maniere des pins : t.id NOT IN (fils des routes) — SANS index sender",
    sql: `${SELECT_UNIFIED}${THREAD_AGGREGATE}
         FROM (SELECT account_id, last_mailbox_id, last_uid, last_epoch, size, unseen
                 FROM threads
                WHERE inbox_size > 0 AND id NOT IN (${PINNED_THREADS})
                  AND id NOT IN (SELECT re.thread_id FROM routage_expediteurs r CROSS JOIN envelopes re ON re.sender_address = r.adresse WHERE re.thread_id IS NOT NULL)
                ORDER BY last_epoch DESC, last_uid DESC, account_id
                LIMIT ?1 OFFSET ?2) t${TAIL}${ORDER}`,
    params: [50, 0],
  },
  {
    nom: "K1  page Kiosque (tranche category_page filtree destination) — SANS index sender",
    sql: `SELECT e.mailbox_id, e.uid, e.date_epoch FROM envelopes e
          WHERE e.mailbox_id = ?1
            AND e.sender_address IN (SELECT adresse FROM routage_expediteurs WHERE destination = 'kiosque')
          ORDER BY e.date_epoch DESC, e.uid DESC LIMIT ?2`,
    params: [inboxId, 50],
  },
  {
    nom: "T0  category_totals EXISTANT (temoin)",
    sql: "SELECT COUNT(*), COALESCE(SUM(NOT e.seen), 0) FROM envelopes e WHERE e.mailbox_id = ?1",
    params: [inboxId],
  },
  {
    nom: "T1  category_totals + exclusion NOT EXISTS",
    sql: `SELECT COUNT(*), COALESCE(SUM(NOT e.seen), 0) FROM envelopes e WHERE e.mailbox_id = ?1 AND NOT ${EXISTE_ROUTAGE}`,
    params: [inboxId],
  },
  {
    nom: "T2  totaux du Kiosque (COUNT filtre destination) — SANS index sender",
    sql: `SELECT COUNT(*), COALESCE(SUM(NOT e.seen), 0) FROM envelopes e
          WHERE e.mailbox_id = ?1
            AND e.sender_address IN (SELECT adresse FROM routage_expediteurs WHERE destination = 'kiosque')`,
    params: [inboxId],
  },
];

// Les mêmes cas dépendants d'un accès inversé (adresse → enveloppes),
// rejoués APRÈS création d'un index envelopes(sender_address).
const casAvecIndex = [
  { ...cas.find((c) => c.nom.startsWith("U4")), nom: "U4i idem U4 — AVEC index envelopes(sender_address)" },
  { ...cas.find((c) => c.nom.startsWith("K1")), nom: "K1i idem K1 — AVEC index" },
  { ...cas.find((c) => c.nom.startsWith("T2")), nom: "T2i idem T2 — AVEC index" },
  { ...cas.find((c) => c.nom.startsWith("U1")), nom: "U1i idem U1 — AVEC index (contrôle : le plan bouge-t-il ?)" },
];

function mesure({ nom, sql, params }) {
  const stmt = db.prepare(sql);
  for (let i = 0; i < 5; i++) stmt.all(...params); // échauffement
  const temps = [];
  for (let i = 0; i < 20; i++) {
    const t0 = process.hrtime.bigint();
    const lignes = stmt.all(...params);
    const t1 = process.hrtime.bigint();
    temps.push(Number(t1 - t0) / 1e6);
    if (i === 0) var nLignes = lignes.length;
  }
  temps.sort((a, b) => a - b);
  const med = (temps[9] + temps[10]) / 2;
  const p95 = temps[18];
  const plan = db
    .prepare(`EXPLAIN QUERY PLAN ${sql}`)
    .all(...params)
    .map((r) => `${"  ".repeat(0)}[${r.id}<-${r.parent}] ${r.detail}`)
    .join("\n    ");
  console.log(
    `\n${nom}\n    lignes=${nLignes}  mediane=${med.toFixed(3)} ms  p95=${p95.toFixed(3)} ms  min=${temps[0].toFixed(3)}  max=${temps[19].toFixed(3)}\n    ${plan}`
  );
  return { nom, med, p95 };
}

console.log("\n=== SANS index envelopes(sender_address) ===");
for (const c of cas) mesure(c);

console.log("\n=== AVEC index idx_spike_sender ON envelopes(sender_address) ===");
const t0 = process.hrtime.bigint();
db.exec("CREATE INDEX IF NOT EXISTS idx_spike_sender ON envelopes(sender_address)");
console.log(
  `creation de l'index : ${(Number(process.hrtime.bigint() - t0) / 1e6).toFixed(1)} ms`
);
for (const c of casAvecIndex) mesure(c);

db.close();

// Spike S2-bis (PLAN-MODE-ORGANISE E2) — le coût de la RÉTENTION du
// Portier dans les requêtes chaudes de la Réception. JETABLE. Usage :
//   node spikes/routage-plan/bench-portier.mjs spikes/routage-plan/banc.db
//
// La question mesurée : « expéditeur en attente » = SANS ligne de
// routage ET premier message POSTÉRIEUR à l'époque d'activation (D3
// arrivées seules). Deux formes candidates :
//   V1 — calcul à la requête (sonde "connu avant l'époque" par index
//        d'expression lower(trim(sender_address)), date_epoch) ;
//   V2 — matérialisation `portier_attente(address PK)` entretenue à
//        l'arrivée (sonde PK, patron routage_expediteurs).
// Sémantique de rétention (les deux variantes) :
//   - un fil QUITTE la Réception s'il porte un message d'un expéditeur
//     routé AILLEURS que 'reception' (il est visible dans sa vue — le
//     miroir exact de fil_route_sql, jamais une perte) ;
//   - un fil est RETENU au Portier si TOUS ses messages viennent
//     d'expéditeurs en attente (un fil mêlé — un inconnu répond dans un
//     fil connu — RESTE en Réception : règle d'or, jamais perdre de
//     courrier ; l'inconnu attend quand même au Portier).
import { DatabaseSync } from "node:sqlite";

const dbPath = process.argv[2] ?? "spikes/routage-plan/banc.db";
const db = new DatabaseSync(dbPath);
db.exec("PRAGMA journal_mode = WAL;");

// ---------------------------------------------------------------- décor
const N_SENDERS = 2000;
const N_ROUTED = 50;
const N_NOUVEAUX = 300; // expéditeurs qui n'existent QU'APRÈS l'époque

// L'époque = la date médiane du courrier (la moitié du courrier est
// « historique », l'autre moitié « arrivée depuis l'activation »).
const { epoque } = db
  .prepare(
    "SELECT date_epoch AS epoque FROM envelopes ORDER BY date_epoch LIMIT 1 OFFSET (SELECT COUNT(*)/2 FROM envelopes)"
  )
  .get();

// Réécriture déterministe des expéditeurs (le seed n'en produit que 8) :
// zipf grossière (20 grosses adresses = la moitié du courrier), et dans
// le courrier POSTÉRIEUR à l'époque, 1 message sur 8 vient d'un
// expéditeur NOUVEAU (nouvN@) qui n'existe nulle part avant — le flux
// d'inconnus du Portier.
db.exec(`
  UPDATE envelopes SET sender_address =
    CASE WHEN uid % 2 = 0 THEN 'gros' || (uid % 20) || '@exemple.fr'
         ELSE 'exp' || (uid % ${N_SENDERS}) || '@exemple.fr' END`);
db.exec(`
  UPDATE envelopes SET sender_address = 'nouv' || (uid % ${N_NOUVEAUX}) || '@exemple.fr'
   WHERE date_epoch > ${epoque} AND uid % 8 = 3`);

// La table de routage au schéma de PRODUCTION (store.rs:335).
db.exec(`
  DROP TABLE IF EXISTS routage_expediteurs;
  CREATE TABLE routage_expediteurs (
    address     TEXT PRIMARY KEY,
    destination TEXT NOT NULL CHECK (destination IN ('reception','kiosque','registre','ecarte')),
    regle       TEXT CHECK (regle IN ('spam','archive','corbeille')),
    epoch       INTEGER NOT NULL
  );`);
const ins = db.prepare(
  "INSERT INTO routage_expediteurs (address, destination, regle, epoch) VALUES (?, ?, NULL, ?)"
);
for (let i = 0; i < N_ROUTED; i++) {
  const adresse = i < 10 ? `gros${i}@exemple.fr` : `exp${i * 7}@exemple.fr`;
  ins.run(adresse, i % 2 === 0 ? "kiosque" : "registre", epoque);
}
// Quelques inconnus DÉJÀ décidés (Oui → reception, Non → ecarte) : le
// Portier réel n'est jamais tout-attente.
for (let i = 0; i < 20; i++) {
  ins.run(
    `nouv${i}@exemple.fr`,
    i % 2 === 0 ? "reception" : "ecarte",
    epoque
  );
}

const total = db.prepare("SELECT COUNT(*) AS n FROM envelopes").get().n;
console.log(
  `decor : ${total} enveloppes, epoque=${epoque}, ${N_NOUVEAUX} nouveaux dont 20 decides, ${N_ROUTED} anciens routes`
);

// ---------------------------------------------------- index d'expression
// V1 comme V2 sondent « ce sender a-t-il du courrier avant l'époque ? »
// (V2 : une fois par arrivée ; V1 : dans la requête chaude). L'index
// porte l'expression EXACTE de fil_route_sql.
let t0 = process.hrtime.bigint();
db.exec(
  "CREATE INDEX IF NOT EXISTS idx_spike_sender_norm ON envelopes(lower(trim(sender_address)), date_epoch)"
);
console.log(
  `creation idx (lower(trim(sender_address)), date_epoch) : ${(Number(process.hrtime.bigint() - t0) / 1e6).toFixed(1)} ms`
);

// -------------------------------------- V2 : matérialisation de l'attente
db.exec(`
  DROP TABLE IF EXISTS portier_attente;
  CREATE TABLE portier_attente (address TEXT PRIMARY KEY, premiere_epoch INTEGER NOT NULL);`);
t0 = process.hrtime.bigint();
db.exec(`
  INSERT INTO portier_attente (address, premiere_epoch)
  SELECT lower(trim(sender_address)) AS a, MIN(date_epoch)
    FROM envelopes
   GROUP BY a
  HAVING MIN(date_epoch) > ${epoque}
     AND NOT EXISTS (SELECT 1 FROM routage_expediteurs r WHERE r.address = a)`);
const nAttente = db.prepare("SELECT COUNT(*) AS n FROM portier_attente").get().n;
console.log(
  `materialisation portier_attente (rattrapage complet 200k) : ${(Number(process.hrtime.bigint() - t0) / 1e6).toFixed(1)} ms, ${nAttente} adresses en attente`
);

// ------------------------------------------------- requêtes reproduites
// Copie CONFORME de unified_page_sql (store.rs:2861).
const SELECT_UNIFIED =
  "SELECT a.id, a.email, e.uid, e.subject, e.sender, e.sender_address, e.message_id, e.date_epoch, e.seen, e.flagged, (SELECT COUNT(*) FROM attachments att WHERE att.mailbox_id = e.mailbox_id AND att.uid = e.uid), e.thread_id, e.in_reply_to, m.name, b.preview, e.to_addrs, e.cc_addrs";
const THREAD_AGGREGATE = ", t.size, t.unseen";
const PINNED_THREADS =
  "SELECT pe.thread_id FROM pins p CROSS JOIN envelopes pe ON pe.mailbox_id = p.mailbox_id AND pe.uid = p.uid WHERE pe.thread_id IS NOT NULL";
const TAIL = `
         JOIN envelopes e ON e.mailbox_id = t.last_mailbox_id AND e.uid = t.last_uid
         JOIN mailboxes m ON m.id = e.mailbox_id
         JOIN accounts a ON a.id = t.account_id
         LEFT JOIN bodies b ON b.mailbox_id = e.mailbox_id AND b.uid = e.uid
         ORDER BY t.last_epoch DESC, t.last_uid DESC, a.id`;

const squelette = (filtres = "") =>
  `${SELECT_UNIFIED}${THREAD_AGGREGATE}
         FROM (SELECT account_id, last_mailbox_id, last_uid, last_epoch, size, unseen
                 FROM threads
                WHERE inbox_size > 0 AND id NOT IN (${PINNED_THREADS})${filtres}
                ORDER BY last_epoch DESC, last_uid DESC, account_id
                LIMIT ?1 OFFSET ?2) t${TAIL}`;

// Le miroir de fil_route_sql : un message routé AILLEURS éjecte le fil.
const HORS_ROUTE_AILLEURS = `
                  AND NOT EXISTS (
                    SELECT 1 FROM envelopes te
                      JOIN routage_expediteurs r
                        ON r.address = lower(trim(te.sender_address))
                       AND r.destination <> 'reception'
                     WHERE te.thread_id = threads.id)`;

// V1 — « le fil est LIBRE » calculé à la requête : il existe un message
// dont l'expéditeur a une ligne (peu importe où : le cas 'ailleurs' est
// déjà éjecté au-dessus) OU du courrier avant l'époque.
const LIBRE_V1 = `
                  AND EXISTS (
                    SELECT 1 FROM envelopes te
                     WHERE te.thread_id = threads.id
                       AND (EXISTS (SELECT 1 FROM routage_expediteurs r2
                                     WHERE r2.address = lower(trim(te.sender_address)))
                            OR EXISTS (SELECT 1 FROM envelopes e0
                                        WHERE lower(trim(e0.sender_address)) = lower(trim(te.sender_address))
                                          AND e0.date_epoch <= ${epoque})))`;

// V2 — « le fil est LIBRE » par sonde PK sur la matérialisation.
const LIBRE_V2 = `
                  AND EXISTS (
                    SELECT 1 FROM envelopes te
                     WHERE te.thread_id = threads.id
                       AND NOT EXISTS (SELECT 1 FROM portier_attente pa
                                        WHERE pa.address = lower(trim(te.sender_address))))`;

// V3 — patron PINS : les fils à écarter en listes MATÉRIALISÉES (NOT
// IN), calculées une fois par requête — jamais une sonde par rangée
// sautée. Les deux listes sont petites par construction : les fils
// routés ailleurs (bornés au courrier des expéditeurs décidés) et les
// fils retenus (bornés au courrier des inconnus depuis l'époque).
const FILS_ROUTES_AILLEURS = `
                    SELECT te.thread_id FROM routage_expediteurs r
                     CROSS JOIN envelopes te ON lower(trim(te.sender_address)) = r.address
                     WHERE r.destination <> 'reception' AND te.thread_id IS NOT NULL`;
// Un fil n'est RETENU que si TOUS ses messages viennent d'inconnus en
// attente — le contrôle « entièrement inconnu » ne se paie que sur la
// petite liste des fils touchés par un inconnu.
const FILS_RETENUS = `
                    SELECT ta.thread_id FROM portier_attente pa
                     CROSS JOIN envelopes ta ON lower(trim(ta.sender_address)) = pa.address
                     WHERE ta.thread_id IS NOT NULL
                       AND NOT EXISTS (
                         SELECT 1 FROM envelopes o
                          WHERE o.thread_id = ta.thread_id
                            AND NOT EXISTS (SELECT 1 FROM portier_attente pa2
                                             WHERE pa2.address = lower(trim(o.sender_address))))`;
const EXCLUSIONS_V3 = `
                  AND id NOT IN (${FILS_ROUTES_AILLEURS})
                  AND id NOT IN (${FILS_RETENUS})`;

const COUNT_TETE = `SELECT COUNT(*) FROM threads
              WHERE inbox_size > 0 AND id NOT IN (${PINNED_THREADS})`;

const cas = [
  { nom: "U0   page unifiee TEMOIN, offset 0", sql: squelette(), params: [50, 0] },
  { nom: "U0d  page unifiee TEMOIN, offset 100000", sql: squelette(), params: [50, 100000] },
  {
    nom: "UV1  Reception organisee V1 (requete), offset 0",
    sql: squelette(HORS_ROUTE_AILLEURS + LIBRE_V1),
    params: [50, 0],
  },
  {
    nom: "UV1d idem, offset 100000",
    sql: squelette(HORS_ROUTE_AILLEURS + LIBRE_V1),
    params: [50, 100000],
  },
  {
    nom: "UV2  Reception organisee V2 (materialisee), offset 0",
    sql: squelette(HORS_ROUTE_AILLEURS + LIBRE_V2),
    params: [50, 0],
  },
  {
    nom: "UV2d idem, offset 100000",
    sql: squelette(HORS_ROUTE_AILLEURS + LIBRE_V2),
    params: [50, 100000],
  },
  {
    nom: "UV3  Reception organisee V3 (listes NOT IN, patron pins), offset 0",
    sql: squelette(EXCLUSIONS_V3),
    params: [50, 0],
  },
  {
    nom: "UV3d idem, offset 100000",
    sql: squelette(EXCLUSIONS_V3),
    params: [50, 100000],
  },
  { nom: "C0   count unifie TEMOIN", sql: COUNT_TETE, params: [] },
  {
    nom: "CV3  count Reception organisee V3",
    sql: COUNT_TETE + EXCLUSIONS_V3,
    params: [],
  },
  {
    nom: "CV1  count Reception organisee V1",
    sql: COUNT_TETE + HORS_ROUTE_AILLEURS + LIBRE_V1,
    params: [],
  },
  {
    nom: "CV2  count Reception organisee V2",
    sql: COUNT_TETE + HORS_ROUTE_AILLEURS + LIBRE_V2,
    params: [],
  },
  {
    nom: "PV1  page du Portier V1 (GROUP BY sur l'index, dernier message par attente)",
    sql: `SELECT a, MAX(date_epoch) AS derniere, COUNT(*) AS n
            FROM (SELECT lower(trim(sender_address)) AS a, date_epoch FROM envelopes)
           GROUP BY a
          HAVING MIN(date_epoch) > ${epoque}
             AND NOT EXISTS (SELECT 1 FROM routage_expediteurs r WHERE r.address = a)
           ORDER BY derniere DESC`,
    params: [],
  },
  {
    nom: "PV2  page du Portier V2 (jointure portier_attente -> dernier message)",
    sql: `SELECT pa.address,
                 (SELECT e.date_epoch FROM envelopes e
                   WHERE lower(trim(e.sender_address)) = pa.address
                   ORDER BY e.date_epoch DESC LIMIT 1) AS derniere,
                 (SELECT COUNT(*) FROM envelopes e2
                   WHERE lower(trim(e2.sender_address)) = pa.address) AS n
            FROM portier_attente pa
           ORDER BY derniere DESC`,
    params: [],
  },
  {
    nom: "NV2  pastille nav V2 (nombre de MESSAGES en attente, somme par index)",
    sql: `SELECT COALESCE(SUM((SELECT COUNT(*) FROM envelopes e
                   WHERE lower(trim(e.sender_address)) = pa.address)), 0)
            FROM portier_attente pa`,
    params: [],
  },
  {
    nom: "NV2s pastille nav V2 (nombre d'EXPEDITEURS en attente)",
    sql: "SELECT COUNT(*) FROM portier_attente",
    params: [],
  },
];

function mesure({ nom, sql, params }) {
  const stmt = db.prepare(sql);
  for (let i = 0; i < 5; i++) stmt.all(...params);
  const temps = [];
  let nLignes = 0;
  for (let i = 0; i < 20; i++) {
    const t0 = process.hrtime.bigint();
    const lignes = stmt.all(...params);
    const t1 = process.hrtime.bigint();
    temps.push(Number(t1 - t0) / 1e6);
    if (i === 0) nLignes = lignes.length;
  }
  temps.sort((a, b) => a - b);
  const med = (temps[9] + temps[10]) / 2;
  const p95 = temps[18];
  const plan = db
    .prepare(`EXPLAIN QUERY PLAN ${sql}`)
    .all(...params)
    .map((r) => `[${r.id}<-${r.parent}] ${r.detail}`)
    .join("\n    ");
  console.log(
    `\n${nom}\n    lignes=${nLignes}  mediane=${med.toFixed(3)} ms  p95=${p95.toFixed(3)} ms  min=${temps[0].toFixed(3)}  max=${temps[19].toFixed(3)}\n    ${plan}`
  );
}

for (const c of cas) mesure(c);

// -------------------- V2 : le coût d'entretien à l'ARRIVÉE d'un message
// Par message inséré en synchro : sonde routage (PK), sonde attente
// (PK), sonde « courrier avant l'époque » (index). 1000 décisions.
const sondeRoutage = db.prepare(
  "SELECT 1 FROM routage_expediteurs WHERE address = ?1"
);
const sondeAttente = db.prepare(
  "SELECT 1 FROM portier_attente WHERE address = ?1"
);
const sondeConnu = db.prepare(
  `SELECT 1 FROM envelopes WHERE lower(trim(sender_address)) = ?1 AND date_epoch <= ${epoque} LIMIT 1`
);
t0 = process.hrtime.bigint();
let inconnus = 0;
for (let i = 0; i < 1000; i++) {
  const a =
    i % 3 === 0
      ? `nouv${i % N_NOUVEAUX}@exemple.fr`
      : `exp${i % N_SENDERS}@exemple.fr`;
  if (!sondeRoutage.get(a) && !sondeAttente.get(a) && !sondeConnu.get(a))
    inconnus++;
}
console.log(
  `\nMV2  entretien V2 : 1000 decisions d'arrivee en ${(Number(process.hrtime.bigint() - t0) / 1e6).toFixed(2)} ms (${inconnus} nouveaux detectes)`
);

db.close();

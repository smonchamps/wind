import { DatabaseSync } from "node:sqlite";
const db = new DatabaseSync("spikes/routage-plan/banc.db");
db.exec("PRAGMA journal_mode = WAL;");
// V4 — drapeau maintenu (patron S1 threads.groupe / size / unseen).
let t0 = process.hrtime.bigint();
try { db.exec("ALTER TABLE threads ADD COLUMN organise_hors INTEGER NOT NULL DEFAULT 0"); } catch {}
console.log("alter threads:", (Number(process.hrtime.bigint() - t0) / 1e6).toFixed(1), "ms");
// Rattrapage complet (activation du mode / reconstruction) :
t0 = process.hrtime.bigint();
db.exec(`UPDATE threads SET organise_hors = 0`);
db.exec(`UPDATE threads SET organise_hors = 1 WHERE id IN (
  SELECT te.thread_id FROM routage_expediteurs r
   CROSS JOIN envelopes te ON te.sender_norm = r.address
   WHERE r.destination <> 'reception' AND te.thread_id IS NOT NULL
  UNION
  SELECT ta.thread_id FROM portier_attente pa
   CROSS JOIN envelopes ta ON ta.sender_norm = pa.address
   WHERE ta.thread_id IS NOT NULL
     AND NOT EXISTS (SELECT 1 FROM envelopes o WHERE o.thread_id = ta.thread_id
       AND NOT EXISTS (SELECT 1 FROM portier_attente pa2 WHERE pa2.address = o.sender_norm)))`);
console.log("rattrapage drapeaux:", (Number(process.hrtime.bigint() - t0) / 1e6).toFixed(1), "ms,",
  db.prepare("SELECT COUNT(*) n FROM threads WHERE organise_hors=1").get().n, "fils hors Reception");
// L'index partiel MIROIR de idx_threads_date_globale.
t0 = process.hrtime.bigint();
db.exec(`CREATE INDEX IF NOT EXISTS idx_spike_date_organise ON threads(last_epoch DESC, last_uid DESC, account_id)
         WHERE inbox_size > 0 AND organise_hors = 0`);
console.log("index partiel:", (Number(process.hrtime.bigint() - t0) / 1e6).toFixed(1), "ms");

const SELECT_UNIFIED = "SELECT a.id, a.email, e.uid, e.subject, e.sender, e.sender_address, e.message_id, e.date_epoch, e.seen, e.flagged, (SELECT COUNT(*) FROM attachments att WHERE att.mailbox_id = e.mailbox_id AND att.uid = e.uid), e.thread_id, e.in_reply_to, m.name, b.preview, e.to_addrs, e.cc_addrs, t.size, t.unseen";
const PINNED_THREADS = "SELECT pe.thread_id FROM pins p CROSS JOIN envelopes pe ON pe.mailbox_id = p.mailbox_id AND pe.uid = p.uid WHERE pe.thread_id IS NOT NULL";
const TAIL = ` JOIN envelopes e ON e.mailbox_id = t.last_mailbox_id AND e.uid = t.last_uid
 JOIN mailboxes m ON m.id = e.mailbox_id JOIN accounts a ON a.id = t.account_id
 LEFT JOIN bodies b ON b.mailbox_id = e.mailbox_id AND b.uid = e.uid
 ORDER BY t.last_epoch DESC, t.last_uid DESC, a.id`;
const page = (f) => `${SELECT_UNIFIED} FROM (SELECT account_id, last_mailbox_id, last_uid, last_epoch, size, unseen
  FROM threads WHERE inbox_size > 0 AND id NOT IN (${PINNED_THREADS})${f}
  ORDER BY last_epoch DESC, last_uid DESC, account_id LIMIT ?1 OFFSET ?2) t${TAIL}`;
const cas = [
  ["U0   temoin offset 0", page(""), [50, 0]],
  ["U0d  temoin offset 100000", page(""), [50, 100000]],
  ["UV4  drapeau offset 0", page(" AND organise_hors = 0"), [50, 0]],
  ["UV4d drapeau offset 100000", page(" AND organise_hors = 0"), [50, 100000]],
  ["C0   count temoin", `SELECT COUNT(*) FROM threads WHERE inbox_size > 0 AND id NOT IN (${PINNED_THREADS})`, []],
  ["CV4  count drapeau", `SELECT COUNT(*) FROM threads WHERE inbox_size > 0 AND organise_hors = 0 AND id NOT IN (${PINNED_THREADS})`, []],
];
for (const [nom, sql, params] of cas) {
  const stmt = db.prepare(sql);
  for (let i = 0; i < 5; i++) stmt.all(...params);
  const t = [];
  for (let i = 0; i < 20; i++) { const a = process.hrtime.bigint(); stmt.all(...params); t.push(Number(process.hrtime.bigint() - a) / 1e6); }
  t.sort((x, y) => x - y);
  const plan = db.prepare("EXPLAIN QUERY PLAN " + sql).all(...params).map(r => r.detail).filter(d => d.includes("threads")).join(" | ");
  console.log(`${nom}  mediane=${((t[9]+t[10])/2).toFixed(3)} ms  p95=${t[18].toFixed(3)} ms   [${plan}]`);
}
// Entretien : recomputer le drapeau d'UN fil (arrivee d'un message).
const recompute = db.prepare(`UPDATE threads SET organise_hors = (
  EXISTS (SELECT 1 FROM envelopes te JOIN routage_expediteurs r ON r.address = te.sender_norm
           AND r.destination <> 'reception' WHERE te.thread_id = threads.id)
  OR NOT EXISTS (SELECT 1 FROM envelopes o WHERE o.thread_id = threads.id
       AND NOT EXISTS (SELECT 1 FROM portier_attente pa WHERE pa.address = o.sender_norm))
  ) WHERE id = ?1`);
const ids = db.prepare("SELECT id FROM threads ORDER BY RANDOM() LIMIT 500").all().map(r => r.id);
t0 = process.hrtime.bigint();
db.exec("BEGIN");
for (const id of ids) recompute.run(id);
db.exec("COMMIT");
console.log("recompute drapeau x500 fils:", (Number(process.hrtime.bigint() - t0) / 1e6).toFixed(2), "ms");
// Decision Oui/Non : tous les fils d'UN expediteur (le plus gros).
const gros = db.prepare("SELECT sender_norm a, COUNT(DISTINCT thread_id) n FROM envelopes GROUP BY sender_norm ORDER BY n DESC LIMIT 1").get();
t0 = process.hrtime.bigint();
db.exec("BEGIN");
const fils = db.prepare("SELECT DISTINCT thread_id FROM envelopes WHERE sender_norm = ?1 AND thread_id IS NOT NULL").all(gros.a);
for (const f of fils) recompute.run(f.thread_id);
db.exec("COMMIT");
console.log(`decision sur le plus gros expediteur (${gros.a}, ${gros.n} fils):`, (Number(process.hrtime.bigint() - t0) / 1e6).toFixed(1), "ms");
db.close();

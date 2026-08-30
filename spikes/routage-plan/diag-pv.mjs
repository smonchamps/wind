import { DatabaseSync } from "node:sqlite";
const db = new DatabaseSync("spikes/routage-plan/banc.db");
const cas = [
  ["PV2n page Portier (attente -> dernier message, via sender_norm)",
   `SELECT pa.address,
      (SELECT e.rowid FROM envelopes e WHERE e.sender_norm = pa.address ORDER BY e.date_epoch DESC LIMIT 1) dernier,
      (SELECT COUNT(*) FROM envelopes e2 WHERE e2.sender_norm = pa.address) n
    FROM portier_attente pa ORDER BY 2 DESC`],
  ["NV2n pastille (nombre de MESSAGES en attente)",
   `SELECT COALESCE(SUM((SELECT COUNT(*) FROM envelopes e WHERE e.sender_norm = pa.address)), 0) FROM portier_attente pa`],
];
for (const [nom, sql] of cas) {
  const stmt = db.prepare(sql);
  for (let i = 0; i < 5; i++) stmt.all();
  const t = [];
  for (let i = 0; i < 20; i++) { const a = process.hrtime.bigint(); stmt.all(); t.push(Number(process.hrtime.bigint() - a) / 1e6); }
  t.sort((x, y) => x - y);
  const plan = db.prepare("EXPLAIN QUERY PLAN " + sql).all().map(r => r.detail).join(" | ");
  console.log(`${nom}  mediane=${((t[9]+t[10])/2).toFixed(3)} ms  p95=${t[18].toFixed(3)}\n  [${plan}]`);
}
db.close();

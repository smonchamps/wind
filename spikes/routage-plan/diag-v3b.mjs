import { DatabaseSync } from "node:sqlite";
const db = new DatabaseSync("spikes/routage-plan/banc.db");
const epoque = 1606000060;
let t0 = process.hrtime.bigint();
try { db.exec("ALTER TABLE envelopes ADD COLUMN sender_norm TEXT GENERATED ALWAYS AS (lower(trim(sender_address))) VIRTUAL"); } catch {}
console.log("alter:", (Number(process.hrtime.bigint() - t0) / 1e6).toFixed(1), "ms");
t0 = process.hrtime.bigint();
db.exec("CREATE INDEX IF NOT EXISTS idx_spike_norm2 ON envelopes(sender_norm, date_epoch)");
console.log("index:", (Number(process.hrtime.bigint() - t0) / 1e6).toFixed(1), "ms");
const FILS_ROUTES_AILLEURS = `SELECT te.thread_id FROM routage_expediteurs r
  CROSS JOIN envelopes te ON te.sender_norm = r.address
  WHERE r.destination <> 'reception' AND te.thread_id IS NOT NULL`;
const FILS_RETENUS = `SELECT ta.thread_id FROM portier_attente pa
  CROSS JOIN envelopes ta ON ta.sender_norm = pa.address
  WHERE ta.thread_id IS NOT NULL
    AND NOT EXISTS (SELECT 1 FROM envelopes o WHERE o.thread_id = ta.thread_id
      AND NOT EXISTS (SELECT 1 FROM portier_attente pa2 WHERE pa2.address = o.sender_norm))`;
for (const [nom, sql] of [["routes-ailleurs", FILS_ROUTES_AILLEURS], ["retenus", FILS_RETENUS]]) {
  const stmt = db.prepare(sql);
  for (let i = 0; i < 3; i++) stmt.all();
  const t = [];
  for (let i = 0; i < 10; i++) { const a = process.hrtime.bigint(); stmt.all(); t.push(Number(process.hrtime.bigint() - a) / 1e6); }
  t.sort((x, y) => x - y);
  console.log(nom, stmt.all().length, "lignes, mediane", t[5].toFixed(2), "ms");
  console.log("  " + db.prepare("EXPLAIN QUERY PLAN " + sql).all().map(r => r.detail).join("\n  "));
}
db.close();

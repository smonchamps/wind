// Spike D-53: the table from raw/*.jsonl — per run and median, MB
// (private working set, one instance) per phase, plus scroll cost.
//   node spikes/feed-memory/summary.mjs [ko]
import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';

const ko = Number(process.argv[2] ?? 100);
const raw = path.join(import.meta.dirname, 'raw');
const runs = {};
for (const f of readdirSync(raw)) {
  const m = f.match(/^([ABC])-(\d+)k-run(\d+)\.jsonl$/);
  if (!m || Number(m[2]) !== ko) continue;
  const lines = readFileSync(path.join(raw, f), 'utf8').trim().split('\n').map((l) => JSON.parse(l));
  const r = { mb: {}, detail: {}, scroll: null, state: {} };
  for (const l of lines) {
    if (l.mb !== undefined) { r.mb[l.phase] = l.mb; r.detail[l.phase] = l.detail; r.heap ??= {}; r.heap[l.phase] = l.jsHeapMb; }
    if (l.phase === 'scroll') r.scroll = l;
    if (l.cards !== undefined) r.state[l.phase] = l;
  }
  (runs[m[1]] ??= []).push({ run: Number(m[3]), ...r });
}
const median = (xs) => { const s = [...xs].sort((a, b) => a - b); const n = s.length; return n ? (n % 2 ? s[(n - 1) / 2] : (s[n / 2 - 1] + s[n / 2]) / 2) : NaN; };
const phases = ['rest', 'feed-page-1', 'feed-10-pages', 'back-inbox', 'back-inbox-25s', 'back-inbox-after-gc'];
const fmt = (x) => (Number.isFinite(x) ? x.toFixed(1) : '—');
console.log(`## ${ko} KB bodies — private working set, MB (one instance, all processes)\n`);
console.log(`| Option | run | ${phases.join(' | ')} |`);
console.log(`|---|---|${phases.map(() => '---:').join('|')}|`);
for (const opt of Object.keys(runs).sort()) {
  const rs = runs[opt].sort((a, b) => a.run - b.run);
  for (const r of rs) console.log(`| ${opt} | ${r.run} | ${phases.map((p) => fmt(r.mb[p])).join(' | ')} |`);
  console.log(`| **${opt}** | **median** | ${phases.map((p) => `**${fmt(median(rs.map((r) => r.mb[p])))}**`).join(' | ')} |`);
}
console.log('\n### Where (renderer / gpu-process, MB, per run)\n');
console.log('| Option | run | page 1 renderer / gpu | 10 pages renderer / gpu | +25 s renderer / gpu | after GC renderer / gpu | JS heap MB (p1 / 10p / +25 s / GC) |');
console.log('|---|---|---:|---:|---:|---:|---|');
const role = (d, r) => fmt(d?.find((x) => x.role === r)?.mb);
for (const opt of Object.keys(runs).sort()) {
  for (const r of runs[opt]) {
    const hp = ['feed-page-1', 'feed-10-pages', 'back-inbox-25s', 'back-inbox-after-gc'];
    console.log(`| ${opt} | ${r.run} | ${hp.map((p) => `${role(r.detail[p], 'renderer')} / ${role(r.detail[p], 'gpu-process')}`).join(' | ')} | ${hp.map((p) => r.heap?.[p] ?? '—').join(' / ')} |`);
  }
}
console.log('\n### Scroll cost (160 cards = 8 pages of 20)\n');
console.log('| Option | run | cards / live iframes / dormant at 10 pages | total scroll ms | page arrival ms (each) | frames | long frames (>50 ms) | max frame ms |');
console.log('|---|---|---|---:|---|---:|---:|---:|');
for (const opt of Object.keys(runs).sort()) {
  for (const r of runs[opt]) {
    const s = r.scroll; const st = r.state['feed-10-pages'];
    if (!s) continue;
    const arrivals = s.pages.filter((p) => p.to > p.from).map((p) => p.ms).join(', ');
    console.log(`| ${opt} | ${r.run} | ${st?.cards} / ${st?.iframes} / ${st?.dormant} | ${s.scrollMs} | ${arrivals} | ${s.frames} | ${s.longFrames} (${s.longFramesMs} ms) | ${s.maxFrameMs} |`);
  }
}

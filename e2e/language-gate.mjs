// Language gate (PLAN-BASCULE-ANGLAIS E1a): the repository is moving from
// French to English, and this net is the ratchet that makes the move
// one-way. It counts French markers per tracked text file — unambiguous
// French function words and accented letters — and compares them with
// `e2e/language-baseline.json`. Any file whose count RISES above its
// baseline (a file absent from the baseline has a baseline of 0) turns
// the gate red. Each step of the switch lowers the baseline with
// `--update`; once every count is 0 the ratchet is an absolute check.
//
//   node e2e/language-gate.mjs            -> verdict, exit 1 on any rise
//   node e2e/language-gate.mjs --update   -> rewrite the baseline
//
// Exempt by decision (PLAN-BASCULE-ANGLAIS D1, D3, D11 and §2): the French
// catalogue, BETA.fr.md, docs/archives/, the rename toolkit, lock files,
// and any line carrying the marker `lang:fr` (a deliberately French
// string, such as a French notification text or a French UI label a
// spec asserts). This file exempts itself: it holds the word list.
import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import path from 'node:path';

const root = path.resolve(import.meta.dirname, '..');
const baselinePath = path.join(root, 'e2e', 'language-baseline.json');
const update = process.argv.includes('--update');

const TEXT = /\.(rs|js|mjs|svelte|css|ps1|py|md|html|yml|yaml|toml|sh|json|txt)$/i;
const EXEMPT = [
  /^docs\/archives\//,
  /^spikes\//, // throw-away, outside the switch (PLAN-BASCULE-ANGLAIS §5)
  /^scripts\/rename\//,
  /(^|\/)catalogue?\.fr\.js$/,
  /(^|\/)catalog\.fr\.js$/,
  /^docs\/BETA\.fr\.md$/,
  /(^|\/)package-lock\.json$/,
  /^Cargo\.lock$/,
  /^e2e\/language-gate\.mjs$/,
  /^e2e\/language-baseline\.json$/,
];

// Function words that are French and only French in this repository's
// text — no English homograph in common use (so not: plus, on, en, a,
// son, par, non, si, y, et, un, de).
const WORDS = 'le la les des une du au aux est sont était été être pas ne pour dans avec sans sur sous que qui dont où ce cet cette ces cela ça ceci mais ou donc car ni alors ainsi aussi très trop peu jamais toujours encore déjà après avant depuis jusqu pendant chez vers entre parmi selon chaque plusieurs quelques tout tous toute toutes rien elle elles ils nous vous leur leurs notre nos votre vos mes tes ses lui eux même mêmes comme quand lorsque puisque parce sinon voici voilà fait faire faut peut doit sera seront avoir avait là'
  .split(' ');
const WORD_RE = new RegExp(`(^|[^\\p{L}\\p{N}_'’-])(${WORDS.join('|')})(?=$|[^\\p{L}\\p{N}_-])`, 'giu');
const ACCENT_RE = /[àâäéèêëîïôöùûüçœÀÂÄÉÈÊËÎÏÔÖÙÛÜÇŒ«»]/gu;

const tracked = execFileSync('git', ['ls-files', '-z'], { cwd: root, encoding: 'utf8' })
  .split('\0')
  .filter((f) => f && TEXT.test(f) && !EXEMPT.some((re) => re.test(f)));

export function countFrench(text) {
  let n = 0;
  for (const line of text.split('\n')) {
    if (line.includes('lang:fr')) continue;
    n += (line.match(ACCENT_RE) ?? []).length;
    n += (line.match(WORD_RE) ?? []).length;
  }
  return n;
}

const counts = {};
for (const f of tracked) {
  const abs = path.join(root, f);
  if (!existsSync(abs)) continue;
  const n = countFrench(readFileSync(abs, 'utf8'));
  if (n > 0) counts[f] = n;
}
const total = Object.values(counts).reduce((a, b) => a + b, 0);

if (update) {
  const sorted = Object.fromEntries(Object.entries(counts).sort(([a], [b]) => (a < b ? -1 : 1)));
  writeFileSync(baselinePath, JSON.stringify(sorted, null, 2) + '\n');
  console.log(`baseline written: ${Object.keys(counts).length} files, ${total} French markers`);
  process.exit(0);
}

const baseline = existsSync(baselinePath) ? JSON.parse(readFileSync(baselinePath, 'utf8')) : {};
let failures = 0;
for (const [f, n] of Object.entries(counts)) {
  const ref = baseline[f] ?? 0;
  if (n > ref) {
    failures += 1;
    console.log(`FAIL ${f}: ${n} French markers, baseline ${ref}`);
  }
}
const baseTotal = Object.values(baseline).reduce((a, b) => a + b, 0);
console.log(
  `language gate: ${tracked.length} files scanned, ${Object.keys(counts).length} with French ` +
    `(${total} markers, baseline ${baseTotal})${failures ? ` — ${failures} file(s) above baseline` : ' — no rise'}`,
);
process.exit(failures ? 1 : 0);

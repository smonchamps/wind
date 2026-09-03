// PLAN-BASCULE-ANGLAIS E5b (2026-09-03): the `{placeholders}` of the
// catalogue VALUES are a bridge with the `t(key, { param })` keys of the
// UI — `t()` substitutes an empty string for an unmatched placeholder,
// silently, in both languages. Nine e2e specs caught it the day the
// param keys were renamed on one side only (empty names, organizers,
// dates). This net reads every `t('literal.key', { … })` call whose
// argument is an object literal, lists its keys, and refuses a value
// placeholder the call does not provide. Calls whose key or argument is
// not a literal are out of its reach (the e2e keeps them).
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync, statSync } from 'node:fs';
import path from 'node:path';
import { FR } from '../apps/desktop/ui-v2/src/lib/catalog.fr.js';
import { EN } from '../apps/desktop/ui-v2/src/lib/catalog.en.js';

const root = path.resolve(import.meta.dirname, '..', 'apps', 'desktop', 'ui-v2', 'src');
function walk(d, out = []) {
  for (const f of readdirSync(d)) {
    const p = path.join(d, f);
    if (statSync(p).isDirectory()) walk(p, out);
    else if (/\.(js|svelte)$/.test(f) && !/catalog\./.test(f)) out.push(p);
  }
  return out;
}

// `t('a.b', { x, y: z, n: 3 })` → ['x', 'y', 'n']; a spread or a computed
// key makes the call unreadable here (skipped, counted).
const CALL = /\bt\(\s*'([a-z_]+\.[a-zA-Z0-9_.-]+)'\s*,\s*\{([^{}]*)\}\s*\)/g;
const keysOf = (arg) => {
  if (arg.includes('...') || arg.includes('[')) return null;
  return arg.split(',').map((p) => p.trim()).filter(Boolean).map((p) => p.split(':')[0].trim());
};
const placeholders = (value) => [...String(value).matchAll(/\{(\w+)\}/g)].map((m) => m[1]);

const calls = [];
for (const f of walk(root)) {
  const src = readFileSync(f, 'utf8');
  for (const m of src.matchAll(CALL)) calls.push({ file: path.basename(f), key: m[1], params: keysOf(m[2]) });
}

test('the net reads the calls it is meant to read', () => {
  assert.ok(calls.length > 100, `only ${calls.length} literal t(key, {…}) calls read`);
});

for (const [name, table] of [['FR', FR], ['EN', EN]]) {
  test(`catalogue ${name}: every {placeholder} is provided by the calls that use the key`, () => {
    const gaps = [];
    for (const { file, key, params } of calls) {
      if (params === null || typeof table[key] !== 'string') continue;
      for (const ph of placeholders(table[key])) {
        if (!params.includes(ph)) gaps.push(`${file}: t('${key}', {${params.join(', ')}}) lacks {${ph}}`);
      }
    }
    assert.deepEqual(gaps, [], gaps.join('\n'));
  });
}

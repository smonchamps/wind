// IPC contract net (PLAN-BASCULE-ANGLAIS E1b): the Tauri commands are a
// string boundary the compiler cannot see. The shell registers them in
// `generate_handler![…]` (apps/desktop/src/main.rs); the UI calls them by
// name through `appel('…')` (lib/transport.js); the specs call a few by
// name through `invoke('…')`. A renamed command that one side forgets is
// a rejection at run time, found by an e2e or in the field — this net
// finds it in seconds.
//
//   node e2e/ipc-contract.mjs   -> verdict, exit 1 on any mismatch
//
// Red when: a name called by the UI or the specs is not registered; a
// registered name has no `#[tauri::command]` function; a `#[tauri::command]`
// function is not registered. Registered-but-uncalled names are listed
// as information only (a command may be reserved for the specs).
import { readFileSync, readdirSync, statSync } from 'node:fs';
import path from 'node:path';

const root = path.resolve(import.meta.dirname, '..');

function walk(dir, exts, out = []) {
  for (const entry of readdirSync(dir)) {
    const p = path.join(dir, entry);
    if (entry === 'node_modules' || entry === 'dist' || entry === 'test-results') continue;
    if (statSync(p).isDirectory()) walk(p, exts, out);
    else if (exts.some((e) => p.endsWith(e))) out.push(p);
  }
  return out;
}

const shell = walk(path.join(root, 'apps', 'desktop', 'src'), ['.rs']).map((p) => readFileSync(p, 'utf8'));
const main = readFileSync(path.join(root, 'apps', 'desktop', 'src', 'main.rs'), 'utf8');

const handlerBlock = main.match(/generate_handler!\[([\s\S]*?)\]/);
if (!handlerBlock) {
  console.log('FAIL generate_handler![…] not found in main.rs');
  process.exit(1);
}
// One entry per line: `commands::name,` or `some::module::name,` — the
// last path segment is the command name; comments are dropped first.
const registered = new Set(
  [...handlerBlock[1].replace(/\/\/[^\n]*/g, '').matchAll(/(?:[a-z_][a-z0-9_]*::)*([a-z_][a-z0-9_]*)\s*(?:,|$)/gm)].map((m) => m[1]),
);

const defined = new Set();
for (const src of shell) {
  // Other attributes and comment lines may sit between `#[tauri::command]`
  // and the fn (`queue_send`: a comment, then `#[allow(clippy::…)]`).
  for (const m of src.matchAll(/#\[tauri::command[^\]]*\](?:\s*(?:#\[[^\]]*\]|\/\/[^\n]*))*\s*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+([a-z_][a-z0-9_]*)/g)) defined.add(m[1]);
}

const ui = walk(path.join(root, 'apps', 'desktop', 'ui-v2', 'src'), ['.js', '.svelte']).map((p) => readFileSync(p, 'utf8')).join('\n');
const specs = walk(path.join(root, 'e2e'), ['.js', '.mjs']).map((p) => readFileSync(p, 'utf8')).join('\n');
const called = new Set([
  ...[...ui.matchAll(/\bcall\(\s*['"]([a-z_][a-z0-9_]*)['"]/g)].map((m) => m[1]),
  ...[...specs.matchAll(/\b(?:appel|call|invoke)\(\s*['"]([a-z_][a-z0-9_]*)['"]/g)].map((m) => m[1]),
]);

let failures = 0;
for (const n of called) if (!registered.has(n)) { failures += 1; console.log(`FAIL called but not registered: ${n}`); }
for (const n of registered) if (!defined.has(n)) { failures += 1; console.log(`FAIL registered but no #[tauri::command] fn: ${n}`); }
for (const n of defined) if (!registered.has(n)) { failures += 1; console.log(`FAIL #[tauri::command] fn not registered: ${n}`); }
const uncalled = [...registered].filter((n) => !called.has(n)).sort();
console.log(
  `ipc contract: ${defined.size} commands defined, ${registered.size} registered, ${called.size} called by name` +
    (uncalled.length ? `; registered but never called by name: ${uncalled.join(', ')}` : ''),
);
process.exit(failures ? 1 : 0);

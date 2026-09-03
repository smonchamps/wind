// The scanner the appliers of PLAN-BASCULE-ANGLAIS share (E6a, the
// cleanup E5d deferred): the JS tokenizer E5b proved on the UI
// (`apply-ui.mjs`), the CSV reader and the tree walker. It renames WHOLE
// identifiers only, and never inside string literals, template-literal
// text, comments or regex literals — a bare `\bword\b` pass reached all
// of those (the E5a lesson).
import { readFileSync, readdirSync, statSync } from 'node:fs';
import path from 'node:path';

const ID_START = /[A-Za-z_$]/, ID = /[\w$]/;

// A repository-relative path with forward slashes (the report key).
export const relTo = (root) => (f) => path.relative(root, f).replace(/\\/g, '/');

// `scripts/rename/<file>` as rows of cells (no quoting: the tables carry
// none), the header included.
export function csvRows(root, file) {
  return readFileSync(path.join(root, 'scripts', 'rename', file), 'utf8')
    .split(/\r?\n/).filter(Boolean).map((l) => l.split(','));
}

export function walk(d, re, out = []) {
  for (const f of readdirSync(d)) {
    if (f === 'node_modules' || f === 'test-results' || f === 'target') continue;
    const p = path.join(d, f);
    if (statSync(p).isDirectory()) walk(p, re, out);
    else if (re.test(f)) out.push(p);
  }
  return out;
}

// Renames identifiers in a JS source from `ctx`:
//   dict      Map old → new (identifiers)
//   keys      Map old → new (string literals exactly equal to a key — the
//             catalogue keys of E5b; empty for the e2e layer)
//   prefixes  [[oldHead, newHead]] applied to the head of a template
//             literal (`\`boite.${dest}\``), longest first
//   found     { strings, dataset } — the hand-review lists: a string
//             literal equal to a dictionary word (a value that bridges a
//             renamed object key), an identifier read after `dataset.`
//             (an attribute name, never renamed)
//   keep      (file, out, src, j, word) → true to leave the word as it is
// `stop` (a char) ends the scan at depth 0 (the closing brace of a Svelte
// mustache). Returns [output, index].
export function scanJs(src, i, stop, file, ctx) {
  const dict = ctx.dict, keys = ctx.keys ?? new Map(), prefixes = ctx.prefixes ?? [];
  const found = ctx.found ?? { strings: [], dataset: [] };
  let out = '';
  let depth = 0;
  let prev = ''; // last significant char, for the regex-literal heuristic
  const N = src.length;
  while (i < N) {
    const c = src[i];
    if (stop && c === stop && depth === 0) return [out, i];
    // `{/if}`, `{/each}`: a block close, not a regex literal
    if (c === '/' && out === '' && stop === '}') { out += c; i++; continue; }
    if (c === '/' && src[i + 1] === '/') { const e = src.indexOf('\n', i); const j = e < 0 ? N : e; out += src.slice(i, j); i = j; continue; }
    if (c === '/' && src[i + 1] === '*') { const e = src.indexOf('*/', i + 2); const j = e < 0 ? N : e + 2; out += src.slice(i, j); i = j; continue; }
    if (c === "'" || c === '"') {
      let j = i + 1;
      while (j < N && src[j] !== c) { if (src[j] === '\\') j++; j++; }
      const lit = src.slice(i + 1, j);
      if (dict.has(lit)) found.strings.push(`${file}: '${lit}' (→ ${dict.get(lit)})  …${src.slice(Math.max(0, i - 45), j + 12).replace(/\s+/g, ' ')}…`);
      out += c + (keys.get(lit) ?? lit) + c;
      i = j + 1; prev = c; continue;
    }
    if (c === '`') {
      out += c; i++;
      let text = '';
      const flush = () => {
        // the head of the template may be a catalogue-key prefix
        if (out.endsWith('`')) for (const [o, n] of prefixes) if (text.startsWith(o)) { text = n + text.slice(o.length); break; }
        out += text; text = '';
      };
      while (i < N && src[i] !== '`') {
        if (src[i] === '\\') { text += src[i] + src[i + 1]; i += 2; continue; }
        if (src[i] === '$' && src[i + 1] === '{') {
          flush(); out += '${';
          const [inner, j] = scanJs(src, i + 2, '}', file, ctx);
          out += inner + '}'; i = j + 1; continue;
        }
        text += src[i++];
      }
      flush(); out += '`'; i++; prev = '`'; continue;
    }
    if (c === '/' && !/[\w$)\]]/.test(prev)) { // regex literal
      let j = i + 1, cls = false;
      while (j < N && (cls || src[j] !== '/')) { if (src[j] === '\\') j++; else if (src[j] === '[') cls = true; else if (src[j] === ']') cls = false; j++; }
      while (ID.test(src[j + 1] ?? '')) j++;
      out += src.slice(i, j + 1); i = j + 1; prev = '/'; continue;
    }
    if (ID_START.test(c)) {
      let j = i + 1;
      while (j < N && ID.test(src[j])) j++;
      const word = src.slice(i, j);
      const afterDataset = out.endsWith('dataset.');
      if (ctx.keep && ctx.keep(file, out, src, j, word)) { out += word; i = j; prev = word[word.length - 1]; continue; }
      if (afterDataset && dict.has(word)) found.dataset.push(`${file}: dataset.${word}`);
      out += !afterDataset && dict.has(word) ? dict.get(word) : word;
      i = j; prev = word[word.length - 1]; continue;
    }
    if (c === '{') depth++;
    else if (c === '}') depth--;
    if (!/\s/.test(c)) prev = c;
    out += c; i++;
  }
  return [out, i];
}

// Relative import paths that name a renamed file (`files`: old → new,
// paths relative to one directory): `'../lib/old.js'` → `'../lib/new.js'`.
export function rewriteImportPaths(src, files) {
  for (const [from, to] of Object.entries(files)) {
    const base = path.basename(from), nb = path.basename(to);
    if (base === nb) continue;
    src = src.replace(new RegExp(`(['"\`][^'"\`]*/)${base.replace(/\./g, '\\.')}(['"\`])`, 'g'), `$1${nb}$2`);
  }
  return src;
}

// Shadowing risk, for the hand-review list: a `new` name already declared
// in a file that also declares `old` — the rename would merge two
// meanings silently.
export function findShadowing(files, dict, rel) {
  const out = [];
  for (const f of files) {
    const src = readFileSync(f, 'utf8');
    for (const [old, nw] of dict) {
      if (!/^[a-z]/.test(old) || !src.includes(old) || !src.includes(nw)) continue;
      const decl = (w) => new RegExp(`\\b(let|const|var|function|import)\\s+(\\{[^}]*\\b)?${w}\\b|\\(([^)]*,\\s*)?${w}\\s*[,)=]`).test(src);
      if (decl(old) && decl(nw)) out.push(`${rel(f)}: ${old} → ${nw}, and ${nw} is already declared`);
    }
  }
  return out;
}

// A whole JS file (a `.mjs` tool, a spec, a `lib/` module).
export function renameJs(src, dict, file = '', found = { strings: [], dataset: [] }) {
  return scanJs(src, 0, null, file, { dict, found })[0];
}

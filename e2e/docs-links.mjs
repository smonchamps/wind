// Markdown link net (PLAN-ENGLISH-SWITCH E1d): the switch renames some
// 115 files — ADR slugs, plans, scripts, specs — and 71 relative links
// plus every `[…](path)` in the skills point at them. No link checker
// existed; a dead link in STANDARD or WORKFLOW is a session that starts
// on a 404. This net resolves every relative markdown link of every
// tracked `.md` file (and CLAUDE.md, the skills, the agent) and turns
// red on the first target that does not exist.
//
//   node e2e/docs-links.mjs   -> verdict, exit 1 on any dead link
//
// Anchors (`#…`) are stripped; URLs with a scheme and mailto are skipped;
// links inside fenced code blocks are skipped (they are examples).
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';

const root = path.resolve(import.meta.dirname, '..');
const files = execFileSync('git', ['ls-files', '-z', '*.md', '**/*.md'], { cwd: root, encoding: 'utf8' })
  .split('\0')
  .filter((f) => f && !f.startsWith('docs/archives/') && !f.startsWith('spikes/') && !f.includes('node_modules/'));

const LINK = /\[[^\]]*\]\(([^)\s]+)(?:\s+"[^"]*")?\)/g;
let checked = 0;
let failures = 0;
for (const f of files) {
  const text = readFileSync(path.join(root, f), 'utf8');
  let fenced = false;
  for (const line of text.split('\n')) {
    if (/^\s*```/.test(line)) { fenced = !fenced; continue; }
    if (fenced) continue;
    for (const m of line.matchAll(LINK)) {
      let target = m[1];
      if (/^[a-z][a-z0-9+.-]*:/i.test(target) || target.startsWith('#')) continue;
      target = decodeURIComponent(target.replace(/#.*$/, ''));
      if (!target) continue;
      checked += 1;
      const abs = path.resolve(root, path.dirname(f), target);
      if (!existsSync(abs)) {
        failures += 1;
        console.log(`FAIL ${f}: dead link -> ${m[1]}`);
      }
    }
  }
}
console.log(`docs links: ${files.length} markdown files, ${checked} relative links checked${failures ? `, ${failures} dead` : ', none dead'}`);
process.exit(failures ? 1 : 0);

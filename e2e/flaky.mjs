// Le compte des flaky du dernier run e2e, lu dans le rapport JSON de
// Playwright (PLAN-AUDIT-V2 E9, D4) : gate.ps1 l'imprime au verdict —
// le chiffre que la decision failOnFlakyTests attendait.
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';

const rapport = path.resolve(import.meta.dirname, 'test-results', 'rapport.json');
if (!existsSync(rapport)) {
  console.log('flaky : rapport absent');
  process.exit(0);
}
const flaky = [];
const visiter = (suite, chemin) => {
  for (const spec of suite.specs ?? []) {
    for (const test of spec.tests ?? []) {
      if (test.status === 'flaky') flaky.push(`${chemin}${spec.file}:${spec.line} › ${spec.title}`);
    }
  }
  for (const sous of suite.suites ?? []) visiter(sous, chemin);
};
for (const suite of JSON.parse(readFileSync(rapport, 'utf8')).suites ?? []) visiter(suite, '');
console.log(`flaky : ${flaky.length}`);
for (const nom of flaky) console.log(`  ${nom}`);

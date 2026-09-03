// The count of flaky tests from the last e2e run, read from Playwright's
// JSON report (PLAN-AUDIT-V2 E9, D4): gate.ps1 prints it at the verdict —
// the figure the failOnFlakyTests decision was waiting for.
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';

const report = path.resolve(import.meta.dirname, 'test-results', 'report.json');
if (!existsSync(report)) {
  console.log('flaky: report absent');
  process.exit(0);
}
const flaky = [];
const walk = (suite, prefix) => {
  for (const spec of suite.specs ?? []) {
    for (const test of spec.tests ?? []) {
      if (test.status === 'flaky') flaky.push(`${prefix}${spec.file}:${spec.line} › ${spec.title}`);
    }
  }
  for (const under of suite.suites ?? []) walk(under, prefix);
};
for (const suite of JSON.parse(readFileSync(report, 'utf8')).suites ?? []) walk(suite, '');
console.log(`flaky: ${flaky.length}`);
for (const name of flaky) console.log(`  ${name}`);

// The "OS follow" test (redesign-screen02) switches the MACHINE's
// theme and restores it in a `finally` — unless the runner is killed
// mid-flight. It first writes the initial value into this witness; if it
// still exists here, we restore it (PLAN-AUDIT-V2 E9, Chief Engineer decision D7).
import { execSync } from 'node:child_process';
import { existsSync, readFileSync, unlinkSync } from 'node:fs';
import path from 'node:path';

export const THEME_WITNESS = path.resolve(import.meta.dirname, 'test-results', 'theme-initial.txt');

export default function globalTeardown() {
  if (process.platform !== 'win32' || !existsSync(THEME_WITNESS)) return;
  const initial = readFileSync(THEME_WITNESS, 'utf8').trim();
  const script = path.resolve(import.meta.dirname, 'dark-toggle.ps1');
  try {
    execSync(`powershell -NoProfile -ExecutionPolicy Bypass -File "${script}" -v ${initial}`);
    console.log(`Windows theme restored (AppsUseLightTheme=${initial}) by the teardown`);
  } finally {
    unlinkSync(THEME_WITNESS);
  }
}

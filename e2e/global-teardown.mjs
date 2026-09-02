// L'epreuve « suivi OS » (refonte-ecran02) bascule le theme de la
// MACHINE et le rend dans un `finally` — sauf si le runner est tue en
// plein vol. Elle ecrit d'abord la valeur initiale dans ce temoin ; s'il
// existe encore ici, on la restaure (PLAN-AUDIT-V2 E9, decision CE D7).
import { execSync } from 'node:child_process';
import { existsSync, readFileSync, unlinkSync } from 'node:fs';
import path from 'node:path';

export const TEMOIN_THEME = path.resolve(import.meta.dirname, 'test-results', 'theme-initial.txt');

export default function globalTeardown() {
  if (process.platform !== 'win32' || !existsSync(TEMOIN_THEME)) return;
  const initial = readFileSync(TEMOIN_THEME, 'utf8').trim();
  const script = path.resolve(import.meta.dirname, 'bascule-sombre.ps1');
  try {
    execSync(`powershell -NoProfile -ExecutionPolicy Bypass -File "${script}" -v ${initial}`);
    console.log(`theme Windows restaure (AppsUseLightTheme=${initial}) par le teardown`);
  } finally {
    unlinkSync(TEMOIN_THEME);
  }
}

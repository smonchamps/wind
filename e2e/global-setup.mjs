// Compile UNE fois les exemples de mail-core (les seeders des decors)
// avant toute spec — hors du timeout de spec (PLAN-AUDIT-V2 E9 : le
// `cargo build` vivait dans le premier `beforeAll`, sous les 180 s).
import { execSync } from 'node:child_process';
import path from 'node:path';

export default function globalSetup() {
  const root = path.resolve(import.meta.dirname, '..');
  execSync('cargo build -p mail-core --examples', { cwd: root, stdio: 'inherit' });
}

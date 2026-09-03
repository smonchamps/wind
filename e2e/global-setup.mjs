// Builds the mail-core examples (the decor seeders) ONCE
// before any spec — outside the spec timeout (PLAN-AUDIT-V2 E9: the
// `cargo build` used to live in the first `beforeAll`, under the 180 s).
import { execSync } from 'node:child_process';
import path from 'node:path';

export default function globalSetup() {
  const root = path.resolve(import.meta.dirname, '..');
  execSync('cargo build -p mail-core --examples', { cwd: root, stdio: 'inherit' });
}

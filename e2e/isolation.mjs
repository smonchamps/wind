// OAuth isolation contract for the benches and e2e suites — the LIST lives in
// `isolation-oauth.json`, a single source shared with the Python
// tools (freeze-probe.py reads it as-is): a provider added in a single
// place covers every launcher.
//
// Why purge: with an OAuth client set in the environment, a
// test that touches the OAuth route would open the REAL browser
// consent — and stay stuck on it. Without these variables, no
// test can touch the real account, even by accident.
import { readFileSync } from 'node:fs';
import path from 'node:path';

export const OAUTH_VARIABLES = JSON.parse(
  readFileSync(path.join(import.meta.dirname, 'isolation-oauth.json'), 'utf8'),
);

export function purgeOAuth(env) {
  for (const variable of OAUTH_VARIABLES) delete env[variable];
}

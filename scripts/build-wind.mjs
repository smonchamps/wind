// Build Wind OUTSIDE the e2e harness with the SAME guards (ui-v2 rebuild,
// re-embedding of the dist when it changed — `generate_context!` does
// not see it by itself —, sweep of the bench zombies). The single home
// of these traps is e2e/rebuild-v2.mjs; this bridge makes it callable
// from PowerShell (scripts/run-wind.ps1) without duplicating the logic.
//
// Usage: node scripts/build-wind.mjs [--debug]
import path from 'node:path';
import { buildV2 } from '../e2e/rebuild-v2.mjs';

const root = path.resolve(import.meta.dirname, '..');
buildV2(root, { release: !process.argv.includes('--debug') });

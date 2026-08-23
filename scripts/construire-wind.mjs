// Construire Wind HORS du harnais e2e avec les MÊMES gardes (rebuild
// ui-v2, ré-embarquement du dist si changé — `generate_context!` ne le
// voit pas seul —, balayage des zombies de banc). La maison unique de
// ces pièges est e2e/rebuild-v2.mjs ; ce pont la rend appelable depuis
// PowerShell (scripts/lancer-wind.ps1) sans dupliquer la logique.
//
// Usage : node scripts/construire-wind.mjs [--debug]
import path from 'node:path';
import { construireV2 } from '../e2e/rebuild-v2.mjs';

const root = path.resolve(import.meta.dirname, '..');
construireV2(root, { release: !process.argv.includes('--debug') });

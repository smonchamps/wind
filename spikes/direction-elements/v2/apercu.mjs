// Aperçu de contrôle : rend UNE ou plusieurs parties seules, avec la
// même feuille de style, pour les regarder sans dérouler 25 000 px.
//
//   node spikes/direction-elements/v2/apercu.mjs ecran02 statut
//
// Jetable, hors du document. Sert la vérification visuelle, rien d'autre.
import { writeFileSync } from 'node:fs';
import path from 'node:path';
import { CSS } from './socle.mjs';
import { marqueSection, couleurs, themes, typographie, troncature, formes, kit } from './parties-1.mjs';
import { coins } from './parties-coins.mjs';
import { icones, ecran01, ecran02, barreEtat } from './parties-2.mjs';
import { ecran03, ecran04, reglages, migration, avis, ligneMessage } from './parties-3.mjs';

const PARTIES = {
  marque: marqueSection, couleurs, themes, typographie, troncature, formes, coins, kit,
  icones, ecran01, ecran02, statut: barreEtat,
  ecran03, ecran04, reglages, migration, avis, ligne: ligneMessage,
};

const demandes = process.argv.slice(2).filter((a) => !a.startsWith('--'));
const nuit = process.argv.includes('--nuit');
const noms = demandes.length ? demandes : ['ecran02'];
for (const n of noms) if (!PARTIES[n]) throw new Error(`partie inconnue : ${n} (parmi ${Object.keys(PARTIES).join(', ')})`);

const html = `<!DOCTYPE html>
<html lang="fr"${nuit ? ' data-theme="elements-nuit"' : ''}><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Aperçu — ${noms.join(', ')}${nuit ? ' · nuit' : ''}</title>
<style>${CSS}</style></head>
<body><div class="dc">${noms.map((n) => PARTIES[n]()).join('\n')}</div></body></html>
`;

const sortie = path.join(import.meta.dirname, 'apercu.html');
writeFileSync(sortie, html, 'utf8');
console.log(`${sortie} — ${noms.join(', ')}${nuit ? ' (nuit)' : ''} — ${(html.length / 1024).toFixed(0)} Ko`);

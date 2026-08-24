// Le sprite du prototype (index.html) ne doit pas diverger du catalogue
// (jeu.mjs). Il a dérivé DEUX FOIS pendant les tours de retours, en
// silence — un glyphe périmé s'affiche parfaitement, rien ne le signale.
//
//   node controle-sprite.mjs             vérifie, sort 1 s'il y a un écart
//   node controle-sprite.mjs --corriger  réécrit le sprite depuis jeu.mjs
//
// La correspondance est déclarée ici : le prototype nomme ses symboles par
// FONCTION (i-envoyes), le catalogue par glyphe Material (send).
import { readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { JEU } from './jeu.mjs';

const PONT = {
  'i-reception':'inbox', 'i-envoyes':'send', 'i-brouillons':'edit_note',
  'i-indesirables':'report', 'i-archives':'archive', 'i-corbeille':'delete',
  'i-toutes':'all_inbox', 'i-chercher':'search', 'i-reglages':'settings',
  'i-sync':'sync', 'i-repondre':'reply', 'i-repondretous':'reply_all',
  'i-epingler':'keep', 'i-fichier':'attach_file', 'i-fil':'forum',
  'i-nonlus':'mark_email_unread', 'i-deplier':'unfold_more',
  'i-replier':'unfold_less', 'i-ouvrir':'open_in_full', 'i-fermer':'close',
  'i-oui':'check_circle', 'i-peutetre':'question_mark',
};
// Hors pont, et pourquoi :
//   i-transferer — A12 : `reply` en symétrie verticale, aucun glyphe propre.
//   i-wind, i-wind-clair — la marque, hors inventaire (trait 2,3).
// Le prototype emploie UN seul symbole d'archive là où l'application en
// emploie deux (`archive` pour l'action, `inventory_2` pour le dossier) :
// c'est exactement la fusion que le chiffrage relève (§6, famille
// « archives »). Le pont pointe sur `archive`, et le prototype montre donc
// ce que coûterait la fusion si elle était tranchée.

const fichier = path.join(import.meta.dirname, 'index.html');
let html = readFileSync(fichier, 'utf8');
const corriger = process.argv.includes('--corriger');

let ecarts = 0, verifies = 0, corriges = 0;
for (const [sym, glyphe] of Object.entries(PONT)) {
  const motif = new RegExp(`(<symbol id="${sym}"[^>]*>)([^]*?)(</symbol>)`);
  const bloc = html.match(motif);
  if (!bloc) { console.log(`ECHEC ${sym} — symbole introuvable`); ecarts += 1; continue; }
  const dansJeu = JEU[glyphe]?.d ?? [];
  if (!dansJeu.length) { console.log(`ECHEC ${sym} — ${glyphe} absent du catalogue`); ecarts += 1; continue; }
  verifies += 1;

  // Le <g> porte le trait ; ce qui vit HORS du <g> (le disque teal de
  // `mark_email_unread`) n'est pas touché — ce n'est pas de la structure.
  const g = bloc[2].match(/(<g[^>]*>)([^]*?)(<\/g>)/);
  if (!g) { console.log(`ECHEC ${sym} — pas de <g> de trait`); ecarts += 1; continue; }

  const attendu = dansJeu.map((d) => `<path d="${d}"/>`).join('');
  const actuel = g[2].replace(/\s+/g, '');
  if (actuel === attendu.replace(/\s+/g, '')) continue;

  ecarts += 1;
  const lire = (s) => [...s.matchAll(/<(path|circle)[^>]*>/g)].map((m) => m[0]).join(' ');
  console.log(`ECART ${sym} (${glyphe})\n   prototype : ${lire(g[2])}\n   catalogue : ${attendu}`);
  if (corriger) {
    html = html.replace(motif, (_, a, corps, c) =>
      a + corps.replace(/(<g[^>]*>)([^]*?)(<\/g>)/, (__, ga, ___, gc) => ga + attendu + gc) + c);
    corriges += 1;
  }
}

if (corriger && corriges) {
  writeFileSync(fichier, html, 'utf8');
  console.log(`\n${corriges} symbole(s) réécrit(s) depuis le catalogue.`);
  process.exit(0);
}
console.log(`\n${verifies} symboles vérifiés, ${ecarts} écart(s)`);
process.exit(ecarts ? 1 : 0);

// ====================================================================
// Génère docs/design/systeme.v2.dc.html À PARTIR de jeu.mjs et du
// document livré (pour le journal des amendements, repris verbatim).
//
//   node spikes/direction-elements/v2/faire.mjs
//
// Pourquoi générer plutôt qu'écrire : un Système qui recopie à la main
// le catalogue de ses glyphes diverge au premier correctif, et un
// document qui recopie ses contrastes ment le jour où une valeur bouge.
// Ici les 78 glyphes viennent du catalogue, les mesures sont CALCULÉES
// à la génération, et le journal est relu à la source.
//
// TROIS gardes, et le script SORT EN ÉCHEC si l'une d'elles cède :
//   1. le relevé des icônes couvre le catalogue dans les DEUX sens
//      (A18 rendu mécanique — c'est l'écart de 10 glyphes du livré qui
//      a montré qu'une promesse ne suffit pas) ;
//   2. aucun contraste sous son seuil ;
//   3. le journal relu compte au moins 70 amendements — si la source
//      change de forme, on le sait au lieu de produire un trou.
// ====================================================================
import { writeFileSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { JEU } from '../jeu.mjs';
import { CSS, banc, bancReperes } from './socle.mjs';
import {
  enTete, sommaire, principes, marqueSection, couleurs, themes,
  typographie, troncature, formes, kit,
} from './parties-1.mjs';
import { coins } from './parties-coins.mjs';
import { icones, ecran01, ecran02, barreEtat, RELEVE } from './parties-2.mjs';
import {
  ecran03, ecran04, reglages, migration, avis, ligneMessage, journal,
} from './parties-3.mjs';

const root = path.resolve(import.meta.dirname, '..', '..', '..');
const SOURCE = path.join(root, 'docs', 'design', 'systeme.dc.html');
const SORTIE = path.join(root, 'docs', 'design', 'systeme.v2.dc.html');

const echecs = [];
const echec = (m) => { echecs.push(m); console.log(`ECHEC ${m}`); };

// --- Garde 1 : le relevé EST l'inventaire ----------------------------
// Le Système livré promettait A18 (« ce qu'il dessine est livré, ce qui
// est livré s'y dessine ») et l'avait perdu sur DIX glyphes sans le
// voir. Une promesse ne tient pas : celle-ci est une assertion.
{
  const dessines = new Set(Object.keys(JEU));
  const releves = new Set(RELEVE.map(([n]) => n));
  for (const n of dessines) {
    if (!releves.has(n)) echec(`glyphe « ${n} » dessiné au catalogue mais absent du relevé (A18)`);
  }
  for (const n of releves) {
    if (!dessines.has(n)) echec(`glyphe « ${n} » au relevé mais absent du catalogue (A18)`);
  }
}

// --- Le journal, relu à la source ------------------------------------
// Le repli à deux cellules reste EN PLACE bien que le défaut d'A48 soit
// réparé (V12) : il a coûté assez cher à trouver pour qu'on garde le
// filet. S'il se redéclenche un jour, le document le DIT au lieu de
// l'escamoter — une date restituée porte son astérisque.
function lireJournal() {
  const doc = readFileSync(SOURCE, 'utf8');
  const i = doc.indexOf('Journal des amendements');
  if (i < 0) throw new Error('journal introuvable dans le document livré');
  const tb = doc.slice(doc.indexOf('<tbody>', i) + 7, doc.indexOf('</tbody>', i));
  const rows = tb.split(/<tr\b[^>]*>/).slice(1).map((bloc) => {
    const tds = [...bloc.matchAll(/<td\b[^>]*>([\s\S]*?)<\/td>/g)].map((m) => m[1].trim());
    if (tds.length === 3) return { date: tds[0], ref: tds[1], texte: tds[2] };
    if (tds.length === 2) return { date: '2026-08-16', ref: tds[0], texte: tds[1], dateRestituee: true };
    throw new Error(`ligne de journal à ${tds.length} cellule(s) : ${bloc.slice(0, 90)}`);
  });
  if (rows.length < 70) echec(`${rows.length} amendements relus — la source a changé de forme`);
  return rows;
}

const rows = lireJournal();

// --- Normalisation sémantique ----------------------------------------
// Toute cellule d'en-tête de table porte scope="col" : la structure ne
// se devine pas, elle se déclare. Fait ici plutôt qu'à 8 endroits.
const normaliser = (html) => html.replace(/<thead>[\s\S]*?<\/thead>/g, (bloc) =>
  bloc.replace(/<th(?![^>]*\bscope=)/g, '<th scope="col"'));

// UNE bascule, hors produit : le thème. Celle des coins a été retirée
// une fois l'arbitrage clos (V14) — un Système qui offre deux états de
// sa propre règle est un Système qui n'a pas tranché.
const PILULE = `
<div class="pilules">
  <div class="pilule" id="pilule-theme" role="group" aria-label="Thème du document">
    <button data-th="elements" aria-pressed="true">Clair</button>
    <button data-th="elements-nuit" aria-pressed="false">Nuit</button>
  </div>
</div>
<script>
document.getElementById('pilule-theme').addEventListener('click', function (e) {
  var b = e.target.closest('button'); if (!b) return;
  Array.prototype.forEach.call(this.querySelectorAll('button'), function (x) {
    x.setAttribute('aria-pressed', String(x === b));
  });
  if (b.dataset.th === 'elements-nuit') document.documentElement.setAttribute('data-theme', 'elements-nuit');
  else document.documentElement.removeAttribute('data-theme');
});
</script>`;

const corps = [
  enTete(),
  sommaire(),
  principes(),
  marqueSection(),
  couleurs(),
  themes(),
  typographie(),
  troncature(),
  formes(),
  coins(),
  kit(),
  icones(),
  ecran01(),
  ecran02(),
  barreEtat(),
  ecran03(),
  ecran04(),
  reglages(),
  migration(),
  avis(),
  ligneMessage(),
  journal(rows),
].join('\n');

const html = `<!DOCTYPE html>
<html lang="fr">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Wind — Système de référence et écrans, v2 « Elements » (exploration)</title>
</head>
<body>
<x-dc>
<helmet>
<meta name="design_doc_mode" content="canvas" />
<style>${CSS}</style>
</helmet>
<div class="dc">
${normaliser(corps)}
</div>
${PILULE}
</x-dc>
</body>
</html>
`;

writeFileSync(SORTIE, html, 'utf8');

// --- Garde 2 : le banc ------------------------------------------------
const mesures = banc();
const echecsBanc = mesures.filter((m) => !m.ok);
const reperes = bancReperes();
const echecsRep = reperes.filter((r) => !r.ok);
for (const m of echecsBanc) {
  echec(`${m.theme} ${m.encre}/${m.fond} ${m.r.toFixed(2)}:1 (seuil ${m.seuil}) — ${m.ou}`);
}
for (const r of echecsRep) {
  echec(`repère ${r.nom} ${r.theme} pastille ${r.pastille.toFixed(2)}:1 · glyphe ${r.glyphesur.toFixed(2)}:1`);
}

console.log(`\n${path.relative(root, SORTIE)} — ${(html.length / 1024).toFixed(0)} Ko`);
console.log(`${Object.keys(JEU).length} glyphes dessinés, ${RELEVE.length} entrées au relevé — parité vérifiée dans les deux sens`);
console.log(`${rows.length} amendements repris (dont ${rows.filter((r) => r.dateRestituee).length} à date restituée)`);
console.log(`${mesures.length} mesures de paires, ${echecsBanc.length} échec(s)`);
console.log(`${reperes.length} mesures de repères, ${echecsRep.length} échec(s)`);
process.exit(echecs.length ? 1 : 0);

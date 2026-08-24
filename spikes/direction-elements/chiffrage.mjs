// ====================================================================
// Chiffrage du POSTE F — « adopter la direction Elements, c'est
// redessiner le jeu d'icônes ». Combien, et de quelle sorte.
//
//   node chiffrage.mjs          verdict
//   node chiffrage.mjs --tout   le détail glyphe par glyphe
//
// Rien n'est estimé : tout est compté sur le catalogue réellement
// dessiné (jeu.mjs) et sur l'inventaire réellement expédié
// (assets/icones/README.md). Les deux sont relus à chaque exécution —
// si l'inventaire bouge, le chiffrage bouge.
// ====================================================================
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { JEU, RESERVES, REPERES } from './jeu.mjs';

const root = path.resolve(import.meta.dirname, '..', '..');

// --- 1. L'inventaire expédié, relu à la source -----------------------
const md = readFileSync(path.join(root, 'assets', 'icones', 'README.md'), 'utf8');
const i0 = md.indexOf('`account_balance` `all_inbox`');
const bloc = md.slice(i0, md.indexOf('Ajouter un glyphe', i0));
const INVENTAIRE = [...new Set(bloc.match(/`([a-z_0-9]+)`/g).map((s) => s.slice(1, -1)))].sort();

// --- 2. Le jeu dessiné couvre-t-il l'inventaire ? ---------------------
const dessines = Object.keys(JEU).sort();
const manquants = INVENTAIRE.filter((n) => !dessines.includes(n));
const enTrop = dessines.filter((n) => !INVENTAIRE.includes(n));

// --- 3. Mesures de forme, par glyphe ---------------------------------
// Ce qu'on mesure, et pourquoi :
//   sous-chemins  — chaque sous-chemin est un trait de plus à caler
//   noeuds        — la quantité de dessin
//   arcs          — le document n'emploie la courbe que trois fois
//                   (rabat de Wind, boucle de Moon, rien d'autre) ;
//                   chaque arc est un écart à la grammaire
//   diagonales    — même raison
//   entiers       — « coordonnées entières » est une règle du document
//   survie 24->16 — une coordonnée du maître survit au palier 16 si
//                   c * 2/3 tombe sur un entier, donc si c est
//                   multiple de 3. C'est la mesure du COÛT DU PALIER :
//                   ce qui ne survit pas se redessine à la main.
function mesurer(g) {
  // Les formes PLEINES (têtes de note, coussinets, triangle d'envoi)
  // comptent comme le reste : ce sont des sous-chemins à caler, et leurs
  // centres sont des coordonnées à faire survivre au palier 16.
  const d = [...g.d, ...(g.barre ? [g.barre] : []), ...(g.remplis || [])].join(' ');
  const nombres = [
    ...(d.match(/-?\d+(?:\.\d+)?/g) || []).map(Number),
    ...(g.pleins || []).flat(),
    ...(g.disque || []),
  ];
  const arcs = (d.match(/[aA]/g) || []).length;
  const entiers = nombres.filter((n) => Number.isInteger(n)).length;
  const survivants = nombres.filter((n) => Number.isInteger(n) && n % 3 === 0).length;
  return {
    sousChemins: g.d.length + (g.barre ? 1 : 0) + (g.disque ? 1 : 0)
      + (g.pleins || []).length + (g.remplis || []).length,
    noeuds: nombres.length,
    arcs,
    entiers, tousEntiers: entiers === nombres.length,
    survie: nombres.length ? survivants / nombres.length : 1,
  };
}

const M = {};
for (const [nom, g] of Object.entries(JEU)) M[nom] = mesurer(g);

// --- 4. Les tailles de rendu RÉELLES ---------------------------------
// Relevées dans systeme.css et les composants : c'est ce qui décide
// combien de paliers il faut, pas une intention.
const TAILLES = [
  { px:10, ou:'.repere-liste — le repère de compte, rangée de liste' },
  { px:12, ou:'.repere-nav — le repère de compte, navigation' },
  { px:13, ou:'boutons compacts' },
  { px:14, ou:'.puce, .btn-statut — puces et barre d’état' },
  { px:15, ou:'variantes de puce' },
  { px:16, ou:'.ms — LE DÉFAUT, partout ailleurs' },
  { px:18, ou:'barres d’actions' },
];
// Le trait de 2 unités sur une grille de 24, rendu à P px, mesure :
const traitRendu = (px) => (2 / 24) * px;

// --- 5. Comptes -------------------------------------------------------
const employes = INVENTAIRE.filter((n) => !RESERVES.includes(n));
const parClasse = (f) => Object.entries(JEU).filter(([n, g]) => INVENTAIRE.includes(n) && f(n, g));
const direct = parClasse((n, g) => g.c === 'direct');
const arbitrage = parClasse((n, g) => g.c === 'arbitrage');
const dur = parClasse((n, g) => g.c === 'dur');
const fusions = {};
for (const [n, g] of Object.entries(JEU)) if (g.f) (fusions[g.f] ||= []).push(n);

const p = (n, t) => `${n} (${(100 * n / t).toFixed(0)} %)`;

console.log('================================================================');
console.log('  POSTE F — chiffrage du jeu d\'icônes sous la direction Elements');
console.log('================================================================\n');

console.log('1. PÉRIMÈTRE');
console.log(`   inventaire expédié (assets/icones/README.md) : ${INVENTAIRE.length} glyphes`);
console.log(`   dont réservés, employés nulle part           : ${RESERVES.length} — ${RESERVES.join(', ')}`);
console.log(`   donc à redessiner pour l'usage               : ${employes.length}`);
console.log(`   dessinés dans jeu.mjs                        : ${dessines.length}`);
if (manquants.length) console.log(`   ECHEC — manquants : ${manquants.join(', ')}`);
if (enTrop.length) console.log(`   hors inventaire : ${enTrop.join(', ')}`);
console.log(`   couverture                                   : ${manquants.length ? 'INCOMPLÈTE' : 'COMPLÈTE'}\n`);

console.log('2. CE QUE LA GRAMMAIRE ABSORBE, ET CE QU\'ELLE REFUSE');
console.log(`   direct    — aucun arbitrage          : ${p(direct.length, INVENTAIRE.length)}`);
console.log(`   arbitrage — une décision à valider   : ${p(arbitrage.length, INVENTAIRE.length)}`);
console.log(`   dur       — la grammaire ne le porte : ${p(dur.length, INVENTAIRE.length)}`);
console.log(`               ${dur.map(([n]) => n).join(', ')}\n`);

const avecArc = Object.entries(M).filter(([, m]) => m.arcs > 0);
const tousEntiers = Object.entries(M).filter(([, m]) => m.tousEntiers);
console.log('3. ÉCART À LA GRAMMAIRE (le document : droites, entiers, presque aucune courbe)');
console.log(`   glyphes employant au moins un arc : ${p(avecArc.length, dessines.length)}`);
console.log(`   glyphes 100 % en coordonnées entières : ${p(tousEntiers.length, dessines.length)}`);
console.log(`   nœuds dessinés au total : ${Object.values(M).reduce((s, m) => s + m.noeuds, 0)}`);
console.log(`   sous-chemins au total   : ${Object.values(M).reduce((s, m) => s + m.sousChemins, 0)}\n`);

console.log('4. LE COÛT DU PALIER — c\'est ici que le poste se joue');
console.log('   Le document impose trois paliers : 16 (grille 16, barres pleines,');
console.log('   pixels calés), 24 (trait 2,0), maître (trait 2,3, à partir de 29 px).');
console.log('   Les tailles de rendu réelles de Wind :\n');
for (const t of TAILLES) {
  const tr = traitRendu(t.px);
  console.log(`     ${String(t.px).padStart(2)} px — trait rendu ${tr.toFixed(2)} px ${
    tr < 1 ? '<-- SOUS LE PIXEL' : tr === Math.round(tr) ? '(entier)' : '(sur demi-pixel)'}   ${t.ou}`);
}
console.log('\n   Aucune taille d\'emploi n\'atteint 21 px : le palier 24 et le palier');
console.log('   maître ne servent QUE la marque et l\'écran vide. Tout le reste tombe');
console.log('   dans le palier 16 — donc se redessine À LA MAIN sur la grille de 16.');
const survieMoyenne = Object.values(M).reduce((s, m) => s + m.survie, 0) / dessines.length;
console.log(`\n   Part des coordonnées du maître qui survivent au passage 24 -> 16`);
console.log(`   (c x 2/3 entier, donc c multiple de 3) : ${(100 * survieMoyenne).toFixed(0)} %`);
console.log('   Le reste tombe sur des tiers de pixel : ce n\'est pas une mise à');
console.log('   l\'échelle, c\'est un second dessin.\n');

console.log('5. LES DOUZE REPÈRES DE COMPTE — la branche de la décision C');
const repDur = REPERES.filter((n) => JEU[n].c === 'dur');
console.log(`   glyphes du jeu dédié : ${REPERES.length}, dont « dur » : ${repDur.length}`);
console.log(`   rendus à 10-12 px, soit un trait de ${traitRendu(10).toFixed(2)} à ${traitRendu(12).toFixed(2)} px :`);
console.log('   sous le palier 16 lui-même. Il leur faudrait un QUATRIÈME palier.');
console.log(`   Si le compte devient un disque nu (§4-C) : ${REPERES.length} glyphes disparaissent,`);
console.log(`   et le jeu à produire tombe de ${employes.length} à ${employes.length - REPERES.length}.\n`);

console.log('6. FUSIONS FORCÉES PAR LA GRAMMAIRE');
for (const [f, ns] of Object.entries(fusions)) {
  console.log(`   ${f.padEnd(12)} : ${ns.join(' = ')}`);
}
console.log('   Réduite, la grammaire fait retomber ces paires sur un même dessin.');
console.log('   Les garder distincts demande d\'ajouter du détail — c\'est-à-dire de');
console.log('   sortir de la grammaire. Chaque paire est une décision, pas un bug.\n');

console.log('7. LE CHIFFRE');
const socle = employes.length - REPERES.length;
console.log(`   Branche « disque nu » (§4-C tranché pour la doctrine) :`);
console.log(`     maîtres 24 à dessiner ......... ${socle}   (faits : ${socle})`);
console.log(`     paliers 16 à caler à la main .. ${socle}   (faits : 0)`);
console.log(`     total de dessins .............. ${socle * 2}   (faits : ${socle}, soit 50 %)`);
console.log(`   Branche « le glyphe de compte reste » :`);
console.log(`     maîtres 24 .................... ${employes.length}   (faits : ${employes.length})`);
console.log(`     paliers 16 .................... ${employes.length}`);
console.log(`     palier 10-12 (à inventer) ..... ${REPERES.length}`);
console.log(`     total de dessins .............. ${employes.length * 2 + REPERES.length}   (faits : ${employes.length})`);
console.log(`\n   Reste à produire, branche doctrinale : ${socle} dessins de palier 16,`);
console.log(`   dont ${dur.filter(([n]) => !JEU[n].repere).length} sur des glyphes déjà classés « dur » au maître.\n`);

if (process.argv.includes('--tout')) {
  console.log('DÉTAIL PAR GLYPHE');
  console.log('  glyphe                    classe      s-ch  nœuds  arcs  entiers  survie 24->16');
  for (const n of INVENTAIRE) {
    const m = M[n], g = JEU[n];
    console.log(`  ${n.padEnd(24)} ${(g.c + (g.r ? '*' : '')).padEnd(11)} ${
      String(m.sousChemins).padStart(4)}  ${String(m.noeuds).padStart(5)}  ${
      String(m.arcs).padStart(4)}  ${(m.tousEntiers ? 'oui' : 'non').padStart(7)}  ${
      (100 * m.survie).toFixed(0).padStart(11)} %`);
  }
  console.log('  * = réservé au sous-ensemble, employé nulle part');
}

process.exit(manquants.length ? 1 : 0);

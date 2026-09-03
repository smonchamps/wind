// Banc de contraste WCAG des jetons de thème (system.css) : chaque
// paire (encre, fond) réellement posée par ui-v2, dans TOUS les thèmes
// de la table (V7 amendée A94 — 4 thèmes, NOMBRE_ATTENDU fait foi).
// Seuils : 4,5:1 pour le texte courant, 3:1 pour le grand texte
// et les composants d'interface (icônes, disques, anneaux) — plus les
// paires de FILET (V3 : le filet porte SEUL la séparation), au seuil
// du filet expédié par Clarity (1,49:1 sur le fond, 1,26:1 sur une
// carte) : sous ce plancher, le filet disparaît et la séparation ment.
//
//   node contraste.mjs        -> tableau complet + verdict
//
// La source est system.css (les jetons expédiés), pas le prototype :
// c'est ce que l'utilisateur voit qui se mesure.
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { lireThemes, lireReperes, NOMBRE_ATTENDU } from './jetons.mjs';

const root = path.resolve(import.meta.dirname, '..');
const css = readFileSync(
  path.join(root, 'apps', 'desktop', 'ui-v2', 'src', 'system.css'),
  'utf8',
);

// Le parseur partagé des deux gates (jetons.mjs) ; seuls les hex sont
// mesurables ici (les ombres et scrims rgba() ne portent pas de paire).
const themes = lireThemes(css, { motifValeur: /#[0-9a-fA-F]{6}/ });

// --- Luminance et rapport WCAG ---------------------------------------
function lum(hex) {
  const c = [1, 3, 5].map((i) => {
    const v = parseInt(hex.slice(i, i + 2), 16) / 255;
    return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
}
const rapport = (a, b) => {
  const [l1, l2] = [lum(a), lum(b)].sort((x, y) => y - x);
  return (l1 + 0.05) / (l2 + 0.05);
};

// --- Les paires posées par ui-v2 (relevé des composants) -------------
// [encre, fond, seuil, où] — la table du Système Elements (section
// Thèmes de couleur), reprise valeur pour valeur : le banc du document
// et cette gate mesurent la MÊME chose.
const PAIRES = [
  ['ink', 'bg', 4.5, 'titres, objets de liste, nav'],
  ['ink', 'surface', 4.5, 'cartes de message, champs'],
  ['ink', 'sel', 4.5, 'rangée sélectionnée, dossier ouvert'],
  ['ink', 'hover', 4.5, 'rangée survolée'],
  ['ink', 'tile', 4.5, "objet d'une rangée épinglée"],
  ['ink2', 'bg', 4.5, 'expéditeurs, corps de texte'],
  ['ink2', 'surface', 4.5, 'puces, boutons secondaires'],
  ['ink2', 'sel', 4.5, 'aperçus (rangée sélectionnée)'],
  ['ink2', 'hover', 4.5, 'aperçus (rangée survolée)'],
  ['ink2', 'tile', 4.5, "aperçu d'une rangée épinglée"],
  ['muted', 'bg', 4.5, "heures, sourcils, barre d'état"],
  ['muted', 'surface', 4.5, 'descriptions, texte de substitution'],
  ['muted', 'sel', 4.5, 'heures (rangée sélectionnée)'],
  ['muted', 'hover', 4.5, 'heures (rangée survolée)'],
  ['muted', 'tile', 4.5, 'heures (rangée épinglée)'],
  ['onAccent', 'accent', 4.5, "libellé du bouton primaire, poignée de l'interrupteur armé"],
  ['onAccent', 'accentH', 4.5, 'libellé du bouton primaire (survol)'],
  ['alert', 'bg', 4.5, "texte d'erreur, mention Brouillon"],
  ['alert', 'sel', 4.5, 'mention Brouillon (rangée choisie)'],
  ['alert', 'hover', 4.5, 'mention Brouillon (rangée survolée)'],
  ['alert', 'tile', 4.5, 'mention Brouillon (rangée épinglée)'],
  ['alert', 'surface', 4.5, 'mention Brouillon (cartes), Supprimer le brouillon'],
  ['alert', 'surface', 3, "icône d'alerte, point d'anomalie, glyphe « Refuser »"],
  ['accent', 'surface', 3, 'icônes, coche, anneau de focus, glyphe « Accepter »'],
  ['accent', 'bg', 3, 'anneau de focus, liseré, poignée de volet'],
  ['accent', 'sel', 3, 'liseré de la ligne choisie, contour de la réponse en cours'],
  ['accent', 'tile', 3, "liseré et anneau de focus sur une rangée épinglée — COMPOSANT seulement : sous le seuil du texte en clair, aucun libellé d'accent ne se pose sur la tuile"],
  ['accent', 'surface', 4.5, 'compteur de non-lus, liens, libellés en accent (TEXTE)'],
  ['accent', 'bg', 4.5, 'compteur de non-lus de la nav (TEXTE, V4)'],
  ['tileInk', 'tile', 4.5, "tuile d'initiales, boîte en cours, rangée épinglée, tuile de date"],
  ['brand', 'bg', 3, 'DISQUE de non-lu et anneau de cycle sur le fond'],
  ['brand', 'surface', 3, 'DISQUE sur une carte'],
  ['brand', 'sel', 3, 'DISQUE sur la rangée choisie'],
  ['brand', 'hover', 3, 'DISQUE sur la rangée survolée'],
  ['brand', 'tile', 3, 'DISQUE sur la rangée épinglée'],
  ['border', 'bg', 1.49, 'filet sur le fond — seuil = le filet EXPÉDIÉ par Clarity'],
  ['border', 'surface', 1.26, 'filet sur une carte — seuil = le filet EXPÉDIÉ par Clarity'],
  ['border', 'tile', 1.26, "filet de la tuile d'initiales — la tuile ne vaut que 1,04:1 sur le fond clair, le filet la fait exister"],
];

let echecs = 0;
// Plancher de complétude : un thème que le motif ne reconnaît pas n'est
// pas un thème « en moins », c'est un thème JAMAIS mesuré qui part en
// production — le trou exact des 14 -nuit invisibles à [a-z]+ (A42).
if (Object.keys(themes).length !== NOMBRE_ATTENDU) {
  echecs += 1;
  console.log(`ECHEC ${Object.keys(themes).length} thème(s) extraits de system.css — ${NOMBRE_ATTENDU} attendus (jetons.mjs) : un bloc échappe au motif, ou la table a changé sans amender le plancher`);
}
// Seuls les ÉCARTS s'impriment (revue A42) : 28 × 25 lignes « ok »
// noyaient les échecs — le verdict final porte les comptes, un rouge
// reste bruyant et localisé.
let mesures = 0;
for (const [nom, t] of Object.entries(themes)) {
  for (const [encre, fond, seuil, ou] of PAIRES) {
    if (!t[encre] || !t[fond]) {
      // Un jeton introuvable n'est pas une paire à sauter : c'est le
      // banc qui ment. Bruyant, comme tout échec.
      echecs += 1;
      console.log(`ECHEC ${nom} · ${encre} sur ${fond} : jeton introuvable dans system.css  ${ou}`);
      continue;
    }
    mesures += 1;
    const r = rapport(t[encre], t[fond]);
    if (r < seuil) {
      echecs += 1;
      console.log(
        `ECHEC ${nom} · ${encre.padEnd(9)} sur ${fond.padEnd(8)} `
        + `${r.toFixed(2).padStart(5)}:1  (seuil ${seuil}:1)  ${ou}`,
      );
    }
  }
}
// --- REPERES (A74, PLAN-RETOURS-8 R1 ; jetons depuis A82) ------------
// Le nuancier des repères de compte : 12 familles × 2 déclinaisons
// (sombre pour les thèmes clairs, claire pour les -nuit — V5 : le
// nuancier SUIT la polarité). Depuis A82 chaque hex sert deux fois
// (pastille des Réglages en background, tracé en color) : les hex
// EXPÉDIÉS vivent en jetons --mk-<hue> dans les blocs :root de
// system.css — on lit CES jetons, jamais une copie. Chaque
// déclinaison doit tenir 3:1 (composant) sur les fonds où le repère se
// pose, et porter son glyphe de pastille à 4,5:1.
const REPERE_FAMILLES = 12;
// `tuile` en fait partie (revue 2026-08-22) : le repère du compte EN
// COURS se pose sur la tuile de nav — l'oublier laissait ce fond-là,
// précisément, sans mesure. `panel` est mort (V3).
const FONDS_REPERE = ['bg', 'sel', 'hover', 'surface', 'tile'];
// Le parseur des blocs --mk-* est PARTAGÉ avec la gate de cohérence
// (jetons.mjs, A94 — lireThemes ne peut pas servir : sa classe de nom
// [a-zA-Z0-9] laisse volontairement passer le trait d'union des
// --mk-*, voir le commentaire du nuancier dans system.css).
const sombres = lireReperes(css, { nuit: false });
const claires = lireReperes(css, { nuit: true });
// Les encres des glyphes se LISENT du CSS expédié, comme les fonds —
// une copie locale mentirait dès que system.css bouge (revue).
const encreSombre = css.match(/\.marker\s*\{[^}]*color:(#[0-9a-fA-F]{6})/)?.[1];
const encreClaire = css.match(
  /\[data-theme\$="-nuit"\] \.marker\s*\{[^}]*color:(#[0-9a-fA-F]{6})/,
)?.[1];
if (!encreSombre || !encreClaire) {
  echecs += 1;
  console.log('ECHEC nuancier des repères : encre de glyphe introuvable dans system.css (.marker { color:… })');
}
for (const [nom, groupe] of [['sombre', sombres], ['claire', claires]]) {
  if (Object.keys(groupe).length !== REPERE_FAMILLES) {
    echecs += 1;
    console.log(`ECHEC nuancier des repères : ${Object.keys(groupe).length} déclinaison(s) ${nom}(s) extraite(s) de system.css — ${REPERE_FAMILLES} attendues`);
  }
}
for (const [nom, t] of Object.entries(themes)) {
  const nuit = nom.endsWith('-nuit');
  const groupe = nuit ? claires : sombres;
  const glyphe = nuit ? encreClaire : encreSombre;
  if (!glyphe) continue;
  for (const [teinte, hex] of Object.entries(groupe)) {
    for (const fond of FONDS_REPERE) {
      if (!t[fond]) continue;
      mesures += 1;
      const r = rapport(hex, t[fond]);
      if (r < 3) {
        echecs += 1;
        console.log(`ECHEC ${nom} · repère ${teinte.padEnd(8)} sur ${fond.padEnd(8)} ${r.toFixed(2).padStart(5)}:1  (seuil 3:1)  Réglages (pastille), nav et ligne (tracé)`);
      }
    }
    mesures += 1;
    const g = rapport(glyphe, hex);
    if (g < 4.5) {
      echecs += 1;
      console.log(`ECHEC ${nom} · glyphe sur repère ${teinte.padEnd(8)} ${g.toFixed(2).padStart(5)}:1  (seuil 4.5:1)`);
    }
  }
}

console.log(echecs === 0
  ? `Tout passe — ${Object.keys(themes).length} thèmes, ${mesures} paires mesurées.`
  : `${echecs} paire(s) sous le seuil.`);
process.exitCode = echecs === 0 ? 0 : 1;

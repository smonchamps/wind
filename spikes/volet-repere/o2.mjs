// ====================================================================
// O2 « le glyphe nu », EN SITUATION — la fenêtre entière, jetable.
//
//   node spikes/volet-repere/o2.mjs
//
// La planche comparative juge sept dessins côte à côte ; elle ne dit
// pas la seule chose qui décide d'O2 : un tracé de 2 unités rendu à
// 18 px se TROUVE-t-il d'un balayage, dans une vraie fenêtre, avec la
// nav à côté et un volet de lecture qui tire l'œil ?
//
// D'où cette page : la fenêtre de Wind au complet, aux deux polarités,
// à la densité réelle (une liste montre ce qu'elle montre — six
// rangées pleines à 860 px de fenêtre, pas quatorze). Le décor est celui
// de la planche : mêmes comptes, mêmes teintes, mêmes glyphes (socle.mjs).
// La fenêtre elle-même vit dans fenetre.mjs — une seule copie.
//
// Ce qu'il faut regarder — et qui ne se mesure pas :
//   1. le glyphe se trouve-t-il SANS le chercher, en descendant ?
//   2. la nav porte ses pastilles PLEINES de 20 px, la liste ses
//      glyphes NUS : le même compte se lit-il comme le même compte ?
//   3. sur la rangée choisie (sol --sel) et en nuit, le tracé tient-il ?
// ====================================================================
import { writeFileSync } from 'node:fs';
import path from 'node:path';
import {
  RACINE, CLAIR, COMPTES, FIL, ligne1, corps, rangPuces,
  pastille, glypheNu, classes, CSS_VOLET,
} from './socle.mjs';
import { fenetre, CSS_FENETRE } from './fenetre.mjs';

const ici = import.meta.dirname;

const rangeeO2 = (l) => {
  const cpt = COMPTES[l.c];
  return `<div class="${classes(l)}"><span class="tete">${glypheNu(cpt)}</span>`
    + ligne1(l) + corps(l) + rangPuces(l) + `</div>`;
};
const rangeeO1 = (l) => {
  const cpt = COMPTES[l.c];
  return `<div class="${classes(l)}"><span class="tete">${pastille(cpt)}</span>`
    + ligne1(l) + corps(l) + rangPuces(l) + `</div>`;
};

// --- La feuille --------------------------------------------------------
const FEUILLE = `
:root { color-scheme:light; }
* { box-sizing:border-box; }
body {
  margin:0; padding:0 0 72px;
  font-family:-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  background:${CLAIR.bg}; color:${CLAIR.ink};
}
.page { max-width:1360px; margin:0 auto; padding:0 40px; }
header.tete-page { padding:48px 0 20px; border-bottom:1px solid ${CLAIR.border}; }
h1.titre-page {
  font-family:'Segoe UI Variable Display', -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  font-weight:340; letter-spacing:-.03em; font-size:40px; margin:0 0 12px;
}
.sourcil { font-size:11px; letter-spacing:.1em; text-transform:uppercase;
  color:${CLAIR.muted}; font-weight:600; margin:0 0 6px; }
.sous-titre { margin:0 0 10px; font-size:15px; line-height:1.6; color:${CLAIR.ink2}; max-width:74ch; }
p.note { font-size:14px; line-height:1.6; color:${CLAIR.ink2}; max-width:78ch; margin:0 0 10px; }
ol.regarder { font-size:14px; line-height:1.6; color:${CLAIR.ink2}; max-width:78ch; padding-left:20px; }
ol.regarder li { margin-bottom:8px; }
b { color:${CLAIR.ink}; font-weight:600; }
code { font-family:Consolas, monospace; font-size:12.5px; background:${CLAIR.surface};
  border:1px solid ${CLAIR.border}; padding:0 4px; }
a { color:${CLAIR.accent}; }
section.bloc { padding:32px 0; border-bottom:1px solid ${CLAIR.border}; }
h2 {
  font-family:'Segoe UI Variable Display', -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  font-weight:340; letter-spacing:-.03em; font-size:24px; margin:0 0 4px;
}
.legende { margin:0 0 14px; font-size:13px; color:${CLAIR.muted}; }
.duo { display:flex; gap:24px; flex-wrap:wrap; align-items:flex-start; }
figure { margin:0; flex:none; }
figcaption { margin-top:8px; font-size:11px; letter-spacing:.08em; text-transform:uppercase;
  color:${CLAIR.muted}; font-weight:600; }

/* ================= LA FENÊTRE ================= */
${CSS_VOLET}
${CSS_FENETRE}
.ligne > .tete { grid-row:1; align-self:center; display:flex; width:18px; height:18px; }
.o1 .ligne > .tete { width:16px; height:16px; }

/* Le duo de fin : le même volet, O2 et O1 */
.cadre { width:400px; height:640px; overflow:hidden;
  border:1px solid var(--border); background:var(--bg); }
`;

const html = `<!doctype html>
<html lang="fr"><head><meta charset="utf-8">
<title>O2 — le glyphe nu, en situation</title>
<style>${FEUILLE}</style></head>
<body><div class="page">
<header class="tete-page">
  <p class="sourcil">Spike jetable · 2026-08-24 · rien ici n’est livré</p>
  <h1 class="titre-page">O2 — le glyphe nu, en situation</h1>
  <p class="sous-titre">La fenêtre entière, aux deux polarités, à la <b>densité réelle</b> :
  une liste montre ce qu’elle montre. Le décor est celui de la planche — mêmes comptes, mêmes
  teintes, glyphes et jetons lus du produit — et les comptes y alternent comme ils alternent
  vraiment quand le tri est la date : c’est le cas le plus défavorable au glyphe nu, et le seul
  qui vaille d’être regardé. Fenêtre de 1280 × 860 ; le défaut du produit est 1000 de large,
  nav 248 et liste 400.</p>
  <p class="note">Le volet de lecture est <b>schématique</b> — assez fidèle pour donner l’échelle
  et tirer l’œil, pas une proposition. Il garde la tuile aux initiales : dans un fil, l’expéditeur
  change d’un message à l’autre (arbitrage D-c de la planche).</p>
  <ol class="regarder">
    <li><b>Le glyphe se trouve-t-il sans le chercher ?</b> Descendez la liste comme vous la
    descendez le matin. S’il faut s’arrêter pour lire la marque, « visible » n’est pas tenu.</li>
    <li><b>Le même compte se lit-il comme le même compte ?</b> La nav porte ses pastilles
    PLEINES de 20 px, la liste ses glyphes NUS. C’est le coût nommé d’O2 — il se juge ici, pas
    en prose.</li>
    <li><b>La rangée choisie et la nuit.</b> Sur <code>--sel</code> et sur le fond nuit, le tracé
    de 1,5 px tient-il ? La mesure dit 4,97:1 au pire ; l’œil dira si c’est assez.</li>
  </ol>
  <p class="note"><a href="planche.html">← les sept dessins</a> ·
  <a href="organisation.html">les quinze organisations</a> ·
  <a href="ligne-expediteur.html">« Expéditeur sur ▣ Boîte »</a> ·
  <a href="v1v7.html">V1 + V7 en situation</a></p>
</header>

<section class="bloc">
  <h2>Elements — clair</h2>
  <p class="legende">Boîte unifiée, trois comptes, rangée « Photos du chantier de Vaise » choisie.</p>
  ${fenetre('theme-clair', rangeeO2)}
</section>

<section class="bloc">
  <h2>Elements · nuit</h2>
  <p class="legende">Le même écran, la même minute — le nuancier suit la polarité (V5).</p>
  ${fenetre('theme-nuit', rangeeO2)}
</section>

<section class="bloc">
  <h2>Si le glyphe nu paraît trop léger — le même volet en O1</h2>
  <p class="legende">Même quatorze rangées, même largeur : à gauche le glyphe nu (O2), à droite
  la pastille pleine (O1). C’est le seul départage qui reste, et il est à l’œil.</p>
  <div class="duo">
    <figure><div class="cadre theme-clair"><div class="liste o2">${FIL.map(rangeeO2).join('')}</div></div>
      <figcaption>O2 — le glyphe nu</figcaption></figure>
    <figure><div class="cadre theme-clair"><div class="liste o1">${FIL.map(rangeeO1).join('')}</div></div>
      <figcaption>O1 — la pastille</figcaption></figure>
    <figure><div class="cadre theme-nuit"><div class="liste o2">${FIL.map(rangeeO2).join('')}</div></div>
      <figcaption>O2 — nuit</figcaption></figure>
    <figure><div class="cadre theme-nuit"><div class="liste o1">${FIL.map(rangeeO1).join('')}</div></div>
      <figcaption>O1 — nuit</figcaption></figure>
  </div>
</section>
</div></body></html>`;

const sortie = path.join(ici, 'o2.html');
writeFileSync(sortie, html, 'utf8');
console.log(`Écrit : ${path.relative(RACINE, sortie)}`);
console.log(`  ${FIL.length} rangées, ${new Set(FIL.map((l) => l.c)).size} comptes, 2 polarités`);

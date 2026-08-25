// Spike JETABLE (PLAN-ESPACEMENT, point dur D2) — les sondes de hauteur
// peuvent-elles rester montées en permanence sans polluer la région
// défilante de la liste ?
//
// Le contexte : Liste.svelte mesure h1/h2 avec deux rangées « sondes »
// rendues une fois puis RETIRÉES du DOM (`{#if !sondees}`). Si
// l'espacement devient réglable, ces hauteurs doivent se re-mesurer à
// chaque changement de cran — et `sondees` n'est jamais remis à false.
// Deux voies : re-sonder ponctuellement, ou garder les sondes montées
// pour qu'elles se re-mesurent seules (ResizeObserver via
// bind:offsetHeight). La seconde rend la classe de bug impossible, mais
// on l'accuse d'ajouter ~203 px de défilement fantôme.
//
//   node spikes/espacement/sondes.mjs
//
// QUATRE variantes, mesurées dans le vrai moteur (msedge = WebView2)
// sur la géométrie EXACTE de Liste.svelte (padding 13, filet 1,
// row-gap 3, puces 24), à cinq hauteurs de cadre.
// Playwright vit sous e2e/ : la résolution ESM part du FICHIER, pas du
// répertoire courant — on l'y envoie explicitement.
import { createRequire } from 'node:module';
import path from 'node:path';

const requis = createRequire(
  path.join(import.meta.dirname, '..', '..', 'e2e', 'package.json'),
);
const { chromium } = requis('@playwright/test');

// La géométrie du produit, recopiée du CSS expédié (Liste.svelte).
const CSS = `
  * { box-sizing:border-box; margin:0; }
  body { font:14px/1.3 "Segoe UI", system-ui, sans-serif; }
  .cadre { position:relative; overflow:auto; height:300px; width:400px;
           border:1px solid #ccc; }
  .espace { position:relative; }
  .fenetre { position:absolute; top:0; left:0; right:0;
             display:flex; flex-direction:column; }
  .ligne { padding:13px 16px; border-top:1px solid #CBC8BB;
           display:grid; grid-template-columns:1fr; row-gap:3px;
           align-items:start; }
  .l1 { display:flex; align-items:baseline; gap:6px; }
  .objet, .apercu { margin:0; overflow:hidden; white-space:nowrap; }
  .apercu { font-size:13px; line-height:1.45; min-height:1.45em; }
  .objet { font-size:14px; line-height:1.3; }
  .puces { height:24px; display:flex; align-items:center; }
  /* A : l'actuel — les sondes vivent puis disparaissent. */
  .sondes { position:absolute; visibility:hidden; left:0; right:0; }
  /* C : la cage NAÏVE — height:0 + overflow:hidden. Elle ne protège
     rien : la cage n'étant pas positionnée, elle n'est pas le bloc
     conteneur des sondes en position:absolute, qui se calent sur
     .cadre et échappent au clip. Gardée au banc comme témoin. */
  .sondes-cage { height:0; overflow:hidden; }
  /* D : la cage QUI TIENT — position:relative en plus, pour devenir le
     bloc conteneur des sondes ; elles sont alors clippées et sortent
     de la région défilante de .cadre. */
  .sondes-cage-relative { position:relative; height:0; overflow:hidden; }
`;

const RANGEE = (avecPuces) => `
  <article class="ligne">
    <div class="l1"><span class="exp">Sonde</span><span class="essor"></span><span class="heure">00:00</span></div>
    <p class="objet">Sonde</p>
    <p class="apercu">Sonde</p>
    ${avecPuces ? '<div class="puces"><span class="puce">2</span></div>' : ''}
  </article>`;

// Une liste COURTE : c'est le seul cas où un débordement fantôme se
// verrait (une boîte à un message dans une petite fenêtre).
const CORPS = (variante) => {
  const sondes = `<div class="sondes">${RANGEE(false)}${RANGEE(true)}</div>`;
  return `
  <div class="cadre" id="cadre">
    ${variante === 'A-retirees' ? '' : ''}
    ${variante === 'B-permanentes-absolute' ? sondes : ''}
    ${variante === 'C-permanentes-en-cage' ? `<div class="sondes-cage">${sondes}</div>` : ''}
    ${variante === 'D-cage-relative' ? `<div class="sondes-cage-relative">${sondes}</div>` : ''}
    <div class="espace" style="height:88px">
      <div class="fenetre">${RANGEE(false)}</div>
    </div>
  </div>`;
};

// msedge, et pas le chromium de Playwright : c'est le moteur RÉEL de
// WebView2, donc la géométrie mesurée est celle que Wind rendra.
const navigateur = await chromium.launch({ channel: 'msedge' });
const page = await (await navigateur.newContext()).newPage();
const resultats = [];

// La pile des sondes mesure 88 + 115 = 203 px. Le débordement fantôme
// ne peut se voir que si le CADRE est plus court que cela : on balaie
// donc de la fenêtre minimale plausible jusqu'au cas confortable.
// 150 px de cadre = une fenêtre d'environ 342 px de haut (chrome de
// Wind : 52 entête + 52 bandeau + 52 onglets + 36 barre d'état).
const HAUTEURS_CADRE = [120, 150, 203, 250, 300];

for (const hCadre of HAUTEURS_CADRE) {
  for (const variante of [
    'A-retirees', 'B-permanentes-absolute', 'C-permanentes-en-cage', 'D-cage-relative',
  ]) {
    await page.setContent(
      `<style>${CSS}\n.cadre{height:${hCadre}px}</style>${CORPS(variante)}`,
    );
    const m = await page.evaluate(() => {
      const cadre = document.getElementById('cadre');
      const sondes = [...document.querySelectorAll('.sondes .ligne')];
      return {
        scrollHeight: cadre.scrollHeight,
        clientHeight: cadre.clientHeight,
        // La mesure que les sondes doivent rendre : h1 puis h2.
        hauteurs: sondes.map((s) => s.offsetHeight),
        // Largeur : une sonde en cage garde-t-elle la largeur du volet ?
        // (une sonde étroite mesurerait une rangée qui n'existe pas)
        largeurs: sondes.map((s) => s.offsetWidth),
      };
    });
    resultats.push({ hCadre, variante, ...m });
  }
}

await navigateur.close();

console.log('UNE rangée servie (88 px) — le pire cas pour un débordement fantôme.');
console.log('La pile des sondes mesure 203 px (88 + 115).\n');
console.log('cadre  variante                  scroll  fantome  h1/h2     largeur');
for (const r of resultats) {
  const fantome = r.scrollHeight - r.clientHeight;
  const alerte = fantome > 0 ? '  <-- BARRE FANTOME' : '';
  console.log(
    `${String(r.hCadre).padStart(5)}  ${r.variante.padEnd(24)} `
    + `${String(r.scrollHeight).padStart(6)}  ${String(fantome).padStart(7)}  `
    + `${(r.hauteurs.join('/') || '—').padEnd(8)}  ${r.largeurs.join('/') || '—'}${alerte}`,
  );
}
console.log('\nLecture : « fantome » = ce que la barre de defilement offre en trop.');
console.log('Une variante n est retenue que si elle rend h1/h2 JUSTES (88/115) a la');
console.log('largeur du volet ET n ajoute AUCUN fantome, a TOUTES les hauteurs.');

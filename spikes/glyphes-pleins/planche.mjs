// Spike JETABLE (sujet CE 16 — « des glyphes dont le remplissage couleur
// est plein »). La planche d'étude qui manque pour trancher : les douze
// repères AU TRAIT (ce que le produit rend depuis A82) contre PLEIN +
// TRAIT, aux deux tailles d'emploi et sur les deux polarités, à leurs
// vraies teintes.
//
//   node spikes/glyphes-pleins/planche.mjs
//
// Pourquoi une planche AVANT tout code : A82 a retiré la pastille le
// 2026-08-24 en écrivant sa perte (« la nav dit le compte plus
// doucement »), et le terrain du 2026-08-25 a validé le point « le
// compte se trouve-t-il encore d'un coup d'œil ? ». L'instruction a
// chiffré COMMENT remplir ; personne n'a établi qu'il FAUT remplir.
// Une demi-journée de planche peut clore le sujet — ou l'ouvrir sur des
// yeux, pas sur un calcul.
//
// Deux faits mesurés que la planche doit RENDRE VISIBLES :
//  · trois glyphes ne se remplissent pas correctement — `shopping_bag`
//    (l'anse s'auto-ferme en pâté), `music_note` et `account_balance`
//    (sous-chemins d'aire nulle : rien à remplir) ;
//  · le remplissage RAPPROCHE les silhouettes (recouvrement moyen
//    0,24 → 0,47) — d'où la bande « en situation », où les douze se
//    regardent les uns les autres comme dans la nav.
//
// Tout est LU du produit : les tracés viennent de lib/icones.js, les
// teintes des jetons --rep-* de systeme.css. Rien n'est recopié.
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';

const requis = createRequire(
  path.join(import.meta.dirname, '..', '..', 'e2e', 'package.json'),
);
const { chromium } = requis('@playwright/test');

const racine = path.resolve(import.meta.dirname, '..', '..');
const src = path.join(racine, 'apps', 'desktop', 'ui-v2', 'src');
const icones = readFileSync(path.join(src, 'lib', 'icones.js'), 'utf8');
const css = readFileSync(path.join(src, 'systeme.css'), 'utf8');

// --- Les douze repères, LUS du catalogue (drapeau repere:true) -------
const REPERES = [];
for (const [, nom, corps] of icones.matchAll(/^ {2}([a-z_0-9]+): *\{([^\n]*)\},?$/gm)) {
  if (!/\brepere:true/.test(corps)) continue;
  const d = [...(corps.match(/d:\[([^\]]*)\]/)?.[1] ?? '').matchAll(/'([^']+)'/g)]
    .map(([, v]) => v);
  const pleins = [...(corps.match(/pleins:\[([^\]]*)\]/)?.[1] ?? '')
    .matchAll(/\[([^\]]+)\]/g)].map(([, v]) => v.split(',').map(Number));
  const remplis = [...(corps.match(/remplis:\[([^\]]*)\]/)?.[1] ?? '')
    .matchAll(/'([^']+)'/g)].map(([, v]) => v);
  REPERES.push({ nom, d, pleins, remplis });
}

// --- Les teintes, LUES des jetons --rep-* ----------------------------
function teintes(nuit) {
  const t = {};
  for (const [, theme, corps] of css.matchAll(
    /:root(\[data-theme="elements-nuit"\])?\s*\{([^}]+)\}/g,
  )) {
    if (Boolean(theme) !== nuit) continue;
    for (const [, nom, hex] of corps.matchAll(/--rep-([a-z]+)\s*:\s*(#[0-9a-fA-F]{6})/g)) {
      t[nom] = hex;
    }
  }
  return t;
}

// Les fonds réels sur lesquels un repère se pose (jetons de thème).
function jetons(nuit) {
  const bloc = css.match(
    nuit
      ? /:root\[data-theme="elements-nuit"\]\s*\{([^}]+)\}/
      : /:root\s*\{([^}]+)\}/,
  )?.[1] ?? '';
  const lire = (n) => bloc.match(new RegExp(`--${n}:(#[0-9a-fA-F]{6})`))?.[1];
  return { bg: lire('bg'), ink: lire('ink'), ink2: lire('ink2'), border: lire('border') };
}

const svg = (r, taille, plein, couleur) => {
  const traits = r.d
    .map((d) => `<path d="${d}"/>`)
    .join('');
  const remplissage = plein
    ? r.d.map((d) => `<path d="${d}" fill="currentColor" stroke="none"/>`).join('')
    : '';
  const pleinsExistants = r.pleins
    .map(([cx, cy, rr]) => `<circle cx="${cx}" cy="${cy}" r="${rr}" fill="currentColor"/>`)
    .join('');
  const remplisExistants = r.remplis
    .map((d) => `<path d="${d}" fill="currentColor" stroke="none"/>`)
    .join('');
  return `<svg viewBox="0 0 24 24" width="${taille}" height="${taille}"
    style="color:${couleur};vertical-align:middle">
    ${remplissage}
    <g fill="none" stroke="currentColor" stroke-width="2"
       stroke-linecap="butt" stroke-linejoin="miter">${traits}</g>
    ${pleinsExistants}${remplisExistants}</svg>`;
};

// Les trois que la mesure dit rétifs au remplissage : on les nomme sur
// la planche plutôt que de laisser le CE deviner pourquoi ils clochent.
const RETIFS = {
  shopping_bag: "l'anse s'auto-ferme en pâté",
  music_note: 'sous-chemins d’aire nulle — rien à remplir',
  account_balance: '5 sous-chemins sur 6 d’aire nulle',
};

function planche(nuit) {
  const t = teintes(nuit);
  const j = jetons(nuit);
  const noms = Object.keys(t);
  const lignes = REPERES.map((r, i) => {
    const c = t[noms[i % noms.length]];
    const note = RETIFS[r.nom]
      ? `<span class="retif">${RETIFS[r.nom]}</span>`
      : '';
    return `<tr>
      <th>${r.nom}${note}</th>
      <td>${svg(r, 16, false, c)}</td>
      <td class="sep">${svg(r, 16, true, c)}</td>
      <td>${svg(r, 14, false, c)}</td>
      <td class="sep">${svg(r, 14, true, c)}</td>
      <td>${svg(r, 12, false, c)}</td>
      <td class="sep">${svg(r, 12, true, c)}</td>
    </tr>`;
  }).join('');

  // « En situation » : les douze côte à côte, comme la colonne de nav
  // les montre — c'est là que la confusabilité se voit, pas en fiche.
  const bande = (plein) => REPERES.map((r, i) => {
    const c = t[noms[i % noms.length]];
    return `<span class="rang">${svg(r, 16, plein, c)}<span class="lib">Compte ${i + 1}</span></span>`;
  }).join('');

  return `<section class="planche" style="background:${j.bg};color:${j.ink}">
    <h2>${nuit ? 'Elements · nuit' : 'Elements (clair)'}</h2>
    <table>
      <thead><tr><th></th>
        <th colspan="2">16 px — la nav</th>
        <th colspan="2">14 px — la ligne</th>
        <th colspan="2">12 px — la pastille des Réglages</th></tr>
        <tr><th></th><th>trait</th><th class="sep">plein+trait</th>
        <th>trait</th><th class="sep">plein+trait</th>
        <th>trait</th><th class="sep">plein+trait</th></tr></thead>
      <tbody>${lignes}</tbody>
    </table>
    <h3>En situation — la colonne de nav</h3>
    <div class="bande"><div class="col"><p>au trait (le produit d'aujourd'hui)</p>${bande(false)}</div>
      <div class="col"><p>plein + trait</p>${bande(true)}</div></div>
  </section>`;
}

const html = `<!doctype html><meta charset="utf-8">
<style>
  body { margin:0; font:13px "Segoe UI", system-ui, sans-serif; }
  .planche { padding:24px 28px; }
  h2 { font-size:15px; font-weight:600; margin:0 0 14px; }
  h3 { font-size:13px; font-weight:600; margin:22px 0 10px; opacity:.75; }
  table { border-collapse:collapse; }
  th, td { padding:5px 12px; text-align:center; }
  thead th { font-size:11px; font-weight:600; opacity:.6; }
  tbody th { text-align:left; font-weight:400; font-family:Consolas, monospace;
             font-size:11.5px; opacity:.8; white-space:nowrap; }
  .retif { display:block; font-family:"Segoe UI", sans-serif; font-size:10px;
           opacity:.55; font-style:italic; }
  td.sep { border-left:1px solid currentColor; border-color:color-mix(in srgb, currentColor 18%, transparent); }
  .bande { display:flex; gap:44px; }
  .col p { font-size:11px; opacity:.6; margin:0 0 8px; }
  .rang { display:flex; align-items:center; gap:10px; padding:5px 8px; }
  .lib { font-size:13px; opacity:.85; }
</style>
${planche(false)}${planche(true)}`;

const navigateur = await chromium.launch({ channel: 'msedge' });
const page = await (await navigateur.newContext({
  viewport: { width: 1100, height: 900 },
  deviceScaleFactor: 2,
})).newPage();
await page.setContent(html);
for (const [i, nom] of ['clair', 'nuit'].entries()) {
  const fichier = path.join(import.meta.dirname, `planche-${nom}.png`);
  await page.locator('.planche').nth(i).screenshot({ path: fichier });
  console.log(`${nom.padEnd(6)} -> ${path.relative(racine, fichier)}`);
}
await navigateur.close();
console.log(`\n${REPERES.length} reperes lus du catalogue : ${REPERES.map((r) => r.nom).join(', ')}`);

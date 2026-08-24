// Génère planche.html À PARTIR de jeu.mjs.
//
//   node planche.mjs
//
// Pourquoi générer plutôt qu'écrire : la planche et le chiffrage doivent
// montrer et mesurer LE MÊME dessin. Un catalogue recopié à la main dans
// une page diverge au premier correctif — et une planche qui ment sur ce
// qu'on a dessiné ne prouve rien. La page produite est autonome (aucun
// module, aucun réseau) : elle s'ouvre au double-clic.
import { writeFileSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { JEU, MARQUE, RESERVES, REPERES } from './jeu.mjs';

const root = path.resolve(import.meta.dirname, '..', '..');
const md = readFileSync(path.join(root, 'assets', 'icones', 'README.md'), 'utf8');
const i0 = md.indexOf('`account_balance` `all_inbox`');
const INVENTAIRE = [...new Set(
  md.slice(i0, md.indexOf('Ajouter un glyphe', i0)).match(/`([a-z_0-9]+)`/g).map((s) => s.slice(1, -1)),
)].sort();

const esc = (s) => String(s).replace(/[&<>"]/g, (c) =>
  ({ '&':'&amp;', '<':'&lt;', '>':'&gt;', '"':'&quot;' }[c]));

// Un glyphe rendu. `px` fixe la taille ; le trait reste 2 unités sur la
// grille de 24 — c'est précisément ce qu'on veut donner à voir.
function svg(nom, px, trait = 2) {
  const g = JEU[nom];
  const chemins = g.d.map((d) => `<path d="${d}"/>`).join('');
  const barre = g.barre ? `<path d="${g.barre}" fill="none" stroke="var(--marque)" stroke-width="${trait}" stroke-linecap="butt"/>` : '';
  // `disque` = le disque d'ÉTAT, en teal. `pleins`/`remplis` = des formes
  // pleines de STRUCTURE (tête de note, coussinets, triangle d'envoi) :
  // en encre courante, jamais en couleur — la couleur reste un état.
  const disque = g.disque ? `<circle cx="${g.disque[0]}" cy="${g.disque[1]}" r="${g.disque[2]}" fill="var(--marque)"/>` : '';
  const pleins = (g.pleins || []).map(([cx, cy, r]) =>
    `<circle cx="${cx}" cy="${cy}" r="${r}" fill="currentColor"/>`).join('');
  const remplis = (g.remplis || []).map((d) =>
    `<path d="${d}" fill="currentColor" stroke="none"/>`).join('');
  return `<svg viewBox="0 0 24 24" width="${px}" height="${px}" aria-hidden="true"><g fill="none" `
    + `stroke="currentColor" stroke-width="${trait}" stroke-linecap="butt" stroke-linejoin="miter">`
    + `${chemins}</g>${barre}${disque}${pleins}${remplis}</svg>`;
}

const marqueSvg = (px) => `<svg viewBox="0 0 24 24" width="${px}" height="${px}" aria-hidden="true">`
  + `<g fill="none" stroke="currentColor" stroke-width="${MARQUE.trait}" stroke-linecap="butt" stroke-linejoin="miter">`
  + MARQUE.d.map((d) => `<path d="${d}"/>`).join('')
  + `</g><path d="${MARQUE.flap}" fill="var(--marque)"/></svg>`;

const cellule = (nom, px = 16) => {
  const g = JEU[nom];
  return `<figure class="cell c-${g.c}${g.r ? ' reserve' : ''}${g.repere ? ' repere' : ''}">
    <span class="glyphe">${svg(nom, px)}</span>
    <figcaption>${esc(nom)}</figcaption></figure>`;
};

const TAILLES = [10, 12, 14, 16, 18, 24, 48];
const ECHELLE = ['inbox', 'hourglass_empty', 'reply', 'settings', 'keyboard', 'star'];
const fusions = {};
for (const [n, g] of Object.entries(JEU)) if (g.f) (fusions[g.f] ||= []).push(n);
const durs = INVENTAIRE.filter((n) => JEU[n].c === 'dur');
const parClasse = (c) => INVENTAIRE.filter((n) => JEU[n].c === c);
const employes = INVENTAIRE.filter((n) => !RESERVES.includes(n));
const notes = INVENTAIRE.filter((n) => JEU[n].note);

const html = `<!DOCTYPE html>
<html lang="fr" data-theme="jour"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Poste F — le jeu complet</title>
<style>
/* GÉNÉRÉ PAR planche.mjs — ne pas éditer à la main. */
:root{
  --bg:#F3F2EE; --surface:#FFFFFF; --tuile:#F2EDE3; --tuileInk:#4A4436;
  --ink:#191D1E; --ink2:#565C5E; --muted:#606668; --border:#CBC8BB;
  --accent:#1A7A7A; --marque:#1F8A8A; --sel:#DDE9E6; --hover:#EAE8E1;
  --alert:#C42D24; color-scheme:light;
}
:root[data-theme="nuit"]{
  --bg:#0D100F; --surface:#171B1A; --tuile:#241F17; --tuileInk:#DFCFAE;
  --ink:#ECEDEA; --ink2:#B4BAB8; --muted:#98A0A1; --border:#333B3A;
  --accent:#3FA39C; --marque:#3FA39C; --sel:#1E322F; --hover:#141817;
  --alert:#EA9A90; color-scheme:dark;
}
*{box-sizing:border-box}
body{margin:0;background:var(--bg);color:var(--ink);-webkit-font-smoothing:antialiased;
  font-family:"Segoe UI Variable Text","Segoe UI",ui-sans-serif,system-ui,-apple-system,sans-serif}
.wrap{max-width:1080px;margin:0 auto;padding:56px 28px 100px}
.display{font-family:"Segoe UI Variable Display","Segoe UI",ui-sans-serif,system-ui,sans-serif;
  font-weight:340;letter-spacing:-.03em}
.sourcil{font-size:11px;letter-spacing:.2em;text-transform:uppercase;color:var(--muted);
  font-weight:600;margin:0 0 14px}
h1{font-size:46px;line-height:1.04;margin:0 0 18px}
.lede{font-size:16px;line-height:1.68;color:var(--ink2);margin:0;max-width:64ch}
section{margin-top:52px}
h2{font-size:26px;line-height:1.2;margin:0 0 6px}
.sub{margin:0 0 22px;font-size:13.5px;line-height:1.65;color:var(--muted);max-width:72ch}
hr{border:0;border-top:1px solid var(--border);margin:0}

.grille{display:grid;grid-template-columns:repeat(auto-fill,minmax(104px,1fr));gap:1px;
  background:var(--border);border:1px solid var(--border)}
.cell{margin:0;background:var(--bg);padding:16px 8px 11px;display:flex;flex-direction:column;
  align-items:center;gap:11px;min-height:88px;justify-content:center}
.cell .glyphe{color:var(--ink);display:grid;place-items:center;height:24px}
.cell figcaption{font-size:10px;line-height:1.35;color:var(--muted);text-align:center;
  word-break:break-word;font-variant-numeric:tabular-nums}
.cell.c-dur{background:var(--tuile)}
.cell.c-dur figcaption{color:var(--tuileInk)}
.cell.reserve{opacity:.45}
.legende{display:flex;gap:20px;flex-wrap:wrap;margin:16px 0 0;font-size:12px;color:var(--muted)}
.legende span{display:inline-flex;align-items:center;gap:7px}
.pastille{width:11px;height:11px;border-radius:2px;border:1px solid var(--border);display:inline-block}

.echelle{border:1px solid var(--border);border-collapse:collapse;width:100%}
.echelle th,.echelle td{border:1px solid var(--border);padding:12px 10px;text-align:center;
  vertical-align:middle}
.echelle thead th{font-size:10px;letter-spacing:.13em;text-transform:uppercase;color:var(--muted);
  font-weight:600}
.echelle tbody th{font-size:11.5px;font-weight:600;text-align:left;white-space:nowrap;color:var(--ink2)}
.echelle td span{color:var(--ink);display:inline-grid;place-items:center}

.duo{display:grid;grid-template-columns:repeat(auto-fill,minmax(300px,1fr));gap:14px}
.fiche{background:var(--surface);border:1px solid var(--border);border-radius:14px;padding:18px 20px;
  display:flex;gap:16px;align-items:flex-start}
.fiche .paire{display:flex;align-items:center;gap:12px;flex:none;color:var(--ink)}
.fiche .txt b{display:block;font-size:13px;margin-bottom:5px}
.fiche .txt span{font-size:12.5px;line-height:1.55;color:var(--muted)}

.fusion{display:flex;align-items:center;gap:14px;padding:13px 0;border-bottom:1px solid var(--border)}
.fusion:last-child{border-bottom:0}
.fusion .paire{display:flex;gap:14px;color:var(--ink);flex:none;width:96px}
.fusion .noms{font-size:13px;color:var(--ink2)}
.fusion .noms em{font-style:normal;color:var(--muted)}

.trait{display:flex;gap:0;align-items:flex-end;border:1px solid var(--border);background:var(--surface)}
.trait div{flex:1;padding:16px 8px 12px;text-align:center;border-right:1px solid var(--border)}
.trait div:last-child{border-right:0}
.trait .bar{background:var(--ink);margin:0 auto 12px;height:40px}
.trait .px{font-size:11px;color:var(--muted);font-variant-numeric:tabular-nums;line-height:1.5}
.trait .px b{display:block;color:var(--ink);font-size:12px}
.trait .sous{background:var(--alert)}

.chiffre{width:100%;border-collapse:collapse;margin-top:6px}
.chiffre th,.chiffre td{border-bottom:1px solid var(--border);padding:11px 10px;text-align:left;
  font-size:13px}
.chiffre thead th{font-size:10px;letter-spacing:.13em;text-transform:uppercase;color:var(--muted);
  font-weight:600}
.chiffre td.n{text-align:right;font-variant-numeric:tabular-nums;white-space:nowrap}
.chiffre tr.total td{font-weight:700;border-bottom:2px solid var(--ink)}

.pilule{position:fixed;right:18px;bottom:18px;display:flex;gap:6px;padding:6px;background:var(--surface);
  border:1px solid var(--border);border-radius:999px;box-shadow:0 6px 20px rgba(0,0,0,.13)}
.pilule button{height:26px;padding:0 13px;font-size:11.5px;border-radius:999px;border:0;
  background:transparent;color:var(--muted);cursor:pointer;font-weight:600;letter-spacing:.04em}
.pilule button[aria-pressed="true"]{background:var(--sel);color:var(--ink)}
:focus-visible{outline:2px solid var(--marque);outline-offset:2px}
@media(max-width:640px){h1{font-size:32px}.wrap{padding:34px 18px 70px}}
</style></head><body><div class="wrap">

<p class="sourcil">Spike « direction Elements » — poste F</p>
<h1 class="display">Soixante-dix-huit glyphes,<br>et deux paliers à dessiner</h1>
<p class="lede">Le jeu vendorisé de Wind compte ${INVENTAIRE.length} glyphes ; ${employes.length}
sont employés, ${RESERVES.length} sont réservés. Les voici tous redessinés dans la grammaire du
document — grille 24, trait 2 unités, bouts nets, jonctions vives. Cette page est
<b>générée</b> depuis <code>jeu.mjs</code> : elle montre exactement ce que
<code>chiffrage.mjs</code> mesure.</p>

<section>
  <h2 class="display">Le jeu, à la taille d'emploi</h2>
  <p class="sub">16 px — la taille par défaut de <code>.ms</code> dans le Système, donc celle
  de la grande majorité des icônes de Wind. Rien n'est agrandi. Les cellules sur fond de tuile
  sont les ${durs.length} glyphes que la grammaire ne porte pas à cette taille.</p>
  <div class="grille">${INVENTAIRE.map((n) => cellule(n, 16)).join('')}</div>
  <p class="legende">
    <span><i class="pastille" style="background:var(--bg)"></i>direct — ${parClasse('direct').length}</span>
    <span><i class="pastille" style="background:var(--bg)"></i>arbitrage — ${parClasse('arbitrage').length}</span>
    <span><i class="pastille" style="background:var(--tuile)"></i>dur — ${durs.length}</span>
    <span><i class="pastille" style="background:var(--bg);opacity:.45"></i>réservé, employé nulle part — ${RESERVES.length}</span>
  </p>
</section>

<section>
  <h2 class="display">Le même jeu, agrandi</h2>
  <p class="sub">48 px : le dessin se juge. C'est aussi la démonstration du problème — ces
  dessins-là sont bons, et ce ne sont pas eux que Wind affiche.</p>
  <div class="grille">${INVENTAIRE.map((n) => cellule(n, 48)).join('')}</div>
</section>

<section>
  <h2 class="display">L'échelle</h2>
  <p class="sub">Le même fichier maître rendu aux sept tailles réellement posées par le Système.
  La colonne 16 px est celle qui compte ; les colonnes 10 et 12 px sont celles des repères de compte.</p>
  <table class="echelle">
    <thead><tr><th>Glyphe</th>${TAILLES.map((t) => `<th>${t} px</th>`).join('')}</tr></thead>
    <tbody>${ECHELLE.map((n) => `<tr><th>${esc(n)}</th>${
      TAILLES.map((t) => `<td><span>${svg(n, t)}</span></td>`).join('')}</tr>`).join('')}</tbody>
  </table>
</section>

<section>
  <h2 class="display">Le trait sous le pixel</h2>
  <p class="sub">Un trait de 2 unités sur une grille de 24, rendu à P pixels, mesure 2 ÷ 24 × P.
  C'est la raison d'être des trois paliers du document — et la raison pour laquelle le maître
  ne peut pas être simplement mis à l'échelle.</p>
  <div class="trait">${[10, 12, 13, 14, 15, 16, 18, 24, 29].map((px) => {
    const l = (2 / 24) * px;
    return `<div><div class="bar${l < 1 ? ' sous' : ''}" style="width:${l.toFixed(2)}px"></div>
      <p class="px"><b>${px} px</b>${l.toFixed(2)} px${l < 1 ? '<br>sous le pixel' : ''}</p></div>`;
  }).join('')}</div>
  <p class="sub" style="margin-top:16px">Aucune taille d'emploi de Wind n'atteint 21 px : le palier 24
  et le palier maître ne servent que la marque ${marqueSvg(18)} et l'écran vide. Tout le reste
  vit dans le palier 16, qui se cale <b>à la main</b>, rectangle par rectangle.</p>
</section>

<section>
  <h2 class="display">Ce que la grammaire refuse</h2>
  <p class="sub">${durs.length} glyphes sur ${INVENTAIRE.length}. À gauche la taille d'emploi,
  à droite le dessin dont elle vient. L'écart entre les deux colonnes est le coût.</p>
  <div class="duo">${durs.map((n) => `<div class="fiche">
    <span class="paire">${svg(n, 16)}${svg(n, 40)}</span>
    <span class="txt"><b>${esc(n)}</b><span>${esc(JEU[n].note
      || 'Trop de sous-chemins ou de nœuds pour tenir à 16 px dans un trait de 2 unités.')}</span></span>
  </div>`).join('')}</div>
</section>

<section>
  <h2 class="display">Fusions forcées</h2>
  <p class="sub">Réduits à la grammaire, ces glyphes retombent sur le même dessin. Les garder
  distincts demande d'ajouter du détail — donc de sortir de la grammaire. Chaque paire est une
  décision à prendre, pas un défaut à corriger.</p>
  <div>${Object.entries(fusions).map(([f, ns]) => `<div class="fusion">
    <span class="paire">${ns.map((n) => svg(n, 24)).join('')}</span>
    <span class="noms"><b>${esc(f)}</b> — ${ns.map((n) => esc(n)).join(' = ')}
    <em>&nbsp;· ${ns.length} entrées du jeu Material, un seul dessin ici</em></span></div>`).join('')}</div>
</section>

<section>
  <h2 class="display">Les douze repères de compte</h2>
  <p class="sub">Rendus à 10 et 12 px dans une pastille colorée (A74). Le trait y mesure
  ${((2 / 24) * 10).toFixed(2)} à ${((2 / 24) * 12).toFixed(2)} px : sous le palier 16 lui-même.
  Sous cette direction, le compte est un <b>disque nu</b> et ces douze glyphes disparaissent —
  c'est l'arbitrage §4-C de la note de spike, et c'est ${REPERES.length} dessins d'un côté ou de
  l'autre de la décision.</p>
  <table class="echelle">
    <thead><tr><th>Glyphe</th><th>10 px</th><th>12 px</th><th>16 px</th><th>40 px</th></tr></thead>
    <tbody>${REPERES.map((n) => `<tr><th>${esc(n)}</th>
      <td><span>${svg(n, 10)}</span></td><td><span>${svg(n, 12)}</span></td>
      <td><span>${svg(n, 16)}</span></td><td><span>${svg(n, 40)}</span></td></tr>`).join('')}</tbody>
  </table>
</section>

<section>
  <h2 class="display">Le chiffre</h2>
  <p class="sub">Un « dessin » = un fichier de glyphe calé sur sa grille. Le maître 24 est fait
  pour les ${INVENTAIRE.length} ; le palier 16 ne l'est pour aucun.</p>
  <table class="chiffre">
    <thead><tr><th>Branche</th><th>À produire</th><th class="n">Dessins</th><th class="n">Faits</th></tr></thead>
    <tbody>
      <tr><td rowspan="3"><b>Disque nu</b><br><span style="color:var(--muted);font-size:12px">§4-C tranché pour la doctrine</span></td>
        <td>maîtres 24</td><td class="n">${employes.length - REPERES.length}</td><td class="n">${employes.length - REPERES.length}</td></tr>
      <tr><td>paliers 16, calés à la main</td><td class="n">${employes.length - REPERES.length}</td><td class="n">0</td></tr>
      <tr class="total"><td>total</td><td class="n">${(employes.length - REPERES.length) * 2}</td>
        <td class="n">${employes.length - REPERES.length} · 50 %</td></tr>
      <tr><td rowspan="4"><b>Le glyphe reste</b><br><span style="color:var(--muted);font-size:12px">A74 conservé</span></td>
        <td>maîtres 24</td><td class="n">${employes.length}</td><td class="n">${employes.length}</td></tr>
      <tr><td>paliers 16</td><td class="n">${employes.length}</td><td class="n">0</td></tr>
      <tr><td>palier 10-12, à inventer</td><td class="n">${REPERES.length}</td><td class="n">0</td></tr>
      <tr class="total"><td>total</td><td class="n">${employes.length * 2 + REPERES.length}</td>
        <td class="n">${employes.length} · ${Math.round(100 * employes.length / (employes.length * 2 + REPERES.length))} %</td></tr>
    </tbody>
  </table>
</section>

<section>
  <h2 class="display">Les décisions prises en dessinant</h2>
  <p class="sub">Chaque note est un endroit où la grammaire ne donnait pas la réponse. Elles ne
  sont pas des détails de dessin : ce sont les endroits où quelqu'un devra trancher.</p>
  <div class="duo">${notes.map((n) => `<div class="fiche">
    <span class="paire">${svg(n, 24)}</span>
    <span class="txt"><b>${esc(n)}</b><span>${esc(JEU[n].note)}</span></span></div>`).join('')}</div>
</section>

</div>
<div class="pilule">
  <button id="j" aria-pressed="true">Clair</button>
  <button id="n" aria-pressed="false">Sombre</button>
</div>
<script>
const poser=(t)=>{document.documentElement.dataset.theme=t;
  j.setAttribute('aria-pressed',String(t==='jour'));n.setAttribute('aria-pressed',String(t==='nuit'));};
j.onclick=()=>poser('jour'); n.onclick=()=>poser('nuit');
</script></body></html>
`;

const sortie = path.join(import.meta.dirname, 'planche.html');
writeFileSync(sortie, html, 'utf8');
console.log(`planche.html écrite — ${INVENTAIRE.length} glyphes, ${(html.length / 1024).toFixed(0)} Kio`);

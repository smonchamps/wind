// Génère comparaison.html — le jeu Elements (jeu.mjs, montré par
// planche.html) face au jeu du Système, tel que le DESSINE réellement
// docs/design/systeme.dc.html.
//
//   node comparaison.mjs
//
// Trois ensembles sont croisés, chacun relu à SA source :
//   DC     — les <span class="ms">…</span> de systeme.dc.html
//   FONTE  — l'inventaire vendorisé (assets/icones/README.md)
//   CODE   — les noms réellement employés dans apps/desktop/ui-v2/src
//   JEU    — le catalogue Elements (jeu.mjs)
//
// La fonte vendorisée est EMBARQUÉE en base64 : la page est autonome et
// ne demande rien au réseau — le DC, lui, tire Material Symbols du CDN
// Google, ce que l'application s'interdit (CSP `font-src 'self'`).
import { readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { JEU } from './jeu.mjs';

const ici = import.meta.dirname;
const root = path.resolve(ici, '..', '..');
const lire = (...p) => readFileSync(path.join(root, ...p), 'utf8');

// --- DC : les glyphes dessinés, et à quelles tailles -------------------
const dc = lire('docs', 'design', 'systeme.dc.html');
const spans = [...dc.matchAll(/<span[^>]*class="ms"([^>]*)>([a-z][a-z_0-9]*)<\/span>/g)];
const DC = new Map();                       // nom -> Map(taille -> compte)
for (const [, attrs, nom] of spans) {
  const t = Number((attrs.match(/font-size:(\d+)px/) || [, 16])[1]);
  if (!DC.has(nom)) DC.set(nom, new Map());
  DC.get(nom).set(t, (DC.get(nom).get(t) || 0) + 1);
}
const taillesDC = new Map();
for (const m of DC.values()) for (const [t, n] of m) taillesDC.set(t, (taillesDC.get(t) || 0) + n);

// --- FONTE : l'inventaire vendorisé -----------------------------------
const md = lire('assets', 'icones', 'README.md');
const i0 = md.indexOf('`account_balance` `all_inbox`');
const FONTE = [...new Set(
  md.slice(i0, md.indexOf('Ajouter un glyphe', i0)).match(/`([a-z_0-9]+)`/g).map((s) => s.slice(1, -1)),
)].sort();

// --- CODE : ce que ui-v2 emploie vraiment ------------------------------
import { readdirSync, statSync } from 'node:fs';
const fichiers = [];
(function marcher(d) {
  for (const e of readdirSync(d)) {
    const f = path.join(d, e);
    if (statSync(f).isDirectory()) marcher(f);
    else if (/\.(svelte|js)$/.test(e)) fichiers.push(f);
  }
})(path.join(root, 'apps', 'desktop', 'ui-v2', 'src'));
const src = fichiers.map((f) => readFileSync(f, 'utf8')).join('\n');
const CODE = new Set(FONTE.filter((n) => new RegExp(`[>'"\\s]${n}[<'"\\s,}]`).test(src)));

// --- Les écarts --------------------------------------------------------
const dcSet = new Set(DC.keys());
const absentsDuDC = FONTE.filter((n) => CODE.has(n) && !dcSet.has(n));   // livré, jamais dessiné
const perimesAuDC = [...dcSet].filter((n) => !CODE.has(n)).sort();        // dessiné, plus livré
const horsFonte = [...dcSet].filter((n) => !FONTE.includes(n)).sort();    // dessiné, pas dans la fonte

// --- La fonte vendorisée, embarquée -----------------------------------
const woff2 = readFileSync(path.join(root, 'assets', 'icones', 'material-symbols-rounded.subset.woff2'))
  .toString('base64');

const esc = (s) => String(s).replace(/[&<>"]/g, (c) =>
  ({ '&':'&amp;', '<':'&lt;', '>':'&gt;', '"':'&quot;' }[c]));

// Le glyphe Elements, rendu depuis le catalogue.
function elements(nom, px) {
  const g = JEU[nom];
  if (!g) return `<span class="absent">—</span>`;
  const chemins = g.d.map((d) => `<path d="${d}"/>`).join('');
  const barre = g.barre ? `<path d="${g.barre}" fill="none" stroke="var(--marque)" stroke-width="2" stroke-linecap="butt"/>` : '';
  const disque = g.disque ? `<circle cx="${g.disque[0]}" cy="${g.disque[1]}" r="${g.disque[2]}" fill="var(--marque)"/>` : '';
  const pleins = (g.pleins || []).map(([cx, cy, r]) =>
    `<circle cx="${cx}" cy="${cy}" r="${r}" fill="currentColor"/>`).join('');
  const remplis = (g.remplis || []).map((d) =>
    `<path d="${d}" fill="currentColor" stroke="none"/>`).join('');
  return `<svg viewBox="0 0 24 24" width="${px}" height="${px}" aria-hidden="true"><g fill="none" `
    + `stroke="currentColor" stroke-width="2" stroke-linecap="butt" stroke-linejoin="miter">`
    + `${chemins}</g>${barre}${disque}${pleins}${remplis}</svg>`;
}
const material = (nom, px) =>
  `<span class="ms" style="font-size:${px}px" aria-hidden="true">${nom}</span>`;

// Tous les noms connus de l'un ou l'autre bord.
const TOUS = [...new Set([...FONTE, ...dcSet, ...Object.keys(JEU)])].sort();

function carte(nom) {
  const g = JEU[nom];
  const badges = [];
  if (!dcSet.has(nom)) badges.push('<i class="b b-manque">absent du DC</i>');
  if (!CODE.has(nom)) badges.push('<i class="b b-perime">plus employé</i>');
  if (g?.f) badges.push(`<i class="b b-fusion">fusion · ${esc(g.f)}</i>`);
  if (g?.c === 'dur') badges.push('<i class="b b-dur">dur</i>');
  const tailles = DC.has(nom)
    ? [...DC.get(nom).entries()].sort((a, b) => a[0] - b[0]).map(([t, n]) => `${t} px ×${n}`).join(', ')
    : 'jamais dessiné';
  return `<figure class="paire">
    <figcaption><b>${esc(nom)}</b>${badges.join('')}</figcaption>
    <div class="cotes">
      <div class="cote"><span class="etiq">Système</span>
        <span class="rendu">${material(nom, 16)}</span><span class="rendu">${material(nom, 40)}</span></div>
      <div class="cote"><span class="etiq">Elements</span>
        <span class="rendu">${elements(nom, 16)}</span><span class="rendu">${elements(nom, 40)}</span></div>
    </div>
    <p class="dc">DC : ${esc(tailles)}</p>
    ${g?.note ? `<p class="note">${esc(g.note)}</p>` : ''}
  </figure>`;
}

const liste = (ns) => ns.map((n) => `<li><span class="rendu">${material(n, 20)}</span>
  <span class="rendu">${elements(n, 20)}</span><code>${esc(n)}</code></li>`).join('');

const html = `<!DOCTYPE html>
<html lang="fr" data-theme="jour"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Elements contre le Système — glyphe par glyphe</title>
<style>
/* GÉNÉRÉ PAR comparaison.mjs — ne pas éditer à la main. */
@font-face{
  font-family:'Material Symbols Rounded'; font-style:normal; font-weight:300 600;
  font-display:block;
  src:url(data:font/woff2;base64,${woff2}) format('woff2');
}
.ms{font-family:'Material Symbols Rounded';font-weight:300;line-height:1;
  font-variation-settings:'opsz' 20,'FILL' 0;display:inline-block}
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
.sub{margin:0 0 22px;font-size:13.5px;line-height:1.65;color:var(--muted);max-width:74ch}
code{font-family:ui-monospace,"Cascadia Mono",Consolas,monospace;font-size:.9em}

.chiffres{display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));gap:1px;
  background:var(--border);border:1px solid var(--border);margin-bottom:8px}
.chiffres div{background:var(--bg);padding:16px 18px}
.chiffres b{display:block;font-size:30px;line-height:1.1;font-weight:340;
  font-family:"Segoe UI Variable Display","Segoe UI",sans-serif;letter-spacing:-.03em}
.chiffres span{font-size:11.5px;line-height:1.5;color:var(--muted);display:block;margin-top:5px}

.ecart{background:var(--surface);border:1px solid var(--border);border-radius:14px;
  padding:22px 24px;margin-bottom:14px}
.ecart.grave{border-left:3px solid var(--alert)}
.ecart h3{margin:0 0 6px;font-size:15px;font-weight:650}
.ecart p{margin:0 0 14px;font-size:13px;line-height:1.6;color:var(--muted);max-width:76ch}
.ecart ul{list-style:none;margin:0;padding:0;display:flex;flex-wrap:wrap;gap:8px}
.ecart li{display:flex;align-items:center;gap:8px;padding:6px 11px 6px 8px;background:var(--bg);
  border:1px solid var(--border);border-radius:8px;font-size:12.5px;color:var(--ink2)}
.ecart li .rendu{color:var(--ink);display:grid;place-items:center}

.paires{display:grid;grid-template-columns:repeat(auto-fill,minmax(258px,1fr));gap:1px;
  background:var(--border);border:1px solid var(--border)}
.paire{margin:0;background:var(--bg);padding:16px 16px 14px}
.paire figcaption{display:flex;flex-wrap:wrap;align-items:center;gap:6px;margin-bottom:13px}
.paire figcaption b{font-size:12.5px;font-weight:650;word-break:break-word}
.b{font-style:normal;font-size:9.5px;letter-spacing:.06em;text-transform:uppercase;font-weight:700;
  padding:2px 6px;border-radius:3px;white-space:nowrap}
.b-manque{background:var(--alert);color:var(--bg)}
.b-perime{background:var(--tuile);color:var(--tuileInk)}
.b-fusion{background:var(--sel);color:var(--accent)}
.b-dur{background:var(--ink2);color:var(--bg)}
.cotes{display:grid;grid-template-columns:1fr 1fr;gap:1px;background:var(--border);
  border:1px solid var(--border)}
.cote{background:var(--surface);padding:11px 10px 12px;display:flex;flex-direction:column;
  align-items:center;gap:10px}
.cote .etiq{font-size:9px;letter-spacing:.13em;text-transform:uppercase;color:var(--muted);
  font-weight:700}
.cote .rendu{color:var(--ink);display:grid;place-items:center;min-height:16px}
.cote .rendu:last-child{min-height:40px}
.absent{color:var(--muted);font-size:20px}
.paire .dc{margin:10px 0 0;font-size:10.5px;color:var(--muted);font-variant-numeric:tabular-nums}
.paire .note{margin:7px 0 0;font-size:11px;line-height:1.5;color:var(--muted);
  padding-top:7px;border-top:1px solid var(--border)}

.temoin{display:flex;align-items:center;gap:14px;background:var(--tuile);color:var(--tuileInk);
  border-radius:10px;padding:14px 18px;font-size:12.5px;line-height:1.55;margin-top:18px}
.temoin .ms{font-size:26px}

.pilule{position:fixed;right:18px;bottom:18px;display:flex;gap:6px;padding:6px;background:var(--surface);
  border:1px solid var(--border);border-radius:999px;box-shadow:0 6px 20px rgba(0,0,0,.13)}
.pilule button{height:26px;padding:0 13px;font-size:11.5px;border-radius:999px;border:0;
  background:transparent;color:var(--muted);cursor:pointer;font-weight:600;letter-spacing:.04em}
.pilule button[aria-pressed="true"]{background:var(--sel);color:var(--ink)}
:focus-visible{outline:2px solid var(--marque);outline-offset:2px}
@media(max-width:640px){h1{font-size:32px}.wrap{padding:34px 18px 70px}}
</style></head><body><div class="wrap">

<p class="sourcil">Spike « direction Elements » — confrontation</p>
<h1 class="display">Elements contre le Système,<br>glyphe par glyphe</h1>
<p class="lede">À gauche le glyphe que <code>docs/design/systeme.dc.html</code> dessine — la
fonte Material Symbols vendorisée, <b>embarquée ici</b>, celle que Wind expédie réellement. À
droite le redessin Elements, lu dans <code>jeu.mjs</code>. Les deux aux mêmes tailles, sur le
même fond. Le croisement des inventaires a sorti un écart qui ne doit rien à cette direction :
il est en tête.</p>

<section>
  <div class="chiffres">
    <div><b>${DC.size}</b><span>glyphes dessinés par le DC</span></div>
    <div><b>${FONTE.length}</b><span>glyphes dans la fonte vendorisée</span></div>
    <div><b>${CODE.size}</b><span>employés par ui-v2</span></div>
    <div><b>${Object.keys(JEU).length}</b><span>redessinés en Elements</span></div>
  </div>
</section>

<section>
  <h2 class="display">Le DC et le produit ont divergé</h2>
  <p class="sub">A18 : « ce document est la source unique : ce qu'il dessine est livré, ce qui
  est livré s'y dessine. » Le relevé dit que ce n'est plus vrai sur ${absentsDuDC.length + perimesAuDC.length}
  glyphes. Cet écart existe indépendamment de la direction Elements — il se corrige dans le DC,
  pas dans un spike.</p>

  <div class="ecart grave">
    <h3>${absentsDuDC.length} glyphes sont LIVRÉS mais ne sont dessinés nulle part dans le DC</h3>
    <p>Ils vivent dans la fonte et dans le code, pas dans la référence. Quatre d'entre eux —
    <code>error</code>, <code>link_off</code>, <code>system_update_alt</code>,
    <code>volunteer_activism</code> — sont exactement les « avis RARES de la fente » que le
    README des icônes dit avoir découverts absents de la police au terrain 0.1.4. Ils ont été
    ajoutés à la fonte ; personne ne les a ajoutés au dessin. Ils ne se voient pas, parce qu'ils
    s'affichent correctement.</p>
    <ul>${liste(absentsDuDC)}</ul>
  </div>

  <div class="ecart">
    <h3>${perimesAuDC.length} glyphes sont DESSINÉS par le DC mais plus employés par le code</h3>
    <p>Ce sont les quatre « réservés » du sous-ensemble : <code>open_in_new</code> (A53, le
    bouton « Rendre indépendante » retiré), <code>storage</code> (A60, le poids rejoint la puce
    du nom), <code>link</code> et <code>format_quote</code> (A62-D1, Lien et Citation quittent
    la barre). La fonte les garde volontairement ; le DC, lui, dessine encore les commandes qui
    les portaient.</p>
    <ul>${liste(perimesAuDC)}</ul>
  </div>

  ${horsFonte.length ? `<div class="ecart grave">
    <h3>${horsFonte.length} glyphes dessinés par le DC ne sont pas dans la fonte</h3>
    <p>Ceux-là s'afficheraient EN TOUTES LETTRES sur un poste.</p>
    <ul>${liste(horsFonte)}</ul></div>`
  : `<div class="ecart"><h3>Aucun glyphe du DC ne manque à la fonte</h3>
    <p>Les ${DC.size} glyphes dessinés par le DC replient tous leur ligature : rien ne
    s'afficherait en toutes lettres. C'est le seul des trois contrôles qui passe.</p></div>`}
</section>

<section>
  <h2 class="display">À quelle taille chacun juge</h2>
  <p class="sub">Le DC pose ses icônes à ces tailles. L'application, elle, les rend de 10 à 18 px
  — <code>.ms</code> vaut 16 px par défaut. Chaque span posé à 22 px dans la référence est un
  glyphe jugé au-dessus de la taille à laquelle il vivra.</p>
  <div class="chiffres">${[...taillesDC.entries()].sort((a, b) => a[0] - b[0]).map(([t, n]) =>
    `<div><b>${n}</b><span>span${n > 1 ? 's' : ''} à ${t} px${t >= 21 ? ' — au-dessus de l’emploi' : ''}</span></div>`).join('')}</div>
  <div class="temoin"><span class="ms">check_circle</span>
    <span><b>Témoin de chargement.</b> Si vous lisez « check_circle » en toutes lettres à gauche,
    la fonte vendorisée ne s'est pas chargée et toute la colonne « Système » de cette page est
    fausse. Si vous voyez une coche cerclée, elle est bonne.</span></div>
</section>

<section>
  <h2 class="display">Glyphe par glyphe</h2>
  <p class="sub">Les ${TOUS.length} noms connus de l'un ou l'autre bord, 16 px puis 40 px.
  Les étiquettes disent l'écart : <i class="b b-manque">absent du DC</i> livré mais jamais
  dessiné · <i class="b b-perime">plus employé</i> dessiné mais retiré du code ·
  <i class="b b-fusion">fusion</i> plusieurs entrées Material pour un seul dessin Elements ·
  <i class="b b-dur">dur</i> la grammaire Elements ne le porte pas à 16 px.</p>
  <div class="paires">${TOUS.map(carte).join('')}</div>
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

writeFileSync(path.join(ici, 'comparaison.html'), html, 'utf8');
console.log(`comparaison.html écrite — ${TOUS.length} glyphes, ${(html.length / 1024).toFixed(0)} Kio (fonte embarquée)`);
console.log(`  DC ${DC.size} · fonte ${FONTE.length} · code ${CODE.size} · Elements ${Object.keys(JEU).length}`);
console.log(`  livrés jamais dessinés : ${absentsDuDC.length} — ${absentsDuDC.join(', ')}`);
console.log(`  dessinés plus employés : ${perimesAuDC.length} — ${perimesAuDC.join(', ')}`);
console.log(`  dessinés hors fonte    : ${horsFonte.length}${horsFonte.length ? ' — ' + horsFonte.join(', ') : ''}`);

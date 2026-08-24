// ====================================================================
// Socle du Système v2 « Elements » — jetons, banc, rendu des glyphes,
// feuille de style du document.
//
// Le document est GÉNÉRÉ (faire.mjs) : les 78 glyphes viennent de
// `jeu.mjs`, les contrastes sont CALCULÉS ici et non recopiés, le
// journal des amendements est relu dans le document normatif. Une
// planche qui ment sur ce qu'on a dessiné ne prouve rien — la même
// règle vaut pour un Système.
// ====================================================================
import { JEU, MARQUE } from '../jeu.mjs';

// --- Les 17 jetons, deux thèmes -------------------------------------
// Valeurs issues du banc du spike (contraste.mjs). Deux corrections du
// minimum, à teinte constante (remède A8), sur la palette brute du
// document d'icônes : --muted #6E7577 -> #606668 et --border #E3E3DD ->
// #CBC8BB. Le teal #1F8A8A du jeu est gardé EXACT comme --marque
// (composant) et dédoublé d'une encre --accent #1A7A7A de même teinte,
// qui seule tient 4,5:1 en texte.
export const THEMES = {
  elements: {
    libelle: 'Elements — clair',
    defaut: true,
    jetons: {
      bg: '#F3F2EE', surface: '#FFFFFF',
      ink: '#191D1E', ink2: '#565C5E', muted: '#606668',
      border: '#CBC8BB', accent: '#1A7A7A', accentH: '#14625E',
      marque: '#1F8A8A', sel: '#DDE9E6', hover: '#EAE8E1',
      tuile: '#F2EDE3', tuileInk: '#4A4436',
      alert: '#C42D24', onAccent: '#FFFFFF',
      shadow: '0 2px 8px rgba(25,29,30,0.08)', scrim: 'rgba(25,29,30,0.28)',
    },
  },
  'elements-nuit': {
    libelle: 'Elements — nuit',
    defaut: false,
    jetons: {
      bg: '#0D100F', surface: '#171B1A',
      ink: '#ECEDEA', ink2: '#B4BAB8', muted: '#98A0A1',
      border: '#333B3A', accent: '#3FA39C', accentH: '#55B7B0',
      marque: '#3FA39C', sel: '#1E322F', hover: '#141817',
      tuile: '#241F17', tuileInk: '#DFCFAE',
      alert: '#EA9A90', onAccent: '#06211F',
      shadow: '0 2px 12px rgba(0,0,0,0.40)', scrim: 'rgba(0,0,0,0.55)',
    },
  },
};

export const ORDRE_JETONS = [
  'bg', 'surface', 'ink', 'ink2', 'muted', 'border',
  'accent', 'accentH', 'marque', 'sel', 'hover',
  'tuile', 'tuileInk', 'alert', 'onAccent', 'shadow', 'scrim',
];

export const EMPLOI_JETON = {
  bg: ['Fond application', "Fond général, volet liste, volet de lecture, navigation, entête et barre d'état — le sol UNIQUE depuis V3 (--panel est mort)"],
  surface: ['Surface / cartes', "Cartes de message, carte de composition, carte d'invitation, champs, puces, surimpressions"],
  ink: ['Encre principale', 'Titres, objets, corps de message'],
  ink2: ['Texte secondaire', 'Aperçus, valeurs de champs, libellés de puces'],
  muted: ['Texte atténué', "Méta, horodatage, sourcils, barre d'état"],
  border: ['Filets / bordures', "Un seul poids : 1 px, partout, sans exception — et depuis V3 il porte SEUL la séparation des volets. Il borde aussi la tuile d'initiales, qui sans lui n'existerait pas (1,04:1 sur le fond clair)"],
  accent: ['Accent (encre)', "Action principale, compteur de non-lus, anneau de focus, libellés en accent. TEXTE : 4,5:1 tenu sur bg, surface, sel et hover — mais PAS sur tuile (4,38:1), où il ne sert qu'en composant"],
  accentH: ['Accent survol / appui', "Survol et enfoncement des surfaces d'accent"],
  marque: ['Marque (composant)', "Le teal EXACT du jeu d'icônes. Le disque d'état, le rabat de l'enveloppe, l'anneau de cycle, la jauge de migration. JAMAIS du texte (3,70:1 sur le fond clair)"],
  sel: ['Teinte sélection', "Ligne choisie (avec le liseré d'accent), onglet actif, dossier ouvert, voile des pièces jointes et des cartes-portes"],
  hover: ['Teinte survol', 'Survol des rangées : lignes de message et rangées de nav'],
  tuile: ['Tuile', "Le sol des objets Wind : boîte en cours, rangée épinglée, tuile d'initiales (V4), tuile de date d'invitation (A76)"],
  tuileInk: ['Encre de tuile', 'Texte et glyphe posés sur la tuile'],
  alert: ['Alerte', "Échec d'envoi, perte de connexion, mention « Brouillon : », « Supprimer le brouillon », glyphe « Refuser ». Jamais décoratif"],
  onAccent: ['Encre sur accent', "DEUX emplois, et deux seulement : le libellé d'un bouton primaire, et la poignée de l'interrupteur armé"],
  shadow: ['Élévation unique', "Cartes de message, carte de composition, fente d'avis, toast, rangée active du rail des Réglages. Rien d'autre"],
  scrim: ['Voile de surimpression', 'Derrière Réglages, la composition, le tiroir de navigation'],
};

// --- Le nuancier des repères de compte (A74) ------------------------
// Couleurs de CONTENU, pas jetons de thème : elles ne suivent pas la
// direction, elles sont mesurées contre elle. La teinte et le glyphe se
// choisissent SÉPARÉMENT — aucun couple n'est imposé.
export const REPERES_TEINTES = {
  rouge:   ['#a93226', '#f1998e'],
  orange:  ['#9c4a06', '#f2a76c'],
  ocre:    ['#7a5c00', '#e5c04b'],
  olive:   ['#556000', '#c3cc4e'],
  vert:    ['#186a2b', '#7fd18c'],
  sapin:   ['#0b635d', '#5ecec4'],
  bleu:    ['#0a5a8f', '#72bdf0'],
  indigo:  ['#3f4dbb', '#aab4fa'],
  violet:  ['#712cb0', '#cba4f5'],
  magenta: ['#991d7c', '#f095d8'],
  rose:    ['#ad204c', '#f79ab4'],
  brun:    ['#7a4a1b', '#dcab7c'],
};
export const REPERE_GLYPHE = { clair: '#ffffff', nuit: '#1c1b1b' };

// Le nuancier FIXE du composeur (A62-D3) : douze teintes de CONTENU,
// posées dans le corps d'un message, jamais dans l'interface.
export const NUANCIER_COMPOSEUR = [
  '#191d1e', '#565c5e', '#8a908f', '#c42d24', '#b0703c', '#7a5c00',
  '#186a2b', '#0b635d', '#0a5a8f', '#3f4dbb', '#712cb0', '#991d7c',
];

// --- Le banc ---------------------------------------------------------
export function lum(hex) {
  const c = [1, 3, 5].map((i) => {
    const v = parseInt(hex.slice(i, i + 2), 16) / 255;
    return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
}
export function rapport(a, b) {
  const [l1, l2] = [lum(a), lum(b)].sort((x, y) => y - x);
  return (l1 + 0.05) / (l2 + 0.05);
}

// [encre, fond, seuil, où] — la table de la gate expédiée, moins les
// paires sur --panel (le jeton n'existe plus), plus les rôles neufs et
// les CINQ paires de la rangée épinglée, dont le sol est --tuile.
export const PAIRES = [
  ['ink', 'bg', 4.5, 'titres, objets de liste, nav'],
  ['ink', 'surface', 4.5, 'cartes de message, champs'],
  ['ink', 'sel', 4.5, 'rangée sélectionnée, dossier ouvert'],
  ['ink', 'hover', 4.5, 'rangée survolée'],
  ['ink', 'tuile', 4.5, "objet d'une rangée épinglée"],
  ['ink2', 'bg', 4.5, 'expéditeurs, corps de texte'],
  ['ink2', 'surface', 4.5, 'puces, boutons secondaires'],
  ['ink2', 'sel', 4.5, 'aperçus (rangée sélectionnée)'],
  ['ink2', 'hover', 4.5, 'aperçus (rangée survolée)'],
  ['ink2', 'tuile', 4.5, "aperçu d'une rangée épinglée"],
  ['muted', 'bg', 4.5, "heures, sourcils, barre d'état"],
  ['muted', 'surface', 4.5, 'descriptions, texte de substitution'],
  ['muted', 'sel', 4.5, 'heures (rangée sélectionnée)'],
  ['muted', 'hover', 4.5, 'heures (rangée survolée)'],
  ['muted', 'tuile', 4.5, 'heures (rangée épinglée)'],
  ['onAccent', 'accent', 4.5, "libellé du bouton primaire, poignée de l'interrupteur armé"],
  ['onAccent', 'accentH', 4.5, 'libellé du bouton primaire (survol)'],
  ['alert', 'bg', 4.5, "texte d'erreur, mention Brouillon"],
  ['alert', 'sel', 4.5, 'mention Brouillon (rangée choisie)'],
  ['alert', 'hover', 4.5, 'mention Brouillon (rangée survolée)'],
  ['alert', 'tuile', 4.5, 'mention Brouillon (rangée épinglée)'],
  ['alert', 'surface', 4.5, 'mention Brouillon (cartes), Supprimer le brouillon'],
  ['alert', 'surface', 3, "icône d'alerte, point d'anomalie, glyphe « Refuser »"],
  ['accent', 'surface', 3, 'icônes, coche, anneau de focus, glyphe « Accepter »'],
  ['accent', 'bg', 3, 'anneau de focus, liseré, poignée de volet'],
  ['accent', 'sel', 3, 'liseré de la ligne choisie, contour de la réponse en cours'],
  ['accent', 'tuile', 3, "liseré et anneau de focus sur une rangée épinglée — COMPOSANT seulement : 4,38:1 en clair, sous le seuil du texte, aucun libellé d'accent ne se pose sur la tuile"],
  ['accent', 'surface', 4.5, 'compteur de non-lus, liens, libellés en accent (TEXTE)'],
  ['accent', 'bg', 4.5, 'compteur de non-lus de la nav (TEXTE, V4)'],
  ['tuileInk', 'tuile', 4.5, "tuile d'initiales, boîte en cours, rangée épinglée, tuile de date"],
  ['marque', 'bg', 3, 'DISQUE de non-lu et anneau de cycle sur le fond'],
  ['marque', 'surface', 3, 'DISQUE sur une carte'],
  ['marque', 'sel', 3, 'DISQUE sur la rangée choisie'],
  ['marque', 'hover', 3, 'DISQUE sur la rangée survolée'],
  ['marque', 'tuile', 3, 'DISQUE sur la rangée épinglée'],
  ['border', 'bg', 1.49, 'filet sur le fond — seuil = le filet EXPÉDIÉ par Clarity'],
  ['border', 'surface', 1.26, 'filet sur une carte — seuil = le filet EXPÉDIÉ par Clarity'],
  ['border', 'tuile', 1.26, "filet de la tuile d'initiales — la tuile ne vaut que 1,04:1 sur le fond clair, le filet la fait exister"],
];

export function banc() {
  const lignes = [];
  for (const [nom, t] of Object.entries(THEMES)) {
    for (const [encre, fond, seuil, ou] of PAIRES) {
      const r = rapport(t.jetons[encre], t.jetons[fond]);
      lignes.push({ theme: nom, encre, fond, seuil, ou, r, ok: r >= seuil });
    }
  }
  return lignes;
}

// Les repères : pastille (composant, 3:1) sur les CINQ fonds où elle se
// pose — la rangée (bg, sel, hover, tuile), et la carte comme le rail
// des Réglages (surface) —, glyphe (texte, 4,5:1) sur la pastille.
export const FONDS_RANGEE = ['bg', 'surface', 'sel', 'hover', 'tuile'];
export function bancReperes() {
  const out = [];
  for (const [nom, [clair, nuit]] of Object.entries(REPERES_TEINTES)) {
    for (const [theme, hex, glyphe] of [
      ['elements', clair, REPERE_GLYPHE.clair],
      ['elements-nuit', nuit, REPERE_GLYPHE.nuit],
    ]) {
      const t = THEMES[theme].jetons;
      const pire = Math.min(...FONDS_RANGEE.map((f) => rapport(hex, t[f])));
      out.push({
        nom, theme, hex, glyphe,
        pastille: pire, glyphesur: rapport(glyphe, hex),
        ok: pire >= 3 && rapport(glyphe, hex) >= 4.5,
      });
    }
  }
  return out;
}

// --- Rendu ------------------------------------------------------------
export const esc = (s) => String(s).replace(/[&<>"]/g, (c) =>
  ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));

// Le rayon de l'icône d'APPLICATION est un ratio de PLATEFORME (15/64),
// hors du système des trois rayons : c'est l'OS qui le dicte.
export const rayonTuileApp = (px) => Math.max(2, Math.round((px * 15) / 64));

// Un glyphe du jeu. `px` fixe la taille rendue ; le trait reste 2 unités
// sur la grille de 24 — c'est le fait, et il se voit.
export function ico(nom, px = 16, trait = 2) {
  const g = JEU[nom];
  if (!g) throw new Error(`glyphe inconnu : ${nom}`);
  const chemins = g.d.map((d) => `<path d="${d}"/>`).join('');
  const barre = g.barre
    ? `<path d="${g.barre}" fill="none" stroke="var(--marque)" stroke-width="${trait}" stroke-linecap="butt"/>`
    : '';
  const disque = g.disque
    ? `<circle cx="${g.disque[0]}" cy="${g.disque[1]}" r="${g.disque[2]}" fill="var(--marque)"/>`
    : '';
  const pleins = (g.pleins || []).map(([cx, cy, r]) =>
    `<circle cx="${cx}" cy="${cy}" r="${r}" fill="currentColor"/>`).join('');
  const remplis = (g.remplis || []).map((d) =>
    `<path d="${d}" fill="currentColor" stroke="none"/>`).join('');
  return `<svg class="g" viewBox="0 0 24 24" width="${px}" height="${px}" aria-hidden="true">`
    + `<g fill="none" stroke="currentColor" stroke-width="${trait}" stroke-linecap="butt" stroke-linejoin="miter">`
    + `${chemins}</g>${barre}${disque}${pleins}${remplis}</svg>`;
}

// « Transférer » = la flèche de « Répondre » en symétrie verticale (A12).
export const icoMiroir = (nom, px = 16) =>
  ico(nom, px).replace('<svg class="g"', '<svg class="g miroir"');

// La marque EN GLYPHE (entête, tiroir, accueil, migration) : enveloppe à
// l'encre courante, rabat en --marque. V11 : la marque est FIGÉE en
// tuile et THÉMÉE en glyphe — un #141414 figé serait invisible en nuit.
export function marque(px = 24) {
  return `<svg class="g marque" viewBox="0 0 24 24" width="${px}" height="${px}" aria-hidden="true">`
    + `<g fill="none" stroke="currentColor" stroke-width="${MARQUE.trait}" stroke-linecap="butt" stroke-linejoin="miter">`
    + MARQUE.d.map((d) => `<path d="${d}"/>`).join('')
    + `</g><path d="${MARQUE.flap}" fill="var(--marque)"/></svg>`;
}

// La marque EN TUILE : figée hors thèmes (W-D3) — structure #141414,
// tuile #F2EDE3, teal #1F8A8A, identiques dans les deux polarités.
export function marqueTuile(px = 64) {
  const trait = px <= 16 ? 2 : MARQUE.trait;
  return `<span class="marque-tuile" style="width:${px}px;height:${px}px;border-radius:${rayonTuileApp(px)}px">`
    + `<svg viewBox="0 0 24 24" width="${px}" height="${px}" aria-hidden="true">`
    + '<rect width="24" height="24" fill="#F2EDE3"/>'
    + `<g fill="none" stroke="#141414" stroke-width="${trait}" stroke-linecap="butt" stroke-linejoin="miter">`
    + MARQUE.d.map((d) => `<path d="${d}"/>`).join('')
    + '</g><path d="' + MARQUE.flap + '" fill="#1F8A8A"/></svg></span>';
}

// --- La feuille de style du document ----------------------------------
const bloc = (sel, jetons) => `${sel}{\n`
  + ORDRE_JETONS.map((j) => `  --${j}:${jetons[j]};`).join('\n')
  + `\n  color-scheme:${jetons.bg === '#F3F2EE' ? 'light' : 'dark'};\n}`;

// Le nuancier des repères EN CSS, comme systeme.css le fait : la teinte
// SUIT la polarité. Sans cela, une pastille figée au clair rend son
// glyphe à 2,35:1 en nuit — le défaut que ce document portait.
const cssReperes = Object.entries(REPERES_TEINTES)
  .map(([nom, [clair]]) => `.rep[data-teinte="${nom}"]{background:${clair}}`).join('\n')
  + '\n'
  + Object.entries(REPERES_TEINTES)
    .map(([nom, [, nuit]]) => `[data-theme="elements-nuit"] .rep[data-teinte="${nom}"]{background:${nuit}}`).join('\n');

export const CSS = `
/* ===== GÉNÉRÉ — ne pas éditer à la main. Source : spikes/direction-elements/v2/ ===== */

/* --- Les 17 jetons, DEUX thèmes. Toute couleur passe par un jeton :
       la bascule de thème est O(1) via <html data-theme="…">. --- */
${bloc(':root', THEMES.elements.jetons)}
${bloc(':root[data-theme="elements-nuit"]', THEMES['elements-nuit'].jetons)}

/* --- LES COINS SONT DROITS (V14, verdict du Chef Ingénieur).
       Zéro rayon. Deux formes rondes dans tout le système, et elles
       disent chacune quelque chose : le DISQUE (l'état, l'identité) et
       la PILULE de l'interrupteur (le glissement). Rien d'autre.

       Les trois jetons restent — ils tiennent la règle à la place de
       l'oeil : il n'y a plus un seul littéral de rayon à écrire, donc
       plus une seule valeur à laisser filer. Ils sont déclarés sur html
       et non sur :root : ils ne dépendent pas de la polarité, et le
       contrat des jetons de couleur ne doit pas s'en trouver gonflé.
       Rembobiner tient en une ligne — remettre 10 / 6 / 2 ici. --- */
html{--r-surface:0;--r-controle:0;--r-tuile:0}

*{box-sizing:border-box}
html,body{margin:0;padding:0}
html{scroll-behavior:smooth}
body{background:var(--bg);color:var(--ink);-webkit-font-smoothing:antialiased;
  font-family:"Segoe UI Variable Text","Segoe UI",-apple-system,BlinkMacSystemFont,ui-sans-serif,system-ui,sans-serif}
a{color:var(--accent);text-decoration:underline;text-underline-offset:2px}
code{font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;font-size:.92em}
b{font-weight:600}

/* --- A8 : le focus est VISIBLE, partout, au clavier — anneau d'accent
       de 2 px décalé de 2 px. Le document s'applique sa propre règle. --- */
:focus-visible{outline:2px solid var(--accent);outline-offset:2px;border-radius:var(--r-tuile)}

/* --- A8 : le mouvement respecte la préférence de l'utilisateur. --- */
@media (prefers-reduced-motion:reduce){
  html{scroll-behavior:auto}
  *,*::before,*::after{animation-duration:.01ms !important;animation-iteration-count:1 !important;
    transition-duration:.01ms !important}
}

/* --- Registre d'AFFICHAGE : graisse 340, interlettrage serré. La
       grammaire du document d'icônes, réservée aux deux plus grands
       corps (V5) — l'autorité reste graduée par la TAILLE. --- */
.display{font-family:"Segoe UI Variable Display","Segoe UI",-apple-system,ui-sans-serif,system-ui,sans-serif;
  font-weight:340;letter-spacing:-.03em}

.dc{max-width:1540px;margin:0 auto;padding:0 32px 140px;display:flex;flex-direction:column;gap:76px}
.dc>header{display:flex;flex-direction:column;gap:14px;max-width:860px;padding-top:56px}
.sourcil{font-size:11px;letter-spacing:.2em;text-transform:uppercase;color:var(--muted);font-weight:600;margin:0}
h1{font-size:48px;line-height:1.04;margin:0}
h2{font-size:26px;line-height:1.2;margin:0;font-weight:600;letter-spacing:-.02em}
h3{font-size:17px;line-height:1.3;margin:0;font-weight:600;letter-spacing:-.01em}
h3.sourcil{font-size:11px;line-height:1.4;letter-spacing:.2em}
.lede{font-size:16px;line-height:1.68;color:var(--ink2);margin:0;max-width:74ch}
.sub{font-size:14px;line-height:1.68;color:var(--ink2);margin:0;max-width:92ch}
.note{font-size:12.5px;line-height:1.6;color:var(--muted);margin:0;max-width:92ch}
/* Le sommaire est collant : l'ancre d'une section se pose SOUS lui,
   quel que soit le nombre de rangs qu'il occupe (mesuré : 82 px à
   1280, jusqu'a trois rangs plus bas). */
section{display:flex;flex-direction:column;gap:22px;scroll-margin-top:124px}
.rangeeH{display:flex;align-items:baseline;gap:14px;flex-wrap:wrap}
.etiq{font-size:11px;letter-spacing:.16em;text-transform:uppercase;color:var(--muted);font-weight:600;margin:0}

/* --- Le bandeau d'exploration : ce document n'est PAS le normatif. --- */
.expl{border:1px solid var(--alert);border-radius:var(--r-surface);padding:16px 20px;display:flex;gap:14px;
  align-items:flex-start;max-width:1100px;background:var(--surface)}
.expl .t{font-size:13px;line-height:1.6;color:var(--ink2);margin:0}
.expl .t b{color:var(--alert)}

/* --- Sommaire COLLANT : le document fait 60 000 px de haut. --- */
.sommaire{position:sticky;top:0;z-index:40;display:flex;flex-wrap:wrap;gap:6px;
  background:var(--bg);border-bottom:1px solid var(--border);margin:0 -32px;padding:10px 32px}
.sommaire a{font-size:11.5px;line-height:1;color:var(--ink2);text-decoration:none;padding:7px 11px;
  border:1px solid var(--border);border-radius:var(--r-controle);background:var(--surface);white-space:nowrap}
.sommaire a:hover{background:var(--hover);color:var(--ink)}
.haut{align-self:flex-start;font-size:11.5px;color:var(--muted);text-decoration:none;
  border:1px solid var(--border);border-radius:var(--r-controle);background:var(--surface);padding:6px 10px}
.haut:hover{background:var(--hover);color:var(--ink)}

/* --- La pilule de thème : HORS produit, c'est l'outil du document. --- */
.pilules{position:fixed;right:20px;bottom:20px;z-index:50;display:flex;flex-direction:column;gap:8px;align-items:flex-end}
.pilule{display:flex;gap:6px;padding:6px;
  background:var(--surface);border:1px solid var(--border);border-radius:var(--r-surface);box-shadow:var(--shadow)}
.pilule button{height:28px;padding:0 15px;font:inherit;font-size:12px;font-weight:600;letter-spacing:.04em;
  border:0;border-radius:var(--r-controle);background:transparent;color:var(--muted);cursor:pointer}
.pilule button[aria-pressed="true"]{background:var(--sel);color:var(--ink)}

/* --- Fiches et tables du document --- */
.fiches{display:grid;grid-template-columns:repeat(auto-fit,minmax(330px,1fr));gap:22px}
.fiches.duo{grid-template-columns:repeat(2,1fr)}
.fiche{background:var(--surface);border:1px solid var(--border);border-radius:var(--r-surface);padding:24px;
  display:flex;flex-direction:column;gap:13px}
.fiche p{margin:0;font-size:13.5px;line-height:1.65;color:var(--ink2)}
.tbl{width:100%;border-collapse:collapse;background:var(--surface);border:1px solid var(--border);
  border-radius:var(--r-surface);overflow:hidden}
.tbl thead th{text-align:left;padding:13px 16px;font-size:10.5px;letter-spacing:.13em;text-transform:uppercase;
  color:var(--muted);font-weight:600;background:var(--hover)}
.tbl td,.tbl tbody th{padding:10px 16px;font-size:13px;line-height:1.55;color:var(--ink2);
  border-top:1px solid var(--border);text-align:left;vertical-align:top}
.tbl tbody th{font-weight:600;color:var(--ink);white-space:nowrap}
.tbl td.mono{font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;font-size:11.5px;white-space:nowrap}
.tbl td.num{font-variant-numeric:tabular-nums;white-space:nowrap;text-align:right}
.tbl td.nw{white-space:nowrap}
.tbl tr.ok td.num{color:var(--ink2)}
.tbl tr.ko td.num{color:var(--alert);font-weight:600}
.swatch{width:26px;height:26px;border-radius:var(--r-controle);border:1px solid var(--border);display:inline-block;vertical-align:middle}
.verdict{display:inline-flex;align-items:center;gap:8px;font-size:12.5px;color:var(--ink2);
  border:1px solid var(--border);border-radius:var(--r-controle);padding:7px 12px;background:var(--surface)}
.verdict .d{width:9px;height:9px;border-radius:50%;background:var(--marque)}

/* --- Le jeu d'icônes --- */
.grille{display:grid;grid-template-columns:repeat(auto-fill,minmax(112px,1fr));gap:1px;
  background:var(--border);border:1px solid var(--border);border-radius:var(--r-surface);overflow:hidden}
.cell{margin:0;background:var(--surface);padding:16px 8px 11px;display:flex;flex-direction:column;
  align-items:center;gap:11px;min-height:90px;justify-content:center}
.cell .gl{color:var(--ink);display:grid;place-items:center;height:24px}
.cell figcaption{font-size:10px;line-height:1.35;color:var(--muted);text-align:center;word-break:break-word}
.cell.c-arbitrage figcaption{text-decoration:underline dotted;text-underline-offset:3px}
.cell.c-dur{background:var(--tuile)}
.cell.c-dur figcaption{color:var(--tuileInk)}
.cell.c-dur .gl{color:var(--tuileInk)}
.cell.reserve{opacity:.45}
.legende{display:flex;gap:22px;flex-wrap:wrap;font-size:12px;color:var(--muted)}
.legende span{display:inline-flex;align-items:center;gap:8px}
.cle{width:12px;height:12px;border-radius:var(--r-tuile);border:1px solid var(--border);display:inline-block}
.echelle{border-collapse:collapse;border:1px solid var(--border);border-radius:var(--r-surface);overflow:hidden;background:var(--surface)}
.echelle th,.echelle td{border:1px solid var(--border);padding:12px 14px;text-align:center;vertical-align:middle}
.echelle thead th{font-size:10.5px;letter-spacing:.13em;text-transform:uppercase;color:var(--muted);font-weight:600}
.echelle tbody th{font-size:11.5px;font-weight:600;text-align:left;white-space:nowrap;color:var(--ink2)}
.echelle td span{color:var(--ink);display:inline-grid;place-items:center}
.miroir{transform:scaleX(-1)}
.bande{display:flex;gap:12px;flex-wrap:wrap;align-items:center}

/* La marque en TUILE : figée hors thèmes (W-D3). Le rayon est un ratio
   de plateforme (15/64), pas un rayon du système. */
.marque-tuile{display:grid;place-items:center;overflow:hidden;flex:none;background:#F2EDE3}
.marque-tuile svg{display:block}

/* ==================================================================
   L'APPLICATION — le même dessin partout, aux jetons.
   ZÉRO rayon (V14). Deux formes rondes, et elles portent un sens : le
   DISQUE (50 %) dit l'état et l'identité, la PILULE (999) dit le
   glissement — et elle ne sert qu'à la piste de l'interrupteur.
   ================================================================== */
.app{width:1440px;max-width:100%;background:var(--bg);border:1px solid var(--border);border-radius:var(--r-surface);
  overflow:hidden;display:flex;flex-direction:column;color:var(--ink);font-size:13px}
svg.g{flex:none}

/* Entête 52 px — gouttières 14/12, recherche bornée à 520 px. */
.app-entete{height:52px;flex:none;display:flex;align-items:center;gap:12px;padding:0 14px 0 12px;
  border-bottom:1px solid var(--border)}
.app-marque{display:flex;align-items:center;gap:9px;width:212px;flex:none;color:var(--ink)}
.app-marque b{font-size:15px;font-weight:600;letter-spacing:-.012em}
.recherche{flex:1;max-width:520px;height:32px;border:1px solid var(--border);border-radius:var(--r-controle);
  background:var(--surface);display:flex;align-items:center;gap:9px;padding:0 12px;color:var(--muted);font-size:13px}
.recherche.active{color:var(--ink);border-color:var(--accent)}
.gestes{margin-left:auto;display:flex;align-items:center;gap:8px}

/* Trois volets : 248 / 400 / 1fr. */
.app-corps{display:grid;grid-template-columns:248px 400px 1fr;min-height:0;flex:1}
.app-corps.deux{grid-template-columns:248px 1fr}
.app-corps.un{grid-template-columns:1fr}

/* Nav — plus de fond de panneau : le filet fait tout (V3). */
.nav{border-right:1px solid var(--border);padding:10px 8px;display:flex;flex-direction:column;gap:2px;min-width:0}
.nav-titre{font-size:11px;letter-spacing:.2em;text-transform:uppercase;color:var(--muted);font-weight:600;
  margin:18px 0 6px;padding:0 10px}
.rang{height:36px;display:flex;align-items:center;gap:10px;padding:0 10px;border-radius:var(--r-controle);
  color:var(--ink2);min-width:0}
.rang span.l{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.rang .n{margin-left:auto;font-size:12px;font-weight:600;font-variant-numeric:tabular-nums;color:var(--accent)}
.rang.actif{background:var(--sel);color:var(--ink);font-weight:600}
.rang.actif .g{color:var(--accent)}
.rang.survol{background:var(--hover)}
.rang.boite{background:var(--tuile);color:var(--tuileInk)}
.rang.boite .g{color:var(--tuileInk)}

/* Liste */
.liste{border-right:1px solid var(--border);display:flex;flex-direction:column;min-width:0}
.bandeau{height:52px;flex:none;display:flex;align-items:center;gap:10px;padding:0 16px;
  border-bottom:1px solid var(--border)}
.bandeau .t{font-size:16px;font-weight:600;letter-spacing:-.01em}
.bandeau.bas{border-bottom:0;border-top:1px solid var(--border);gap:8px;margin-top:auto}
.bandeau .compte{margin-left:auto;display:flex;align-items:baseline;gap:5px}
.bandeau .compte b{font-size:15px;color:var(--accent);font-variant-numeric:tabular-nums}
.bandeau .compte span{font-size:12px;color:var(--muted);font-variant-numeric:tabular-nums}
.flot{flex:1;min-height:0;overflow:hidden}
.section-liste{font-size:11px;letter-spacing:.2em;text-transform:uppercase;color:var(--muted);font-weight:600;
  padding:11px 16px 7px;margin:0;border-bottom:1px solid var(--border)}

/* La rangée de message — deux gabarits, h1 nue et h2 porteuse (A44). */
.rangee{display:flex;gap:12px;padding:11px 16px 11px 14px;border-bottom:1px solid var(--border);
  position:relative;align-items:flex-start;min-width:0}
.rangee.sel{background:var(--sel)}
.rangee.sel::before{content:"";position:absolute;left:0;top:0;bottom:0;width:2px;background:var(--accent)}
.rangee.survol{background:var(--hover)}
.rangee.epingle{background:var(--tuile)}
.rangee .col{display:flex;flex-direction:column;align-items:center;gap:4px;flex:none}
.rangee .txt{min-width:0;flex:1;display:flex;flex-direction:column;gap:3px}
.l1{display:flex;align-items:center;gap:8px;min-width:0}
.l1 .exp{font-size:13px;color:var(--ink2);overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.l1 .h{margin-left:auto;font-size:12px;color:var(--muted);white-space:nowrap;font-variant-numeric:tabular-nums}
.obj{font-size:14px;color:var(--ink);overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.apr{font-size:13px;color:var(--ink2);overflow:hidden;text-overflow:ellipsis;white-space:nowrap;line-height:1.45}
.rangee.nonlu .exp{font-weight:600;color:var(--ink)}
.rangee.nonlu .obj{font-weight:700}
.brouillon{color:var(--alert);font-weight:600}
.surligne{background:var(--sel);border-radius:var(--r-tuile);padding:0 2px}

/* La TUILE d'initiales — un CARRÉ NET (V4, porté à zéro par V14) : le
   rond est rendu au disque.
   Elle porte le FILET : mesurée, la tuile ne vaut que 1,04:1 sur le fond
   clair — sans filet elle n'existe pas. Le filet la borde à 1,44:1. */
.tuileini{width:28px;height:28px;border-radius:var(--r-tuile);background:var(--tuile);color:var(--tuileInk);
  border:1px solid var(--border);
  display:flex;align-items:center;justify-content:center;font-size:11px;font-weight:600;flex:none;
  letter-spacing:.02em}
.tuileini.p26{width:26px;height:26px;font-size:10.5px}

/* Le DISQUE — un seul emploi dans tout le système : l'état. Ø 9.
   L'ANNEAU — le MÊME disque, Ø 9 lui aussi, évidé, pendant qu'une
   action tourne. Le vide du haut rend la rotation lisible. */
.disque{width:9px;height:9px;border-radius:50%;background:var(--marque);flex:none}
.anneau{width:9px;height:9px;border-radius:50%;flex:none;
  border:2px solid var(--marque);border-top-color:transparent;animation:tourne 1s linear infinite}
@keyframes tourne{to{transform:rotate(360deg)}}
@media (prefers-reduced-motion:reduce){.anneau{animation:none}}

/* La pastille de repère de compte (A74) — le SEUL autre rond, et il
   porte un glyphe que la couleur seule ne dirait pas. La teinte SUIT la
   polarité : figée au clair, son glyphe tomberait à 2,35:1 en nuit. */
.rep{width:16px;height:16px;border-radius:50%;display:inline-flex;align-items:center;justify-content:center;
  flex:none;color:${REPERE_GLYPHE.clair}}
[data-theme="elements-nuit"] .rep{color:${REPERE_GLYPHE.nuit}}
/* .echantillon : la SEULE pastille dont la teinte est figée en ligne —
   la table de référence du nuancier doit montrer les deux valeurs
   quel que soit le thème affiché. Partout ailleurs, data-teinte. */
.rep.echantillon{background:none}
.rep.p24{width:24px;height:24px}
.rep.p32{width:32px;height:32px}
${cssReperes}

/* Le rang de puces (A44) */
.puces{display:flex;gap:6px;flex-wrap:wrap;align-items:center}
.puce{height:24px;display:inline-flex;align-items:center;gap:6px;padding:0 12px;border-radius:var(--r-controle);
  border:1px solid var(--border);background:var(--surface);font-size:12px;color:var(--ink2);white-space:nowrap}
.puce .poids{color:var(--muted)}
.puce.h32{height:32px;font-size:13px}

/* A70 — le voile d'une pièce jointe : RECOUVREMENT ABSOLU, géométrie
   stable, fond --sel. Ce n'est jamais une puce de plus. */
.pj{position:relative;display:inline-flex}
.pj .voile-pj{position:absolute;inset:0;border-radius:var(--r-controle);background:var(--sel);
  box-shadow:inset 0 0 0 1px var(--accent);display:flex;align-items:center;justify-content:center;gap:6px;
  font-size:12px;font-weight:600;color:var(--ink)}

/* Boutons, onglets — même rayon 6, même filet, même hauteur 32. */
.btn{height:32px;display:inline-flex;align-items:center;gap:8px;padding:0 14px;border-radius:var(--r-controle);
  border:1px solid var(--border);background:var(--surface);color:var(--ink2);font-size:13px;
  font-family:inherit;white-space:nowrap}
.btn.primaire{background:var(--accent);border-color:var(--accent);color:var(--onAccent);font-weight:600}
.btn.primaire.survol{background:var(--accentH);border-color:var(--accentH)}
.btn.survol{background:var(--hover)}
.btn.appui{background:var(--sel)}
.btn.eteint{opacity:.4}
.btn.alerte{color:var(--alert)}
.btn.nu{border-color:transparent;background:transparent;height:26px;padding:0 8px;color:var(--ink2)}
.btn.icone{width:32px;padding:0;justify-content:center}
.btn.actif{background:var(--sel);border-color:var(--accent);color:var(--ink);font-weight:600}
.btn.h30{height:30px}
.onglet{height:32px;display:inline-flex;align-items:center;gap:8px;padding:0 14px;border-radius:var(--r-controle);
  border:1px solid var(--border);background:var(--surface);color:var(--ink2);font-size:13px;white-space:nowrap}
.onglet.actif{background:var(--sel);color:var(--ink);font-weight:600}
.onglet.actif .g{color:var(--accent)}

/* Le fil — À PLAT (A46/A72) : seules les cartes s'élèvent. */
.fil{display:flex;flex-direction:column;min-width:0;padding:18px 22px;gap:14px;overflow:hidden}
.fil-tete{display:flex;align-items:center;gap:12px;flex-wrap:wrap}
.fil-tete .t{font-size:24px;line-height:1.2;flex:1 1 100%;min-width:0;overflow:hidden;
  text-overflow:ellipsis;white-space:nowrap}
.carte{background:var(--surface);border-radius:var(--r-surface);box-shadow:var(--shadow);padding:16px;
  display:flex;flex-direction:column;gap:12px}
.carte.replie{flex-direction:row;align-items:center;gap:10px;padding:10px 16px}
.carte.replie .nom{font-size:13px;font-weight:600;color:var(--ink);white-space:nowrap}
.carte.replie .ap{font-size:13px;color:var(--ink2);overflow:hidden;text-overflow:ellipsis;white-space:nowrap;flex:1;min-width:0}
.carte.replie .h{font-size:12px;color:var(--muted);white-space:nowrap}
.msg-tete{display:flex;align-items:center;gap:10px}
.msg-tete .nom{font-size:15px;font-weight:600;line-height:1.25}
.msg-tete .adr{font-size:12px;color:var(--muted)}
.msg-tete .h{margin-left:auto;font-size:12px;color:var(--muted);white-space:nowrap}

/* Le corps du courriel : la dalle BAKÉE par mail-render — encre #222222,
   fond #ffffff (crates/mail-render/src/lib.rs, Palette::default), dans
   les DEUX thèmes (A61). Wind ne pose QUE ces deux valeurs : le lien
   garde ce que le message, ou le navigateur, lui donne. */
.corps-mail{background:#ffffff;color:#222222;border-radius:var(--r-controle);padding:14px 16px;font-size:15px;
  line-height:1.65;border:1px solid var(--border)}
.corps-mail p{margin:0 0 12px}
.corps-mail p:last-child{margin-bottom:0}
.corps-mail a{color:LinkText}
.garde-images{padding:10px 14px;display:flex;align-items:center;gap:10px;font-size:13px;color:var(--ink2);
  background:var(--bg);border:1px solid var(--border);border-radius:var(--r-controle)}
.barre-msg{display:flex;gap:8px;padding-top:2px}
.barre-fil{display:flex;gap:8px;flex-wrap:wrap;padding-top:4px}
.joints{display:flex;flex-direction:column;gap:8px}

/* La carte d'INVITATION (A76) : une carte DANS la carte de message —
   rayon 10, SANS élévation : elle appartient au flot du contenu. */
.invitation{border:1px solid var(--border);border-radius:var(--r-surface);background:var(--surface)}
.inv-tete{display:flex;align-items:center;gap:10px;padding:12px 14px 0}
.inv-kicker{font-size:12px;font-weight:600;letter-spacing:.1em;text-transform:uppercase;color:var(--muted);flex:1}
.inv-kicker.annulee{color:var(--alert)}
.inv-statut{font-size:12px;color:var(--ink2);white-space:nowrap}
.inv-corps{display:flex;gap:14px;padding:12px 14px 14px;align-items:flex-start}
.inv-tuile{width:52px;height:52px;border-radius:var(--r-tuile);background:var(--tuile);color:var(--tuileInk);
  border:1px solid var(--border);
  display:flex;flex-direction:column;align-items:center;justify-content:center;gap:1px;flex:none}
.inv-tuile.eteinte{background:var(--bg);color:var(--muted)}
.inv-mois{font-size:10px;font-weight:600;letter-spacing:.08em;text-transform:uppercase}
.inv-jour{font-size:20px;font-weight:600;line-height:1}
.inv-details{display:flex;flex-direction:column;gap:4px;min-width:0}
.inv-titre{font-size:15px;font-weight:600;color:var(--ink)}
.inv-titre.barre{color:var(--ink2);text-decoration:line-through}
.inv-quand{font-size:13px;color:var(--ink2)}
.inv-lieu{font-size:13px;color:var(--muted)}
.inv-annulee{font-size:13px;color:var(--alert)}
.inv-repondant{font-size:13px;font-weight:600;color:var(--ink2)}
.inv-actions{display:flex;gap:10px;padding:12px 14px;border-top:1px solid var(--border);flex-wrap:wrap}
.ton-accepte .g{color:var(--accent)}
.ton-provisoire .g{color:var(--muted)}
.ton-refuse .g{color:var(--alert)}

/* Barre d'état 36 px — la région du CONTINU. */
.statut{height:36px;flex:none;display:flex;align-items:center;gap:10px;padding:0 14px;
  border-top:1px solid var(--border);color:var(--muted);font-size:12px}
.statut .txt{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.statut .pt{width:7px;height:7px;border-radius:50%;background:var(--alert);flex:none}
.statut .bt{margin-left:auto}

/* Champs et formulaires */
.champ{height:40px;display:flex;align-items:center;padding:0 12px;border:1px solid var(--border);
  border-radius:var(--r-controle);background:var(--surface);color:var(--muted);font-size:13px;min-width:0}
.champ.h32{height:32px}
.champ.plein{color:var(--ink)}
.champ.zone{height:auto;min-height:78px;align-items:flex-start;padding:10px 12px;line-height:1.6}
.etiqchamp{font-size:12px;font-weight:600;letter-spacing:.1em;text-transform:uppercase;color:var(--muted)}
/* L'interrupteur : piste 36 × 20, poignée 16 — la cote LIVRÉE. */
.inter{width:36px;height:20px;border-radius:999px;background:var(--surface);border:1px solid var(--border);
  display:inline-flex;align-items:center;padding:1px;flex:none}
.inter i{width:16px;height:16px;border-radius:50%;background:var(--muted);display:block}
.inter.arme{background:var(--accent);border-color:var(--accent);justify-content:flex-end}
.inter.arme i{background:var(--onAccent)}

/* Surimpressions */
.scene{position:relative;width:1440px;max-width:100%;border:1px solid var(--border);border-radius:var(--r-surface);
  overflow:hidden;background:var(--bg)}
.voile{position:absolute;inset:0;background:var(--scrim);display:flex;align-items:center;justify-content:center}
.modale{background:var(--surface);border-radius:var(--r-surface);box-shadow:var(--shadow);overflow:hidden;display:flex;
  flex-direction:column}
.modale-tete{height:48px;flex:none;display:flex;align-items:center;gap:10px;padding:0 16px;
  border-bottom:1px solid var(--border)}
.modale-tete .t{font-size:15px;font-weight:600}
.modale-pied{height:56px;flex:none;display:flex;align-items:center;gap:8px;padding:0 16px;
  border-top:1px solid var(--border);justify-content:flex-end}

/* Colonne d'accueil / migration : la géométrie de l'écran 01. */
.colonne{width:520px;max-width:100%;display:flex;flex-direction:column;gap:18px}
.colonne.large{width:760px}
.jauge{height:6px;background:var(--hover);overflow:hidden}
.jauge i{display:block;height:100%;background:var(--marque)}
.jauge.indet i{width:38%;animation:glisse 1.6s ease-in-out infinite}
@keyframes glisse{0%{margin-left:0}50%{margin-left:62%}100%{margin-left:0}}
@media (prefers-reduced-motion:reduce){.jauge.indet i{animation:none;width:100%;opacity:.45}}

/* Cartes-portes du récapitulatif (A75) */
.portes{display:grid;grid-template-columns:repeat(3,1fr);gap:14px}
.porte{position:relative;background:var(--surface);border:1px solid var(--border);border-radius:var(--r-surface);
  padding:14px;display:flex;flex-direction:column;gap:8px;overflow:hidden}
.porte.choisi{background:var(--sel);box-shadow:inset 0 0 0 2px var(--accent)}
.porte .vl{position:absolute;inset:0;background:var(--sel);display:flex;align-items:center;justify-content:center;
  gap:8px;font-size:13px;font-weight:600;color:var(--ink)}
.mini{height:64px;border:1px solid var(--border);border-radius:var(--r-controle);display:flex;overflow:hidden}
.mini i{display:block;height:100%}
.mini.h96{height:96px}

/* Fente d'avis et toast */
.avis{background:var(--surface);border:1px solid var(--border);border-radius:var(--r-surface);box-shadow:var(--shadow);
  padding:12px 14px;display:flex;align-items:center;gap:10px;font-size:13px;color:var(--ink2)}
.toast{background:var(--surface);border:1px solid var(--border);border-radius:var(--r-controle);box-shadow:var(--shadow);
  padding:10px 14px;display:inline-flex;align-items:center;gap:10px;font-size:13px;color:var(--ink2)}

/* Le rail des Réglages */
.rail{width:220px;flex:none;border-right:1px solid var(--border);padding:10px 8px;display:flex;
  flex-direction:column;gap:2px}
.rail .rang.actif{background:var(--surface);box-shadow:var(--shadow);color:var(--ink)}
.reglage{display:flex;align-items:flex-start;gap:16px;padding:14px 0;border-bottom:1px solid var(--border)}
.reglage:last-child{border-bottom:0}
.reglage .d{flex:1;min-width:0;display:flex;flex-direction:column;gap:4px}
.reglage .d b{font-size:13px;font-weight:600;color:var(--ink)}
.reglage .d span{font-size:12.5px;line-height:1.55;color:var(--muted)}
.touche{display:inline-flex;align-items:center;justify-content:center;min-width:26px;height:24px;padding:0 7px;
  border:1px solid var(--border);border-radius:var(--r-controle);background:var(--bg);font-size:12px;font-weight:600;
  color:var(--ink);font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace}

/* La barre de mise en forme du composeur, et son nuancier */
.miseenforme{display:flex;align-items:center;gap:4px;flex-wrap:wrap;padding:8px 12px;
  border-top:1px solid var(--border);border-bottom:1px solid var(--border);background:var(--bg)}
.sep{width:1px;height:20px;background:var(--border);margin:0 4px}
.nuancier{display:grid;grid-template-columns:repeat(6,26px);gap:6px;padding:12px;background:var(--surface);
  border:1px solid var(--border);border-radius:var(--r-surface);box-shadow:var(--shadow);width:fit-content}
.nuancier i{width:26px;height:26px;border-radius:var(--r-controle);border:1px solid var(--border);display:block}

/* Les schémas de disposition */
.schemas{display:flex;gap:16px;flex-wrap:wrap}
.schema{width:196px;display:flex;flex-direction:column;gap:7px}
.schema .cadre{height:106px;border:1px solid var(--border);border-radius:var(--r-controle);display:flex;overflow:hidden;
  background:var(--bg)}
.schema .cadre i{display:block;height:100%;border-right:1px solid var(--border)}
.schema .cadre i:last-child{border-right:0}
.schema b{font-size:12px;font-weight:600;color:var(--ink)}
.schema span{font-size:11.5px;line-height:1.5;color:var(--muted)}
.poignee{width:7px;background:var(--accent);opacity:.9}

@media(max-width:1200px){.dc{padding:0 18px 90px}.fiches.duo{grid-template-columns:1fr}
  .sommaire{margin:0 -18px;padding:10px 18px}}
`;

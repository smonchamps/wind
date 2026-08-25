// ====================================================================
// Socle du spike « repère de boîte » — la matière commune aux planches.
//
// Une seule implémentation de la lecture des jetons, du rendu des
// glyphes et du dessin de la rangée : deux copies divergeraient en
// silence, et c'est exactement le défaut que le Système reproche aux
// maquettes qui recopient leurs hex.
//
// Tout se LIT du produit :
//   apps/desktop/ui-v2/src/systeme.css     les 17 jetons × 2 thèmes,
//                                          les 24 hex du nuancier A74
//   apps/desktop/ui-v2/src/lib/icones.js   les 78 glyphes du jeu livré
//   apps/desktop/ui-v2/src/lib/initiales.js les initiales du témoin
//   e2e/jetons.mjs                          le parseur des gates
// ====================================================================
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { JEU } from '../../apps/desktop/ui-v2/src/lib/icones.js';
import { lireThemes } from '../../e2e/jetons.mjs';

export { initiales } from '../../apps/desktop/ui-v2/src/lib/initiales.js';

export const RACINE = path.resolve(import.meta.dirname, '..', '..');
const css = readFileSync(
  path.join(RACINE, 'apps', 'desktop', 'ui-v2', 'src', 'systeme.css'),
  'utf8',
);

// --- Les jetons expédiés ---------------------------------------------
const THEMES = lireThemes(css, { motifValeur: /#[0-9a-fA-F]{6}/ });
export const CLAIR = THEMES.elements;
export const NUIT = THEMES['elements-nuit'];
if (!CLAIR || !NUIT) throw new Error('systeme.css : les deux thèmes ne se lisent pas');

// --- Le nuancier des repères (mêmes regex que e2e/contraste.mjs) ------
function lireReperes(prefixe) {
  const r = {};
  for (const [, teinte, hex] of css.matchAll(new RegExp(
    `${prefixe}\\.repere\\[data-teinte="([a-z]+)"\\]\\s*\\{\\s*background:(#[0-9a-fA-F]{6})`,
    'g',
  ))) r[teinte] = hex;
  return r;
}
export const REP_SOMBRES = lireReperes('(?<!-nuit"\\] )');
export const REP_CLAIRES = lireReperes('\\[data-theme\\$="-nuit"\\] ');
export const ENCRE_SOMBRE = css.match(/\.repere\s*\{[^}]*color:(#[0-9a-fA-F]{6})/)?.[1];
export const ENCRE_CLAIRE = css.match(
  /\[data-theme\$="-nuit"\] \.repere\s*\{[^}]*color:(#[0-9a-fA-F]{6})/,
)?.[1];
if (Object.keys(REP_SOMBRES).length !== 12 || Object.keys(REP_CLAIRES).length !== 12) {
  throw new Error('nuancier des repères : 12 familles attendues par polarité');
}
export const TEINTES = Object.keys(REP_SOMBRES);

// --- Contraste WCAG (mêmes formules que la gate) ----------------------
function lum(hex) {
  const c = [1, 3, 5].map((i) => {
    const v = parseInt(hex.slice(i, i + 2), 16) / 255;
    return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
}
export const rapport = (a, b) => {
  const [l1, l2] = [lum(a), lum(b)].sort((x, y) => y - x);
  return (l1 + 0.05) / (l2 + 0.05);
};
export const fmt = (n) => n.toFixed(2).replace('.', ',');

// --- Les glyphes, rendus comme Icone.svelte ---------------------------
export function ico(nom, taille = 16, classe = '') {
  const g = JEU[nom] ?? { d: [] };
  const traits = (g.d ?? []).map((d) => `<path d="${d}"/>`).join('');
  const pleins = (g.pleins ?? [])
    .map(([cx, cy, r]) => `<circle cx="${cx}" cy="${cy}" r="${r}" fill="currentColor"/>`).join('');
  const remplis = (g.remplis ?? [])
    .map((d) => `<path d="${d}" fill="currentColor" stroke="none"/>`).join('');
  const disqueIc = g.disque
    ? `<circle cx="${g.disque[0]}" cy="${g.disque[1]}" r="${g.disque[2]}" fill="var(--marque)"/>` : '';
  return `<svg class="ic ${classe}" viewBox="0 0 24 24" width="${taille}" height="${taille}" aria-hidden="true"`
    + `><g fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="butt" stroke-linejoin="miter">`
    + `${traits}</g>${pleins}${remplis}${disqueIc}</svg>`;
}

export const jetonsDe = (t, reperes) =>
  Object.entries(t).map(([k, v]) => `--${k}:${v};`).join('')
  + Object.entries(reperes).map(([n, h]) => `--rep-${n}:${h};`).join('');

// --- Le décor : trois comptes, et des rangées ------------------------
export const COMPTES = {
  travail: { nom: 'Travail', court: 'Travail', icone: 'work', teinte: 'bleu', adresse: 'p.mercier@vantis.fr' },
  maison: { nom: 'Maison', court: 'Maison', icone: 'home', teinte: 'vert', adresse: 'paul@atelier-nord.fr' },
  etudes: { nom: 'Études', court: 'Études', icone: 'school', teinte: 'violet', adresse: 'p.mercier@etu-lyon.fr' },
};

// Les six rangées de la planche comparative — les mêmes pour les huit
// dessins : la comparaison porte sur le repère et sur rien d'autre.
export const LIGNES = [
  {
    c: 'travail', exp: 'Camille Roux', heure: '09:41', nonlu: true,
    objet: 'Contrat Vantis — v4 pour relecture',
    apercu: 'Voici la version corrigée, les deux annexes sont à jour.',
    puces: [['attach_file', '2 fichiers']],
  },
  {
    c: 'travail', exp: 'Sofia Nardi', heure: '08:12', nonlu: true,
    objet: 'Atelier de septembre',
    apercu: 'Deux salles réservées à Milan.',
  },
  {
    c: 'maison', exp: 'Marine Alonso', heure: '4 août',
    objet: 'Relevé de juillet',
    apercu: 'Votre relevé est disponible dans votre espace client.',
  },
  {
    c: 'maison', exp: 'Thomas Petit', heure: '3 août', choisie: true,
    objet: 'Photos du week-end',
    apercu: 'J’ai mis les photos du lac dans le dossier partagé.',
    puces: [['forum', '3 messages']],
  },
  {
    c: 'travail', exp: 'Yann Bernard', heure: '2 août',
    objet: 'Re: planning des salles',
    apercu: 'Je confirme pour jeudi, salle 2 au premier étage.',
  },
  {
    c: 'etudes', exp: 'École Jean-Moulin', heure: '1 août',
    objet: 'Réunion parents-professeurs',
    apercu: 'La réunion se tiendra le mardi 9 septembre à 18 h.',
  },
];

// --- Le décor long : quatorze rangées d'une boîte unifiée -------------
// Trois comptes qui alternent comme ils alternent vraiment quand le tri
// est la date : c'est le cas le plus défavorable au glyphe nu, et le
// seul qui vaille d'être regardé.
export const FIL = [
  { c: 'travail', exp: 'Camille Roux', heure: '09:41', nonlu: true,
    objet: 'Contrat Vantis — v4 pour relecture',
    apercu: 'Voici la version corrigée, les deux annexes sont à jour.',
    puces: [['attach_file', '2 fichiers']] },
  { c: 'travail', exp: 'Sofia Nardi', heure: '09:07', nonlu: true,
    objet: 'Atelier de septembre',
    apercu: 'Deux salles réservées à Milan.' },
  { c: 'maison', exp: 'Marine Alonso', heure: '08:52',
    objet: 'Relevé de juillet',
    apercu: 'Votre relevé est disponible dans votre espace client.' },
  { c: 'travail', exp: 'Thomas Petit', heure: '08:30', choisie: true,
    objet: 'Photos du chantier de Vaise',
    apercu: 'J’ai mis les prises de vue dans le dossier partagé.',
    puces: [['forum', '3 messages']] },
  { c: 'etudes', exp: 'École Jean-Moulin', heure: '08:04', nonlu: true,
    objet: 'Réunion parents-professeurs',
    apercu: 'La réunion se tiendra le mardi 9 septembre à 18 h.' },
  { c: 'travail', exp: 'Yann Bernard', heure: '4 août',
    objet: 'Re: planning des salles',
    apercu: 'Je confirme pour jeudi, salle 2 au premier étage.' },
  { c: 'maison', exp: 'Syndic Beauregard', heure: '4 août',
    objet: 'Assemblée générale — convocation',
    apercu: 'Vous trouverez l’ordre du jour et les pouvoirs en pièce jointe.',
    puces: [['attach_file', '3 fichiers']] },
  { c: 'travail', exp: 'Léa Fontaine', heure: '3 août',
    objet: 'Devis Kessler — relance',
    apercu: 'Sans retour de leur part d’ici vendredi, je relance par téléphone.' },
  { c: 'etudes', exp: 'Secrétariat pédagogique', heure: '3 août', nonlu: true,
    objet: 'Inscription au semestre — pièces manquantes',
    apercu: 'Il manque le justificatif de domicile pour finaliser le dossier.' },
  { c: 'maison', exp: 'Thomas Petit', heure: '2 août',
    objet: 'Week-end du 15 ?',
    apercu: 'On est quatre pour l’instant, dis-moi si vous venez.',
    puces: [['forum', '6 messages']] },
  { c: 'travail', exp: 'Camille Roux', heure: '2 août',
    objet: 'Compte rendu du comité',
    apercu: 'Décisions actées et points reportés au prochain comité.' },
  { c: 'travail', exp: 'Support Vantis', heure: '1 août',
    objet: 'Votre demande #4812 a été résolue',
    apercu: 'Nous restons à votre disposition si le problème réapparaît.' },
  { c: 'maison', exp: 'Marine Alonso', heure: '1 août',
    objet: 'Rendez-vous du 12 septembre',
    apercu: 'Le créneau de 14 h 30 est confirmé.' },
  { c: 'etudes', exp: 'Bibliothèque universitaire', heure: '31 juil.',
    objet: 'Prêt arrivant à échéance',
    apercu: 'Deux ouvrages sont à rendre avant le 5 septembre.' },
];

// Le JOUR d'une rangée, pour les intercalaires : une heure nue dit
// aujourd'hui, une date se dit telle quelle.
export const jourDe = (l) => (/^\d{1,2}:\d{2}$/.test(l.heure) ? "Aujourd'hui" : l.heure);

// Les SUITES de rangées consécutives d'un même compte (le peloton, le
// filet de changement) : calculées une fois, jamais recopiées.
export function suites(lignes) {
  const out = [];
  for (const l of lignes) {
    const derniere = out[out.length - 1];
    if (derniere && derniere[0].c === l.c) derniere.push(l);
    else out.push([l]);
  }
  return out;
}

// --- Les morceaux communs d'une rangée -------------------------------
export const disque = (l) => (l.nonlu ? '<span class="disque"></span>' : '');
export const ligne1 = (l, avant = '', apres = '') =>
  `<div class="l1">${disque(l)}<span class="exp">${l.exp}</span>${avant}`
  + `<span class="heure">${l.heure}</span>${apres}</div>`;
export const corps = (l) =>
  `<p class="objet">${l.objet}</p><p class="apercu">${l.apercu}</p>`;
export const rangPuces = (l, tete = '') => {
  const p = (l.puces ?? []).map(([n, t]) => `<span class="puce">${ico(n, 14)}${t}</span>`).join('');
  if (!tete && !p) return '';
  return `<div class="puces">${tete}${p}</div>`;
};
export const pastille = (cpt, taille = 'p16') =>
  `<span class="repere ${taille}" data-teinte="${cpt.teinte}" title="${cpt.nom} — ${cpt.adresse}">`
  + `${ico(cpt.icone, taille === 'p16' ? 10 : 12)}</span>`;
export const glypheNu = (cpt, taille = 18) =>
  `<span class="glyphe-compte" data-teinte="${cpt.teinte}" title="${cpt.nom} — ${cpt.adresse}">`
  + `${ico(cpt.icone, taille)}</span>`;
export const classes = (l) => `ligne${l.nonlu ? ' nonlu' : ''}${l.choisie ? ' choisie' : ''}`;

// --- Le dessin des pistes, repris de Liste.svelte au pixel ------------
export const CSS_VOLET = `
.theme-clair { ${jetonsDe(CLAIR, REP_SOMBRES)} }
.theme-nuit { ${jetonsDe(NUIT, REP_CLAIRES)} }
.liste { display:flex; flex-direction:column; }
.ligne {
  padding:13px 16px; border-top:1px solid var(--border);
  border-left:2px solid transparent;
  display:grid; grid-template-columns:auto 1fr; column-gap:10px;
  row-gap:3px; align-items:start;
}
.ligne:first-child { border-top:none; }
.ligne.choisie { background:var(--sel); border-left-color:var(--accent); }
.avatar {
  grid-row:1 / span 3; width:28px; height:28px; border-radius:0;
  background:var(--tuile); border:1px solid var(--border);
  display:grid; place-items:center; font-size:11px; font-weight:600; color:var(--tuileInk);
}
.col-avatar { grid-row:1 / span 3; display:flex; flex-direction:column; align-items:center; gap:4px; }
.l1, .objet, .apercu, .puces { grid-column:2; min-width:0; }
.l1 { display:flex; align-items:baseline; gap:10px; }
.l1 .disque { align-self:center; }
.exp { font-size:14px; color:var(--ink); flex:1; min-width:0;
  overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
.nonlu .exp { font-weight:700; }
.heure { font-size:12px; color:var(--muted); flex:none; }
.objet { margin:0; font-size:14px; font-weight:400; line-height:1.3; color:var(--ink);
  overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
.nonlu .objet { font-weight:700; }
.apercu { margin:0; font-size:13px; line-height:1.45; color:var(--ink2);
  overflow:hidden; text-overflow:ellipsis; white-space:nowrap; min-height:1.45em; }
.puces { height:24px; display:flex; align-items:center; gap:6px; overflow:hidden; }
.puce { display:inline-flex; align-items:center; gap:5px; height:24px; padding:0 9px;
  font-size:12px; color:var(--ink2); background:var(--surface);
  border:1px solid var(--border); white-space:nowrap; }
.puce .ic { width:14px; height:14px; }
.disque { width:9px; height:9px; border-radius:50%; background:var(--marque); flex:none; }
.anneau { width:9px; height:9px; border-radius:50%; flex:none;
  border:2px solid var(--marque); border-top-color:transparent; }
.ic { flex:none; vertical-align:middle; }

/* La pastille de repère — la règle du produit, rescopée par thème. */
.repere { display:inline-flex; align-items:center; justify-content:center;
  border-radius:50%; flex:none; color:${ENCRE_SOMBRE}; }
.theme-nuit .repere { color:${ENCRE_CLAIRE}; }
.repere.p16 { width:16px; height:16px; }
.repere.p20 { width:20px; height:20px; }
${TEINTES.map((n) =>
  `.theme-clair .repere[data-teinte="${n}"] { background:${REP_SOMBRES[n]}; }`
  + `.theme-nuit .repere[data-teinte="${n}"] { background:${REP_CLAIRES[n]}; }`).join('\n')}

/* Le glyphe nu (O2) : la teinte devient l'encre du tracé. */
.glyphe-compte { display:inline-flex; }
${TEINTES.map((n) =>
  `.theme-clair .glyphe-compte[data-teinte="${n}"] { color:${REP_SOMBRES[n]}; }`
  + `.theme-nuit .glyphe-compte[data-teinte="${n}"] { color:${REP_CLAIRES[n]}; }`).join('\n')}
`;

// --- « Expéditeur sur <glyphe> Libellé » — la forme retenue ----------
// Décisions du Chef Ingénieur (2026-08-24, verdict sur la mise en
// situation) :
//   1. la phrase se LIT — elle évite d'avoir à se souvenir en
//      permanence d'une couleur ou d'un logo. Forme retenue.
//   2. la nav et la ligne peuvent dire la même chose, mais le GLYPHE
//      doit être exactement le même : il vient d'une seule source, le
//      repère du compte (`reperes[account_id].icone`) — Nav.svelte et
//      Liste.svelte n'ont jamais deux tables.
//   3. le glyphe reste : il donne la chaleur, et couvrir couleur ET
//      forme couvre la majorité des goûts pour une implémentation
//      simple.
//   4. la mécanique de repli (V7) est ÉCARTÉE. À la place, le libellé
//      de boîte se TRONQUE à l'ellipse quand il s'approche de l'heure —
//      ce qui règle du même geste le problème des noms longs.
//      (D4 borne la SAISIE : 60 caractères refusés, jamais tronqués à
//      l'entrée. Elle ne dit rien de l'affichage — tronquer au rendu ne
//      la heurte pas.)
//   5. le même schéma se réplique derrière le nom de l'expéditeur au
//      volet de lecture.
export const blocBoite = (cpt) =>
  `<span class="boite" title="${cpt.nom} — ${cpt.adresse}">`
  + `<span class="mot">sur</span>`
  + `<span class="glyphe-compte" data-teinte="${cpt.teinte}">${ico(cpt.icone, 14)}</span>`
  + `<span class="lib">${cpt.nom}</span></span>`;

export const blocSur = (l) => blocBoite(COMPTES[l.c]);

export const rangSur = (l, cpt = COMPTES[l.c]) =>
  `<div class="${classes(l)} sans-tete"><div class="l1">${disque(l)}`
  + `<span class="exp">${l.exp}</span>${blocBoite(cpt)}`
  + `<span class="essor"></span><span class="heure">${l.heure}</span></div>`
  + `${corps(l)}${rangPuces(l)}</div>`;

// L'ordre de troncature EST le dessin (décision 4). Trois règles, et
// elles se disent en une phrase chacune :
//   — l'heure ne se coupe JAMAIS (flex:none) : c'est le repère de
//     lecture de la colonne ;
//   — le bloc boîte cède TROIS fois plus vite que l'expéditeur et ne
//     prend jamais plus du TIERS de la ligne — c'est lui qu'un nom long
//     ferait déborder, c'est donc lui qui rend du terrain. Le tiers est
//     MESURÉ, pas choisi : six plafonds essayés sur un nom de 32
//     caractères, à 300 / 400 / 640 px. À la moitié, deux noms
//     d'expéditeur se coupent au défaut ; à 30 %, ce sont les libellés
//     COURTS qui se coupent pour rien à la borne basse (7 sur 16). Le
//     tiers est le plus serré qui ne coupe jamais un nom court, et le
//     plus large qui n'entame jamais l'expéditeur au défaut ;
//   — les deux se terminent à l'ellipse, jamais à la coupe sèche.
// La préposition et le glyphe ne rétrécissent pas : un « sur » tronqué
// ne voudrait rien dire.
export const CSS_SUR = `
.sans-tete { grid-template-columns:1fr; }
.sans-tete > .l1, .sans-tete > .objet, .sans-tete > .apercu, .sans-tete > .puces { grid-column:1; }
.l1 { gap:6px; }
.l1 .exp { flex:0 1 auto; min-width:0; }
.l1 .essor { flex:1 1 0; min-width:0; }
.l1 .heure { flex:none; }
.boite { flex:0 3 auto; min-width:0; max-width:33%;
  display:inline-flex; align-items:center; gap:5px;
  font-size:13px; color:var(--ink2); white-space:nowrap; }
.boite .mot, .boite .sep, .boite .glyphe-compte { flex:none; }
.boite .mot, .boite .sep { color:var(--muted); }
.boite .lib { min-width:0; overflow:hidden; text-overflow:ellipsis;
  white-space:nowrap; color:var(--ink2); }
/* Une rangée non lue met la graisse sur ce qu'elle dit, pas sur ses
   circonstances : le bloc boîte reste en graisse normale (A8/V6). */
.nonlu .boite { font-weight:400; }
`;

// ====================================================================
// Sept versions de « Expéditeur sur <icône> Libellé » — page JETABLE.
//
//   node spikes/volet-repere/ligne-expediteur.mjs
//
// Énoncé du Chef Ingénieur (2026-08-24, troisième passe) : la boîte se
// dit SUR LA LIGNE DE L'EXPÉDITEUR, en toutes lettres, de la forme
// « Expéditeur sur <icône> Libellé Boîte ».
//
// C'est la seule famille qui rende la boîte LISIBLE (un mot, pas un
// code de couleur) sans quitter la rangée. Son point dur est unique et
// il est mesurable : la ligne d'entête portait déjà le nom et l'heure,
// à 400 px de volet. On y ajoute deux à quatre-vingts pixels.
//
// D'où la règle de cette planche : chaque version est rendue À 400 PX
// (le défaut, lib/largeurs.svelte.js) ET À 300 PX — la BORNE BASSE du
// volet (BORNES.liste = [300, 640]). Une version qui ne tient qu'au
// défaut n'est pas livrable : l'utilisateur peut tirer la poignée.
//
// Les règles en jeu :
//   A8   le mot dit la boîte — la couleur n'est jamais seule. Toutes
//        les versions la tiennent, y compris sans glyphe.
//   A33  marges symétriques ; « 12 px pour les puces ».
//   A44  deux gabarits de hauteur, et deux seulement.
//   D4   le nom de compte est REFUSÉ à 60 caractères, jamais tronqué
//        (PLAN-RETOURS-9) — ce qu'on coupe ici, c'est l'expéditeur.
//   V6   rien au-dessous de 24 px ne change de graisse.
// ====================================================================
import { writeFileSync } from 'node:fs';
import path from 'node:path';
import {
  RACINE, CLAIR, ico, COMPTES, FIL, corps, rangPuces, classes,
  CSS_VOLET, CSS_SUR, rangSur,
} from './socle.mjs';

const ici = import.meta.dirname;

// Neuf rangées : les premières du décor, plus les DEUX noms les plus
// longs — c'est eux qui disent si la ligne tient.
const RANGS = [...FIL.slice(0, 6), FIL[8], FIL[13]];

const disque = (l) => (l.nonlu ? '<span class="disque"></span>' : '');
const glypheT = (l, t = 14) =>
  `<span class="glyphe-compte" data-teinte="${COMPTES[l.c].teinte}">${ico(COMPTES[l.c].icone, t)}</span>`;
const nom = (l) => COMPTES[l.c].nom;
const titre = (l) => `${COMPTES[l.c].nom} — ${COMPTES[l.c].adresse}`;

// La ligne d'entête, montée version par version. `exp` porte toujours
// min-width:0 + ellipsis : ce qui se coupe est l'expéditeur, jamais le
// nom de la boîte (D4).
const entete = (l, boite, { fer = false } = {}) =>
  `<div class="l1">${disque(l)}<span class="exp">${l.exp}</span>`
  + `<span class="boite${fer ? ' fer' : ''}" title="${titre(l)}">${boite}</span>`
  + `<span class="essor"></span><span class="heure">${l.heure}</span></div>`;

const rang = (l, boite, opts) =>
  `<div class="${classes(l)} sans-tete">${entete(l, boite, opts)}${corps(l)}${rangPuces(l)}</div>`;

// Le témoin : la ligne d'entête d'aujourd'hui, sans bloc boîte — la
// référence contre laquelle le coût des sept se lit.
const rangTemoin = (l) =>
  `<div class="${classes(l)} sans-tete"><div class="l1">${disque(l)}`
  + `<span class="exp">${l.exp}</span><span class="essor"></span>`
  + `<span class="heure">${l.heure}</span></div>${corps(l)}${rangPuces(l)}</div>`;

// ====================================================================
// Les sept versions
// ====================================================================
const VERSIONS = [
  {
    id: 'v0', num: 'T', nom: 'Le témoin — la ligne d’aujourd’hui',
    sous: '« Camille Roux »',
    meca: 'Ce qui est livré : le nom et l’heure, rien d’autre',
    rang: rangTemoin,
    dit: `La ligne d’entête telle qu’elle est aujourd’hui : le disque de non-lu, le nom de
      l’expéditeur, l’heure. Elle est ici pour une seule raison — le coût des sept versions ne
      veut rien dire s’il ne se lit pas contre une référence mesurée.`,
    cout: `Elle ne dit pas la boîte. C’est tout le sujet.`,
    regles: `L’existant.`,
  },
  {
    id: 'v1', num: 'V1', nom: 'La phrase',
    sous: '« Camille Roux sur <span class="ex-g">▣</span> Travail »',
    meca: 'La forme demandée, telle quelle — le bloc boîte suit le nom',
    rang: (l) => rangSur(l),
    dit: `La rangée se lit comme une phrase : « Camille Roux sur Travail ». Le bloc boîte est
      collé au nom — il se déplace avec lui —, la préposition à l’encre atténuée, le glyphe à la
      teinte du compte, le libellé à l’encre secondaire. C’est la version la plus explicite des
      sept : rien à deviner, rien à apprendre, rien qui demande la vision des couleurs.`,
    cout: `C’est la plus chère en largeur, avec V6 : le bloc mesure <b>83 px</b> et ramène la
      place du nom de 304 à <b>219 px</b> — au défaut, <b>aucun des huit noms ne se coupe</b>,
      y compris « Bibliothèque universitaire ». À 300 px, deux se coupent. C’est là que la
      version se juge : regardez la troisième colonne.`,
    regles: `A8 tenu par le mot lui-même. D4 servi : ce qui se coupe est l’expéditeur, jamais le
      nom de compte. Deux gabarits intacts.`,
  },
  {
    id: 'v2', num: 'V2', nom: 'Le point médian',
    sous: '« Camille Roux · <span class="ex-g">▣</span> Travail »',
    meca: 'La préposition tombe, un point médian tient la jointure',
    rang: (l) => rang(l, `<span class="sep">·</span>${glypheT(l)}<span class="lib">${nom(l)}</span>`),
    dit: `Le mot « sur » est la seule chose de la phrase qui ne porte aucune information : il
      coûte trois lettres et une espace de chaque côté. Le point médian les remplace — c’est la
      jointure déjà employée par le fil (« adresse · à destinataire », A45), donc rien de neuf
      dans la grammaire.`,
    cout: `On perd la lecture en phrase : « Camille Roux · Travail » se lit comme une
      juxtaposition, et un lecteur pressé peut prendre « Travail » pour une seconde adresse. Le
      glyphe rattrape beaucoup — mais c’est lui, alors, qui porte le sens.`,
    regles: `Aucune règle touchée. Économie mesurée ci-contre, sur le nom d’expéditeur.`,
  },
  {
    id: 'v3', num: 'V3', nom: 'Le glyphe seul',
    sous: '« Camille Roux sur <span class="ex-g">▣</span> »',
    meca: 'La préposition reste, le libellé tombe — le glyphe EST le libellé',
    rang: (l) => rang(l, `<span class="mot">sur</span>${glypheT(l, 16)}`),
    dit: `« Camille Roux sur ▣ » : la phrase annonce qu’une boîte suit, le glyphe la nomme.
      C’est la version la moins chère en largeur — <b>38 px</b> de bloc, contre 83 pour V1 — et
      celle qui laisse le plus de place au nom : 260 px au défaut, 160 à la borne basse, où elle
      est la seule des six formes à ne couper qu’un nom sur huit.`,
    cout: `<b>Elle sort de l’énoncé</b> : le libellé demandé n’est plus écrit. Le glyphe seul
      redevient un code à apprendre, et si deux comptes partagent une famille d’icône (rien ne
      l’interdit — le jeu compte douze glyphes pour un nombre de comptes non borné), la ligne
      ment. L’infobulle et l’aria disent le nom, mais A8 ne se satisfait pas d’une infobulle.`,
    regles: `A8 : à la limite — le glyphe est une forme, donc l’information n’est pas portée par
      la seule couleur ; mais elle n’est plus portée par un MOT, ce qui était le sujet.`,
  },
  {
    id: 'v4', num: 'V4', nom: 'Le sourcil',
    sous: '« Camille Roux <span class="ex-c">SUR TRAVAIL</span> »',
    meca: 'Petites capitales de 11 px, sans glyphe, sans couleur',
    rang: (l) => rang(l, `<span class="cap">sur ${nom(l)}</span>`),
    dit: `La boîte passe dans la grammaire du sourcil — 11 px, interlettrage .1em, capitales,
      encre atténuée : exactement le « BOÎTES » de la nav. La typographie dit « ceci est une
      étiquette, pas un second nom », et la confusion de V2 disparaît sans coûter un mot de
      plus. Zéro couleur, zéro glyphe : la version la plus sobre des sept.`,
    cout: `Les capitales prennent plus de place que les bas-de-casse à taille égale : mesuré,
      « SUR TRAVAIL » à 11 px coûte <b>78 px</b> contre 83 pour « sur ▣ Travail » à 13 — cinq
      pixels d’écart, pour un glyphe et une couleur en moins. Et le compte perd
      sa couleur : deux comptes se distinguent au mot, jamais au coup d’œil — ce qui est un
      choix défendable, pas un accident.`,
    regles: `A8 tenu par construction (rien n’est dit par la couleur). La grammaire du sourcil
      existe déjà — aucune règle neuve, aucune paire à mesurer.`,
  },
  {
    id: 'v5', num: 'V5', nom: 'La puce',
    sous: '« Camille Roux <span class="ex-p">▣ Travail</span> »',
    meca: 'Le bloc boîte devient un objet — la grammaire des puces, montée à 20 px',
    rang: (l) => rang(l, `<span class="puce-entete">${glypheT(l, 12)}${nom(l)}</span>`),
    dit: `La boîte cesse d’être du texte et devient un OBJET : sol <code>--surface</code>, filet
      de 1 px, glyphe à la teinte, libellé de 11 px. Elle se détache de la phrase, donc elle ne
      peut pas se lire comme un second nom d’expéditeur — et elle annonce, visuellement, qu’elle
      est cliquable le jour où l’on voudra qu’un clic filtre sur ce compte.`,
    cout: `<b>Une taille de puce qui n’existe pas.</b> Le Système en connaît une : 24 px, marges
      de 12 (A33). Une puce de 20 px à marges de 8 est une décision de Système, pas un réglage —
      et une puce dans la ligne d’entête, à côté du rang de puces du bas, met deux puces de deux
      grammaires dans la même rangée. Elle est aussi la seule des sept à changer la
      <b>hauteur</b> de la rangée : 89,7 px contre 88,4 (mesuré) — les sondes suivraient, mais
      c’est un gabarit de plus à re-mesurer.`,
    regles: `A33 à trancher (une seconde taille de puce). A44 : la ligne d’entête grandit — les
      hauteurs bougent, les sondes suivent, mais c’est mesuré ci-dessous.`,
  },
  {
    id: 'v6', num: 'V6', nom: 'Le fer à droite',
    sous: 'La phrase de V1, poussée contre l’heure',
    meca: 'Le bloc boîte quitte le nom et s’aligne — une colonne se forme',
    rang: (l) => rang(l, `<span class="mot">sur</span>${glypheT(l)}<span class="lib">${nom(l)}</span>`, { fer: true }),
    dit: `Même matière que V1, mais le bloc boîte est poussé contre l’heure au lieu de suivre le
      nom. Conséquence : d’une rangée à l’autre, les boîtes s’alignent — il se forme une
      <b>colonne</b> qu’on lit verticalement, ce qu’aucune version « en phrase » ne permet. On
      balaie les origines sans lire les noms.`,
    cout: `On perd la phrase : « Camille Roux » et « sur Travail » ne se touchent plus, et un
      grand vide s’ouvre entre les deux quand le nom est court. Le mot « sur », séparé de son
      sujet, devient bancal — c’est la version qui gagnerait le plus à passer en V2 ou V4.`,
    regles: `Aucune règle touchée. C’est un arbitrage de lecture : la phrase, ou la colonne.`,
  },
  {
    id: 'v7', num: 'V7', nom: 'La boîte incoupable',
    sous: 'V1, plus un repli mesuré sous la largeur seuil',
    meca: 'La troncature s’ordonne, et le libellé se retire quand il ne tient plus',
    rang: (l) => rang(l, `<span class="mot">sur</span>${glypheT(l)}`
      + `<span class="lib repliable">${nom(l)}</span>`),
    dit: `<b>Écartée au verdict du 2026-08-24</b>, au profit de la troncature du libellé à
      l’ellipse (décision 4) : un libellé qui disparaît d’un coup au franchissement d’un seuil
      surprend, là où une ellipse dit ce qu’elle fait. Ce qui suit reste la trace de ce qui a été
      comparé. — La forme de V1, plus un ordre de troncature
      explicite. L’expéditeur s’ellipse le premier (il est le plus long et le plus redondant —
      l’objet en dit souvent autant) ; le libellé de boîte ne se coupe JAMAIS (D4) ; et
      <b>au-dessous de la largeur seuil, le libellé se retire de lui-même</b> et laisse le
      glyphe — la rangée retombe sur V3 au lieu de se casser.`,
    cout: `Un seuil est un chiffre à défendre : ici <b>360 px de volet</b>, en requête de
      conteneur (c’est la largeur du VOLET qui décide, pas celle de la fenêtre — la poignée
      existe). C’est une mécanique de plus dans une rangée qui n’en avait pas, et elle se teste
      des deux côtés du seuil. En échange, mesuré : à 300 px le bloc retombe à 36 px, la place
      du nom remonte à 162 px et <b>aucun nom ne se coupe</b> — la seule des sept dans ce cas.`,
    regles: `D4 servi à la lettre : le nom de compte n’est jamais tronqué, il est retiré ou
      entier. A44 intact — le repli ne change pas la hauteur, seulement la largeur occupée.`,
  },
];

// --- Les pièces du volet ---------------------------------------------
const BANDEAU = `<header class="bandeau"><h1>Boîte de réception</h1></header>`;
const PIED = `<div class="onglets">
  <span class="onglet actif">${ico('inbox')}Tous</span>
  <span class="onglet">${ico('mark_email_unread')}Non lus</span>
</div>`;

const colonne = (v, largeur, theme) =>
  `<div class="${theme} colonne ${v.id}" style="width:${largeur}px">${BANDEAU}`
  + `<div class="cadre-liste"><div class="liste">${RANGS.map(v.rang).join('')}</div></div>`
  + `${PIED}</div>`;

// --- La feuille --------------------------------------------------------
const FEUILLE = `
:root { color-scheme:light; }
* { box-sizing:border-box; }
body { margin:0; padding:0 0 80px;
  font-family:-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  background:${CLAIR.bg}; color:${CLAIR.ink}; }
.page { max-width:1340px; margin:0 auto; padding:0 36px; }
header.tete-page { padding:52px 0 22px; border-bottom:1px solid ${CLAIR.border}; }
h1.titre-page { font-family:'Segoe UI Variable Display', -apple-system, BlinkMacSystemFont,
  "Segoe UI", sans-serif; font-weight:340; letter-spacing:-.03em; font-size:40px; margin:0 0 12px; }
.sourcil { font-size:11px; letter-spacing:.1em; text-transform:uppercase;
  color:${CLAIR.muted}; font-weight:600; margin:0 0 6px; }
.sous-titre { margin:0 0 10px; font-size:15px; line-height:1.6; color:${CLAIR.ink2}; max-width:76ch; }
p { font-size:14px; line-height:1.6; color:${CLAIR.ink2}; margin:0 0 10px; max-width:78ch; }
b { color:${CLAIR.ink}; font-weight:600; }
code { font-family:Consolas, monospace; font-size:12.5px; background:${CLAIR.surface};
  border:1px solid ${CLAIR.border}; padding:0 4px; }
a { color:${CLAIR.accent}; }
section.ver { padding:30px 0; border-bottom:1px solid ${CLAIR.border}; }
.tete-ver { display:flex; align-items:flex-start; gap:14px; margin-bottom:18px; }
.num { display:inline-flex; align-items:center; justify-content:center; width:36px; height:32px;
  border:1px solid ${CLAIR.border}; background:${CLAIR.surface};
  font-size:13px; font-weight:600; color:${CLAIR.ink}; flex:none; }
.tete-ver h2 { margin:0 0 3px; font-size:19px; font-weight:600; color:${CLAIR.ink}; }
.forme { margin:0 0 3px; font-size:14px; color:${CLAIR.ink}; }
.ex-g { color:${CLAIR.accent}; }
.ex-c { font-size:11px; letter-spacing:.1em; color:${CLAIR.muted}; font-weight:600; }
.ex-p { font-size:11px; border:1px solid ${CLAIR.border}; background:${CLAIR.surface}; padding:1px 6px; }
.meca { margin:0; font-size:13px; color:${CLAIR.muted}; }
.planches { display:flex; gap:20px; align-items:flex-start; flex-wrap:wrap; }
figure { margin:0; flex:none; }
figcaption { margin-top:8px; font-size:11px; letter-spacing:.08em; text-transform:uppercase;
  color:${CLAIR.muted}; font-weight:600; }
figcaption.borne { color:${CLAIR.alert}; }
.prose { flex:1; min-width:300px; }
.prose h3 { font-size:12px; letter-spacing:.06em; text-transform:uppercase;
  color:${CLAIR.muted}; margin:0 0 6px; font-weight:600; }
.prose h3 + p { margin-bottom:14px; }
table.banc { border-collapse:collapse; font-size:12.5px; margin:16px 0 8px; }
table.banc th, table.banc td { border:1px solid ${CLAIR.border}; padding:5px 10px; text-align:right; }
table.banc th[scope="row"] { text-align:left; font-weight:600; color:${CLAIR.ink}; }
table.banc thead th { font-size:11px; color:${CLAIR.muted}; text-transform:uppercase;
  letter-spacing:.06em; font-weight:600; }
table.banc td.coupe { color:${CLAIR.alert}; font-weight:600; }
ul { font-size:14px; line-height:1.6; color:${CLAIR.ink2}; max-width:78ch; padding-left:20px; }
li { margin-bottom:8px; }

/* ================= LE VOLET, À TAILLE RÉELLE ================= */
${CSS_VOLET}
.colonne { height:470px; display:flex; flex-direction:column;
  background:var(--bg); border:1px solid var(--border); overflow:hidden; }
.bandeau { flex:none; height:52px; display:flex; align-items:center;
  padding:0 16px; background:var(--bg); border-bottom:1px solid var(--border); }
.bandeau h1 { margin:0; font-size:16px; font-weight:600; color:var(--ink);
  overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
.cadre-liste { flex:1; min-height:0; overflow:hidden; }
.onglets { flex:none; height:52px; padding:0 12px; display:flex; align-items:center; gap:10px;
  border-top:1px solid var(--border); background:var(--bg); }
.onglet { height:32px; padding:0 14px; display:inline-flex; align-items:center; gap:8px;
  font-size:13px; color:var(--ink2); background:var(--surface); border:1px solid var(--border);
  white-space:nowrap; }
.onglet.actif { font-weight:600; color:var(--ink); background:var(--sel); border-color:var(--accent); }
/* La ligne d'entête et l'ordre de troncature viennent du socle — c'est
   la forme retenue, elle n'a qu'une implémentation. */
${CSS_SUR}
.boite.fer { margin-left:auto; }
/* V7 — le repli au seuil. ÉCARTÉ par le Chef Ingénieur le 2026-08-24 au
   profit de la troncature du libellé (décision 4) ; la règle reste ici,
   locale à cette planche, comme trace de ce qui a été comparé. */
.v7 .cadre-liste { container-type:inline-size; }
@container (max-width: 360px) { .v7 .repliable { display:none; } }
/* V6 : le mot est déjà séparé de son sujet — l'essor ne doit pas
   s'insérer entre le bloc et l'heure. */
.v6 .essor { display:none; }
/* Une rangée non lue met la graisse sur ce qu'elle dit, pas sur ses
   circonstances : le bloc boîte reste en graisse normale (V6/A8). */
.nonlu .boite { font-weight:400; }

/* V4 — le sourcil */
.cap { font-size:11px; letter-spacing:.1em; text-transform:uppercase;
  color:var(--muted); font-weight:600; }

/* V5 — la puce d'entête (20 px, marges de 8 : une taille de plus) */
.puce-entete { display:inline-flex; align-items:center; gap:5px; height:20px; padding:0 8px;
  font-size:11px; color:var(--ink2); background:var(--surface); border:1px solid var(--border); }

/* V7 — le repli : sous 360 px de volet, le libellé se retire et laisse
   le glyphe. Requête de CONTENEUR : c'est la largeur du volet qui
   décide, pas celle de la fenêtre. */
.v7 .cadre-liste { container-type:inline-size; }
@container (max-width: 360px) { .v7 .repliable { display:none; } }
`;

// --- La page ----------------------------------------------------------
const blocs = VERSIONS.map((v) => `
<section class="ver" id="${v.id}">
  <div class="tete-ver">
    <span class="num">${v.num}</span>
    <div><h2>${v.nom}</h2><p class="forme">${v.sous}</p><p class="meca">${v.meca}</p></div>
  </div>
  <div class="planches">
    <figure>${colonne(v, 400, 'theme-clair')}<figcaption>400 px — le défaut</figcaption></figure>
    <figure>${colonne(v, 400, 'theme-nuit')}<figcaption>400 px — nuit</figcaption></figure>
    <figure>${colonne(v, 300, 'theme-clair')}<figcaption class="borne">300 px — la borne basse</figcaption></figure>
    <div class="prose">
      <h3>Ce que ça dit</h3><p>${v.dit}</p>
      <h3>Ce que ça coûte</h3><p>${v.cout}</p>
      <h3>Règles touchées</h3><p>${v.regles}</p>
    </div>
  </div>
</section>`).join('');

const html = `<!doctype html>
<html lang="fr"><head><meta charset="utf-8">
<title>Sept fois « Expéditeur sur ▣ Boîte »</title>
<style>${FEUILLE}</style></head>
<body><div class="page">
<header class="tete-page">
  <p class="sourcil">Spike jetable · 2026-08-24 · rien ici n’est livré</p>
  <h1 class="titre-page">Sept fois « Expéditeur sur ▣ Boîte »</h1>
  <p class="sous-titre">La boîte se dit <b>sur la ligne de l’expéditeur, en toutes lettres</b>.
  C’est la seule famille qui rende l’origine <b>lisible</b> — un mot, pas un code de couleur —
  sans quitter la rangée, et elle tient A8 sans rien demander au nuancier.</p>
  <p><b>Le point dur est unique, et il se mesure.</b> La ligne d’entête porte déjà le nom et
  l’heure ; on lui ajoute un bloc. Le banc du bas dit, pour chaque version, ce que le bloc prend
  et ce qu’il reste au nom — <b>contre un témoin</b>, la ligne d’aujourd’hui. Chaque version est
  rendue à 400 px (le défaut de <code>lib/largeurs.svelte.js</code>) <b>et à 300 px</b> — la
  borne basse du volet (<code>BORNES.liste = [300, 640]</code>). Une version qui ne tient qu’au
  défaut n’est pas livrable : la poignée existe, et elle se tire.</p>
  <p>Un choix commun aux sept, et il est décidé ici : <b>ce qui se coupe est l’expéditeur, jamais
  le nom de boîte</b> — D4 (PLAN-RETOURS-9) refuse un nom de compte à 60 caractères plutôt que
  de le tronquer ; le tronquer à l’affichage dirait le contraire. Et sur une rangée non lue, la
  graisse reste sur le nom et l’objet : les circonstances du message ne crient pas.</p>
  <p><a href="planche.html">← les sept dessins</a> ·
  <a href="organisation.html">les quinze organisations</a> ·
  <a href="o2.html">O2 en situation</a></p>
</header>
${blocs}
<section class="ver" id="banc">
  <h2 style="font-family:'Segoe UI Variable Display',sans-serif;font-weight:340;
    letter-spacing:-.03em;font-size:24px;margin:0 0 6px">Le banc — ce que coûte chaque version</h2>
  <p>Mesuré au rendu de cette page : largeur du bloc boîte, place restante pour le nom
  d’expéditeur, et nombre de noms coupés sur les huit rangées. Les chiffres sont écrits par
  <code>banc.js</code> à l’ouverture — ils ne peuvent pas se désynchroniser du dessin.</p>
  <div id="banc-cible"></div>
  <h3 style="font-size:12px;letter-spacing:.06em;text-transform:uppercase;color:${CLAIR.muted};
    margin:22px 0 6px;font-weight:600">Ce que le banc dit</h3>
  <ul>
    <li><b>La famille tient au défaut, et ce n’était pas acquis.</b> À 400 px, <b>aucune</b> des
    sept ne coupe un nom d’expéditeur sur les huit rangées — « Bibliothèque universitaire » et
    « Secrétariat pédagogique » compris. Le témoin laisse 304 px au nom ; la plus chère lui en
    laisse encore 219.</li>
    <li><b>Le prix se paie à la borne basse.</b> À 300 px, toutes coupent deux noms sur huit,
    sauf V3 (un) et <b>V7 (aucun)</b>. Le volet se règle à la poignée : ce n’est pas un cas
    théorique.</li>
    <li><b>V7 n’est pas une huitième forme, c’est une mécanique</b> — l’ordre de troncature et
    le repli au seuil s’appliquent à V1, V2, V4, V5 ou V6 sans les changer. La question
    « quelle forme » et la question « comment elle rétrécit » sont deux décisions séparées.</li>
    <li><b>V5 est la seule à toucher la hauteur</b> (89,7 contre 88,4 px) : un gabarit à
    re-sonder, pour une puce dont la taille n’existe pas encore au Système.</li>
  </ul>
  <h3 style="font-size:12px;letter-spacing:.06em;text-transform:uppercase;color:${CLAIR.alert};
    margin:22px 0 6px;font-weight:600">Le fait que ce décor cache — et qui décide</h3>
  <p>Les trois comptes s’appellent ici <b>Travail</b>, <b>Maison</b>, <b>Études</b> : six à sept
  caractères. Or D4 (PLAN-RETOURS-9) accepte un nom de compte jusqu’à <b>60 caractères</b> —
  « Association des parents d’élèves » est un nom légitime, et il ferait à lui seul plus que la
  ligne entière. <b>Toute cette famille suppose des noms courts</b>, et cette supposition n’est
  écrite nulle part.</p>
  <p>Trois issues, et c’est une décision produit, pas un dessin : (1) un <b>nom court</b> distinct
  du nom d’affichage, borné à ~12 caractères et demandé à l’ajout du compte ; (2) le repli de V7
  déclenché non par la largeur mais par la <b>longueur du libellé</b> — au-delà de N caractères,
  le glyphe seul ; (3) tronquer le libellé, ce que D4 refuse explicitement pour le nom de compte
  et que je ne recommande pas.</p>
  <h3 style="font-size:12px;letter-spacing:.06em;text-transform:uppercase;color:${CLAIR.muted};
    margin:22px 0 6px;font-weight:600">Ce que je recommande</h3>
  <p><b>V1 + la mécanique de V7</b> si vous voulez la phrase telle que vous l’avez écrite : c’est
  la forme la plus explicite, et le repli la rend livrable jusqu’à la borne basse.
  <b>V4 + V7</b> si vous préférez qu’aucune information ne passe par la couleur — cinq pixels de
  moins que V1, et le sourcil dit « étiquette » là où V2 laisse croire à un second nom.
  <b>V6</b> reste la seule qui donne une colonne à balayer plutôt qu’une phrase à lire : c’est un
  arbitrage de lecture, et il se tranche à l’œil, sur les colonnes ci-dessus.</p>
  <p>Et quelle que soit la forme retenue, la question des <b>noms courts</b> se règle avant, pas
  après — sinon le premier compte au nom long cassera la ligne au terrain.</p>
</section>
</div>
<script>
(() => {
  const versions = ${JSON.stringify(VERSIONS.map((v) => ({ id: v.id, num: v.num, nom: v.nom })))};
  const GAP = 6;
  const mesure = (id, largeur) => {
    const col = [...document.querySelectorAll('.' + id + '.theme-clair.colonne')]
      .find((c) => Math.round(c.getBoundingClientRect().width) === largeur);
    if (!col) return null;
    const l1s = [...col.querySelectorAll('.l1')];
    const boites = l1s.map((l) => l.querySelector('.boite')).filter(Boolean);
    // La place OFFERTE au nom : la largeur de la ligne d'entete moins
    // tout ce qui ne se coupe jamais (disque, bloc boite, heure) et les
    // gouttieres. Mesurer la largeur RENDUE du nom ne dirait que sa
    // longueur quand il tient — un chiffre sans objet.
    const dispo = l1s.map((l) => {
      const enfants = [...l.children].filter((e) => !e.classList.contains('exp')
        && !e.classList.contains('essor'));
      const pris = enfants.reduce((n, e) => n + e.getBoundingClientRect().width, 0);
      const gouttieres = GAP * ([...l.children].length - 1);
      return l.clientWidth - pris - gouttieres;
    });
    const exps = l1s.map((l) => l.querySelector('.exp'));
    const coupes = exps.filter((e) => e.scrollWidth > e.clientWidth + 1).length;
    const nues = [...col.querySelectorAll('.ligne')].slice(1)
      .filter((r) => !r.querySelector('.puces'));
    return {
      boite: boites.length ? Math.round(Math.max(...boites.map((b) => b.getBoundingClientRect().width))) : null,
      dispo: Math.round(Math.min(...dispo)),
      coupes,
      total: exps.length,
      haut: Math.round(nues[0].getBoundingClientRect().height * 10) / 10,
    };
  };
  const px = (m, cle) => (!m || m[cle] === null ? '<td>—</td>' : '<td>' + m[cle] + ' px</td>');
  const cut = (m) => (!m ? '<td>—</td>'
    : '<td class="' + (m.coupes > 0 ? 'coupe' : '') + '">' + m.coupes + ' / ' + m.total + '</td>');
  const lignes = versions.map((v) => ({ v, a: mesure(v.id, 400), b: mesure(v.id, 300) }));
  document.getElementById('banc-cible').innerHTML =
    '<table class="banc"><thead>'
    + '<tr><th></th><th colspan="3">400 px — le défaut</th>'
    + '<th colspan="2">300 px — la borne basse</th><th>rangée</th></tr>'
    + '<tr><th></th><th>bloc boîte</th><th>place au nom</th><th>noms coupés</th>'
    + '<th>place au nom</th><th>noms coupés</th><th>hauteur nue</th></tr></thead><tbody>'
    + lignes.map((r) => '<tr><th scope="row">' + r.v.num + ' · ' + r.v.nom + '</th>'
      + px(r.a, 'boite') + px(r.a, 'dispo') + cut(r.a)
      + px(r.b, 'dispo') + cut(r.b)
      + (r.a ? '<td>' + r.a.haut + ' px</td>' : '<td>—</td>')
      + '</tr>').join('')
    + '</tbody></table>';
})();
</script>
</body></html>`;

const sortie = path.join(ici, 'ligne-expediteur.html');
writeFileSync(sortie, html, 'utf8');
console.log(`Écrit : ${path.relative(RACINE, sortie)}`);
console.log(`  ${VERSIONS.length} versions × 2 largeurs (400 / 300) × 2 polarités`);
console.log(`  ${RANGS.length} rangées, dont les deux noms les plus longs du décor`);

// ====================================================================
// Sept dessins du volet central, comparés — page JETABLE, hors produit.
//
//   node spikes/volet-repere/planche.mjs
//
// Énoncé du Chef Ingénieur (2026-08-24) : retirer la tuile aux
// INITIALES DE L'EXPÉDITEUR de la rangée de liste, et rendre le repère
// de la BOÎTE DE RÉCEPTION « à la fois visible et discret ». La forme
// du repère est libre.
//
// La planche est mise en situation à TAILLE RÉELLE — 400 px, la
// largeur par défaut du volet (lib/largeurs.svelte.js) — et dans les
// DEUX polarités : un repère se juge sur le fond où il se pose, pas
// sur une planche de contact.
//
// Rien n'est recopié : jetons, nuancier, glyphes et initiales viennent
// du produit, par socle.mjs.
//
// Les contraintes en jeu, toutes déjà écrites au Système :
//   V14  zéro rayon. Deux formes rondes seulement, et elles disent
//        quelque chose : le disque (état, identité) et la pilule
//        (le glissement, à la seule piste d'interrupteur).
//   V4   le disque dit l'état. La tuile d'initiales est carrée
//        PRÉCISÉMENT pour lui rendre son unicité ; « reste un seul
//        autre rond dans tout le système : la pastille de repère ».
//   A8   jamais la couleur seule (WCAG 1.4.1). V5 garde le glyphe DANS
//        la pastille pour cette raison, contre la doctrine du jeu.
//   A74  le repère est OPTIONNEL, et il ne s'affiche aujourd'hui que
//        là où les comptes se mélangent (D3 : boîte unifiée, recherche).
//   A33  marges symétriques dans toute puce, tout bouton, tout onglet.
//   A44  DEUX gabarits de hauteur, et deux seulement (h1 nue, h2
//        porteuse) : le fenêtrage de Liste.svelte en dépend.
//   V3   le filet de 1 px porte SEUL la séparation ; Wind reste plat.
// ====================================================================
import { writeFileSync } from 'node:fs';
import path from 'node:path';
import {
  RACINE, CLAIR, NUIT, REP_SOMBRES, REP_CLAIRES, TEINTES,
  rapport, fmt, ico, initiales, COMPTES, LIGNES,
  ligne1, corps, rangPuces, pastille, glypheNu, classes, CSS_VOLET,
} from './socle.mjs';

const ici = import.meta.dirname;

// --- Les huit dessins -------------------------------------------------
// Chacun rend la MÊME matière : seule la place et la forme du repère
// changent, pour que la comparaison porte sur ça et rien d'autre.
const DESSINS = {
  temoin: (l) => {
    const cpt = COMPTES[l.c];
    return `<div class="${classes(l)}">`
      + `<span class="col-avatar"><span class="avatar">${initiales(l.exp)}</span>`
      + `${pastille(cpt)}</span>`
      + ligne1(l) + corps(l) + rangPuces(l) + `</div>`;
  },
  o1: (l) => {
    const cpt = COMPTES[l.c];
    return `<div class="${classes(l)}"><span class="tete">${pastille(cpt)}</span>`
      + ligne1(l) + corps(l) + rangPuces(l) + `</div>`;
  },
  o2: (l) => {
    const cpt = COMPTES[l.c];
    return `<div class="${classes(l)}"><span class="tete">${glypheNu(cpt)}</span>`
      + ligne1(l) + corps(l) + rangPuces(l) + `</div>`;
  },
  // La rive se centre sur la hauteur RÉELLE de la rangée (rangée nue ou
  // porteuse) : le texte est donc groupé dans son propre bloc, sinon
  // `grid-row:1/-1` ne couvre que la première ligne (grille implicite)
  // et la marque flotte 35 px trop haut — mesuré, pas supposé.
  o3: (l) => {
    const cpt = COMPTES[l.c];
    return `<div class="${classes(l)} rive"><div class="bloc-texte">`
      + ligne1(l) + corps(l) + rangPuces(l) + `</div>`
      + `<span class="col-rive">${pastille(cpt)}</span></div>`;
  },
  o4: (l, i, suite) => {
    const cpt = COMPTES[l.c];
    const change = i === 0 || suite[i - 1].c !== l.c;
    return `<div class="${classes(l)}"><span class="tete">`
      + (change ? pastille(cpt) : '') + `</span>`
      + ligne1(l) + corps(l) + rangPuces(l) + `</div>`;
  },
  o5: (l) => {
    const cpt = COMPTES[l.c];
    return `<div class="${classes(l)} sans-tete">`
      + ligne1(l, `<span class="mot-compte" title="${cpt.adresse}">${cpt.court}</span>`)
      + corps(l) + rangPuces(l) + `</div>`;
  },
  o6: (l) => {
    const cpt = COMPTES[l.c];
    return `<div class="${classes(l)} sans-tete liseré" data-teinte="${cpt.teinte}"`
      + ` title="${cpt.nom} — ${cpt.adresse}">`
      + ligne1(l) + corps(l) + rangPuces(l) + `</div>`;
  },
  o7: (l) => {
    const cpt = COMPTES[l.c];
    const chip = `<span class="puce puce-compte" data-teinte="${cpt.teinte}">`
      + `${ico(cpt.icone, 14)}${cpt.court}</span>`;
    return `<div class="${classes(l)} sans-tete">`
      + ligne1(l) + corps(l) + rangPuces(l, chip) + `</div>`;
  },
};

const rendreListe = (cle, theme) =>
  `<div class="cadre ${theme}"><div class="liste ${cle}">`
  + LIGNES.map((l, i) => DESSINS[cle](l, i, LIGNES)).join('')
  + `</div></div>`;

// --- La prose : ce que ça dit, ce que ça coûte, ce que la gate en dit -
// Largeur réellement offerte à l'objet et à l'aperçu, volet au défaut de
// 400 px (lib/largeurs.svelte.js) : 400 − 1 px de filet de colonne
// (`.colonne`, border-right) − 2 px de liseré de rangée − 16 + 16 de
// marges = 365 px, moins la colonne de tête et sa gouttière. Vérifié au
// rendu de cette planche (à 1 px près : le cadre y est bordé des deux
// côtés).
const largeurTexte = (tete, gouttiere) => 365 - tete - gouttiere;
const TEXTE_TEMOIN = largeurTexte(28, 10); // 327

const OPTIONS = [
  {
    id: 'temoin', num: 'T', nom: 'Le témoin — ce qui est livré',
    meca: 'La tuile aux initiales de l’expéditeur, le repère empilé dessous',
    dit: `La colonne de tête porte DEUX objets : une tuile carrée de 28 px aux initiales de
      l’expéditeur (V4 : carrée pour rendre le rond au disque) et, empilée dessous, la pastille
      de repère de 16 px — visible seulement en boîte unifiée et en recherche (A74, D3).`,
    cout: `Les initiales ne disent rien que le nom d’expéditeur, écrit en toutes lettres à 10 px
      au-dessus, ne dise déjà. Elles coûtent 38 px de largeur sur les 400 du volet, et elles
      écrasent le repère : le vrai renseignement — de quelle boîte ce message vient — est
      l’objet le plus petit de la rangée, et il vit SOUS l’objet le plus gros.`,
    gate: `L’existant. <code>refonte-ecran02.spec.js:93</code> tient la tuile,
      <code>refonte-retours-8.spec.js:97</code> tient le badge.`,
    chiffres: [`texte : ${TEXTE_TEMOIN} px`, 'h1 90 / h2 117', 'paires mesurées : l’existant'],
    verdict: null,
  },
  {
    id: 'o1', num: 'O1', nom: 'La pastille prend la place',
    meca: 'Substitution en place — la pastille monte à la place de la tuile',
    dit: `La tuile disparaît, la pastille de 16 px prend sa colonne, centrée géométriquement sur
      la première ligne de texte. Rien d’autre ne bouge. Le repère cesse d’être un satellite :
      il devient l’ancre de la rangée, à la place exacte où l’œil descend déjà la liste.`,
    cout: `C’est le plus petit pas des sept — et le moins de dessin neuf : zéro objet créé, zéro
      jeton, zéro paire de contraste à mesurer (le nuancier d’A74 les a toutes payées).
      Contrepartie : la pastille reste un DISQUE PLEIN de couleur, donc l’objet le plus contrasté
      de la rangée. « Visible » est acquis, « discret » se discute — c’est le point à trancher
      à l’œil, sur cette planche.`,
    gate: `Le test de la tuile meurt, celui du badge s’amende. Le Système : la phrase de V4
      « l’avatar d’initiales devient une tuile carrée » perd son objet — un amendement à écrire.`,
    chiffres: [`texte : ${largeurTexte(16, 10)} px (+12)`, 'h1/h2 inchangés', 'aucune paire neuve'],
    verdict: 'ok',
  },
  {
    id: 'o2', num: 'O2', nom: 'Le glyphe nu',
    meca: 'Dématérialisation — on retire le contenant, on garde le signe',
    dit: `Le disque tombe : le glyphe du compte est tracé à même le fond, 18 px, à l’encre de sa
      teinte. C’est la doctrine du jeu d’icônes appliquée à la lettre — « le jeu ne met jamais
      rien dans un contenant » — et c’est ce qui rend au disque son unicité ABSOLUE : après O2,
      le seul rond du système dit l’état, et rien d’autre. La phrase de V14 se simplifie au lieu
      de s’allonger.`,
    cout: `Le fait qui a fondé V5 se retourne : la pastille portait un glyphe à 4,5:1 sur un
      fond choisi ; le glyphe nu se pose sur <code>--bg</code>, où sa teinte ne tient que le
      seuil COMPOSANT de 3:1. Mesuré ci-contre, famille par famille : c’est légal (un tracé est
      un composant, pas du texte) et c’est le plancher. Un trait de 2 unités rendu à 18 px pèse
      1,5 px — l’objet devient franchement discret, peut-être trop pour être trouvé d’un
      balayage. Second coût, réel : la nav garde ses pastilles pleines de 20 px ; le même compte
      se lit en disque à gauche et en glyphe nu au centre.`,
    gate: `Les 24 mesures « glyphe sur pastille » perdent leur objet ; les 120 mesures
      « teinte sur fond » restent, inchangées. La règle <code>.repere</code> de systeme.css
      garde ses deux emplois (nav, Réglages) : rien à retirer.`,
    chiffres: [`texte : ${largeurTexte(18, 10)} px (+10)`, 'h1/h2 inchangés', 'aucune paire neuve'],
    verdict: 'ok',
  },
  {
    id: 'o3', num: 'O3', nom: 'La rive',
    meca: 'Déplacement au bord de fuite — la colonne de tête disparaît',
    dit: `Plus rien à gauche : les trois rangs de texte prennent la marge, alignés au même fer.
      Le repère passe à droite, dans une rive de 16 px, centré sur la hauteur de la rangée. La
      liste devient une colonne de texte — c’est le dessin le plus typographique des sept — et
      l’identité de la boîte sort du chemin de lecture : on la trouve quand on la cherche.`,
    cout: `Un défaut concret, à nommer avant de l’aimer : la barre de défilement d’A44 est
      NATIVE et posée SUR le contenu (0 px réservé, mesure du 2026-08-16). Une marque au bord
      droit passe sous la poignée pendant le défilement — exactement le moment où l’œil balaie.
      Il faut lui garder sa gouttière (12 px) et le vérifier à la fenêtre réelle, pas ici. Et
      l’heure, déjà ferrée à droite, se retrouve à deux objets du bord.`,
    gate: `Le fenêtrage ne bouge pas (la rive s’aligne sur la hauteur, elle ne la fait pas).
      Le Système : la grille de la rangée change de sens — une planche à refaire.`,
    chiffres: [`texte : ${largeurTexte(16, 12)} px, au fer à gauche`, 'h1/h2 inchangés', 'aucune paire neuve'],
    verdict: 'reserve',
  },
  {
    id: 'o4', num: 'O4', nom: 'Le relais',
    meca: 'Différentiel — la marque ne dit plus « d’où », elle dit « ça change »',
    dit: `Le repère ne se répète pas : il n’est imprimé que sur la rangée où le compte CHANGE
      par rapport à celle du dessus. Une suite de messages d’une même boîte se lit d’un seul
      tenant, sans redite. C’est le dessin le plus silencieux des sept, et le seul où la marque
      porte une information que la rangée seule ne contient pas.`,
    cout: `Deux coûts, et le premier décide. (1) <b>Le fait n’est pas connu</b> : en boîte
      unifiée triée par date, deux comptes actifs alternent presque à chaque rangée — la planche
      ci-contre en montre quatre marques sur six, et c’est un décor favorable. Si vos vraies
      boîtes alternent autant, O4 économise le bruit d’une rangée sur six et rend la lecture
      AMBIGUË sur les cinq autres (une rangée nue ne dit plus rien par elle-même : elle
      s’interprète par le haut). <b>À mesurer sur vos comptes avant d’en discuter le dessin.</b>
      (2) Le fenêtrage sert des tranches : la rangée du dessus n’est pas toujours chargée à la
      couture de deux pages — il faut un repli explicite (inconnu ⇒ on imprime), donc une marque
      qui peut apparaître deux fois de suite au défilement.`,
    gate: `Une logique neuve dans le chemin d’affichage, et un cas de bord qui ne se voit qu’au
      défilement profond — le genre de défaut qui se trouve au terrain, pas au test.`,
    chiffres: [`texte : ${largeurTexte(16, 10)} px (+12)`, 'h1/h2 inchangés', '4 marques sur 6 ici'],
    verdict: 'mesurer',
  },
  {
    id: 'o5', num: 'O5', nom: 'Le mot',
    meca: 'Typographie — l’identité se lit, elle ne se voit pas',
    dit: `Ni glyphe ni couleur : le nom du compte (celui de PLAN-RETOURS-9, D3) écrit en petites
      capitales de 11 px à l’encre atténuée, dans la ligne d’entête, juste avant l’heure. La
      grammaire existe déjà — c’est celle du sourcil « BOÎTES » de la nav, au caractère près.
      C’est la seule des sept qui ne peut pas être mal lue : elle ne demande rien à la vision
      des couleurs, rien à la mémoire des glyphes, rien au survol.`,
    cout: `Elle prend de la place là où il y en a le moins : la ligne d’entête porte déjà le nom
      d’expéditeur (tronqué dès 400 px) et l’heure. « Travail » coûte une cinquantaine de pixels
      au nom. Et elle exige un nom COURT : l’adresse ne tient pas — il faudrait soit rendre le
      nom personnalisé obligatoire, soit en dériver un (la partie avant l’arobase), soit tronquer
      ce qui ne doit jamais l’être (le nom est refusé à 60 caractères, jamais coupé — D4).`,
    gate: `Aucune paire neuve (<code>muted</code> sur bg / sel / hover / tuile est mesuré à
      4,5:1). Aucune icône neuve. C’est l’option la moins chère de la gate — et la plus chère
      de la ligne d’entête.`,
    chiffres: [`texte : ${largeurTexte(0, 0)} px`, 'h1/h2 inchangés', 'zéro couleur, zéro glyphe'],
    verdict: 'ok',
  },
  {
    id: 'o6', num: 'O6', nom: 'Le liseré',
    meca: 'Le bord de la rangée — le moins d’encre possible',
    dit: `Aucun objet neuf, aucun pixel neuf : le liseré de 2 px que CHAQUE rangée réserve déjà à
      gauche (transparent au repos, accent quand la rangée est choisie) prend la teinte du compte
      au repos. La liste ne gagne pas une marque : elle allume une réserve. C’est le maximum de
      discrétion atteignable — et le maximum de largeur pour le texte.`,
    cout: `Deux, et ils sont structurels. (1) <b>A8 tombe</b> : le compte serait dit par la
      COULEUR SEULE, ce qu’A74 a refusé et ce que V5 a re-tranché sur une mesure. Un daltonien
      lit trois barres identiques. (2) <b>Le liseré est déjà pris</b> : c’est le signal de la
      sélection. Regardez la quatrième rangée ci-contre — la rangée choisie perd son compte, ou
      l’accent perd sa rangée. Deux sens sur deux pixels, l’un efface l’autre.`,
    gate: `Aucune paire neuve, mais un amendement qui RENVERSE A74 sur cette surface — décision
      du Chef Ingénieur, pas de l’ingénieur. En l’état, je ne la recommande pas seule : elle vit
      si elle se marie (le liseré + le mot d’O5, ou le liseré + la marque de changement d’O4).`,
    chiffres: [`texte : ${largeurTexte(0, 0)} px`, 'h1/h2 inchangés', 'A8 rompu au repos'],
    verdict: 'non',
  },
  {
    id: 'o7', num: 'O7', nom: 'La puce de compte',
    meca: 'Promotion — le repère entre dans la grammaire des puces',
    dit: `Le repère devient une puce comme les autres : glyphe à la teinte + nom du compte, sol
      <code>--surface</code>, filet de 1 px, marges symétriques (A33), 24 px de haut. Elle dit
      le compte en toutes lettres ET en couleur ET en glyphe — trois canaux, aucune ambiguïté
      possible. Effet de bord d’ingénierie, réel : toute rangée devient PORTEUSE, donc il n’y a
      plus qu’un gabarit de hauteur — <code>chipsParPage</code>, <code>extraPuce</code> et la
      correction itérative du fenêtrage (A44) perdent leur raison d’être.`,
    cout: `La densité, et c’est cher : 90 px de rangée deviennent 117 px, soit <b>−23 % de
      rangées à l’écran</b> — sur 800 px de liste, 8,9 rangées deviennent 6,8. Pour un client
      courrier, c’est le coût le plus lourd des sept, et il se paie à chaque regard. La
      « discrétion » demandée n’y est pas non plus : une puce à trois canaux est ce que la
      rangée a de plus bavard.`,
    gate: `Simplification nette du fenêtrage (un gabarit au lieu de deux) contre une perte de
      densité mesurée. À ne retenir que si la densité n’est PAS le sujet — ce qu’il faudrait
      dire explicitement.`,
    chiffres: [`texte : ${largeurTexte(0, 0)} px`, 'un seul gabarit : 117 px', '−23 % de rangées'],
    verdict: 'reserve',
  },
];

// --- Le banc du glyphe nu (O2) : la teinte, tracée à même le fond -----
const FONDS_BANC = [['bg', 'repos'], ['hover', 'survol'], ['sel', 'choisie'], ['tuile', 'épinglée']];
function bancGlypheNu() {
  const lignes = [];
  for (const teinte of TEINTES) {
    const hexClair = REP_SOMBRES[teinte];
    const hexNuit = REP_CLAIRES[teinte];
    const cellules = [];
    for (const [fond] of FONDS_BANC) {
      cellules.push(rapport(hexClair, CLAIR[fond]));
      cellules.push(rapport(hexNuit, NUIT[fond]));
    }
    const pire = Math.min(...cellules);
    lignes.push(`<tr${pire < 3 ? ' class="sous"' : ''}><th scope="row">${teinte}</th>`
      + cellules.map((r) => `<td>${fmt(r)}</td>`).join('')
      + `<td class="pire">${fmt(pire)}</td></tr>`);
  }
  return lignes.join('');
}
export const PIRE_GLOBAL = Math.min(...TEINTES.flatMap((t) =>
  FONDS_BANC.flatMap(([f]) => [rapport(REP_SOMBRES[t], CLAIR[f]), rapport(REP_CLAIRES[t], NUIT[f])])));

// --- La feuille de style ---------------------------------------------
const FEUILLE = `
:root { color-scheme:light; }
* { box-sizing:border-box; }
body {
  margin:0; padding:0 0 80px;
  font-family:-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  background:${CLAIR.bg}; color:${CLAIR.ink};
}
.page { max-width:1320px; margin:0 auto; padding:0 32px; }
header.tete-page { padding:56px 0 24px; border-bottom:1px solid ${CLAIR.border}; }
h1 {
  font-family:'Segoe UI Variable Display', -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  font-weight:340; letter-spacing:-.03em; font-size:40px; margin:0 0 12px;
}
.sous-titre { margin:0; font-size:15px; line-height:1.6; color:${CLAIR.ink2}; max-width:70ch; }
.sourcil {
  font-size:11px; letter-spacing:.1em; text-transform:uppercase;
  color:${CLAIR.muted}; font-weight:600; margin:0 0 6px;
}
section.bloc { padding:36px 0; border-bottom:1px solid ${CLAIR.border}; }
h2 {
  font-family:'Segoe UI Variable Display', -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  font-weight:340; letter-spacing:-.03em; font-size:24px; margin:0 0 4px;
}
h3 { font-size:12px; letter-spacing:.06em; text-transform:uppercase; color:${CLAIR.muted}; margin:18px 0 6px; }
p { font-size:14px; line-height:1.6; color:${CLAIR.ink2}; margin:0 0 10px; max-width:78ch; }
code { font-family:Consolas, monospace; font-size:12.5px; background:${CLAIR.surface};
  border:1px solid ${CLAIR.border}; padding:0 4px; }
b { color:${CLAIR.ink}; font-weight:600; }
a { color:${CLAIR.accent}; }
.num {
  display:inline-flex; align-items:center; justify-content:center; width:32px; height:32px;
  border:1px solid ${CLAIR.border}; background:${CLAIR.surface};
  font-size:13px; font-weight:600; color:${CLAIR.ink}; flex:none;
}
.tete-opt { display:flex; align-items:flex-start; gap:14px; margin-bottom:20px; }
.meca { margin:0; font-size:13px; color:${CLAIR.muted}; }
.planches { display:flex; gap:24px; align-items:flex-start; flex-wrap:wrap; }
figure { margin:0; flex:none; }
figcaption { margin-top:8px; font-size:11px; letter-spacing:.08em; text-transform:uppercase;
  color:${CLAIR.muted}; font-weight:600; }
.prose { flex:1; min-width:340px; }
.chiffres { display:flex; flex-wrap:wrap; gap:8px; margin-top:16px; padding:0; }
.chiffres span {
  font-size:12px; color:${CLAIR.ink2}; background:${CLAIR.surface};
  border:1px solid ${CLAIR.border}; padding:3px 9px;
}
.verdict { margin-top:14px; font-size:13px; font-weight:600; }
.verdict.ok { color:${CLAIR.accent}; }
.verdict.reserve { color:${CLAIR.muted}; }
.verdict.mesurer { color:${CLAIR.muted}; }
.verdict.non { color:${CLAIR.alert}; }
table.banc { border-collapse:collapse; font-size:12px; margin-top:12px; }
table.banc th, table.banc td { border:1px solid ${CLAIR.border}; padding:4px 8px; text-align:right; }
table.banc th[scope="row"] { text-align:left; font-weight:600; }
table.banc thead th { font-weight:600; font-size:11px; color:${CLAIR.muted};
  text-transform:uppercase; letter-spacing:.06em; }
table.banc td.pire { font-weight:600; color:${CLAIR.ink}; background:${CLAIR.surface}; }
table.banc tr.sous td { color:${CLAIR.alert}; }
ul { font-size:14px; line-height:1.6; color:${CLAIR.ink2}; max-width:78ch; padding-left:20px; }
li { margin-bottom:8px; }


.verdict-plan { border:1px solid ${CLAIR.border}; background:${CLAIR.surface}; padding:12px 14px; margin:14px 0 0; font-size:13.5px; line-height:1.6; max-width:80ch; }
/* ================= LE VOLET, À TAILLE RÉELLE ================= */
.cadre { width:400px; border:1px solid var(--border); background:var(--bg); }
${CSS_VOLET}

/* O1 — la pastille prend la colonne de tête, centrée sur la 1re ligne. */
.o1 .tete, .o2 .tete, .o4 .tete { grid-row:1; align-self:center; display:flex; }
.o1 .tete, .o4 .tete { width:16px; height:16px; }
.o2 .tete { width:18px; height:18px; }

/* O3 — la rive : le texte au fer à gauche, la marque au bord de fuite,
   centrée géométriquement sur la rangée (A33 : aucune correction à l'œil). */
.o3 .ligne { grid-template-columns:1fr auto; column-gap:12px; align-items:center; }
.o3 .bloc-texte { grid-column:1; display:grid; row-gap:3px; min-width:0; }
.o3 .l1, .o3 .objet, .o3 .apercu, .o3 .puces { grid-column:1; }
.o3 .col-rive { grid-column:2; display:flex; }

/* O5, O6, O7 — plus de colonne de tête du tout. */
.sans-tete { grid-template-columns:1fr; }
.sans-tete .l1, .sans-tete .objet, .sans-tete .apercu, .sans-tete .puces { grid-column:1; }

/* O5 — le mot : la grammaire du sourcil de la nav, au caractère près. */
.mot-compte { flex:none; font-size:11px; letter-spacing:.1em; text-transform:uppercase;
  color:var(--muted); font-weight:600; }

/* O6 — le liseré : la réserve de 2 px prend la teinte du compte. */
${TEINTES.map((n) =>
  `.theme-clair .o6 .ligne[data-teinte="${n}"] { border-left-color:${REP_SOMBRES[n]}; }`
  + `.theme-nuit .o6 .ligne[data-teinte="${n}"] { border-left-color:${REP_CLAIRES[n]}; }`).join('\n')}
.o6 .ligne.choisie { border-left-color:var(--accent) !important; }

/* O7 — la puce de compte : la grammaire des puces, glyphe à la teinte. */
${TEINTES.map((n) =>
  `.theme-clair .puce-compte[data-teinte="${n}"] .ic { color:${REP_SOMBRES[n]}; }`
  + `.theme-nuit .puce-compte[data-teinte="${n}"] .ic { color:${REP_CLAIRES[n]}; }`).join('\n')}
.puce-compte { color:var(--ink); font-weight:600; }
`;

// --- La page ----------------------------------------------------------
const bloc = (o) => {
  const verdicts = {
    ok: 'Tient les règles — départage à l’œil',
    reserve: 'Réserve nommée ci-dessus',
    mesurer: 'À mesurer avant d’en discuter le dessin',
    non: 'Demande un renversement d’A74 — décision du Chef Ingénieur',
  };
  return `
<section class="bloc" id="${o.id}">
  <div class="tete-opt">
    <span class="num">${o.num}</span>
    <div><h2>${o.nom}</h2><p class="meca">${o.meca}</p></div>
  </div>
  <div class="planches">
    <figure>${rendreListe(o.id, 'theme-clair')}<figcaption>Elements — clair</figcaption></figure>
    <figure>${rendreListe(o.id, 'theme-nuit')}<figcaption>Elements · nuit</figcaption></figure>
    <div class="prose">
      <h3>Ce que ça dit</h3><p>${o.dit}</p>
      <h3>Ce que ça coûte</h3><p>${o.cout}</p>
      <h3>Gate, e2e, Système</h3><p>${o.gate}</p>
      <p class="chiffres">${o.chiffres.map((c) => `<span>${c}</span>`).join('')}</p>
      ${o.verdict ? `<p class="verdict ${o.verdict}">${verdicts[o.verdict]}</p>` : ''}
      ${o.id === 'o2' ? bancO2() : ''}
    </div>
  </div>
</section>`;
};

function bancO2() {
  return `
<h3>Mesuré — la teinte tracée à même le fond (seuil composant 3:1)</h3>
<table class="banc">
  <thead><tr><th></th>
    ${FONDS_BANC.map(([, ou]) => `<th colspan="2">${ou}</th>`).join('')}
    <th>pire</th></tr>
  <tr><th></th>${FONDS_BANC.map(() => '<th>clair</th><th>nuit</th>').join('')}<th></th></tr></thead>
  <tbody>${bancGlypheNu()}</tbody>
</table>
<p style="margin-top:8px">Les ${TEINTES.length * FONDS_BANC.length * 2} mesures sont
calculées à la génération, aux formules de <code>e2e/contraste.mjs</code>, sur les hex lus dans
<code>systeme.css</code>. Pire cas du nuancier entier : <b>${fmt(PIRE_GLOBAL)}:1</b> —
${PIRE_GLOBAL >= 3 ? 'au-dessus du seuil composant, sur les quatre fonds et les deux polarités'
    : 'SOUS le seuil composant'}. Le glyphe nu est donc légal ; ce que la mesure ne dit pas,
c’est s’il se TROUVE d’un balayage — ça, c’est le verdict de l’œil.
<b>O2 se juge en situation</b> : <a href="o2.html">la fenêtre entière, quatorze rangées,
la nav à côté</a>.</p>`;
}

const html = `<!doctype html>
<html lang="fr"><head><meta charset="utf-8">
<title>Sept dessins du volet central — le repère de boîte</title>
<style>${FEUILLE}</style></head>
<body><div class="page">
<header class="tete-page">
  <p class="sourcil">Spike jetable · 2026-08-24 · rien ici n’est livré</p>
  <h1>Sept dessins du volet central</h1>
  <p class="sous-titre">Retirer la tuile aux initiales de l’expéditeur, et rendre le repère de la
  boîte de réception <b>à la fois visible et discret</b>. Les sept dessins portent la même matière —
  mêmes six rangées, mêmes trois comptes, même largeur de 400 px (le défaut du volet) — pour que la
  comparaison porte sur le repère et sur rien d’autre. Les jetons et les 24 hex du nuancier sont
  LUS de <code>systeme.css</code>, les glyphes importés du jeu livré : cette planche ne peut pas
  mentir sur ce que Wind expédie. La quatrième rangée est <b>choisie</b> dans les huit dessins —
  c’est là que se voient les conflits.</p>
<p class="verdict-plan"><b>Verdict rendu le 2026-08-24.</b> Cette planche est une <b>trace d’exploration</b> : les arbitrages qu’elle pose sont tranchés, et le normatif vit désormais dans <code>docs/PLAN-REPERE-LIGNE.md</code> — la boîte se dit <b>en toutes lettres</b> sur la ligne de l’expéditeur, la tuile aux initiales quitte la liste seule, le repère devient un <b>tracé nu</b> (nav comprise). Ne pas relire ceci comme une question ouverte.</p>
</header>

<section class="bloc" id="enjeu">
  <h2>Ce que le retrait décide, avant même de choisir un dessin</h2>
  <p>Trois arbitrages tombent du seul fait de retirer la tuile. Ils valent pour les sept dessins,
  et ils sont au Chef Ingénieur.</p>
  <ul>
    <li><b>D-a — le repère s’affiche-t-il PARTOUT ?</b> Aujourd’hui il ne paraît que là où les
    comptes se mélangent (A74, D3 : boîte unifiée, recherche) ; la tuile, elle, était toujours là.
    Retirée, la colonne de tête devient VIDE dans la vue d’un seul compte : ou bien le texte se
    décale entre les vues (l’œil le voit), ou bien la colonne reste réservée sur du vide (38 px
    payés pour rien). Trancher : repère partout, ou colonne qui disparaît avec lui.</li>
    <li><b>D-b — le compte SANS repère.</b> Le repère est optionnel (A74) : deux comptes sans
    repère sont aujourd’hui indiscernables en liste. Trois issues : un repli neutre (le glyphe
    <code>inbox</code> à l’encre atténuée — honnête mais muet), une teinte attribuée d’office à
    la connexion (le compte a toujours une identité, l’utilisateur la change), ou le repère rendu
    obligatoire à l’ajout de compte. Le dessin ne peut pas trancher ça à la place du produit.</li>
    <li><b>D-c — la portée : la liste seule, ou le fil aussi ?</b> Les cartes du fil portent la
    même tuile aux initiales (A45, <code>Fil.svelte:264</code> et <code>:428</code>), et le
    dossier Brouillons aussi (<code>Liste.svelte:888</code>). Dans un fil, l’expéditeur CHANGE
    d’un message à l’autre : l’initiale y travaille vraiment. Mon avis — la retirer de la liste,
    la garder au fil, et le dire au Système ; mais deux dessins pour un même objet demandent une
    phrase écrite, sinon c’est une incohérence.</li>
  </ul>
</section>

${OPTIONS.map(bloc).join('')}

<section class="bloc" id="ecartees">
  <h2>Écartées — pour que rien ne se re-propose sans raison neuve</h2>
  <ul>
    <li><b>Le sol teinté</b> (la rangée prend un lavis de la teinte du compte) : la couleur seule
    à nouveau (A8), et surtout le fond de rangée porte DÉJÀ quatre états — repos, survol,
    choisie, épinglée. Un cinquième sens sur le même canal les efface tous ; et chaque encre de
    la rangée devrait se re-mesurer sur douze fonds × deux polarités, soit une centaine de paires
    neuves pour la gate.</li>
    <li><b>La tuile aux initiales du COMPTE</b> (« TR », « MA ») : c’est remplacer une tuile par
    une tuile — la demande était de la retirer. Et deux comptes sur le même domaine rendent les
    mêmes lettres.</li>
    <li><b>Le repère au survol seulement</b> : une information qui n’existe qu’au geste n’est pas
    « visible » ; elle est absente pour qui balaie, et inexistante au clavier.</li>
    <li><b>Le liseré cranté</b> (une encoche par compte, pour tenir A8 sans couleur) : douze
    motifs de 2 px ne se distinguent pas à l’œil — ce serait une conformité de façade.</li>
  </ul>
</section>

<section class="bloc" id="verdict">
  <h2>Ce que je recommande — et ce qui reste à trancher</h2>
  <p>Aucune des sept n’introduit une paire de contraste neuve : le nuancier d’A74 avait déjà tout
  payé. Aucune, sauf O7, ne touche aux deux gabarits de hauteur. Le départage n’est donc pas
  technique — il est <b>à l’œil, sur cette planche, aux deux polarités</b>.</p>
  <p>Mon classement, à défendre : <b>O2 (le glyphe nu)</b> d’abord — c’est la seule qui rende
  service au Système au lieu de lui coûter (après elle, le rond ne dit plus QUE l’état, et la
  phrase de V14 se raccourcit), et « discret » y est obtenu par la forme, pas par la taille.
  Puis <b>O1</b>, le plus petit pas, si le glyphe nu se révèle trop léger à la fenêtre réelle.
  Puis <b>O5 (le mot)</b> si vous voulez une identité qui ne demande rien à la couleur — c’est
  la plus robuste des sept, et la plus coûteuse en largeur.</p>
  <p><b>Ce que cette planche ne peut pas dire</b>, et qu’il faut aller voir : le rendu à la
  fenêtre réelle sur vos vraies boîtes (un navigateur n’est pas WebView2 à votre échelle de
  texte), la fréquence d’alternance des comptes en boîte unifiée (elle décide d’O4 à elle seule),
  et le passage de la poignée de défilement sur la rive d’O3.</p>
</section>
</div></body></html>`;

const sortie = path.join(ici, 'planche.html');
writeFileSync(sortie, html, 'utf8');
console.log(`Écrit : ${path.relative(RACINE, sortie)}`);
console.log(`  ${TEINTES.length} teintes × 2 polarités`);
console.log(`  banc du glyphe nu : pire cas ${fmt(PIRE_GLOBAL)}:1 (seuil composant 3:1)`);

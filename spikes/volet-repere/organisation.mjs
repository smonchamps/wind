// ====================================================================
// Quinze organisations du volet central — page JETABLE, hors produit.
//
//   node spikes/volet-repere/organisation.mjs
//
// Énoncé du Chef Ingénieur (2026-08-24, suite des sept dessins) :
// quinze propositions NEUVES d'organisation du volet central, pour que
// la boîte ayant reçu chaque message se voie simplement, intuitivement,
// aux règles du Système, et en harmonie avec le reste de l'interface.
//
// La différence avec la planche des sept : là on déplaçait une MARQUE ;
// ici on réorganise le VOLET — l'ordre des rangs, le regroupement du
// flot, ce qui appartient à la colonne plutôt qu'à la rangée, le
// bandeau et le pied. Les sept dessins (planche.html) restent
// disponibles : plusieurs des quinze se marient avec l'un d'eux.
//
// RÈGLE DE COMPARAISON : là où une organisation a besoin d'une marque
// SANS que ce soit son sujet, c'est le glyphe nu d'O2 qui sert — le
// même partout. Sinon on comparerait des marques, pas des
// organisations.
//
// Décor : le même que o2.html — quatorze rangées, trois comptes,
// alternance forte (12 suites pour 14 rangées, 6 journées), volet à
// 400 px. Un décor défavorable aux organisations qui regroupent : c'est
// voulu, et c'est dit à chaque fois que ça compte.
// ====================================================================
import { writeFileSync } from 'node:fs';
import path from 'node:path';
import {
  RACINE, CLAIR, TEINTES, REP_SOMBRES, REP_CLAIRES,
  ico, COMPTES, FIL, jourDe, suites,
  disque, ligne1, corps, rangPuces, glypheNu, classes, CSS_VOLET,
} from './socle.mjs';

const ici = import.meta.dirname;
const glyphe = (l, t = 16) => glypheNu(COMPTES[l.c], t);
const teinteVar = (l) => `var(--rep-${COMPTES[l.c].teinte})`;

// --- Les pièces communes du volet ------------------------------------
const BANDEAU = `<header class="bandeau"><h1>Boîte de réception</h1><p class="verdict-plan"><b>Verdict rendu le 2026-08-24.</b> Cette planche est une <b>trace d’exploration</b> : les arbitrages qu’elle pose sont tranchés, et le normatif vit désormais dans <code>docs/PLAN-REPERE-LIGNE.md</code> — la boîte se dit <b>en toutes lettres</b> sur la ligne de l’expéditeur, la tuile aux initiales quitte la liste seule, le repère devient un <b>tracé nu</b> (nav comprise). Ne pas relire ceci comme une question ouverte.</p>
</header>`;
const PIED = `<div class="onglets">
  <span class="onglet actif">${ico('inbox')}Tous</span>
  <span class="onglet">${ico('mark_email_unread')}Non lus</span>
  <span class="onglet">${ico('edit_note')}Brouillons</span>
</div>`;

// La rangée de référence, sans marque — celle que les organisations
// qui marquent ailleurs (ou pas du tout) reprennent telle quelle.
const rangNu = (l) =>
  `<div class="${classes(l)} sans-tete">${ligne1(l)}${corps(l)}${rangPuces(l)}</div>`;
// La rangée au glyphe nu en tête (O2) — la marque neutre de la planche.
const rangO2 = (l) =>
  `<div class="${classes(l)}"><span class="tete">${glyphe(l)}</span>`
  + `${ligne1(l)}${corps(l)}${rangPuces(l)}</div>`;

const colonne = (cle, { bandeau = BANDEAU, pied = PIED, flot }) =>
  `<div class="colonne ${cle}">${bandeau}`
  + `<div class="cadre-liste"><div class="liste">${flot}</div></div>${pied}</div>`;

// ====================================================================
// FAMILLE A — La rangée réorganisée
// ====================================================================

// P1 · L'objet d'abord : l'ordre de lecture change, le compte
// accompagne l'expéditeur au deuxième rang.
const p1 = (l) => `<div class="${classes(l)} sans-tete">
  <div class="r-objet"><p class="objet">${l.objet}</p><span class="heure">${l.heure}</span></div>
  <div class="r-exp">${disque(l)}${glyphe(l, 14)}<span class="exp">${l.exp}</span></div>
  <p class="apercu">${l.apercu}</p>${rangPuces(l)}</div>`;

// P2 · La colonne « quand et d'où » : l'heure quitte la ligne d'entête
// pour une colonne de droite, le repère se pose sous elle.
const p2 = (l) => `<div class="${classes(l)} p2-ligne">
  <div class="bloc-texte">
    <div class="l1">${disque(l)}<span class="exp">${l.exp}</span></div>
    <p class="objet">${l.objet}</p><p class="apercu">${l.apercu}</p>${rangPuces(l)}
  </div>
  <span class="coin"><span class="heure">${l.heure}</span>${glyphe(l)}</span></div>`;

// P3 · La rangée dense : l'aperçu rejoint l'objet sur un seul rang.
const p3 = (l) => `<div class="${classes(l)} dense">
  <span class="tete">${glyphe(l)}</span>
  <div class="l1">${disque(l)}<span class="exp">${l.exp}</span><span class="heure">${l.heure}</span></div>
  <p class="objet">${l.objet}<span class="suite"> — ${l.apercu}</span></p>${rangPuces(l)}</div>`;

// P4 · La colonne d'état ET d'identité : le disque de non-lu quitte la
// ligne d'entête et s'empile avec le repère dans une colonne de tête
// qui existe TOUJOURS — la réponse structurelle à D-a.
const p4 = (l) => `<div class="${classes(l)} p4-ligne">
  <span class="col-etat"><span class="fente">${disque(l)}</span>${glyphe(l)}</span>
  <div class="l1"><span class="exp">${l.exp}</span><span class="heure">${l.heure}</span></div>
  <p class="objet">${l.objet}</p><p class="apercu">${l.apercu}</p>${rangPuces(l)}</div>`;

// ====================================================================
// FAMILLE B — Le flot organisé
// ====================================================================

// P5 · Les sourcils de compte : le flot est trié par compte, chaque
// section porte son en-tête collant, aucune marque par rangée.
const ORDRE = ['travail', 'maison', 'etudes'];
const p5Flot = ORDRE.map((c) => {
  const lignes = FIL.filter((l) => l.c === c);
  const cpt = COMPTES[c];
  return `<div class="section"><div class="sourcil-compte">`
    + `<span class="sc-glyphe" data-teinte="${cpt.teinte}">${ico(cpt.icone, 14)}</span>`
    + `${cpt.nom}<span class="sc-n">${lignes.length}</span></div>`
    + lignes.map(rangNu).join('') + `</div>`;
}).join('');

// P6 · Les intercalaires de jour : le flot garde l'ordre de date et
// gagne le rythme des journées ; la marque peut alors être légère.
const p6Flot = (() => {
  let jour = null;
  return FIL.map((l) => {
    const j = jourDe(l);
    const tete = j === jour ? '' : `<div class="jour">${j}</div>`;
    jour = j;
    return tete + rangO2(l);
  }).join('');
})();

// P7 · Le peloton : une bride verticale accolée à la SUITE des rangées
// d'un même compte — elle dit l'étendue, pas seulement le changement.
const p7Flot = suites(FIL).map((suite) => {
  const cpt = COMPTES[suite[0].c];
  return `<div class="run" style="--t:var(--rep-${cpt.teinte})">`
    + `<span class="bride"><span class="bride-glyphe" data-teinte="${cpt.teinte}">`
    + `${ico(cpt.icone, 16)}</span><span class="trait"></span></span>`
    + `<div class="rangs">${suite.map(rangNu).join('')}</div></div>`;
}).join('');

// P8 · Les voies : une colonne par compte, côte à côte dans le volet.
const p8Flot = `<div class="voies">${ORDRE.map((c) => {
  const cpt = COMPTES[c];
  return `<div class="voie"><div class="voie-tete">`
    + `<span class="voie-glyphe" data-teinte="${cpt.teinte}">${ico(cpt.icone, 14)}</span>`
    + `${cpt.nom}</div>`
    + FIL.filter((l) => l.c === c).map((l) => `<div class="${classes(l)} voie-ligne">`
      + `<div class="l1">${disque(l)}<span class="exp">${l.exp}</span></div>`
      + `<p class="objet">${l.objet}</p></div>`).join('')
    + `</div>`;
}).join('')}</div>`;

// ====================================================================
// FAMILLE C — Ce qui appartient au volet, pas à la rangée
// ====================================================================

// P9 · La gouttière de repères : un rail de 24 px qui appartient à la
// COLONNE, séparé par le filet unique — les marques font une vraie
// colonne, et la teinte de sélection ne la traverse pas.
const p9Flot = FIL.map((l) =>
  `<span class="rail" style="--t:${teinteVar(l)}">${glyphe(l)}</span>${rangNu(l)}`).join('');

// P10 · Le filet de compte : le filet de séparation ne se pose QU'AU
// changement de compte — le regroupement se fait par la structure, pas
// par la couleur ; le glyphe ouvre chaque bloc.
const p10Flot = suites(FIL).map((suite) =>
  `<div class="bloc-compte">`
  + suite.map((l, i) => (i === 0 ? rangO2(l) : rangNu(l))).join('')
  + `</div>`).join('');

// P11 · Le sommaire de tête : sous le bandeau, la légende des comptes
// présents — l'œil apprend le nuancier avant de descendre.
const P11_BANDEAU = BANDEAU + `<div class="sommaire">${ORDRE.map((c) => {
  const cpt = COMPTES[c];
  return `<span class="som-puce"><span class="som-glyphe" data-teinte="${cpt.teinte}">`
    + `${ico(cpt.icone, 14)}</span>${cpt.nom}`
    + `<span class="som-n">${FIL.filter((l) => l.c === c).length}</span></span>`;
}).join('')}</div>`;

// P12 · Le décrochement : l'indentation dit le compte. Zéro couleur,
// zéro glyphe — la seule réponse purement structurelle des quinze.
const DECALAGE = { travail: 0, maison: 10, etudes: 20 };
const p12Flot = FIL.map((l) =>
  `<div class="${classes(l)} sans-tete" style="padding-left:${16 + DECALAGE[l.c]}px">`
  + `${ligne1(l)}${corps(l)}${rangPuces(l)}</div>`).join('');

// ====================================================================
// FAMILLE D — Le bandeau et le pied
// ====================================================================

// P13 · Le bandeau segmenté : le bandeau cesse de nommer la boîte (la
// nav le dit déjà) et devient le sélecteur de compte.
const P13_BANDEAU = `<header class="bandeau bandeau-seg">
  <span class="seg actif">${ico('all_inbox', 14)}Toutes</span>
  ${ORDRE.map((c) => {
    const cpt = COMPTES[c];
    return `<span class="seg"><span class="seg-glyphe" data-teinte="${cpt.teinte}">`
      + `${ico(cpt.icone, 14)}</span>${cpt.nom}</span>`;
  }).join('')}
</header>`;

// P14 · Le pied à deux registres : les onglets d'état gardent leur
// rang, les comptes en prennent un second.
const P14_PIED = PIED + `<div class="onglets registre-2">
  <span class="seg actif">${ico('all_inbox', 14)}Toutes</span>
  ${ORDRE.map((c) => {
    const cpt = COMPTES[c];
    return `<span class="seg"><span class="seg-glyphe" data-teinte="${cpt.teinte}">`
      + `${ico(cpt.icone, 14)}</span>${cpt.nom}</span>`;
  }).join('')}
</div>`;

// P15 · La bascule « Grouper par boîte » : l'organisation devient un
// geste. Le volet montré est le mode DATE (marque légère) ; la bascule
// le fait passer à l'organisation de P5.
const P15_BANDEAU = `<header class="bandeau">
  <h1>Boîte de réception</h1>
  <span class="bascule">${ico('all_inbox', 14)}Grouper par boîte</span>
</header>`;

// ====================================================================
// Les quinze
// ====================================================================
const PROPOSITIONS = [
  // --- Famille A ------------------------------------------------------
  {
    id: 'p1', num: 'P1', famille: 'A · La rangée réorganisée', nom: 'L’objet d’abord',
    meca: 'L’ordre des rangs s’inverse — l’objet monte, l’expéditeur descend',
    dit: `Le rang 1 devient l’objet (c’est ce qu’on cherche en balayant une liste), le rang 2
      l’expéditeur — et le compte se pose là, contre le nom : « qui » et « d’où » sont la même
      question, ils vivent sur la même ligne. Le rang 3 ne bouge pas.`,
    cout: `C’est un renversement d’habitude : tous les clients courrier mettent l’expéditeur en
      tête, et la maquette Classique aussi. Le non-lu se dit toujours par la graisse et le
      disque, mais la graisse porte maintenant sur l’objet, plus sur le nom.`,
    regles: `Trois rangs, deux gabarits : A44 intact. Aucune paire neuve. Aucun glyphe neuf.`,
    chiffres: ['deux gabarits : 87 / 114', 'texte : 326 px', 'aucune paire neuve'],
    verdict: 'ok',
    vue: () => colonne('p1', { flot: FIL.map(p1).join('') }),
  },
  {
    id: 'p2', num: 'P2', famille: 'A · La rangée réorganisée', nom: 'La colonne « quand et d’où »',
    meca: 'L’heure quitte la ligne d’entête et emmène le repère dans une colonne de droite',
    dit: `Le coin haut-droit d’une rangée répond déjà à « quand ». Il répond maintenant aussi à
      « d’où » : l’heure, et sous elle le glyphe du compte. Les deux circonstances du message
      sont au même endroit, et la ligne d’entête est rendue ENTIÈRE au nom de l’expéditeur —
      qui cesse d’être tronqué à 400 px.`,
    cout: `La barre de défilement d’A44 est native et posée SUR le contenu (0 px réservé) : la
      poignée passe au bord droit, à 16 px du glyphe. À vérifier à la fenêtre réelle — c’est le
      même point que la rive d’O3, en moins exposé.`,
    regles: `Trois rangs, deux gabarits : A44 intact. Le coin est centré géométriquement sur ses
      deux objets, aucune correction à l’œil (A33).`,
    chiffres: ['deux gabarits : 88 / 115', 'le coin coûte 41 px de texte', 'nom d’expéditeur : pleine largeur'],
    verdict: 'ok',
    vue: () => colonne('p2', { flot: FIL.map(p2).join('') }),
  },
  {
    id: 'p3', num: 'P3', famille: 'A · La rangée réorganisée', nom: 'La rangée dense',
    meca: 'Deux rangs au lieu de trois — l’aperçu rejoint l’objet',
    dit: `L’objet et l’aperçu partagent un rang, séparés d’un tiret cadratin : l’objet à l’encre
      pleine, la suite atténuée. La rangée passe de 88 à 67 px — un quart de moins — et l’écran
      gagne <b>33 % de rangées</b> (mesuré) — et dans une liste plus dense, une marque de tête se trouve MIEUX, parce que la
      colonne de marques est plus serrée.`,
    cout: `L’aperçu se coupe beaucoup plus tôt : à 400 px, il ne reste qu’une poignée de mots
      après l’objet. C’est un arbitrage produit — voir plus de messages, ou en lire davantage de
      chacun — et il ne se tranche pas au dessin.`,
    regles: `Toujours DEUX gabarits — 67 nue, 94 porteuse (mesuré) : A44 tient, les sondes
      mesurent les nouvelles hauteurs sans changer une ligne du fenêtrage.`,
    chiffres: ['deux gabarits : 67 / 94', '+33 % de rangées (mesuré)', 'aucune paire neuve'],
    verdict: 'ok',
    vue: () => colonne('p3', { flot: FIL.map(p3).join('') }),
  },
  {
    id: 'p4', num: 'P4', famille: 'A · La rangée réorganisée', nom: 'La colonne d’état et d’identité',
    meca: 'Le disque de non-lu descend dans la colonne de tête et s’empile avec le repère',
    dit: `Une seule colonne de 16 px porte les deux choses que la rangée dit d’elle-même : son
      ÉTAT (le disque de 9 px, V4) au-dessus, son ORIGINE (le glyphe du compte) au-dessous.
      La fente du disque est RÉSERVÉE même sur une rangée lue : la ligne d’entête commence
      exactement au même fer sur toutes les rangées, ce qui n’est pas le cas aujourd’hui.`,
    cout: `Le disque quitte le centre de la rangée pour le haut de la colonne — V4 dit qu’il est
      « posé sur le centre géométrique de la rangée par construction ». Ici il est centré sur sa
      fente, dans une colonne centrée sur la première ligne : c’est une géométrie DÉCLARÉE, mais
      c’en est une autre, et le Système doit l’écrire.`,
    regles: `<b>Réponse structurelle à D-a</b> : la colonne existe toujours (elle porte le
      disque), donc le texte ne se décale JAMAIS entre la vue d’un compte et la boîte unifiée —
      l’arbitrage disparaît au lieu de se trancher.`,
    chiffres: ['deux gabarits : 88 / 115', 'fer d’entête stable', 'D-a résolu par construction'],
    verdict: 'ok',
    vue: () => colonne('p4', { flot: FIL.map(p4).join('') }),
  },
  // --- Famille B ------------------------------------------------------
  {
    id: 'p5', num: 'P5', famille: 'B · Le flot organisé', nom: 'Les sourcils de compte',
    meca: 'Le flot est trié par compte ; chaque section porte son en-tête',
    dit: `Plus une seule marque par rangée : la POSITION dit le compte. Le flot se coupe en
      sections, chacune ouverte par un sourcil (glyphe, nom, compte de messages) qui reste collé
      en haut pendant qu’on descend sa section. C’est l’organisation la plus lisible des quinze
      quand on veut traiter une boîte à la fois.`,
    cout: `Deux, et le premier est lourd. (1) <b>Le tri par date meurt</b> : le haut de la liste
      n’est plus « le plus récent », et un message urgent arrivé sur la troisième boîte se trouve
      en bas d’écran. (2) <b>Le fenêtrage y perd son modèle</b> : un sourcil est un TROISIÈME
      gabarit de hauteur, et il faut savoir à quelle section appartient l’index n — A44 est
      renversé, pas amendé.`,
    regles: `A44 renversé (trois hauteurs), et le tri par date — l’hypothèse de toute la liste —
      devient une option parmi deux.`,
    chiffres: ['3 sections ici', 'A44 renversé', 'tri par date perdu'],
    verdict: 'reserve',
    vue: () => colonne('p5', { flot: p5Flot }),
  },
  {
    id: 'p6', num: 'P6', famille: 'B · Le flot organisé', nom: 'Les intercalaires de jour',
    meca: 'Le flot garde l’ordre de date et gagne le rythme des journées',
    dit: `Un intercalaire ouvre chaque journée — « Aujourd’hui », « 4 août ». Ça ne dit pas le
      compte, ça donne à l’œil des ANCRES : entre deux intercalaires, une poignée de rangées, et
      dans un si petit paquet une marque légère (ici le glyphe nu) se trouve sans effort. C’est
      l’organisation qui rend les autres marques moins chères.`,
    cout: `Six intercalaires pour quatorze rangées sur ce décor — c’est beaucoup de chrome pour
      peu de rangées ; sur une boîte active (des dizaines de messages par jour) le rapport
      s’inverse et devient très favorable. <b>À mesurer sur vos boîtes</b>, comme O4.`,
    regles: `L’intercalaire est un troisième gabarit : A44 est touché — moins que P5 (l’ordre de
      date, lui, est intact et l’intercalaire se déduit de la rangée sans rien savoir des
      autres pages).`,
    chiffres: ['6 journées / 14 rangées ici', 'A44 : 3e gabarit', 'ordre de date intact'],
    verdict: 'mesurer',
    vue: () => colonne('p6', { flot: p6Flot }),
  },
  {
    id: 'p7', num: 'P7', famille: 'B · Le flot organisé', nom: 'Le peloton',
    meca: 'Une bride verticale accolée à la SUITE des rangées d’un même compte',
    dit: `Là où O4 marquait le changement, la bride montre l’ÉTENDUE : le glyphe en tête de la
      suite, puis un trait de 2 px à la teinte du compte qui court jusqu’à la dernière rangée du
      groupe. On lit « ces quatre-là viennent de Travail » d’un seul regard, sans compter.`,
    cout: `<b>Le décor tranche</b> : douze suites pour quatorze rangées ici — la bride n’a
      presque rien à tenir, et douze traits de 2 px valent une trame. Elle ne devient belle que
      si vos boîtes arrivent par paquets. Et elle est <b>hostile au fenêtrage</b> : une suite est
      un élément qui ENJAMBE des rangées, alors que le volet positionne les rangées une à une.`,
    regles: `A8 tenu (glyphe + couleur). Mais deux coûts que la mesure a sortis. Le fenêtrage
      devrait apprendre à couper une bride en deux à la couture de deux pages. Et surtout, le
      filet passe de la rangée à la SUITE : la hauteur de n rangées n’est plus n × h, elle
      dépend du nombre de suites qu’elles contiennent — <b>le fenêtrage ne peut plus calculer
      une position sans connaître le regroupement</b>, ce qui est exactement ce qu’A44 lui
      évitait.`,
    chiffres: ['12 suites / 14 rangées ici', 'hauteur ≠ n × h', 'fenêtrage : coût élevé'],
    verdict: 'mesurer',
    vue: () => colonne('p7', { flot: p7Flot }),
  },
  {
    id: 'p8', num: 'P8', famille: 'B · Le flot organisé', nom: 'Les voies',
    meca: 'Une colonne par compte, côte à côte dans le volet',
    dit: `La question disparaît : chaque compte a sa voie, la position EST l’information, et on
      voit d’un coup ce qui arrive partout. C’est le seul des quinze où l’on compare deux boîtes
      d’un seul regard.`,
    cout: `<b>Mesuré, et rédhibitoire au gabarit du produit</b> : à 400 px, trois voies font
      133 px chacune (mesuré) — l’objet tient six mots, l’aperçu ne tient plus du tout, et la voie ci-contre
      a déjà dû l’abandonner. Ça ne vit qu’en deux volets (la liste prend la largeur) et à deux
      comptes. Et le défilement devient une question sans bonne réponse : trois voies qui
      défilent ensemble mentent sur les dates, séparément coûtent trois barres.`,
    regles: `Le volet n’est plus une liste : ni A29/A30 (le dessin des pistes), ni A44 (le
      fenêtrage) ne s’appliquent. C’est un autre écran.`,
    chiffres: ['133 px par voie à 400', 'aperçu abandonné', 'A29/A30/A44 hors jeu'],
    verdict: 'non',
    vue: () => colonne('p8', { flot: p8Flot, pied: PIED }),
  },
  // --- Famille C ------------------------------------------------------
  {
    id: 'p9', num: 'P9', famille: 'C · Ce qui appartient au volet', nom: 'La gouttière de repères',
    meca: 'Un rail de 24 px qui appartient à la COLONNE, séparé par le filet unique',
    dit: `La marque sort de la rangée : elle vit sur un rail qui appartient au volet, bordé du
      filet de 1 px — le seul séparateur du Système (V3). Deux conséquences que les dessins
      « dans la rangée » n’ont pas : les marques forment une vraie <b>colonne</b> qu’on lit
      verticalement, et la teinte de sélection <b>s’arrête au filet</b> — choisir une rangée ne
      touche plus son identité.`,
    cout: `Un filet vertical qui court sur toute la hauteur est un trait de plus dans une
      interface qui n’en a qu’un — il faut vouloir cette ligne. Et le rail consomme 25 px qui ne
      servent qu’à ça, contre 26 pour la colonne de tête d’O1 (16 + 10 de gouttière) : le coût
      est le même, la lecture est différente.`,
    regles: `V3 servi (le filet porte la séparation), A44 intact, aucune paire neuve. La rangée,
      elle, redevient du texte pur.`,
    chiffres: ['rail 24 px + filet', 'la sélection ne traverse pas', 'aucune paire neuve'],
    verdict: 'ok',
    vue: () => colonne('p9', { flot: p9Flot }),
  },
  {
    id: 'p10', num: 'P10', famille: 'C · Ce qui appartient au volet', nom: 'Le filet de compte',
    meca: 'Le filet de séparation ne se pose QU’AU changement de compte',
    dit: `Le filet cesse de séparer toutes les rangées : il ne paraît qu’entre deux comptes
      différents, et le glyphe ouvre chaque bloc. Le regroupement se fait par la STRUCTURE, pas
      par la couleur — c’est le filet unique de V3 promu au rang d’outil de lecture, sans ajouter
      un seul objet au Système.`,
    cout: `Le filet entre rangées est le dessin des pistes (A29/A30) : le retirer à l’intérieur
      d’un bloc fait fondre les rangées les unes dans les autres, et ce qui gagne en groupe perd
      en dénombrement — on ne sait plus, d’un regard, où finit un message. Sur un décor à
      alternance forte (douze blocs pour quatorze rangées), le filet revient presque partout et
      le dessin ne dit plus rien.`,
    regles: `A29/A30 touchés (le filet de rangée). A8 tenu : le glyphe ouvre le bloc, la couleur
      ne porte rien seule.`,
    chiffres: ['12 blocs / 14 rangées ici', 'A29/A30 touchés', 'zéro objet neuf'],
    verdict: 'reserve',
    vue: () => colonne('p10', { flot: p10Flot }),
  },
  {
    id: 'p11', num: 'P11', famille: 'C · Ce qui appartient au volet', nom: 'Le sommaire de tête',
    meca: 'Sous le bandeau, la légende des comptes présents dans la vue',
    dit: `Une bande de 32 px sous le bandeau dit quels comptes sont présents, avec leur glyphe,
      leur teinte et leur compte de messages. L’œil apprend le nuancier AVANT de descendre — et
      une marque par rangée peut alors être très légère, puisqu’elle est déjà expliquée. C’est le
      seul des quinze qui ENSEIGNE le code au lieu de le supposer connu.`,
    cout: `<b>Le compteur est le piège</b> : « les comptages ont quitté le chemin d’affichage »
      (A64, et c’est ce qui a éteint perf-lecture). Un décompte par compte SUR LA VUE COURANTE
      est exactement le genre de chiffre qui rentrerait par la fenêtre. Sans les nombres, la
      bande reste utile — et gratuite. Coût restant : 32 px de chrome permanent, soit une
      rangée sur six perdue à 560 px de liste.`,
    regles: `Aucune règle de dessin touchée. Mais A64 doit être respecté : la légende SANS
      compteurs, ou des compteurs qui ne se calculent pas au rendu.`,
    chiffres: ['32 px de chrome', '≈ 1/3 de rangée', 'A64 : pas de comptage au rendu'],
    verdict: 'reserve',
    vue: () => colonne('p11', { bandeau: P11_BANDEAU, flot: FIL.map(rangO2).join('') }),
  },
  {
    id: 'p12', num: 'P12', famille: 'C · Ce qui appartient au volet', nom: 'Le décrochement',
    meca: 'L’indentation dit le compte — zéro couleur, zéro glyphe',
    dit: `Chaque compte a son retrait : 0, 10, 20 px. La seule proposition purement structurelle
      des quinze — rien à voir avec la vision des couleurs, rien à mémoriser dans un jeu de
      glyphes, et A8 est tenu par construction puisqu’aucune couleur ne porte rien.`,
    cout: `<b>Elle ne s’apprend pas.</b> Un retrait n’a pas de nom : rien ne dit que 10 px
      signifie Maison, et l’ordre des comptes est arbitraire. Le fer à gauche — ce qui fait tenir
      une liste debout — se déchire, et au-delà de trois comptes le dernier est indenté hors de
      la lisibilité. Le tenir honnêtement demanderait une légende, donc P11, donc autant marquer.`,
    regles: `Aucune règle rompue. C’est une proposition qui tombe sur l’usage, pas sur le
      Système — et elle est ici pour ne plus être re-proposée.`,
    chiffres: ['0 / 10 / 20 px', 'fer à gauche perdu', 'zéro couleur'],
    verdict: 'non',
    vue: () => colonne('p12', { flot: p12Flot }),
  },
  // --- Famille D ------------------------------------------------------
  {
    id: 'p13', num: 'P13', famille: 'D · Le bandeau et le pied', nom: 'Le bandeau segmenté',
    meca: 'Le bandeau cesse de nommer la boîte et devient le sélecteur de compte',
    dit: `La nav dit déjà quel dossier est ouvert ; le bandeau de 52 px répète cette information.
      Il devient donc le segment des comptes : Toutes, puis un segment par boîte, glyphe et
      teinte. En « Toutes », les rangées portent une marque légère ; dès qu’un compte est
      sélectionné, la marque disparaît — elle n’apprend plus rien.`,
    cout: `<b>Ça renverse une décision CE</b> : le bandeau ne porte que le nom de la boîte, SEUL
      (verdict du 2026-08-16, E1 — « Tout marquer lu » avait été écarté de là). Et à 400 px,
      quatre segments laissent <b>65 px de marge</b> (mesuré) : un cinquième compte déborde, et
      il faudrait le défilement horizontal, que le Système n’a nulle part.`,
    regles: `E1 renversé (décision du Chef Ingénieur). La grammaire du segment existe déjà —
      c’est celle des onglets du pied, A33 comprise.`,
    chiffres: ['4 segments, 65 px de marge', 'E1 renversé', 'marque conditionnelle'],
    verdict: 'reserve',
    vue: () => colonne('p13', { bandeau: P13_BANDEAU, flot: FIL.map(rangO2).join('') }),
  },
  {
    id: 'p14', num: 'P14', famille: 'D · Le bandeau et le pied', nom: 'Le pied à deux registres',
    meca: 'Les onglets d’état gardent leur rang, les comptes en prennent un second',
    dit: `Le pied est déjà l’endroit où l’on filtre (Tous, Non lus, Brouillons). Les comptes y
      prennent un second rang, à la même grammaire : le filtre d’ÉTAT et le filtre d’ORIGINE
      voisinent au même endroit, sans toucher au flot ni à la rangée.`,
    cout: `Le pied passe de 52 à 104 px : 52 px de liste perdus en permanence — mesuré, six
      dixièmes de rangée — pour un geste qu’on fait rarement. À 400 px, le second registre ne
      tient QUE dans la grammaire compacte des segments (celle des onglets déborde de 11 px,
      mesuré), et un quatrième compte déborderait à son tour. Enfin, deux registres empilés se
      lisent comme un seul jeu de sept filtres — c’est l’ambiguïté qu’un pied à un rang évitait.`,
    regles: `Aucune règle de dessin rompue ; c’est la densité et la clarté du pied qui paient.
      La nav, elle, fait déjà exactement ce travail — le doublon est réel.`,
    chiffres: ['pied 52 → 104 px', '61 px de marge au 2e rang', 'doublon avec la nav'],
    verdict: 'non',
    vue: () => colonne('p14', { pied: P14_PIED, flot: FIL.map(rangO2).join('') }),
  },
  {
    id: 'p15', num: 'P15', famille: 'D · Le bandeau et le pied', nom: 'La bascule « Grouper par boîte »',
    meca: 'L’organisation devient un geste — date, ou boîtes',
    dit: `Le volet ne choisit plus : il propose. En mode date (montré ci-contre) le flot est
      celui d’aujourd’hui, avec une marque légère ; la bascule le fait passer à l’organisation de
      P5, sections et sourcils, sans marque. Un seul contrôle, dans le bandeau, à la grammaire
      des boutons nus.`,
    cout: `<b>Refuser de trancher se paie deux fois</b> : deux flots à écrire, deux à tester, un
      fenêtrage qui doit tenir les deux, une préférence à persister — et un utilisateur à qui on
      demande de résoudre ce que le produit n’a pas su résoudre. Le Système dit la chose plus
      durement encore, à V14 : « un Système qui offre deux états de sa propre règle est un
      Système qui n’a pas tranché ».`,
    regles: `Rien de rompu, tout de doublé. À ne retenir que si les DEUX organisations se révèlent
      indispensables au terrain — jamais pour éviter un arbitrage.`,
    chiffres: ['2 flots à tenir', '1 préférence de plus', 'A44 dans les deux modes'],
    verdict: 'non',
    vue: () => colonne('p15', { bandeau: P15_BANDEAU, flot: FIL.map(rangO2).join('') }),
  },
];

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
ul, ol { font-size:14px; line-height:1.6; color:${CLAIR.ink2}; max-width:78ch; padding-left:20px; }
li { margin-bottom:8px; }
b { color:${CLAIR.ink}; font-weight:600; }
code { font-family:Consolas, monospace; font-size:12.5px; background:${CLAIR.surface};
  border:1px solid ${CLAIR.border}; padding:0 4px; }
a { color:${CLAIR.accent}; }
h2.famille { font-family:'Segoe UI Variable Display', -apple-system, BlinkMacSystemFont,
  "Segoe UI", sans-serif; font-weight:340; letter-spacing:-.03em; font-size:24px;
  margin:44px 0 0; padding-top:28px; border-top:1px solid ${CLAIR.border}; }
section.prop { padding:28px 0; border-bottom:1px solid ${CLAIR.border}; }
.tete-prop { display:flex; align-items:flex-start; gap:14px; margin-bottom:18px; }
.num { display:inline-flex; align-items:center; justify-content:center; width:36px; height:32px;
  border:1px solid ${CLAIR.border}; background:${CLAIR.surface};
  font-size:13px; font-weight:600; color:${CLAIR.ink}; flex:none; }
.tete-prop h3 { margin:0 0 3px; font-size:19px; font-weight:600; color:${CLAIR.ink}; }
.meca { margin:0; font-size:13px; color:${CLAIR.muted}; }
.planches { display:flex; gap:22px; align-items:flex-start; flex-wrap:wrap; }
figure { margin:0; flex:none; }
figcaption { margin-top:8px; font-size:11px; letter-spacing:.08em; text-transform:uppercase;
  color:${CLAIR.muted}; font-weight:600; }
.prose { flex:1; min-width:330px; }
.prose h4 { font-size:12px; letter-spacing:.06em; text-transform:uppercase;
  color:${CLAIR.muted}; margin:0 0 6px; font-weight:600; }
.prose h4 + p { margin-bottom:14px; }
.chiffres { display:flex; flex-wrap:wrap; gap:8px; margin-top:4px; }
.chiffres span { font-size:12px; color:${CLAIR.ink2}; background:${CLAIR.surface};
  border:1px solid ${CLAIR.border}; padding:3px 9px; }
.verdict { margin-top:12px; font-size:13px; font-weight:600; }
.verdict.ok { color:${CLAIR.accent}; }
.verdict.reserve, .verdict.mesurer { color:${CLAIR.muted}; }
.verdict.non { color:${CLAIR.alert}; }


.verdict-plan { border:1px solid ${CLAIR.border}; background:${CLAIR.surface}; padding:12px 14px; margin:14px 0 0; font-size:13.5px; line-height:1.6; max-width:80ch; }
/* ================= LE VOLET, À TAILLE RÉELLE ================= */
${CSS_VOLET}
.colonne { width:400px; height:560px; display:flex; flex-direction:column;
  background:var(--bg); border:1px solid var(--border); overflow:hidden; }
.bandeau { flex:none; height:52px; display:flex; align-items:center; gap:10px;
  padding:0 16px; background:var(--bg); border-bottom:1px solid var(--border); }
.bandeau h1 { margin:0; font-size:16px; font-weight:600; line-height:1.3; color:var(--ink);
  flex:1; min-width:0; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
.cadre-liste { flex:1; min-height:0; overflow:hidden; }
.onglets { flex:none; height:52px; padding:0 12px; display:flex; align-items:center; gap:10px;
  border-top:1px solid var(--border); background:var(--bg); }
.onglet { height:32px; padding:0 14px; display:inline-flex; align-items:center; gap:8px;
  font-size:13px; color:var(--ink2); background:var(--surface); border:1px solid var(--border);
  white-space:nowrap; }
.onglet.actif { font-weight:600; color:var(--ink); background:var(--sel); border-color:var(--accent); }
.tete { grid-row:1; align-self:center; display:flex; width:16px; height:16px; }
/* Sans colonne de tête : une seule colonne, et sa gouttière ne se paie
   plus. Sans cette règle la grille garde « auto 1fr » — 10 px perdus et
   l'aperçu tombe en colonne 2 (mesuré, corrigé). */
.sans-tete { grid-template-columns:1fr; }
.sans-tete > .l1, .sans-tete > .objet, .sans-tete > .apercu, .sans-tete > .puces,
.sans-tete > .r-objet, .sans-tete > .r-exp { grid-column:1; }

/* P1 — l'objet d'abord */
.p1 .r-objet { grid-column:1; display:flex; align-items:baseline; gap:10px; min-width:0; }
.p1 .r-exp { grid-column:1; display:flex; align-items:center; gap:8px; min-width:0; }
.p1 .r-objet .objet { flex:1; min-width:0; }
.p1 .r-exp .exp { font-size:13px; color:var(--ink2); }
.p1 .r-exp .disque, .p1 .r-exp .glyphe-compte { align-self:center; flex:none; }
.p1 .nonlu .exp { font-weight:600; }

/* P2 — la colonne « quand et d'où » */
.p2-ligne { grid-template-columns:1fr auto; column-gap:12px; align-items:start; }
.p2 .bloc-texte { grid-column:1; display:grid; row-gap:3px; min-width:0; }
.p2 .bloc-texte .l1, .p2 .bloc-texte .objet,
.p2 .bloc-texte .apercu, .p2 .bloc-texte .puces { grid-column:1; }
.p2 .coin { grid-column:2; display:flex; flex-direction:column; align-items:center; gap:6px; }

/* P3 — la rangée dense */
.dense .objet { white-space:nowrap; }
.dense .suite { color:var(--ink2); font-weight:400; }

/* P4 — la colonne d'état et d'identité */
.p4-ligne .col-etat { grid-row:1 / span 3; display:flex; flex-direction:column;
  align-items:center; gap:5px; width:16px; }
.p4-ligne .fente { height:9px; display:flex; align-items:center; justify-content:center; }
.p4-ligne .l1 { gap:0; }

/* P5 — les sourcils de compte */
.sourcil-compte { position:sticky; top:0; z-index:1; height:28px; display:flex;
  align-items:center; gap:8px; padding:0 16px; background:var(--bg);
  border-bottom:1px solid var(--border); font-size:11px; font-weight:600;
  letter-spacing:.1em; text-transform:uppercase; color:var(--muted); }
.sourcil-compte .sc-n { margin-left:auto; letter-spacing:0; color:var(--accent); }
.p5 .ligne:first-child { border-top:1px solid var(--border); }

/* P6 — les intercalaires de jour */
.jour { height:28px; display:flex; align-items:center; padding:0 16px;
  border-top:1px solid var(--border); font-size:11px; font-weight:600;
  letter-spacing:.1em; text-transform:uppercase; color:var(--muted); }
.jour + .ligne { border-top:1px solid var(--border); }

/* P7 — le peloton */
.run { display:grid; grid-template-columns:auto 1fr; border-top:1px solid var(--border); }
.run:first-child { border-top:none; }
.bride { display:flex; flex-direction:column; align-items:center; gap:6px;
  padding:14px 0 14px 16px; }
.bride .trait { width:2px; flex:1; background:var(--t); }
.p7 .rangs { min-width:0; }
.p7 .ligne { border-top:none; padding-left:10px; }
.p7 .ligne + .ligne { border-top:1px solid var(--border); }

/* P8 — les voies */
.voies { display:grid; grid-template-columns:repeat(3, 1fr); height:100%; }
.voie { min-width:0; border-right:1px solid var(--border); display:flex; flex-direction:column; }
.voie:last-child { border-right:none; }
.voie-tete { height:28px; display:flex; align-items:center; gap:6px; padding:0 8px;
  border-bottom:1px solid var(--border); font-size:10px; font-weight:600;
  letter-spacing:.06em; text-transform:uppercase; color:var(--muted); }
.voie-ligne { padding:10px 8px; border-top:1px solid var(--border);
  grid-template-columns:1fr; row-gap:2px; }
.voie-ligne:first-of-type { border-top:none; }
.voie-ligne .l1, .voie-ligne .objet { grid-column:1; }
.voie-ligne .exp { font-size:12px; }
.voie-ligne .objet { font-size:12px; }

/* P9 — la gouttière de repères */
.p9 .liste { display:grid; grid-template-columns:25px 1fr; }
.rail { border-right:1px solid var(--border); border-top:1px solid var(--border);
  display:flex; align-items:center; justify-content:center; }
.p9 .liste > .rail:first-child { border-top:none; }
.p9 .liste > .ligne:nth-child(2) { border-top:none; }

/* P10 — le filet de compte */
.bloc-compte { border-top:1px solid var(--border); }
.bloc-compte:first-child { border-top:none; }
.p10 .ligne { border-top:none; }

/* P11 — le sommaire de tête */
.sommaire { flex:none; height:32px; display:flex; align-items:center; gap:6px;
  padding:0 12px; border-bottom:1px solid var(--border); background:var(--bg); }
.som-puce { display:inline-flex; align-items:center; gap:5px; height:22px; padding:0 8px;
  font-size:11px; color:var(--ink2); background:var(--surface); border:1px solid var(--border); }
.som-n { color:var(--accent); font-weight:600; font-variant-numeric:tabular-nums; }

/* P13 — le bandeau segmenté */
.bandeau-seg { gap:6px; padding:0 10px; }
.seg { display:inline-flex; align-items:center; gap:5px; height:28px; padding:0 9px;
  font-size:12px; color:var(--ink2); background:var(--surface);
  border:1px solid var(--border); white-space:nowrap; }
.seg.actif { font-weight:600; color:var(--ink); background:var(--sel); border-color:var(--accent); }

/* P14 — le pied à deux registres. La grammaire des onglets déborde à
   400 px (410 px mesurés) : le second registre prend celle, compacte,
   des segments — et un quatrième compte déborderait à son tour. */
.registre-2 { height:52px; gap:6px; }

/* P15 — la bascule */
.bascule { display:inline-flex; align-items:center; gap:6px; height:26px; padding:0 10px;
  font-size:12px; color:var(--ink2); flex:none; }

/* Les glyphes teintés hors rangée (sourcils, voies, sommaire, segments) */
${['sc-glyphe', 'bride-glyphe', 'voie-glyphe', 'som-glyphe', 'seg-glyphe'].map((c) =>
  TEINTES.map((n) =>
    `.theme-clair .${c}[data-teinte="${n}"] { color:${REP_SOMBRES[n]}; }`
    + `.theme-nuit .${c}[data-teinte="${n}"] { color:${REP_CLAIRES[n]}; }`).join('\n')).join('\n')}
.sc-glyphe, .bride-glyphe, .voie-glyphe, .som-glyphe, .seg-glyphe { display:inline-flex; }
`;

// --- La page ----------------------------------------------------------
const VERDICTS = {
  ok: 'Tient les règles — départage à l’œil',
  reserve: 'Réserve nommée ci-dessus',
  mesurer: 'À mesurer sur vos boîtes avant d’en discuter le dessin',
  non: 'Écartée — la raison est écrite, pour qu’elle ne se re-propose pas',
};

let familleCourante = null;
const blocs = PROPOSITIONS.map((p) => {
  const tete = p.famille === familleCourante ? '' : `<h2 class="famille">${p.famille}</h2>`;
  familleCourante = p.famille;
  const vueClair = p.vue().replace('class="colonne', 'class="theme-clair colonne');
  const vueNuit = p.vue().replace('class="colonne', 'class="theme-nuit colonne');
  return `${tete}
<section class="prop" id="${p.id}">
  <div class="tete-prop">
    <span class="num">${p.num}</span>
    <div><h3>${p.nom}</h3><p class="meca">${p.meca}</p></div>
  </div>
  <div class="planches">
    <figure>${vueClair}<figcaption>Elements — clair</figcaption></figure>
    <figure>${vueNuit}<figcaption>Elements · nuit</figcaption></figure>
    <div class="prose">
      <h4>Ce que ça dit</h4><p>${p.dit}</p>
      <h4>Ce que ça coûte</h4><p>${p.cout}</p>
      <h4>Règles touchées</h4><p>${p.regles}</p>
      <p class="chiffres">${p.chiffres.map((c) => `<span>${c}</span>`).join('')}</p>
      <p class="verdict ${p.verdict}">${VERDICTS[p.verdict]}</p>
    </div>
  </div>
</section>`;
}).join('');

const html = `<!doctype html>
<html lang="fr"><head><meta charset="utf-8">
<title>Quinze organisations du volet central</title>
<style>${FEUILLE}</style></head>
<body><div class="page">
<header class="tete-page">
  <p class="sourcil">Spike jetable · 2026-08-24 · rien ici n’est livré</p>
  <h1 class="titre-page">Quinze organisations du volet central</h1>
  <p class="sous-titre">La planche des sept déplaçait une <b>marque</b>. Celle-ci réorganise le
  <b>volet</b> : l’ordre des rangs, le regroupement du flot, ce qui appartient à la colonne
  plutôt qu’à la rangée, le bandeau et le pied. Quatre familles, quinze propositions, chacune
  rendue à taille réelle (400 px, le défaut du volet) et dans les deux polarités.</p>
  <p><b>Règle de comparaison</b> — là où une organisation a besoin d’une marque sans que ce soit
  son sujet, c’est le <b>glyphe nu</b> d’O2 qui sert, le même partout : sinon on comparerait des
  marques, pas des organisations. Le décor est celui de
  <a href="o2.html">o2.html</a> : quatorze rangées, trois comptes, <b>alternance forte</b> —
  ${suites(FIL).length} suites pour ${FIL.length} rangées,
  ${new Set(FIL.map(jourDe)).size} journées. C’est un décor <b>défavorable</b> à tout ce qui
  regroupe : c’est voulu, et c’est dit là où ça compte.</p>
  <p><a href="planche.html">← les sept dessins</a> · <a href="o2.html">O2 en situation</a></p>
</header>
${blocs}
<section class="prop" id="synthese">
  <h2 class="famille" style="margin-top:0">Ce que ça donne</h2>
  <p><b>Quatre tiennent les règles sans réserve</b> — P1, P2, P3, P4 : toutes dans la famille A,
  toutes à trois ou deux rangs, toutes compatibles avec A44 et sans une paire de contraste
  neuve. Ce n’est pas un hasard : réorganiser la RANGÉE est ce que le Système autorise le plus
  volontiers, parce qu’il n’a jamais figé l’ordre de ses rangs.</p>
  <p><b>P4 mérite d’être regardée en premier.</b> Elle est la seule des vingt-deux propositions
  (sept + quinze) qui <b>fasse disparaître l’arbitrage D-a</b> au lieu de le trancher : la
  colonne de tête porte le disque de non-lu, donc elle existe toujours, donc le texte ne se
  décale jamais entre la vue d’un compte et la boîte unifiée. Elle se marie avec n’importe
  quelle marque — pastille d’O1 ou glyphe nu d’O2.</p>
  <p><b>P9 est la plus juste vis-à-vis du Système</b> : elle sert V3 (le filet unique porte la
  séparation), elle rend la rangée à son texte, et elle est la seule où la teinte de sélection
  ne traverse pas l’identité. Elle coûte un trait vertical permanent — c’est tout, et ça se
  regarde à l’œil.</p>
  <p><b>Trois se marient</b> plutôt qu’elles ne s’opposent : P6 (les journées) rend n’importe
  quelle marque moins chère ; P11 (le sommaire) enseigne le nuancier ; P3 (la densité) resserre
  la colonne de marques. Une organisation retenue + une marque des sept, c’est le format
  probable de la décision.</p>
  <p><b>Quatre sont écartées, et la raison est écrite</b> : P8 (les voies — 132 px par voie,
  mesuré), P12 (le décrochement — un retrait n’a pas de nom), P14 (le pied à deux registres —
  doublon avec la nav, deux rangées perdues), P15 (la bascule — refuser de trancher se paie
  deux fois).</p>
  <p><b>Ce que cette planche ne peut pas dire</b>, et qu’il faut aller voir : le rendu à la
  fenêtre réelle ; l’alternance réelle des comptes sur vos boîtes (elle décide de P7 et pèse
  sur P10) ; et le nombre de messages par journée (il décide de P6).</p>
</section>
</div></body></html>`;

const sortie = path.join(ici, 'organisation.html');
writeFileSync(sortie, html, 'utf8');
console.log(`Écrit : ${path.relative(RACINE, sortie)}`);
console.log(`  ${PROPOSITIONS.length} propositions, 4 familles, 2 polarités`);
console.log(`  décor : ${FIL.length} rangées, ${suites(FIL).length} suites, ${new Set(FIL.map(jourDe)).size} journées`);

// ====================================================================
// Sept signatures de cycle, comparées — page JETABLE, hors document.
//
//   node spikes/direction-elements/v2/signatures.mjs
//
// La signature est ce qui a remplacé le trait hitofude (V2) : la marque
// de « quelque chose tourne », à gauche de la barre d'état de 36 px, à
// 9 px de haut. Elle se juge à cette taille-là et pas à une autre —
// d'où la mise en situation à taille réelle, systématiquement première.
//
// Quatre contraintes, toutes déjà écrites au Système :
//   V4/V14  le DISQUE dit l'état — le rond est réservé, le carré est un
//           contenant. Une signature carrée se bat contre la doctrine.
//   V2      repos et cycle doivent être LE MÊME OBJET (le disque plein,
//           le disque évidé), pas deux dessins.
//   A8      mouvement coupé, la signature reste LISIBLE et DISTINCTE du
//           repos. Une animation qui gèle sur l'état de repos ne dit
//           plus rien.
//   A52     la signature ne porte JAMAIS la mesure : le pourcentage vit
//           dans le texte. Ce qui ressemble à une jauge est disqualifié.
// ====================================================================
import { writeFileSync } from 'node:fs';
import path from 'node:path';
import { CSS, THEMES, rapport } from './socle.mjs';
import { ico } from './socle.mjs';

const J = THEMES.elements.jetons;

const OPTIONS = [
  {
    id: 's1', nom: "L'anneau", famille: 'disque',
    sous: 'Le disque évidé, rotation continue — la proposition en place (V2)',
    dit: `Le disque plein du repos, ouvert d'un quart et mis à tourner. <b>Repos et cycle sont
      littéralement le même objet</b> : 9 px, <code>--marque</code>, 2 px de paroi. La rotation dit
      « en cours » sans ambiguïté dans aucune culture d'interface.`,
    cout: `Aucun. C'est le seul des sept qui n'ajoute ni forme, ni couleur, ni valeur.`,
    gel: `Anneau évidé, immobile — distinct du disque plein du repos. <b>A8 tenu.</b>`,
    verdict: 'ok',
  },
  {
    id: 's2', nom: 'Le disque et l’anneau', famille: 'disque',
    sous: 'Alternance plein ↔ évidé, sans rotation',
    dit: `L'objectif et l'objectif en train de se faire, en alternance. C'est la <b>doctrine à la
      lettre</b> : le jeu dit « le disque plein désigne un objectif », l'évidé le même en train
      d'advenir. Aucune rotation, aucune forme neuve, deux états déjà nommés.`,
    cout: `Le clignotement est associé à l'<b>alerte</b> dans presque toutes les interfaces. À 1,2 s
      c'est lent, mais le risque est réel — et c'est une signature qui vit en permanence en bas
      d'écran.`,
    gel: `Fige sur l'évidé — distinct du repos. <b>A8 tenu.</b>`,
    verdict: 'ok',
  },
  {
    id: 's5', nom: 'Le rabat de la marque', famille: 'disque',
    sous: 'Le demi-disque de Wind, r 3,25, qui tourne',
    dit: `La signature devient <b>la marque elle-même</b> : le demi-disque du rabat de l'enveloppe,
      seule forme orientée de tout le jeu. Identité maximale — on ne peut pas confondre Wind avec
      autre chose.`,
    cout: `Elle prend la <b>seule forme orientée</b> du jeu et lui fait perdre son orientation en la
      faisant tourner : ce qui donnait son sens au rabat — être tangent au bord haut — n'existe plus.
      Et à 9 px, un demi-disque de r 3,25 rendu à 1,2 px de corde est au bord du lisible.`,
    gel: `Demi-disque immobile, orienté au hasard de l'arrêt. Lisible, mais l'arrêt est arbitraire.`,
    verdict: 'reserve',
  },
  {
    id: 's6', nom: 'Le battement', famille: 'disque',
    sous: 'Le disque du repos, qui bat',
    dit: `Le minimum absolu : rien de neuf, le disque du repos change seulement d'intensité. Zéro
      forme, zéro valeur, zéro dessin.`,
    cout: `<b>Il échoue à A8.</b> Mouvement coupé, il gèle sur le disque plein — c'est-à-dire
      exactement l'état de repos : la signature ne dit plus rien du tout. Le geler à mi-intensité ne
      sauve rien, un disque terne se lit « inactif », pas « en cours ».`,
    gel: `Disque plein = l'état de repos. <b>A8 rompu.</b>`,
    verdict: 'ko',
  },
  {
    id: 's3', nom: 'Le carré au huitième de tour', famille: 'carré',
    sous: 'Un carré de 9, huit positions, pas de transition',
    dit: `La grammaire Elements transposée au mouvement : des positions <b>discrètes</b>, pas
      d'interpolation — l'équivalent temporel des jonctions vives. La silhouette bat carré / losange,
      ce qui se voit très bien à 9 px.`,
    cout: `<b>Il se bat contre V4 et V14</b> : dans ce système le carré est un contenant et le rond
      dit l'état. Un carré de 9 px qui bouge en bas d'écran se lit comme une tuile qui tremble.
      <b>Et il déborde de sa place, mesuré</b> : un carré de 9 en rotation balaie un disque de
      9 × √2 = <b>12,73 px</b>, soit 1,86 px de trop de chaque côté. Sa boîte reste à 9, donc la
      mise en page ne bouge pas — mais le dessin, lui, empiète. C'est le genre de correction que
      cette direction refuse par principe : aucune correction optique.`,
    gel: `Carré immobile — distinct du disque, mais il ressemble à une tuile de 9 px.`,
    verdict: 'reserve',
  },
  {
    id: 's4', nom: 'Les quatre coins', famille: 'carré',
    sous: 'Quatre carrés de 2 aux angles d’un 9 × 9, allumés en ronde',
    dit: `La rotation <b>sans rotation</b> : rien ne tourne, c'est la position allumée qui se déplace.
      Quatre coordonnées entières, aucune transformation, aucune correction optique. Doctrinalement,
      c'est le plus pur des sept.`,
    cout: `Quatre marques là où le système en réserve <b>une</b>. Et à 9 px, quatre carrés de 2 px
      séparés de 5 px forment une masse grise plus qu'un mouvement.`,
    gel: `Les quatre allumés : un cadre pointillé de 9 px. Distinct, mais muet.`,
    verdict: 'reserve',
  },
  {
    id: 's7', nom: 'Le balayage', famille: 'carré',
    sous: 'Un segment de 3 qui traverse une piste de 9',
    dit: `Le geste du trait hitofude, réduit à sa mécanique : quelque chose passe, de gauche à
      droite, en boucle.`,
    cout: `Deux objections, et elles sont lourdes. <b>A52</b> : une piste avec un segment dedans se
      lit comme une <b>mesure</b> — c'est précisément ce que A52 interdit à la signature, et c'est le
      défaut qui a tué le tracé partiel chez Chromium (A40). <b>A36</b> : la barre fine avait été
      retirée, la remettre est un retour en arrière non demandé.`,
    gel: `Segment arrêté quelque part dans la piste — se lit comme un pourcentage figé. <b>A52
      rompu.</b>`,
    verdict: 'ko',
  },
];

const ANIM = `
/* ===== Les sept signatures — page de comparaison, hors document ===== */
.sig{--sig:9px;--paroi:2px;flex:none;display:block}
.sig.gr{--sig:48px;--paroi:10px}
.fige .sig,.fige .sig *{animation:none !important}

/* 1 — l'anneau : le disque évidé qui tourne */
.s1{width:var(--sig);height:var(--sig);border-radius:50%;
  border:var(--paroi) solid var(--marque);border-top-color:transparent;
  animation:sigTourne 1s linear infinite}

/* 2 — le disque et l'anneau : alternance plein / évidé */
.s2{width:var(--sig);height:var(--sig);border-radius:50%;
  border:var(--paroi) solid var(--marque);background:var(--marque);
  animation:sigVide 1.2s steps(1,end) infinite}

/* 5 — le rabat de la marque, qui tourne */
.s5{width:var(--sig);height:var(--sig)}
.s5 g{transform-origin:12px 12px;animation:sigTourne 1.2s linear infinite}

/* 6 — le battement */
.s6{width:var(--sig);height:var(--sig);border-radius:50%;background:var(--marque);
  animation:sigBat 1.2s ease-in-out infinite}

/* 3 — le carré au huitième de tour */
.s3{width:var(--sig);height:var(--sig);background:var(--marque);
  animation:sigTourne 1.2s steps(8,end) infinite}

/* 4 — les quatre coins */
.s4{width:var(--sig);height:var(--sig);position:relative}
.s4 i{position:absolute;width:calc(var(--sig)*2/9);height:calc(var(--sig)*2/9);
  background:var(--marque);animation:sigCoin 1.2s steps(1,end) infinite}
.s4 i:nth-child(1){left:0;top:0;animation-delay:0s}
.s4 i:nth-child(2){right:0;top:0;animation-delay:.3s}
.s4 i:nth-child(3){right:0;bottom:0;animation-delay:.6s}
.s4 i:nth-child(4){left:0;bottom:0;animation-delay:.9s}

/* 7 — le balayage */
.s7{width:var(--sig);height:var(--sig);overflow:hidden;
  box-shadow:inset 0 0 0 1px var(--border)}
.s7 i{display:block;width:calc(var(--sig)/3);height:100%;background:var(--marque);
  animation:sigBalaye 1.1s linear infinite}

@keyframes sigTourne{to{transform:rotate(360deg)}}
@keyframes sigVide{0%,49.9%{background:var(--marque)}50%,100%{background:transparent}}
@keyframes sigBat{0%,100%{opacity:1}50%{opacity:.28}}
@keyframes sigCoin{0%,24.9%{opacity:1}25%,100%{opacity:.22}}
@keyframes sigBalaye{from{transform:translateX(-100%)}to{transform:translateX(300%)}}

/* mise en page de la comparaison */
.opt{background:var(--surface);border:1px solid var(--border);padding:22px;
  display:grid;grid-template-columns:300px 1fr;gap:22px;align-items:start}
.opt .g1{display:flex;flex-direction:column;gap:14px}
.opt .g2{display:flex;flex-direction:column;gap:11px}
.opt h3{font-size:17px;margin:0}
.opt .sscrit{font-size:12.5px;color:var(--muted);margin:0}
.opt p{margin:0;font-size:13.5px;line-height:1.62;color:var(--ink2)}
.opt .lab{font-size:10.5px;letter-spacing:.14em;text-transform:uppercase;color:var(--muted);font-weight:600}
.situ{border:1px solid var(--border);background:var(--bg)}
.paire{display:flex;align-items:center;gap:26px}
.paire .c{display:flex;flex-direction:column;align-items:center;gap:9px}
.paire .c span{font-size:10.5px;color:var(--muted)}
.badge{display:inline-flex;align-items:center;gap:7px;font-size:11px;font-weight:600;
  letter-spacing:.08em;text-transform:uppercase;padding:5px 10px;border:1px solid var(--border);
  color:var(--ink2);background:var(--bg)}
.badge.ok{color:var(--accent);border-color:var(--accent)}
.badge.ko{color:var(--alert);border-color:var(--alert)}
.fam{font-size:11px;letter-spacing:.14em;text-transform:uppercase;color:var(--muted);font-weight:600;
  margin:34px 0 -8px}
`;

// Le rabat de la marque, recentré sur le centre du cercle pour tourner
// autour de lui : le demi-disque d'origine a son centre en (12 ; 9,15),
// on le pose à (12 ; 12) — aucune cote décidée, une translation.
const rabat = `<svg class="sig s5" viewBox="0 0 24 24" aria-hidden="true">
  <g><path d="M8.75 12A3.25 3.25 0 0 0 15.25 12Z" fill="var(--marque)"/></g></svg>`;

const forme = (id, gr) => {
  const c = `sig ${id}${gr ? ' gr' : ''}`;
  if (id === 's5') return gr ? rabat.replace('sig s5', 'sig s5 gr') : rabat;
  if (id === 's4') return `<span class="${c}"><i></i><i></i><i></i><i></i></span>`;
  if (id === 's7') return `<span class="${c}"><i></i></span>`;
  return `<span class="${c}"></span>`;
};

const barre = (id) => `
  <div class="situ">
    <div class="statut" style="border-top:0">
      ${forme(id)}
      <span class="txt">Synchronisation · 2/4 · marie@atelier-brindille.fr · 37 %</span>
      <span class="bt"><span class="btn nu eteint">${ico('sync', 16)}Synchronisation…</span></span>
    </div>
  </div>`;

const bloc = (o, n) => `
  <div class="opt">
    <div class="g1">
      <p class="lab">Proposition ${n}</p>
      <h3>${o.nom}</h3>
      <p class="sscrit">${o.sous}</p>
      <div class="paire">
        <span class="c"><span class="sig" style="width:9px;height:9px;border-radius:50%;background:var(--marque)"></span><span>repos</span></span>
        <span class="c">${forme(o.id)}<span>cycle</span></span>
        <span class="c fige">${forme(o.id)}<span>mouvement coupé</span></span>
      </div>
      <div class="paire" style="gap:30px">
        <span class="c">${forme(o.id, true)}<span>agrandi ×5,3</span></span>
        <span class="c fige">${forme(o.id, true)}<span>coupé, agrandi</span></span>
      </div>
      <span class="badge ${o.verdict === 'ko' ? 'ko' : o.verdict === 'ok' ? 'ok' : ''}">
        ${o.verdict === 'ok' ? 'tient les quatre règles' : o.verdict === 'ko' ? 'rompt une règle' : 'réserve'}</span>
    </div>
    <div class="g2">
      <p class="lab">En situation — barre d'état de 36 px, taille réelle</p>
      ${barre(o.id)}
      <p><b>Ce qu'elle dit.</b> ${o.dit}</p>
      <p><b>Ce qu'elle coûte.</b> ${o.cout}</p>
      <p><b>Mouvement coupé (A8).</b> ${o.gel}</p>
    </div>
  </div>`;

const html = `<!DOCTYPE html>
<html lang="fr"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Wind — sept signatures de cycle</title>
<style>${CSS}${ANIM}</style></head>
<body><div class="dc" style="gap:26px">
  <header id="haut">
    <p class="sourcil">Wind · direction « Elements » · exploration hors document</p>
    <h1 class="display">Sept signatures<br>pendant un cycle</h1>
    <p class="lede">Ce qui a remplacé le trait hitofude (V2) : la marque de « quelque chose tourne »,
    à gauche de la barre d'état de 36 px, à <b>9 px</b> de haut. Elle se juge à cette taille-là et pas
    à une autre — la mise en situation à taille réelle vient donc toujours en premier, et
    l'agrandissement seulement après, pour juger le dessin.</p>
  </header>

  <div class="expl">
    <p class="t"><b>Quatre règles, déjà écrites, écartent quatre propositions sur sept.</b>
    <b>V4/V14</b> — le disque dit l'état, le carré est un contenant : une signature carrée se bat
    contre la doctrine. <b>V2</b> — repos et cycle doivent être <i>le même objet</i>, pas deux
    dessins. <b>A8</b> — mouvement coupé, la signature reste lisible <i>et distincte du repos</i>.
    <b>A52</b> — la signature ne porte jamais la mesure : ce qui ressemble à une jauge est
    disqualifié. Les sept sont quand même construites et animées : une règle se vérifie mieux sur ce
    qu'elle refuse.</p>
  </div>

  <p class="fam">Famille du disque — conforme à V4 / V14</p>
  ${OPTIONS.filter((o) => o.famille === 'disque').map((o, i) => bloc(o, i + 1)).join('')}

  <p class="fam">Famille du carré — en tension avec V4 / V14</p>
  ${OPTIONS.filter((o) => o.famille === 'carré').map((o, i) => bloc(o, i + 5)).join('')}

  <section style="margin-top:34px">
    <h2>Ce que je recommande</h2>
    <p class="sub"><b>L'anneau (proposition 1) est tenu</b>, et c'est un résultat, pas une paresse :
    des sept, c'est le seul qui n'ajoute ni forme, ni couleur, ni valeur, qui fasse du repos et du
    cycle le même objet, et qui gèle sur un état distinct. La rotation continue dit « en cours » sans
    ambiguïté ; aucune des six autres ne fait mieux sur les quatre règles à la fois.</p>
    <p class="sub"><b>Si tu veux bouger, la seule qui vaille l'essai est la 2</b> — le disque et
    l'anneau en alternance. Elle est doctrinalement <i>plus pure</i> que l'anneau : elle n'anime rien
    d'autre que les deux états que le Système nomme déjà (« le disque plein désigne un objectif »,
    l'évidé le même en train de se faire), et elle se passe de rotation. Son risque est réel et il ne
    se mesure pas : un clignotement se lit « alerte » dans la plupart des interfaces. À 9 px, en
    teal, à 1,2 s — <b>c'est à l'œil que ça se tranche</b>, et c'est pour ça qu'elle est ici animée à
    taille réelle dans une vraie barre d'état.</p>
    <p class="sub"><b>Deux sont disqualifiées, pas par goût :</b> le <b>battement</b> (6) gèle sur
    l'état de repos — A8 rompu ; le <b>balayage</b> (7) se lit comme une mesure — A52 rompu, et il
    ressuscite la barre fine qu'A36 avait retirée. <b>Trois portent une réserve</b> : le rabat (5)
    fait perdre son orientation à la seule forme orientée du jeu ; le carré (3) et les quatre coins
    (4) mettent un contenant là où le système a réservé le rond.</p>
    <p class="note">Page jetable, hors du Système. Une fois le choix fait, il rentre dans
    <code>socle.mjs</code> (la classe <code>.anneau</code>) et dans l'amendement V2 — nulle part
    ailleurs : la signature n'est dessinée qu'à un seul endroit.</p>
  </section>
</div>
<div class="pilules">
  <div class="pilule" id="pilule-theme" role="group" aria-label="Thème">
    <button data-th="elements" aria-pressed="true">Clair</button>
    <button data-th="elements-nuit" aria-pressed="false">Nuit</button>
  </div>
</div>
<script>
document.getElementById('pilule-theme').addEventListener('click', function (e) {
  var b = e.target.closest('button'); if (!b) return;
  Array.prototype.forEach.call(this.querySelectorAll('button'), function (x) {
    x.setAttribute('aria-pressed', String(x === b));
  });
  if (b.dataset.th === 'elements-nuit') document.documentElement.setAttribute('data-theme', 'elements-nuit');
  else document.documentElement.removeAttribute('data-theme');
});
</script>
</body></html>
`;

const sortie = path.join(import.meta.dirname, 'signatures.html');
writeFileSync(sortie, html, 'utf8');
console.log(`${sortie} — ${OPTIONS.length} propositions, ${(html.length / 1024).toFixed(0)} Ko`);
console.log(`marque/bg clair ${rapport(J.marque, J.bg).toFixed(2)}:1 · nuit ${rapport(THEMES['elements-nuit'].jetons.marque, THEMES['elements-nuit'].jetons.bg).toFixed(2)}:1 (seuil 3, composant)`);

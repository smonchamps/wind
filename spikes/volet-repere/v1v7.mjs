// ====================================================================
// LA FORME RETENUE, EN SITUATION — la fenêtre entière, jetable.
//
//   node spikes/volet-repere/v1v7.mjs
//
// « Expéditeur sur <glyphe> Libellé », après le verdict du Chef
// Ingénieur du 2026-08-24 sur la première mise en situation :
//
//   1. la phrase se LIT — elle évite d'avoir à se souvenir en
//      permanence d'une couleur ou d'un logo. Forme confirmée.
//   2. que la nav et la ligne disent la même chose n'est pas
//      choquant ; le GLYPHE, lui, doit être exactement le même.
//   3. le glyphe reste : il donne de la chaleur et une humanité
//      discrète ; couleur ET forme couvrent la majorité des goûts pour
//      une implémentation simple.
//   4. CHANGEMENT — la mécanique de repli (V7) est écartée. Le libellé
//      de boîte se TRONQUE à l'ellipse quand il s'approche de l'heure ;
//      c'est ce qui règle le problème des noms longs.
//   5. le même schéma se réplique derrière le nom de l'expéditeur au
//      VOLET DE LECTURE.
//
// La rangée et son ordre de troncature viennent de socle.mjs
// (`rangSur`, `CSS_SUR`), la fenêtre de fenetre.mjs : la planche
// comparative et cette mise en situation montrent LE MÊME objet.
// ====================================================================
import { writeFileSync } from 'node:fs';
import path from 'node:path';
import {
  RACINE, CLAIR, ico, COMPTES, FIL, rangSur, blocBoite,
  CSS_VOLET, CSS_SUR,
} from './socle.mjs';
import { fenetre, voletSeul, lecture, CSS_FENETRE } from './fenetre.mjs';

const ici = import.meta.dirname;

const rang = (l) => rangSur(l);

// --- Décision 4 : le décor du NOM LONG --------------------------------
// D4 accepte un nom de compte jusqu'à 60 caractères. Celui-ci en fait
// 32 — un nom parfaitement ordinaire, et le seul moyen honnête de
// montrer ce que fait la troncature.
const APEL = {
  nom: 'Association des parents d’élèves', court: 'APEL',
  icone: 'school', teinte: 'ocre', adresse: 'contact@apel-jean-moulin.fr',
};
const COMPTES_LONG = [COMPTES.travail, COMPTES.maison, APEL];
const rangLong = (l) => rangSur(l, l.c === 'etudes' ? APEL : COMPTES[l.c]);

// --- Décision 2 : le même glyphe, et maintenant le même objet ---------
// Le Chef Ingénieur a tranché le contenant : GLYPHE NU dans la nav. Les
// deux surfaces portent désormais le même tracé, à la même teinte, sans
// contenant — seule la taille suit son contexte (16 dans la nav, comme
// les dossiers ; 14 dans la ligne, comme son texte).
const echantillonGlyphe = (cpt) => `
<div class="echo">
  <div class="echo-nav">
    <span class="glyphe-compte" data-teinte="${cpt.teinte}">${ico(cpt.icone, 16)}</span>
    <span class="echo-lib">${cpt.nom}</span>
  </div>
  <div class="echo-fleche">le même objet →</div>
  <div class="echo-ligne">
    <span class="exp-echo">Camille Roux</span>${blocBoite(cpt)}
  </div>
</div>`;

// Le bloc « Boîtes » de la nav, avant et après — pour que ce qui a été
// donné et ce qui a été pris se voient d'un regard.
const blocBoites = (avant) => `
<div class="nav nav-echo">
  <p class="titre-nav">Boîtes</p>
  <div class="rang"><span class="icone">${ico('all_inbox')}</span>
    <span class="libelle">Toutes les boîtes</span></div>
  ${Object.values(COMPTES).map((c) => `
    <div class="rang">${avant
      ? `<span class="repere p20" data-teinte="${c.teinte}">${ico(c.icone, 12)}</span>`
      : `<span class="glyphe-compte" data-teinte="${c.teinte}">${ico(c.icone, 16)}</span>`}
      <span class="libelle">${c.nom}</span></div>`).join('')}
</div>`;

const FEUILLE = `
:root { color-scheme:light; }
* { box-sizing:border-box; }
body { margin:0; padding:0 0 72px;
  font-family:-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  background:${CLAIR.bg}; color:${CLAIR.ink}; }
.page { max-width:1360px; margin:0 auto; padding:0 40px; }
header.tete-page { padding:48px 0 20px; border-bottom:1px solid ${CLAIR.border}; }
h1.titre-page { font-family:'Segoe UI Variable Display', -apple-system, BlinkMacSystemFont,
  "Segoe UI", sans-serif; font-weight:340; letter-spacing:-.03em; font-size:40px; margin:0 0 12px; }
.sourcil { font-size:11px; letter-spacing:.1em; text-transform:uppercase;
  color:${CLAIR.muted}; font-weight:600; margin:0 0 6px; }
.sous-titre { margin:0 0 10px; font-size:15px; line-height:1.6; color:${CLAIR.ink2}; max-width:76ch; }
p { font-size:14px; line-height:1.6; color:${CLAIR.ink2}; max-width:78ch; margin:0 0 10px; }
ul, ol { font-size:14px; line-height:1.6; color:${CLAIR.ink2}; max-width:78ch; padding-left:20px; }
li { margin-bottom:8px; }
b { color:${CLAIR.ink}; font-weight:600; }
code { font-family:Consolas, monospace; font-size:12.5px; background:${CLAIR.surface};
  border:1px solid ${CLAIR.border}; padding:0 4px; }
a { color:${CLAIR.accent}; }
section.bloc { padding:32px 0; border-bottom:1px solid ${CLAIR.border}; }
h2 { font-family:'Segoe UI Variable Display', -apple-system, BlinkMacSystemFont,
  "Segoe UI", sans-serif; font-weight:340; letter-spacing:-.03em; font-size:24px; margin:0 0 4px; }
.legende { margin:0 0 14px; font-size:13px; color:${CLAIR.muted}; max-width:80ch; }
.legende.borne { color:${CLAIR.alert}; }
figure { margin:0; flex:none; }
figcaption { margin-top:8px; font-size:11px; letter-spacing:.08em; text-transform:uppercase;
  color:${CLAIR.muted}; font-weight:600; }
.duo { display:flex; gap:24px; flex-wrap:wrap; align-items:flex-start; }
.decisions { display:grid; gap:0; margin:14px 0 4px; max-width:80ch;
  border:1px solid ${CLAIR.border}; }
.decisions .d { display:grid; grid-template-columns:40px 1fr; gap:12px;
  padding:10px 14px; border-top:1px solid ${CLAIR.border}; font-size:14px; line-height:1.55;
  color:${CLAIR.ink2}; }
.decisions .d:first-child { border-top:none; }
.decisions .n { font-weight:600; color:${CLAIR.ink}; }
.decisions .d.chg { background:${CLAIR.surface}; }
table.banc { border-collapse:collapse; font-size:12.5px; margin:14px 0 8px; }
table.banc th, table.banc td { border:1px solid ${CLAIR.border}; padding:5px 10px; text-align:right; }
table.banc th[scope="row"] { text-align:left; font-weight:600; color:${CLAIR.ink}; }
table.banc thead th { font-size:11px; color:${CLAIR.muted}; text-transform:uppercase;
  letter-spacing:.06em; font-weight:600; }
table.banc td.coupe { color:${CLAIR.alert}; font-weight:600; }
table.banc td.ok { color:${CLAIR.accent}; font-weight:600; }

/* ================= LA FENÊTRE ================= */
${CSS_VOLET}
${CSS_FENETRE}
${CSS_SUR}

/* Le volet de lecture, montré seul (décision 5) */
.lecture-seule { width:631px; height:420px; border:1px solid var(--border);
  background:var(--bg); overflow:hidden; display:flex; }
.lecture-seule .lecture { flex:1; }

/* L'échantillon de la décision 2 : la nav et la ligne, côte à côte */
.echo { display:flex; align-items:center; gap:18px; flex-wrap:wrap;
  padding:14px 16px; border:1px solid var(--border); background:var(--bg); }
.echo-nav { display:flex; align-items:center; gap:10px; padding:8px 10px;
  border:1px solid transparent; }
.echo-lib { font-size:14px; color:var(--ink2); }
.echo-fleche { font-size:11px; letter-spacing:.1em; text-transform:uppercase;
  color:var(--muted); font-weight:600; }
.echo-ligne { display:flex; align-items:baseline; gap:6px; }
.exp-echo { font-size:14px; color:var(--ink); font-weight:700; }
.nav-echo { width:248px; padding:12px; border:1px solid var(--border);
  background:var(--bg); gap:2px; }
.nav-echo .rang { padding:8px 10px; }
`;

const html = `<!doctype html>
<html lang="fr"><head><meta charset="utf-8">
<title>La forme retenue, en situation</title>
<style>${FEUILLE}</style></head>
<body><div class="page">
<header class="tete-page">
  <p class="sourcil">Spike jetable · 2026-08-24 · rien ici n’est livré</p>
  <h1 class="titre-page">La forme retenue, en situation</h1>
  <p class="sous-titre">« <b>Camille Roux sur ▣ Travail</b> », mise à jour au verdict du Chef
  Ingénieur. Deux choses ont changé depuis la version précédente : la mécanique de repli est
  <b>écartée</b> au profit d’une troncature à l’ellipse, et le même schéma est <b>répliqué au
  volet de lecture</b>.</p>
  <div class="decisions">
    <div class="d"><span class="n">1</span><span>La phrase se lit — elle évite d’avoir à se
      souvenir en permanence d’une couleur ou d’un logo. <b>Forme confirmée.</b></span></div>
    <div class="d"><span class="n">2</span><span>La nav et la ligne peuvent dire la même chose ;
      le <b>glyphe doit être exactement le même</b>. Il l’est par construction — une seule source,
      le repère du compte (<code>reperes[account_id].icone</code>) : <code>Nav.svelte</code> et
      <code>Liste.svelte</code> lisent la même table, il n’y en a jamais eu deux. <b>Tranché
      depuis</b> : le contenant aussi — <b>glyphe nu dans la nav</b>, la pastille pleine quitte
      l’écran 02.</span></div>
    <div class="d"><span class="n">3</span><span>Le glyphe reste : il donne la chaleur et une
      humanité discrète. Couleur <b>et</b> forme couvrent la majorité des goûts pour une
      implémentation simple — c’est exactement ce que fait A74, et rien à ajouter.</span></div>
    <div class="d chg"><span class="n">4</span><span><b>Changement.</b> Le repli au seuil est
      écarté. Le libellé de boîte se <b>tronque à l’ellipse</b> quand il s’approche de l’heure.
      Trois règles : l’heure ne se coupe jamais ; le bloc boîte cède <b>trois fois plus vite</b>
      que l’expéditeur et ne prend jamais plus <b>du tiers</b> de la ligne ; les deux se terminent
      à l’ellipse, jamais à la coupe sèche. Le tiers est <b>mesuré</b> — voir le banc.</span></div>
    <div class="d chg"><span class="n">5</span><span><b>Ajout.</b> Le même schéma derrière le nom
      de l’expéditeur au <b>volet de lecture</b> — carte dépliée et rangées repliées.</span></div>
  </div>
  <p><b>Une correction que je me dois</b> : j’avais écrit que « D4 refuse de tronquer un nom de
  compte ». D4 borne la <b>saisie</b> — 60 caractères refusés, jamais tronqués à l’entrée — et ne
  dit rien de l’affichage. La décision 4 ne heurte donc aucune règle.</p>
  <p><a href="ligne-expediteur.html">← les sept versions de la ligne</a> ·
  <a href="planche.html">les sept dessins</a> ·
  <a href="organisation.html">les quinze organisations</a> ·
  <a href="o2.html">O2 en situation</a></p>
</header>

<section class="bloc" data-cas="400">
  <h2>Elements — clair · volet à 400 px, le défaut</h2>
  <p class="legende">Boîte unifiée, trois comptes, rangée « Photos du chantier de Vaise »
  choisie. Le volet de lecture dit désormais la boîte lui aussi (décision 5) — regardez la carte
  dépliée et la rangée repliée au-dessus d’elle.</p>
  ${fenetre('theme-clair', rang, { surLecture: true })}
</section>

<section class="bloc">
  <h2>Elements · nuit — volet à 400 px</h2>
  <p class="legende">Le même écran, la même minute. Le mot ne dépend pas de la polarité ; le
  glyphe suit le nuancier (V5).</p>
  ${fenetre('theme-nuit', rang, { surLecture: true })}
</section>

<section class="bloc" data-cas="long400">
  <h2>Décision 4 — le nom long, à 400 px</h2>
  <p class="legende">Le troisième compte s’appelle ici « <b>Association des parents d’élèves</b> »
  — 32 caractères, un nom parfaitement ordinaire (D4 en accepte 60). La ligne le tronque à
  l’ellipse dès qu’il s’approche de l’heure : le nom d’expéditeur garde sa place (aucun n’est
  coupé, mesuré), l’heure ne bouge pas, et la ligne ne déborde jamais. Le libellé entier reste
  à l’infobulle.</p>
  <p class="legende"><b>Constat en passant</b> : la <b>nav aussi</b> le tronque — sa rangée offre
  172 px au libellé pour 199 nécessaires (mesuré). C’est la règle d’aujourd’hui
  (<code>Nav.svelte</code>, <code>.libelle</code> en <code>text-overflow:ellipsis</code>), pas
  une conséquence de ce dessin ; mais elle dit qu’un nom long n’est <b>entier nulle part</b>
  dans l’écran 02. Si ça gêne, c’est la question du nom court qui revient — pas celle de la
  troncature.</p>
  ${fenetre('theme-clair', rangLong, { comptes: COMPTES_LONG, surLecture: true, boiteLecture: 'travail' })}
</section>

<section class="bloc" data-cas="long300">
  <h2>Le nom long à la borne basse — volet tiré à 300 px</h2>
  <p class="legende borne">La même fenêtre, la poignée tirée à fond (<code>BORNES.liste</code> =
  [300, 640]). Sans repli au seuil, c’est la troncature seule qui travaille : elle rogne d’abord
  le libellé de boîte, puis le nom d’expéditeur. <b>C’est ici que la décision 4 se juge</b> — le
  glyphe et la préposition restent, donc la ligne dit toujours « ceci vient d’une boîte », même
  quand elle ne peut plus la nommer. Les libellés <b>courts</b>, eux, restent entiers : c’est ce
  qui a fixé le plafond au tiers.</p>
  ${fenetre('theme-clair', rangLong, { liste: 300, comptes: COMPTES_LONG, surLecture: true })}
</section>

<section class="bloc" data-cas="640">
  <h2>L’autre borne — le volet seul à 640 px</h2>
  <p class="legende">Poignée poussée à fond. La ligne respire : nom entier, libellé entier, et
  l’aperçu gagne 240 px. Aucune troncature nulle part.</p>
  <div class="duo">
    <figure>${voletSeul('theme-clair', rangLong, { liste: 640, hauteur: 560 })}
      <figcaption>640 px — la borne haute</figcaption></figure>
    <figure>${voletSeul('theme-nuit', rangLong, { liste: 640, hauteur: 560 })}
      <figcaption>640 px — nuit</figcaption></figure>
  </div>
</section>

<section class="bloc">
  <h2>Décision 5 — le volet de lecture, de près</h2>
  <p class="legende">Le même bloc, derrière le nom de l’expéditeur : sur la rangée repliée et sur
  la carte dépliée. Il y garde sa graisse normale — c’est le nom qui porte l’autorité, la boîte
  n’est qu’une circonstance.</p>
  <div class="duo">
    <figure><div class="theme-clair lecture-seule">${lecture({ sur: true })}</div>
      <figcaption>Elements — clair</figcaption></figure>
    <figure><div class="theme-nuit lecture-seule">${lecture({ sur: true })}</div>
      <figcaption>Elements · nuit</figcaption></figure>
  </div>
  <p><b>Une observation, et je m’arrête là</b> : dans un fil, tous les messages viennent de la
  MÊME boîte — la mention se répète donc à l’identique d’une carte à l’autre. Elle reste juste,
  et elle vaut sûrement pour le premier coup d’œil ; si la répétition gêne au terrain, la porter
  une seule fois (sur la tête du fil, à côté du titre) donnerait la même information pour un
  seul énoncé. C’est un constat, pas une contre-proposition.</p>
</section>

<section class="bloc">
  <h2>Décision 2 — le même glyphe, et maintenant le même objet</h2>
  <p class="legende">Verdict : <b>glyphe nu dans la nav</b>. Les deux surfaces portent désormais
  le même tracé, à la même teinte, sans contenant ; seule la taille suit son contexte — 16 px
  dans la nav (celle des glyphes de dossier, juste au-dessus, dans la même colonne) et 14 px
  dans la ligne (celle de son texte). Le tracé, lui, vient de la même source : le nom d’icône du
  repère, dans <code>lib/icones.js</code>.</p>
  <div class="duo">
    <figure><div class="theme-clair">${echantillonGlyphe(COMPTES.travail)}</div>
      <figcaption>Travail — clair</figcaption></figure>
    <figure><div class="theme-nuit">${echantillonGlyphe(COMPTES.maison)}</div>
      <figcaption>Maison — nuit</figcaption></figure>
  </div>
  <h3 style="font-size:12px;letter-spacing:.06em;text-transform:uppercase;color:${CLAIR.muted};
    margin:22px 0 8px;font-weight:600">Ce que la nav gagne et ce qu’elle perd</h3>
  <div class="duo">
    <figure><div class="theme-clair">${blocBoites(true)}</div>
      <figcaption>Avant — la pastille pleine (20 px)</figcaption></figure>
    <figure><div class="theme-clair">${blocBoites(false)}</div>
      <figcaption>Après — le glyphe nu (16 px)</figcaption></figure>
    <figure><div class="theme-nuit">${blocBoites(false)}</div>
      <figcaption>Après — nuit</figcaption></figure>
  </div>
  <p><b>Gagné</b> : les rangées de comptes cessent d’être des rangées à part. Elles s’alignent
  sur les six dossiers du dessus — même taille de glyphe, même géométrie de rangée —, et la
  colonne entière se lit d’un seul rythme. Et le mot du volet retrouve <b>exactement</b> ce que
  la nav montre.</p>
  <p><b>Perdu</b> : le fond coloré donnait au repère une surface, donc une présence à distance ;
  un tracé de 2 unités à 16 px pèse 1,3 px. La nav dit maintenant le compte plus doucement — ce
  qui est cohérent avec le reste du dessin, mais qui se constate à la fenêtre réelle, pas ici.</p>
  <p><b>Conséquence de Système, à écrire</b> : la pastille disparaît de l’écran 02. Elle ne meurt
  pas — elle reste aux <b>Réglages</b> (<code>Reglages.svelte:460</code> et le nuancier de choix),
  où elle est une pastille de <b>choix</b> et non une marque d’identité. Ce qui tombe, c’est la
  phrase de V4/V14 : « reste un seul autre rond dans tout le système, la pastille de repère ».
  Dans l’écran de tous les jours, le disque ne dit plus <b>que</b> l’état — ce qu’O2 promettait,
  obtenu par un autre chemin. Les contrastes, eux, ne bougent pas : la teinte tracée sur
  <code>bg</code>, <code>hover</code>, <code>sel</code> et <code>tuile</code> est déjà mesurée au
  seuil composant (pire cas du nuancier : <b>4,97:1</b>) ; les 24 mesures « glyphe sur pastille »
  perdent leur objet dans la nav et le gardent aux Réglages.</p>
</section>

<section class="bloc">
  <h2>Le banc — ce que la troncature fait, aux trois largeurs</h2>
  <p class="legende">Mesuré sur cette page à l’ouverture, sur les rangées réellement rendues.
  « Libellé » dit si le nom de boîte est entier ou coupé à l’ellipse ; « noms coupés » compte les
  expéditeurs tronqués.</p>
  <div id="banc-cible"></div>
  <h2 style="font-size:19px;font-weight:600;font-family:inherit;letter-spacing:0;margin:22px 0 4px">
    Pourquoi le tiers — six plafonds essayés</h2>
  <p>Le plafond du bloc boîte n’est pas un chiffre choisi, c’est le résultat d’un essai sur le
  nom de 32 caractères ci-dessus, aux trois largeurs. À <b>la moitié</b>, le bloc prend 183 px au
  défaut et <b>deux noms d’expéditeur se coupent</b> — la circonstance mange le sujet. À
  <b>30 %</b>, ce sont les libellés <b>courts</b> qui se coupent pour rien à la borne basse
  (<b>7 sur 16</b> : « Travail » tronqué alors qu’il tenait). Le <b>tiers</b> est à la fois le
  plus serré qui ne coupe jamais un libellé court, et le plus large qui n’entame jamais
  l’expéditeur au défaut. 33, 34, 35 et 36 % donnent le même résultat : c’est un plateau, pas
  une valeur de justesse — d’où le tiers, qui se dit en un mot.</p>
  <p><b>Une conséquence à assumer</b> : avec ce plafond, un nom de 32 caractères reste tronqué
  <b>même à 640 px</b> (le bloc y serait plafonné à 200 px pour 226 nécessaires). La boîte est
  une circonstance : elle ne prend pas le tiers d’une rangée quelle que soit la largeur. Le nom
  entier reste à l’infobulle, et la nav l’affiche complet.</p>
  <h2 style="font-size:19px;font-weight:600;font-family:inherit;letter-spacing:0;margin:22px 0 4px">
    Trouvé en chemin — et qui ne vient pas de ce dessin</h2>
  <p>À 300 px, le <b>pied de la liste déborde</b> : les trois onglets « Tous / Non lus /
  Brouillons » demandent <span id="mesure-onglets">…</span> dans une colonne qui en offre 299.
  Ce n’est <b>pas</b> un effet de cette forme — le pied ne dépend pas de la rangée et porte les
  mêmes trois onglets aujourd’hui, à la même grammaire (<code>Liste.svelte</code> :
  <code>.onglets</code>, <code>gap:10px</code>, <code>padding:0 12px</code>). Autrement dit
  <b>le produit livré déborde déjà à sa propre borne basse</b> ; la maquette ne fait que le
  rendre visible parce qu’elle clippe. <b>À vérifier à la fenêtre réelle</b>, poignée tirée à
  fond : si le constat tient, c’est un terrain à part entière.</p>
</section>
</div>
<script>
(() => {
  const GAP = 6;
  const mesure = (racine) => {
    const col = racine.querySelector('.colonne');
    if (!col) return null;
    const l1s = [...col.querySelectorAll('.l1')];
    if (!l1s.length) return null;
    const dispo = l1s.map((l) => {
      const pris = [...l.children]
        .filter((e) => !e.classList.contains('exp') && !e.classList.contains('essor'))
        .reduce((n, e) => n + e.getBoundingClientRect().width, 0);
      return l.clientWidth - pris - GAP * ([...l.children].length - 1);
    });
    const exps = l1s.map((l) => l.querySelector('.exp'));
    const libs = l1s.map((l) => l.querySelector('.lib'));
    const boites = l1s.map((l) => l.querySelector('.boite').getBoundingClientRect().width);
    const libCoupes = libs.filter((e) => e.scrollWidth > e.clientWidth + 1).length;
    return {
      boite: Math.round(Math.max(...boites)),
      libCoupes,
      libTotal: libs.length,
      dispo: Math.round(Math.min(...dispo)),
      coupes: exps.filter((e) => e.scrollWidth > e.clientWidth + 1).length,
      total: exps.length,
    };
  };
  const cas = [
    ['400 px — noms courts', '[data-cas="400"]'],
    ['400 px — nom long', '[data-cas="long400"]'],
    ['300 px — nom long, borne basse', '[data-cas="long300"]'],
    ['640 px — nom long, borne haute', '[data-cas="640"]'],
  ].map(([nom, sel]) => {
    const racine = document.querySelector(sel);
    return { nom, m: racine ? mesure(racine) : null };
  }).filter((r) => r.m);
  document.getElementById('banc-cible').innerHTML =
    '<table class="banc"><thead><tr><th>cas</th><th>bloc boîte</th><th>libellé</th>'
    + '<th>place au nom</th><th>noms coupés</th></tr></thead><tbody>'
    + cas.map((r) => {
      const lib = r.m.libCoupes === 0
        ? '<td class="ok">entier</td>'
        : '<td class="coupe">tronqué · ' + r.m.libCoupes + ' / ' + r.m.libTotal + '</td>';
      return '<tr><th scope="row">' + r.nom + '</th>'
        + '<td>' + r.m.boite + ' px</td>' + lib
        + '<td>' + r.m.dispo + ' px</td>'
        + '<td class="' + (r.m.coupes > 0 ? 'coupe' : 'ok') + '">'
        + r.m.coupes + ' / ' + r.m.total + '</td></tr>';
    }).join('')
    + '</tbody></table>';

  // Le pied à la borne basse — constat de passage, avec son chiffre.
  const pied = document.querySelector('[data-cas="long300"] .onglets');
  const cible = document.getElementById('mesure-onglets');
  if (pied && cible) {
    const l = [...pied.children].reduce((n, e) => n + e.getBoundingClientRect().width, 0)
      + 10 * (pied.children.length - 1) + 24;
    cible.textContent = Math.round(l) + ' px';
  }
})();
</script>
</body></html>`;

const sortie = path.join(ici, 'v1v7.html');
writeFileSync(sortie, html, 'utf8');
console.log(`Écrit : ${path.relative(RACINE, sortie)}`);
console.log(`  décisions 1-5 appliquées ; nom long : « ${APEL.nom} » (${APEL.nom.length} caractères)`);
console.log(`  4 cas mesurés : 400 court, 400 long, 300 long, 640 long`);

// ====================================================================
// La fenêtre de Wind, en maquette — la matière partagée des mises en
// situation (o2.html, v1v7.html).
//
// Entête 52 px, colonnes nav / liste / lecture, barre d'état 36 px. Les
// cotes sont celles du produit : nav 248 et liste 400 par défaut
// (lib/largeurs.svelte.js), bornes de la liste [300, 640].
//
// Le volet de lecture est SCHÉMATIQUE — assez fidèle pour donner
// l'échelle et tirer l'œil, pas une proposition. Il garde la tuile aux
// initiales : dans un fil, l'expéditeur change d'un message à l'autre
// (arbitrage D-c de planche.html).
//
// Deux copies de cette fenêtre divergeraient en silence : il n'y en a
// qu'une, et elle vit ici.
// ====================================================================
import { ico, initiales, COMPTES, FIL, disque, classes, blocBoite } from './socle.mjs';

// La marque en GLYPHE (V1/V11) : l'enveloppe suit l'encre courante, le
// rabat prend --marque. Verbatim de lib/icones.js.
const MARQUE_D = 'M4 8h16v9H4z';
const MARQUE_FLAP = 'M8.75 9.15A3.25 3.25 0 0 0 15.25 9.15Z';
export const marque = (taille = 20) =>
  `<svg class="ic" viewBox="0 0 24 24" width="${taille}" height="${taille}" aria-hidden="true">`
  + `<g fill="none" stroke="currentColor" stroke-width="2.3" stroke-linecap="butt"`
  + ` stroke-linejoin="miter"><path d="${MARQUE_D}"/></g>`
  + `<path d="${MARQUE_FLAP}" fill="var(--marque)"/></svg>`;

// --- La navigation (Nav.svelte : 248 px, six dossiers, puis Boîtes) ---
const DOSSIERS = [
  ['inbox', 'Boîte de réception', 12, true],
  ['send', 'Envoyés', 0, false],
  ['edit_note', 'Brouillons', 0, false],
  ['report', 'Indésirables', 3, false],
  ['inventory_2', 'Archives', 0, false],
  ['delete', 'Corbeille', 0, false],
];
// Décision du Chef Ingénieur (2026-08-24) : le GLYPHE NU dans la nav.
// La pastille pleine de 20 px disparaît de l'écran 02 ; les deux
// surfaces portent désormais le même objet — le tracé du repère, à la
// teinte du compte, sans contenant. Taille 16 : celle des glyphes de
// dossier juste au-dessus, dans la même colonne — les comptes cessent
// d'être des rangées à part. La pastille ne meurt pas pour autant :
// elle reste aux Réglages (Reglages.svelte:460 et le nuancier de
// choix), où elle est une PASTILLE DE CHOIX et non une marque
// d'identité.
const glypheNav = (cpt) =>
  `<span class="glyphe-compte" data-teinte="${cpt.teinte}" title="${cpt.nom} — ${cpt.adresse}">`
  + `${ico(cpt.icone, 16)}</span>`;

export const nav = (comptes = Object.values(COMPTES)) => `
<nav class="nav">
  ${DOSSIERS.map(([g, lib, n, actif]) => `
    <div class="rang${actif ? ' actif' : ''}">
      <span class="icone">${ico(g)}</span><span class="libelle">${lib}</span>
      ${n > 0 ? `<span class="pastille-nav">${n}</span>` : ''}
    </div>`).join('')}
  <div class="boites">
    <p class="titre-nav">Boîtes</p>
    <div class="tuile-nav">
      <span class="icone-tuile">${ico('all_inbox')}</span>
      <span class="titre-tuile">Toutes les boîtes</span>
    </div>
    ${comptes.map((c) => `
      <div class="rang">${glypheNav(c)}<span class="libelle">${c.nom}</span></div>`).join('')}
  </div>
</nav>`;

// --- Le volet central (Liste.svelte : bandeau, flot, onglets) ---------
export const volet = (rangee, rangs = FIL) => `
<section class="colonne">
  <header class="bandeau"><h1>Boîte de réception</h1></header>
  <div class="cadre-liste"><div class="liste">${rangs.map(rangee).join('')}</div></div>
  <div class="onglets">
    <span class="onglet actif">${ico('inbox')}Tous</span>
    <span class="onglet">${ico('mark_email_unread')}Non lus</span>
    <span class="onglet">${ico('edit_note')}Brouillons</span>
  </div>
</section>`;

// --- Le volet de lecture (Fil.svelte — schématique, aux jetons) -------
// Décision 5 (2026-08-24) : le même schéma se réplique derrière le nom
// de l'expéditeur au volet de lecture — sur la carte dépliée ET sur les
// rangées repliées, pour que la ligne soit la même partout. `sur` reste
// optionnel : la mise en situation d'O2 ne dit pas la boîte en mots.
export const lecture = ({ sur = false, boite = 'travail' } = {}) => {
  const b = sur ? blocBoite(COMPTES[boite]) : '';
  return `
<section class="lecture">
  <div class="tete-fil">
    <div class="gestes">
      <button class="nu">${ico('archive')}Archiver</button>
      <button class="nu">${ico('delete')}Supprimer</button>
      <button class="nu">${ico('report')}Signaler comme spam</button>
      <span class="essor"></span>
      <button class="nu">${ico('keep')}Épingler</button>
    </div>
    <h3 class="titre display">Photos du chantier de Vaise</h3>
    <div class="barre-fil">
      <span class="puce">${ico('forum', 14)}3 messages</span>
      <span class="essor"></span>
      <button class="nu">${ico('open_in_full')}Ouvrir</button>
      <button class="nu">${ico('unfold_more')}Tout déplier</button>
    </div>
  </div>
  <div class="fil-corps">
    <div class="replie">
      <span class="avatar petit">${initiales('Paul Mercier')}</span>
      <span class="replie-exp">Paul Mercier</span>${b}
      <span class="replie-ap">Parfait, je regarde ça ce soir.</span>
      <span class="quand">3 août, 11 h 02</span>
    </div>
    <article class="deplie">
      <div class="tete-message">
        <span class="avatar">${initiales('Thomas Petit')}</span>
        <span class="qui">
          <span class="auteur">Thomas Petit${b}</span>
          <span class="adr">thomas.petit@atelier-nord.fr · à Paul Mercier</span>
        </span>
        <span class="quand">3 août, 08 h 30</span>
      </div>
      <div class="contenu">
        <p class="txt">Bonjour Paul,</p>
        <p class="txt">J’ai mis les prises de vue dans le dossier partagé — la série du matin
        est nettement plus lisible que celle de mercredi, la lumière rasante ne pardonne rien
        sur les enduits.</p>
        <p class="txt">Dis-moi si tu veux que je repasse jeudi pour les façades nord ; sinon je
        garde le créneau pour le rapport.</p>
        <p class="txt">Thomas</p>
        <div class="pieces">
          <p class="titre-fichiers">Fichiers</p>
          <span class="puce">${ico('description', 14)}vaise-facade-nord.pdf<span class="poids">2,4 Mo</span></span>
        </div>
        <div class="reponses">
          <button class="nu">${ico('reply')}Répondre</button>
          <button class="nu">${ico('reply_all')}Répondre à tous</button>
          <button class="nu">${ico('reply', 16, 'miroir')}Transférer</button>
        </div>
      </div>
    </article>
  </div>
</section>`;
};

// --- La fenêtre entière ------------------------------------------------
// `liste` : la largeur du volet central, en px. Le produit la règle à la
// poignée entre 300 et 640 ; la maquette la pose, pour montrer ce que
// devient le dessin aux deux bouts.
export const fenetre = (theme, rangee, {
  liste = 400, rangs = FIL, largeur = 1280,
  comptes = Object.values(COMPTES), surLecture = false, boiteLecture = 'travail',
} = {}) => `
<div class="fenetre ${theme}" style="width:${largeur}px; --l-liste:${liste}px">
  <header class="entete">
    <span class="marque-titre">${marque(20)}Wind</span>
    <span class="recherche">${ico('search')}<span class="ph">Chercher dans le courrier</span></span>
    <button class="principal">${ico('edit_square')}Écrire</button>
    <button>${ico('settings')}Réglages</button>
  </header>
  <div class="colonnes">${nav(comptes)}${volet(rangee, rangs)}`
  + `${lecture({ sur: surLecture, boite: boiteLecture })}</div>
  <div class="statut">
    <span class="texte"><span class="disque"></span>À jour — dernière relève il y a 2 min</span>
    <span class="essor"></span>
    <button class="btn-statut">${ico('sync')}Synchroniser</button>
  </div>
</div>`;

// Le volet central SEUL, à la largeur voulue — pour montrer une borne
// sans re-rendre une fenêtre entière.
export const voletSeul = (theme, rangee, { liste = 400, rangs = FIL, hauteur = 470 } = {}) =>
  `<div class="${theme} volet-seul" style="width:${liste}px; height:${hauteur}px">`
  + volet(rangee, rangs) + `</div>`;

// --- Le dessin de la fenêtre ------------------------------------------
export const CSS_FENETRE = `
.fenetre { height:860px; display:flex; flex-direction:column;
  background:var(--bg); color:var(--ink); border:1px solid var(--border); overflow:hidden; }
.entete { height:52px; flex:none; background:var(--bg); border-bottom:1px solid var(--border);
  display:flex; align-items:center; gap:12px; padding:0 14px; }
.marque-titre { font-size:18px; font-weight:600; width:212px; color:var(--ink);
  display:flex; align-items:center; gap:10px; }
.recherche { flex:1; max-width:520px; height:32px; display:flex; align-items:center; gap:10px;
  padding:0 14px; font-size:13px; color:var(--ink2);
  background:var(--surface); border:1px solid var(--border); }
.recherche .ph { color:var(--ink2); }
.fenetre button, .volet-seul button {
  height:32px; padding:0 16px; display:inline-flex; align-items:center; gap:8px;
  font-size:13px; color:var(--ink); background:var(--surface);
  border:1px solid var(--border); font-family:inherit; }
.fenetre button.principal {
  font-weight:600; color:var(--onAccent); background:var(--accent); border-color:var(--accent);
  margin-left:auto; }
.colonnes { flex:1; display:grid;
  grid-template-columns:248px var(--l-liste, 400px) minmax(0,1fr); min-height:0; }

/* Nav (A29, V4/V14) */
.nav { background:var(--bg); border-right:1px solid var(--border);
  padding:20px 12px; display:flex; flex-direction:column; gap:2px; min-height:0; }
.nav .rang { display:flex; align-items:center; gap:10px; flex:none;
  padding:8px 10px; border:1px solid transparent; }
.nav .rang.actif { background:var(--sel); border-color:var(--accent); }
.nav .icone { color:var(--muted); display:inline-flex; }
.nav .actif .icone { color:var(--accent); }
.nav .libelle { font-size:14px; color:var(--ink2); flex:1; min-width:0;
  overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
.nav .actif .libelle { font-weight:600; color:var(--ink); }
.pastille-nav { flex:none; font-size:12px; font-weight:600; color:var(--accent);
  font-variant-numeric:tabular-nums; }
.boites { margin-top:auto; padding-top:16px; border-top:1px solid var(--border);
  display:flex; flex-direction:column; gap:6px; }
.titre-nav { margin:0 0 4px; padding:0 10px; font-size:11px; letter-spacing:.1em;
  text-transform:uppercase; color:var(--muted); font-weight:600; }
.tuile-nav { display:flex; align-items:center; gap:10px; flex:none; padding:9px 12px;
  background:var(--tuile); color:var(--tuileInk); border:1px solid var(--border); }
.icone-tuile { color:var(--tuileInk); display:inline-flex; }
.titre-tuile { font-size:13px; font-weight:600; }

/* Volet central (Liste.svelte) */
.colonne { display:flex; flex-direction:column; min-height:0;
  background:var(--bg); border-right:1px solid var(--border); }
.volet-seul { border:1px solid var(--border); overflow:hidden; display:flex; }
.volet-seul .colonne { flex:1; border-right:none; }
.bandeau { flex:none; height:52px; display:flex; align-items:center; padding:0 16px;
  background:var(--bg); border-bottom:1px solid var(--border); }
.bandeau h1 { margin:0; font-size:16px; font-weight:600; line-height:1.3; color:var(--ink);
  overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
.cadre-liste { flex:1; min-height:0; overflow:hidden; }
.onglets { flex:none; height:52px; padding:0 12px; display:flex; align-items:center; gap:10px;
  border-top:1px solid var(--border); background:var(--bg); }
.onglet { height:32px; padding:0 14px; display:inline-flex; align-items:center; gap:8px;
  font-size:13px; color:var(--ink2); background:var(--surface); border:1px solid var(--border);
  white-space:nowrap; }
.onglet.actif { font-weight:600; color:var(--ink); background:var(--sel); border-color:var(--accent); }

/* Volet de lecture (Fil.svelte — schématique, aux jetons) */
.lecture { min-width:0; display:flex; flex-direction:column; background:var(--bg); overflow:hidden; }
.tete-fil { flex:none; padding:14px 20px 12px; border-bottom:1px solid var(--border); }
.gestes, .barre-fil, .reponses { display:flex; align-items:center; gap:8px; }
.essor { flex:1; }
.fenetre button.nu { background:transparent; border-color:transparent; color:var(--ink2);
  padding:0 10px; }
.titre { margin:10px 0 8px; font-size:24px; line-height:1.2; color:var(--ink);
  overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
.display { font-family:'Segoe UI Variable Display', -apple-system, BlinkMacSystemFont,
  "Segoe UI", sans-serif; font-weight:340; letter-spacing:-.03em; }
.fil-corps { flex:1; min-height:0; overflow:hidden; padding:16px 20px 20px; }
.replie { display:flex; align-items:center; gap:10px; padding:12px 20px;
  background:var(--surface); border:1px solid var(--border); font-size:13px; }
.replie-exp { font-weight:600; color:var(--ink); flex:none; }
.replie-ap { color:var(--ink2); min-width:0; overflow:hidden;
  text-overflow:ellipsis; white-space:nowrap; }
.quand { margin-left:auto; color:var(--muted); font-size:12px; flex:none; white-space:nowrap; }
.avatar.petit { width:26px; height:26px; grid-row:auto; }
.deplie { background:var(--surface); border:1px solid var(--border);
  box-shadow:var(--shadow); margin-top:12px; display:flex; flex-direction:column; }
.tete-message { display:flex; align-items:center; gap:10px; padding:12px 20px;
  border-bottom:1px solid var(--border); }
.tete-message .avatar { grid-row:auto; }
.qui { min-width:0; display:flex; flex-direction:column; }
.auteur { font-size:15px; font-weight:600; color:var(--ink);
  display:flex; align-items:baseline; gap:6px; min-width:0; }
/* Décision 5 : le bloc boîte de la ligne, au volet de lecture. Il y
   garde sa graisse normale — c'est le NOM qui porte l'autorité. */
.auteur .boite, .replie .boite { font-weight:400; flex:0 3 auto; }
.replie .boite { margin-right:2px; }
.adr { font-size:12px; color:var(--muted); }
.contenu { padding:14px 20px 18px; display:flex; flex-direction:column; gap:12px; }
.txt { margin:0; font-size:14px; line-height:1.6; color:var(--ink); }
.titre-fichiers { margin:0 0 8px; font-size:12px; font-weight:600; letter-spacing:.1em;
  text-transform:uppercase; color:var(--muted); }
.poids { color:var(--muted); margin-left:6px; }
.reponses { padding-top:4px; }

/* Barre d'état (V2 : le disque plein dit le repos) */
.statut { height:36px; flex:none; background:var(--bg); border-top:1px solid var(--border);
  display:flex; align-items:center; gap:14px; padding:0 24px; font-size:12px; color:var(--muted); }
.statut .texte { display:flex; align-items:center; gap:10px; }
.btn-statut { height:26px; padding:0 12px; display:inline-flex; align-items:center; gap:7px;
  font-size:12px; font-weight:600; color:var(--ink2); background:var(--surface);
  border:1px solid var(--border); font-family:inherit; }
.btn-statut .ic { width:14px; height:14px; }
.ic.miroir { transform:scaleX(-1); }
`;

export { disque, classes };

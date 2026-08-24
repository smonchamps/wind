// ====================================================================
// Système v2 « Elements » — parties 10 à 13 :
// Icônes (le jeu complet), Écran 01 (accueil et guichet),
// Écran 02 (boîte de réception), Barre d'état et synchronisation.
// ====================================================================
import { JEU, RESERVES, REPERES } from '../jeu.mjs';
import { ico, icoMiroir, marque, marqueTuile, esc } from './socle.mjs';
import { haut } from './parties-1.mjs';

// --- Le relevé : un glyphe, un libellé, un emploi ---------------------
// Repris du Système livré, complété des dix glyphes qu'il ne dessinait
// nulle part (V8) — l'écart trouvé en croisant le document et la fonte.
// `faire.mjs` VÉRIFIE que ce relevé couvre le catalogue dans les deux
// sens : c'est A18 rendu mécanique, et non plus promis.
export const RELEVE = [
  ['search', 'Chercher un message, une personne, un fichier', 'Champ de recherche, en-tête'],
  ['edit_square', 'Écrire', 'Action principale, en-tête'],
  ['settings', 'Réglages', 'Accès secondaire, en-tête'],
  ['menu', 'Ouvrir la navigation', 'Bouton du tiroir, en-tête — mode un volet (absent du relevé livré, V8)'],
  ['inbox', "Boîte de réception · Tous · Ce n'est pas un spam", 'Dossier (nav), onglet de filtre, et le geste inverse du signalement'],
  ['send', 'Envoyés · Envoyer', "Dossier (nav) et action d'envoi"],
  ['edit_note', 'Brouillons', 'Dossier (nav), onglet de filtre, mention du brouillon de fil'],
  ['drafts', 'Enregistrer le brouillon', 'Geste du composeur'],
  ['report', 'Indésirables · Signaler comme spam', 'Dossier (nav) et geste de tri du fil'],
  ['archive', 'Archives · Archiver', 'Dossier (nav) et action'],
  ['inventory_2', 'Archives (dossier)', "Le dossier, quand l'action archive est présente au même écran — sinon fusionne avec archive"],
  ['delete', 'Corbeille · Supprimer', 'Dossier (nav), action, et « Supprimer le brouillon » en alerte'],
  ['all_inbox', 'Toutes les boîtes', 'Boîte agrégée (nav)'],
  ['person', 'paul@atelier-nord.fr', "Boîte de compte et tuile de la boîte en cours — le DÉFAUT d'un compte sans repère ; un repère choisi le remplace par sa pastille (A74)"],
  ['home', 'Maison', 'Repère de compte (A74) — jeu dédié, réservé'],
  ['work', 'Travail', 'Repère de compte (A74)'],
  ['school', 'Études', 'Repère de compte (A74)'],
  ['star', 'Étoile', 'Repère de compte (A74)'],
  ['favorite', 'Cœur', 'Repère de compte (A74)'],
  ['flight', 'Voyage', 'Repère de compte (A74)'],
  ['shopping_bag', 'Achats', 'Repère de compte (A74)'],
  ['account_balance', 'Banque', 'Repère de compte (A74)'],
  ['sports_esports', 'Jeux', 'Repère de compte (A74)'],
  ['eco', 'Nature', 'Repère de compte (A74)'],
  ['pets', 'Animaux', 'Repère de compte (A74)'],
  ['music_note', 'Musique', 'Repère de compte (A74)'],
  ['mark_email_unread', 'Non lus', 'Onglet de filtre, pied de liste — le seul glyphe du jeu qui porte un disque teal'],
  ['forum', '3 messages', 'Puce conversation (nombre de messages)'],
  ['attach_file', '2 fichiers · Joindre', 'Puce pièces jointes et action de la composition'],
  ['description', 'Contrat_Vantis_v4.pdf', 'Puce fichier (nom du document, et son poids depuis A59)'],
  ['storage', 'Réservé (A60)', 'La puce de poids autonome, retirée — glyphe conservé au sous-ensemble'],
  ['unfold_more', 'Tout déplier', 'Tout déplier le fil (bascule, A46/A47)'],
  ['unfold_less', 'Tout replier', 'Tout replier le fil (bascule, A46/A47)'],
  ['open_in_full', 'Ouvrir', "Le volet vers l'écran 03 (A46)"],
  ['download', 'Enregistrer', "Voile de survol d'une puce de pièce jointe en lecture (A70)"],
  ['system_update_alt', 'Mise à jour prête', "Avis de mise à jour (absent du relevé livré, V8)"],
  ['keep', 'Épingler · Épinglé', 'Barre du fil (Réception) et marque de la ligne épinglée (A73)'],
  ['keep_off', 'Désépingler', "La bascule inverse d'« Épingler » (A73)"],
  ['bookmark', 'Thèmes', 'Rangée du rail des Réglages'],
  ['reply', 'Répondre', 'Action de message (barre du message)'],
  ['reply_all', 'Répondre à tous', 'Action de message ; la double flèche le distingue de « Répondre » (A14)'],
  ['reply', 'Transférer', 'La flèche de « Répondre » en symétrie verticale (classe .miroir), jamais forward (A12)', true],
  ['open_in_new', 'Réservé (A53)', 'Fenêtre de composition détachée — glyphe conservé au sous-ensemble'],
  ['close', '(sans libellé)', "Fermer la composition, retirer une pièce — jamais « refuser » (A76)"],
  ['check_circle', 'Accepter · Acceptée', "Réponse à une invitation (A76), glyphe en accent ; la coche des toasts et de l'accueil"],
  ['cancel', 'Refuser · Refusée', 'Refuser une invitation (A76), glyphe en alerte — le pendant rond de check_circle, jamais close'],
  ['question_mark', 'Provisoire', 'La réponse provisoire à une invitation (A76), glyphe en neutre'],
  ['error', 'Incident', "Rapport d'incident dans la fente d'avis (absent du relevé livré, V8)"],
  ['info', 'À propos', 'Rangée du rail des Réglages'],
  ['warning', 'Refus au plafond', "Le message d'alerte sous la rangée de pièces jointes"],
  ['hourglass_empty', 'Rapatriement…', 'Pièce en cours de rapatriement au transfert ; envoi programmé en attente'],
  ['arrow_back', 'Boîte de réception · Revenir à cette étape', "Le SEUL sens « revenir » (A75)"],
  ['group_add', 'Cc', 'Ajouter une adresse en copie'],
  ['visibility_off', 'Cci', 'Ajouter une copie cachée'],
  ['person_add', 'Ajouter un compte', 'Le guichet déplié des Réglages (absent du relevé livré, V8)'],
  ['notifications', 'Notifications', 'Rangée du rail des Réglages'],
  ['display_settings', 'Affichage', 'Rangée du rail des Réglages'],
  ['keyboard', 'Raccourcis', 'Rangée du rail des Réglages'],
  ['signature', 'Signature · Enregistrer', 'Groupe Signature des Réglages (A68)'],
  ['priority_high', 'Important', 'Bascule « important » du composeur (A67)'],
  ['schedule_send', 'Envoyer plus tard · Programmer', "Envoi différé : geste du composeur et avis d'un envoi programmé (A69)"],
  ['sync', 'Synchroniser · Réessayer', "Bouton de relève de la barre d'état (S-D1, A16)"],
  ['link_off', 'Images distantes bloquées', "La garde d'images, par message (absent du relevé livré, V8)"],
  ['link', 'Réservé (A62)', '« Lien » retiré de la barre de mise en forme — glyphe conservé'],
  ['format_quote', 'Réservé (A62)', '« Citation » retirée de la barre — glyphe conservé'],
  ['format_bold', 'Gras', 'Mise en forme du corps (composition, A62)'],
  ['format_italic', 'Italique', 'Mise en forme du corps (composition, A62)'],
  ['format_underlined', 'Souligné', 'Mise en forme du corps (composition, A62)'],
  ['strikethrough_s', 'Barré', 'Mise en forme du corps (composition, A62)'],
  ['format_color_text', 'Couleur du texte', 'Ouvre le nuancier fixe de 12 teintes (A62-D3) — sa barre basse est le SEUL élément coloré du jeu'],
  ['format_align_left', 'Aligner à gauche', 'Alignement du corps (composition, A62)'],
  ['format_align_center', 'Centrer', 'Alignement du corps (composition, A62)'],
  ['format_align_right', 'Aligner à droite', 'Alignement du corps (composition, A62)'],
  ['format_list_bulleted', 'Liste', 'Mise en forme du corps (A62) — puces CARRÉES : des puces rondes seraient trois disques'],
  ['format_list_numbered', 'Liste numérotée', 'Mise en forme du corps (composition, A62)'],
  ['format_indent_decrease', 'Diminuer le retrait', 'Retrait du corps (composition, A62)'],
  ['format_indent_increase', 'Augmenter le retrait', 'Retrait du corps (composition, A62)'],
  ['format_clear', 'Effacer la mise en forme', 'Retire toute mise en forme de la sélection (composition, A62)'],
  ['volunteer_activism', 'Consentement télémétrie', "Avis de consentement (absent du relevé livré, V8)"],
];

// --------------------------------------------------------------------
export const icones = () => {
  const noms = Object.keys(JEU).sort();
  const parClasse = (c) => noms.filter((n) => JEU[n].c === c);
  const fusions = {};
  for (const [n, g] of Object.entries(JEU)) if (g.f) (fusions[g.f] ||= []).push(n);
  const arcs = noms.filter((n) => JEU[n].arc);
  const ECHELLE = ['inbox', 'reply', 'settings', 'format_list_numbered', 'star', 'hourglass_empty'];
  const TAILLES = [10, 12, 14, 16, 18, 24, 48];

  const cellule = (n) => {
    const g = JEU[n];
    return `<figure class="cell c-${g.c}${g.r ? ' reserve' : ''}">
      <span class="gl">${ico(n, 16)}</span><figcaption>${esc(n)}</figcaption></figure>`;
  };

  return `
  <section id="icones">
    ${haut()}
    <h2>Icônes</h2>
    <p class="sub">Le jeu entier est <b>dessiné</b>, pas composé : ${noms.length} glyphes en SVG en
    ligne, grille de 24, trait de 2 unités, bouts nets, jonctions vives, coordonnées entières. La
    police Material Symbols et son sous-ensemble vendorisé <b>disparaissent</b> (V8) — avec eux
    disparaît la dernière dépendance de fonte du produit, et l'écart de dix glyphes que le Système
    livré traînait sans le voir : six glyphes étaient <b>livrés sans être dessinés nulle part</b>
    (<code>error</code>, <code>link_off</code>, <code>menu</code>, <code>person_add</code>,
    <code>system_update_alt</code>, <code>volunteer_activism</code>) et quatre étaient
    <b>dessinés sans être employés</b>. Ici tout est dessiné, et le <b>générateur le vérifie</b> :
    il refuse de produire ce document si le relevé et le catalogue divergent d'un seul glyphe, dans
    un sens ou dans l'autre. A18 n'est plus une promesse, c'est une assertion.</p>

    <p class="etiq">Le jeu complet, à la taille d'emploi (16 px) — rien n'est agrandi</p>
    <div class="grille">${noms.map(cellule).join('')}</div>
    <div class="legende">
      <span><i class="cle" style="background:var(--surface)"></i>
        <b>direct</b> — nom simple, fond de surface : la grammaire suffit, aucun arbitrage
        (${parClasse('direct').length})</span>
      <span><i class="cle" style="background:var(--surface)"></i>
        <b><span style="text-decoration:underline dotted;text-underline-offset:3px">arbitrage</span></b>
        — nom souligné de pointillé : il a fallu décider une courbe, une diagonale, une réduction
        (${parClasse('arbitrage').length})</span>
      <span><i class="cle" style="background:var(--tuile)"></i>
        <b>dur</b> — fond de tuile : la grammaire ne le porte pas à 16 px ; dessiné quand même, c'est
        un report (${parClasse('dur').length})</span>
      <span style="opacity:.55"><i class="cle" style="background:var(--surface)"></i>
        <b>réservé</b> — éteint : au sous-ensemble, employé nulle part (${RESERVES.length})</span>
    </div>
    <p class="note">Écart mesuré à la grammaire : <b>${arcs.length} glyphes sur ${noms.length}</b>
    emploient au moins un arc, quand le document d'icônes n'en emploie que dans le rabat de Wind.
    Les ${parClasse('dur').length} durs sont : ${parClasse('dur').map((n) => `<code>${n}</code>`).join(', ')}.
    Douze d'entre eux sont les repères de compte, rendus à 10-12 px dans une pastille — ils passent
    <b>sous le palier 16 lui-même</b>, et il leur faudrait un quatrième palier que le document n'a
    pas. C'est le prix de la décision de garder le glyphe dans la pastille (V5).</p>

    <p class="etiq">Le palier — le coût réel n'est pas le dessin</p>
    <p class="sub">Un trait de 2 unités sur une grille de 24, rendu à P px, mesure 2 ÷ 24 × P. Les
    tailles d'emploi de Wind <b>n'atteignent qu'un seul palier</b> : aucune icône du produit ne monte
    à 21 px. Tout tombe dans le palier 16, qui se cale à la main, rectangle par rectangle — et le
    maître ne s'y met pas à l'échelle : 37 % seulement de ses coordonnées survivent au passage
    24 → 16 (il faut être multiple de 3 pour tomber juste). <b>Ce palier n'est pas dessiné</b> ; les
    glyphes ci-dessous sont les maîtres réduits, et ils montrent honnêtement ce que cela donne.</p>
    <table class="echelle">
      <thead><tr><th>Glyphe</th>${TAILLES.map((t) => `<th>${t} px</th>`).join('')}<th>Trait rendu</th></tr></thead>
      <tbody>
        ${ECHELLE.map((n) => `<tr><th scope="row">${esc(n)}</th>
          ${TAILLES.map((t) => `<td><span>${ico(n, t)}</span></td>`).join('')}
          <td style="font-size:11px;color:var(--muted);font-variant-numeric:tabular-nums;text-align:left">
            ${TAILLES.map((t) => (2 / 24 * t).toFixed(2)).join(' · ')} px</td></tr>`).join('\n        ')}
      </tbody>
    </table>
    <p class="note">À 10 px — la taille d'un repère de compte dans une rangée — le trait vaut
    <b>0,83 px</b> : il passe sous le pixel. À 16 px, la taille par défaut de tout le produit, il vaut
    <b>1,33 px</b> et tombe sur des tiers de pixel. C'est la dette V9, et elle ne se délègue pas à
    une mise à l'échelle.</p>

    <p class="etiq">Trois fusions que la grammaire force</p>
    <div class="fiches">
      ${Object.entries(fusions).map(([f, l]) => `<div class="fiche">
        <h3 class="sourcil">${esc(f)}</h3>
        <div style="display:flex;gap:26px;align-items:center">
          ${l.map((n) => `<span style="display:flex;flex-direction:column;align-items:center;gap:8px">
            <span style="color:var(--ink)">${ico(n, 32)}</span>
            <code style="font-size:11px;color:var(--muted)">${esc(n)}</code></span>`).join('')}
        </div>
        <p>Réduits à la grammaire, ces glyphes retombent sur le <b>même dessin</b>. Les garder
        distincts demande d'ajouter du détail — donc de sortir de la grammaire. Trois décisions à
        prendre, pas trois défauts à corriger.</p></div>`).join('\n      ')}
    </div>

    <p class="etiq">Le relevé — un glyphe, un sens, un emploi</p>
    <p class="sub">Une icône ne porte qu'un seul sens dans tout le produit ; quand elle sert deux
    gestes proches (dossier et action), les deux sont notés. Ce relevé <b>est</b> l'inventaire : il
    n'y a plus de contrat séparé à tenir à jour, puisqu'il n'y a plus de fonte à sous-ensembler.</p>
    <table class="tbl">
      <thead><tr><th style="width:52px">Icône</th><th style="width:200px">Symbole</th>
        <th style="width:280px">Libellé</th><th>Emploi</th></tr></thead>
      <tbody>
        ${RELEVE.map(([n, lib, emploi, miroir]) => `<tr>
          <td style="color:var(--ink)">${miroir ? icoMiroir(n, 20) : ico(n, 20)}</td>
          <td class="mono">${esc(n)}${miroir ? ' <span style="color:var(--muted)">(miroir)</span>' : ''}</td>
          <td>${lib}</td><td>${emploi}</td></tr>`).join('\n        ')}
      </tbody>
    </table>
    <p class="note">Les douze repères de compte forment un <b>jeu dédié</b>, réservé à ce seul usage
    et jamais réemployé ailleurs (A74, A3 tenu par réservation) :
    ${REPERES.map((n) => `<code>${n}</code>`).join(' · ')}.</p>
    <p class="note"><b>Une question ouverte, et elle n'est pas de dessin.</b> Ces 78 glyphes sont
    <b>redessinés d'après</b> les formes de Material Symbols — ce n'est pas la même chose que
    redistribuer la police (Apache 2.0), et ce n'est pas non plus une création ex nihilo. La ligne
    « À propos » du produit dit aujourd'hui « police embarquée, licence Apache 2.0 » ; elle devra dire
    autre chose, et <b>quoi exactement reste à trancher</b> avant toute adoption. Consigné, pas
    escamoté.</p>
  </section>`;
};

// ====================================================================
// Les briques de l'application, partagées par tous les écrans.
// ====================================================================
export const enteteApp = ({ menu = false, recherche = null } = {}) => `
  <div class="app-entete">
    ${menu ? `<span class="btn icone">${ico('menu', 16)}</span>` : ''}
    <span class="app-marque"${menu ? ' style="width:auto"' : ''}>${marque(20)}<b>Wind</b></span>
    <span class="recherche${recherche ? ' active' : ''}">${ico('search', 16)}${recherche
      || 'Chercher un message, une personne, un fichier'}
      ${recherche ? `<span style="margin-left:auto;color:var(--muted)">${ico('close', 14)}</span>` : ''}</span>
    <span class="gestes">
      <span class="btn">${ico('edit_square', 16)}Écrire</span>
      <span class="btn icone">${ico('settings', 16)}</span>
    </span>
  </div>`;

// Une pastille de repère : la teinte passe par data-teinte, JAMAIS par un
// hex en ligne — sinon elle reste claire en nuit et son glyphe tombe à
// 2,35:1. Le titre porte l'adresse : la couleur ne dit jamais seule.
export const repere = (glyphe, teinte, adresse, taille = 16) =>
  `<span class="rep${taille === 24 ? ' p24' : ''}" data-teinte="${teinte}" title="${esc(adresse)}">${ico(glyphe, taille === 24 ? 14 : 10)}</span>`;

const rang = (glyphe, libelle, { n, etat = '', rep } = {}) => `
  <div class="rang ${etat}">
    ${rep || ico(glyphe, 16)}
    <span class="l">${libelle}</span>${n ? `<span class="n">${n}</span>` : ''}
  </div>`;

export const nav = ({ boite = 'inbox' } = {}) => `
  <nav class="nav" aria-label="Dossiers et boîtes">
    ${rang('inbox', 'Boîte de réception', { n: 4, etat: boite === 'inbox' ? 'actif' : '' })}
    ${rang('edit_note', 'Brouillons', { n: 3, etat: boite === 'drafts' ? 'actif' : '' })}
    ${rang('send', 'Envoyés')}
    ${rang('report', 'Indésirables', { n: 2 })}
    ${rang('inventory_2', 'Archives')}
    ${rang('delete', 'Corbeille')}
    <p class="nav-titre">Boîtes</p>
    ${rang('all_inbox', 'Toutes les boîtes', { etat: boite === 'unifiee' ? 'actif' : '' })}
    ${rang('person', 'paul@atelier-nord.fr', { etat: boite === 'inbox' ? 'boite' : '' })}
    ${rang(null, 'Atelier Brindille', { rep: repere('work', 'bleu', 'marie@atelier-brindille.fr') })}
  </nav>`;

const tuile = (ini, p26) => `<span class="tuileini${p26 ? ' p26' : ''}">${ini}</span>`;

export const rangee = ({
  ini, exp, heure, objet, apercu, brouillon, nonlu, etat = '', puces, rep, epingle,
}) => `
  <div class="rangee ${etat} ${nonlu ? 'nonlu' : ''}">
    <span class="col">${tuile(ini)}${rep || ''}</span>
    <span class="txt">
      <span class="l1">
        ${nonlu ? '<span class="disque"></span>' : ''}
        ${epingle ? `<span style="color:var(--tuileInk);display:inline-flex">${ico('keep', 14)}</span>` : ''}
        <span class="exp">${exp}</span><span class="h">${heure}</span>
      </span>
      <span class="obj">${objet}</span>
      <span class="apr">${brouillon ? '<span class="brouillon">Brouillon : </span>' : ''}${apercu}</span>
      ${puces ? `<span class="puces" style="margin-top:5px">${puces}</span>` : ''}
    </span>
  </div>`;

export const puce = (glyphe, texte, poids) =>
  `<span class="puce">${ico(glyphe, 14)}${texte}${poids ? `<span class="poids">${poids}</span>` : ''}</span>`;

export const onglets = (actif = 'inbox', compteur = true) => `
  <div class="bandeau bas">
    <span class="onglet ${actif === 'inbox' ? 'actif' : ''}">${ico('inbox', 16)}Tous</span>
    <span class="onglet ${actif === 'unread' ? 'actif' : ''}">${ico('mark_email_unread', 16)}Non lus</span>
    <span class="onglet ${actif === 'drafts' ? 'actif' : ''}">${ico('edit_note', 16)}Brouillons</span>
    ${compteur ? '<span class="compte"><b>4</b><span>/ 18</span></span>' : ''}
  </div>`;

export const statut = ({ texte, bouton = 'Synchroniser', etat = 'repos' }) => `
  <div class="statut">
    ${etat === 'cycle' ? '<span class="anneau"></span>' : '<span class="disque"></span>'}
    ${etat === 'alerte' ? '<span class="pt"></span>' : ''}
    <span class="txt"${etat === 'alerte' ? ' style="color:var(--alert)"' : ''}>${texte}</span>
    <span class="bt"><span class="btn nu${etat === 'cycle' ? ' eteint' : ''}">${ico('sync', 16)}${bouton}</span></span>
  </div>`;

// --------------------------------------------------------------------
const filVolet = () => `
  <div class="fil" aria-label="Conversation">
    <div class="fil-tete">
      <span class="t display">Relecture du contrat Vantis</span>
      <span class="puces">${puce('forum', '3 messages')}${puce('attach_file', '2 fichiers')}</span>
      <span style="margin-left:auto;display:flex;gap:6px">
        <span class="btn nu">${ico('open_in_full', 16)}Ouvrir</span>
        <span class="btn nu">${ico('unfold_more', 16)}Tout déplier</span>
      </span>
    </div>
    <div class="carte replie">${tuile('PM', true)}<span class="nom">Paul Mérand</span>
      <span class="ap">Merci, je regarde ça ce soir et je te réponds demain.</span><span class="h">Lundi, 18:20</span></div>
    <div class="carte replie">${tuile('SN', true)}<span class="nom">Sofia Nardi</span>
      <span class="ap">J'ajoute la grille tarifaire mise à jour au fil.</span><span class="h">Mardi, 11:05</span></div>
    <div class="carte">
      <div class="msg-tete">${tuile('CR')}
        <span><span class="nom">Camille Rousseau</span><br>
        <span class="adr">c.rousseau@atelier-nord.fr · à Paul Mérand</span></span>
        <span class="h">Aujourd'hui, 09:12</span></div>
      <div class="joints">
        <span class="etiqchamp">Fichiers joints</span>
        <span class="puces">${puce('description', 'Contrat_Vantis_v4.pdf', '1,2 Mo')}</span>
      </div>
      <div class="corps-mail" style="font-size:14px">
        <p>Bonjour Paul, j'ai repris les articles 4 et 7 après notre échange de lundi. Il reste la
        clause de renouvellement à trancher : reconduction tacite de douze mois, ou renégociation
        annuelle.</p>
      </div>
      <div class="barre-msg">
        <span class="btn primaire">${ico('reply', 16)}Répondre</span>
        <span class="btn">${ico('reply_all', 16)}Répondre à tous</span>
        <span class="btn">${icoMiroir('reply', 16)}Transférer</span>
      </div>
    </div>
    <div class="barre-fil">
      <span class="btn">${ico('archive', 16)}Archiver</span>
      <span class="btn">${ico('delete', 16)}Supprimer</span>
      <span class="btn">${ico('report', 16)}Signaler comme spam</span>
      <span class="btn">${ico('keep', 16)}Épingler</span>
    </div>
  </div>`;

// --------------------------------------------------------------------
const miniFenetre = (h96) => `<span class="mini${h96 ? ' h96' : ''}">
  <i style="width:22%;background:var(--bg);border-right:1px solid var(--border)"></i>
  <i style="width:32%;background:var(--bg);border-right:1px solid var(--border)"></i>
  <i style="flex:1;background:var(--surface)"></i></span>`;

export const ecran01 = () => `
  <section id="ecran01">
    ${haut()}
    <p class="sourcil">Écran 01</p>
    <h2>Accueil et guichet de compte</h2>
    <p class="sub">Le premier lancement est un <b>parcours en quatre étapes</b> (A75), dans une
    colonne centrée sur le fond d'application, élargie à 760 px aux étapes 2-4. Chaque étape dit,
    dans cet ordre : son titre, la ligne « Étape n/4 », son texte, son contenu, puis la marche
    (Retour / Continuer, Terminer à la fin) — et <b>Continuer ne s'affiche jamais grisé</b> : absent
    tant qu'il ne peut pas continuer. Le hero passe au <b>registre d'affichage</b> (graisse 340) et
    le trait hitofude qui l'accompagnait est remplacé par la <b>marque en tuile</b> (V2/V11).</p>

    <p class="etiq">Étape 1 — les comptes</p>
    <div class="scene" style="padding:40px;display:flex;justify-content:center">
      <div class="colonne">
        <div style="display:flex;align-items:center;gap:14px">${marqueTuile(40)}
          <h3 class="display" style="font-size:40px;line-height:1.05">Bienvenue dans Wind</h3></div>
        <p class="note" style="color:var(--muted)">Étape 1/4</p>
        <p style="margin:0;font-size:15px;line-height:1.65;color:var(--ink2)">Pour commencer, ajoutez
        une adresse email.</p>
        <div style="display:flex;gap:10px">
          <span class="champ" style="flex:1">Adresse e-mail</span>
          <span class="btn primaire" style="height:40px">Ajouter</span>
        </div>
      </div>
    </div>

    <div class="fiches duo">
      <div class="fiche">
        <h3 class="sourcil">Domaine inconnu : le guichet générique révélé</h3>
        <div style="display:flex;flex-direction:column;gap:10px">
          <span class="champ plein">camille@atelier-nord.fr</span>
          <span class="champ">Mot de passe</span>
          <div style="display:grid;grid-template-columns:1fr 88px;gap:10px">
            <span class="champ plein">imap.atelier-nord.fr</span><span class="champ plein">993</span>
          </div>
          <div style="display:grid;grid-template-columns:1fr 88px;gap:10px">
            <span class="champ plein">smtp.atelier-nord.fr</span><span class="champ plein">465</span>
          </div>
          <div style="display:flex;gap:10px;align-items:center">
            <span class="btn primaire" style="height:40px">Ajouter</span>
            <span class="btn" style="height:40px">Retour</span>
          </div>
        </div>
        <p>Porte simple : le domaine choisit le flux. Gmail et Microsoft partent au consentement
        navigateur ; tout autre domaine révèle les champs serveur, pré-remplis <code>imap.domaine</code>
        / <code>smtp.domaine</code>, ports 993 / 465. Le guichet révélé <b>masque Continuer</b>, son
        « Ajouter » est toujours primaire, et « Retour » replie les champs. Le mot de passe rejoint le
        coffre du système, jamais un fichier.</p>
      </div>
      <div class="fiche">
        <h3 class="sourcil">Les trois réponses du guichet : note, attente, erreur</h3>
        <div style="display:flex;flex-direction:column;gap:12px">
          <span class="note" style="color:var(--muted)">Autorisation en cours dans votre navigateur…</span>
          <span style="display:inline-flex;align-items:center;gap:9px;font-size:13px;color:var(--muted)">
            <span class="anneau"></span> Vérification de la connexion au serveur…</span>
          <span style="display:inline-flex;align-items:center;gap:9px;font-size:13px;color:var(--alert)">
            ${ico('warning', 16)} Connexion impossible : le serveur n'a pas répondu.</span>
        </div>
        <p>Une seule réponse à la fois, jamais un empilement. L'attente est une note atténuée —
        accompagnée depuis V2 de l'<b>anneau</b>, qui remplace le trait ; l'erreur est du texte
        d'alerte sous le formulaire, jamais une surimpression. La géométrie de cette fiche est celle
        des Réglages : entrées 40 px, 13 px, sans ombre — <b>une seule implémentation, deux
        surfaces</b> (A11).</p>
      </div>
    </div>

    <p class="etiq">Étape 2 — la disposition, sur une capture réelle de l'application</p>
    <div class="scene" style="padding:34px;display:flex;justify-content:center">
      <div class="colonne large">
        <h3 class="display" style="font-size:32px">Choisissez votre disposition de fenêtre</h3>
        <p class="note" style="color:var(--muted)">Étape 2/4 — vous pourrez la changer plus tard dans
        Réglages &gt; Affichage.</p>
        <div style="background:var(--surface);border-radius:var(--r-surface);box-shadow:var(--shadow);padding:16px;
          display:flex;flex-direction:column;gap:14px;align-items:center">
          ${miniFenetre(true)}
          <div style="display:flex;gap:10px;justify-content:center">
            <span class="btn actif">Trois volets</span>
            <span class="btn">Deux volets</span>
            <span class="btn">Un volet</span>
          </div>
        </div>
        <div style="display:flex;gap:10px;justify-content:flex-end">
          <span class="btn" style="height:40px">Retour</span>
          <span class="btn primaire" style="height:40px">Continuer</span>
        </div>
      </div>
    </div>
    <p class="note">Une <b>capture réelle</b> de l'application — décor Elements, recadrée au-dessus de
    la barre d'état, régénérée par <code>e2e/capture-accueil.mjs</code> — au-dessus de trois boutons
    <b>centrés</b>, l'ensemble dans <b>une élévation</b>. L'image suit le bouton <b>survolé ou
    focalisé</b> tant qu'il l'est, le choix sinon ; le choix s'applique et se persiste à l'instant.
    La sélection se dit comme une ligne choisie : fond <code>--sel</code> + contour d'accent, en
    liseré interne, <b>aucun décalage</b>.</p>

    <p class="etiq">Étape 3 — le thème : deux fiches, et non plus vingt-huit (V7)</p>
    <div class="scene" style="padding:34px;display:flex;justify-content:center">
      <div class="colonne large">
        <h3 class="display" style="font-size:32px">Choisissez votre thème</h3>
        <p class="note" style="color:var(--muted)">Étape 3/4</p>
        <div style="display:grid;grid-template-columns:repeat(2,1fr);gap:14px">
          <div class="porte choisi"><span class="etiqchamp">Elements</span>${miniFenetre()}</div>
          <div class="porte"><span class="etiqchamp">Elements · nuit</span>${miniFenetre()}</div>
        </div>
        <div style="display:flex;gap:10px;justify-content:flex-end">
          <span class="btn" style="height:40px">Retour</span>
          <span class="btn primaire" style="height:40px">Continuer</span>
        </div>
      </div>
    </div>
    <p class="note">Les fiches sont des <b>fenêtres miniatures</b> aux couleurs de la table du
    contrat — jamais un hex recopié — et <b>dans la disposition choisie à l'étape 2</b>. Le geste est
    immédiat : le thème s'applique au clic, il ne se confirme pas.</p>

    <p class="etiq">Étape 4 — le récapitulatif, en cartes-portes côte à côte</p>
    <div class="scene" style="padding:34px;display:flex;justify-content:center">
      <div class="colonne large">
        <h3 class="display" style="font-size:32px">Tout est prêt.</h3>
        <p class="note" style="color:var(--muted)">Étape 4/4 — vérifiez vos choix avant de continuer.</p>
        <div class="portes">
          <div class="porte"><span class="etiqchamp">Comptes</span>
            <b style="font-size:13px">2 adresses</b>${miniFenetre()}</div>
          <div class="porte"><span class="etiqchamp">Disposition</span>
            <b style="font-size:13px">Trois volets</b>${miniFenetre()}</div>
          <div class="porte"><span class="etiqchamp">Thème</span>
            <b style="font-size:13px">Elements</b>${miniFenetre()}
            <span class="vl">${ico('arrow_back', 16)}Revenir à cette étape</span></div>
        </div>
        <div style="display:flex;gap:10px;justify-content:flex-end">
          <span class="btn" style="height:40px">Retour</span>
          <span class="btn primaire" style="height:40px">Terminer</span>
        </div>
      </div>
    </div>
    <p class="note">Au survol comme au focus clavier, un <b>voile</b> couvre la carte et dit « Revenir
    à cette étape » (les règles du voile des pièces jointes, A70 : recouvrement absolu, fond
    <code>--sel</code>, géométrie stable, glyphe <code>arrow_back</code>). Le clic ramène à l'étape
    concernée ; « Terminer » pose la marque <code>wind-accueil-fait</code> et rend la fenêtre
    standard. Le parcours ne se joue qu'<b>une</b> fois : une installation existante est réputée
    accueillie ; un parcours commencé puis abandonné reprend au lancement suivant. L'ordre du
    démarrage tient (A41) : migration, langue, puis l'accueil.</p>
  </section>`;

// --------------------------------------------------------------------
export const ecran02 = () => `
  <section id="ecran02">
    ${haut()}
    <p class="sourcil">Écran 02</p>
    <h2>Boîte de réception</h2>
    <p class="sub">Trois volets, 248 / 400 / 1fr. L'entête tient <b>52 px</b>, le bandeau de titre de
    la liste et le bandeau de filtre du bas tiennent <b>52 px</b> eux aussi, la barre d'état
    <b>36 px</b>. Depuis V3, <b>aucun de ces bandeaux n'a de fond propre</b> : ils vivent sur le sol
    unique, et le filet de 1 px porte seul la séparation. La liste est plate — ni carte ni ombre — et
    le volet de lecture aussi : seules les cartes de message s'élèvent. Les <b>épinglés</b> ouvrent
    le même défilement, sous leur sourcil (A73).</p>

    <div class="app">
      ${enteteApp()}
      <div class="app-corps">
        ${nav()}
        <div class="liste">
          <div class="bandeau"><span class="t">Boîte de réception</span></div>
          <div class="flot">
            <p class="section-liste">Épinglés</p>
            ${rangee({
              ini: 'SN', exp: 'Sofia Nardi', heure: '4 août', epingle: true, etat: 'epingle',
              objet: 'Atelier de septembre',
              apercu: 'Nous visons la semaine du 14, deux salles réservées à Milan.',
            })}
            <p class="section-liste">Tous les messages</p>
            ${rangee({
              ini: 'CR', exp: 'Camille Rousseau', heure: '09:12',
              objet: 'Relecture du contrat Vantis',
              apercu: 'Bonjour Camille, merci pour la v4, je penche pour la reconduction tacite…',
              brouillon: true, etat: 'sel',
              puces: puce('forum', '3 messages') + puce('attach_file', '2 fichiers'),
            })}
            ${rangee({
              ini: 'YB', exp: 'Yanis Belkacem', heure: '08:40', nonlu: true,
              objet: 'Planning de la semaine 33',
              apercu: 'Deux créneaux se chevauchent mardi après-midi, je propose de décaler.',
            })}
            ${rangee({
              ini: 'LF', exp: 'Léa Fontaine', heure: 'Hier', etat: 'survol',
              objet: 'Compte rendu du 4 août',
              apercu: 'Trois décisions actées, une question ouverte sur le calendrier.',
            })}
            ${rangee({
              ini: 'SC', exp: 'Service comptabilité', heure: '5 août',
              objet: 'Facture 2026-0841 réglée',
              apercu: 'Le virement a été émis le 6 août, réception sous deux jours ouvrés.',
            })}
          </div>
          ${onglets('inbox')}
        </div>
        ${filVolet()}
      </div>
      ${statut({ texte: 'Tous les messages sont à jour · dernière synchronisation il y a 2 minutes' })}
    </div>

    <p class="sub"><b>La rangée.</b> Chaque ligne porte une <b>tuile d'initiales de 28 px</b> —
    un carré net, sol <code>--tuile</code>, encre <code>--tuileInk</code>, filet de 1 px,
    11 px 600 — <b>visuelle seule</b> : aucun geste, la sélection en lot n'existe pas. Le
    <b>non-lu</b> se dit par un <b>disque de 9 px</b> en tête de première ligne, plus la graisse de
    l'expéditeur et de l'objet. Le survol teinte le fond sans déplacer le contenu ; la sélection prend
    <code>--sel</code> et le liseré d'accent de 2 px au bord gauche — jamais d'ombre ni de surface
    blanche. Le <b>pied de liste</b> porte les trois onglets et le couple « non-lus / total » en
    chiffres tabulaires.</p>

    <div class="fiches duo">
      <div class="fiche">
        <h3 class="sourcil">Le dossier Brouillons : reprise au clic</h3>
        <div style="border:1px solid var(--border);border-radius:var(--r-controle);overflow:hidden">
          ${rangee({ ini: 'C', exp: 'À : c.rousseau@atelier-nord.fr', heure: '14:32',
            objet: 'Re : Relecture du contrat Vantis',
            apercu: 'Bonjour Camille, merci pour la v4, je penche pour la recond…' })}
          ${rangee({ ini: 'L', exp: 'À : l.fontaine@atelier-nord.fr', heure: '09:15',
            objet: 'Merci pour le compte rendu',
            apercu: 'Bonjour Léa, bien reçu, merci, je relis le calendrier de li…' })}
          ${rangee({ ini: '—', exp: '<i style="color:var(--muted)">(sans destinataire)</i>', heure: 'lun.',
            objet: '<i style="color:var(--muted)">(sans objet)</i>',
            apercu: 'Idées pour la réunion de rentrée : budget, planning, re…' })}
        </div>
        <p>Même gabarit de ligne, servi par les <b>brouillons locaux</b> plutôt que par le dossier
        IMAP : les brouillons hors ligne y figurent, et <b>le clic rouvre le composeur</b>. L'expéditeur
        devient le destinataire (« À : … ») — la tuile aussi : ses initiales, un tiret quand il n'y en
        a pas. Pas de mention « Brouillon » par ligne : tout le dossier en est. La barre d'état garde
        sa forme : « Brouillons · 3 messages ».</p>
      </div>
      <div class="fiche">
        <h3 class="sourcil">Le repère de compte, en boîte unifiée seulement</h3>
        <div style="border:1px solid var(--border);border-radius:var(--r-controle);overflow:hidden">
          ${rangee({ ini: 'MB', exp: 'Marie Brindille', heure: '11:02', nonlu: true,
            objet: 'Devis atelier — relance', apercu: 'Je te renvoie le devis corrigé.',
            rep: repere('work', 'bleu', 'marie@atelier-brindille.fr') })}
          ${rangee({ ini: 'CR', exp: 'Camille Rousseau', heure: '09:12',
            objet: 'Relecture du contrat Vantis', apercu: 'Il reste la clause de renouvellement.',
            rep: repere('home', 'sapin', 'paul@atelier-nord.fr') })}
        </div>
        <p>En <b>boîte unifiée seulement</b> (là où identifier le compte a un sens ; jamais dans la vue
        d'un seul compte, où il ne dirait rien), une ligne dont le compte porte un repère montre,
        <b>sous la tuile</b>, une pastille de 16 px — teinte du nuancier, glyphe du jeu dédié à 10 px,
        <b>infobulle = l'adresse</b>. Elle s'empile dans la colonne de la tuile (28 + 4 + 16), plus
        courte que les trois rangs de contenu : les deux gabarits de hauteur ne bougent pas, le
        fenêtrage non plus. Décorative pour les lecteurs d'écran : l'information est portée par
        l'adresse, déjà lisible ailleurs.</p>
      </div>
    </div>

    <p class="etiq">La recherche — un résultat est un message, pas une conversation</p>
    <div class="app">
      ${enteteApp({ recherche: 'contrat vantis' })}
      <div class="app-corps">
        ${nav({ boite: 'unifiee' })}
        <div class="liste">
          <div class="bandeau"><span class="t">Recherche</span>
            <span class="compte"><b>7</b><span>résultats</span></span></div>
          <div class="flot">
            ${rangee({ ini: 'CR', exp: 'Camille Rousseau', heure: '09:12',
              objet: 'Relecture du <span class="surligne">contrat Vantis</span>',
              apercu: "J'ai repris les articles 4 et 7 après notre échange de lundi.",
              rep: repere('home', 'sapin', 'paul@atelier-nord.fr') })}
            ${rangee({ ini: 'PM', exp: 'Paul Mérand', heure: 'Lundi',
              objet: 'Re : Relecture du <span class="surligne">contrat Vantis</span>',
              apercu: 'Merci, je regarde ça ce soir et je te réponds demain.',
              rep: repere('home', 'sapin', 'paul@atelier-nord.fr') })}
            ${rangee({ ini: 'MB', exp: 'Marie Brindille', heure: '2 août',
              objet: '<span class="surligne">Contrat</span> de sous-traitance',
              apercu: 'Le <span class="surligne">Vantis</span> arrive en fin de période.',
              rep: repere('work', 'bleu', 'marie@atelier-brindille.fr') })}
          </div>
          <div class="bandeau bas">
            <span style="font-size:12px;color:var(--muted)">Affinez votre recherche
            (<code>from:</code>, <code>to:</code>, <code>date:</code>) pour aller plus loin.</span>
          </div>
        </div>
        ${filVolet()}
      </div>
      ${statut({ texte: 'Recherche · 7 résultats' })}
    </div>
    <p class="note">La recherche <b>traverse les comptes et les dossiers</b> : le repère de compte y
    apparaît donc, comme en boîte unifiée. Un résultat est un <b>message</b>, pas une conversation —
    le cœur ne joint pas les fils en recherche : la puce « n messages » n'y figure pas, par
    construction. La barre d'état dit le compte : « Recherche · N résultats », ou
    « <b>Recherche · N sur M résultats</b> » quand le rendu est plafonné (100 max, A50) — le total M
    dit ce que le plafond cache. Le pied de liste remplace les onglets par la note d'affinage.</p>

    <p class="etiq">Les modes d'affichage, et les poignées</p>
    <div class="schemas">
      <div class="schema">
        <span class="cadre">
          <i style="width:24%;background:var(--bg)"></i><i class="poignee"></i>
          <i style="width:34%;background:var(--bg)"></i><i class="poignee"></i>
          <i style="flex:1;background:var(--surface)"></i></span>
        <b>Trois volets — le défaut</b>
        <span>248 / 400 / 1fr. Deux poignées : nav↔liste et liste↔fil.</span>
      </div>
      <div class="schema">
        <span class="cadre">
          <i style="width:24%;background:var(--bg)"></i><i class="poignee"></i>
          <i style="flex:1;background:var(--bg)"></i></span>
        <b>Deux volets</b>
        <span>248 / 1fr. La liste prend la largeur, gabarit de ligne inchangé ; ouvrir un message
        <b>est</b> l'écran 03, plein écran. Une seule poignée.</span>
      </div>
      <div class="schema">
        <span class="cadre"><i style="flex:1;background:var(--bg)"></i></span>
        <b>Un volet</b>
        <span>La liste seule, sans filet droit ; la nav vit dans un <b>tiroir</b>. Aucune poignée.</span>
      </div>
    </div>
    <p class="note">Les largeurs se règlent à la souris (A44) : une poignée de <b>7 px</b> à cheval sur
    chaque frontière, curseur <code>col-resize</code>, <b>trait d'accent de 2 px</b> au survol, à la
    saisie et au focus. Bornes : nav <b>180–400 px</b> (défaut 248), liste <b>300–640 px</b> (défaut
    400) ; le fil prend le reste. <b>Double-clic : retour au défaut.</b> Au clavier (A8), la poignée
    est un <code>separator</code> focalisable : flèches gauche/droite, 16 px par pas. Largeurs
    persistées ; valeur hors bornes → défaut. Le <b>plafond de la fenêtre</b> tient par-dessus les
    bornes : une frontière n'écrase jamais le fil sous <b>120 px</b> de réserve, et au rétrécissement
    de la fenêtre, la liste cède. Le marquage lu, les raccourcis et la barre d'état sont identiques
    dans tous les modes.</p>

    <div class="fiches duo">
      <div class="fiche">
        <h3 class="sourcil">Le tiroir de navigation (un volet)</h3>
        <div style="position:relative;height:150px;border:1px solid var(--border);border-radius:var(--r-controle);overflow:hidden;background:var(--bg)">
          <div style="position:absolute;inset:0;background:var(--scrim)"></div>
          <div style="position:absolute;left:0;top:0;bottom:0;width:220px;background:var(--bg);
            border-right:1px solid var(--border);padding:10px 8px">
            <div style="height:44px;display:flex;align-items:center;gap:9px;padding:0 8px;color:var(--ink)">
              ${marque(20)}<b style="font-size:15px">Wind</b>
              <span style="margin-left:auto;color:var(--muted)">${ico('close', 16)}</span></div>
            <div class="rang actif">${ico('inbox', 16)}<span class="l">Boîte de réception</span><span class="n">4</span></div>
            <div class="rang">${ico('edit_note', 16)}<span class="l">Brouillons</span><span class="n">3</span></div>
            <div class="rang">${ico('send', 16)}<span class="l">Envoyés</span></div>
          </div>
        </div>
        <p>Le bouton (glyphe <code>menu</code>, 32 px, <code>aria-expanded</code>) vit à gauche de la
        marque. Le tiroir est une <b>surimpression de 268 px</b> sous scrim, en-tête 60 px (marque,
        mot « Wind », fermer) puis la <b>nav réutilisée telle quelle</b>. Choisir un dossier ferme le
        tiroir ; Échap ferme dans l'ordre des surimpressions ; le scrim est un bouton, au clic comme
        au clavier (A8). <code>role="dialog"</code> et <code>aria-modal</code>.</p>
      </div>
      <div class="fiche">
        <h3 class="sourcil">La liste sous recharge, et le geste et sa destination</h3>
        <span class="toast">${ico('hourglass_empty', 16)}Copie en cours de synchronisation : réessayez dans un instant.</span>
        <p>Une recharge <b>ne montre pas d'attente sur des lignes déjà servies</b> : les lignes
        affichées restent en place, chaque page est remplacée à l'arrivée de sa version fraîche, sans
        squelette et sans saut de défilement (A23). Le total ne retombe pas à zéro. Supprimer,
        archiver, envoyer : la destination montre le message <b>immédiatement</b>, hors ligne compris
        (A24) — un <b>écho local</b> visuellement identique, que la vraie copie remplace sans
        mouvement visible. Les autres gestes sur l'écho <b>attendent la réconciliation</b>, et le
        <b>toast</b> ci-dessus le dit — surface, filet, coin vif, élévation unique, jamais plus d'un à
        la fois. Si la copie n'apparaît pas après une relève propre, l'écho se retire et l'incident se
        consigne.</p>
      </div>
    </div>
  </section>`;

// --------------------------------------------------------------------
export const barreEtat = () => {
  const etat = (titre, contenu, note) => `
    <div class="fiche">
      <h3 class="sourcil">${titre}</h3>
      <div class="app" style="width:auto">
        <div style="height:56px;display:flex;align-items:center;justify-content:center;color:var(--muted);
          font-size:12px;border-bottom:1px solid var(--border)">… volet de lecture …</div>
        ${contenu}
      </div>
      ${note ? `<p>${note}</p>` : ''}
    </div>`;

  return `
  <section id="statut">
    ${haut()}
    <h2>Barre d'état et synchronisation</h2>
    <p class="sub">La barre d'état de <b>36 px</b> est la région du <b>continu</b> : un seul message à
    la fois, texte 12 px atténué, sans empilement. L'horodatage se re-rend toutes les 30 secondes.
    Le <b>bouton de relève</b> (S-D1, A16), 26 px, vit à droite : « Synchroniser » au repos, inhibé
    pendant un cycle, « Réessayer » sur échec ; son glyphe reste immobile. Hors ligne, le geste manuel
    force toujours une relève. À gauche vit la <b>signature</b> : depuis V2, le <b>disque</b> plein et
    immobile au repos, l'<b>anneau</b> évidé et tournant dès qu'une action est en cours — cycle,
    intégrale, rattrapage, attente d'envoi. Le <b>pourcentage</b>, quand un dénominateur exact existe,
    vit dans le <b>texte</b> de la ligne, jamais dans la signature (A52 tient : l'ancien tracé masqué
    à une longueur partielle restait figé chez Chromium). Les états d'alerte se précèdent d'un point
    de 7 px.</p>
    <div class="fiches">
      ${etat('1 · À jour', statut({ texte: 'Tous les messages sont à jour · dernière synchronisation il y a 2 minutes' }))}
      ${etat("2 · Cycle en cours — l'anneau tourne, le % dans le texte",
        statut({ texte: 'Synchronisation · 2/4 · marie@atelier-brindille.fr · [Gmail]/Tous les messages… · 37 %',
          bouton: 'Synchronisation…', etat: 'cycle' }),
        `Le cycle courant est <b>visible</b> : compte, position, boîte ou étape, et pourcentage de
        l'intégrale quand il existe. Sonde à la seconde pendant le cycle seulement.`)}
      ${etat('3 · Échec total',
        statut({ texte: 'Synchronisation impossible · nouvelle tentative automatique · dernière synchronisation il y a 12 minutes',
          bouton: 'Réessayer', etat: 'alerte' }))}
      ${etat('4 · Échec partiel',
        statut({ texte: '1 compte sur 2 injoignable · nouvelle tentative automatique', bouton: 'Réessayer', etat: 'alerte' }),
        `Un compte injoignable reste signalé même quand l'autre répond : sans cet état, l'horodatage
        rajeuni par le compte joignable masquerait l'anomalie.`)}
      ${etat('5 · Hors ligne',
        statut({ texte: 'Hors ligne · dernière synchronisation il y a 18 minutes' }),
        `L'état réseau vient de l'OS par la WebView, <b>à l'instant</b>, sans attente de timeout.
        L'application sert le stock local. Les cycles automatiques se suspendent ; le retour réseau
        déclenche une passe légère immédiate. « Hors ligne » est un <b>état</b>, pas un incident : il
        vit dans la ligne, jamais dans la fente d'avis.`)}
      ${etat('6 · Rattrapage',
        statut({ texte: 'Rattrapage des messages · 412 restants · 88 %', bouton: 'Synchronisation…', etat: 'cycle' }),
        `Les rattrapages passent par la même ligne (A55) — le pourcentage vit dans le TEXTE, plafonné
        à 99 tant qu'il reste un corps à rapatrier, jamais « 100 % » sur une traîne encore en cours.
        « Rattrapage des aperçus… » suit la même forme.`)}
      ${etat('7 · Recherche plafonnée (A50)',
        statut({ texte: 'Recherche · 100 sur 348 résultats' }),
        `Le rendu est plafonné à 100 : le total <b>M</b> dit ce que le plafond cache, sans quoi
        l'utilisateur croirait avoir tout vu.`)}
      ${etat("8 · Attente d'envoi",
        statut({ texte: "2 messages en attente d'envoi", bouton: 'Synchronisation…', etat: 'cycle' }),
        `L'attente de la boîte d'envoi vit ici, dans la ligne de progression ; <b>seul l'échec monte
        en avis</b> (voir « Avis et progression »).`)}
    </div>
  </section>`;
};

// ====================================================================
// V14 — les coins. L'arbitrage est CLOS : le système passe au coin vif.
//
// La section garde la comparaison côte à côte, non plus pour choisir
// mais comme trace : ce qui a été écarté, et pourquoi. Chaque colonne
// porte ses trois jetons de forme en ligne — elles ne dépendent pas de
// l'état du document, elles montrent deux états fixes.
// ====================================================================
import { ico } from './socle.mjs';
import { haut } from './parties-1.mjs';

const echantillon = (rayons) => `
  <div style="${rayons};display:flex;flex-direction:column;gap:14px">
    <div class="carte" style="gap:10px">
      <div class="msg-tete"><span class="tuileini">CR</span>
        <span><span class="nom">Camille Rousseau</span><br>
        <span class="adr">c.rousseau@atelier-nord.fr</span></span>
        <span class="h">09:12</span></div>
      <div class="puces">
        <span class="puce">${ico('forum', 14)}3 messages</span>
        <span class="puce">${ico('description', 14)}Contrat_Vantis_v4.pdf<span class="poids">1,2 Mo</span></span>
      </div>
    </div>
    <div class="bande">
      <span class="btn primaire">${ico('send', 16)}Envoyer</span>
      <span class="btn">${ico('attach_file', 16)}Joindre</span>
      <span class="onglet actif">${ico('inbox', 16)}Tous</span>
      <span class="champ h32" style="width:140px">Objet</span>
    </div>
    <div class="bande">
      <span class="rang actif" style="width:176px">${ico('inbox', 16)}<span class="l">Réception</span><span class="n">4</span></span>
      <span class="rang boite" style="width:176px">${ico('person', 16)}<span class="l">paul@atelier-nord.fr</span></span>
    </div>
    <div class="bande" style="gap:18px">
      <span class="disque" title="le disque : l'état"></span>
      <span class="anneau" title="l'anneau : le cycle"></span>
      <span class="rep p24" data-teinte="sapin" title="la pastille : l'identité">${ico('home', 14)}</span>
      <span class="inter arme" title="la pilule : le glissement"><i></i></span>
    </div>
  </div>`;

export const coins = () => `
  <section id="coins">
    ${haut()}
    <h2>Les coins</h2>
    <p class="sub"><b>Zéro rayon.</b> Surfaces, contrôles, tuiles, cartes, modales, puces, champs,
    onglets, rangées de navigation, fenêtre d'application : <b>coin vif partout</b>, dans ce document
    comme dans le produit. Restent <b>deux formes rondes</b>, et chacune dit quelque chose — le
    <b>disque</b> (l'état, l'identité) et la <b>pilule</b> de l'interrupteur (le glissement). Une seule
    exception, déclarée et permanente : l'icône d'application, dont le rayon est une cote de
    plateforme. C'est le verdict V14 ; voici ce qui l'a établi, et ce qu'il coûte.</p>
    <div class="rangeeH">
      <span class="verdict"><span class="d"></span><b>Validé au terrain le 2026-08-24</b></span>
      <span class="note" style="margin:0">Constat du Chef Ingénieur sur le rendu réel du document ;
      l'idiome de la plateforme se re-constatera à la première fenêtre d'application livrée.</span>
    </div>

    <div class="fiches">
      <div class="fiche">
        <h3 class="sourcil">1 · Ce que la direction fait — mesuré, pas supposé</h3>
        <p><b>Les dessins sont strictement à coins vifs.</b> Les 78 glyphes, <b>219 sous-chemins</b>,
        <b>654 commandes de tracé</b> : <b>zéro coin arrondi</b>, <code>stroke-linejoin: miter</code>
        et <code>stroke-linecap: butt</code> partout, sans une exception.</p>
        <p><b>Les documents qui les présentent sont arrondis — et indisciplinés.</b> La planche des
        glyphes et le jeu d'icônes emploient à eux deux <b>huit rayons différents</b> : 18, 16, 14,
        12, 11, 10, 9 et 2 px, plus la pilule. Ce sont des <b>pages de présentation</b>, pas un
        système : elles ne pouvaient pas servir de modèle à la grammaire des contenants.</p>
        <p>La direction avait donc une grammaire stricte pour ce qu'elle <i>dessine</i> et
        <b>aucune</b> pour ce qu'elle <i>contient</i>. Il fallait décider — la recopier n'aurait pas
        été de la cohérence, mais un héritage de désordre.</p>
      </div>
      <div class="fiche">
        <h3 class="sourcil">2 · Ce qui a décidé : l'argument de V4, jusqu'au bout</h3>
        <p>V4 avait donné à la tuile d'initiales un rayon de 2 px pour une raison précise : <b>à 6 px
        elle devenait franchement ronde et volait au disque son unicité de sens</b>. L'argument était
        bon, et il n'était appliqué qu'à moitié. Si le rond porte du sens, alors <b>chaque contenant
        arrondi le dilue</b> : une carte à 10, un bouton à 6 et une tuile à 2 étaient trois degrés de
        rondeur qui ne disaient rien.</p>
        <p style="color:var(--ink);font-size:15px;line-height:1.6"><b>Zéro rayon. Deux formes rondes,
        et elles disent quelque chose : le disque</b> (l'état, l'identité) <b>et la pilule</b> (le
        glissement).</p>
        <p>La règle est plus courte que celle qu'elle remplace — « trois rayons, aucune autre
        valeur » — et elle se vérifie d'un regard. C'est le geste de V3, qui a tué
        <code>--panel</code> pour laisser le filet faire le travail : ici on tue les rayons pour
        laisser le rond faire le sens. Et c'est Takumi : retirer, pas ajouter.</p>
      </div>
      <div class="fiche">
        <h3 class="sourcil">3 · Ce que ça coûte — dit, pas escamoté</h3>
        <p><b>Windows 11 arrondit tout.</b> Wind vise Windows — Segoe UI Variable, WebView2, Tauri —
        et Fluent arrondit fenêtres, menus, champs et boutons. Une application à coins vifs peut y
        lire comme <b>étrangère à sa plateforme</b>. C'était le seul argument sérieux contre, et il ne
        se jugeait pas sur une planche.</p>
        <p><b>Constat terrain du 2026-08-24</b> : ce document rendu dans le navigateur, parcouru à
        l'écran réel — <b>coins droits validés, gardés</b>. La réserve est nommée : ce qui a été
        regardé est le <b>document</b>, pas une fenêtre d'application posée à côté des applications
        du système. L'idiome Fluent se re-constatera à la première fenêtre livrée. Si elle dit autre
        chose, le retour tient en <b>une ligne</b> — remettre 10 / 6 / 2 aux trois jetons de forme.
        C'est pour ça qu'ils existent, et ils restent.</p>
        <p><b>L'icône d'application reste arrondie</b> — 15/64, cote de plateforme. Elle devient la
        <b>seule forme arrondie de tout le produit</b>, au point le plus regardé. Exception déclarée,
        permanente, et visible.</p>
        <p><b>L'élévation supporte moins bien le coin vif</b> : une ombre portée sous un angle droit
        se voit plus qu'elle ne se ressent. Coût faible — le système ne s'élève qu'à cinq endroits.</p>
      </div>
    </div>

    <p class="etiq">4 · Ce qui a été écarté, et ce qui a été retenu</p>
    <div class="fiches duo">
      <div class="fiche">
        <h3 class="sourcil">Écarté — 10 / 6 / 2</h3>
        ${echantillon('--r-surface:10px;--r-controle:6px;--r-tuile:2px')}
      </div>
      <div class="fiche">
        <h3 class="sourcil">Retenu — zéro rayon</h3>
        ${echantillon('--r-surface:0;--r-controle:0;--r-tuile:0')}
      </div>
    </div>
    <p class="note">Les quatre formes de la dernière rangée — disque, anneau, pastille, pilule —
    <b>ne bougent pas</b> d'une colonne à l'autre, et c'est le point : ce ne sont pas des rayons, ce
    sont des formes qui portent un sens. Dans la colonne retenue, elles sont les <b>seules</b> choses
    rondes du système. <b>La jauge a quitté cette rangée</b> : une barre de progression n'est pas une
    forme porteuse mais un contenant — elle est passée au coin vif avec le reste.</p>

    <p class="etiq">5 · Comment la règle se tient</p>
    <p class="sub">Par trois <b>jetons de forme</b> — <code>--r-surface</code>,
    <code>--r-controle</code>, <code>--r-tuile</code> — déclarés sur <code>html</code> et non sur
    <code>:root</code> : ils ne dépendent pas de la polarité, et le contrat des jetons de couleur ne
    doit pas s'en trouver gonflé. <b>Il n'y a plus un seul littéral de rayon à écrire</b> dans tout le
    système : « aucune autre valeur » cesse d'être une règle qu'on obéit à la main pour devenir une
    règle qu'on ne peut plus enfreindre par distraction. La bascule qui servait à l'arbitrage a été
    <b>retirée</b> : un Système qui offre deux états de sa propre règle est un Système qui n'a pas
    tranché.</p>
  </section>`;

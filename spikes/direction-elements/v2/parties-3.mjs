// ====================================================================
// Système v2 « Elements » — parties 14 à 20 :
// Écran 03 (conversation), Écran 04 (composition), Réglages, Migration,
// Avis et progression, Ligne de message, Journal des amendements.
// ====================================================================
import { ico, icoMiroir, marque, marqueTuile, esc, THEMES, NUANCIER_COMPOSEUR } from './socle.mjs';
import { haut } from './parties-1.mjs';
import { puce, rangee, statut, repere } from './parties-2.mjs';

const tuile = (ini, p26) => `<span class="tuileini${p26 ? ' p26' : ''}">${ini}</span>`;

// La puce d'une pièce jointe et son VOILE (A70) : recouvrement absolu,
// géométrie stable — jamais une puce de plus.
const pjVoilee = (nom, poids) => `<span class="pj">
  <span class="puce">${ico('description', 14)}${nom}<span class="poids">${poids}</span></span>
  <span class="voile-pj">${ico('download', 14)}Enregistrer</span></span>`;

// --------------------------------------------------------------------
export const ecran03 = () => `
  <section id="ecran03">
    ${haut()}
    <p class="sourcil">Écran 03</p>
    <h2>Conversation en lecture</h2>
    <p class="sub">L'écran 03 est <b>à plat</b> (A72) : plus de carte pleine — une colonne de lecture
    centrée, bornée à 960 px, la scène défile en un seul flot. Le volet de lecture et l'écran 03 sont
    <b>deux cadres du même objet</b> (composant Fil, état partagé) : agrandir ne recharge rien,
    dépliage et corps survivent. Le titre passe au <b>registre d'affichage</b> (24 px, graisse 340).</p>

    <div class="scene" style="padding:26px;display:flex;justify-content:center">
      <div style="width:960px;max-width:100%;display:flex;flex-direction:column;gap:14px">
        <div style="display:flex;align-items:center;gap:10px">
          <span class="btn nu">${ico('arrow_back', 16)}Boîte de réception</span>
        </div>
        <div class="fil-tete">
          <span class="t display">Relecture du contrat Vantis</span>
          <span class="puces">${puce('forum', '3 messages')}${puce('attach_file', '2 fichiers')}</span>
          <span style="margin-left:auto"><span class="btn nu">${ico('unfold_more', 16)}Tout déplier</span></span>
        </div>
        <div class="carte replie">${tuile('PM', true)}<span class="nom">Paul Mérand</span>
          <span class="ap">Merci, je regarde ça ce soir et je te réponds demain.</span>
          <span class="h">Lundi, 18:20</span></div>
        <div class="carte replie">${tuile('SN', true)}<span class="nom">Sofia Nardi</span>
          <span class="ap">J'ajoute la grille tarifaire mise à jour au fil.</span>
          <span class="h">Mardi, 11:05</span></div>
        <div class="carte">
          <div class="msg-tete">${tuile('CR')}
            <span><span class="nom">Camille Rousseau</span><br>
            <span class="adr">c.rousseau@atelier-nord.fr · à Paul Mérand</span></span>
            <span class="h">Aujourd'hui, 09:12</span></div>
          <div class="joints">
            <span class="etiqchamp">Fichiers joints</span>
            <span class="puces">
              ${pjVoilee('Contrat_Vantis_v4.pdf', '1,2 Mo')}
              ${puce('description', 'Annexe_tarifs.xlsx', '84 Ko')}
            </span>
          </div>
          <div class="garde-images">${ico('link_off', 16)}
            <span style="flex:1">Les images distantes de ce message sont bloquées.</span>
            <span class="btn nu">Afficher les images</span></div>
          <div class="corps-mail">
            <p>Bonjour Paul,</p>
            <p>J'ai repris les articles 4 et 7 après notre échange de lundi. Il reste la clause de
            renouvellement à trancher : reconduction tacite de douze mois, ou renégociation annuelle.
            Les deux options sont annotées dans le document.</p>
            <p>Si tu peux me dire d'ici jeudi, je transmets la version finale au cabinet vendredi
            matin. La version annotée est lisible sur <a href="#ecran03">l'espace partagé</a>.</p>
            <p>Camille</p>
          </div>
          <div class="barre-msg">
            <span class="btn primaire">${ico('reply', 16)}Répondre</span>
            <span class="btn">${ico('reply_all', 16)}Répondre à tous</span>
            <span class="btn">${icoMiroir('reply', 16)}Transférer</span>
          </div>
        </div>
        <div class="carte replie" style="box-shadow:none;border:1px dashed var(--accent);background:var(--bg)">
          <span style="color:var(--alert);display:inline-flex;align-items:center;gap:7px;font-size:13px;font-weight:600">
            ${ico('edit_note', 16)}Brouillon</span>
          <span class="ap">Bonjour Camille, merci pour la v4, je penche pour la reconduction tacite…</span>
          <span class="h">14:32</span>
          <span class="btn nu">Reprendre</span>
        </div>
        <div class="barre-fil">
          <span class="btn">${ico('archive', 16)}Archiver</span>
          <span class="btn">${ico('delete', 16)}Supprimer</span>
          <span class="btn">${ico('report', 16)}Signaler comme spam</span>
        </div>
      </div>
    </div>

    <div class="fiches duo">
      <div class="fiche">
        <h3 class="sourcil">Le voile d'une pièce jointe (A70) — recouvrement absolu</h3>
        <div class="bande">
          <span class="puces">${puce('description', 'Contrat_Vantis_v4.pdf', '1,2 Mo')}</span>
          <span style="color:var(--muted);font-size:12px">au repos →</span>
          <span class="puces">${pjVoilee('Contrat_Vantis_v4.pdf', '1,2 Mo')}</span>
        </div>
        <p>Au survol comme au focus, un voile <b>recouvre entièrement</b> la puce et dit l'action
        avant le clic : fond <code>--sel</code>, liseré d'accent <b>interne</b>, glyphe
        <code>download</code>, libellé « Enregistrer ». <b>La géométrie ne bouge pas</b> : le voile
        prend exactement la boîte de la puce, rien ne se décale, rien ne s'ajoute à la rangée. Ce
        n'est jamais une puce de plus — une puce de plus déplacerait les voisines et ferait mentir le
        compte de la tête du fil.</p>
      </div>
      <div class="fiche">
        <h3 class="sourcil">Règles de transmission (A1, A61)</h3>
        <p>Le corps du message est à 16 px du bord de sa carte, comme l'en-tête. Le texte brut
        converti se lit en colonne de 68 caractères ; la mise en page d'un expéditeur n'est pas bridée.
        Le corps s'affiche <b>toujours sur dalle claire</b> — <code>mail-render</code> bake
        <b>encre <code>#222222</code>, fond <code>#ffffff</code></b> (<code>Palette::default</code>)
        — <b>quel que soit le thème</b> : un courriel est composé pour un fond clair, on l'affiche tel
        quel. Ce sont les <b>deux seules</b> valeurs que Wind pose dans le document ; un lien garde ce
        que le message, ou le navigateur, lui donne. Le <code>color-scheme</code> du document suit le
        fond baké : poignées de défilement cohérentes. La <b>garde d'images distantes</b> est
        <b>par message</b> (glyphe <code>link_off</code>), collée au corps qu'elle concerne, sur le
        fond d'application — c'est l'un des deux emplois de <code>--panel</code> que V3 fait
        retomber sur <code>--bg</code>.</p>
      </div>
    </div>

    <p class="etiq">La carte d'invitation (A76) — une carte DANS la carte de message</p>
    <div class="fiches duo">
      <div class="fiche">
        <h3 class="sourcil">Invitation reçue, sans réponse</h3>
        <div class="invitation">
          <div class="inv-tete"><span class="inv-kicker">Invitation</span>
            <span class="inv-statut">Vous n'avez pas répondu</span></div>
          <div class="inv-corps">
            <span class="inv-tuile"><span class="inv-mois">Sept.</span><span class="inv-jour">11</span></span>
            <div class="inv-details">
              <span class="inv-titre">Atelier de septembre — cadrage</span>
              <span class="inv-quand">jeudi 11 septembre, 14:00 – 15:30</span>
              <span class="inv-lieu">Salle Milan · organisé par Sofia Nardi</span>
            </div>
          </div>
          <div class="inv-actions">
            <span class="btn h30 ton-accepte">${ico('check_circle', 16)}Accepter</span>
            <span class="btn h30 ton-provisoire">${ico('question_mark', 16)}Provisoire</span>
            <span class="btn h30 ton-refuse">${ico('cancel', 16)}Refuser</span>
          </div>
        </div>
        <p>Elle vit <b>en tête du contenu</b>, avant les fichiers : c'est l'objet du message. Tuile de
        date à la paire <code>--tuile</code> / <code>--tuileInk</code> — le sol des objets Wind, coin
        vif, filet de 1 px, comme la tuile d'initiales. <b>Trois boutons neutres</b> (A14 intact : la
        carte ne hiérarchise pas la réponse) ; <b>la couleur est portée par l'icône</b>, accent /
        neutre / alerte, et <b>le texte double toujours</b> (A8). Coin vif, <b>sans élévation</b> :
        elle appartient au flot du contenu, pas au fil.</p>
      </div>
      <div class="fiche">
        <h3 class="sourcil">Réponse en place, et invitation annulée</h3>
        <div class="invitation">
          <div class="inv-tete"><span class="inv-kicker">Invitation</span>
            <span class="inv-statut">Vous avez accepté</span></div>
          <div class="inv-corps">
            <span class="inv-tuile"><span class="inv-mois">Sept.</span><span class="inv-jour">11</span></span>
            <div class="inv-details">
              <span class="inv-titre">Atelier de septembre — cadrage</span>
              <span class="inv-quand">jeudi 11 septembre, 14:00 – 15:30</span>
              <span class="inv-repondant">Sofia Nardi a accepté</span>
            </div>
          </div>
          <div class="inv-actions">
            <span class="btn h30 actif ton-accepte">${ico('check_circle', 16)}Accepter</span>
            <span class="btn h30 ton-provisoire">${ico('question_mark', 16)}Provisoire</span>
            <span class="btn h30 ton-refuse">${ico('cancel', 16)}Refuser</span>
          </div>
        </div>
        <div class="invitation" style="margin-top:12px">
          <div class="inv-tete"><span class="inv-kicker annulee">Invitation annulée</span></div>
          <div class="inv-corps">
            <span class="inv-tuile eteinte"><span class="inv-mois">Sept.</span><span class="inv-jour">11</span></span>
            <div class="inv-details">
              <span class="inv-titre barre">Atelier de septembre — cadrage</span>
              <span class="inv-annulee">La réunion a été annulée par l'organisateur.</span>
            </div>
          </div>
        </div>
        <p>La réponse en cours se dit par <code>aria-pressed</code> — fond <code>--sel</code>, liseré
        d'accent, graisse : <b>la sélection d'A75</b>, la même partout. L'annulation passe le sourcil
        en alerte, la tuile en <b>éteint</b> (fond d'application, encre atténuée) et le titre en
        <b>barré</b> ; les trois boutons disparaissent — on ne répond pas à ce qui n'a plus lieu. Une
        <b>heure flottante</b> s'affiche telle quelle, jamais convertie, et le dit :
        « heure locale de l'organisateur ».</p>
      </div>
    </div>

    <div class="fiches duo">
      <div class="fiche">
        <h3 class="sourcil">Le brouillon du fil, en dernière position</h3>
        <p>Un brouillon relié au fil occupe la dernière position, sous forme de message replié au
        <b>trait d'accent pointillé</b>, avec la mention ✎ Brouillon (alerte, glyphe
        <code>edit_note</code>) à la place de l'auteur et l'heure de dernière édition. Un clic n'importe
        où sur le bloc reprend le brouillon ; le bouton nomme le geste. Le composeur se superpose, la
        conversation reste montée dessous. Ce bloc rend la conversation cohérente avec la liste, dont
        l'aperçu porte « Brouillon : ».</p>
      </div>
      <div class="fiche">
        <h3 class="sourcil">La réponse est PAR message (A58)</h3>
        <p>Depuis A58, Répondre / Répondre à tous / Transférer visent <b>chaque message</b> — barre en
        bas de chaque carte dépliée, <b>le nôtre compris</b> : répondre sur son propre message vise
        alors les <b>destinataires d'origine</b> (le À pour Répondre, À+Cc pour Répondre à tous),
        jamais soi-même. « Répondre à tous » reste <b>entre</b> Répondre et Transférer (A14). La barre
        du <b>fil</b> ne garde que les gestes de <b>tri</b> : Archiver, Supprimer, Signaler comme spam
        — qui devient « Ce n'est pas un spam » (glyphe <code>inbox</code>) en vue Indésirables — et,
        en Réception seule, « Épingler » / « Désépingler » (A73). Jamais de menu « Plus ».</p>
      </div>
    </div>
  </section>`;

// --------------------------------------------------------------------
export const ecran04 = () => {
  const outil = (n, actif) => `<span class="btn icone${actif ? ' actif' : ''}" style="height:32px">${ico(n, 16)}</span>`;
  return `
  <section id="ecran04">
    ${haut()}
    <p class="sourcil">Écran 04</p>
    <h2>Composition</h2>
    <p class="sub">Carte de composition en surimpression : <b>surface + élévation unique</b>, coin vif.
    L'entête ne répète plus l'objet — le champ Objet le dit dessous. L'entête et la barre de mise en
    forme portent le <b>fond d'application</b> (V3 : ce qui était <code>--panel</code>), ce qui encadre
    la carte haut et bas sans ajouter de jeton.</p>

    <div class="scene" style="padding:30px;display:flex;justify-content:center">
      <div style="width:900px;max-width:100%;background:var(--surface);border-radius:var(--r-surface);
        box-shadow:var(--shadow);overflow:hidden;display:flex;flex-direction:column">
        <div class="modale-tete" style="background:var(--bg)">
          <span class="t">Nouveau message</span>
          <span style="margin-left:auto;color:var(--muted)">${ico('close', 16)}</span>
        </div>
        <div style="padding:14px 16px;display:flex;flex-direction:column;gap:10px">
          <div style="display:flex;align-items:center;gap:12px">
            <span class="etiqchamp" style="width:44px">De</span>
            <span style="font-size:13px;color:var(--ink)">Atelier Nord — paul.merand@atelier-nord.fr</span>
            <span style="color:var(--muted);font-size:11px">▾</span>
          </div>
          <div style="display:flex;align-items:center;gap:12px;border-top:1px solid var(--border);padding-top:10px">
            <span class="etiqchamp" style="width:44px">À</span>
            <span style="font-size:13px;color:var(--ink);flex:1">Camille Rousseau &lt;c.rousseau@atelier-nord.fr&gt;</span>
            <span class="btn nu">${ico('group_add', 16)}Cc</span>
            <span class="btn nu">${ico('visibility_off', 16)}Cci</span>
          </div>
          <div style="display:flex;align-items:center;gap:12px;border-top:1px solid var(--border);padding-top:10px">
            <span class="etiqchamp" style="width:44px">Objet</span>
            <span style="font-size:13px;color:var(--ink)">Re : Relecture du contrat Vantis avant vendredi</span>
          </div>
        </div>
        <div class="miseenforme">
          <span class="btn" style="height:28px;padding:0 10px">Police ▾</span>
          <span class="btn" style="height:28px;padding:0 10px">Taille ▾</span>
          <span class="sep"></span>
          ${outil('format_bold', true)}${outil('format_italic')}${outil('format_underlined')}${outil('strikethrough_s')}
          ${outil('format_color_text')}
          <span class="sep"></span>
          ${outil('format_align_left', true)}${outil('format_align_center')}${outil('format_align_right')}
          <span class="sep"></span>
          ${outil('format_list_bulleted')}${outil('format_list_numbered')}
          ${outil('format_indent_decrease')}${outil('format_indent_increase')}
          <span class="sep"></span>
          ${outil('format_clear')}
          <span class="sep"></span>
          ${outil('priority_high', true)}
        </div>
        <div style="padding:16px;display:flex;flex-direction:column;gap:14px">
          <div style="font-size:15px;line-height:1.65;color:var(--ink)">
            <p style="margin:0 0 12px">Bonjour Camille,</p>
            <p style="margin:0">Je retiens la reconduction tacite de douze mois, avec un préavis de
            trois mois. Je te renvoie le document annoté demain matin.</p>
          </div>
          <div class="puces">
            <span class="puce">${ico('description', 14)}Contrat_Vantis_v4.pdf<span class="poids">1,2 Mo</span>${ico('close', 14)}</span>
            <span class="puce">${ico('description', 14)}Annexe_tarifs.xlsx<span class="poids">84 Ko</span>${ico('close', 14)}</span>
            <span style="font-size:12px;color:var(--muted);align-self:center;font-variant-numeric:tabular-nums">1,3 Mo / 25 Mo</span>
          </div>
        </div>
        <div class="modale-pied" style="background:var(--bg);justify-content:flex-start">
          <span class="btn primaire">${ico('send', 16)}Envoyer</span>
          <span class="btn">${ico('schedule_send', 16)}Envoyer plus tard</span>
          <span class="btn">${ico('attach_file', 16)}Joindre</span>
          <span class="btn">${ico('drafts', 16)}Enregistrer le brouillon</span>
          <span style="margin-left:auto;display:flex;gap:8px">
            <span class="btn">Annuler</span>
            <span class="btn alerte">${ico('delete', 16)}Supprimer le brouillon</span>
          </span>
        </div>
      </div>
    </div>

    <div class="fiches">
      <div class="fiche">
        <h3 class="sourcil">Le nuancier de couleur du texte (A62-D3) — douze teintes fixes</h3>
        <div class="nuancier" role="group" aria-label="Couleur du texte">
          ${NUANCIER_COMPOSEUR.map((c) => `<i style="background:${c}" title="${c}"></i>`).join('')}
        </div>
        <p>Le glyphe <code>format_color_text</code> ouvre une carte au-dessus de la barre — surface,
        coin vif, élévation unique, l'idiome de la carte d'échéance. <b>Douze teintes, fixes</b> :
        pas de sélecteur libre, pas de roue chromatique. Ce sont des <b>couleurs de contenu</b> — elles
        vivent dans le corps du message, jamais dans l'interface — et la <b>barre basse</b> du glyphe
        porte la teinte choisie : c'est le seul élément coloré de tout le jeu d'icônes, et il ne
        décore pas, il dit un état.</p>
      </div>
      <div class="fiche">
        <h3 class="sourcil">La carte d'échéance « Envoyer plus tard » (A69)</h3>
        <div style="background:var(--surface);border:1px solid var(--border);border-radius:var(--r-surface);
          box-shadow:var(--shadow);padding:14px;display:flex;flex-direction:column;gap:10px;width:fit-content">
          <span class="etiqchamp">Envoyer plus tard</span>
          <div style="display:flex;flex-direction:column;gap:6px">
            <span class="btn" style="justify-content:flex-start;width:230px">${ico('schedule_send', 16)}Ce soir · 18:00</span>
            <span class="btn actif" style="justify-content:flex-start;width:230px">${ico('schedule_send', 16)}Demain matin · 08:00</span>
            <span class="btn" style="justify-content:flex-start;width:230px">${ico('schedule_send', 16)}Lundi matin · 08:00</span>
          </div>
          <div style="display:flex;gap:8px;justify-content:flex-end">
            <span class="btn">Annuler</span><span class="btn primaire">Programmer</span></div>
        </div>
        <p>Elle s'ouvre <b>au-dessus du pied</b>, jamais en surimpression du composeur : l'idiome du
        nuancier. Trois échéances nommées et une saisie libre ; l'envoi programmé se dit ensuite dans
        la ligne d'état, glyphe <code>schedule_send</code>. Ce n'est pas un envoi : le brouillon reste
        un brouillon jusqu'à l'heure dite, et « Annuler » le rend au composeur.</p>
      </div>
      <div class="fiche">
        <h3 class="sourcil">Le refus au plafond : le composeur dit pourquoi, sans avis</h3>
        <div class="puces">
          <span class="puce">${ico('description', 14)}Contrat_Vantis_v4.pdf<span class="poids">1,2 Mo</span>${ico('close', 14)}</span>
          <span style="font-size:12px;color:var(--muted);align-self:center;font-variant-numeric:tabular-nums">1,2 Mo / 25 Mo</span>
        </div>
        <span style="display:inline-flex;align-items:flex-start;gap:9px;font-size:13px;color:var(--alert);line-height:1.5">
          ${ico('warning', 16)}« Presentation_chantier.mp4 » dépasse la place restante (23,8 Mo).</span>
        <p>Le plafond (25 Mo par message) se refuse <b>au geste</b>, jamais à l'envoi : la puce
        n'apparaît pas, le message d'alerte sous la rangée nomme le fichier refusé et la place
        restante. Il s'efface à la pièce suivante acceptée ou au retrait d'une puce. Texte d'alerte
        seul ; la fente d'avis n'en porte rien.</p>
      </div>
      <div class="fiche">
        <h3 class="sourcil">Le transfert : trois états par pièce, l'envoi gardé</h3>
        <div class="puces">
          <span class="puce" style="color:var(--muted)">${ico('hourglass_empty', 14)}Annexe_tarifs.xlsx · rapatriement…</span>
          <span class="puce">${ico('description', 14)}Contrat_Vantis_v4.pdf<span class="poids">1,2 Mo</span>${ico('close', 14)}</span>
          <span class="puce" style="color:var(--alert);border-color:var(--alert)">
            ${ico('warning', 14)}Photos_reunion.zip · Réessayer ${ico('close', 14)}</span>
        </div>
        <p>Chaque pièce d'origine est rapatriée du serveur à l'ouverture et versée au brouillon.
        Trois états : rapatriement (glyphe <code>hourglass_empty</code>, atténué), arrivée (puce
        pleine, retirable), échec (nom en alerte, « Réessayer » et croix de retrait). <b>L'envoi est
        gardé</b> tant que des pièces manquent. En <b>réponse</b>, aucune puce héritée : l'usage du
        courrier ne transmet pas les pièces d'origine.</p>
      </div>
      <div class="fiche">
        <h3 class="sourcil">La confirmation destructive (A57)</h3>
        <div style="display:flex;align-items:center;gap:10px;flex-wrap:wrap;padding:10px 0;border-top:1px solid var(--border)">
          <span style="font-size:13px;color:var(--ink)">Supprimer ce brouillon ? C'est définitif.</span>
          <span style="margin-left:auto;display:flex;gap:8px">
            <span class="btn alerte">${ico('delete', 16)}Supprimer</span>
            <span class="btn">Annuler</span></span>
        </div>
        <p>« Supprimer le brouillon » est le geste <b>destructif volontaire</b>, à droite du pied, en
        teinte d'alerte. Il n'apparaît que s'il y a une matière à jeter, et il <b>passe une
        confirmation</b> : le pied cède la place à la question avant l'irréversible. Il ne se confond
        pas avec « Annuler », qui, lui, conserve — « Fermer = conserver » couvre les pièces.</p>
      </div>
      <div class="fiche">
        <h3 class="sourcil">La ligne « De » (A10, A78)</h3>
        <p>Elle se choisit quand <b>plusieurs comptes</b> existent — sélecteur natif habillé aux jetons
        de la ligne ; à un seul compte, elle reste un <b>texte figé</b>. Depuis A78, le sélecteur dit
        « <b>Nom — adresse</b> » quand le compte porte un nom personnalisé : <b>la valeur reste
        l'adresse</b>, la donnée fonctionnelle d'envoi ne change pas, et le nom ne touche jamais le
        <code>From:</code> des messages sortants.</p>
      </div>
    </div>
  </section>`;
};

// --------------------------------------------------------------------
export const reglages = () => {
  const rangRail = (glyphe, libelle, actif) =>
    `<div class="rang${actif ? ' actif' : ''}">${ico(glyphe, 16)}<span class="l">${libelle}</span></div>`;
  const cadre = (contenu) => `<div class="scene" style="padding:26px;display:flex;justify-content:center">
      <div class="modale" style="width:800px;height:480px;max-width:100%">
        <div class="modale-tete"><span class="t">Réglages</span>
          <span style="margin-left:auto;color:var(--muted)">${ico('close', 16)}</span></div>
        <div style="flex:1;display:flex;min-height:0">${contenu}</div>
        <div class="modale-pied"><span class="btn primaire">Terminé</span></div>
      </div></div>`;
  const rail = (actif) => `<nav class="rail" aria-label="Groupes de réglages">
      ${rangRail('person', 'Comptes', actif === 'comptes')}
      ${rangRail('bookmark', 'Thèmes', actif === 'themes')}
      ${rangRail('display_settings', 'Affichage', actif === 'affichage')}
      ${rangRail('notifications', 'Notifications', actif === 'notifications')}
      ${rangRail('signature', 'Signature', actif === 'signature')}
      ${rangRail('keyboard', 'Raccourcis', actif === 'raccourcis')}
      ${rangRail('info', 'À propos', actif === 'apropos')}
    </nav>`;

  const RACCOURCIS = [
    ['c', 'Écrire un nouveau message'],
    ['r', 'Répondre à la conversation sélectionnée'],
    ['f', 'Transférer la conversation sélectionnée'],
    ['e', 'Archiver la conversation sélectionnée'],
    ['Suppr', 'Supprimer la conversation sélectionnée'],
    ['/', 'Aller à la recherche'],
    ['Échap', 'Fermer la surimpression, sortir du champ, revenir à la boîte'],
  ];

  return `
  <section id="reglages">
    ${haut()}
    <p class="sourcil">Surimpression</p>
    <h2>Réglages</h2>
    <p class="sub">Surimpression de <b>800 × 640</b>, bornée à l'écran, même carte, en-tête 48 px,
    pied « Terminé ». Rail gauche de 220 px : <b>plus de fond de panneau</b> (V3) — un filet le sépare,
    et la rangée active est la seule à porter une surface et l'ombre unique. Volet droit défilant :
    sourcil de section + contenu du groupe. <b>Sept groupes à contenu réel</b>, et les sept sont
    dessinés ci-dessous.</p>

    ${cadre(`${rail('affichage')}
      <div style="flex:1;padding:18px 22px;overflow:hidden;min-width:0">
        <p class="sourcil" style="margin-bottom:6px">Affichage</p>
        <div class="reglage">
          <span class="d"><b>Sombre automatique</b>
            <span>Suivre le réglage sombre du système : la déclinaison nuit du thème choisi s'affiche
            quand il est actif, la claire revient dès qu'il s'éteint. Un thème nuit choisi à la main
            reste tel quel.</span></span>
          <span class="inter arme"><i></i></span>
        </div>
        <div class="reglage">
          <span class="d"><b>Langue</b><span>La langue de l'interface, appliquée immédiatement.</span></span>
          <span class="champ h32" style="width:150px;color:var(--ink)">Français&nbsp;&nbsp;<span style="color:var(--muted)">▾</span></span>
        </div>
        <div class="reglage">
          <span class="d"><b>Disposition</b><span>Le nombre de volets de l'écran principal. En
            dessous de trois, la lecture s'ouvre en plein écran ; en un volet, la navigation vit
            dans un tiroir.</span></span>
          <span class="champ h32" style="width:150px;color:var(--ink)">Trois volets&nbsp;&nbsp;<span style="color:var(--muted)">▾</span></span>
        </div>
      </div>`)}

    <div class="fiches duo">
      <div class="fiche">
        <h3 class="sourcil">Comptes — la rangée est la porte du repère et du nom</h3>
        <div style="border:1px solid var(--border);border-radius:var(--r-controle);overflow:hidden">
          <div style="display:flex;align-items:center;gap:10px;padding:10px 12px;border-bottom:1px solid var(--border)">
            ${repere('work', 'bleu', 'marie@atelier-brindille.fr', 24)}
            <span style="flex:1;min-width:0"><b style="font-size:13px">Atelier Brindille</b><br>
              <span style="font-size:12px;color:var(--muted)">marie@atelier-brindille.fr</span></span>
            <span class="btn nu alerte">${ico('delete', 16)}Retirer le compte</span>
          </div>
          <div style="display:flex;align-items:center;gap:10px;padding:10px 12px;border-bottom:1px solid var(--border)">
            <span style="color:var(--muted);display:inline-flex">${ico('person', 24)}</span>
            <span style="flex:1;font-size:13px">paul@atelier-nord.fr</span>
            <span style="font-size:12px;color:var(--alert)">Déconnecté</span>
            <span class="btn nu">Reconnecter</span>
          </div>
          <div style="padding:10px 12px;display:flex;flex-direction:column;gap:9px">
            <span class="etiqchamp">Icône et couleur du compte</span>
            <span class="bande" style="gap:7px">${['home', 'work', 'school', 'star', 'favorite', 'flight'].map((g) =>
              `<span class="btn icone" style="height:30px;width:30px">${ico(g, 16)}</span>`).join('')}</span>
            <span class="bande" style="gap:7px">${['rouge', 'ocre', 'vert', 'sapin', 'bleu', 'violet'].map((t) =>
              `<span class="rep p24" data-teinte="${t}" title="${t}"></span>`).join('')}</span>
            <span class="bande"><span class="btn nu">Retirer le repère</span>
              <span class="btn nu">${ico('person_add', 16)}Ajouter un compte</span></span>
          </div>
        </div>
        <p>L'icône de la rangée est <b>la porte du repère</b> (A74) : elle montre l'état persisté —
        pastille ou <code>person</code> neutre — et déplie sous la rangée la carte de choix, rangée
        des glyphes puis rangée des teintes. Un repère n'existe qu'<b>entier</b> : le premier choix
        attend son jumeau, ensuite chaque clic applique immédiatement. Le <b>libellé</b> est la porte
        du nom personnalisé (A78). Le retrait porte <b>icône + texte</b> (A77) et se confirme en
        nommant ce qu'il fait : « Retirer marie@… ? Son courrier local est effacé et sa connexion
        oubliée. Rien n'est supprimé sur le serveur. » Un compte déconnecté le dit dans sa rangée et
        offre « Reconnecter ».</p>
      </div>
      <div class="fiche">
        <h3 class="sourcil">Thèmes — deux fiches, et non plus vingt-huit (V7)</h3>
        <div style="display:flex;gap:12px">
          ${Object.entries(THEMES).map(([, t]) => {
            const k = t.jetons;
            return `<span style="flex:1;display:flex;flex-direction:column;gap:7px">
            <span style="display:flex;height:70px;border:1px solid ${k.border};border-radius:var(--r-controle);overflow:hidden;
              ${t.defaut ? `box-shadow:inset 0 0 0 2px ${k.accent}` : ''}">
              <i style="width:22%;background:${k.bg};border-right:1px solid ${k.border}"></i>
              <i style="width:32%;background:${k.bg};border-right:1px solid ${k.border}"></i>
              <i style="flex:1;background:${k.surface}"></i></span>
            <span style="font-size:12px;color:var(--ink2)">${esc(t.libelle)}</span></span>`;
          }).join('')}
        </div>
        <p>Les vignettes montrent chaque thème <b>sans l'appliquer</b>, aux valeurs de la table du
        contrat — jamais un hex recopié. La sélection se dit par le liseré d'accent interne, sans
        décalage. Le thème de base est la seule valeur persistée ; le suffixe <code>-nuit</code> est
        un état dérivé du réglage « Sombre automatique ».</p>
      </div>
      <div class="fiche">
        <h3 class="sourcil">Notifications</h3>
        <div class="reglage" style="border-bottom:0">
          <span class="d"><b>Bulles d'arrivée</b><span>Une bulle système annonce les nouveaux
            messages à la synchronisation. La couper n'arrête jamais la synchronisation
            elle-même.</span></span>
          <span class="inter"><i></i></span>
        </div>
        <p>Un seul réglage, et il ne ment pas sur sa portée : couper la bulle ne coupe pas la relève.
        La préférence vit en base et se lit par le shell à l'émission — elle n'est pas un état de
        l'interface.</p>
      </div>
      <div class="fiche">
        <h3 class="sourcil">Signature — un éditeur par compte (A68)</h3>
        <div style="display:flex;flex-direction:column;gap:10px">
          <span class="champ h32" style="width:100%;color:var(--ink)">paul@atelier-nord.fr&nbsp;&nbsp;<span style="color:var(--muted)">▾</span></span>
          <span class="champ zone">Paul Mérand · Atelier Nord<br>04 78 00 00 00</span>
          <div class="reglage" style="border-bottom:0;padding:6px 0">
            <span class="d"><b>Aussi dans les réponses et transferts</b>
              <span>Sinon, la signature ne s'ajoute qu'aux nouveaux messages.</span></span>
            <span class="inter arme"><i></i></span>
          </div>
          <span class="bande"><span class="btn primaire">${ico('signature', 16)}Enregistrer</span>
            <span class="btn">Appliquer à tous les comptes</span></span>
        </div>
        <p>Une signature <b>par compte</b>, ajoutée au bas des messages composés ; la mise en forme du
        composeur s'y applique. Le sélecteur de compte est natif, habillé aux jetons.</p>
      </div>
      <div class="fiche">
        <h3 class="sourcil">Raccourcis — table en référence, lecture seule</h3>
        <table class="tbl" style="border-radius:var(--r-controle)">
          <thead><tr><th style="width:88px">Touche</th><th>Geste</th></tr></thead>
          <tbody>${RACCOURCIS.map(([k, g]) =>
            `<tr><td class="nw"><span class="touche">${esc(k)}</span></td><td>${g}</td></tr>`).join('')}
          </tbody>
        </table>
        <p>Touches <b>identiques dans toutes les langues</b> : ce sont des positions, pas des mots.
        Et la note qui évite le piège : <b>dans un champ de saisie, les lettres redeviennent des
        lettres</b> ; seul Échap garde un sens.</p>
      </div>
      <div class="fiche">
        <h3 class="sourcil">À propos</h3>
        <div style="display:flex;flex-direction:column;gap:10px">
          <div class="bande">${marqueTuile(40)}<span><b style="font-size:13px">Wind</b><br>
            <span style="font-size:12px;color:var(--muted)">Version 0.8.0</span></span></div>
          <div class="reglage" style="border-bottom:0;padding:6px 0">
            <span class="d"><b>Mises à jour</b><span>Vous êtes à jour.</span></span>
            <span class="btn">Vérifier les mises à jour</span>
          </div>
          <p style="font-size:12px;color:var(--muted)"><b>Icônes</b> — 78 glyphes dessinés dans le
          dépôt, servis en SVG ; aucune police, aucun réseau. <i>Formulation d'attribution à
          arrêter</i> : les dessins dérivent des formes Material Symbols (Apache 2.0) sans en
          redistribuer la police.</p>
        </div>
        <p>La version est la <b>vraie</b>, lue au binaire. La ligne des icônes <b>change avec V8</b> :
        le produit ne livre plus de police, la mention « police embarquée, licence Apache 2.0 » ne dit
        plus ce qui est livré. La formulation exacte reste à trancher — elle est consignée ici pour
        ne pas se perdre.</p>
      </div>
    </div>
    <p class="note">La rangée de réglage : libellé (13 px, 600) et description atténuée ; interrupteur
    <code>role="switch"</code> aux jetons, piste 36 × 20, poignée 16. Les sélecteurs (Langue,
    Disposition, De, compte de signature) sont natifs, habillés aux jetons de la ligne. Clavier
    partout (A8) : chaque rangée du rail est un bouton, anneau de focus accent de 2 px décalé de
    2 px, Échap ferme ; la surimpression porte <code>role="dialog"</code> et
    <code>aria-modal</code>.</p>
  </section>`;
};

// --------------------------------------------------------------------
export const migration = () => {
  const colonne = (contenu) => `<div class="scene" style="padding:34px;display:flex;justify-content:center">
      <div class="colonne">
        <div class="bande">${marqueTuile(28)}<b style="font-size:15px">Wind</b></div>
        ${contenu}
      </div></div>`;

  return `
  <section id="migration">
    ${haut()}
    <p class="sourcil">Écran de démarrage</p>
    <h2>Migration</h2>
    <p class="sub">La modale de migration est la seule surface <b>exclusive et bloquante</b>. Elle
    précède les mises à jour uniques qui gèleraient l'interface si elles se faisaient à la première
    commande : l'adoption d'une base héritée et la reconstruction de l'index de recherche. Son libellé
    est <b>générique</b> — vrai des deux passes et de leur cumul. La géométrie est celle de l'écran 01
    (colonne 520 px sur fond d'application).</p>

    <p class="etiq">1 · La préparation — jauge indéterminée, jamais « 0 % »</p>
    ${colonne(`
        <h3 class="display" style="font-size:32px">Mise à jour de votre boîte.</h3>
        <p style="margin:0;font-size:15px;line-height:1.65;color:var(--ink2)">Préparation…</p>
        <div class="jauge indet"><i></i></div>
        <div style="display:flex"><span style="margin-left:auto"><span class="btn">Annuler</span></span></div>`)}

    <p class="etiq">2 · En cours — le compte réel, et le pourcentage dans le texte</p>
    ${colonne(`
        <h3 class="display" style="font-size:32px">Mise à jour de votre boîte.</h3>
        <p style="margin:0;font-size:15px;line-height:1.65;color:var(--ink2)">Environ 12 480 messages
        sont mis à jour. Cette mise à jour ne se fait qu'une fois et n'efface rien.</p>
        <div class="jauge"><i style="width:62%"></i></div>
        <div style="display:flex;align-items:center;gap:12px">
          <span style="font-size:12px;color:var(--muted);font-variant-numeric:tabular-nums">62 %</span>
          <span style="margin-left:auto"><span class="btn">Annuler</span></span>
        </div>`)}

    <p class="etiq">3 · L'échec — annoncé sans perte, et reprenable</p>
    ${colonne(`
        <h3 class="display" style="font-size:32px">Mise à jour de votre boîte.</h3>
        <p style="margin:0;font-size:15px;line-height:1.65;color:var(--alert);display:flex;gap:9px;align-items:flex-start">
          ${ico('warning', 16)} La mise à jour s'est interrompue. <b>Rien n'est perdu : elle peut être
          relancée.</b></p>
        <div style="display:flex;gap:10px">
          <span class="btn primaire">Reprendre</span><span class="btn">Plus tard</span>
        </div>`)}

    <p class="note">La jauge de 6 px reste <b>indéterminée</b> pendant la préparation, jamais
    « 0 % » : un zéro qui ne bouge pas se lit comme un blocage. « Annuler » interrompt la passe à son
    prochain palier et défait tout ; elle se rejoue au prochain lancement, ou immédiatement par
    « Reprendre ». La jauge reste une <b>jauge</b> et non l'anneau : elle a un dénominateur exact, et
    A52 vaut ici comme ailleurs — un pourcentage se dit en chiffres et en longueur, la signature ne
    porte jamais la mesure. Avec <code>prefers-reduced-motion</code>, la jauge indéterminée cesse de
    glisser et se montre pleine, atténuée.</p>
  </section>`;
};

// --------------------------------------------------------------------
export const avis = () => `
  <section id="avis">
    ${haut()}
    <h2>Avis et progression</h2>
    <p class="sub">Les messages de l'application — mises à jour, incidents, attentes — obéissent à la
    même règle que le reste : <b>trois régions, jamais un empilement</b> (A4).</p>

    <p class="etiq">1 · La fente d'avis, en haut — au plus UN, par priorité fixe</p>
    <div class="fiches">
      <div class="fiche">
        <h3 class="sourcil">Priorité 1 — échec d'envoi</h3>
        <div class="avis"><span style="color:var(--alert);display:inline-flex">${ico('warning', 16)}</span>
          <span style="flex:1">L'envoi « Re : Relecture du contrat Vantis » a été refusé par le serveur.</span>
          <span class="btn nu">Réessayer</span><span style="color:var(--muted)">${ico('close', 16)}</span></div>
        <p>Le seul avis que la boîte d'envoi fait monter : l'<b>attente</b> reste dans la ligne de
        progression, seul l'<b>échec</b> devient un avis.</p>
      </div>
      <div class="fiche">
        <h3 class="sourcil">Priorité 2 — mise à jour prête</h3>
        <div class="avis">${ico('system_update_alt', 16)}
          <span style="flex:1">Wind 0.8.1 est prêt à être installé.</span>
          <span class="btn nu">Installer</span><span style="color:var(--muted)">${ico('close', 16)}</span></div>
        <p>Elle ne s'installe jamais d'elle-même en cours de session : l'avis propose, le
        redémarrage dispose.</p>
      </div>
      <div class="fiche">
        <h3 class="sourcil">Priorité 3 — rapport d'incident</h3>
        <div class="avis"><span style="color:var(--alert);display:inline-flex">${ico('error', 16)}</span>
          <span style="flex:1">Un incident a été consigné. Vous pouvez l'envoyer à l'équipe.</span>
          <span class="btn nu">Envoyer</span><span style="color:var(--muted)">${ico('close', 16)}</span></div>
        <p>L'incident est <b>déjà consigné localement</b> quand l'avis paraît ; l'envoi est un geste
        séparé et facultatif.</p>
      </div>
      <div class="fiche">
        <h3 class="sourcil">Priorité 4 — consentement télémétrie</h3>
        <div class="avis">${ico('volunteer_activism', 16)}
          <span style="flex:1">Autoriser l'envoi de statistiques d'usage anonymes ?</span>
          <span class="btn nu">Autoriser</span><span class="btn nu">Refuser</span></div>
        <p>Le dernier de la file, et le seul sans croix : un consentement se répond, il ne
        s'écarte pas.</p>
      </div>
    </div>
    <p class="note">Au plus <b>un</b> avis persistant à la fois, choisi par cette priorité fixe. L'avis
    est une surface claire avec l'ombre unique. Le suivant n'apparaît qu'une fois le précédent résolu
    ou écarté. La source « brouillons » a été retirée : la reprise d'un brouillon vit au dossier
    Brouillons et dans la conversation, jamais dans la fente.</p>

    <div class="fiches duo">
      <div class="fiche">
        <h3 class="sourcil">2 · La ligne de progression, en bas</h3>
        ${statut({ texte: 'Synchronisation · 2/4 · marie@atelier-brindille.fr · 37 %', bouton: 'Synchronisation…', etat: 'cycle' })}
        <p>Au plus <b>une</b> progression (synchronisation ou rattrapage), en texte atténué, sous
        l'avis en priorité. L'attente de la boîte d'envoi (« N en attente ») vit ici. La signature à
        gauche, le bouton de relève à droite. La ligne reste unique.</p>
      </div>
      <div class="fiche">
        <h3 class="sourcil">3 · Le toast, et les surfaces bloquantes</h3>
        <span class="toast">${ico('check_circle', 16)}Compte ajouté.</span>
        <p>Le <b>toast</b> dit un fait accompli et s'efface seul : surface, filet, coin vif, élévation
        unique, <b>jamais plus d'un</b>, jamais une action irréversible. Au démarrage, deux surfaces
        seulement occupent l'écran, dans cet ordre : la <b>modale de migration</b> (exclusive et
        bloquante) puis, au premier lancement, le <b>parcours d'accueil</b> — lui couvre la fenêtre
        mais ne bloque aucune machine : la synchronisation démarre derrière. En dehors de ces
        deux-là, rien ne bloque jamais. Et <b>rien ne touche la base avant la migration</b> (A41).</p>
      </div>
    </div>
  </section>`;

// --------------------------------------------------------------------
export const ligneMessage = () => `
  <section id="ligne">
    ${haut()}
    <p class="sourcil">États</p>
    <h2>Ligne de message</h2>
    <p class="sub">Deux gabarits — <b>h1 nue</b> et <b>h2 porteuse</b> de son rang de puces (A44) —
    tous deux sondés par la mécanique de fenêtrage. Une ligne qui a quelque chose à dire porte un rang
    de puces de 24 px et s'en agrandit ; il n'y a pas de rang réservé sur les lignes nues. La tuile
    enjambe les rangs du contenu, le rang de puces vit en colonne 2.</p>
    <div style="width:520px;max-width:100%;border:1px solid var(--border);border-radius:var(--r-surface);overflow:hidden">
      ${rangee({ ini: 'SN', exp: 'Sofia Nardi', heure: '4 août', objet: 'Atelier de septembre',
        apercu: 'Nous visons la semaine du 14.' })}
      ${rangee({ ini: 'YB', exp: 'Yanis Belkacem', heure: '08:40', nonlu: true,
        objet: 'Planning de la semaine 33', apercu: 'Deux créneaux se chevauchent mardi.' })}
      ${rangee({ ini: 'LF', exp: 'Léa Fontaine', heure: 'Hier', etat: 'survol',
        objet: 'Compte rendu du 4 août', apercu: 'Trois décisions actées.' })}
      ${rangee({ ini: 'CR', exp: 'Camille Rousseau', heure: '09:12', etat: 'sel',
        objet: 'Relecture du contrat Vantis', apercu: 'Il reste la clause de renouvellement.',
        puces: puce('forum', '3 messages') + puce('attach_file', '2 fichiers') })}
      ${rangee({ ini: 'SN', exp: 'Sofia Nardi', heure: '4 août', epingle: true, etat: 'epingle',
        objet: 'Atelier de septembre', apercu: 'Deux salles réservées à Milan.' })}
    </div>
    <div class="legende" style="margin-top:2px">
      <span>Lu, au repos</span><span>Non lu</span><span>Survol</span>
      <span>Sélectionné (rang porteur)</span><span>Épinglé</span>
    </div>
    <p class="sub"><b>Règles de transmission.</b> Le non-lu se dit par le <b>disque de 9 px</b> et par
    la graisse — jamais par une pastille pleine, jamais par une couleur d'objet. Dans le dossier
    <b>Envoyés</b> (A48), l'expéditeur étant SOI, la ligne dit le <b>destinataire</b> — « À : … »,
    tuile aux initiales du destinataire — tiré de l'À stocké de l'ENVELOPE ; un envoi non encore
    rattrapé retombe sur l'expéditeur, jamais une ligne muette. Le rang de puces est <b>factuel et
    silencieux</b> : il ne porte jamais d'état ni d'alerte — compter, pas signaler. Le survol teinte
    le fond <b>sans déplacer le contenu</b>. La sélection prend <code>--sel</code> et le liseré
    d'accent de 2 px au bord gauche ; jamais d'ombre ni de surface blanche. La rangée <b>épinglée</b>
    prend le sol <code>--tuile</code> et la marque <code>keep</code> (A73) — et sur ce sol, l'accent
    ne sert qu'en liseré, jamais en libellé (mesuré : 4,38:1). Au raccourci <b>e</b> ou <b>Suppr</b>,
    la ligne du dessous devient la sélection (A38) : en trois volets elle ouvre son volet comme au
    clic, en deux ou un volet elle s'allume seulement. Le geste à la souris ne déplace pas la
    sélection. Les puces disent les <b>mêmes règles que la tête du Fil</b> : « n messages » si le fil
    en a plus d'un, « n fichiers » si pièces jointes — 0 tant que le corps n'est pas rapatrié, la puce
    apparaît au fil du rattrapage, jamais à tort. En recherche, un résultat est un message et non une
    conversation : la puce de fil n'y figure pas, par construction.</p>
  </section>`;

// --------------------------------------------------------------------
// Les décisions propres à la v2. Elles ne se numérotent PAS en A-n : le
// journal A1-A78 appartient au document livré, on ne s'y inscrit pas
// depuis une exploration.
export const AMENDEMENTS_V2 = [
  ['2026-08-24', 'V1', `<b>La marque devient l'icône Elements, et le teal entre à la table des jetons.</b>
    L'enveloppe sur tuile au rabat en demi-disque remplace l'enveloppe à pastille « W » ; la pastille
    <b>disparaît</b> — le jeu ne met jamais rien dans un contenant en coin, et le « W » redisait le nom.
    Les deux couleurs de marque restent figées (W-D3) : tuile <code>#F2EDE3</code>, teal
    <code>#1F8A8A</code>. Le teal entre à la table sous le jeton neuf <code>--marque</code>, mesuré
    COMPOSANT (3:1) et jamais texte, et il est dédoublé d'une encre <code>--accent</code>
    <code>#1A7A7A</code> de même teinte pour tout ce qui se lit.`],
  ['2026-08-24', 'V2', `<b>Le trait hitofude meurt ; la paire disque / anneau le remplace.</b>
    Décision du Chef Ingénieur. A28, A36 et A40 tombent — la signature calligraphique n'a pas de place
    dans un système entièrement construit. Le disque plein <code>--marque</code> de <b>9 px</b> dit le
    repos, l'anneau évidé du <b>même diamètre</b> dit qu'une action tourne : ce n'est pas une forme de
    plus, c'est le même disque avec 2 px de paroi et son quart haut ouvert. Deux emplacements, la barre
    d'état et la modale de migration. A52 tient et se renforce : le pourcentage vit dans le TEXTE,
    jamais dans la signature. <b>C'est une perte assumée</b> — le trait avait coûté plusieurs
    chantiers, dont la découverte A40 (SMIL dans le masque).
    <b>Confrontée à six autres propositions le 2026-08-24, et confirmée.</b> Sur demande du Chef
    Ingénieur, sept signatures ont été dessinées et animées à taille réelle dans une vraie barre
    d'état (<code>spikes/direction-elements/v2/signatures.mjs</code>). Verdict : <b>l'anneau est
    gardé</b>. Ce qui a été écarté, et pour quelle raison — de sorte que rien ne se re-propose sans
    raison neuve : le <b>battement</b> du disque gèle sur l'état de repos (A8 rompu) ; le
    <b>balayage</b> se lit comme une mesure (A52 rompu) et ressuscite la barre fine retirée par A36 ;
    le <b>carré au huitième de tour</b> et les <b>quatre coins</b> mettent un contenant là où V4/V14
    réservent le rond à l'état — et le carré déborde de 1,86 px de chaque côté en tournant
    (9 × √2 = 12,73) ; le <b>rabat de la marque</b> retire son orientation à la seule forme orientée
    du jeu. Seule l'<b>alternance plein ↔ évidé</b> tenait les quatre règles comme l'anneau : elle a
    été écartée à l'œil, un clignotement se lisant « alerte ». L'anneau reste le seul des sept qui
    n'ajoute ni forme, ni couleur, ni valeur.`],
  ['2026-08-24', 'V3', `<b><code>--panel</code> meurt : deux sols au lieu de trois.</b> Décision du
    Chef Ingénieur. Navigation, entête, bandeaux et barre d'état cessent d'être en retrait ; le filet
    de 1 px porte SEUL la séparation. Wind reste <b>plat</b> — A29, A30, A46 et A72 tiennent tous : les
    panneaux bordés à rayon 18 du document d'icônes ne sont PAS repris. Le compte de jetons ne bouge
    pas : <code>--panel</code> sort, <code>--marque</code> entre, dix-sept rôles. <b>Coût relevé</b> :
    16 emplois de <code>var(--panel)</code> dans 7 composants, dont <b>deux qui ne sont pas des fonds
    de panneau mais des états</b> — la tuile de date <i>éteinte</i> d'une invitation annulée et le fond
    de la garde d'images : ils retombent sur <code>--bg</code>. Conséquence de bord : le filet, qu'A28
    déclarait décoratif, devient porteur — ce document le <b>mesure</b>, seuil = le filet expédié.`],
  ['2026-08-24', 'V4', `<b>Le rond est rendu au disque.</b> Décision du Chef Ingénieur. Le disque n'a
    plus qu'un emploi dans tout le système : dire l'état. Deux ronds sont donc retirés — l'avatar
    d'initiales devient une <b>tuile carrée</b> (28 px, rayon 2 — porté à <b>0</b> par V14, sol <code>--tuile</code>, encre
    <code>--tuileInk</code>, <b>filet de 1 px</b> : mesurée, la tuile ne vaut que 1,04:1 sur le fond
    clair, sans filet elle n'existe pas), et la <b>pastille pleine de non-lus</b> de la navigation
    devient un <b>nombre</b> en chiffres tabulaires à l'accent. En échange, le non-lu d'une rangée
    gagne un disque de 9 px. <b>Ce qui est renversé, exactement</b> : A29 point 2, qui IMPOSAIT la
    pastille de non-lus pleine dans la nav ; et la règle de prose « jamais par une pastille colorée »
    de la section Ligne de message, qui ne portait aucune référence. A29 visait une pastille qui
    criait ; le disque ne crie pas, il est le même objet que l'anneau de la barre d'état, et il est
    posé sur le centre géométrique de la rangée par construction. La graisse de l'expéditeur et de
    l'objet reste : le non-lu n'est jamais dit par la couleur seule (A8).`],
  ['2026-08-24', 'V5', `<b>Le repère de compte GARDE son glyphe.</b> Décision du Chef Ingénieur,
    contre la doctrine du document d'icônes, qui ne met jamais rien dans un disque. Le fait qui tranche
    est mesuré : aucune des six teintes d'élément ne porte un glyphe à 4,5:1 dans les deux polarités,
    et A74 met un glyphe dans la pastille PRÉCISÉMENT pour que le compte ne soit pas dit par la couleur
    seule (A8, WCAG 1.4.1). Appliquer la direction à la lettre <b>régresserait l'accessibilité</b>.
    Le nuancier passe par <code>.rep[data-teinte]</code> et <b>suit la polarité</b> : une teinte figée
    au clair rendrait son glyphe à <b>2,35:1</b> en nuit — la première rédaction de ce document faisait
    exactement cette faute dans ses propres maquettes, elle est corrigée. Coût conservé et consigné :
    douze glyphes rendus à 10-12 px, donc <b>sous le palier 16 lui-même</b> (voir V9).`],
  ['2026-08-24', 'V6', `<b>Un registre d'affichage, graisse 340.</b> Le titre de conversation (24 px)
    et le hero d'accueil (40 px) passent en graisse 340, interlettrage -.03em — la grammaire du
    document d'icônes. Le principe ne bouge pas : l'autorité reste graduée par la TAILLE, et rien
    au-dessous de 24 px ne change. Repli explicite : sans fonte variable, 340 retombe sur 400 — le
    titre reste un titre, il n'est jamais gras, et aucun dessin ne dépend de la graisse.`],
  ['2026-08-24', 'V7', `<b>Deux thèmes, et deux seulement</b> — renverse A42. Les quatorze combinaisons
    Wada et leurs quatorze nuits sont retirées ; restent « Elements » (défaut, sans attribut) et
    « Elements · nuit ». La table des jetons passe de 476 cellules à 34, l'étape 3 de l'accueil de 28
    fiches à 2, le groupe Thèmes des Réglages de même. Ce qui est perdu : le choix. Ce qui est gagné :
    deux polarités réellement composées et mesurées, au lieu de vingt-huit à tenir. <b>Coût d'adoption
    relevé fichier par fichier</b> à la section Thèmes : sept fichiers, et <b>trois contrôles de la
    gate</b> à revoir — pas seulement le plancher <code>NOMBRE_ATTENDU</code>, comme la première
    rédaction de ce document l'affirmait à tort.`],
  ['2026-08-24', 'V8', `<b>Les glyphes sont dessinés, la fonte meurt.</b> Les 78 glyphes de
    l'inventaire sont redessinés dans la grammaire du document et servis en <b>SVG en ligne</b>.
    Material Symbols Rounded et son sous-ensemble vendorisé disparaissent, et avec eux le lien CDN que
    ce document tirait encore alors que l'application se l'interdit. <b>Effet de bord : l'écart des dix
    glyphes se ferme.</b> Six glyphes étaient livrés sans être dessinés nulle part
    (<code>error</code>, <code>link_off</code>, <code>menu</code>, <code>person_add</code>,
    <code>system_update_alt</code>, <code>volunteer_activism</code>) et quatre étaient dessinés sans
    être employés — A18 n'était plus vrai sur dix glyphes. Le relevé de la section Icônes EST
    désormais l'inventaire, et <b>le générateur refuse de produire ce document si le relevé et le
    catalogue divergent d'un seul glyphe</b>, dans un sens ou dans l'autre : A18 devient une
    assertion. <b>Trois fusions restent à arbitrer</b>. <b>Et une question qui n'est pas de dessin</b> :
    les 78 glyphes sont <i>redessinés d'après</i> les formes Material — la ligne « À propos » qui dit
    « police embarquée, licence Apache 2.0 » doit changer, et la formulation d'attribution reste à
    trancher avant toute adoption.`],
  ['2026-08-24', 'V9', `<b>Dette ouverte : le palier 16 n'est pas dessiné.</b> Le document d'icônes
    impose trois paliers ; les tailles d'emploi de Wind n'en atteignent qu'un — <b>aucune icône du
    produit ne monte à 21 px</b>. Tout tombe dans le palier 16, qui se cale à la main, rectangle par
    rectangle, et le maître ne s'y met pas à l'échelle : <b>37 % seulement</b> de ses coordonnées
    survivent au passage 24 → 16, les 63 % restants atterrissent sur des tiers de pixel. Les douze
    repères de compte, rendus à 10-12 px, passent sous le palier 16 lui-même. <b>Chiffrage : 74
    maîtres faits, 74 paliers 16 à faire, 12 paliers 10-12 à inventer — 160 dessins au total, 46 %
    faits.</b> Ce document montre les maîtres réduits : il ne prétend pas que le palier existe.`],
  ['2026-08-24', 'V10', `<b>Trois rayons, deux formes, et une cote de plateforme — étape
    intermédiaire, dépassée le jour même par V14.</b> 10 px pour les surfaces, 6 px pour les contrôles,
    plus <b>2 px</b> pour la tuile : une tuile n'est ni une surface ni un contrôle, et à 28 px un rayon
    de 6 la rend franchement ronde, elle reprendrait au disque l'unicité que V4 vient de lui rendre.
    <b>La règle vaut pour ce document comme pour le produit</b> — la première rédaction employait huit
    autres valeurs dans ses propres illustrations, elles ont été ramenées aux trois. La <b>pilule</b>
    et le <b>disque</b> sont déclarés <b>formes</b> et non rayons ; le rayon de l'icône d'application
    (15/64) est une cote de <b>plateforme</b>, hors système. <b>Effet de bord assumé</b> : la rangée
    de navigation passe du rayon 8 (A29) au rayon 6 — le système n'a pas de 8. <b>Ce qui reste de
    V10</b> : les trois jetons de forme, la doctrine « pilule et disque ne sont pas des rayons », et
    l'exception de plateforme. <b>Ce qui tombe</b> : les valeurs 10 / 6 / 2, mises à zéro par V14 —
    l'argument de V4, appliqué à moitié ici, l'est jusqu'au bout là.`],
  ['2026-08-24', 'V11', `<b>La marque a deux régimes, et le document dit lequel s'applique où.</b>
    <b>En tuile</b> — icône d'application, accueil, migration, « À propos » — elle est <b>figée hors
    thèmes</b> (W-D3) : structure <code>#141414</code>, tuile <code>#F2EDE3</code>, teal
    <code>#1F8A8A</code>, identiques dans les deux polarités. <b>En glyphe</b> — entête, tiroir — elle
    suit l'encre courante et <code>--marque</code>. Ce n'est pas une entorse à W-D3 mais sa borne : la
    tuile porte son propre sol, un glyphe nu n'en a pas, et un <code>#141414</code> figé posé sur le
    fond nuit serait invisible (1,25:1). La première rédaction appliquait les deux régimes sans les
    énoncer.`],
  ['2026-08-24', 'V12', `<b>Défaut trouvé dans le document livré — et réparé.</b> La ligne du
    journal des amendements qui porte <b>A48</b> n'avait que <b>deux</b> cellules : sa cellule de date
    manquait, et la référence occupait la colonne des dates. Cela ne se voyait pas au rendu — le
    tableau se referme tout seul — mais tout lecteur automatique du journal trébuchait dessus.
    <b>Corrigé dans <code>docs/design/systeme.dc.html</code> le 2026-08-24</b>, sur verdict du Chef
    Ingénieur : une cellule posée, datée <code>2026-08-16</code> d'après le contenu de l'amendement
    (« verdicts D1-D4 du 2026-08-16 ») et au format exact de ses voisines. Une ligne ajoutée, rien
    d'autre touché ; la gate de cohérence reste verte. Les 78 lignes du journal ont désormais leurs
    trois cellules, et ce document n'a plus rien à restituer.`],
  ['2026-08-24', 'V13', `<b>Revue de la v2 par son auteur, et ce qu'elle a corrigé.</b> Le document a
    été relu contre ses propres énoncés, mesures et greps à l'appui. Onze contradictions trouvées et
    corrigées, dont trois qui comptent : (1) <b>la pastille de repère était figée au clair</b> dans les
    maquettes — glyphe à 2,35:1 et pastille à 2,62:1 en nuit, soit exactement la régression
    d'accessibilité que V5 prétend éviter (corrigé par <code>.rep[data-teinte]</code>) ; (2) <b>le coût
    d'adoption était sous-estimé</b> — « seul <code>NOMBRE_ATTENDU</code> » était faux, ce sont sept
    fichiers et trois contrôles de gate (relevé écrit à la section Thèmes) ; (3) <b>la dalle du corps
    de courriel inventait ses valeurs</b> — <code>#1a1a1a</code> et un lien <code>#1a5f7a</code>
    imaginaire, quand <code>mail-render</code> bake <code>#222222</code> sur <code>#ffffff</code> et ne
    pose aucune couleur de lien. Ajoutés au passage : le voile de pièce jointe en <b>recouvrement
    absolu</b> et non en puce de plus (A70), la carte d'<b>invitation</b> (A76) et la <b>recherche</b>
    (A50) que ni ce document ni le livré ne dessinaient, les trois états de la <b>migration</b>, les
    quatre <b>avis</b> de la priorité fixe, les sept groupes des <b>Réglages</b>, le <b>nuancier</b> et
    la <b>carte d'échéance</b> du composeur, les <b>schémas de disposition</b>, les <b>épinglés</b> en
    tête de liste, le <b>toast</b>, le compteur de pied de liste, et la règle
    <code>:focus-visible</code> que le document énonçait sans se l'appliquer.`],
  ['2026-08-24', 'V14', `<b>Les coins passent au DROIT. Zéro rayon.</b> Verdict du Chef
    Ingénieur, sur question posée par lui — « pour être cohérent avec la direction artistique, ne
    faut-il pas passer sur des formes à coins droits ? »
    <b>Ce que la mesure a établi</b> : les 78 glyphes, 219 sous-chemins et 654 commandes de tracé
    n'emploient <b>aucun coin arrondi</b> (miter et butt partout) ; mais les deux documents qui les
    présentent emploient <b>huit rayons différents</b> — 18, 16, 14, 12, 11, 10, 9, 2, plus la pilule.
    La direction a donc une grammaire stricte pour ce qu'elle DESSINE et <b>aucune</b> pour ce qu'elle
    CONTIENT : on ne pouvait pas la recopier, il fallait décider.
    <b>Ce qui a décidé</b> : l'argument de V4 — « le rond dit l'état » — ne supporte pas d'être
    appliqué à moitié. Une carte à 10, un bouton à 6 et une tuile à 2 sont trois degrés de rondeur qui
    ne disent rien et qui diluent le seul rond qui parle. La règle devient plus courte que celle
    qu'elle remplace : <b>zéro rayon ; deux formes rondes, et elles disent quelque chose — le disque
    (l'état, l'identité) et la pilule de l'interrupteur (le glissement)</b>. C'est le geste de V3 :
    tuer un jeton pour laisser un autre porter le sens.
    <b>Ce qui bouge, exactement</b> : surfaces, contrôles, tuiles, cartes, modales, puces, champs,
    onglets, rangées de nav, fenêtre d'application, invitation, toast, avis, nuancier — tous à
    <b>0</b>. La <b>jauge</b> perd sa pilule au passage : une barre de progression n'est pas une forme
    porteuse, c'est un contenant. La bascule de thème de ce document, qui est un contrôle segmenté et
    non une pilule, suit la règle elle aussi. <b>Ne bougent pas</b> : le disque, l'anneau, la pastille
    de repère et la poignée d'interrupteur (50 %), la piste de l'interrupteur (999).
    <b>L'exception, déclarée et permanente</b> : l'icône d'application garde son rayon de plateforme
    (15/64) — c'est l'OS qui le dicte. Elle devient la <b>seule forme arrondie</b> de tout le produit,
    et c'est visible au point le plus regardé : consigné, pas escamoté.
    <b>Le coût, et son verdict.</b> Le seul argument sérieux contre était l'idiome de la plateforme :
    Windows 11 arrondit tout, et une application à coins vifs peut y lire comme étrangère. Il ne se
    jugeait pas sur une planche. <b>Constat terrain du 2026-08-24</b> : le document rendu dans le
    navigateur du Chef Ingénieur, parcouru à l'écran réel — <b>coins droits validés, gardés</b>.
    Réserve nommée, parce qu'un constat vaut ce que porte son support : ce qui a été regardé est le
    <b>document</b>, pas une fenêtre d'application posée à côté d'Explorateur et de Réglages Windows.
    L'idiome Fluent se re-constatera à la première fenêtre livrée ; si elle dit autre chose, le retour
    tient toujours en <b>une ligne</b> — remettre 10 / 6 / 2 aux trois jetons de
    <code>socle.mjs</code>. C'est précisément pour ça qu'ils existent, et ils restent.
    <b>Ce qui a été retiré</b> : la bascule « Coins » qui servait à l'arbitrage. Un Système qui offre
    deux états de sa propre règle est un Système qui n'a pas tranché — la comparaison reste, à la
    section « Les coins », comme trace de ce qui a été écarté et pourquoi.`],
];

export const journal = (rows) => `
  <section id="journal">
    ${haut()}
    <h2>Journal des amendements</h2>
    <p class="sub">Ce journal est une <b>archive de faits datés</b> : il ne se réécrit pas. Les
    soixante-dix-huit amendements A1–A78 sont repris <b>verbatim</b> du document livré
    (<code>docs/design/systeme.dc.html</code>) — les relire est le seul moyen de savoir ce que cette
    v2 renverse et ce qu'elle conserve. Les décisions propres à l'exploration sont consignées en tête,
    sous la référence <b>V-n</b>, et ne prétendent à aucune autorité tant que le Chef Ingénieur n'a pas
    tranché.</p>

    <p class="etiq">Les décisions de la v2 — ${AMENDEMENTS_V2.length} entrées</p>
    <table class="tbl">
      <thead><tr><th style="width:110px">Date</th><th style="width:96px">Réf.</th><th>Décision</th></tr></thead>
      <tbody>
        ${AMENDEMENTS_V2.map(([d, r, t]) => `<tr>
          <td class="nw">${d}</td><th scope="row">${r}</th><td>${t}</td></tr>`).join('\n        ')}
      </tbody>
    </table>

    <p class="etiq">Le journal repris — A1 à A78, verbatim</p>
    <table class="tbl">
      <thead><tr><th style="width:110px">Date</th><th style="width:120px">Réf.</th><th>Amendement</th></tr></thead>
      <tbody>
        ${rows.map((r) => `<tr>
          <td class="nw">${r.date}${r.dateRestituee ? ' <span style="color:var(--alert)" title="date restituée : la cellule manque au document livré (V12)">*</span>' : ''}</td>
          <th scope="row">${r.ref}</th><td>${r.texte}</td></tr>`).join('\n        ')}
      </tbody>
    </table>
    ${rows.some((r) => r.dateRestituee) ? `<p class="note">* Date <b>restituée</b> : la ligne
    correspondante du document livré n'a pas de cellule de date (voir V12).</p>` : `<p class="note">Les
    78 lignes portent leurs trois cellules : le défaut d'A48 relevé en V12 est <b>réparé à la
    source</b>, ce document n'a plus rien à restituer.</p>`}
  </section>`;

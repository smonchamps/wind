// ====================================================================
// Système v2 « Elements » — parties 1 à 9 :
// en-tête, Principes, Marque, Couleurs, Thèmes, Typographie,
// Troncature, Formes/élévation/signature, Kit unifié.
// ====================================================================
import { REPERES } from '../jeu.mjs';
import {
  THEMES, ORDRE_JETONS, EMPLOI_JETON, REPERES_TEINTES,
  banc, bancReperes, rapport, ico, marque, marqueTuile, rayonTuileApp, esc,
} from './socle.mjs';

const J = THEMES.elements.jetons;
const N = THEMES['elements-nuit'].jetons;

export const SECTIONS = [
  ['principes', 'Principes'], ['marque', 'Marque'], ['couleurs', 'Couleurs'],
  ['themes', 'Thèmes'], ['typographie', 'Typographie'], ['troncature', 'Troncature'],
  ['formes', 'Formes et signature'], ['coins', 'Les coins'], ['kit', 'Kit unifié'], ['icones', 'Icônes'],
  ['ecran01', '01 · Accueil'], ['ecran02', '02 · Réception'], ['statut', "Barre d'état"],
  ['ecran03', '03 · Conversation'], ['ecran04', '04 · Composition'],
  ['reglages', 'Réglages'], ['migration', 'Migration'], ['avis', 'Avis'],
  ['ligne', 'Ligne de message'], ['journal', 'Journal'],
];

export const sommaire = () => `
  <nav class="sommaire" aria-label="Sommaire du document">
    ${SECTIONS.map(([id, t]) => `<a href="#${id}">${t}</a>`).join('')}
  </nav>`;

export const haut = () => '<a class="haut" href="#haut">↑ Haut du document</a>';

// --------------------------------------------------------------------
export const enTete = () => `
  <header id="haut">
    <p class="sourcil">Wind · direction « Elements » · v2 — exploration</p>
    <h1 class="display">Système de référence<br>et écrans, v2</h1>
    <p class="lede">Une seule règle de forme, appliquée partout. Grille de 24, trait de 2 unités,
    bouts nets, jonctions vives, coordonnées entières — <b>aucune correction optique</b> dans tout le
    système. Le disque n'a qu'un emploi : il dit l'état. La couleur ne décore jamais ; elle dit
    l'élément à la marque, et un état partout ailleurs. Deux thèmes, une polarité chacun, tous deux
    tirés des couleurs posées autour de l'icône Wind. Ce document est <b>généré</b> depuis le
    catalogue des glyphes : il ne peut pas montrer un dessin différent de celui qui est mesuré.</p>
  </header>

  <div class="expl">
    ${ico('warning', 20)}
    <p class="t"><b>EXPLORATION — ce document n'est pas normatif.</b> Le Système livré reste
    <code>docs/design/systeme.dc.html</code> (direction « Clarity », v2 « Wada », 28 thèmes), seul
    document lu par la gate <code>e2e/coherence-systeme.mjs</code> et seul à faire foi tant que le
    Chef Ingénieur n'a pas tranché. Rien ici n'est livré. Le journal des amendements A1–A78 est repris
    <b>intégralement et verbatim</b> du normatif — ce sont des faits datés, ils ne se réécrivent pas —
    et les décisions propres à cette v2 sont consignées à sa suite, sous la référence <b>V-n</b>.
    <b>Un arbitrage de forme</b> : ce document porte ses couleurs en <b>jetons CSS</b> et non en
    styles en ligne, ce qui lui permet de basculer de thème en O(1) et lui interdit d'écrire un hex
    ailleurs que dans la table du contrat. Le prix est réel : l'éditeur de canevas manipule mal ce
    qu'il ne voit pas en ligne, l'édition élément par élément y perd. Le choix est assumé — un
    Système à deux polarités ne peut pas se prouver avec des couleurs recopiées.</p>
  </div>`;

// --------------------------------------------------------------------
export const principes = () => `
  <section id="principes">
    ${haut()}
    <h2>Principes</h2>
    <p class="sub">Deux gestes venus du Japon guidaient déjà cette direction ; la v2 leur en ajoute un
    troisième, emprunté au jeu d'icônes Elements. Ils ne sont pas des ornements : ils expliquent
    chaque règle du système, et ils s'accordent — les trois disent la même chose, retirer plutôt
    qu'ajouter.</p>
    <div class="fiches">
      <div class="fiche">
        <h3 class="sourcil">Omotenashi, l'hospitalité</h3>
        <h3>Anticiper, puis s'effacer</h3>
        <p>L'hôte prépare tout à l'avance et retire ce qui encombre, pour que l'invité n'ait rien à
        demander. De là viennent la règle des quatre actions au plus, le serveur qui se configure
        seul, et les deux choix visuels offerts une seule fois à l'accueil, jamais redemandés.</p>
      </div>
      <div class="fiche">
        <h3 class="sourcil">Takumi, l'artisan</h3>
        <h3>La justesse par la retenue</h3>
        <p>Le maître atteint la précision en retirant, jamais en ajoutant. De là viennent le filet
        unique de 1 px, l'accent unique, l'élévation unique, et depuis V14 le coin vif. Chaque trait porte
        une intention ; rien n'est décoratif. La matière est chaude, l'ombre tire vers l'encre.</p>
      </div>
      <div class="fiche">
        <h3 class="sourcil">La règle unique (v2)</h3>
        <h3>Une seule forme, zéro arbitrage</h3>
        <p>Le jeu d'icônes Elements ne contient pas une seule correction optique : les marqueurs sont
        posés sur le <b>centre géométrique</b> de leur contenant, et une seule distance est décidée
        dans tout le système — celle qui écarte le disque de Stone — parce qu'elle a une raison
        chiffrée. La v2 transpose la doctrine ligne à ligne : ce qui ne se déduit pas d'une cote se
        supprime, ou se justifie par écrit.</p>
      </div>
    </div>
    <p class="note">Ce que la v2 <b>retire</b> pour tenir ces trois principes ensemble : le trait
    hitofude (A28/A36/A40), le fond de panneau <code>--panel</code>, les 26 thèmes Wada surnuméraires
    (A42), la fonte d'icônes, l'avatar rond et la pastille pleine de non-lus. Chacun de ces retraits
    est consigné au journal, avec ce qu'il coûte.</p>
  </section>`;

// --------------------------------------------------------------------
export const marqueSection = () => `
  <section id="marque">
    ${haut()}
    <h2>Marque</h2>
    <p class="sub">Wind est l'outil courrier et agenda de la suite <b>Elements</b> (« ce que le vent
    porte, le rythme des jours »). La suite pose la règle : <b>l'icône dit la fonction, le disque dit
    l'élément</b>. La marque de Wind est une <b>enveloppe sur tuile</b>, dont le rabat est un
    demi-disque teal — c'est la <b>seule forme orientée</b> de tout le jeu, et elle est placée par
    <b>tangence</b>, jamais par correction optique : le côté plat du demi-disque est tangent au bord
    intérieur haut de l'enveloppe. La pastille « W » en coin <b>disparaît</b> (V1) : le jeu ne met
    jamais rien dans un contenant en coin, et le « W » redisait ce que le nom disait déjà.</p>
    <div class="fiches duo">
      <div class="fiche">
        <h3 class="sourcil">La construction</h3>
        <div style="display:flex;align-items:center;gap:28px">
          ${marqueTuile(96)}
          <p>Grille de 24, tuile pleine <code>#F2EDE3</code>, structure <code>#141414</code>, trait
          <b>2,3</b> au palier maître. Enveloppe <code>M4 8h16v9H4z</code> — quatre coordonnées
          entières. Rabat : demi-disque de rayon <b>3,25</b>, de surface <b>strictement égale</b> au
          disque plein de rayon 2,3 qui marque les cinq autres outils de la suite. Le rayon de la
          tuile est un <b>ratio de plateforme</b> — 15/64 à toute taille, ${rayonTuileApp(96)} px
          ici. Depuis V14 c'est la <b>seule forme arrondie qui reste</b> dans tout le produit, et elle ne
          nous appartient pas : c'est l'OS qui la dicte.</p>
        </div>
      </div>
      <div class="fiche">
        <h3 class="sourcil">Les déclinaisons — figées hors thèmes (W-D3)</h3>
        <div class="bande" style="align-items:flex-end">
          ${[64, 48, 32, 24, 16].map((px) => `<span style="display:flex;flex-direction:column;align-items:center;gap:8px">
            ${marqueTuile(px)}
            <span style="font-size:10px;letter-spacing:.12em;text-transform:uppercase;color:var(--muted)">${px}</span>
          </span>`).join('')}
        </div>
        <p>Trois paliers, comme la suite : le palier 16 (grille de 16, barres pleines de 2 px), le
        palier 24 (trait 2,0 sur coordonnées entières) et le maître (trait 2,3, à partir de 29 px).
        Le teal <code>#1F8A8A</code> et la tuile <code>#F2EDE3</code> sont les <b>seules</b> couleurs
        de la marque ; ce sont elles qui ont donné les deux palettes de cette v2, et elles ne varient
        dans aucun thème. <b>Le palier 16 de l'application n'est pas dessiné</b> — la dette V9.</p>
      </div>
    </div>
    <div class="fiches duo">
      <div class="fiche">
        <h3 class="sourcil">Deux régimes, un seul énoncé (V11)</h3>
        <div class="bande">
          ${marqueTuile(48)}
          <span class="app-entete" style="border:1px solid var(--border);border-radius:var(--r-controle);flex:1;min-width:220px">
            <span class="app-marque" style="width:auto">${marque(20)}<b>Wind</b></span>
          </span>
        </div>
        <p><b>En tuile</b> — icône d'application, écran d'accueil, modale de migration — la marque est
        <b>figée</b> : structure <code>#141414</code>, tuile <code>#F2EDE3</code>, teal
        <code>#1F8A8A</code>, identiques dans les deux polarités. <b>En glyphe</b> — l'entête, le
        tiroir — elle suit l'encre courante et <code>--marque</code>. Ce n'est pas une entorse à
        W-D3, c'est sa borne : la tuile porte son propre sol, un glyphe nu n'en a pas, et un
        <code>#141414</code> figé posé sur le fond nuit serait <b>invisible</b>
        (${rapport('#141414', N.bg).toFixed(2)}:1). La règle s'énonce donc en deux temps, et le
        document dit lequel s'applique où.</p>
      </div>
      <div class="fiche">
        <h3 class="sourcil">La signature — ce qui remplace le trait hitofude (V2)</h3>
        <div class="bande" style="gap:28px">
          <span style="display:inline-flex;align-items:center;gap:10px;font-size:12.5px;color:var(--muted)">
            <span class="disque"></span> au repos</span>
          <span style="display:inline-flex;align-items:center;gap:10px;font-size:12.5px;color:var(--muted)">
            <span class="anneau"></span> pendant un cycle</span>
        </div>
        <p>Le trait calligraphique n'a aucune place dans un système entièrement construit : il est
        <b>mort</b> (A28, A36 et A40 tombent — la découverte du SMIL dans le masque avec). Il est
        remplacé par la <b>paire disque / anneau</b>, qui est déjà la grammaire du jeu : le disque
        plein désigne un objectif atteint, l'anneau évidé le même disque en train de se faire.
        <b>Même diamètre, Ø 9 px</b>, même jeton <code>--marque</code> ; l'anneau n'est pas une forme
        de plus, c'est le disque avec 2 px de paroi et son quart haut ouvert — sans ce vide, la
        rotation ne se verrait pas. Deux emplacements : à gauche de la barre d'état, et en tête de la
        modale de migration. Avec <code>prefers-reduced-motion</code>, l'anneau s'arrête et reste
        évidé. <b>C'est une perte</b> : le trait avait coûté plusieurs chantiers.</p>
        <p><b>Confrontée à six autres, et confirmée</b> (2026-08-24) : battement, alternance
        plein ↔ évidé, carré au huitième de tour, quatre coins, balayage, rabat de la marque — toutes
        dessinées et animées à taille réelle. Deux rompent une règle (A8, A52), trois mettent un
        contenant là où le rond dit l'état, une retire son orientation à la seule forme orientée du
        jeu. <b>L'anneau est le seul des sept qui n'ajoute ni forme, ni couleur, ni valeur</b> — voir
        V2 pour le détail de ce qui a été écarté, et pourquoi.</p>
      </div>
    </div>
  </section>`;

// --------------------------------------------------------------------
export const couleurs = () => `
  <section id="couleurs">
    ${haut()}
    <h2>Couleurs</h2>
    <p class="sub"><b>Dix-sept rôles</b>, comme au Système livré — mais ce ne sont plus tout à fait
    les mêmes : <code>--panel</code> est <b>mort</b> (V3, le fond unique et le filet font tout) et
    <code>--marque</code> est <b>né</b> (V1, le teal exact du jeu d'icônes, composant et jamais
    texte). Toute couleur passe par un jeton ; aucun hex n'est écrit ailleurs que dans cette table —
    à trois exceptions <b>nommées</b> : les deux couleurs figées de la marque en tuile (V1/V11), les
    vingt-quatre teintes du nuancier des repères (A74), et les deux valeurs que
    <code>mail-render</code> bake dans le corps d'un courriel (A61).</p>
    <table class="tbl">
      <thead><tr>
        <th style="width:34px"></th><th style="width:150px">Rôle</th><th style="width:130px">Jeton</th>
        <th style="width:150px">Clair</th><th style="width:150px">Nuit</th><th>Emploi</th>
      </tr></thead>
      <tbody>
        ${ORDRE_JETONS.map((j) => {
          const [role, emploi] = EMPLOI_JETON[j];
          const uni = /^#/.test(J[j]);
          return `<tr>
          <td>${uni ? `<span class="swatch" style="background:${J[j]}"></span>` : ''}</td>
          <th scope="row">${esc(role)}</th>
          <td class="mono">--${j}</td>
          <td class="mono">${esc(J[j])}</td>
          <td class="mono">${esc(N[j])}</td>
          <td>${emploi}</td></tr>`;
        }).join('\n        ')}
      </tbody>
    </table>
    <p class="note"><b>Pourquoi deux teals.</b> Le <code>#1F8A8A</code> du jeu d'icônes vaut
    <b>${rapport('#1F8A8A', J.bg).toFixed(2)}:1</b> sur le fond clair : conforme pour un disque, un
    filet ou un anneau de focus (seuil 3), <b>non conforme pour du texte</b> (seuil 4,5). Il est donc
    gardé <b>exact</b> comme <code>--marque</code> et dédoublé d'une encre <code>--accent</code>
    <code>#1A7A7A</code> — même teinte, luminosité minimale, le remède A8 — qui tient
    <b>${rapport(J.accent, J.surface).toFixed(2)}:1</b> sur la surface. La marque ne porte jamais un
    mot ; l'accent ne porte jamais un disque.</p>
    <p class="note"><b>Deux valeurs de la palette brute ont été corrigées du minimum</b>, à teinte
    constante, avant d'entrer ici : le texte atténué <code>#6E7577</code> → <code>#606668</code>
    (4,19:1 → ${rapport(J.muted, J.bg).toFixed(2)}:1) et le filet <code>#E3E3DD</code> →
    <code>#CBC8BB</code> (1,15:1 → ${rapport(J.border, J.bg).toFixed(2)}:1). Le seuil du filet n'est
    pas inventé : c'est le filet que Wind expédie déjà. Dans un document aéré 1,15:1 tient ; dans une
    liste où le filet est le <b>seul</b> séparateur de rangées, il disparaît.</p>
    <p class="note"><b>Un durcissement assumé.</b> A28 déclare les filets « décoratifs, hors seuils
    du banc : seul le texte se mesure ». Cette v2 les mesure quand même, avec pour seuil le filet
    réellement expédié par Clarity — parce que V3 tue <code>--panel</code> et fait du filet le
    <b>seul</b> séparateur de volets. Un filet qui portait un fond en renfort pouvait rester
    décoratif ; un filet qui porte seul ne le peut plus.</p>
  </section>`;

// --------------------------------------------------------------------
export const themes = () => {
  const mesures = banc();
  const echecs = mesures.filter((m) => !m.ok);
  const reperes = bancReperes();
  const echecsRep = reperes.filter((r) => !r.ok);

  return `
  <section id="themes">
    ${haut()}
    <h2>Thèmes de couleur</h2>
    <p class="sub"><b>Deux thèmes</b>, et deux seulement (V7, renverse A42) : « Elements » — le
    défaut, sans attribut — et sa déclinaison sombre « Elements · nuit », d'identifiant suffixé
    <code>-nuit</code>. Les quatorze combinaisons Wada et leurs quatorze nuits sont <b>retirées</b> :
    la direction est <b>une</b> palette, pas quatorze. Les deux thèmes sortent des couleurs posées
    autour de l'icône Wind — la tuile <code>#F2EDE3</code> devient <code>--tuile</code>, le teal
    <code>#1F8A8A</code> devient <code>--marque</code>, et le reste est composé autour d'eux. Le
    réglage « Sombre automatique » affiche la déclinaison <code>-nuit</code> quand le réglage sombre
    du système est actif ; le thème de base est la seule valeur persistée, le suffixe est un état
    dérivé.</p>

    <p class="etiq">Les deux fenêtres, à la vignette des Réglages</p>
    <div class="fiches duo">
      ${Object.entries(THEMES).map(([, t]) => {
        const k = t.jetons;
        return `<div class="fiche" style="background:${k.bg};border-color:${k.border};color:${k.ink}">
        <h3 class="sourcil" style="color:${k.muted}">${esc(t.libelle)}${t.defaut ? ' · défaut' : ''}</h3>
        <div style="display:flex;height:132px;border:1px solid ${k.border};border-radius:var(--r-controle);overflow:hidden">
          <div style="width:60px;background:${k.bg};border-right:1px solid ${k.border};padding:8px 6px;
            display:flex;flex-direction:column;gap:5px">
            <span style="height:14px;border-radius:var(--r-tuile);background:${k.sel}"></span>
            <span style="height:14px;border-radius:var(--r-tuile);background:${k.tuile}"></span>
            <span style="height:14px;border-radius:var(--r-tuile);background:${k.hover}"></span>
          </div>
          <div style="width:96px;background:${k.bg};border-right:1px solid ${k.border};padding:8px 6px;
            display:flex;flex-direction:column;gap:6px">
            <span style="height:10px;border-radius:var(--r-tuile);background:${k.ink};opacity:.85"></span>
            <span style="height:8px;border-radius:var(--r-tuile);background:${k.ink2};opacity:.5"></span>
            <span style="height:10px;border-radius:var(--r-tuile);background:${k.sel}"></span>
            <span style="height:8px;border-radius:var(--r-tuile);background:${k.ink2};opacity:.5"></span>
          </div>
          <div style="flex:1;background:${k.bg};padding:8px;display:flex;flex-direction:column;gap:6px">
            <span style="height:34px;border-radius:var(--r-controle);background:${k.surface};border:1px solid ${k.border}"></span>
            <span style="height:34px;border-radius:var(--r-controle);background:${k.surface};border:1px solid ${k.border}"></span>
            <span style="width:64px;height:18px;border-radius:var(--r-controle);background:${k.accent}"></span>
          </div>
        </div>
        <div style="display:flex;gap:6px;flex-wrap:wrap">
          ${['bg', 'surface', 'tuile', 'sel', 'hover', 'accent', 'marque', 'alert'].map((j) =>
            `<span class="swatch" title="--${j} ${k[j]}" style="background:${k[j]};border-color:${k.border}"></span>`).join('')}
        </div>
      </div>`;
      }).join('\n      ')}
    </div>

    <p class="etiq">Le contrat des jetons — ce que le code doit livrer, valeur pour valeur</p>
    <table class="tbl">
      <thead><tr><th style="width:130px">Jeton</th>
        ${Object.entries(THEMES).map(([, t]) => `<th>${esc(t.libelle)}</th>`).join('')}
      </tr></thead>
      <tbody>
        ${ORDRE_JETONS.map((j) => `<tr><th scope="row" class="mono" style="font-weight:400">--${j}</th>
          ${Object.keys(THEMES).map((id) =>
            `<td data-theme="${id}" data-jeton="${j}" class="mono">${esc(THEMES[id].jetons[j])}</td>`).join('')}
        </tr>`).join('\n        ')}
      </tbody>
    </table>

    <p class="etiq">Ce que coûte l'adoption — sept fichiers, et trois contrôles de la gate à revoir</p>
    <p class="sub">La forme de la table ci-dessus est <b>celle que la gate sait déjà lire</b>
    (cellules <code>data-theme</code> / <code>data-jeton</code>). Cela ne veut <b>pas</b> dire que la
    gate passerait sans être touchée — la première rédaction de ce document l'affirmait, et c'était
    faux. Le relevé exact :</p>
    <table class="tbl">
      <thead><tr><th style="width:300px">Ce qu'il faut toucher</th><th>Pourquoi</th></tr></thead>
      <tbody>
        <tr><th scope="row"><code>e2e/jetons.mjs</code></th><td><code>NOMBRE_ATTENDU</code> passe de
          <b>28 à 2</b>, et le repli du thème par défaut passe de <code>nature</code> à
          <code>elements</code> — le parseur nomme aujourd'hui le bloc <code>:root</code> sans
          attribut d'après l'ancienne table.</td></tr>
        <tr><th scope="row"><code>e2e/coherence-systeme.mjs</code> · contrôle 2</th><td>Il interdit tout
          compteur de glyphes hors du journal, parce que le contrat vivait ailleurs. V8 tue ce
          contrat : <b>le contrôle devient sans objet</b> et doit être retiré, ou inversé — ce
          document dit « 78 glyphes » et « 24 glyphes », et il a raison de le dire.</td></tr>
        <tr><th scope="row"><code>e2e/coherence-systeme.mjs</code> · contrôle 3</th><td>Il exige le renvoi
          à <code>assets/icones/README.md</code> dans le corps du document. V8 supprime le
          sous-ensemble ; le renvoi n'a plus d'objet, et le relevé de la section Icônes le
          remplace.</td></tr>
        <tr><th scope="row"><code>e2e/coherence-systeme.mjs</code> · contrôles 4 et 5</th><td>Ils
          comparent les <b>28 fiches</b> de <code>lib/theme.js</code> et les clés
          <code>theme.&lt;id&gt;.nom</code> des deux catalogues. Deux fiches, deux clés × deux
          langues.</td></tr>
        <tr><th scope="row"><code>lib/theme.js</code>, <code>catalogue.fr.js</code>,
          <code>catalogue.en.js</code></th><td>Les 28 fiches et leurs libellés. Les pastilles de fiche
          citent <code>panel</code> parmi cinq rôles : <b>V3 le supprime</b>, la forme de
          <code>FICHES</code> change avec.</td></tr>
        <tr><th scope="row"><code>e2e/contraste.mjs</code></th><td>La table de paires perd
          <code>--panel</code> et gagne les cinq paires de la rangée épinglée et les rôles neufs
          (<code>--marque</code>).</td></tr>
        <tr><th scope="row"><code>e2e/tests/refonte-ecran02.spec.js</code></th><td>Il nomme des thèmes de
          la table Wada.</td></tr>
        <tr><th scope="row"><b>16 emplois de <code>var(--panel)</code></b> dans 7 composants</th>
          <td><code>App</code>, <code>Nav</code>, <code>Liste</code>, <code>Fil</code>,
          <code>Composition</code>, <code>Reglages</code>, <code>ModaleMigration</code>. Deux d'entre
          eux ne sont pas des fonds de panneau mais des <b>états</b> — la tuile de date
          <i>éteinte</i> d'une invitation annulée et le fond de la garde d'images : ils retombent sur
          <code>--bg</code>, ce que ce document dessine.</td></tr>
        <tr><th scope="row"><code>reglages.iconesValeur</code> (catalogues)</th><td>La ligne « À propos »
          dit « Material Symbols Rounded (Google), licence Apache 2.0 ; police embarquée ». V8 retire
          la police : la mention doit dire ce qui est réellement livré, et <b>la question de
          l'attribution reste ouverte</b> — les 78 glyphes sont <i>redessinés d'après</i> les formes
          Material, ce qui n'est pas la même chose que les redistribuer. À faire trancher avant toute
          adoption.</td></tr>
      </tbody>
    </table>

    <p class="etiq">Le banc — mêmes formules WCAG, même table de paires que la gate expédiée</p>
    <div class="rangeeH">
      <span class="verdict"><span class="d"></span><b>${mesures.length} mesures, ${echecs.length} échec${echecs.length > 1 ? 's' : ''}</b></span>
      <span class="verdict"><span class="d"></span><b>${reperes.length} repères mesurés, ${echecsRep.length} échec${echecsRep.length > 1 ? 's' : ''}</b></span>
      <span class="note" style="margin:0">Calculé à la génération de ce document, jamais recopié.</span>
    </div>
    <table class="tbl">
      <thead><tr><th style="width:170px">Paire</th><th style="width:80px">Seuil</th>
        <th style="width:110px">Clair</th><th style="width:110px">Nuit</th><th>Où</th></tr></thead>
      <tbody>
        ${(() => {
          const vus = new Set();
          const out = [];
          for (const m of mesures.filter((x) => x.theme === 'elements')) {
            const cle = `${m.encre}/${m.fond}/${m.seuil}`;
            if (vus.has(cle)) continue;
            vus.add(cle);
            const nuit = mesures.find((x) => x.theme === 'elements-nuit'
              && x.encre === m.encre && x.fond === m.fond && x.seuil === m.seuil);
            const ok = m.ok && nuit.ok;
            out.push(`<tr class="${ok ? 'ok' : 'ko'}">
              <th scope="row" class="mono" style="font-weight:400">--${m.encre} / --${m.fond}</th>
              <td class="num">${m.seuil}</td>
              <td class="num">${m.r.toFixed(2)}:1</td>
              <td class="num">${nuit.r.toFixed(2)}:1</td>
              <td>${m.ou}</td></tr>`);
          }
          return out.join('\n        ');
        })()}
      </tbody>
    </table>
    <p class="note"><b>Une règle est sortie de ce banc.</b> La rangée épinglée a <code>--tuile</code>
    pour sol, et l'accent n'y vaut que
    <b>${rapport(J.accent, J.tuile).toFixed(2)}:1</b> en clair — au-dessus du seuil d'un composant
    (3), <b>sous celui d'un texte</b> (4,5). D'où l'interdit, écrit et mesuré : sur la tuile, l'accent
    ne sert qu'en <b>liseré et en anneau de focus</b>, jamais en libellé. Les quatre autres encres y
    passent sans réserve.</p>

    <p class="etiq">Le nuancier des repères de compte (A74) — douze teintes, deux polarités</p>
    <p class="sub">Ce sont des couleurs de <b>contenu</b>, pas des jetons de thème : elles ne suivent
    pas la direction, elles sont <b>mesurées contre elle</b>. Fait re-dérivé au banc : aucune teinte
    unique ne tient 3:1 sur le fond clair <b>et</b> sur le fond nuit — chaque famille vit donc en deux
    déclinaisons, la sombre servie au thème clair (glyphe blanc), la claire au thème nuit (glyphe
    <code>#1c1b1b</code>), par <code>.rep[data-teinte]</code> exactement comme
    <code>systeme.css</code>. <b>Le glyphe montré ci-dessous est arbitraire et identique sur toutes
    les lignes</b> : la teinte et le glyphe se choisissent <b>séparément</b>, aucun couple n'est
    imposé. La pastille est mesurée sur les cinq fonds où elle se pose — la rangée
    (<code>bg</code>, <code>sel</code>, <code>hover</code>, <code>tuile</code>), la carte et le rail
    des Réglages (<code>surface</code>).</p>
    <table class="tbl">
      <thead><tr><th style="width:110px">Teinte</th>
        <th style="width:64px">Clair</th><th style="width:110px"></th><th style="width:120px">Pire fond</th>
        <th style="width:64px">Nuit</th><th style="width:110px"></th><th style="width:120px">Pire fond</th>
        <th>Glyphe sur la pastille</th></tr></thead>
      <tbody>
        ${Object.keys(REPERES_TEINTES).map((nom) => {
          const c = reperes.find((r) => r.nom === nom && r.theme === 'elements');
          const n = reperes.find((r) => r.nom === nom && r.theme === 'elements-nuit');
          const ok = c.ok && n.ok;
          return `<tr class="${ok ? 'ok' : 'ko'}">
          <th scope="row">${esc(nom)}</th>
          <td><span class="rep p24 echantillon" style="background:${c.hex};color:#fff">${ico('home', 14)}</span></td>
          <td class="mono">${c.hex}</td><td class="num">${c.pastille.toFixed(2)}:1</td>
          <td><span class="rep p24 echantillon" style="background:${n.hex};color:#1c1b1b">${ico('home', 14)}</span></td>
          <td class="mono">${n.hex}</td><td class="num">${n.pastille.toFixed(2)}:1</td>
          <td class="num">${c.glyphesur.toFixed(2)}:1 · ${n.glyphesur.toFixed(2)}:1</td></tr>`;
        }).join('\n        ')}
      </tbody>
    </table>
    <p class="etiq">Et les douze glyphes du jeu dédié, indépendants de la teinte</p>
    <div class="bande">
      ${REPERES.map((n) => `<span style="display:flex;flex-direction:column;align-items:center;gap:7px">
        <span style="color:var(--ink)">${ico(n, 24)}</span>
        <code style="font-size:10.5px;color:var(--muted)">${esc(n)}</code></span>`).join('')}
    </div>

    <p class="note"><b>La palette de la suite ne survit pas au double thème, et c'est mesuré.</b>
    Les six teintes d'élément du jeu d'icônes, posées en disque nu sur les cinq fonds de rangée :
    ${(() => {
      const SUITE = { Wind: '#1F8A8A', Stone: '#B0703C', River: '#2153A0', Flame: '#D8332A', Helios: '#E0AE1C', Moon: '#6C4E9C' };
      const fonds = ['bg', 'surface', 'sel', 'hover', 'tuile'];
      return Object.entries(SUITE).map(([nom, hex]) => {
        const c = Math.min(...fonds.map((f) => rapport(hex, J[f])));
        const n = Math.min(...fonds.map((f) => rapport(hex, N[f])));
        const ok = c >= 3 && n >= 3;
        return `<b>${nom}</b> ${c.toFixed(2)} / ${n.toFixed(2)} ${ok ? '✓' : '✗'}`;
      }).join(' · ');
    })()}.
    <b>Helios ne tient pas sur le papier</b> : dans son icône, c'est le <code>#141414</code> qui
    l'entoure qui lui fabrique son contraste ; posé nu, il s'éteint. C'est pourquoi le nuancier des
    repères reste celui d'A74, mesuré, et non la palette de la suite.</p>
  </section>`;
};

// --------------------------------------------------------------------
export const typographie = () => `
  <section id="typographie">
    ${haut()}
    <h2>Typographie</h2>
    <p class="sub">Police système. L'autorité est graduée par la <b>taille</b>, pas par la graisse.
    La v2 ajoute un <b>registre d'affichage</b> emprunté au document d'icônes (V6) : graisse
    <b>340</b>, interlettrage <b>-.03em</b>, réservé aux <b>deux plus grands corps</b> — le titre de
    conversation et le hero d'accueil. Partout ailleurs, rien ne change : 400, 500 ou 600.</p>
    <table class="tbl">
      <thead><tr><th style="width:90px">Taille</th><th style="width:330px">Rôle</th>
        <th style="width:230px">Graisse / interlignage</th><th>Échantillon</th></tr></thead>
      <tbody>
        <tr><th scope="row">11 px</th><td>Sourcil de section, titre de fiche, sections de nav</td>
          <td>600 · interlettrage .2em · capitales</td>
          <td><span class="sourcil">Boîtes</span></td></tr>
        <tr><th scope="row">12 px</th><td>Méta, horodatage, compteurs, barre d'état, étiquette de champ
          (capitales, .1em)</td>
          <td>400 à 600 · chiffres tabulaires</td>
          <td><span style="font-size:12px;color:var(--muted);font-variant-numeric:tabular-nums">09:12</span>
            &nbsp;<span class="etiqchamp">Objet</span></td></tr>
        <tr><th scope="row">13 px</th><td>Aperçu de liste, contrôles, puces d'info</td>
          <td>400 · 1,45</td>
          <td><span style="font-size:13px;color:var(--ink2)">J'ai repris les articles 4 et 7 après notre échange.</span></td></tr>
        <tr><th scope="row">14 px</th><td>Objet dans la liste des messages (non-lus en graisse 700)</td>
          <td>400 / 700 · 1,3</td>
          <td><span style="font-size:14px">Relecture du contrat Vantis</span></td></tr>
        <tr><th scope="row">15 px</th><td>Corps du message, nom d'expéditeur déplié, titre d'invitation</td>
          <td>400 / 600 · 1,65</td>
          <td><span style="font-size:15px">Il reste la clause de renouvellement à trancher.</span></td></tr>
        <tr><th scope="row">16 px</th><td>Titre de boîte (bandeau de liste), mot « Wind »</td>
          <td>600 · 1,3</td>
          <td><span style="font-size:16px;font-weight:600">Boîte de réception</span></td></tr>
        <tr><th scope="row">24 px</th><td>Titre de conversation, volet de lecture — <b>registre d'affichage</b></td>
          <td>340 · 1,2 · -.03em</td>
          <td><span class="display" style="font-size:24px">Relecture du contrat Vantis</span></td></tr>
        <tr><th scope="row">40 px</th><td>Hero d'accueil, usage unique — <b>registre d'affichage</b></td>
          <td>340 · 1,05 · -.03em</td>
          <td><span class="display" style="font-size:40px">Votre adresse</span></td></tr>
      </tbody>
    </table>
    <p class="note">Le registre d'affichage demande une fonte variable (« Segoe UI Variable
    Display » sur Windows, la cible de Wind). Le repli est explicite et documenté : sur une
    plateforme sans fonte variable, la graisse 340 retombe sur 400 — le titre reste un titre, il
    n'est jamais gras. Aucun dessin ne dépend de la graisse.</p>
  </section>`;

// --------------------------------------------------------------------
export const troncature = () => {
  const ligne = (deuxLignes) => `
    <div class="rangee" style="border:1px solid var(--border);border-radius:var(--r-controle);background:var(--surface)">
      <span class="col"><span class="tuileini">CR</span></span>
      <span class="txt">
        <span class="l1"><span class="exp">Camille Rousseau</span><span class="h">09:12</span></span>
        <span class="obj"${deuxLignes ? ' style="white-space:normal;overflow:visible"' : ''}>Relecture du contrat Vantis et de ses annexes tarifaires du second semestre</span>
        <span class="apr">Il reste la clause de renouvellement à trancher.</span>
      </span>
    </div>`;
  return `
  <section id="troncature">
    ${haut()}
    <h2>Troncature des titres</h2>
    <p class="sub">Un titre — objet dans la liste, titre du volet de lecture ou de la conversation —
    ne passe <b>jamais</b> à la ligne : au-delà de la largeur disponible, il se tronque sur une seule
    ligne avec une ellipse finale, comme l'aperçu. La hauteur des lignes reste constante et leur
    alignement régulier ; le titre complet reste lisible dans le volet de lecture.</p>
    <div class="fiches duo">
      <div class="fiche"><h3 class="sourcil">Appliqué</h3>${ligne(false)}
        <p>Une seule ligne, ellipse en fin. La ligne conserve sa hauteur ; l'objet et l'aperçu se
        coupent de la même façon.</p></div>
      <div class="fiche"><h3 class="sourcil">À éviter</h3>${ligne(true)}
        <p>Le titre qui passe à la ligne fait varier la hauteur des lignes et casse le rythme vertical
        de la liste — et il casse aussi le fenêtrage, qui compte des gabarits.</p></div>
    </div>
  </section>`;
};

// --------------------------------------------------------------------
export const formes = () => `
  <section id="formes">
    ${haut()}
    <h2>Formes, élévation, signature</h2>
    <p class="sub">Wind reste <b>plat</b> (V3) : ni carte ni ombre dans la liste, ni dans la
    navigation, ni au volet de lecture. Le document d'icônes est fait de panneaux bordés à rayon 18 —
    la v2 ne les reprend pas : à l'écran d'un client courrier dense, la carte partout redevient une
    page web. Seuls les <b>objets</b> — un message, une invitation, un avis — sont posés sur du
    papier ; et seuls les trois premiers s'élèvent.</p>
    <div class="fiches">
      <div class="fiche">
        <h3 class="sourcil">Zéro rayon — et deux formes qui n'en sont pas</h3>
        <div style="display:flex;gap:18px;align-items:flex-end;flex-wrap:wrap">
          <span style="width:72px;height:52px;border:1px solid var(--border);border-radius:var(--r-surface);background:var(--surface)"></span>
          <span style="width:52px;height:32px;border:1px solid var(--border);border-radius:var(--r-controle);background:var(--surface)"></span>
          <span class="tuileini">CR</span>
          <span class="inter arme"><i></i></span>
          <span class="disque"></span>
        </div>
        <p><b>Zéro rayon</b> (V14). Surfaces, contrôles, tuiles : coin vif partout, dans ce document
        comme dans le produit. La règle tient dans une phrase, et elle se vérifie d'un regard.</p>
        <p>Restent <b>deux formes rondes</b>, et elles ne sont pas des rayons : le <b>disque</b>
        (50 %) dit l'état et l'identité — non-lu, cycle, repère de compte, poignée d'interrupteur — et
        la <b>pilule</b> (999) dit le glissement, à la seule piste de l'interrupteur. Ce sont les
        <b>seules</b> choses rondes du système, et c'est ce qui leur donne leur force : V4 avait rendu
        le rond au disque à moitié, V14 va jusqu'au bout.</p>
        <p>Une exception, déclarée et permanente : le rayon de l'<b>icône d'application</b> (15/64)
        est une cote de <b>plateforme</b>, hors système — c'est l'OS qui la dicte. Les trois jetons
        <code>--r-surface</code> / <code>--r-controle</code> / <code>--r-tuile</code> restent : ils
        tiennent la règle à la place de l'œil, il n'y a plus un seul littéral de rayon à écrire.</p>
      </div>
      <div class="fiche">
        <h3 class="sourcil">Une élévation</h3>
        <div style="height:52px;border-radius:var(--r-surface);background:var(--surface);box-shadow:var(--shadow)"></div>
        <p><code>${esc(J.shadow)}</code> en clair, <code>${esc(N.shadow)}</code> en nuit. Ombre
        chaude, jamais grise. Réservée aux <b>cartes de message</b> du fil, à la <b>carte de
        composition</b>, à la <b>fente d'avis</b>, au <b>toast</b> et à la rangée active du rail des
        Réglages. Le volet de lecture et l'écran 03 sont à plat : jamais d'élévation englobante ; la
        carte d'invitation ne s'élève pas non plus, elle appartient au flot du contenu.</p>
      </div>
      <div class="fiche">
        <h3 class="sourcil">Le disque dit l'état — et rien d'autre (V4)</h3>
        <div style="display:flex;gap:20px;align-items:center;flex-wrap:wrap">
          <span style="display:inline-flex;align-items:center;gap:8px;font-size:12.5px;color:var(--muted)"><span class="disque"></span> non-lu</span>
          <span style="display:inline-flex;align-items:center;gap:8px;font-size:12.5px;color:var(--muted)"><span class="anneau"></span> cycle</span>
          <span style="display:inline-flex;align-items:center;gap:8px;font-size:12.5px;color:var(--muted)"><span class="tuileini">PM</span> identité</span>
          <span style="display:inline-flex;align-items:center;gap:8px;font-size:12.5px;color:var(--muted)"><b style="color:var(--accent);font-variant-numeric:tabular-nums">4</b> compte</span>
        </div>
        <p>Ø <b>9 px</b>, en <code>--marque</code>, deux emplois : le non-lu d'une rangée et
        l'indicateur de la barre d'état. Pour lui rendre son unicité, deux ronds sont retirés :
        l'<b>avatar d'initiales devient un carré</b> (tuile, coin vif depuis V14) et la <b>pastille pleine de
        non-lus de la navigation devient un nombre</b> en chiffres tabulaires, à l'accent. Reste un
        seul autre rond dans tout le système : la pastille de repère de compte, Ø 16 px — et elle
        porte un glyphe, donc elle dit quelque chose que la couleur seule ne dirait pas (A8).</p>
      </div>
      <div class="fiche">
        <h3 class="sourcil">Marges symétriques (A33)</h3>
        <div style="display:flex;gap:8px;flex-wrap:wrap">
          <span class="btn">${ico('reply', 16)}Répondre</span>
          <span class="puce h32">${ico('description', 16)}contrat.pdf</span>
          <span class="btn icone">${ico('close', 16)}</span>
        </div>
        <p>Dans toute puce, tout bouton, tout onglet, le contenu est margé <b>identiquement à gauche
        et à droite</b> : un seul padding horizontal (12 px pour les puces, 14 px pour les boutons et
        onglets). Une icône de tête ou de fin ne réduit jamais la marge de son côté.</p>
      </div>
      <div class="fiche">
        <h3 class="sourcil">Aucune correction optique</h3>
        <p>La règle du jeu d'icônes s'applique à l'interface : un marqueur se pose sur le
        <b>centre géométrique</b> de son contenant, jamais à l'œil. Le disque de non-lu est centré sur
        <b>toute</b> la rangée — rangée nue ou rangée porteuse de puces — par construction, en flex,
        et non par un décalage réglé à la main. Une distance qui ne se déduit pas d'une cote se
        supprime, ou se justifie par écrit dans ce document.</p>
      </div>
      <div class="fiche">
        <h3 class="sourcil">Le filet, unique</h3>
        <div style="display:flex;flex-direction:column;gap:0">
          <span style="height:26px;border-bottom:1px solid var(--border)"></span>
          <span style="height:26px;border-bottom:1px solid var(--border)"></span>
          <span style="height:26px"></span>
        </div>
        <p>1 px, <code>--border</code>, partout, sans exception. Depuis V3 il porte <b>seul</b> la
        séparation des volets, la limite de l'entête et celle de la barre d'état — le fond de panneau
        ne vient plus l'aider. Il borde aussi la tuile d'initiales, qui sans lui n'existerait pas.
        Mesuré : ${rapport(J.border, J.bg).toFixed(2)}:1 sur le fond clair,
        ${rapport(N.border, N.bg).toFixed(2)}:1 sur le fond nuit,
        ${rapport(J.border, J.tuile).toFixed(2)}:1 sur la tuile claire.</p>
      </div>
      <div class="fiche">
        <h3 class="sourcil">Le focus, visible partout (A8)</h3>
        <div class="bande">
          <span class="btn" style="outline:2px solid var(--accent);outline-offset:2px">${ico('reply', 16)}Répondre</span>
          <span class="champ h32" style="width:150px;outline:2px solid var(--accent);outline-offset:2px">Objet</span>
        </div>
        <p>Anneau d'accent de <b>2 px</b>, décalé de <b>2 px</b>, au clavier seulement
        (<code>:focus-visible</code>), jamais supprimé sans remplacement. Il vaut pour tout ce qui se
        focalise : boutons, champs, rangées du rail, poignées de volet, cartes-portes de l'accueil —
        et pour les propres commandes de ce document.</p>
      </div>
    </div>
  </section>`;

// --------------------------------------------------------------------
export const kit = () => `
  <section id="kit">
    ${haut()}
    <h2>Kit unifié</h2>
    <p class="sub">Boutons, onglets de filtre et puces d'info partagent le même <b>coin vif</b>,
    le même filet de <b>1 px</b> et la même hauteur de <b>32 px</b> — une seule exception, nommée : le
    <b>bouton nu</b> (26 px, sans bordure ni fond), réservé aux gestes de la tête du fil et de la
    barre d'état. Les icônes sont <b>dessinées</b> (SVG en ligne, grille 24, trait 2 unités) et non
    plus composées en fonte : voir la section suivante, et l'amendement V8.</p>
    <div class="fiches duo">
      <div class="fiche">
        <h3 class="sourcil">Boutons : repos, survol, appui, désactivé</h3>
        <div style="display:flex;gap:10px;flex-wrap:wrap">
          <span class="btn primaire">${ico('send', 16)}Envoyer</span>
          <span class="btn primaire survol">${ico('send', 16)}Envoyer</span>
          <span class="btn primaire eteint">Envoyer</span>
        </div>
        <div style="display:flex;gap:10px;flex-wrap:wrap">
          <span class="btn">${ico('attach_file', 16)}Joindre</span>
          <span class="btn survol">${ico('attach_file', 16)}Joindre</span>
          <span class="btn appui">${ico('attach_file', 16)}Joindre</span>
          <span class="btn eteint">${ico('attach_file', 16)}Joindre</span>
        </div>
        <p>Un seul bouton primaire par surface. « Continuer » ne s'affiche <b>jamais</b> grisé :
        absent tant qu'il ne peut pas continuer.</p>
      </div>
      <div class="fiche">
        <h3 class="sourcil">Onglets de filtre : actif, repos</h3>
        <div style="display:flex;gap:8px;flex-wrap:wrap">
          <span class="onglet actif">${ico('inbox', 16)}Tous</span>
          <span class="onglet">${ico('mark_email_unread', 16)}Non lus</span>
          <span class="onglet">${ico('edit_note', 16)}Brouillons</span>
        </div>
        <p>L'onglet actif prend la teinte de sélection et passe son glyphe à l'accent. Le glyphe
        <code>mark_email_unread</code> porte le <b>seul disque teal du jeu d'icônes</b> : il dit un
        état, exactement comme le disque de non-lu de la rangée. La cohérence n'est pas un hasard,
        c'est la même règle.</p>
      </div>
      <div class="fiche">
        <h3 class="sourcil">Puces d'info et compteurs</h3>
        <div style="display:flex;gap:8px;flex-wrap:wrap;align-items:center">
          <span class="puce h32">${ico('forum', 16)}3 messages</span>
          <span class="puce h32">${ico('attach_file', 16)}2 fichiers</span>
          <span class="puce h32">${ico('description', 16)}Contrat_Vantis_v4.pdf<span class="poids">1,2 Mo</span></span>
        </div>
        <div style="display:flex;gap:10px;align-items:baseline">
          <b style="font-size:16px;color:var(--accent);font-variant-numeric:tabular-nums">4</b>
          <span style="font-size:12px;color:var(--muted);font-variant-numeric:tabular-nums">/ 18</span>
        </div>
        <p>Une puce porte une seule information, précédée de l'icône de son type. Deux informations
        font deux puces — <b>exception assumée</b> (A59) : la puce d'une pièce jointe regroupe son nom
        ET son poids, un fichier est un seul objet à saisir. Le non-lu est le chiffre héros, le total
        reste subordonné ; ce couple vit au <b>pied de la liste</b>, et depuis V4 le même chiffre
        remplace la pastille pleine de la navigation.</p>
      </div>
      <div class="fiche">
        <h3 class="sourcil">La règle des glyphes — ce qui change avec la fonte (V8)</h3>
        <div style="display:flex;gap:10px;flex-wrap:wrap">
          <span class="rang" style="width:184px;border:1px solid var(--border)">
            ${ico('inbox', 16)}<span class="l">Boîte de réception</span></span>
          <span class="rang actif" style="width:184px">
            ${ico('inbox', 16)}<span class="l">Boîte de réception</span></span>
        </div>
        <p>Corps <b>16 px</b>, <b>toujours à gauche</b> du libellé, en contour, à l'encre secondaire.
        Le <b>remplissage disparaît</b> : la fonte avait un axe FILL, le dessin n'en a pas — et le jeu
        Elements est tout en contour, hors les formes pleines de structure. Le dossier ouvert se dit
        donc par ce qu'il disait déjà : la teinte de sélection sous la rangée, le libellé en graisse,
        et le glyphe passé à l'<b>accent</b>. Un état de moins à dessiner, et la même information.</p>
      </div>
      <div class="fiche">
        <h3 class="sourcil">Interrupteur et champs</h3>
        <div style="display:flex;gap:14px;align-items:center;flex-wrap:wrap">
          <span class="inter arme"><i></i></span><span class="inter"><i></i></span>
          <span class="champ" style="width:200px">Adresse e-mail</span>
          <span class="champ plein" style="width:200px">paul@atelier-nord.fr</span>
        </div>
        <p>Interrupteur <code>role="switch"</code> aux jetons : piste <b>36 × 20</b> en surface et
        filet au repos, accent armé, <b>poignée de 16 px</b> — la cote livrée, en
        <code>--onAccent</code>, le second et dernier emploi de ce jeton. Champs de 40 px à l'accueil,
        32 px en géométrie compacte (Réglages, entête).</p>
      </div>
    </div>
  </section>`;

<script>
  // Écran 03 — la conversation PLEIN ÉCRAN : depuis UI v3 (décision D4
  // du 2026-08-16), un CADRE (entête retour/Écrire + scène) autour de
  // Fil.svelte, le même objet que le volet de lecture. L'exclusivité
  // vit dans le store (`fil.cadre`, revue v3) : ce composant rend
  // quand le cadre est 'plein', point — aucun booléen local à
  // désynchroniser. Agrandir ne recharge rien (`agrandirFil`) ; une
  // sélection directe en 1-2 volets recharge (`ouvrirFil`).
  import Icone from './Icone.svelte';
  import Fil from './Fil.svelte';
  import BarreFil from './BarreFil.svelte';
  import { fil, ouvrirFil, agrandirFil } from './lib/fil.svelte.js';
  import { t } from './lib/texte.svelte.js';
  import { cleLibelleBoite } from './lib/organise.svelte.js';

  let {
    brouillons = [],
    // A80/D5 : le fil dit la boîte derrière le nom — les repères et
    // les noms de compte descendent de l'App, dans les DEUX cadres.
    reperes = {},
    noms = {},
    comptes = [],
    melange = false,
    onreprendre = () => {},
    onretour = () => {},
    onarchiver = () => {},
    onsupprimer = () => {},
    onrepondre = () => {},
    onrepondretous = () => {},
    ontransferer = () => {},
    onspam = () => {},
    onnonspam = () => {},
    estIndesirable = false,
    onecrire = () => {},
    onflash = () => {},
    epinglable = false,
    onepingler = () => {},
    organise = false,
    ondeplacer = () => {},
    oncote = () => {},
  } = $props();

  export function ouvrir(nouvelle) {
    // Le MÊME objet déjà tenu par le volet : changement de taille pur.
    if (fil.ligne && fil.cadre === 'volet'
        && fil.ligne.account_id === nouvelle.account_id
        && fil.ligne.mailbox === nouvelle.mailbox
        && fil.ligne.uid === nouvelle.uid) {
      agrandirFil();
      return Promise.resolve(fil.derniereOuvertureMs);
    }
    return ouvrirFil(nouvelle, 'plein');
  }
  export function estOuverte() {
    return fil.cadre === 'plein';
  }
</script>

{#if fil.cadre === 'plein' && fil.ligne}
  <div class="ecran03" data-testid="conversation">
    <header class="entete">
      <button type="button" class="retour" data-testid="retour-boite" onclick={onretour}>
        <Icone nom="arrow_back" />{t(cleLibelleBoite('reception'))}</button>
      <!-- Terrain 2026-09-02 (CE) : à l'écran 03, les gestes de tri du
           fil vivent DANS la barre d'entête — un seul composant avec
           le volet (BarreFil), dessin « entete ». -->
      <div class="tri">
        <BarreFil dessin="entete" {estIndesirable} {epinglable} {organise}
                  {onarchiver} {onspam} {onnonspam} {onepingler} {ondeplacer} {oncote} />
      </div>
      <button type="button" class="principal" onclick={onecrire}>
        <Icone nom="edit_square" />{t('entete.ecrire')}</button>
    </header>

    <!-- R3 (PLAN-RETOURS-7) : l'écran 03 est À PLAT, comme le volet
         (A46 étendu) — plus de carte englobante : la scène défile en un
         seul flot, seules les cartes de message s'élèvent. Colonne de
         lecture centrée et bornée (D2 : ~960 px). -->
    <div class="scene">
      <div class="colonne">
        <Fil {brouillons} {reperes} {noms} {comptes} {melange} {organise} {ondeplacer} {oncote}
             {onreprendre} {onarchiver} {onsupprimer}
             {onrepondre} {onrepondretous} {ontransferer}
             {onspam} {onnonspam} {estIndesirable} {onflash}
             {epinglable} {onepingler} />
      </div>
    </div>
  </div>
{/if}

<style>
  /* Géométrie de l'écran 03 — l'entête à 52 px depuis UI v3 (E4) :
     les deux cadres du même objet partagent le même chrome, sans saut
     à l'agrandissement (revue v3 : 60 px faisait sauter de 8 px). */
  .ecran03 {
    position:absolute; inset:0; display:flex; flex-direction:column;
    background:var(--bg); z-index:1;
  }
  /* Terrain 2026-09-02 (passe 4, CE) : les gestes de tri s'alignent
     sur le bord GAUCHE de la colonne des messages. L'entête est une
     grille à trois pistes dont la centrale reproduit la colonne de la
     scène (960 px, centrée dans les mêmes gouttières de 28 px) ; le
     retour à gauche, « Écrire » à droite, chacun dans sa piste. */
  .entete {
    height:52px; flex:none; background:var(--surface);
    border-bottom:1px solid var(--border); display:grid;
    grid-template-columns:minmax(auto, 1fr) minmax(0, 960px) minmax(auto, 1fr);
    align-items:center; gap:12px; padding:0 28px;
  }
  .tri { justify-self:start; min-width:0; }
  .entete > .principal { justify-self:end; }
  button {
    height:32px; padding:0 16px; display:inline-flex; align-items:center;
    gap:8px; font-size:13px; color:var(--ink); background:var(--surface);
    border:1px solid var(--border); border-radius:var(--r-controle); cursor:pointer;
  }
  button:hover { background:var(--sel); }
  /* Passe 5 (2026-09-02) : dans la grille, un bouton s'étire sur sa
     piste par défaut — le retour garde sa largeur de contenu. */
  .retour { padding:0 14px; justify-self:start; }
  .principal {
    font-weight:600; color:var(--onAccent); background:var(--accent);
    border-color:var(--accent);
  }
  .principal:hover { background:var(--accentH); border-color:var(--accentH); }

  /* La scène est LE flot (R3) : elle seule défile — le fil est à plat
     dedans, aucune surface, bordure ni ombre englobantes. */
  /* `scrollbar-gutter` des DEUX côtés : la colonne reste centrée dans
     la même largeur que la piste centrale de l'entête, barre de
     défilement ou non — l'alignement tient au pixel. */
  .scene {
    flex:1; padding:18px 28px 28px; overflow-y:auto; min-height:0;
    scrollbar-gutter:stable both-edges;
  }
  .colonne { max-width:960px; margin:0 auto; min-height:100%; display:flex; flex-direction:column; }
</style>

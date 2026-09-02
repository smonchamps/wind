<script>
  // Le volet de lecture de l'écran 02 — depuis UI v3 (verdict CE du
  // 2026-08-16, ANNOTATIONS-V3 §6), le volet montre le FIL en cartes :
  // il n'est qu'un CADRE autour de Fil.svelte, le même objet que
  // l'écran 03 (décision D4). L'exclusivité des cadres est portée par
  // le store (`fil.cadre`, revue v3) : ce composant ne rend l'objet
  // que quand le cadre est le sien — rien à réconcilier à la main.
  import Fil from './Fil.svelte';
  import { fil, ouvrirFil, fermerFil } from './lib/fil.svelte.js';
  import { t } from './lib/texte.svelte.js';

  let {
    brouillons = [],
    // A80/D5 : le fil dit la boîte derrière le nom — les repères et
    // les noms de compte descendent de l'App, dans les DEUX cadres.
    reperes = {},
    noms = {},
    comptes = [],
    melange = false,
    onreprendre = () => {},
    onarchiver = () => {},
    onsupprimer = () => {},
    onconversation = () => {},
    onrepondre = () => {},
    onrepondretous = () => {},
    ontransferer = () => {},
    onspam = () => {},
    onnonspam = () => {},
    estIndesirable = false,
    onflash = () => {},
    organise = false,
    ondeplacer = () => {},
    oncote = () => {},
    epinglable = false,
    onepingler = () => {},
  } = $props();

  export function ouvrir(nouvelle) {
    return ouvrirFil(nouvelle, 'volet');
  }
  export function fermer() {
    fermerFil();
  }
  export function etat() {
    return { derniereOuvertureMs: fil.derniereOuvertureMs };
  }
</script>

<main aria-label={t('lecture.aria')} data-testid="volet-lecture">
  {#if fil.cadre === 'volet' && fil.ligne}
    <Fil {brouillons} {reperes} {noms} {comptes} {melange} {organise} {ondeplacer} {oncote}
         {onreprendre} {onarchiver} {onsupprimer}
         {onrepondre} {onrepondretous} {ontransferer}
         {onspam} {onnonspam} {estIndesirable} {onflash}
         {epinglable} {onepingler}
         onagrandir={onconversation} />
  {:else if fil.cadre !== 'plein'}
    <p class="vide">{t('lecture.vide')}</p>
  {/if}
</main>

<style>
  /* Le volet est À PLAT (terrain A46, .voletLecture du prototype) :
     un panneau qui défile en un seul flot — l'élévation appartient aux
     cartes de message, jamais au volet entier. */
  main {
    /* Terrain 2026-09-02 (passe 4) : PAS de padding haut sur le cadre
       qui défile — un `position:sticky` se borne au bord du CONTENU du
       cadre, sous son padding : la barre collée du fil s'arrêtait 18 px
       sous le haut visible et le message passait dans la bande. L'air
       du haut est rendu par le fil (--fil-haut). */
    background:var(--bg); padding:0 22px 18px; --fil-haut:18px; min-width:0;
    display:flex; flex-direction:column; min-height:0; overflow-y:auto;
    /* RETOURS-14 R1 (revue) : la barre du fil est collante avec un
       z-index — sans contexte d'empilement ici, elle passerait
       AU-DESSUS des voiles modaux (Composition/Réglages, z-index 2). */
    isolation:isolate;
  }
  .vide {
    margin:auto; font-size:13px; line-height:1.5; color:var(--muted);
    text-align:center; padding:40px;
  }
</style>

<script>
  // LA barre du fil (RETOURS-14 R1/D1 ; A56, A58, A73, A96) : les
  // gestes de TRI de la conversation — Archiver, Signaler comme spam /
  // « Ce n'est pas un spam », Épingler ; en mode organisé, Mettre de
  // côté et « Déplacer vers… ». UN composant, DEUX dessins (terrain
  // 2026-09-02, passe 3 du STOP 2 de la vague 2, verdict CE) : au
  // VOLET, collée sous l'entête du fil — bande à plat sur le fond,
  // filet, collante en tête au défilement (R1) ; à l'ÉCRAN 03, ses
  // boutons vivent directement dans la barre d'entête de la scène
  // (Conversation.svelte), entre le retour et « Écrire ».
  import Icone from './Icone.svelte';
  import Menu from './Menu.svelte';
  import { fil, estEcho } from './lib/fil.svelte.js';
  import { t } from './lib/texte.svelte.js';

  let {
    dessin = 'volet',
    estIndesirable = false,
    epinglable = false,
    organise = false,
    onarchiver = () => {},
    onspam = () => {},
    onnonspam = () => {},
    onepingler = () => {},
    ondeplacer = () => {},
    oncote = () => {},
  } = $props();

  // Le menu « Déplacer vers… » se referme au changement de ligne — sans
  // ce reflet, le menu du fil A resterait ouvert au-dessus du fil B et
  // un clic distrait routerait B.
  let menuDeplacer = $state(false);
  $effect(() => {
    void fil.ligne;
    menuDeplacer = false;
  });
</script>

<div class="actions" class:volet={dessin === 'volet'} class:entete={dessin === 'entete'}
     data-testid="barre-fil">
  <button type="button" data-testid="archiver" onclick={() => onarchiver(fil.ligne)}>
    <Icone nom="archive" />{t('action.archiver')}</button>
  {#if estIndesirable}
    <button type="button" data-testid="pas-spam" onclick={() => onnonspam(fil.ligne)}>
      <Icone nom="inbox" />{t('action.pasSpam')}</button>
  {:else}
    <button type="button" data-testid="signaler-spam" onclick={() => onspam(fil.ligne)}>
      <Icone nom="report" />{t('action.signalerSpam')}</button>
  {/if}
  <!-- R4 (PLAN-RETOURS-7) : épingler LA conversation — bascule
       dite par son libellé ET aria-pressed ; l'état vient du cœur
       (pin_state) et suit le geste. Jamais sur un écho. -->
  {#if epinglable && !estEcho(fil.ligne)}
    <button type="button" data-testid="epingler" aria-pressed={fil.epingle}
            onclick={() => onepingler(fil.ligne)}>
      <Icone nom={fil.epingle ? 'keep_off' : 'keep'} />
      {fil.epingle ? t('action.desepingler') : t('action.epingler')}</button>
  {/if}
  <!-- PLAN-MODE-ORGANISE E1 : le routage manuel — un expéditeur,
       une destination. Jamais sur un écho (pas d'enveloppe). Sans
       glyphe : aucun dessin existant ne porte ce sens (A3), le
       texte suffit dans la barre. -->
  {#if organise && !estEcho(fil.ligne)}
    <!-- E5 : la bascule de la pile — l'état est SEMÉ de la ligne
         servie (patron de l'épingle, revue 2026-08-21 : jamais un
         aller-retour par ouverture) et suit le geste (App, jeton
         du store) ; le geste remonte à l'App, qui possède la
         commande. -->
    <button type="button" data-testid="mettre-de-cote"
            aria-pressed={fil.cote}
            onclick={() => oncote(fil.ligne)}>
      <Icone nom={fil.cote ? 'keep_off' : 'pile'} />
      {fil.cote ? t('pile.reprendre') : t('pile.mettre')}</button>
    <span class="deplacer">
      <button type="button" data-testid="deplacer-vers"
              aria-haspopup="menu" aria-expanded={menuDeplacer}
              onclick={() => (menuDeplacer = !menuDeplacer)}>
        {t('action.deplacerVers')}</button>
      <Menu ouvert={menuDeplacer} testid="deplacer-menu" largeur={170} absolu
            onfermer={() => (menuDeplacer = false)}>
          {#each ['reception', 'kiosque', 'registre'] as dest (dest)}
            <button type="button" role="menuitem"
                    data-testid={`deplacer-${dest}`}
                    onclick={() => { menuDeplacer = false; ondeplacer(fil.ligne, dest); }}>
              {t(`boite.${dest}`)}</button>
          {/each}
        </Menu>
    </span>
  {/if}
</div>

<style>
  .actions { display:flex; gap:12px; flex-wrap:wrap; align-items:center; }
  /* Volet : collée sous l'entête du fil, à plat sur le fond, le filet
     la ferme ; collante en tête au défilement (R1, D1) — z-index
     au-dessus des cartes élevées. */
  .actions.volet {
    flex:none; padding:6px 0 12px; position:sticky; top:0; z-index:4;
    background:var(--bg); border-bottom:1px solid var(--border);
  }
  /* Entête de l'écran 03 : en ligne dans la barre, rien de plus. */
  .actions.entete { flex:none; gap:8px; }
  .actions button {
    height:32px; padding:0 16px; display:inline-flex; align-items:center;
    gap:8px; font-size:13px; color:var(--ink); background:var(--surface);
    border:1px solid var(--border); border-radius:var(--r-controle); cursor:pointer;
  }
  .actions button:hover { background:var(--sel); }
  .deplacer { position:relative; display:inline-flex; }
</style>

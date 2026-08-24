<script>
  // La marque Elements (V1/V11 — PLAN-ELEMENTS) : l'enveloppe au rabat
  // en demi-disque, verbatim du document d'icônes (trait 2,3 ; arc
  // r 3.25 tangent au bord intérieur haut). DEUX régimes, et le
  // document dit lequel s'applique où (V11) :
  //   - EN TUILE (`tuile`) — icône d'application, accueil, migration,
  //     « À propos » : FIGÉE hors thèmes (W-D3) — structure #141414,
  //     tuile #F2EDE3, teal #1F8A8A, identiques dans les deux
  //     polarités. Le rayon est une cote de PLATEFORME (15/64), la
  //     SEULE forme arrondie du produit (V14, exception déclarée).
  //   - EN GLYPHE (défaut) — entête, tiroir : l'enveloppe suit l'encre
  //     courante, le rabat prend --marque. Un #141414 figé posé sur le
  //     fond nuit serait invisible (1,25:1) — c'est la borne de W-D3,
  //     pas une entorse.
  import { MARQUE } from './lib/icones.js';

  let { taille = 20, tuile = false } = $props();
  const rayon = $derived(Math.max(2, Math.round((taille * 15) / 64)));
  const trait = $derived(tuile && taille <= 16 ? 2 : MARQUE.trait);
</script>

{#if tuile}
  <span class="marque-tuile" aria-hidden="true"
        style="width:{taille}px; height:{taille}px; --r-plateforme:{rayon}px">
    <svg viewBox="0 0 24 24" width={taille} height={taille}>
      <rect width="24" height="24" fill="#F2EDE3" />
      <g fill="none" stroke="#141414" stroke-width={trait}
         stroke-linecap="butt" stroke-linejoin="miter">
        {#each MARQUE.d as d (d)}<path {d} />{/each}
      </g>
      <path d={MARQUE.flap} fill="#1F8A8A" />
    </svg>
  </span>
{:else}
  <svg class="ic" data-nom="marque" viewBox="0 0 24 24"
       width={taille} height={taille} aria-hidden="true">
    <g fill="none" stroke="currentColor" stroke-width={MARQUE.trait}
       stroke-linecap="butt" stroke-linejoin="miter">
      {#each MARQUE.d as d (d)}<path {d} />{/each}
    </g>
    <path d={MARQUE.flap} fill="var(--marque)" />
  </svg>
{/if}

<style>
  .marque-tuile {
    display:inline-flex; overflow:hidden; flex:none;
    border-radius:var(--r-plateforme);
  }
  .marque-tuile svg { display:block; }
</style>

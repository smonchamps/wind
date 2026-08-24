<script>
  // L'icône « Elements » (V8 — PLAN-ELEMENTS) : un SVG en ligne du
  // catalogue (lib/icones.js), à l'encre courante (currentColor) — le
  // disque d'état et la barre de couleur prennent --marque, seuls
  // éléments colorés légitimes du jeu. `taille` en px : 16 est la
  // taille d'emploi de tout le produit ; les contextes plus petits la
  // posent en CSS (width/height sur .ic), qui prime sur l'attribut.
  // `miroir` : « Transférer » porte la flèche de « Répondre » en
  // symétrie verticale (A12). `data-nom` est la couture des tests et
  // de la gate de cohérence. Un nom inconnu rend un SVG vide — visible
  // à l'œil et au data-nom, jamais un crash.
  import { JEU } from './lib/icones.js';

  let { nom, taille = 16, miroir = false } = $props();
  const g = $derived(JEU[nom] ?? { d: [] });
</script>

<svg class="ic" class:miroir data-nom={nom} viewBox="0 0 24 24"
     width={taille} height={taille} aria-hidden="true">
  <g fill="none" stroke="currentColor" stroke-width="2"
     stroke-linecap="butt" stroke-linejoin="miter">
    {#each g.d as d (d)}<path {d} />{/each}
  </g>
  {#if g.barre}
    <path d={g.barre} fill="none" stroke="var(--marque)" stroke-width="2"
          stroke-linecap="butt" />
  {/if}
  {#if g.disque}
    <circle cx={g.disque[0]} cy={g.disque[1]} r={g.disque[2]}
            fill="var(--marque)" />
  {/if}
  {#each g.pleins ?? [] as [cx, cy, r] (`${cx},${cy}`)}
    <circle {cx} {cy} {r} fill="currentColor" />
  {/each}
  {#each g.remplis ?? [] as d (d)}
    <path {d} fill="currentColor" stroke="none" />
  {/each}
</svg>

<script>
  // The Elements brand (V1/V11 — PLAN-ELEMENTS): the envelope with a
  // half-disc flap, verbatim from the icon document (stroke 2.3; arc
  // r 3.25 tangent to the top inner edge). TWO regimes, and the
  // document states which applies where (V11):
  //   - AS A TILE (`tile`) — app icon, onboarding, migration,
  //     “About”: FIXED outside themes (W-D3) — structure #141414,
  //     tile #F2EDE3, teal #1F8A8A, identical in both polarities. The
  //     radius is a PLATFORM measurement (15/64), the product's ONLY
  //     rounded shape (V14, declared exception).
  //   - AS A GLYPH (default) — header, drawer: the envelope follows
  //     the current ink, the flap takes --marque. A fixed #141414 laid
  //     on the night background would be invisible (1.25:1) — that is
  //     W-D3's bound, not a breach.
  import { BRAND } from './lib/icons.js';

  let { size = 20, tile = false } = $props();
  const radius = $derived(Math.max(2, Math.round((size * 15) / 64)));
  const stroke = $derived(tile && size <= 16 ? 2 : BRAND.stroke);
</script>

{#if tile}
  <span class="marque-tuile" aria-hidden="true"
        style="width:{size}px; height:{size}px; --r-plateforme:{radius}px">
    <svg viewBox="0 0 24 24" width={size} height={size}>
      <rect width="24" height="24" fill="#F2EDE3" />
      <g fill="none" stroke="#141414" stroke-width={stroke}
         stroke-linecap="butt" stroke-linejoin="miter">
        {#each BRAND.d as d (d)}<path {d} />{/each}
      </g>
      <path d={BRAND.flap} fill="#1F8A8A" />
    </svg>
  </span>
{:else}
  <svg class="ic" data-nom="marque" viewBox="0 0 24 24"
       width={size} height={size} aria-hidden="true">
    <g fill="none" stroke="currentColor" stroke-width={BRAND.stroke}
       stroke-linecap="butt" stroke-linejoin="miter">
      {#each BRAND.d as d (d)}<path {d} />{/each}
    </g>
    <path d={BRAND.flap} fill="var(--marque)" />
  </svg>
{/if}

<style>
  .marque-tuile {
    display:inline-flex; overflow:hidden; flex:none;
    border-radius:var(--r-plateforme);
  }
  .marque-tuile svg { display:block; }
</style>

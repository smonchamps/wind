<script>
  // The “Elements” icon (V8 — PLAN-ELEMENTS): an inline SVG from the
  // catalogue (lib/icons.js), in the current ink (currentColor) —
  // the status dot and the color bar take --brand, the set's only
  // legitimate colored elements. `size` in px: 16 is the whole
  // product's default size; smaller contexts set it in CSS
  // (width/height on .ic), which overrides the attribute. `mirror`:
  // “Forward” carries “Reply”'s arrow in vertical symmetry (A12).
  // `data-name` is the seam for the tests and the coherence gate. An
  // unknown name renders an empty SVG — visible to the eye and to
  // data-name, never a crash.
  import { SET } from './lib/icons.js';

  let { name, size = 16, mirror = false } = $props();
  const g = $derived(SET[name] ?? { d: [] });
</script>

<svg class="ic" class:mirror={mirror} data-name={name} viewBox="0 0 24 24"
     width={size} height={size} aria-hidden="true">
  <g fill="none" stroke="currentColor" stroke-width="2"
     stroke-linecap="butt" stroke-linejoin="miter">
    {#each g.d as d (d)}<path {d} />{/each}
  </g>
  {#if g.bar}
    <path d={g.bar} fill="none" stroke="var(--brand)" stroke-width="2"
          stroke-linecap="butt" />
  {/if}
  {#if g.dot}
    <circle cx={g.dot[0]} cy={g.dot[1]} r={g.dot[2]}
            fill="var(--brand)" />
  {/if}
  {#each g.dots ?? [] as [cx, cy, r] (`${cx},${cy}`)}
    <circle {cx} {cy} {r} fill="currentColor" />
  {/each}
  {#each g.filled ?? [] as d (d)}
    <path {d} fill="currentColor" stroke="none" />
  {/each}
</svg>

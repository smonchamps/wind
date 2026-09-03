<script>
  // RETOURS-14 R9 (field pass 2026-08-31, 2nd pass): THE sort button
  // of a section — it opens a dropdown MENU of the four sorts (most
  // recent / oldest / sender A → Z / Z → A), each entry with its
  // glyph (`tri_*`, A104 — set 87 → 91). The button follows the
  // drawing of the bare “Expand all” buttons (Thread) and is
  // right-aligned by the ROW hosting it; the state lives with the
  // caller — four surfaces, ONE component (D-47 lesson). The menu is
  // the product's menu drawing.
  import Icon from './Icon.svelte';
  import Menu from './Menu.svelte';
  import { t } from './lib/text.svelte.js';

  let { value = 'date-desc', onchange = () => {} } = $props();

  const SORTS = [
    { id: 'date-desc', label: 'sort.dateDesc', icon: 'sort_newest' },
    { id: 'date-asc', label: 'sort.dateAsc', icon: 'sort_oldest' },
    { id: 'alpha-az', label: 'sort.alphaAz', icon: 'sort_az' },
    { id: 'alpha-za', label: 'sort.alphaZa', icon: 'sort_za' },
  ];
  const current = $derived(SORTS.find((x) => x.id === value) ?? SORTS[0]);

  let isOpen = $state(false);
  let x = $state(0);
  let y = $state(0);
  function toggle(e) {
    e.stopPropagation();
    if (isOpen) {
      isOpen = false;
      return;
    }
    const r = e.currentTarget.getBoundingClientRect();
    x = r.right - 230;
    y = r.bottom + 4;
    isOpen = true;
  }
  function choose(id) {
    isOpen = false;
    onchange(id);
  }
</script>

<button type="button" class="bare" data-testid="sort-section"
        title={t('sort.aria')} aria-label={t('sort.aria')}
        aria-haspopup="menu" aria-expanded={isOpen}
        onclick={toggle}>
  <Icon name={current.icon} />{t(current.label)}</button>

<Menu isOpen={isOpen} x={x} y={y} testid="sort-menu" width={220}
      onclose={() => (isOpen = false)}>
    {#each SORTS as choice (choice.id)}
      <button type="button" role="menuitemradio" data-testid={`sort-${choice.id}`}
              aria-checked={choice.id === value}
              onclick={() => choose(choice.id)}>
        <Icon name={choice.icon} />{t(choice.label)}</button>
    {/each}
  </Menu>

<style>
  /* The exact drawing of the thread's bare button (“Expand all”) —
     copied here for lack of a global class; if a third copy appears,
     promote it to system.css (.header-view pattern). */
  .bare {
    height:26px; padding:0 9px; display:inline-flex; align-items:center;
    gap:6px; font-size:12px; color:var(--ink2); background:none;
    border:1px solid transparent; border-radius:var(--r-control); cursor:pointer;
    white-space:nowrap; flex:none;
  }
  .bare:hover, .bare[aria-expanded="true"] { background:var(--sel); }
</style>

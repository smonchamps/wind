<script>
  // RETOURS-14 R6 (D7): the Paper trail GROUPED by sender — one row
  // per routed sender, sorted by recency of the last message (the
  // Cleanup's pattern, never the alphabet — D7), expanding onto its
  // threads. The data stays the routed flow (`paper_trail_groups` /
  // `paper_trail_group_page`, same bounds as the flat view); opening a
  // thread goes through the list's path (`onopen` → App), the
  // reading pane stays the reader.
  import Icon from './Icon.svelte';
  import Menu from './Menu.svelte';
  import SectionSort from './SectionSort.svelte';
  import { call } from './lib/transport.js';
  import { sortComparator } from './lib/sort.js';
  import { when } from './lib/when.js';
  import { t } from './lib/text.svelte.js';
  import { activation } from './lib/keyboard.js';

  let {
    account = null,
    onopen = () => {},
    ontotal = () => {},
    // Review: sender gestures do not die with the flat view — the ⋯
    // of a group routes the ENTIRE sender (Move to…, Screen out); the
    // App owns the command and the toasts.
    onroute = () => {},
  } = $props();

  const PAGE = 200;

  let groups = $state([]);
  let served = $state(false);
  let isOpen = $state(null);
  let openMessages = $state([]);
  // Review: a group's page is PAGINATED — “See more” loads the rest,
  // never a silent truncation at 200 while the row announces the true
  // count.
  let morePossible = $state(false);
  let loadingMore = $state(false);
  // The ⋯ of a group: { address, who, x, y } (pattern of the
  // product's menus — D-47 family, recorded).
  let menu = $state(null);
  // R9: the section sort — recency by default (D7), the button
  // cycles; the order served by the core stays recency, the sort is a
  // PRESENTATION (the groups are already all there).
  let sort = $state('date-desc');
  let token = 0;
  const sortedGroups = $derived(
    [...groups].sort(sortComparator(sort, (g) => g.lastEpoch, (g) => g.who ?? g.address)),
  );

  async function load() {
    const j = (token += 1);
    try {
      const g = await call('paper_trail_groups', { accountId: account });
      if (j !== token) return;
      groups = g;
      served = true;
      ontotal(g.reduce((n, x) => n + x.threads, 0));
      // The expanded group may have disappeared (verdict removed elsewhere).
      if (isOpen && !g.some((x) => x.address === isOpen)) {
        isOpen = null;
        openMessages = [];
      }
    } catch (err) {
      console.error('paper_trail_groups :', err);
    }
  }

  export function reload() {
    load();
  }

  $effect(() => {
    void account;
    served = false;
    isOpen = null;
    openMessages = [];
    load();
  });

  async function toggleGroup(address) {
    if (isOpen === address) {
      isOpen = null;
      openMessages = [];
      morePossible = false;
      return;
    }
    isOpen = address;
    openMessages = [];
    morePossible = false;
    try {
      const rows = await call('paper_trail_group_page', {
        address, accountId: account, offset: 0, limit: PAGE,
      });
      if (isOpen === address) {
        openMessages = rows;
        morePossible = rows.length === PAGE;
      }
    } catch (err) {
      console.error('paper_trail_group_page :', err);
    }
  }
  async function loadMore() {
    if (!isOpen || loadingMore) return;
    loadingMore = true;
    const address = isOpen;
    try {
      const rows = await call('paper_trail_group_page', {
        address, accountId: account, offset: openMessages.length, limit: PAGE,
      });
      if (isOpen === address) {
        openMessages = [...openMessages, ...rows];
        morePossible = rows.length === PAGE;
      }
    } catch (err) {
      console.error('paper_trail_group_page :', err);
    } finally {
      loadingMore = false;
    }
  }
  function openMenu(e, g) {
    e.stopPropagation();
    const r = e.currentTarget.getBoundingClientRect();
    menu = {
      address: g.address, who: g.who ?? g.address,
      x: r.left,
      y: r.bottom + 4,
    };
  }
  function gesture(destination) {
    const { address, who } = menu;
    menu = null;
    onroute(address, who, destination);
  }
</script>

<div class="scene" data-testid="paper-trail">
  <div class="colonne">
    <!-- R2/R11: the mode's normalized view header — glyph + title,
         classes shared from system.css. Title alone (D2 pattern). -->
    <h2 class="display entete-vue" data-testid="registre-titre">
      <span class="glyphe-titre" aria-hidden="true"><Icon name="paper_trail" size={26} /></span>{t('mailbox.paper_trail')}
      <span class="essor-titre"></span>
      <SectionSort value={sort} onchange={(v) => (sort = v)} /></h2>
    {#if groups.length}
      {#each sortedGroups as g (g.address)}
        <!-- The row is a div-button (.ligne pattern of the List): the
             ⋯ lives INSIDE — a button nested in a button would be
             invalid and unreachable by keyboard. -->
        <div class="rang-groupe" data-testid="registre-groupe" role="button"
             tabindex="0" data-adresse={g.address} aria-expanded={isOpen === g.address}
             onclick={() => toggleGroup(g.address)}
             onkeydown={activation(() => toggleGroup(g.address))}>
          <span class="empile" aria-hidden="true"><span></span><span></span><span></span></span>
          <span class="corps">
            <span class="l1">
              <span class="exp">{g.who ?? g.address}</span>
              <span class="essor"></span>
              <span class="heure">{when(g.lastEpoch)}</span>
            </span>
            <span class="l2">
              <span class="nombre">{t(g.threads > 1 ? 'paper_trail.threads' : 'paper_trail.thread', { n: g.threads })}</span>
              {#if g.lastSubject}<span class="objet">{g.lastSubject}</span>{/if}
            </span>
          </span>
          <span class="gestes" role="button" tabindex="0"
                data-testid="registre-gestes" aria-haspopup="menu"
                aria-expanded={menu?.address === g.address}
                aria-label={t('list.gestures')}
                onclick={(e) => openMenu(e, g)}
                onkeydown={(e) => e.key === 'Enter' && openMenu(e, g)}>
            <Icon name="more_horiz" size={16} /></span>
        </div>
        {#if isOpen === g.address}
          <div class="dedans" data-testid="registre-messages">
            {#each openMessages as m (m.account_id + '/' + m.mailbox + '/' + m.uid)}
              <button type="button" class="rang-message" data-testid="registre-message"
                      class:nonlu={m.thread_unseen > 0}
                      onclick={() => onopen(m)}>
                <span class="objet-m">{m.subject}</span>
                <span class="essor"></span>
                <span class="heure">{when(m.epoch)}</span>
              </button>
            {/each}
            {#if morePossible}
              <button type="button" class="voir-plus" data-testid="registre-plus"
                      disabled={loadingMore} onclick={loadMore}>
                {t('paper_trail.seeMore')}</button>
            {/if}
          </div>
        {/if}
      {/each}
    {:else if served}
      <p class="vide" data-testid="registre-vide">{t('list.empty')}</p>
    {/if}
  </div>
</div>

<Menu isOpen={menu !== null} x={menu?.x ?? 0} y={menu?.y ?? 0}
      testid="registre-menu" width={220} onclose={() => (menu = null)}>
    {#each ['inbox', 'feed'] as dest (dest)}
      <button type="button" role="menuitem" data-testid={`registre-vers-${dest}`}
              onclick={() => gesture(dest)}>
        <Icon name={dest === 'inbox' ? 'inbox' : 'feed'} />{t('list.moveTo', { mailbox: t(`mailbox.${dest}`) })}</button>
    {/each}
    <div class="filet"></div>
    <button type="button" role="menuitem" data-testid="registre-ecarter"
            onclick={() => gesture('screened_out')}>
      <Icon name="visibility_off" />{t('list.screenOut')}</button>
  </Menu>

<style>
  /* The Paper trail's scene — the Feed's geometry (centered column,
     the scene scrolls). */
  .scene { flex:1; overflow:auto; padding:28px 36px 60px; min-width:0; }
  .essor-titre { flex:1; }
  .colonne { max-width:720px; margin:0 auto; }
  /* The row of a group: the Feed's stack + the Cleanup's two lines
     (sender/time, count/subject of the last one). */
  .rang-groupe {
    width:100%; display:flex; align-items:center; gap:12px;
    padding:12px 10px; font-size:13px; color:var(--ink); text-align:left;
    background:none; border:none; border-top:1px solid var(--border);
    cursor:pointer;
  }
  .rang-groupe:hover { background:var(--hover); }
  .empile { position:relative; width:20px; height:16px; flex:none; }
  .empile span {
    position:absolute; inset:0; background:var(--surface);
    border:1px solid var(--border);
  }
  .empile span:nth-child(1) { transform:translate(4px, -4px); }
  .empile span:nth-child(2) { transform:translate(2px, -2px); }
  .corps { flex:1; min-width:0; display:flex; flex-direction:column; gap:2px; }
  .l1, .l2 { display:flex; align-items:baseline; gap:8px; min-width:0; }
  .exp {
    font-weight:600; min-width:0;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
  }
  .essor { flex:1; }
  .heure { flex:none; font-size:12px; color:var(--muted); font-variant-numeric:tabular-nums; }
  .nombre { flex:none; font-size:12px; font-weight:600; color:var(--accent); font-variant-numeric:tabular-nums; }
  .l2 .objet {
    min-width:0; color:var(--ink2);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
  }
  /* The threads of an expanded group — bare rows, detached under the
     stack; the unread keeps its bold (the list's rule). */
  .dedans { border-top:1px solid var(--border); }
  .rang-message {
    width:100%; display:flex; align-items:baseline; gap:8px;
    padding:8px 10px 8px 42px; font-size:13px; color:var(--ink);
    text-align:left; background:none; border:none; cursor:pointer;
  }
  .rang-message:hover { background:var(--hover); }
  .rang-message .objet-m {
    min-width:0; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
  }
  .rang-message.nonlu .objet-m { font-weight:600; }
  .vide {
    margin:24px 0 0; font-size:13px; line-height:1.5; color:var(--muted);
  }
  /* The ⋯: reserved space, opacity only (List/Feed pattern). */
  .gestes {
    flex:none; width:24px; height:24px; align-self:center;
    display:inline-flex; align-items:center; justify-content:center;
    opacity:0; color:var(--muted); border:1px solid transparent;
  }
  .rang-groupe:hover .gestes, .gestes:focus-visible,
  .gestes[aria-expanded="true"] { opacity:1; }
  .gestes:hover, .gestes[aria-expanded="true"] {
    background:var(--hover); border-color:var(--border); color:var(--ink);
  }
  .voir-plus {
    margin:6px 0 10px 42px; height:30px; padding:0 14px;
    display:inline-flex; align-items:center; font-size:13px;
    color:var(--ink); background:var(--surface);
    border:1px solid var(--border); border-radius:var(--r-control);
    cursor:pointer;
  }
  .voir-plus:hover { background:var(--sel); }
  .voir-plus:disabled { opacity:.6; cursor:default; }
</style>

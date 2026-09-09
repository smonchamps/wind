<script>
  // Keep each surviving card in its original read section while reading.
  // Only nearby cards mount their body iframe; other cards retain measured height.
  import { untrack } from 'svelte';
  import Icon from './Icon.svelte';
  import Menu from './Menu.svelte';
  import SectionSort from './SectionSort.svelte';
  import Stacked from './Stacked.svelte';
  import { sortComparator } from './lib/sort.js';
  import { call } from './lib/transport.js';
  import ImagePermission from './ImagePermission.svelte';
  import { watchImagePermissions } from './lib/image-permissions.js';
  import { showImages, alwaysShowImages } from './lib/thread.svelte.js';
  import { watchViews } from './lib/views.svelte.js';
  import { autoBody } from './lib/body.js';
  import { wireLinks } from './lib/links.js';
  import { when } from './lib/when.js';
  import { t } from './lib/text.svelte.js';

  let {
    account = null,
    onflash = () => {},
    onmove = () => {},
    onsetaside = () => {},
    ontotal = () => {},
  } = $props();

  const PAGE = 20;
  let cards = $state([]);
  let exhausted = $state(false);
  let inFlight = $state(false);
  // Emptiness never asserts itself without proof (List's E2 lesson):
  // `served` only turns true on a RECEIVED response — an IPC failure
  // does not paint "Nothing in the Feed", and the entry does not flash it.
  let served = $state(false);
  let generation = 0;
  let requestedExtent = PAGE;
  let moreRequested = false;

  const cardKey = (r) => `${r.account_id}:${r.mailbox}:${r.version?.mailbox_id ?? 0}:${r.version?.uid_validity ?? 0}:${r.uid}`;

  async function load(since) {
    const capturedGen = ++generation;
    const capturedAccount = account;
    requestedExtent = Math.max(requestedExtent, since + PAGE);
    const limit = since === 0 ? requestedExtent : PAGE;
    inFlight = true;
    try {
      const page = [];
      let complete = false;
      // Reconcile every served page. Retaining an unverified tail leaves deleted
      // messages visible; a failed refresh leaves the previous snapshot intact.
      for (let offset = since; offset < since + limit; offset += PAGE) {
        const batch = await call('feed_cards', {
          accountId: capturedAccount, offset, limit: PAGE,
        });
        if (capturedGen !== generation) return;
        page.push(...batch);
        complete = batch.length < PAGE;
        if (complete) break;
      }
      const previous = new Map(cards.map((card) => [cardKey(card.row), card]));
      const fresh = new Map();
      for (const card of page) {
        const key = cardKey(card.row);
        const old = previous.get(key);
        fresh.set(key, old ? { ...card, read: old.read } : card);
      }
      cards = since === 0 ? [...fresh.values()]
        : [...cards, ...[...fresh].filter(([key]) => !previous.has(key)).map(([, card]) => card)];
      exhausted = complete;
      served = true;
      if (since === 0) {
        call('category_total', { category: 'feed', accountId: capturedAccount, unread: false })
          .then((n) => {
            if (capturedGen === generation) ontotal(n);
          })
          .catch(() => {});
      }
    } catch (err) {
      console.error('feed_cards :', err);
    } finally {
      if (capturedGen === generation) {
        inFlight = false;
        if (moreRequested) {
          moreRequested = false;
          if (!exhausted) load(cards.length);
        }
      }
    }
  }

  export function reload() {
    untrack(() => load(0));
  }

  watchViews(reload);
  watchImagePermissions(() => {
    cards = cards.map(card => ({ ...card, document: null, images_message_allowed: false }));
    reload();
  });

  let scopeGeneration = 0;
  $effect(() => {
    void account;
    untrack(() => {
      scopeGeneration += 1;
      cards = [];
      requestedExtent = PAGE;
      moreRequested = false;
      exhausted = false;
      served = false;
      visibleIndex = 0;
      heights = {};
      replies = {};
      openGroups = {};
      if (scene) scene.scrollTop = 0;
      ontotal(null);
      load(0);
    });
  });

  // Windowing (PLAN-AUDIT-V2 E10): a live iframe + a
  // ResizeObserver PER card — ten pages = two hundred documents. Only
  // cards within WINDOW rows of the first visible one carry
  // their iframe; a card that leaves the window leaves a block of its
  // measured height (scrolling does not jump), and gets it back on
  // return.
  const WINDOW = 5;
  let visibleIndex = $state(0);
  let heights = $state({});
  const offWindow = (i) => Math.abs(i - visibleIndex) > WINDOW;
  let measureRequested = false;
  function measureWindow(scene) {
    if (measureRequested) return;
    measureRequested = true;
    const capturedScope = scopeGeneration;
    requestAnimationFrame(() => {
      measureRequested = false;
      if (capturedScope !== scopeGeneration) return;
      const top = scene.getBoundingClientRect().top;
      const articles = scene.querySelectorAll('article.card');
      let first = 0;
      for (let i = 0; i < articles.length; i += 1) {
        if (articles[i].getBoundingClientRect().bottom > top) { first = i; break; }
      }
      // The height of bodies about to leave the window, taken BEFORE
      // they unmount.
      const fresh = { ...heights };
      articles.forEach((article, i) => {
        if (Math.abs(i - first) > WINDOW) {
          const body = article.querySelector('iframe.body');
          if (body) fresh[article.dataset.key] = body.offsetHeight;
        }
      });
      heights = fresh;
      visibleIndex = first;
    });
  }

  // The next page when the bottom approaches — one flight at a time.
  function onScroll(e) {
    const el = e.currentTarget;
    measureWindow(el);
    if (exhausted) return;
    if (el.scrollTop + el.clientHeight >= el.scrollHeight - 600) {
      if (inFlight) moreRequested = true;
      else load(cards.length);
    }
  }

  async function grantImages(card, always) {
    try {
      // The reading pane's grants, unchanged: a card is a message.
      await (always ? alwaysShowImages(card.row) : showImages(card.row));
    } catch (err) {
      onflash(t('error.imagePermission', { err }));
    }
  }

  // A card's fold (CE finding at E5bis's visual STOP): each
  // card collapses/expands on the right, like the messages of the
  // reading pane. R10: an UNREAD card arrives expanded, a READ card
  // (within its group) arrives collapsed onto the subject line.
  let replies = $state({});
  const isCollapsed = (card) => replies[cardKey(card.row)] ?? card.read;
  function toggleCollapse(card) {
    replies[cardKey(card.row)] = !isCollapsed(card);
  }

  // R10 — the two sections, computed from the SERVED state (card.read):
  // marks made in flight do not touch it.
  // R9 (field 2026-08-31): each section carries ITS OWN sort — the
  // defaults stay the prior order (unread by served recency,
  // groups alphabetically A → Z); the button cycles, presentation only
  // (sortComparator, the collation follows the UI's language).
  let sortUnread = $state('date-desc');
  let sortRead = $state('alpha-az');
  const unread = $derived(
    cards
      .filter((c) => !c.read)
      .sort(sortComparator(sortUnread, (c) => c.row.epoch, (c) => c.row.sender ?? '')),
  );
  const groups = $derived.by(() => {
    const byWhom = new Map();
    for (const c of cards) {
      if (!c.read) continue;
      const who = c.row.sender ?? '';
      if (!byWhom.has(who)) byWhom.set(who, []);
      byWhom.get(who).push(c);
    }
    return [...byWhom.entries()]
      .map(([who, theirCards]) => ({ who, cards: theirCards }))
      .sort(sortComparator(
        sortRead,
        (g) => Math.max(...g.cards.map((c) => c.row.epoch)),
        (g) => g.who,
      ));
  });
  // The DOM rank of every card, sections and groups combined: this is
  // what the window compares against the first visible card (E10).
  const ranks = $derived(
    new Map([...unread, ...groups.flatMap((g) => g.cards)].map((c, i) => [cardKey(c.row), i])),
  );
  // Backlog 97: fold the WHOLE unread section as one — the compact
  // overview the beta asked for. Per-card entries in the same map, so
  // a manual fold afterwards still wins card by card; a folded card
  // has no read witness (R10), so folding never marks anything read.
  const allFolded = $derived(unread.length > 0 && unread.every((c) => isCollapsed(c)));
  function toggleAllUnread() {
    const fold = !allFolded;
    for (const c of unread) replies[cardKey(c.row)] = fold;
  }
  let openGroups = $state({});

  // R10 — the read witness: a node at the FOOT of every unread
  // card; when it enters the scene, the elevation's bottom has been
  // shown — the card marks itself (idempotent, one write per card).
  let scene = $state(null);
  const witnesses = new Map();
  let observer = null;
  // Backlog 102 (beta): "the moment I open a letter it turns read".
  // Seeing the foot for an INSTANT is not reading — a short card fits
  // the scene whole, and "Show images" shifts the foot into view by
  // itself. The witness now requires a DWELL: the foot stays in the
  // scene for 2 s before the card marks itself. Leaving early cancels.
  // e2e seam (the __e2eLinks pattern): the dwell is compiled to 2 s in
  // releases, adjustable in the harness to prove both directions.
  const dwellMs = () => {
    // typeof, not ||: a harness asking for dwell 0 (the immediate-mark
    // direction) must get 0, not the production 2 s.
    const seam = import.meta.env.VITE_E2E === '1' ? globalThis.window?.__e2eFeedDwell : undefined;
    return typeof seam === 'number' ? seam : 2000;
  };
  const dwells = new Map();
  function cancelDwell(node) {
    clearTimeout(dwells.get(node));
    dwells.delete(node);
  }
  $effect(() => {
    if (!scene) return;
    observer = new IntersectionObserver((entries) => {
      for (const e of entries) {
        const card = witnesses.get(e.target);
        if (!card) continue;
        if (!e.isIntersecting) {
          cancelDwell(e.target);
          continue;
        }
        if (dwells.has(e.target)) continue;
        dwells.set(e.target, setTimeout(() => {
          dwells.delete(e.target);
          observer?.unobserve(e.target);
          markRead(card, e.target);
        }, dwellMs()));
      }
    }, { root: scene });
    // Witnesses mounted before the effect (the first render) get
    // observed here — the action runs ahead of the observer.
    for (const node of witnesses.keys()) observer.observe(node);
    // Time out of sight is not reading: a hidden window cancels the
    // running dwells; coming back re-observes so the visible feet
    // start a fresh dwell (an unchanged intersection would otherwise
    // never re-fire).
    const visibility = () => {
      if (document.visibilityState === 'hidden') {
        for (const node of dwells.keys()) cancelDwell(node);
      } else {
        for (const node of witnesses.keys()) {
          observer?.unobserve(node);
          observer?.observe(node);
        }
      }
    };
    document.addEventListener('visibilitychange', visibility);
    return () => {
      document.removeEventListener('visibilitychange', visibility);
      for (const node of dwells.keys()) cancelDwell(node);
      observer?.disconnect();
      observer = null;
    };
  });
  function readWitness(node, card) {
    witnesses.set(node, card);
    observer?.observe(node);
    return {
      destroy() {
        cancelDwell(node);
        witnesses.delete(node);
        observer?.unobserve(node);
      },
    };
  }
  const marked = new Set();
  async function markRead(card, witness) {
    const k = cardKey(card.row);
    if (marked.has(k)) return;
    marked.add(k);
    try {
      await call('feed_mark_read', {
        accountId: card.row.account_id,
        mailbox: card.row.mailbox,
        uid: card.row.uid,
        version: card.row.version,
      });
    } catch (err) {
      // The write failed: the witness RE-ARMS (review — without the
      // re-observe, "next time around" was a lie: an unobserved
      // node never comes back) and the mark will replay.
      marked.delete(k);
      if (witnesses.has(witness)) observer?.observe(witness);
      console.error('feed_mark_read :', err);
    }
  }

  // A card's gestures menu (the pattern of the rows' ⋯).
  let menu = $state(null);
  function openMenu(e, card) {
    e.stopPropagation();
    // Second click on the same ⋯ closes (backlog 91 family).
    if (menu && menu.key === cardKey(card.row)) {
      menu = null;
      return;
    }
    const r = e.currentTarget.getBoundingClientRect();
    menu = {
      row: card.row,
      key: cardKey(card.row),
      x: r.left,
      y: r.bottom + 4,
      anchor: e.currentTarget,
    };
  }
  function gesture(fn, ...args) {
    const { row } = menu;
    menu = null;
    fn(row, ...args);
  }
</script>


{#snippet cardBlock(card)}
  <article class="card" data-testid="feed-card" data-key={cardKey(card.row)}>
    <div class="from">
      <span class="name">{card.row.sender}</span>
      <button type="button" class="gestures" data-testid="feed-gestures"
              aria-label={t('list.gestures')} aria-haspopup="menu"
              aria-expanded={menu?.key === cardKey(card.row)}
              onclick={(e) => openMenu(e, card)}>
        <Icon name="more_horiz" size={14} /></button>
      <span class="time">{when(card.row.epoch)}</span>
    </div>
    <!-- The fold (CE finding, 3 passes): the exact button of the
         reading pane — glyph + text, bare button —, ON THE SUBJECT
         LINE, aligned right. -->
    <div class="rank-subject">
      <h3 class="display">{card.row.subject}</h3>
      <button type="button" class="bare" data-testid="feed-fold"
              aria-expanded={!isCollapsed(card)}
              onclick={() => toggleCollapse(card)}>
        <Icon name={isCollapsed(card) ? 'unfold_more' : 'unfold_less'} />
        {isCollapsed(card) ? t('action.expand') : t('action.collapse')}</button>
    </div>
    {#if isCollapsed(card)}
      <p class="preview">{card.row.preview ?? ''}</p>
    {:else if offWindow(ranks.get(cardKey(card.row)) ?? 0) && card.document !== null}
      <!-- Out of window: the block keeps the height of the unmounted body. -->
      <div class="body-dormant" style={`height:${heights[cardKey(card.row)] ?? 0}px`}
           data-testid="feed-dormant-body"></div>
    {:else if card.document !== null}
      {#if card.images_message_allowed}
        <ImagePermission message={card.row} {onflash} />
      {/if}
      {#if card.remote_images_blocked > 0}
        <!-- R1: the image guard, as in the reading pane —
             without it, a newsletter all in remote images would be
             an empty slab with no recourse (E5bis review). -->
        <div class="images-guard" data-testid="feed-images-guard">
          <span>{t('reading.blockedImages', { n: card.remote_images_blocked })}</span>
          <button type="button" onclick={() => grantImages(card, false)}>
            {t('reading.showImages')}</button>
          <button type="button" onclick={() => grantImages(card, true)}>
            {t('reading.alwaysShowImages')}</button>
        </div>
      {/if}
      <iframe class="body" sandbox="allow-same-origin" srcdoc={card.document}
              title={card.row.subject} use:autoBody
              onload={(ev) => wireLinks(ev.currentTarget)}></iframe>
    {:else}
      <!-- Body not yet cached: the preview says the essential, the
           normal backfill will fill in the card. -->
      <p class="preview">{card.row.preview ?? ''}</p>
    {/if}
    {#if !card.read && !isCollapsed(card)}
      <!-- R10: the read witness — the FOOT of the elevation; to
           see it pass by is to have read the card to the bottom. -->
      <div class="read-witness" use:readWitness={card} aria-hidden="true"></div>
    {/if}
  </article>
{/snippet}

<div class="scene" data-testid="feed" onscroll={onScroll} bind:this={scene}>
  <div class="column">
    <!-- R11 (RETOURS-13): the header at the Screener's format — glyph +
         title + two CE sentences, left-justified on the column. -->
    <h2 class="display header-view" data-testid="feed-title">
      <span class="glyph-title" aria-hidden="true"><Icon name="feed" size={26} /></span>{t('mailbox.feed')}</h2>
    <p class="subtitle-view">{t('feed.subtitle1')}<br />{t('feed.subtitle2')}</p>
    {#if cards.length}
      <!-- Field RETOURS-13 (C5): the section title stays visible
           when everything is read — the Screener's checkmark says the work is done. -->
      <div class="row-section">
        <p class="rule-label" data-testid="feed-section-unread">{t('feed.sectionUnread')}</p>
        {#if unread.length}
          <!-- Backlog 97: the section-wide fold — the same bare
               glyph+text button as the per-card fold, in the header. -->
          <button type="button" class="bare fold-all" data-testid="feed-fold-all"
                  aria-expanded={!allFolded}
                  onclick={toggleAllUnread}>
            <Icon name={allFolded ? 'unfold_more' : 'unfold_less'} />
            {allFolded ? t('feed.unfoldAll') : t('feed.foldAll')}</button>
          <SectionSort value={sortUnread} onchange={(v) => (sortUnread = v)} />
        {/if}
      </div>
      {#if unread.length}
        {#each unread as card (cardKey(card.row))}
          {@render cardBlock(card)}
        {/each}
      {:else}
        <div class="all-read" data-testid="feed-all-read">
          <span class="ic-yes" aria-hidden="true"><Icon name="check_circle" /></span>{t('feed.allRead')}
        </div>
      {/if}
    {/if}
    {#if groups.length}
      <div class="row-section">
        <p class="rule-label" data-testid="feed-section-read">{t('feed.sectionRead')}</p>
        <SectionSort value={sortRead} onchange={(v) => (sortRead = v)} />
      </div>
      {#each groups as g (g.who)}
        <!-- D5: a collapsed group's row shows a PILE
             of elevations (the set-aside's visual); the click expands
             its cards, collapsed onto the subject line. -->
        <button type="button" class="rank-group" data-testid="feed-group"
                aria-expanded={!!openGroups[g.who]}
                onclick={() => (openGroups[g.who] = !openGroups[g.who])}>
          <Stacked />
          <span class="who" data-testid="feed-group-name">{g.who}</span>
          <span class="count">{g.cards.length}</span>
        </button>
        {#if openGroups[g.who]}
          {#each g.cards as card (cardKey(card.row))}
            {@render cardBlock(card)}
          {/each}
        {/if}
      {/each}
    {/if}
    {#if served && cards.length === 0 && !inFlight}
      <p class="empty" data-testid="feed-empty">{t('feed.empty')}</p>
    {/if}
  </div>
</div>

<Menu isOpen={menu !== null} x={menu?.x ?? 0} y={menu?.y ?? 0}
      anchor={menu?.anchor ?? null}
      testid="feed-menu" onclose={() => (menu = null)}>
    {#each ['inbox', 'paper_trail'] as dest (dest)}
      <button type="button" role="menuitem" data-testid={`feed-to-${dest}`}
              onclick={() => gesture(onmove, dest)}>
        <Icon name={dest === 'inbox' ? 'inbox' : 'paper_trail'} />{t('list.moveTo', { mailbox: t(`mailbox.${dest}`) })}</button>
    {/each}
    <div class="net"></div>
    <button type="button" role="menuitem" data-testid="feed-aside"
            onclick={() => gesture(onsetaside)}>
      <Icon name="pile" />{t('pile.put')}</button>
    <div class="net"></div>
    <button type="button" role="menuitem" data-testid="feed-screen-out"
            onclick={() => gesture(onmove, 'screened_out')}>
      <Icon name="visibility_off" />{t('list.screenOut')}</button>
  </Menu>

<style>
  .scene { flex:1; overflow:auto; padding:28px 36px 60px; min-width:0; }
  .column { max-width:720px; margin:0 auto; }
  /* R11: the header and the label rule are the SHARED classes of
     system.css (.header-view / .subtitle-view / .rule-label —
     one copy, Screener and Feed). */
  .card { padding:26px 0 10px; border-top:1px solid var(--border); }
  /* R9: the section line carries the sort on the right. */
  .row-section { display:flex; align-items:center; gap:10px; }
  .row-section .rule-label { flex:1; min-width:0; }
  /* R10 — a collapsed group's row: pile of elevations + name +
     count, the drawing of a row (never a filled button). */
  .rank-group {
    width:100%; display:flex; align-items:center; gap:12px;
    padding:12px 10px; font-size:13px; color:var(--ink); text-align:left;
    background:none; border:none; border-top:1px solid var(--border);
    cursor:pointer;
  }
  .rank-group:hover { background:var(--hover); }
  .rank-group .who {
    flex:1; min-width:0; font-weight:600;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
  }
  .rank-group .count {
    flex:none; font-size:12px; font-weight:600; color:var(--accent);
    font-variant-numeric:tabular-nums;
  }
  /* The read witness: a node with no geometry — it moves
     nothing, it only exists for the observer. */
  .read-witness { height:1px; }
  /* C5: "all read" — the Screener's checkmark (accent), the drawing
     of its emptiness (top stroke by the section, dimmed text). */
  .all-read {
    display:flex; align-items:center; gap:8px; padding:12px 0;
    font-size:13px; color:var(--ink2); border-top:1px solid var(--border);
  }
  .ic-yes :global(.ic) { color:var(--accent); }
  .from { display:flex; align-items:baseline; gap:8px; margin-bottom:10px; }
  .from .name { font-size:13px; font-weight:600; color:var(--ink2); flex:1; min-width:0;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .from .time { font-size:12px; color:var(--muted); flex:none; }
  /* The ⋯: reserved space, opacity only (the geometry does not move). */
  .gestures {
    flex:none; width:24px; height:24px; padding:0; align-self:center;
    display:inline-flex; align-items:center; justify-content:center;
    opacity:0; color:var(--muted); background:none;
    border:1px solid transparent;
  }
  .card:hover .gestures, .gestures:focus-visible, .gestures[aria-expanded="true"] { opacity:1; }
  .gestures:hover, .gestures[aria-expanded="true"] {
    background:var(--hover); border-color:var(--border); color:var(--ink);
  }
  /* The fold: the BARE button of the reading pane (glyph + text), on
     the SUBJECT LINE, on the right (CE finding, 3 passes). */
  .rank-subject {
    display:flex; align-items:center; gap:12px; margin:0 0 12px;
  }
  .rank-subject h3 { margin:0; flex:1; min-width:0; }
  .rank-subject .bare { flex:none; }
  .bare {
    height:26px; padding:0 9px; display:inline-flex; align-items:center;
    gap:6px; font-size:12px; color:var(--ink2); background:none;
    border:1px solid transparent; border-radius:var(--r-control); cursor:pointer;
    white-space:nowrap;
  }
  .bare:hover { background:var(--sel); }
  h3 { margin:0; font-size:24px; line-height:1.25; color:var(--ink); }
  .body { width:100%; border:none; display:block; background:#fff; }
  .preview { margin:0 0 8px; font-size:13px; line-height:1.5; color:var(--ink2); }
  .images-guard {
    display:flex; align-items:center; gap:10px; flex-wrap:wrap;
    padding:8px 12px; margin:0 0 8px; font-size:12px; color:var(--ink2);
    background:var(--surface); border:1px solid var(--border);
  }
  .images-guard button { height:26px; padding:0 10px; font-size:12px; }
  .empty { margin:8px 0 0; font-size:13px; line-height:1.5; color:var(--ink2); max-width:66ch; }
  .net { border-top:1px solid var(--border); margin:4px 0; }
</style>

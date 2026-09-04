<script>
  // Feed in CARDS (PLAN-MODE-ORGANISE E5bis, then RETOURS-13
  // R10/R11) — newsletters arrive ALREADY OPEN,
  // centered ~720 px column, one card = sender + time, subject in
  // display, the WHOLE BODY (auto-CSP document, sandboxed iframe S1),
  // the ⋯ of gestures. R10 reverses A100's "nothing is marked read":
  // a card whose elevation BOTTOM has been shown is READ (witness
  // IntersectionObserver → `feed_mark_read`, pins pattern) — the
  // scene splits into "Unread" (expanded, chronological) and
  // "Previously read" (groups by sender in alphabetical order, collapsed
  // to a pile — D5). Sectioning is computed AS OF the page's service:
  // a card never jumps during reading. Bodies come
  // from the CACHE by served page (D5/S3); no windowing: cards
  // are added page by page as scrolling goes (the limit stated in the PLAN).
  import Icon from './Icon.svelte';
  import Menu from './Menu.svelte';
  import SectionSort from './SectionSort.svelte';
  import { sortComparator } from './lib/sort.js';
  import { call } from './lib/transport.js';
  import { views } from './lib/views.svelte.js';
  import { autoBody } from './lib/body.js';
  import { wireLinks } from './lib/links.js';
  import { when } from './lib/when.js';
  import { t } from './lib/text.svelte.js';

  let {
    account = null,
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

  const cardKey = (r) => `${r.account_id}:${r.mailbox}:${r.uid}`;

  async function load(since) {
    const capturedGen = ++generation;
    inFlight = true;
    try {
      const page = await call('feed_cards', {
        accountId: account,
        offset: since,
        limit: PAGE,
      });
      if (capturedGen !== generation) return;
      if (since === 0) {
        // Merge by key (PLAN-AUDIT-V2 E10): a re-served page USED TO
        // REPLACE everything — the read card jumped section during
        // reading and pages 2..n disappeared. `read` stays FROZEN at
        // first service (sections are computed from the served state,
        // R10); cards already served beyond the page stay behind.
        const previous = new Map(cards.map((c) => [cardKey(c.row), c]));
        const fresh = page.map((c) => {
          const old = previous.get(cardKey(c.row));
          return old ? { ...c, read: old.read } : c;
        });
        const views = new Set(fresh.map((c) => cardKey(c.row)));
        cards = [...fresh, ...cards.slice(PAGE).filter((c) => !views.has(cardKey(c.row)))];
      } else {
        // De-duplication on append (E5bis review): an arrival between
        // two pages shifts the offsets — the same card re-served
        // would cause a key collision (the keyed each's crash).
        const views = new Set(cards.map((c) => cardKey(c.row)));
        cards = [...cards, ...page.filter((c) => !views.has(cardKey(c.row)))];
      }
      exhausted = page.length < PAGE;
      served = true;
      // The total follows every reload (E5bis review: a ⋯ that drains
      // cards left the status bar at the previous count).
      if (since === 0) {
        call('category_total', { category: 'feed', accountId: account, unread: false })
          .then((n) => {
            if (capturedGen === generation) ontotal(n);
          })
          .catch(() => {});
      }
    } catch (err) {
      console.error('feed_cards :', err);
    } finally {
      if (capturedGen === generation) inFlight = false;
    }
  }

  export function reload() {
    load(0);
  }

  // E7 (PLAN-AUDIT-V3, closes D-48): the view subscribes to the shared
  // invalidation signal — any writer that bumps it (gesture handlers,
  // the resting probe on a generation move, Settings) reloads this view
  // while it is mounted, without a ref wired per surface.
  let seenGeneration = views.generation;
  $effect(() => {
    if (views.generation !== seenGeneration) {
      seenGeneration = views.generation;
      reload();
    }
  });


  // New scope (account) → start over from the top.
  $effect(() => {
    void account;
    load(0);
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
    requestAnimationFrame(() => {
      measureRequested = false;
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
    if (exhausted || inFlight) return;
    if (el.scrollTop + el.clientHeight >= el.scrollHeight - 600) {
      load(cards.length);
    }
  }

  // R1: granting images — THE product's commands (message or
  // sender), then THE card is re-served: it comes back with its
  // images, the guard disappears. Field STOP 2 PLAN-AUDIT-V2
  // (2026-09-02): `load(0)` only re-served page 0 — the merge
  // by key (E10) kept a card beyond it unchanged, and its guard
  // stayed after ten pages scrolled. The card re-serves wherever it
  // is, via `message_body` (the same document as the pane); for the
  // sender rule, every still-guarded served card — the
  // core arbitrates, a third party's card re-renders identically.
  async function grantImages(card, always) {
    try {
      await call(always ? 'allow_images_sender' : 'allow_images_message', {
        accountId: card.row.account_id,
        mailbox: card.row.mailbox,
        uid: card.row.uid,
      });
      const targets = always ? cards.filter((c) => c.remote_images_blocked > 0) : [card];
      await Promise.all(targets.map(serveAgain));
    } catch (err) {
      console.error('images kiosque :', err);
    }
  }
  async function serveAgain(card) {
    const view = await call('message_body', {
      accountId: card.row.account_id,
      mailbox: card.row.mailbox,
      uid: card.row.uid,
      showImages: false,
    });
    const key = cardKey(card.row);
    cards = cards.map((c) =>
      cardKey(c.row) === key
        ? { ...c, document: view.document, remote_images_blocked: view.remote_images_blocked }
        : c,
    );
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
  let openGroups = $state({});

  // R10 — the read witness: a node at the FOOT of every unread
  // card; when it enters the scene, the elevation's bottom has been
  // shown — the card marks itself (idempotent, one write per card).
  let scene = $state(null);
  const witnesses = new Map();
  let observer = null;
  $effect(() => {
    if (!scene) return;
    observer = new IntersectionObserver((entries) => {
      for (const e of entries) {
        if (!e.isIntersecting) continue;
        const card = witnesses.get(e.target);
        if (!card) continue;
        observer?.unobserve(e.target);
        markRead(card, e.target);
      }
    }, { root: scene });
    // Witnesses mounted before the effect (the first render) get
    // observed here — the action runs ahead of the observer.
    for (const node of witnesses.keys()) observer.observe(node);
    return () => {
      observer?.disconnect();
      observer = null;
    };
  });
  function readWitness(node, card) {
    witnesses.set(node, card);
    observer?.observe(node);
    return {
      destroy() {
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
    const r = e.currentTarget.getBoundingClientRect();
    menu = {
      row: card.row,
      key: cardKey(card.row),
      x: r.left,
      y: r.bottom + 4,
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
        {#if unread.length}<SectionSort value={sortUnread} onchange={(v) => (sortUnread = v)} />{/if}
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
          <span class="stacked" aria-hidden="true"><span></span><span></span><span></span></span>
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
  /* The pile (D5): three offset elevations, the set-aside's fan
     visual in miniature. */
  .stacked { position:relative; width:20px; height:16px; flex:none; }
  /* V14: zero radius — the pile's sheets are bare
     rectangles, like the set-aside pile's visual. */
  .stacked span {
    position:absolute; inset:0; background:var(--surface);
    border:1px solid var(--border);
  }
  .stacked span:nth-child(1) { transform:translate(4px, -4px); }
  .stacked span:nth-child(2) { transform:translate(2px, -2px); }
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

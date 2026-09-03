<script>
  // Nav 248 px of screen 02 — the tracks' drawing (A29, amended V4/
  // V14): 14 px rows with sharp corners, active item in selection hue
  // bordered in accent, unread count as a BARE NUMBER in accent (the
  // solid badge is dead). The “hero / total” counts leave the nav
  // (W2-D4) — the status bar states the totals. The six canonical
  // folders, then “Mailboxes”: All mailboxes + one row per REAL
  // account — the “Work / Personal” fiction does not exist; `person`
  // icon (decision D7), label = address. The CURRENT mailbox takes
  // the event tile's drawing (--tile/--tileInk, W2-D5).
  import Icon from './Icon.svelte';
  import { activation } from './lib/keyboard.js';
  import { mailboxLabelKey } from './lib/organized.svelte.js';
  import { t } from './lib/text.svelte.js';

  // R1 (PLAN-RETOURS-8, A74), amended A82: an account can carry a
  // marker (icon from the dedicated set + hue from the swatch) — it
  // replaces `person` with the glyph's bare STROKE in the account's
  // hue, 16 px like the folder glyphs (the solid badge left the nav:
  // the nav and the row carry exactly the same object, D2). Without a
  // marker, the original D7 rendering does not change.
  // PLAN-RETOURS-9 (D4): the custom name REPLACES the address on the
  // tile — the nav states the chosen identity, not the technical data.
  // PLAN-MODE-ORGANISE E1/E2: in organized mode, Feed, Paper trail and
  // Screener insert under the Inbox — the row/badge/onchoose pattern
  // is the same, only the folder table recomposes. The Screener's
  // badge (`screener`, E2) counts the MESSAGES waiting at the desk —
  // the prototype's drawing. Classic mode: zero diff.
  let {
    accounts = [], markers = {}, names = {}, category, account,
    organized = false, screener = 0,
    // RETOURS-14 R7: the Feed's badges (cards never
    // opened, D8) and the Paper trail's (IMAP unread), `screener` pattern.
    feed = 0, paperTrail = 0,
    onchoose = () => {},
  } = $props();

  const sum = (field) => accounts.reduce((n, c) => n + c[field], 0);

  // The account filter bounds the folders' counts, as in the
  // prototype where changing Mailbox re-filters the list.
  const view = $derived(
    account === null ? null : accounts.find((c) => c.account_id === account),
  );
  const de = (field) => (view ? view[field] : sum(field));

  const folders = $derived([
    {
      // RETOURS-13 R3: the label comes from THE shared rule — in
      // organized mode “Inbox”, in classic the long label.
      id: 'inbox', icon: 'inbox',
      label: t(mailboxLabelKey('inbox')),
      unread: de('inbox_unread'),
    },
    ...(organized
      ? [
          { id: 'feed', icon: 'feed', label: t('mailbox.feed'), unread: feed },
          { id: 'paper_trail', icon: 'paper_trail', label: t('mailbox.paper_trail'), unread: paperTrail },
          { id: 'screener', icon: 'screener', label: t('mailbox.screener'), unread: screener },
          // Part B (PLAN-HORIZON-NETTOYAGE): the mode's 5th section.
          { id: 'cleanup', icon: 'cleanup', label: t('mailbox.cleanup') },
        ]
      : []),
    { id: 'sent', icon: 'send', label: t('mailbox.sent') },
    { id: 'drafts', icon: 'edit_note', label: t('mailbox.drafts') },
    {
      id: 'junk', icon: 'report', label: t('mailbox.junk'),
      unread: de('junk_unread'),
    },
    { id: 'archive', icon: 'inventory_2', label: t('mailbox.archive') },
    { id: 'trash', icon: 'delete', label: t('mailbox.trash') },
  ]);

  // The tile counts nothing (A36, field E3): the Inbox's badge
  // already states the unread — the tile carries only the identity.
  const mailboxes = $derived([
    { id: null, icon: 'all_inbox', label: t('nav.all') },
    ...accounts.map((c) => ({
      id: c.account_id, icon: 'person', label: names[c.account_id] ?? c.email,
      marker: markers[c.account_id] ?? null,
    })),
  ]);
</script>

<nav aria-label={t('nav.aria')} data-testid="nav">
  {#each folders as d (d.id)}
    <div class="rank" class:active={category === d.id}
         data-testid="nav-folder" data-category={d.id}
         role="button" tabindex="0" aria-current={category === d.id}
         onclick={() => onchoose({ category: d.id })}
         onkeydown={activation(() => onchoose({ category: d.id }))}>
      <span class="icon" aria-hidden="true"><Icon name={d.icon} /></span>
      <span class="label">{d.label}</span>
      {#if d.unread > 0}
        <span class="badge">{d.unread}</span>
      {/if}
    </div>
    {#if organized && d.id === 'cleanup'}
      <!-- RETOURS-13 R12: the divider between the organized folders
           and the rest — the drawing of `.boites`'s divider. -->
      <div class="separator" data-testid="nav-separator"></div>
    {/if}
  {/each}

  <div class="mailboxes">
    <p class="title">{t('nav.mailboxes')}</p>
    {#each mailboxes as b (b.id)}
      {#if account === b.id}
        <!-- The current mailbox: the event tile (A29, W2-D5),
             the address alone — no counter (A36). -->
        <div class="tile" data-testid="nav-mailbox"
             role="button" tabindex="0" aria-current="true"
             onclick={() => onchoose({ account: b.id })}
             onkeydown={activation(() => onchoose({ account: b.id }))}>
          {#if b.marker}
            <span class="bare-marker" data-testid="nav-marker"
                  data-hue={b.marker.hue} aria-hidden="true"><Icon name={b.marker.icon} size={16} /></span>
          {:else}
            <span class="icon-tile" aria-hidden="true"><Icon name={b.icon} /></span>
          {/if}
          <span class="title-tile">{b.label}</span>
        </div>
      {:else}
        <div class="rank" data-testid="nav-mailbox"
             role="button" tabindex="0" aria-current="false"
             onclick={() => onchoose({ account: b.id })}
             onkeydown={activation(() => onchoose({ account: b.id }))}>
          {#if b.marker}
            <span class="bare-marker" data-testid="nav-marker"
                  data-hue={b.marker.hue} aria-hidden="true"><Icon name={b.marker.icon} size={16} /></span>
          {:else}
            <span class="icon" aria-hidden="true"><Icon name={b.icon} /></span>
          {/if}
          <span class="label">{b.label}</span>
        </div>
      {/if}
    {/each}
  </div>
</nav>

<style>
  nav {
    background:var(--bg); border-right:1px solid var(--border);
    padding:20px 12px; display:flex; flex-direction:column; gap:2px;
    min-height:0; overflow:auto;
  }
  /* PLAN-RETOURS-10 R4: the glyph aligns to the label's BASELINE,
     then drops 2 px — the OPTICAL alignment the CE chose on the
     board (variant C, three captures, field pass of 2026-08-27): the
     original flex centering placed the SVG ~2.6 px below the baseline
     (too low), the pure baseline made it look too high. The
     mechanics: align-items:baseline requires the SVG to keep its
     default vertical-align (the global `middle` of `.ic`,
     system.css, is overridden below), and the drop is a transform —
     outside the geometry, the rows do not move. */
  .rank {
    display:flex; align-items:baseline; gap:10px; flex:none;
    padding:8px 10px; border-radius:var(--r-control); cursor:pointer;
    border:1px solid transparent;
  }
  .icon :global(.ic), .icon-tile :global(.ic),
  .bare-marker :global(.ic) {
    vertical-align:baseline; transform:translateY(2px);
  }
  .rank:hover { background:var(--hover); }
  .rank.active { background:var(--sel); border-color:var(--accent); }
  .icon { color:var(--muted); }
  .active .icon { color:var(--accent); }
  .label {
    font-size:14px; color:var(--ink2); flex:1; min-width:0;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
  }
  .active .label { font-weight:600; color:var(--ink); }
  /* V4 — the solid pill is dead: the counter is a BARE NUMBER,
     tabular digits in accent (accent/bg pair measured as TEXT). */
  .badge {
    flex:none; font-size:12px; font-weight:600; color:var(--accent);
    font-variant-numeric:tabular-nums;
  }
  .separator {
    margin:6px 0; border-top:1px solid var(--border); flex:none;
  }
  .mailboxes {
    margin-top:auto; padding-top:16px; border-top:1px solid var(--border);
    display:flex; flex-direction:column; gap:6px;
  }
  .title {
    margin:0 0 4px; padding:0 10px; font-size:11px; letter-spacing:.1em;
    text-transform:uppercase; color:var(--muted); font-weight:600;
  }
  .tile {
    display:flex; align-items:baseline; gap:10px; flex:none;
    padding:9px 12px; border-radius:var(--r-control); cursor:pointer;
    background:var(--tile); color:var(--tileInk);
    border:1px solid var(--border);
  }
  .icon-tile { color:var(--tileInk); }
  .title-tile {
    font-size:13px; font-weight:600; min-width:0;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
  }
</style>

<script>
  // The Screener (PLAN-MODE-ORGANISE E2) — the shape settled at the
  // prototype over six CE passes (spikes/mode-organise): title and
  // subtitle centered, the rule-label “Voulez-vous recevoir leurs
  // messages ?” in the drawing of “Historique du Portier”, then ONE
  // row per sender waiting, IN THE FORMAT of the central pane's rows
  // (unread dot, sender, time that never yields, subject, preview)
  // plus the address in plain text — the only data the Screener adds.
  // Yes / No buttons 44 px on the right, each capped with a mini ⋯:
  // on Yes it ROUTES (Inbox / Feed / Paper trail), on No it sets the
  // RULE (junk / archiving / deletion — `trash` at heart, D4: never a
  // permanent deletion). The bare click follows the set DEFAULTS
  // (RETOURS-13 R5/R9: shipped Yes → Inbox, No → Trash; Settings >
  // Screener changes them). **A yes/no, nothing else** — neither
  // sorting nor processing the message at the desk (CE verdict, pass
  // 1): the row does not open. The sender is never notified; the
  // history states the rule chosen and “Reinstate” undoes it.
  import Icon from './Icon.svelte';
  import Menu from './Menu.svelte';
  import { call } from './lib/transport.js';
  import { when } from './lib/when.js';
  import { t } from './lib/text.svelte.js';
  import { SCREENED_OUT_LABEL } from './lib/screener.js';
  import SectionSort from './SectionSort.svelte';
  import { sortComparator } from './lib/sort.js';

  let { onchange = () => {}, onflash = () => {} } = $props();

  let ranks = $state([]);
  let screenedOut = $state([]);
  // RETOURS-13 R5/R9: the default actions of the bare click — read
  // from the core (Settings > Screener sets them); the shipped
  // defaults hold as long as the database has not answered.
  let defaults = $state({ yes: 'inbox', no: 'trash' });
  // The open mini ⋯: { address, who, type: 'yes'|'no', x, y }.
  let menu = $state(null);
  // R9: the history sort — most recent first by default
  // (the served order), the button cycles; presentation only.
  let sortHistory = $state('date-desc');
  const sortedScreenedOut = $derived(
    [...screenedOut].sort(sortComparator(sortHistory, (r) => r.epoch, (r) => r.address)),
  );

  export async function reload() {
    try {
      const [pending, history] = await Promise.all([
        call('screener_waiting'),
        call('routings'),
      ]);
      ranks = pending;
      // The desk's history shows only the SCREENED OUT (prototype):
      // a Yes shows in its own view, a No shows only here.
      screenedOut = history.filter((r) => r.destination === 'screened_out');
    } catch (err) {
      console.error('portier :', err);
    }
  }
  $effect(() => {
    (async () => {
      // RETOURS-13 review: the defaults are read ONCE, BEFORE the
      // desk — the first row is only painted once the defaults are
      // known (never a bare click on a stale shipped default), a
      // read failure keeps the shipped ones WITHOUT blocking the
      // rows, and later decisions do not re-pay the IPC.
      try {
        defaults = await call('screener_defaults_get');
      } catch (err) {
        console.error('screener_defaults_get :', err);
      }
      reload();
    })();
  });

  const TOAST_NO = {
    spam: 'toast.screenerNoSpam',
    archive: 'toast.screenerNoArchive',
    trash: 'toast.screenerNoTrash',
  };
  const MAILBOX_OF = {
    inbox: 'screener.theInbox',
    feed: 'screener.theFeed',
    paper_trail: 'screener.thePaperTrail',
  };

  // The verdict — the E1 command, THE single gate for routing. `who`:
  // the row's display name, for the toast.
  async function decide(address, who, destination, rule = null) {
    menu = null;
    try {
      await call('route_sender', { address, destination, rule });
      if (destination === 'screened_out') {
        onflash(t(rule ? TOAST_NO[rule] : 'toast.screenerNoBare', { who }));
      } else if (destination === 'inbox') {
        onflash(t('toast.screenerYesBare', { who }));
      } else {
        onflash(t('toast.screenerYesTo', { who, mailbox: t(MAILBOX_OF[destination]) }));
      }
      await reload();
      onchange();
    } catch (err) {
      onflash(t('error.preference', { err }));
    }
  }

  async function reinstate(routing) {
    try {
      await call('remove_routing', { address: routing.address });
      onflash(t('toast.screenerReinstated', { who: routing.address }));
      await reload();
      onchange();
    } catch (err) {
      onflash(t('error.preference', { err }));
    }
  }

  // The mini ⋯ sits at the corner of the button; the menu anchors at
  // the click point, bound to the window (prototype pattern).
  function openMini(e, entry, type) {
    e.stopPropagation();
    const r = e.currentTarget.getBoundingClientRect();
    menu = {
      address: entry.address,
      who: entry.row.sender,
      type,
      x: r.left,
      y: r.bottom + 4,
    };
  }
</script>


<div class="scene" data-testid="screener">
  <div class="colonne">
    <!-- RETOURS-13 R4/R7: the screener glyph caps the title; glyph,
         title and subtitle (three CE sentences) left-align on the
         rows column — the centered header is dead. -->
    <h2 class="display entete-vue" data-testid="portier-titre">
      <span class="glyphe-titre" aria-hidden="true"><Icon name="screener" size={26} /></span>{t('mailbox.screener')}</h2>
    <p class="sous-titre-vue">{t('screener.subtitle1')}<br />{t('screener.subtitle2')}<br />{t('screener.subtitle3')}</p>

    <!-- RETOURS-13 field finding (C2): the section title stays visible
         even with no new sender — the empty state shows below. -->
    <p class="regle-libelle">{t('screener.question')}</p>
    {#if ranks.length}
      {#each ranks as entry (entry.address)}
        <div class="rang-portier" data-testid="portier-rang" data-adresse={entry.address}>
          <div class="msg" class:nonlu={entry.row.thread_unseen > 0}>
            <div class="l1">
              {#if entry.row.thread_unseen > 0}<span class="disque"></span>{/if}
              <span class="exp">{entry.row.sender}</span>
              <span class="adr">&lt;{entry.address}&gt;</span>
              <span class="essor"></span>
              <span class="heure">{when(entry.row.epoch)}</span>
            </div>
            <p class="objet">{entry.row.subject}</p>
            <p class="apercu">{entry.row.preview ?? ''}</p>
          </div>
          <div class="choix">
            <span class="btn-portier">
              <button type="button" class="gros" data-testid="portier-oui"
                      onclick={() => decide(entry.address, entry.row.sender, defaults.yes)}>
                <span class="ic-oui"><Icon name="check_circle" /></span>{t('screener.yes')}</button>
              <button type="button" class="mini" data-testid="portier-mini-oui"
                      aria-label={t('screener.yesChoice')} aria-haspopup="menu"
                      aria-expanded={menu?.address === entry.address && menu?.type === 'yes'}
                      onclick={(e) => openMini(e, entry, 'yes')}>
                <Icon name="more_horiz" size={12} /></button>
            </span>
            <span class="btn-portier">
              <button type="button" class="gros" data-testid="portier-non"
                      onclick={() => decide(entry.address, entry.row.sender, 'screened_out',
                        defaults.no === 'screened_out' ? null : defaults.no)}>
                <span class="ic-non"><Icon name="cancel" /></span>{t('screener.no')}</button>
              <button type="button" class="mini" data-testid="portier-mini-non"
                      aria-label={t('screener.noChoice')} aria-haspopup="menu"
                      aria-expanded={menu?.address === entry.address && menu?.type === 'no'}
                      onclick={(e) => openMini(e, entry, 'no')}>
                <Icon name="more_horiz" size={12} /></button>
            </span>
          </div>
        </div>
      {/each}
    {:else}
      <div class="vide" data-testid="portier-vide">
        <span class="ic-oui"><Icon name="check_circle" /></span>{t('screener.empty')}
      </div>
    {/if}

    <!-- R9: the sort, on the right of the section title row. -->
    <div class="ligne-section historique">
      <p class="regle-libelle">{t('screener.history')}</p>
      {#if screenedOut.length}<SectionSort value={sortHistory} onchange={(v) => (sortHistory = v)} />{/if}
    </div>
    {#if screenedOut.length}
      {#each sortedScreenedOut as routing (routing.address)}
        <div class="rang-historique" data-testid="portier-historique">
          <span class="ic-hist" aria-hidden="true"><Icon name="visibility_off" /></span>
          <span class="qui"><b>{routing.address}</b> : {t(SCREENED_OUT_LABEL[routing.rule] ?? 'screener.screenedOut')}</span>
          <button type="button" data-testid="portier-reintegrer"
                  onclick={() => reinstate(routing)}>{t('screener.reinstate')}</button>
        </div>
      {/each}
    {:else}
      <p class="historique-vide">{t('screener.historyEmpty')}</p>
    {/if}
  </div>
</div>

<Menu isOpen={menu !== null} x={menu?.x ?? 0} y={menu?.y ?? 0}
      testid="portier-menu" onclose={() => (menu = null)}>
    {#if menu.type === 'yes'}
      <p class="titre-menu">{t('screener.yesTo')}</p>
      <button type="button" role="menuitem" data-testid="portier-vers-reception"
              onclick={() => decide(menu.address, menu.who, 'inbox')}>
        <Icon name="inbox" />{t('screener.toInbox')}</button>
      <button type="button" role="menuitem" data-testid="portier-vers-kiosque"
              onclick={() => decide(menu.address, menu.who, 'feed')}>
        <Icon name="feed" />{t('screener.toFeed')}</button>
      <button type="button" role="menuitem" data-testid="portier-vers-registre"
              onclick={() => decide(menu.address, menu.who, 'paper_trail')}>
        <Icon name="paper_trail" />{t('screener.toPaperTrail')}</button>
    {:else}
      <p class="titre-menu">{t('screener.noWillBe')}</p>
      <button type="button" role="menuitem" data-testid="portier-regle-spam"
              onclick={() => decide(menu.address, menu.who, 'screened_out', 'spam')}>
        <Icon name="report" />{t('screener.ruleSpam')}</button>
      <button type="button" role="menuitem" data-testid="portier-regle-archive"
              onclick={() => decide(menu.address, menu.who, 'screened_out', 'archive')}>
        <Icon name="inventory_2" />{t('screener.ruleArchive')}</button>
      <button type="button" role="menuitem" data-testid="portier-regle-corbeille"
              onclick={() => decide(menu.address, menu.who, 'screened_out', 'trash')}>
        <Icon name="delete" />{t('screener.ruleTrash')}</button>
    {/if}
  </Menu>

<style>
  .scene { flex:1; overflow:auto; padding:28px 36px 60px; min-width:0; }
  .colonne { max-width:820px; margin:0 auto; }
  /* The header and the rule-label live in ONE copy in system.css
     (.entete-vue / .sous-titre-vue / .regle-libelle — RETOURS-13,
     shared with the Feed); only the local variant stays here. */
  /* R9: the section row carries the sort on the right; the history's
     spacing now lives on the ROW. */
  .ligne-section {
    display:flex; align-items:center; gap:10px;
  }
  .ligne-section .regle-libelle { flex:1; min-width:0; }
  .ligne-section.historique { margin-top:34px; }
  .rang-portier {
    display:flex; align-items:center; gap:18px; padding:20px 0;
    border-top:1px solid var(--border);
  }
  /* The message: THE format of the central pane's rows (l1 / subject /
     preview), plus the address in plain text. The time never yields. */
  .msg { flex:1; min-width:0; display:grid; grid-template-columns:1fr; row-gap:3px; }
  .l1 { display:flex; align-items:baseline; gap:8px; min-width:0; }
  .l1 :global(.disque) { align-self:center; }
  .exp { font-size:14px; font-weight:400; color:var(--ink); flex:0 1 auto; min-width:0;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .nonlu .exp { font-weight:700; }
  .adr { font-size:13px; color:var(--muted); flex:0 1 auto; min-width:0;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .essor { flex:1; }
  .heure { font-size:12px; color:var(--muted); flex:none; }
  .objet { margin:0; font-size:14px; color:var(--ink);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .nonlu .objet { font-weight:700; }
  .apercu { margin:0; font-size:13px; color:var(--ink2);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  /* The choices, on the RIGHT (CE verdict, pass 3): Yes / No 44 px,
     mini ⋯ at the top-right corner of each. */
  .choix { display:flex; gap:12px; flex:none; }
  .choix .gros { height:44px; padding:0 18px; font-weight:600; }
  .ic-oui :global(.ic) { color:var(--accent); }
  .ic-non :global(.ic) { color:var(--alert); }
  .btn-portier { position:relative; display:inline-flex; }
  .mini {
    position:absolute; top:-8px; right:-8px; width:19px; height:19px; padding:0;
    display:inline-flex; align-items:center; justify-content:center;
    background:var(--surface); border:1px solid var(--border);
    border-radius:var(--r-control); color:var(--muted); cursor:pointer;
  }
  .mini:hover, .mini[aria-expanded="true"] { background:var(--sel); color:var(--ink); }
  /* RETOURS-13 R8: the empty state left-aligns, like the header. */
  .vide {
    display:flex; align-items:center; gap:8px;
    padding:12px 0; font-size:13px; color:var(--ink2);
  }
  .rang-historique {
    display:flex; align-items:center; gap:10px; padding:10px 2px;
    border-top:1px solid var(--border); font-size:13px; color:var(--ink2);
  }
  .ic-hist :global(.ic) { color:var(--alert); }
  .qui { flex:1; min-width:0; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .rang-historique button { height:28px; padding:0 12px; font-size:12px; }
  .historique-vide { margin:8px 0 0; font-size:13px; line-height:1.5; color:var(--ink2); max-width:66ch; }
  /* The mini ⋯ menu — the product's menu drawing. */
  .titre-menu {
    margin:4px 8px 6px; font-size:11px; letter-spacing:.06em;
    text-transform:uppercase; color:var(--muted); font-weight:600;
  }
</style>

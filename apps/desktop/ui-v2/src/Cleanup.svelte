<script>
  // Spring Cleaning (PLAN-HORIZON-NETTOYAGE, pane B) — the 5th
  // section of Organized mode. Two screens in ONE scene:
  // — the INTRO (EB1): header at the Screener's pattern, subtext CE word
  //   for word, range and scope (D6), "Start cleaning";
  // — the SORT (EB3): the Screener's organization, but the rows are
  //   GROUPS by sender — Yes/No applies to the whole group and
  //   applies to the range's stock AND to the future (D5); you enter
  //   a group to VIEW its messages (never sort at the message level —
  //   scope refusal of the PLAN); the progress bar at the top says
  //   the percentage of groups processed. The session is PERSISTED
  //   (D8): reopening the section resumes the sort where it left off. A
  //   bare click follows the Screener's defaults (D9), the mini ⋯ overrides.
  import Icon from './Icon.svelte';
  import Menu from './Menu.svelte';
  import SectionSort from './SectionSort.svelte';
  import { sortComparator } from './lib/sort.js';
  import { call } from './lib/transport.js';
  import { when } from './lib/when.js';
  import { t } from './lib/text.svelte.js';
  import {
    CLEANUP_RANGES as RANGES,
    CLEANUP_SCOPES as SCOPES,
  } from './lib/vocabularies.js';

  let { onchange = () => {}, onflash = () => {} } = $props();

  let range = $state('1a');
  let scope = $state('inbox');
  // null = intro; otherwise { range, scope, total, processed }.
  let session = $state(null);
  let groups = $state([]);
  // R9 (field 2026-08-31): the section's sort — recency by
  // default (the served order), the button cycles; presentation only.
  let sort = $state('date-desc');
  const sortedGroups = $derived(
    [...groups].sort(sortComparator(sort, (g) => g.lastEpoch, (g) => g.who ?? g.address)),
  );
  let defaults = $state({ yes: 'inbox', no: 'trash' });
  // The expanded group (address) and its messages — VIEW, nothing else.
  let isOpen = $state(null);
  let openMessages = $state([]);
  // The open mini ⋯: { address, who, type: 'yes'|'no', x, y }.
  let menu = $state(null);
  // Only one verdict in flight (review 2026-08-30): a double-click on
  // Yes = two verdicts for one group, and `processed` exceeds the total.
  let busy = $state(false);

  $effect(() => {
    (async () => {
      // D9: the Screener's defaults, read ONCE — the first row only
      // paints once the defaults are known (Screener pattern).
      try {
        defaults = await call('screener_defaults_get');
      } catch (err) {
        console.error('screener_defaults_get :', err);
      }
      // D8: a session already underway resumes — the intro does not show.
      try {
        session = await call('cleanup_state');
        if (session) await loadGroups();
      } catch (err) {
        console.error('cleanup_state :', err);
      }
    })();
  });

  async function loadGroups() {
    groups = await call('cleanup_groups');
  }

  async function start() {
    try {
      session = await call('cleanup_start', { range, scope });
      isOpen = null;
      await loadGroups();
    } catch (err) {
      onflash(t('error.preference', { err }));
    }
  }

  async function finish() {
    try {
      await call('cleanup_finish');
      session = null;
      groups = [];
      isOpen = null;
    } catch (err) {
      onflash(t('error.preference', { err }));
    }
  }

  const MAILBOX_OF = {
    inbox: 'screener.theInbox',
    feed: 'screener.theFeed',
    paper_trail: 'screener.thePaperTrail',
  };
  const TOAST_NO = {
    spam: 'toast.screenerNoSpam',
    archive: 'toast.screenerNoArchive',
    trash: 'toast.screenerNoTrash',
  };

  // The GROUP verdict — the same vocabulary as the Screener, the
  // `cleanup_verdict` gate also applies the rule to the range's stock.
  async function decide(address, who, destination, rule = null) {
    if (busy) return;
    busy = true;
    menu = null;
    try {
      session = await call('cleanup_verdict', { address, destination, rule });
      if (destination === 'screened_out') {
        onflash(t(rule ? TOAST_NO[rule] : 'toast.screenerNoBare', { who }));
      } else if (destination === 'inbox') {
        onflash(t('toast.screenerYesBare', { who }));
      } else {
        onflash(t('toast.screenerYesTo', { who, mailbox: t(MAILBOX_OF[destination]) }));
      }
      if (isOpen === address) isOpen = null;
      // The decided group leaves the list IN PLACE (review 2026-08-30:
      // re-aggregating the whole database on every click paid for the
      // groups-by-verdict query); the database is authoritative on the
      // next pass.
      groups = groups.filter((g) => g.address !== address);
      onchange();
    } catch (err) {
      onflash(t('error.preference', { err }));
    } finally {
      busy = false;
    }
  }

  async function toggleGroup(address) {
    if (isOpen === address) {
      isOpen = null;
      return;
    }
    try {
      openMessages = await call('cleanup_messages', { address });
      isOpen = address;
    } catch (err) {
      onflash(t('error.preference', { err }));
    }
  }

  function openMini(e, group, type) {
    e.stopPropagation();
    const r = e.currentTarget.getBoundingClientRect();
    menu = {
      address: group.address,
      who: group.who ?? group.address,
      type,
      x: r.left,
      y: r.bottom + 4,
    };
  }

  // The gauge follows what's LEFT, not a verdict counter (review
  // 2026-08-30): a verdict placed elsewhere (Screener, "Screen out
  // this sender" from a list) makes a group disappear without going
  // through here — counted on `processed`, the "done" screen would
  // have shown a bar stuck under 100%.
  const percent = $derived(
    session && session.total > 0
      ? Math.min(
          100,
          Math.max(0, Math.round(((session.total - groups.length) * 100) / session.total)),
        )
      : 100,
  );
</script>


<div class="scene" data-testid="cleanup">
  <div class="column">
    {#if !session}
      <h2 class="display header-view" data-testid="cleanup-title">
        <span class="glyph-title" aria-hidden="true"><Icon name="cleanup" size={26} /></span>{t('mailbox.cleanup')}</h2>
      <p class="subtitle-view">{t('cleanup.subtitle')}</p>

      <p class="rule-label">{t('cleanup.range')}</p>
      <div class="choice-range" role="radiogroup" aria-label={t('cleanup.range')}>
        {#each RANGES as p (p)}
          <button type="button" class="badge-range" class:chosen={range === p}
                  role="radio" aria-checked={range === p}
                  data-testid="cleanup-range" data-range={p}
                  onclick={() => (range = p)}>{t(`horizon.${p}`)}</button>
        {/each}
      </div>

      <p class="rule-label">{t('cleanup.scope')}</p>
      <div class="choice-range" role="radiogroup" aria-label={t('cleanup.scope')}>
        {#each SCOPES as pe (pe)}
          <button type="button" class="badge-range" class:chosen={scope === pe}
                  role="radio" aria-checked={scope === pe}
                  data-testid="cleanup-scope" data-scope={pe}
                  onclick={() => (scope = pe)}>{t(`cleanup.scope.${pe}`)}</button>
        {/each}
      </div>

      <button type="button" class="start" data-testid="cleanup-start"
              onclick={start}>{t('cleanup.start')}</button>
    {:else}
      <!-- The progress bar AT THE TOP (CE statement): % of groups
           processed — the migration gauge's drawing. -->
      <div class="progress" data-testid="cleanup-progress"
           role="progressbar" aria-valuemin="0" aria-valuemax="100"
           aria-valuenow={percent} aria-label={t('cleanup.progressAria')}>
        <div class="gauge"><div class="filled" style="width:{percent}%"></div></div>
        <span class="pct">{t('cleanup.progress', { p: percent })}</span>
      </div>

      <h2 class="display header-view">
        <span class="glyph-title" aria-hidden="true"><Icon name="cleanup" size={26} /></span>{t('mailbox.cleanup')}</h2>

      <div class="row-section">
        <p class="rule-label">{t('screener.question')}</p>
        {#if groups.length}<SectionSort value={sort} onchange={(v) => (sort = v)} />{/if}
      </div>
      {#if groups.length}
        {#each sortedGroups as g (g.address)}
          <div class="rank-group" data-testid="cleanup-group" data-address={g.address}>
            <!-- The row's body is the group's DOOR: you enter to
                 view — the verdict, itself, stays with the buttons. -->
            <button type="button" class="msg" data-testid="cleanup-open"
                    aria-expanded={isOpen === g.address}
                    onclick={() => toggleGroup(g.address)}>
              <span class="l1">
                <span class="sender">{g.who ?? g.address}</span>
                <span class="addr">&lt;{g.address}&gt;</span>
                <span class="grow"></span>
                <span class="time">{when(g.lastEpoch)}</span>
              </span>
              <span class="l2">
                <span class="count">{t(g.messages > 1 ? 'cleanup.messages' : 'cleanup.message', { n: g.messages })}</span>
                {#if g.lastSubject}<span class="subject">{g.lastSubject}</span>{/if}
              </span>
            </button>
            <div class="choice">
              <span class="btn-screener">
                <button type="button" class="big" data-testid="cleanup-yes"
                        onclick={() => decide(g.address, g.who ?? g.address, defaults.yes)}>
                  <span class="ic-yes"><Icon name="check_circle" /></span>{t('screener.yes')}</button>
                <button type="button" class="mini" data-testid="cleanup-mini-yes"
                        aria-label={t('screener.yesChoice')} aria-haspopup="menu"
                        aria-expanded={menu?.address === g.address && menu?.type === 'yes'}
                        onclick={(e) => openMini(e, g, 'yes')}>
                  <Icon name="more_horiz" size={12} /></button>
              </span>
              <span class="btn-screener">
                <button type="button" class="big" data-testid="cleanup-no"
                        onclick={() => decide(g.address, g.who ?? g.address, 'screened_out',
                          defaults.no === 'screened_out' ? null : defaults.no)}>
                  <span class="ic-no"><Icon name="cancel" /></span>{t('screener.no')}</button>
                <button type="button" class="mini" data-testid="cleanup-mini-no"
                        aria-label={t('screener.noChoice')} aria-haspopup="menu"
                        aria-expanded={menu?.address === g.address && menu?.type === 'no'}
                        onclick={(e) => openMini(e, g, 'no')}>
                  <Icon name="more_horiz" size={12} /></button>
              </span>
            </div>
          </div>
          {#if isOpen === g.address}
            <div class="inside" data-testid="cleanup-messages">
              <!-- The key carries the ACCOUNT: UIDs restart from 1 per
                   mailbox — "INBOX/42" would exist twice as soon as the
                   same letter touches two accounts (msgKey pattern). -->
              {#each openMessages as m (m.account_id + '/' + m.mailbox + '/' + m.uid)}
                <div class="rank-message">
                  <span class="subject-m">{m.subject}</span>
                  <span class="grow"></span>
                  <span class="time">{when(m.epoch)}</span>
                </div>
              {/each}
            </div>
          {/if}
        {/each}
      {:else}
        <!-- Not a single group left: the cleaning is done — the
             Screener's checkmark, and the exit. -->
        <div class="empty" data-testid="cleanup-empty">
          <span class="ic-yes"><Icon name="check_circle" /></span>{t('cleanup.done')}
        </div>
      {/if}

      <button type="button" class="finish" data-testid="cleanup-finish"
              onclick={finish}>{t('cleanup.finish')}</button>
    {/if}
  </div>
</div>

<Menu isOpen={menu !== null} x={menu?.x ?? 0} y={menu?.y ?? 0}
      testid="cleanup-menu" onclose={() => (menu = null)}>
    {#if menu.type === 'yes'}
      <p class="title-menu">{t('screener.yesTo')}</p>
      <button type="button" role="menuitem" data-testid="cleanup-to-inbox"
              onclick={() => decide(menu.address, menu.who, 'inbox')}>
        <Icon name="inbox" />{t('screener.toInbox')}</button>
      <button type="button" role="menuitem" data-testid="cleanup-to-feed"
              onclick={() => decide(menu.address, menu.who, 'feed')}>
        <Icon name="feed" />{t('screener.toFeed')}</button>
      <button type="button" role="menuitem" data-testid="cleanup-to-paper-trail"
              onclick={() => decide(menu.address, menu.who, 'paper_trail')}>
        <Icon name="paper_trail" />{t('screener.toPaperTrail')}</button>
    {:else}
      <p class="title-menu">{t('screener.noWillBe')}</p>
      <button type="button" role="menuitem" data-testid="cleanup-rule-spam"
              onclick={() => decide(menu.address, menu.who, 'screened_out', 'spam')}>
        <Icon name="report" />{t('screener.ruleSpam')}</button>
      <button type="button" role="menuitem" data-testid="cleanup-rule-archive"
              onclick={() => decide(menu.address, menu.who, 'screened_out', 'archive')}>
        <Icon name="inventory_2" />{t('screener.ruleArchive')}</button>
      <button type="button" role="menuitem" data-testid="cleanup-rule-trash"
              onclick={() => decide(menu.address, menu.who, 'screened_out', 'trash')}>
        <Icon name="delete" />{t('screener.ruleTrash')}</button>
    {/if}
  </Menu>

<style>
  /* R9: the section line carries the sort on the right. */
  .row-section { display:flex; align-items:center; gap:10px; }
  .row-section .rule-label { flex:1; min-width:0; }
  .scene { flex:1; overflow:auto; padding:28px 36px 60px; min-width:0; }
  .column { max-width:820px; margin:0 auto; }
  /* --- Intro --------------------------------------------------------- */
  .choice-range { display:flex; flex-wrap:wrap; gap:8px; padding:12px 0 4px; }
  .badge-range {
    height:32px; padding:0 14px; font-size:13px; color:var(--ink);
    background:var(--surface); border:1px solid var(--border);
    border-radius:var(--r-control); cursor:pointer;
  }
  .badge-range:hover { background:var(--sel); }
  .badge-range.chosen {
    border-color:var(--accent); color:var(--accent); font-weight:600;
    background:var(--sel);
  }
  .start {
    margin-top:26px; height:40px; padding:0 22px; font-size:14px;
    font-weight:600; color:var(--onAccent); background:var(--accent);
    border:1px solid var(--accent); border-radius:var(--r-control);
    cursor:pointer;
  }
  .start:hover { background:var(--accentH); border-color:var(--accentH); }
  /* --- Sort ----------------------------------------------------------- */
  /* The gauge: the migration modal's drawing (6 px, filled with
     the accent), the % in the TEXT (A52). */
  .progress { display:flex; align-items:center; gap:12px; padding:2px 0 18px; }
  .gauge {
    flex:1; height:6px; background:var(--sel);
    border-radius:999px; overflow:hidden;
  }
  .filled { height:100%; background:var(--accent); transition:width .25s ease; }
  .pct { flex:none; font-size:12.5px; color:var(--ink2); }
  .rank-group {
    display:flex; align-items:center; gap:18px; padding:16px 0;
    border-top:1px solid var(--border);
  }
  .msg {
    flex:1; min-width:0; display:flex; flex-direction:column; gap:3px;
    padding:4px 6px; margin:0 -6px; text-align:left;
    background:transparent; border:1px solid transparent;
    border-radius:var(--r-control); cursor:pointer;
  }
  .msg:hover { background:var(--sel); border-color:var(--border); }
  .l1, .l2 { display:flex; align-items:baseline; gap:8px; min-width:0; }
  .sender { font-size:14px; font-weight:600; color:var(--ink); flex:0 1 auto; min-width:0;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .addr { font-size:13px; color:var(--muted); flex:0 1 auto; min-width:0;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .grow { flex:1; }
  .time { font-size:12px; color:var(--muted); flex:none; }
  .count { font-size:12.5px; color:var(--accent); font-weight:600; flex:none; }
  .subject { font-size:13px; color:var(--ink2); min-width:0;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .choice { display:flex; gap:12px; flex:none; }
  .choice .big { height:44px; padding:0 18px; font-weight:600; }
  .ic-yes :global(.ic) { color:var(--accent); }
  .ic-no :global(.ic) { color:var(--alert); }
  .btn-screener { position:relative; display:inline-flex; }
  .mini {
    position:absolute; top:-8px; right:-8px; width:19px; height:19px; padding:0;
    display:inline-flex; align-items:center; justify-content:center;
    background:var(--surface); border:1px solid var(--border);
    border-radius:var(--r-control); color:var(--muted); cursor:pointer;
  }
  .mini:hover, .mini[aria-expanded="true"] { background:var(--sel); color:var(--ink); }
  .inside {
    margin:0 0 8px; padding:4px 12px 10px 24px;
    border-left:2px solid var(--border);
  }
  .rank-message { display:flex; align-items:baseline; gap:8px; padding:5px 0; min-width:0; }
  .subject-m { font-size:13px; color:var(--ink2); min-width:0;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .empty {
    display:flex; align-items:center; gap:8px;
    padding:12px 0; font-size:13px; color:var(--ink2);
    border-top:1px solid var(--border);
  }
  .finish {
    margin-top:22px; height:32px; padding:0 16px; font-size:13px;
    color:var(--ink); background:var(--surface);
    border:1px solid var(--border); border-radius:var(--r-control);
    cursor:pointer;
  }
  .finish:hover { background:var(--sel); }
  /* The mini ⋯'s menu — the product's menus drawing (Screener). */
  .title-menu {
    margin:4px 8px 6px; font-size:11px; letter-spacing:.06em;
    text-transform:uppercase; color:var(--muted); font-weight:600;
  }
</style>

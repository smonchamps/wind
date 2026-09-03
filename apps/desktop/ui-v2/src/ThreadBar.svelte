<script>
  // THE thread bar (RETOURS-14 R1/D1; A56, A58, A73, A96): the
  // conversation's TRIAGE gestures — Archive, Report as spam /
  // “Not spam”, Pin; in organized mode, Set aside and “Move to…”.
  // ONE component, TWO drawings (field pass 2026-09-02, pass 3 of
  // wave 2's STOP 2, CE verdict): on the PANE, stuck under the
  // thread's header — flat band on the background, divider, sticky
  // at the top on scroll (R1); at SCREEN 03, its buttons live
  // directly in the scene's header bar (Conversation.svelte), between
  // the back button and “Compose”.
  import Icon from './Icon.svelte';
  import Menu from './Menu.svelte';
  import { thread, isEcho } from './lib/thread.svelte.js';
  import { t } from './lib/text.svelte.js';

  let {
    drawing = 'pane',
    isJunk = false,
    pinnable = false,
    organized = false,
    onarchive = () => {},
    onspam = () => {},
    onnotspam = () => {},
    onpin = () => {},
    onmove = () => {},
    onsetaside = () => {},
  } = $props();

  // The “Move to…” menu closes on a row change — without this
  // reflex, thread A's menu would stay open above thread B and a
  // stray click would route B.
  let moveMenu = $state(false);
  $effect(() => {
    void thread.row;
    moveMenu = false;
  });
</script>

<div class="actions" class:pane={drawing === 'pane'} class:header={drawing === 'header'}
     data-testid="bar-thread">
  <button type="button" data-testid="archive" onclick={() => onarchive(thread.row)}>
    <Icon name="archive" />{t('action.archive')}</button>
  {#if isJunk}
    <button type="button" data-testid="not-spam" onclick={() => onnotspam(thread.row)}>
      <Icon name="inbox" />{t('action.notSpam')}</button>
  {:else}
    <button type="button" data-testid="report-spam" onclick={() => onspam(thread.row)}>
      <Icon name="report" />{t('action.reportSpam')}</button>
  {/if}
  <!-- R4 (PLAN-RETOURS-7): pinning THE conversation — a toggle
       stated by its label AND aria-pressed; the state comes from the
       core (pin_state) and follows the gesture. Never on an echo. -->
  {#if pinnable && !isEcho(thread.row)}
    <button type="button" data-testid="pin" aria-pressed={thread.pin}
            onclick={() => onpin(thread.row)}>
      <Icon name={thread.pin ? 'keep_off' : 'keep'} />
      {thread.pin ? t('action.unpin') : t('action.pin')}</button>
  {/if}
  <!-- PLAN-MODE-ORGANISE E1: the manual routing — one sender,
       one destination. Never on an echo (no envelope). No glyph:
       no existing drawing carries this meaning (A3), the text is
       enough in the bar. -->
  {#if organized && !isEcho(thread.row)}
    <!-- E5: the pile toggle — the state is SEEDED from the served
         row (the pin's pattern, review 2026-08-21: never a round
         trip per opening) and follows the gesture (App, store
         token); the gesture goes up to the App, which owns the
         command. -->
    <button type="button" data-testid="put-aside"
            aria-pressed={thread.aside}
            onclick={() => onsetaside(thread.row)}>
      <Icon name={thread.aside ? 'keep_off' : 'pile'} />
      {thread.aside ? t('pile.resume') : t('pile.put')}</button>
    <span class="move">
      <button type="button" data-testid="move-to"
              aria-haspopup="menu" aria-expanded={moveMenu}
              onclick={() => (moveMenu = !moveMenu)}>
        {t('action.moveTo')}</button>
      <Menu isOpen={moveMenu} testid="move-menu" width={170} absolute
            onclose={() => (moveMenu = false)}>
          {#each ['inbox', 'feed', 'paper_trail'] as dest (dest)}
            <button type="button" role="menuitem"
                    data-testid={`move-${dest}`}
                    onclick={() => { moveMenu = false; onmove(thread.row, dest); }}>
              {t(`mailbox.${dest}`)}</button>
          {/each}
        </Menu>
    </span>
  {/if}
</div>

<style>
  .actions { display:flex; gap:12px; flex-wrap:wrap; align-items:center; }
  /* Pane: stuck under the thread's header, flat on the background,
     the divider closes it off; sticky at the top on scroll (R1, D1)
     — z-index above the raised cards. */
  .actions.pane {
    flex:none; padding:6px 0 12px; position:sticky; top:0; z-index:4;
    background:var(--bg); border-bottom:1px solid var(--border);
  }
  /* Screen 03's header: inline in the bar, nothing more. */
  .actions.header { flex:none; gap:8px; }
  .actions button {
    height:32px; padding:0 16px; display:inline-flex; align-items:center;
    gap:8px; font-size:13px; color:var(--ink); background:var(--surface);
    border:1px solid var(--border); border-radius:var(--r-control); cursor:pointer;
  }
  .actions button:hover { background:var(--sel); }
  .move { position:relative; display:inline-flex; }
</style>

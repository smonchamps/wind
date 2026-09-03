<script>
  // The reading pane of screen 02 — since UI v3 (CE verdict of
  // 2026-08-16, ANNOTATIONS-V3 §6), the pane shows the THREAD as
  // cards: it is only a FRAME around Thread.svelte, the same object as
  // screen 03 (decision D4). The frames' exclusivity is carried by
  // the store (`thread.frame`, v3 review): this component renders the
  // object only when the frame is its own — nothing to reconcile by
  // hand.
  import Thread from './Thread.svelte';
  import { thread, openThread, closeThread } from './lib/thread.svelte.js';
  import { t } from './lib/text.svelte.js';

  let {
    drafts = [],
    // A80/D5: the thread states the mailbox behind the name — the
    // markers and the account names come down from the App, in BOTH
    // frames.
    markers = {},
    names = {},
    accounts = [],
    mixed = false,
    onresume = () => {},
    onarchive = () => {},
    ondelete = () => {},
    onconversation = () => {},
    onreply = () => {},
    onreplyall = () => {},
    onforward = () => {},
    onspam = () => {},
    onnotspam = () => {},
    isJunk = false,
    onflash = () => {},
    organized = false,
    onmove = () => {},
    onsetaside = () => {},
    pinnable = false,
    onpin = () => {},
  } = $props();

  export function open(newRow) {
    return openThread(newRow, 'pane');
  }
  export function close() {
    closeThread();
  }
  export function snapshot() {
    return { lastOpenMs: thread.lastOpenMs };
  }
</script>

<main aria-label={t('reading.aria')} data-testid="volet-lecture">
  {#if thread.frame === 'pane' && thread.line}
    <Thread {drafts} {markers} {names} {accounts} {mixed} {organized} {onmove} {onsetaside}
         {onresume} {onarchive} {ondelete}
         {onreply} {onreplyall} {onforward}
         {onspam} {onnotspam} {isJunk} {onflash}
         {pinnable} {onpin}
         onenlarge={onconversation} />
  {:else if thread.frame !== 'full'}
    <p class="vide">{t('reading.empty')}</p>
  {/if}
</main>

<style>
  /* The pane is FLAT (field A46, the prototype's .voletLecture): a
     panel that scrolls in one single flow — the elevation belongs to
     the message cards, never to the whole pane. */
  main {
    /* Field pass 2026-09-02 (pass 4): NO top padding on the scrolling
       frame — a `position:sticky` bounds itself to the frame's
       CONTENT edge, under its padding: the thread's stuck bar stopped
       18 px below the visible top and the message showed through the
       band. The top air is rendered by the thread (--fil-haut). */
    background:var(--bg); padding:0 22px 18px; --fil-haut:18px; min-width:0;
    display:flex; flex-direction:column; min-height:0; overflow-y:auto;
    /* RETOURS-14 R1 (review): the thread's bar is sticky with a
       z-index — without a stacking context here, it would pass
       ABOVE the modal veils (Compose/Settings, z-index 2). */
    isolation:isolate;
  }
  .vide {
    margin:auto; font-size:13px; line-height:1.5; color:var(--muted);
    text-align:center; padding:40px;
  }
</style>

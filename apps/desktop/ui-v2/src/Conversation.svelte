<script>
  // Screen 03 — the FULL SCREEN conversation: since UI v3 (decision
  // D4 of 2026-08-16), a FRAME (back/Compose header + scene) around
  // Thread.svelte, the same object as the reading pane. The exclusivity
  // lives in the store (`thread.frame`, v3 review): this component
  // renders when the frame is 'full', period — no local boolean to
  // desync. Enlarging reloads nothing (`enlargeThread`); a direct
  // selection in 1-2 panes reloads (`openThread`).
  import Icon from './Icon.svelte';
  import Thread from './Thread.svelte';
  import ThreadBar from './ThreadBar.svelte';
  import { thread, openThread, enlargeThread } from './lib/thread.svelte.js';
  import { t } from './lib/text.svelte.js';
  import { mailboxLabelKey } from './lib/organized.svelte.js';

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
    onback = () => {},
    onarchive = () => {},
    ondelete = () => {},
    onreply = () => {},
    onreplyall = () => {},
    onforward = () => {},
    onspam = () => {},
    onnotspam = () => {},
    isJunk = false,
    oncompose = () => {},
    onflash = () => {},
    pinnable = false,
    onpin = () => {},
    organized = false,
    onmove = () => {},
    onsetaside = () => {},
  } = $props();

  export function open(newRow) {
    // The SAME object already held by the pane: a pure size change.
    if (thread.line && thread.frame === 'pane'
        && thread.line.account_id === newRow.account_id
        && thread.line.mailbox === newRow.mailbox
        && thread.line.uid === newRow.uid) {
      enlargeThread();
      return Promise.resolve(thread.lastOpenMs);
    }
    return openThread(newRow, 'full');
  }
  export function isOpen() {
    return thread.frame === 'full';
  }
</script>

{#if thread.frame === 'full' && thread.line}
  <div class="ecran03" data-testid="conversation">
    <header class="entete">
      <button type="button" class="retour" data-testid="retour-boite" onclick={onback}>
        <Icon name="arrow_back" />{t(mailboxLabelKey('inbox'))}</button>
      <!-- Field pass 2026-09-02 (CE): at screen 03, the thread's triage
           gestures live INSIDE the header bar — one single component
           with the pane (BarreFil), “entete” drawing. -->
      <div class="tri">
        <ThreadBar drawing="header" {isJunk} {pinnable} {organized}
                  {onarchive} {onspam} {onnotspam} {onpin} {onmove} {onsetaside} />
      </div>
      <button type="button" class="principal" onclick={oncompose}>
        <Icon name="edit_square" />{t('header.compose')}</button>
    </header>

    <!-- R3 (PLAN-RETOURS-7): screen 03 is FLAT, like the pane
         (A46 extended) — no more enclosing card: the scene scrolls in
         one single flow, only the message cards rise. Reading column
         centered and bound (D2: ~960 px). -->
    <div class="scene">
      <div class="colonne">
        <Thread {drafts} {markers} {names} {accounts} {mixed} {organized} {onmove} {onsetaside}
             {onresume} {onarchive} {ondelete}
             {onreply} {onreplyall} {onforward}
             {onspam} {onnotspam} {isJunk} {onflash}
             {pinnable} {onpin} />
      </div>
    </div>
  </div>
{/if}

<style>
  /* Screen 03's geometry — the header at 52 px since UI v3 (E4):
     the two frames of the same object share the same chrome, with no
     jump on enlarging (v3 review: 60 px caused an 8 px jump). */
  .ecran03 {
    position:absolute; inset:0; display:flex; flex-direction:column;
    background:var(--bg); z-index:1;
  }
  /* Field pass 2026-09-02 (pass 4, CE): the triage gestures align on
     the LEFT edge of the messages column. The header is a
     three-track grid whose center track reproduces the scene's
     column (960 px, centered in the same 28 px gutters); back on the
     left, “Compose” on the right, each in its own track. */
  .entete {
    height:52px; flex:none; background:var(--surface);
    border-bottom:1px solid var(--border); display:grid;
    grid-template-columns:minmax(auto, 1fr) minmax(0, 960px) minmax(auto, 1fr);
    align-items:center; gap:12px; padding:0 28px;
  }
  .tri { justify-self:start; min-width:0; }
  .entete > .principal { justify-self:end; }
  button {
    height:32px; padding:0 16px; display:inline-flex; align-items:center;
    gap:8px; font-size:13px; color:var(--ink); background:var(--surface);
    border:1px solid var(--border); border-radius:var(--r-controle); cursor:pointer;
  }
  button:hover { background:var(--sel); }
  /* Pass 5 (2026-09-02): in the grid, a button stretches over its
     track by default — the back button keeps its content width. */
  .retour { padding:0 14px; justify-self:start; }
  .principal {
    font-weight:600; color:var(--onAccent); background:var(--accent);
    border-color:var(--accent);
  }
  .principal:hover { background:var(--accentH); border-color:var(--accentH); }

  /* The scene is THE flow (R3): it alone scrolls — the thread lies
     flat inside it, no enclosing surface, border or shadow. */
  /* `scrollbar-gutter` on BOTH sides: the column stays centered at
     the same width as the header's center track, scrollbar or not —
     the alignment holds to the pixel. */
  .scene {
    flex:1; padding:18px 28px 28px; overflow-y:auto; min-height:0;
    scrollbar-gutter:stable both-edges;
  }
  .colonne { max-width:960px; margin:0 auto; min-height:100%; display:flex; flex-direction:column; }
</style>

<script>
  // The “Set aside” pile (PLAN-MODE-ORGANISE E5) — the prototype's
  // shape: a pile-button at the bottom right of the organized Inbox
  // (visual of three sheets, label + count in accent), the FAN of
  // mini-cards on click (one card = subject + sender · time, on tile
  // ground), “See the board” = the previews in a grid on one screen,
  // “Done” sends the message back where it came from. The data comes
  // from the core (`set_aside_pile`, the threads' heads on the
  // unified skeleton); the gestures go up to the App through
  // `onchange` — the component owns the pile, never the lists.
  import Icon from './Icon.svelte';
  import { call } from './lib/transport.js';
  import { when } from './lib/when.js';
  import { t } from './lib/text.svelte.js';

  let { onopen = () => {}, onchange = () => {}, onflash = () => {} } = $props();

  let cards = $state([]);
  let fan = $state(false);
  let board = $state(false);

  export async function reload() {
    try {
      cards = await call('set_aside_pile');
    } catch (err) {
      console.error('pile :', err);
    }
  }
  $effect(() => {
    reload();
  });

  function open(line) {
    fan = false;
    board = false;
    onopen(line);
  }

  // “Done”: the thread leaves the pile and returns where it came
  // from — THE product command, then the pile AND the lists refresh.
  async function finish(line) {
    try {
      await call('toggle_set_aside', {
        accountId: line.account_id,
        mailbox: line.mailbox,
        uid: line.uid,
      });
      onflash(t('toast.resumedPile'));
      await reload();
      if (cards.length === 0) board = false;
      onchange();
    } catch (err) {
      onflash(t('error.preference', { err }));
    }
  }
</script>

<svelte:window
  onkeydown={(e) => {
    if (e.key === 'Escape') {
      fan = false;
      board = false;
    }
  }} />

{#if cards.length > 0}
  <div class="pile-zone">
    {#if fan}
      <div class="eventail" role="dialog" aria-label={t('pile.aria')} data-testid="pile-eventail">
        <p class="tete-e">{t('pile.setAside')}</p>
        {#each cards as line (`${line.account_id}:${line.mailbox}:${line.uid}`)}
          <button type="button" class="carte-e" data-testid="pile-carte"
                  onclick={() => open(line)}>
            <span class="o">{line.subject}</span>
            <span class="e">{line.sender} · {when(line.epoch)}</span>
          </button>
        {/each}
        <div class="pied-e">
          <button type="button" data-testid="pile-voir-tableau"
                  onclick={() => { fan = false; board = true; }}>
            <Icon name="pile" />{t('pile.seeBoard')}</button>
        </div>
      </div>
    {/if}
    <button type="button" class="pile-bouton" data-testid="pile-bouton"
            aria-expanded={fan}
            onclick={() => (fan = !fan)}>
      <span class="pile-visuel" aria-hidden="true"><span></span><span></span><span></span></span>
      <span class="pile-libelle">{t('pile.setAside')} <span class="n">{cards.length}</span></span>
    </button>
  </div>
{/if}

{#if board}
  <!-- The board screen: the previews in a grid, full screen — the
       prototype's overlay (back, title, note, cards). -->
  <div class="tableau" data-testid="pile-tableau">
    <div class="tableau-int">
      <div class="tete-t">
        <button type="button" class="retour-t" data-testid="pile-tableau-retour"
                aria-label={t('action.close')}
                onclick={() => (board = false)}>
          <Icon name="arrow_back" /></button>
        <h2 class="display">{t('pile.boardTitle')}</h2>
      </div>
      <p class="note-t"><Icon name="info" />{t('pile.boardNote')}</p>
      <div class="grille">
        {#each cards as line (`${line.account_id}:${line.mailbox}:${line.uid}`)}
          <div class="carte-t" data-testid="pile-tableau-carte">
            <span class="e">{line.sender} · {when(line.epoch)}</span>
            <span class="o">{line.subject}</span>
            <span class="x">{line.preview ?? ''}</span>
            <div class="actions-t">
              <button type="button" onclick={() => open(line)}>{t('pile.open')}</button>
              <button type="button" data-testid="pile-terminer"
                      onclick={() => finish(line)}>
                <Icon name="check" />{t('action.done')}</button>
            </div>
          </div>
        {/each}
      </div>
    </div>
  </div>
{/if}

<style>
  .pile-zone {
    position:absolute; right:28px; bottom:64px; z-index:20;
    display:flex; flex-direction:column; align-items:flex-end; gap:10px;
  }
  .pile-bouton {
    height:auto; padding:10px 14px 12px; display:flex; flex-direction:column;
    align-items:center; gap:8px; background:var(--surface);
    border:1px solid var(--border); box-shadow:0 8px 24px rgba(0,0,0,.14);
    cursor:pointer;
  }
  .pile-bouton:hover { background:var(--sel); }
  .pile-visuel { position:relative; width:52px; height:38px; }
  .pile-visuel span {
    position:absolute; left:0; right:0; height:30px;
    background:var(--tile); border:1px solid var(--border);
  }
  .pile-visuel span:nth-child(1) { top:8px; transform:rotate(-3deg); }
  .pile-visuel span:nth-child(2) { top:4px; transform:rotate(2deg); }
  .pile-visuel span:nth-child(3) { top:0; background:var(--surface); }
  .pile-libelle {
    font-size:12px; font-weight:600; color:var(--ink2); display:flex; gap:5px;
  }
  .pile-libelle .n { color:var(--accent); font-variant-numeric:tabular-nums; }
  .eventail {
    width:330px; max-height:420px; overflow:auto; display:flex;
    flex-direction:column; background:var(--surface);
    border:1px solid var(--border); box-shadow:0 8px 24px rgba(0,0,0,.14);
  }
  .tete-e {
    margin:0; padding:12px 14px 8px; font-size:11px; letter-spacing:.1em;
    text-transform:uppercase; color:var(--muted); font-weight:600;
  }
  .carte-e {
    display:flex; flex-direction:column; gap:2px; padding:10px 14px;
    border:none; border-top:1px solid var(--border); cursor:pointer;
    text-align:left; background:var(--tile); color:var(--tileInk);
  }
  .carte-e:hover { filter:brightness(0.97); }
  .carte-e .o {
    font-size:13px; font-weight:600; overflow:hidden;
    text-overflow:ellipsis; white-space:nowrap;
  }
  .carte-e .e {
    font-size:12px; opacity:.85; overflow:hidden;
    text-overflow:ellipsis; white-space:nowrap;
  }
  .pied-e { padding:10px 14px; border-top:1px solid var(--border); }
  .pied-e button { width:100%; justify-content:center; }
  /* The board — the full-screen overlay (z above the panes,
     below the modals). */
  .tableau {
    position:fixed; inset:0; z-index:25; background:var(--bg);
    overflow:auto; padding:18px 28px 40px;
  }
  .tableau-int { max-width:1080px; margin:0 auto; }
  .tete-t { display:flex; align-items:center; gap:10px; }
  .tete-t h2 { margin:0; font-size:24px; line-height:1.25; color:var(--ink); }
  .retour-t {
    width:32px; height:32px; padding:0; display:inline-flex;
    align-items:center; justify-content:center; flex:none;
  }
  .note-t {
    display:flex; align-items:baseline; gap:8px; margin:10px 0 22px;
    font-size:13px; line-height:1.5; color:var(--ink2); max-width:70ch;
  }
  .note-t :global(.ic) { color:var(--muted); align-self:center; flex:none; }
  .grille {
    display:grid; grid-template-columns:repeat(auto-fill, minmax(300px, 1fr));
    gap:14px;
  }
  .carte-t {
    display:flex; flex-direction:column; gap:6px; padding:14px;
    background:var(--surface); border:1px solid var(--border);
  }
  .carte-t .e { font-size:12px; color:var(--muted); }
  .carte-t .o { font-size:14px; font-weight:600; color:var(--ink); }
  .carte-t .x {
    font-size:13px; color:var(--ink2); line-height:1.5;
    display:-webkit-box; -webkit-line-clamp:3; -webkit-box-orient:vertical;
    overflow:hidden;
  }
  .actions-t { display:flex; gap:8px; margin-top:6px; }
</style>

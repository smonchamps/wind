<script>
  import Brand from './Brand.svelte';
  // The migration modal (ADR 0012, switch debt §6): exclusive and
  // blocking at startup. Without it, the FIRST command to come along
  // would pay for adopting a legacy database silently, in a frozen
  // interface. “Cancel” undoes everything — the whole pass replays at
  // the next launch, or right away through “Resume”.
  //
  // `ensure()` only returns control once the database is migrated
  // (or if there was nothing to do): the App opens nothing before
  // that. It returns `true` when the database is CONFIRMED clean;
  // `false` when the probe could not answer — the App then forbids
  // itself any optional write (setting the detected language), which
  // would be the first full opening, silent adoption even (A41).
  import { call } from './lib/transport.js';
  import { t } from './lib/text.svelte.js';

  let visible = $state(false);
  let note = $state('');
  let percent = $state(null); // null = preparing, never “0 %”
  let report = $state('');
  let cancelable = $state(true);
  let resume = $state(false);
  let resolveResume = null;

  export async function ensure() {
    let probe;
    try {
      probe = await call('migration_check');
    } catch {
      // Probe impossible: the normal opening will say it better than a
      // pointless screen — startup is not blocked.
      return false;
    }
    if (probe.pending === null || probe.pending === undefined) return true;
    note = t('migration.note', { n: probe.pending });
    visible = true;
    while (!(await onePass())) {
      resume = true;
      await new Promise((resolve) => { resolveResume = resolve; });
      resume = false;
    }
    visible = false;
    return true;
  }

  async function onePass() {
    report = '';
    percent = null;
    cancelable = true;
    const pollTimer = setInterval(async () => {
      try {
        const progress = await call('migration_progress');
        percent = progress.percent ?? null;
      } catch { /* the next reading will suffice */ }
    }, 300);
    try {
      const migrated = await call('migration_run');
      if (migrated) return true;
      report = t('migration.cancelled');
    } catch (err) {
      report = t('migration.failure', { err });
    } finally {
      clearInterval(pollTimer);
    }
    return false;
  }

  function cancel() {
    // One click is enough: the pass cancels at its next checkpoint.
    cancelable = false;
    call('migration_cancel').catch(() => {});
  }
</script>

{#if visible}
  <div class="scrim" data-testid="migration-modal">
    <div class="card" role="dialog" aria-modal="true"
         aria-label={t('migration.aria')}>
      <!-- V11: the brand AS A TILE (fixed outside themes) — the modal
           precedes any theme applied, the tile carries its own ground. -->
      <span class="brand-band"><Brand tile size={28} /><b>Wind</b></span>
      <h3 class="title">{t('migration.title')}</h3>
      <p class="note">{note}</p>
      {#if !report}
        <div class="gauge" class:indeterminate={percent === null}>
          <div class="filled" style="width:{percent ?? 0}%"></div>
        </div>
        <p class="state" data-testid="migration-state">
          {percent === null ? t('migration.preparing') : t('migration.percent', { p: percent })}</p>
        <button type="button" disabled={!cancelable} onclick={cancel}>{t('action.cancel')}</button>
      {:else}
        <p class="state">{report}</p>
        {#if resume}
          <button type="button" class="main"
                  onclick={() => resolveResume?.()}>{t('action.resume')}</button>
        {/if}
      {/if}
    </div>
  </div>
{/if}

<style>
  /* The geometry of screen 01 (520 px column), the modal being silent
     at the prototype: the System fills in, no shape invented. */
  .scrim {
    position:absolute; inset:0; background:var(--bg); z-index:4;
    display:flex; align-items:center; justify-content:center;
  }
  .card { width:520px; display:flex; flex-direction:column; gap:18px; }
  .title {
    margin:0; font-size:32px; line-height:1.15; font-weight:600;
    letter-spacing:-.02em; color:var(--ink);
  }
  .note, .state { margin:0; font-size:13px; line-height:1.5; color:var(--muted); }
  .gauge {
    height:6px; background:var(--bg); border:1px solid var(--border);
    border-radius:var(--r-control); overflow:hidden;
  }
  .filled { height:100%; background:var(--accent); transition:width .3s; }
  .indeterminate .filled { width:30%; animation:va-et-vient 1.2s ease-in-out infinite alternate; }
  @keyframes va-et-vient { from { margin-left:0; } to { margin-left:70%; } }
  button {
    height:32px; padding:0 16px; align-self:flex-start; display:inline-flex;
    align-items:center; gap:8px; font-size:13px; color:var(--ink);
    background:var(--surface); border:1px solid var(--border);
    border-radius:var(--r-control); cursor:pointer;
  }
  button:hover { background:var(--sel); }
  button:disabled { opacity:.6; cursor:default; }
  .main {
    font-weight:600; color:var(--onAccent); background:var(--accent);
    border-color:var(--accent);
  }
  .main:hover { background:var(--accentH); border-color:var(--accentH); }
</style>

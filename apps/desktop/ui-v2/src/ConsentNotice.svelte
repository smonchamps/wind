<script>
  import { t } from './lib/text.svelte.js';
  let { state: consent, oncancel } = $props();
  let copied = $state(false);
  let copyFailed = $state(false);
  async function copy() {
    try {
      await navigator.clipboard.writeText(consent.url);
      copied = true;
      copyFailed = false;
    } catch { copyFailed = true; }
  }
</script>

{#if consent?.active}
  <div class="consent" data-testid="oauth-consent">
    <p role="status">{t(consent.cancelled ? 'consent.cancelled' : consent.finishing
      ? 'consent.finishing' : consent.manual ? 'consent.manual' : 'consent.waiting')}</p>
    {#if consent.url && !consent.cancelled && !consent.finishing}
      <label>{t('consent.link')}
        <input type="text" readonly value={consent.url} onclick={(event) => event.currentTarget.select()}>
      </label>
    {/if}
    <div class="actions">
      {#if consent.url && !consent.cancelled && !consent.finishing}
        <button type="button" onclick={copy}>{t(copied ? 'consent.copied' : 'consent.copy')}</button>
      {/if}
      <button type="button" onclick={oncancel}
              disabled={consent.cancelling || consent.cancelled || consent.finishing}>{t('consent.cancel')}</button>
    </div>
    {#if copyFailed}<p role="alert">{t('consent.copyFailed')}</p>{/if}
    {#if consent.cancelError}<p role="alert">{t('consent.cancelFailed')}</p>{/if}
  </div>
{/if}

<style>
  .consent { display:flex; flex-direction:column; gap:12px; min-width:0; }
  p { margin:0; color:var(--muted); font-size:13px; line-height:1.5; }
  label { display:flex; flex-direction:column; gap:8px; color:var(--ink2); font-size:13px; }
  input { min-width:0; width:100%; height:40px; padding:0 12px; font-size:13px;
    color:var(--ink); background:var(--surface); border:1px solid var(--border); border-radius:var(--r-control); }
  .actions { display:flex; flex-wrap:wrap; gap:12px; }
  button { min-height:36px; padding:0 14px; font-size:13px; cursor:pointer;
    color:var(--ink); background:var(--surface); border:1px solid var(--border); border-radius:var(--r-control); }
  button:hover { background:var(--sel); }
  button:disabled { opacity:.6; cursor:default; }
</style>

<script>
  // The notice slot (PLAN-UI-V2 §6, region A4): at the top, AT MOST
  // ONE notice at a time, by decreasing priority — send failure >
  // update > crash > telemetry > drafts. The style is the Clarity
  // signature; the send failure carries the alert border, it is the
  // only one.
  //
  // The App owns the sources and supplies the chosen notice; this
  // surface only displays and reports the decisions. “Later”
  // dismisses only for the session — the next notice takes its
  // place. The alert reads through the icon (the left divider died
  // with the signature, A29).
  import Icon from './Icon.svelte';
  let { notice = null } = $props();
</script>

{#if notice}
  <div class="fente" class:alerte={notice.alert} data-testid="fente-avis">
    <span class="icone" aria-hidden="true"><Icon name={notice.icon ?? 'info'} /></span>
    <span class="texte" data-testid="avis-texte">{notice.text}</span>
    {#each notice.actions as action (action.label)}
      <button type="button" class:principal={action.primary}
              disabled={action.disabled}
              onclick={action.do}>{action.label}</button>
    {/each}
  </div>
{/if}

<style>
  .fente {
    flex:none; background:var(--surface);
    border-bottom:1px solid var(--border);
    display:flex; align-items:center; gap:14px; padding:10px 24px;
  }
  .icone { color:var(--accent); }
  .alerte .icone { color:var(--alert); }
  .texte { flex:1; font-size:13px; color:var(--ink); min-width:0; }
  button {
    height:32px; padding:0 16px; display:inline-flex; align-items:center;
    gap:8px; font-size:13px; color:var(--ink); background:var(--surface);
    border:1px solid var(--border); border-radius:var(--r-control); cursor:pointer;
    flex:none;
  }
  button:hover { background:var(--sel); }
  button:disabled { opacity:.6; cursor:default; }
  .principal {
    font-weight:600; color:var(--onAccent); background:var(--accent);
    border-color:var(--accent);
  }
  .principal:hover { background:var(--accentH); border-color:var(--accentH); }
</style>

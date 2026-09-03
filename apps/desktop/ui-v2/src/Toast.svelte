<script>
  // Toast: bottom centered, checkmark in accent (as a stroke — V8,
  // the Elements set has no filled state; the state reads through
  // the background and the text).
  import Icon from './Icon.svelte';
  let { message = null } = $props();
</script>

{#if message}
  <div class="toast" data-testid="toast" role="status">
    <span class="check" aria-hidden="true"><Icon name="check_circle" /></span>
    <span class="text">{message}</span>
  </div>
{/if}

<style>
  .toast {
    /* Above EVERYTHING — conversation (1) and overlays (2): the
       toast often confirms the action that just closed one of them. */
    position:absolute; left:50%; bottom:28px; transform:translateX(-50%);
    z-index:3;
    background:var(--surface); border:1px solid var(--border);
    border-radius:var(--r-control);
    box-shadow:var(--shadow); padding:10px 16px;
    display:flex; align-items:center; gap:10px;
  }
  .check { color:var(--accent); }
  .text { font-size:13px; color:var(--ink); }
</style>

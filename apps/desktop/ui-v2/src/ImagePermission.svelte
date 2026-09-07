<script>
  import { call } from './lib/transport.js';
  import { imagePermissionsChanged } from './lib/image-permissions.js';
  import { t } from './lib/text.svelte.js';
  let { message, onflash = () => {} } = $props();
  let busy = $state(false);
  async function revoke() {
    if (busy) return;
    busy = true;
    try {
      const stillAllowed = await call('revoke_images_message', {
        accountId: message.account_id, mailbox: message.mailbox, uid: message.uid, version: message.version,
      });
      imagePermissionsChanged();
      onflash(t(stillAllowed ? 'reading.senderStillAllowsImages' : 'reading.imagePermissionRemoved'));
    } catch (err) { onflash(t('error.imagePermission', { err })); }
    finally { busy = false; }
  }
</script>

<div class="permission" data-testid="message-image-permission">
  <span>{t('reading.messageImagesAllowed')}</span>
  <button type="button" data-testid="revoke-message-images" disabled={busy} onclick={revoke}>
    {t('reading.revokeImages')}
  </button>
</div>

<style>
  .permission { display:flex; align-items:center; gap:10px; flex-wrap:wrap; padding:10px 14px;
    font-size:13px; color:var(--ink2); background:var(--bg); border:1px solid var(--border);
    border-radius:var(--r-control); }
  span { flex:1; }
  button { height:26px; padding:0 10px; font-size:12px; color:var(--ink); background:var(--surface);
    border:1px solid var(--border); border-radius:var(--r-control); cursor:pointer; }
  button:hover { background:var(--sel); }
</style>

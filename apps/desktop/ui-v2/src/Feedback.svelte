<script>
  // The beta feedback form (PLAN-RETOURS-11 R3, field pass of
  // 2026-08-28): one field, one send. The message goes out through
  // THE send queue (`queue_send`) — the golden rule “never a lost
  // send”, the offline net and the status bar apply here for free,
  // no new send path. The feedback address is a product constant
  // (CE decision D7 / beta field); the sender is the workstation's
  // first account — the button does not exist without an account
  // (App.svelte guards it).
  import Icon from './Icon.svelte';
  import { call } from './lib/transport.js';
  import { t } from './lib/text.svelte.js';

  const FEEDBACK_ADDRESS = 'feedback-wind@fcts.io';

  let { accounts = [], onflash = () => {} } = $props();

  let visible = $state(false);
  let text = $state('');
  let sendInProgress = $state(false);
  let field = $state(null);

  export function open() {
    text = '';
    visible = true;
  }
  export function isOpen() {
    return visible;
  }
  export function close() {
    visible = false;
  }

  $effect(() => {
    if (visible) field?.focus();
  });

  async function send() {
    const account = accounts[0];
    if (sendInProgress || !text.trim() || !account) return;
    sendInProgress = true;
    try {
      // The version travels in the SUBJECT — reviewing the feedback
      // needs it, and the tester has nothing to copy over. If it is
      // missing, the feedback still goes out.
      let version = '';
      try {
        version = await call('app_version');
      } catch {
        version = '';
      }
      await call('queue_send', {
        accountId: account.account_id,
        to: FEEDBACK_ADDRESS,
        cc: '',
        bcc: '',
        subject: version ? `Retour Wind ${version}` : 'Retour Wind',
        body: text.trim(),
        bodyHtml: null,
        replyToMailbox: null,
        replyToUid: null,
        draftId: null,
        important: false,
        sendAtEpoch: null,
      });
    } catch (err) {
      onflash(t('error.send', { err }));
      return;
    } finally {
      sendInProgress = false;
    }
    visible = false;
    onflash(t('feedback.thanks'));
    // The IMMEDIATE send, like the composer (field pass of 2026-08-28:
    // without it, the feedback waited for the next flush — cycle or
    // manual sync). In the background; offline, the queue waits
    // and the status bar says so — the normal send path.
    call('flush_outbox').catch((err) => console.error('flush_outbox :', err));
  }
</script>

{#if visible}
  <div class="scrim" role="presentation"
       onclick={(e) => { if (e.target === e.currentTarget) close(); }}>
    <!-- tabindex -1: the dialog is programmatically focusable (a11y
         of the role); the real focus goes to the field on opening. -->
    <div class="card" role="dialog" aria-modal="true" tabindex="-1"
         aria-label={t('feedback.title')} data-testid="back-card"
         onkeydown={(e) => { if (e.key === 'Escape') close(); }}>
      <div class="head">
        <Icon name="feedback" />
        <span class="title">{t('feedback.title')}</span>
        <button type="button" class="close" aria-label={t('action.close')}
                onclick={close}><Icon name="close" /></button>
      </div>
      <p class="under">{t('feedback.subtitle')}</p>
      <textarea bind:this={field} bind:value={text} rows="6"
                data-testid="back-text"
                placeholder={t('feedback.placeholder')}
                aria-label={t('feedback.title')}></textarea>
      <div class="foot">
        <button type="button" class="secondary" onclick={close}>
          {t('action.cancel')}</button>
        <!-- “Send” ABSENT as long as the field is empty — never
             greyed out (the onboarding journey's rule, RETOURS-8 D4). -->
        {#if text.trim() && !sendInProgress}
          <button type="button" class="main" data-testid="back-send"
                  onclick={send}>{t('action.send')}</button>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  /* The overlay with the Settings card's tokens, at small size. */
  .scrim {
    position:absolute; inset:0; background:var(--scrim); z-index:2;
    display:flex; align-items:center; justify-content:center; padding:36px;
  }
  .card {
    width:520px; max-width:100%; background:var(--surface);
    border:1px solid var(--border);
    border-radius:var(--r-surface); box-shadow:var(--shadow);
    display:flex; flex-direction:column; gap:10px; padding:18px 22px 16px;
  }
  .head { display:flex; align-items:center; gap:10px; color:var(--ink); }
  .head :global(.ic) { color:var(--ink2); }
  .title { font-size:15px; font-weight:600; flex:1; }
  .close {
    height:32px; width:32px; padding:0; display:inline-flex;
    align-items:center; justify-content:center; color:var(--ink2);
    background:var(--surface); border:1px solid var(--border);
    border-radius:var(--r-control); cursor:pointer;
  }
  .close:hover { background:var(--sel); }
  .under { margin:0; font-size:12px; line-height:1.5; color:var(--muted); }
  textarea {
    resize:vertical; min-height:110px; padding:10px 12px;
    font:inherit; font-size:13px; color:var(--ink);
    background:var(--bg); border:1px solid var(--border);
    border-radius:var(--r-control);
  }
  textarea:focus-visible { outline:2px solid var(--accent); outline-offset:2px; }
  .foot { display:flex; justify-content:flex-end; gap:8px; }
  .secondary, .main {
    height:32px; padding:0 14px; font-size:13px; cursor:pointer;
    border-radius:var(--r-control);
  }
  .secondary {
    color:var(--ink); background:var(--surface);
    border:1px solid var(--border);
  }
  .secondary:hover { background:var(--sel); }
  .main {
    color:var(--onAccent); background:var(--accent);
    border:1px solid var(--accent); font-weight:600;
  }
  .main:hover { background:var(--accentH); border-color:var(--accentH); }
</style>

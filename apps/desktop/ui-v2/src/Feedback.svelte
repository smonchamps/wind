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
    <div class="carte" role="dialog" aria-modal="true" tabindex="-1"
         aria-label={t('feedback.title')} data-testid="retour-carte"
         onkeydown={(e) => { if (e.key === 'Escape') close(); }}>
      <div class="tete">
        <Icon name="feedback" />
        <span class="titre">{t('feedback.title')}</span>
        <button type="button" class="fermer" aria-label={t('action.close')}
                onclick={close}><Icon name="close" /></button>
      </div>
      <p class="sous">{t('feedback.subtitle')}</p>
      <textarea bind:this={field} bind:value={text} rows="6"
                data-testid="retour-texte"
                placeholder={t('feedback.placeholder')}
                aria-label={t('feedback.title')}></textarea>
      <div class="pied">
        <button type="button" class="secondaire" onclick={close}>
          {t('action.cancel')}</button>
        <!-- “Send” ABSENT as long as the field is empty — never
             greyed out (the onboarding journey's rule, RETOURS-8 D4). -->
        {#if text.trim() && !sendInProgress}
          <button type="button" class="principal" data-testid="retour-envoyer"
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
  .carte {
    width:520px; max-width:100%; background:var(--surface);
    border:1px solid var(--border);
    border-radius:var(--r-surface); box-shadow:var(--shadow);
    display:flex; flex-direction:column; gap:10px; padding:18px 22px 16px;
  }
  .tete { display:flex; align-items:center; gap:10px; color:var(--ink); }
  .tete :global(.ic) { color:var(--ink2); }
  .titre { font-size:15px; font-weight:600; flex:1; }
  .fermer {
    height:32px; width:32px; padding:0; display:inline-flex;
    align-items:center; justify-content:center; color:var(--ink2);
    background:var(--surface); border:1px solid var(--border);
    border-radius:var(--r-control); cursor:pointer;
  }
  .fermer:hover { background:var(--sel); }
  .sous { margin:0; font-size:12px; line-height:1.5; color:var(--muted); }
  textarea {
    resize:vertical; min-height:110px; padding:10px 12px;
    font:inherit; font-size:13px; color:var(--ink);
    background:var(--bg); border:1px solid var(--border);
    border-radius:var(--r-control);
  }
  textarea:focus-visible { outline:2px solid var(--accent); outline-offset:2px; }
  .pied { display:flex; justify-content:flex-end; gap:8px; }
  .secondaire, .principal {
    height:32px; padding:0 14px; font-size:13px; cursor:pointer;
    border-radius:var(--r-control);
  }
  .secondaire {
    color:var(--ink); background:var(--surface);
    border:1px solid var(--border);
  }
  .secondaire:hover { background:var(--sel); }
  .principal {
    color:var(--onAccent); background:var(--accent);
    border:1px solid var(--accent); font-weight:600;
  }
  .principal:hover { background:var(--accentH); border-color:var(--accentH); }
</style>

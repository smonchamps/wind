<script>
  // Shared form for onboarding and Settings.
  import { onDestroy, untrack } from 'svelte';
  import ConsentNotice from './ConsentNotice.svelte';
  import { createConsent } from './lib/consent.js';
  import { call } from './lib/transport.js';
  import { t } from './lib/text.svelte.js';
  import { connectionError } from './lib/auth-error.js';
  import { IMPORT_HORIZONS as HORIZONS } from './lib/vocabularies.js';

  let {
    onadd = () => {},
    compact = false,
    repair = null,
    onboarding = false,
    mainAdd = false,
    // 3rd field pass (finding 2): onboarding hides its “Continue”
    // when the generic desk is revealed — it needs to know this.
    ongeneric = () => {},
  } = $props();

  let consentState = $state(null);
  const consent = createConsent(call, (value) => { consentState = value; });
  onDestroy(() => consent.dispose());

  const initial = untrack(() => repair);
  let address = $state(initial?.email ?? '');
  // ADR 0029 (D1/D2): the depth of history imported locally — the
  // choice travels INSIDE the add command (the account id exists only
  // once it returns). Default “1 year” (CE decision D2).
  let horizon = $state('1a');
  let generic = $state(!!initial);
  let password = $state('');
  let username = $state(initial?.username ?? null);
  let imapHost = $state(initial?.imapHost ?? '');
  let imapPort = $state(String(initial?.imapPort ?? 993));
  let smtpHost = $state(initial?.smtpHost ?? '');
  let smtpPort = $state(String(initial?.smtpPort ?? 465));
  let busy = $state(false);
  let pending = $state('');
  let error = $state('');

  const domain = () => (address.split('@')[1] ?? '').toLowerCase();
  const isGoogle = () => ['gmail.com', 'googlemail.com'].includes(domain());
  const isMicrosoft = () =>
    /^(outlook|hotmail|live|msn)\./.test(domain()) || domain() === 'outlook.com';

  async function proceed() {
    error = '';
    const typing = address.trim();
    if (!typing.includes('@')) {
      error = t('desk.addressInvalid');
      return;
    }
    if (!repair && (isGoogle() || isMicrosoft())) {
      busy = true;
      pending = t('desk.authorization');
      try {
        const connected = await consent.run(isGoogle() ? 'add_account' : 'add_microsoft_account',
          { email: typing, horizon });
        if (connected) onadd();
      } catch (err) {
        error = connectionError(err, 'error.connection', isGoogle() ? 'Google' : 'Microsoft');
      } finally {
        busy = false;
        pending = '';
      }
      return;
    }
    if (!generic) {
      // Unknown domain: the generic desk reveals itself, nothing is sent.
      generic = true;
      ongeneric(true);
      if (!imapHost) imapHost = `imap.${domain()}`;
      if (!smtpHost) smtpHost = `smtp.${domain()}`;
      return;
    }
    busy = true;
    pending = t('desk.checking');
    try {
      await call(repair ? 'repair_generic_account' : 'add_generic_account', {
        ...(repair ? { accountId: repair.accountId } : {}),
        input: {
          email: typing,
          username: username?.trim() || typing,
          password: password,
          imapHost: imapHost.trim(),
          imapPort: Number(imapPort) || 993,
          smtpHost: smtpHost.trim(),
          smtpPort: Number(smtpPort) || 465,
        },
        horizon,
      });
      password = '';
      onadd();
    } catch (err) {
      error = t('error.connection', { err });
    } finally {
      busy = false;
      pending = '';
    }
  }
</script>

<div class="desk" class:compact class:onboarding aria-busy={busy}>
  <div class="form">
    <label for="ob-adresse">{t('desk.address')}</label>
    <div class="bar">
      <input id="ob-adresse" type="email" bind:value={address} disabled={busy || !!repair}
             data-testid="onboarding-address"
             onkeydown={(e) => e.key === 'Enter' && !busy && proceed()}>
      {#if onboarding && !generic}
        <button type="button" class={mainAdd ? 'primary' : 'secondary'}
                data-testid="desk-continue"
                disabled={busy} onclick={proceed}>{t('onboarding.add')}</button>
      {/if}
    </div>
    <!-- ADR 0029: the depth of history — visible on BOTH surfaces and
         for the three flows (the choice travels with the add). -->
    {#if !repair}
    <div class="horizon">
      <label for="ob-horizon">{t('desk.horizon')}</label>
      <span class="select-wrap">
        <select id="ob-horizon" class="select-desk lg" bind:value={horizon} disabled={busy}
                data-testid="desk-horizon">
          {#each HORIZONS as h (h)}
            <option value={h}>{t(`horizon.${h}`)}</option>
          {/each}
        </select>
      </span>
    </div>
    {/if}
    {#if generic}
      <label for="ob-username">{t('desk.username')}</label>
      <input id="ob-username" type="text" autocomplete="username" aria-describedby="ob-username-hint"
             value={username ?? address.trim()} disabled={busy || !!repair}
             oninput={(event) => { username = event.currentTarget.value; }}>
      <p id="ob-username-hint" class="note">{t('desk.usernameHint')}</p>
      <label for="ob-mdp">{t('desk.password')}</label>
      <input id="ob-mdp" type="password" bind:value={password} disabled={busy}
             aria-describedby={repair ? "repair-password-hint" : undefined}>
      {#if repair}<p id="repair-password-hint" class="note">{t('desk.keepPassword')}</p>{/if}
      <div class="servers">
        <span>
          <label for="ob-imap">{t('desk.imap')}</label>
          <input id="ob-imap" type="text" bind:value={imapHost} disabled={busy || !!repair}>
        </span>
        <span class="port">
          <label for="ob-imap-port">{t('desk.port')}</label>
          <input id="ob-imap-port" type="text" bind:value={imapPort} disabled={busy}>
        </span>
      </div>
      <div class="servers">
        <span>
          <label for="ob-smtp">{t('desk.smtp')}</label>
          <input id="ob-smtp" type="text" bind:value={smtpHost} disabled={busy}>
        </span>
        <span class="port">
          <label for="ob-smtp-port">{t('desk.port')}</label>
          <input id="ob-smtp-port" type="text" bind:value={smtpPort} disabled={busy}>
        </span>
      </div>
    {/if}
    {#if !onboarding}
      <button type="button" class="main" data-testid="desk-continue"
              disabled={busy} onclick={proceed}>{t(repair ? 'desk.saveConnection' : 'action.continue')}</button>
    {:else if generic}
      <!-- Finding 3: on the revealed generic desk, the action is
           “Add” (secondary) and “Back” folds the fields back. -->
      <!-- 3rd pass (finding 2): on the revealed generic desk,
           “Add” is THE gesture — always primary (the walkthrough's
           Continue is hidden meanwhile). -->
      <div class="actions">
        <button type="button" class="primary"
                data-testid="desk-continue"
                disabled={busy} onclick={proceed}>{t('onboarding.add')}</button>
        <button type="button" class="secondary" data-testid="desk-back"
                disabled={busy}
                onclick={() => { generic = false; error = ''; password = ''; username = null; ongeneric(false); }}>{t('onboarding.back')}</button>
      </div>
    {/if}
  </div>
  {#if error}
    <p class="error" data-testid="onboarding-error">{error}</p>
  {:else if consentState?.active}
    <ConsentNotice state={consentState} oncancel={() => consent.cancel()} />
  {:else if pending}
    <p class="note">{pending}</p>
  {:else if generic}
    <p class="note">{t('desk.noteGeneric')}</p>
  {/if}
  <!-- The “server auto-detected” note is dead (CE feedback 2026-08-30,
       visual STOP EA2): superfluous. -->
</div>

<style>
  /* Tokens of the prototype's screen 01; `compact` for Settings. */
  .desk { display:flex; flex-direction:column; gap:14px; }
  .form { display:flex; flex-direction:column; gap:12px; }
  label { font-size:13px; color:var(--ink2); }
  input {
    height:52px; font-size:15px; padding:0 16px; background:var(--surface);
    color:var(--ink); border:1px solid var(--border); border-radius:var(--r-control);
    box-shadow:var(--shadow); outline:none; width:100%;
  }
  .compact input { height:40px; font-size:13px; box-shadow:none; }
  /* The horizon selector: the entries' drawing, at reduced height —
     a setting, not an input (native selector, A26's pattern). */
  .horizon { display:flex; flex-direction:column; gap:12px; }
  /* .select-desk.lg (system.css, D-47): the shared select shape at
     this component's 40 px site variation. */
  .servers { display:flex; gap:12px; }
  .servers span { display:flex; flex-direction:column; gap:12px; flex:1; }
  .servers .port { flex:0 0 110px; }
  .main {
    height:32px; padding:0 16px; align-self:flex-start; font-size:13px;
    font-weight:600; color:var(--onAccent); background:var(--accent);
    border:1px solid var(--accent); border-radius:var(--r-control); cursor:pointer;
  }
  .main:hover { background:var(--accentH); border-color:var(--accentH); }
  .main:disabled { opacity:.6; cursor:default; }
  /* The bar: the input + its button, on the same row in `onboarding` —
     at the SAME height (2nd field pass, finding 2). */
  .bar { display:flex; gap:12px; }
  .bar input { flex:1; min-width:0; }
  .onboarding .bar input { height:40px; font-size:14px; }
  .secondary, .primary {
    height:40px; padding:0 18px; flex:none; align-self:center;
    font-size:13px; font-weight:600; border-radius:var(--r-control); cursor:pointer;
  }
  .secondary {
    color:var(--ink); background:var(--surface);
    border:1px solid var(--border);
  }
  .secondary:hover { background:var(--sel); }
  /* Finding 1 (2nd pass): as long as Continue is greyed out, “Add”
     is THE gesture — it takes the primary drawing. */
  .primary {
    color:var(--onAccent); background:var(--accent);
    border:1px solid var(--accent);
  }
  .primary:hover { background:var(--accentH); border-color:var(--accentH); }
  .secondary:disabled, .primary:disabled { opacity:.6; cursor:default; }
  .actions { display:flex; gap:12px; }
  .actions .secondary, .actions .primary { align-self:flex-start; }
  .note { margin:0; font-size:13px; line-height:1.5; color:var(--muted); }
  .error { margin:0; font-size:13px; line-height:1.5; color:var(--alert); }
</style>

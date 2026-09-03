<script>
  // The account-add desk — ONE implementation, two surfaces: screen 01
  // (zero accounts) and the “Accounts” section of Settings (A11).
  // Simple gate (D4): the domain picks the existing flow — Gmail and
  // Microsoft through the browser consent, any other domain reveals
  // the generic desk's IMAP/SMTP fields.
  //
  // `compact` tightens the geometry to live in the Settings overlay
  // (40 px entries); screen 01 keeps its prototype 52 px.
  // `accueil` (field 2026-08-22, findings 1-3 of the two passes): the
  // redesigned screen 01's presentation — the address bar (40 px, its
  // button's height) carries “Add” on its right, and the revealed
  // generic desk gains a “Back” that folds the server fields back.
  // `mainAdd`: as long as the walkthrough's Continue is greyed
  // out (no account), “Add” is THE gesture — primary; once an account
  // exists, it becomes secondary again. The “server detected” note is
  // not shown during onboarding (2nd pass, finding 2).
  import { call } from './lib/transport.js';
  import { t } from './lib/text.svelte.js';
  import { IMPORT_HORIZONS as HORIZONS } from './lib/vocabularies.js';

  let {
    onadd = () => {},
    compact = false,
    accueil = false,
    mainAdd = false,
    // 3rd field pass (finding 2): onboarding hides its “Continue”
    // when the generic desk is revealed — it needs to know this.
    ongeneric = () => {},
  } = $props();

  let address = $state('');
  // ADR 0029 (D1/D2): the depth of history imported locally — the
  // choice travels INSIDE the add command (the account id exists only
  // once it returns). Default “1 year” (CE decision D2).
  let horizon = $state('1a');
  let generic = $state(false);
  let password = $state('');
  let imapHost = $state('');
  let imapPort = $state('993');
  let smtpHost = $state('');
  let smtpPort = $state('465');
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
    if (isGoogle() || isMicrosoft()) {
      busy = true;
      pending = t('desk.authorization');
      try {
        await (isGoogle()
          ? call('add_account', { horizon })
          : call('add_microsoft_account', { email: typing, horizon }));
        onadd();
      } catch (err) {
        error = t('error.connection', { err });
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
      await call('add_generic_account', {
        input: {
          email: typing,
          username: null,
          password: password,
          imapHost: imapHost.trim(),
          imapPort: Number(imapPort) || 993,
          smtpHost: smtpHost.trim(),
          smtpPort: Number(smtpPort) || 465,
        },
        horizon,
      });
      onadd();
    } catch (err) {
      error = t('error.connection', { err });
    } finally {
      busy = false;
      pending = '';
    }
  }
</script>

<div class="guichet" class:compact class:accueil>
  <div class="formulaire">
    <label for="ob-adresse">{t('desk.address')}</label>
    <div class="barre">
      <input id="ob-adresse" type="email" bind:value={address}
             data-testid="onboarding-adresse"
             onkeydown={(e) => e.key === 'Enter' && !busy && proceed()}>
      {#if accueil && !generic}
        <button type="button" class={mainAdd ? 'primaire' : 'secondaire'}
                data-testid="onboarding-continuer"
                disabled={busy} onclick={proceed}>{t('onboarding.add')}</button>
      {/if}
    </div>
    <!-- ADR 0029: the depth of history — visible on BOTH surfaces and
         for the three flows (the choice travels with the add). -->
    <div class="horizon">
      <label for="ob-horizon">{t('desk.horizon')}</label>
      <select id="ob-horizon" bind:value={horizon} disabled={busy}
              data-testid="guichet-horizon">
        {#each HORIZONS as h (h)}
          <option value={h}>{t(`horizon.${h}`)}</option>
        {/each}
      </select>
    </div>
    {#if generic}
      <label for="ob-mdp">{t('desk.password')}</label>
      <input id="ob-mdp" type="password" bind:value={password}>
      <div class="serveurs">
        <span>
          <label for="ob-imap">{t('desk.imap')}</label>
          <input id="ob-imap" type="text" bind:value={imapHost}>
        </span>
        <span class="port">
          <label for="ob-imap-port">{t('desk.port')}</label>
          <input id="ob-imap-port" type="text" bind:value={imapPort}>
        </span>
      </div>
      <div class="serveurs">
        <span>
          <label for="ob-smtp">{t('desk.smtp')}</label>
          <input id="ob-smtp" type="text" bind:value={smtpHost}>
        </span>
        <span class="port">
          <label for="ob-smtp-port">{t('desk.port')}</label>
          <input id="ob-smtp-port" type="text" bind:value={smtpPort}>
        </span>
      </div>
    {/if}
    {#if !accueil}
      <button type="button" class="principal" data-testid="onboarding-continuer"
              disabled={busy} onclick={proceed}>{t('action.continue')}</button>
    {:else if generic}
      <!-- Finding 3: on the revealed generic desk, the action is
           “Add” (secondary) and “Back” folds the fields back. -->
      <!-- 3rd pass (finding 2): on the revealed generic desk,
           “Add” is THE gesture — always primary (the walkthrough's
           Continue is hidden meanwhile). -->
      <div class="actions">
        <button type="button" class="primaire"
                data-testid="onboarding-continuer"
                disabled={busy} onclick={proceed}>{t('onboarding.add')}</button>
        <button type="button" class="secondaire" data-testid="guichet-retour"
                disabled={busy}
                onclick={() => { generic = false; error = ''; ongeneric(false); }}>{t('onboarding.back')}</button>
      </div>
    {/if}
  </div>
  {#if error}
    <p class="erreur" data-testid="onboarding-erreur">{error}</p>
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
  .guichet { display:flex; flex-direction:column; gap:14px; }
  .formulaire { display:flex; flex-direction:column; gap:12px; }
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
  select {
    height:40px; font-size:13px; padding:0 12px; background:var(--surface);
    color:var(--ink); border:1px solid var(--border);
    border-radius:var(--r-control); outline:none; align-self:flex-start;
    min-width:220px; cursor:pointer;
  }
  select:disabled { opacity:.6; cursor:default; }
  .serveurs { display:flex; gap:12px; }
  .serveurs span { display:flex; flex-direction:column; gap:12px; flex:1; }
  .serveurs .port { flex:0 0 110px; }
  .principal {
    height:32px; padding:0 16px; align-self:flex-start; font-size:13px;
    font-weight:600; color:var(--onAccent); background:var(--accent);
    border:1px solid var(--accent); border-radius:var(--r-control); cursor:pointer;
  }
  .principal:hover { background:var(--accentH); border-color:var(--accentH); }
  .principal:disabled { opacity:.6; cursor:default; }
  /* The bar: the input + its button, on the same row in `accueil` —
     at the SAME height (2nd field pass, finding 2). */
  .barre { display:flex; gap:12px; }
  .barre input { flex:1; min-width:0; }
  .accueil .barre input { height:40px; font-size:14px; }
  .secondaire, .primaire {
    height:40px; padding:0 18px; flex:none; align-self:center;
    font-size:13px; font-weight:600; border-radius:var(--r-control); cursor:pointer;
  }
  .secondaire {
    color:var(--ink); background:var(--surface);
    border:1px solid var(--border);
  }
  .secondaire:hover { background:var(--sel); }
  /* Finding 1 (2nd pass): as long as Continue is greyed out, “Add”
     is THE gesture — it takes the primary drawing. */
  .primaire {
    color:var(--onAccent); background:var(--accent);
    border:1px solid var(--accent);
  }
  .primaire:hover { background:var(--accentH); border-color:var(--accentH); }
  .secondaire:disabled, .primaire:disabled { opacity:.6; cursor:default; }
  .actions { display:flex; gap:12px; }
  .actions .secondaire, .actions .primaire { align-self:flex-start; }
  .note { margin:0; font-size:13px; line-height:1.5; color:var(--muted); }
  .erreur { margin:0; font-size:13px; line-height:1.5; color:var(--alert); }
</style>

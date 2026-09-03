<script>
  // Screen 01, redesigned as a first-launch journey (PLAN-RETOURS-8
  // R2, A75 — reverses "the onboarding that only asks for an address";
  // form settled at the field of 2026-08-22, findings 1-8): five
  // steps — accounts, layout, theme, beta (RETOURS-11, field of
  // 2026-08-28), summary. Each step says its title, then
  // "Step n/5", then its text. Two regimes:
  // `complete` (first install: the whole journey, the mark
  // `wind-accueil-fait` set at Finish) and desk only (a workstation
  // back to zero accounts: onboarding without steps, which fades away
  // at the first account — the prior behavior, unchanged).
  //
  // Step 2 shows REAL SCREENSHOTS of the application (Clarity
  // fixture, e2e/capture-accueil.mjs — finding 5); step 3 draws
  // its windows in the colors of CARDS AND in the layout chosen
  // at step 2 (finding 6). Step 5 recaps the three choices —
  // each is a door back to its step (finding 8). Steps
  // 2 and 3 touch neither the database nor the network:
  // `applyPanes` / `applyTheme` apply AND persist.
  // Only step 1 (AccountDesk, A11) talks to the shell.
  import Icon from './Icon.svelte';
  import AccountDesk from './AccountDesk.svelte';
  import Brand from './Brand.svelte';
  import { t } from './lib/text.svelte.js';
  import { THEME_CARDS, applyTheme, displayedTheme } from './lib/theme.js';
  import { currentPanes, applyPanes } from './lib/panes.svelte.js';
  import { markOnboardingDone, markOnboardingStarted } from './lib/onboarding.js';
  import preview3 from './assets/accueil/disposition-3.png';
  import preview2 from './assets/accueil/disposition-2.png';
  import preview1 from './assets/accueil/disposition-1.png';

  let { accounts = [], complete = false, onadd = () => {}, onfinish = () => {} } = $props();

  const STEPS = 5;
  const PREVIEWS = { 3: preview3, 2: preview2, 1: preview1 };
  let step = $state(1);
  // Finding 2: as soon as one address exists, the add bar folds
  // behind "Add another email address" — and reopens on
  // click. `onAdd` folds it back after every successful add.
  let addOpen = $state(false);
  // 2nd field pass, finding 3: step 2's preview is ONE image —
  // the hovered button's for as long as it is hovered, the chosen one's otherwise.
  let panesHover = $state(null);
  // 3rd pass, finding 2: the revealed generic desk takes over the
  // whole step bar — the journey's "Continue" hides during that time.
  let genericOpen = $state(false);
  // D4: step 1 requires at least one account — Wind is usable as soon
  // as the journey ends.
  const canContinue = $derived(step !== 1 || accounts.length > 0);
  // The chosen pane layout is READ from the shared module ($state runes) —
  // no local mirror to resynchronize (review 2026-08-22).
  const panes = $derived(currentPanes());
  // The checkmark follows the DISPLAYED card (the Settings pattern,
  // review A42): under OS dark tracking, it's the -night variant that
  // gets checked — even when the OS switches during step 3 (same
  // listener as Settings).
  let activeTheme = $state(displayedTheme());
  // The card of the DISPLAYED theme — the summary's thumbnail
  // colors itself from it (2nd pass, finding 4).
  const activeCard = $derived(THEME_CARDS.find((f) => f.id === activeTheme) ?? THEME_CARDS[0]);
  $effect(() => {
    const track = () => (activeTheme = displayedTheme());
    document.addEventListener('wind:theme-affiche', track);
    return () => document.removeEventListener('wind:theme-affiche', track);
  });
  // The "started" mark is set as soon as the journey DISPLAYS: an
  // account added at step 1 then the app quit before Finish does not
  // make an install "already onboarded" — the journey
  // resumes (review 2026-08-22).
  $effect(() => {
    if (complete) markOnboardingStarted();
  });

  function onAdd() {
    addOpen = false;
    genericOpen = false;
    onadd();
  }
  function proceed() {
    if (!canContinue) return;
    if (step < STEPS) step += 1;
  }
  function back() {
    if (step > 1) step -= 1;
  }
  function chooseTheme(id) {
    applyTheme(id);
    activeTheme = displayedTheme();
  }
  function finish() {
    markOnboardingDone();
    onfinish();
  }
</script>

<div class="ecran01" data-testid="onboarding">
  <!-- The window-preview of step 3: n = drawn layout (the one
       chosen at step 2, finding 6), c = a card's colors. V3:
       header and nav are no longer inset (--panel is dead) — the
       card's STROKE alone draws the separation, as in the product. -->
  {#snippet window(n, c)}
    <span class="fenetre" aria-hidden="true"
          style="background:{c.bg}; border-color:{c.border}">
      <span class="f-tete" style="border-color:{c.border}"></span>
      <span class="f-corps">
        {#if n !== 1}<span class="f-nav" style="border-right:1px solid {c.border}"></span>{/if}
        <span class="f-liste">
          <span style="background:{c.surface}"></span>
          <span style="background:{c.accent}"></span>
          <span style="background:{c.surface}"></span>
        </span>
        {#if n === 3}<span class="f-lecture"
              style="background:{c.surface}; border-color:{c.border}"></span>{/if}
      </span>
    </span>
  {/snippet}
  {#snippet progress(n)}
    {#if complete}
      <p class="progression" data-testid="accueil-progression">
        {t('onboarding.step', { n, total: STEPS })}</p>
    {/if}
  {/snippet}
  <div class="colonne" class:large={complete && step !== 1}>
    {#if !complete || step === 1}
      <!-- Finding 1: "Welcome to Wind" — the TILED brand
           (V11: frozen across themes, platform radius) replaces the
           hitofude stroke (V2). The desk block stays ONE (step 1 of
           the journey AND the screen of an onboarded workstation back
           to zero accounts). -->
      <h3 class="titre display"><Brand tile size={40} />
        <span>{t('onboarding.welcome')} <span class="marque">Wind</span></span></h3>
      {@render progress(1)}
      <p class="sous">{t('onboarding.addUnder')}</p>
      {#if complete && accounts.length > 0}
        <ul class="ajoutes" data-testid="accueil-comptes">
          {#each accounts as c (c.account_id)}
            <li><Icon name="check_circle" />{c.email}</li>
          {/each}
        </ul>
      {/if}
      {#if accounts.length === 0 || addOpen}
        <!-- Finding 1 (2nd pass): as long as Continue is not there
             (no account), "Add" is THE gesture — primary. -->
        <AccountDesk accueil mainAdd={accounts.length === 0}
                       ongeneric={(v) => (genericOpen = v)}
                       onadd={onAdd} />
      {:else}
        <!-- Finding 2: the bar folded behind an explicit door. -->
        <button type="button" class="secondaire" data-testid="accueil-ajouter-autre"
                onclick={() => (addOpen = true)}>
          {t('onboarding.addOther')}</button>
      {/if}
    {:else if step === 2}
      <h3 class="titre display">{t('onboarding.panesTitle')}</h3>
      {@render progress(2)}
      <p class="sous">{t('onboarding.panesUnder')}</p>
      <!-- Findings 5 (1st pass), 3 (2nd) and 3 (3rd pass): ONE REAL
           screenshot of the application — refreshed on hover (and on
           keyboard focus, A8) of the targeted button, back to the
           choice otherwise —, the image + buttons set together in one
           elevation, buttons centered. -->
      <div class="cadre-volets">
        <img class="capture-grande" data-testid="accueil-apercu"
             data-volets={panesHover ?? panes}
             src={PREVIEWS[panesHover ?? panes]} alt="" />
        <div class="boutons-volets" data-testid="accueil-volets">
          {#each [3, 2, 1] as n (n)}
            <button type="button" class="choix-volet" class:choisie={panes === n}
                    data-testid="accueil-volet" data-volets={n}
                    aria-pressed={panes === n}
                    onclick={() => applyPanes(n)}
                    onmouseenter={() => (panesHover = n)}
                    onmouseleave={() => (panesHover = null)}
                    onfocus={() => (panesHover = n)}
                    onblur={() => (panesHover = null)}>
              {t(`panes.${n}`)}</button>
          {/each}
        </div>
      </div>
    {:else if step === 3}
      <h3 class="titre display">{t('onboarding.themeTitle')}</h3>
      {@render progress(3)}
      <p class="sous">{t('onboarding.themeUnder')}</p>
      <div class="cartes themes" data-testid="accueil-themes">
        {#each THEME_CARDS as card (card.id)}
          {@const [accent, bg, border, surface] = card.swatches}
          <button type="button" class="carte" class:choisie={activeTheme === card.id}
                  data-testid="accueil-theme" data-theme-id={card.id}
                  aria-pressed={activeTheme === card.id}
                  onclick={() => chooseTheme(card.id)}>
            <!-- The window in THE card's colors (CARDS is measured
                 against system.css by the gate), in the chosen
                 layout (finding 6). -->
            {@render window(panes, { accent, bg, border, surface })}
            <span class="carte-nom">{t(`theme.${card.id}.name`)}</span>
          </button>
        {/each}
      </div>
    {:else if step === 4}
      <!-- The beta step (RETOURS-11 R3, field of 2026-08-28): say
           that Wind is in beta and show the header's Feedback
           button — the sample is INERT (aria-hidden on the
           drawing), the real button lives at the top right once the
           journey is done. -->
      <h3 class="titre display">{t('onboarding.betaTitle')}</h3>
      {@render progress(4)}
      <p class="sous">{t('onboarding.betaUnder')}</p>
      <div class="beta" data-testid="accueil-beta">
        <span class="echantillon" aria-hidden="true">
          <Icon name="feedback" />{t('header.feedback')}</span>
        <p class="beta-texte">{t('onboarding.betaButton')}</p>
      </div>
    {:else}
      <h3 class="titre display">{t('onboarding.endTitle')}</h3>
      {@render progress(5)}
      <p class="sous">{t('onboarding.endText')}</p>
      <!-- Finding 8 (1st pass): each recapped choice is a
           door back to its step; finding 4 (2nd pass): thumbnails
           for Layout and Theme, and on hover as on keyboard
           focus a VEIL covers the row and says "Back to this
           step" — the visual rules of the attachments' veil
           (A70). -->
      <div class="recap" data-testid="accueil-recap">
        <button type="button" class="ligne-recap" data-testid="recap-comptes"
                aria-label="{t('group.accounts')} : {t('onboarding.goBack')}"
                onclick={() => (step = 1)}>
          <span class="recap-titre">{t('group.accounts')}</span>
          <span class="recap-valeur">{accounts.map((c) => c.email).join(' · ')}</span>
          <span class="voile" aria-hidden="true">
            <Icon name="arrow_back" />{t('onboarding.goBack')}</span>
        </button>
        <button type="button" class="ligne-recap" data-testid="recap-volets"
                aria-label="{t('settings.panes')} : {t('onboarding.goBack')}"
                onclick={() => (step = 2)}>
          <span class="recap-titre">{t('settings.panes')}</span>
          <!-- 4th field pass: the text ABOVE the image. -->
          <span class="recap-valeur">{t(`panes.${panes}`)}</span>
          <img class="mini" src={PREVIEWS[panes]} alt="" />
          <span class="voile" aria-hidden="true">
            <Icon name="arrow_back" />{t('onboarding.goBack')}</span>
        </button>
        <button type="button" class="ligne-recap" data-testid="recap-theme"
                aria-label="{t('onboarding.theme')} : {t('onboarding.goBack')}"
                onclick={() => (step = 3)}>
          <span class="recap-titre">{t('onboarding.theme')}</span>
          <!-- 4th field pass: the text ABOVE the image. -->
          <span class="recap-valeur">{t(`theme.${activeTheme}.name`)}</span>
          {#if activeCard}
            {@const [accent, bg, border, surface] = activeCard.swatches}
            <span class="mini-theme">{@render window(panes, { accent, bg, border, surface })}</span>
          {/if}
          <span class="voile" aria-hidden="true">
            <Icon name="arrow_back" />{t('onboarding.goBack')}</span>
        </button>
      </div>
    {/if}
    {#if complete}
      <div class="marche">
        {#if step > 1}
          <button type="button" class="secondaire" data-testid="accueil-retour"
                  onclick={back}>{t('onboarding.back')}</button>
        {/if}
        {#if step < STEPS}
          <!-- 3rd pass, findings 1-2: "Continue" NEVER displays
               grayed out — absent as long as no account exists, and
               hidden while the generic desk is revealed
               ("Add" is the primary gesture there). -->
          {#if canContinue && !(step === 1 && genericOpen)}
            <button type="button" class="principal" data-testid="accueil-continuer"
                    onclick={proceed}>{t('onboarding.continue')}</button>
          {/if}
        {:else}
          <button type="button" class="principal" data-testid="accueil-terminer"
                  onclick={finish}>{t('onboarding.finish')}</button>
        {/if}
      </div>
    {/if}
  </div>
</div>

<style>
  /* Geometry of the prototype's screen 01 — the column widens at
     steps 2-4, the screen scrolls if the theme grid overflows. */
  .ecran01 {
    position:absolute; inset:0; display:flex; align-items:safe center;
    justify-content:center; background:var(--bg); z-index:1;
    overflow:auto; padding:32px 0;
  }
  .colonne { width:520px; display:flex; flex-direction:column; gap:22px; }
  .colonne.large { width:760px; }
  /* V6: step titles switch to the display register
     (weight 340, -.03em — global class .display, set on every
     h3); the size does not move. */
  .titre {
    margin:0; font-size:40px; line-height:1.1; color:var(--ink);
    display:flex; align-items:center; gap:14px;
  }
  /* The brand in the title: "Wind", non-breaking. */
  .marque { white-space:nowrap; }
  .progression {
    margin:0; font-size:13px; font-weight:600; color:var(--muted);
    font-variant-numeric:tabular-nums;
  }
  .sous { margin:0; font-size:15px; line-height:1.5; color:var(--ink2); }
  .ajoutes {
    margin:0; padding:0; list-style:none; display:flex;
    flex-direction:column; gap:6px;
  }
  .ajoutes li {
    display:flex; align-items:center; gap:8px; font-size:14px;
    color:var(--ink);
  }
  .ajoutes :global(.ic) { color:var(--accent); }

  .cartes { display:flex; gap:14px; flex-wrap:wrap; }
  /* V7/A94: the living table in two columns (2×2 for four cards —
     four abreast would crush the thumbnails); the 28 grid is
     dead. The screen scrolls if the grid overflows (.ecran01). */
  .cartes.themes {
    display:grid; grid-template-columns:repeat(2, 1fr); gap:14px;
  }
  .carte {
    display:flex; flex-direction:column; gap:8px; padding:10px;
    background:var(--surface); border:1px solid var(--border);
    border-radius:var(--r-surface); cursor:pointer; flex:1;
  }
  .carte:hover { background:var(--sel); }
  /* Findings 5/7: the selection is visible — thickened outline
     (inner trim, no grid shift) + the selection tint of the
     central pane's rows. */
  .carte.choisie {
    border-color:var(--accent);
    box-shadow:inset 0 0 0 1px var(--accent);
    background:var(--sel);
  }
  .carte-nom { font-size:13px; font-weight:600; color:var(--ink); }
  /* Finding 3 (2nd and 3rd passes): step 2's single preview and its
     row of buttons, in ONE elevation (surface + single shadow),
     buttons centered. */
  .cadre-volets {
    display:flex; flex-direction:column; gap:14px; padding:14px;
    background:var(--surface); border:1px solid var(--border);
    border-radius:var(--r-surface); box-shadow:var(--shadow);
  }
  .capture-grande {
    width:100%; height:auto; display:block; border-radius:var(--r-controle);
    border:1px solid var(--border);
  }
  .boutons-volets { display:flex; gap:12px; justify-content:center; }
  .choix-volet {
    height:40px; padding:0 18px; font-size:14px; font-weight:600;
    color:var(--ink); background:var(--surface);
    border:1px solid var(--border); border-radius:var(--r-controle); cursor:pointer;
  }
  .choix-volet:hover { background:var(--sel); }
  .choix-volet.choisie {
    border-color:var(--accent);
    box-shadow:inset 0 0 0 1px var(--accent);
    background:var(--sel);
  }

  /* Step 3's preview window, in the card's colors. */
  .fenetre {
    display:flex; flex-direction:column; height:110px; border-radius:var(--r-controle);
    border:1px solid var(--border); overflow:hidden;
  }
  .f-tete { height:14px; flex:none; border-bottom:1px solid var(--border); }
  .f-corps { display:flex; flex:1; min-height:0; }
  .f-nav { width:22%; flex:none; }
  .f-liste {
    flex:1; display:flex; flex-direction:column; gap:4px; padding:6px;
  }
  .f-liste span { height:10px; border-radius:var(--r-controle); }
  .f-liste span:nth-child(2) { opacity:.85; }
  .f-lecture { width:38%; flex:none; border-left:1px solid var(--border); }

  /* Finding 4 (3rd pass): the three recaps side by side, each in a
     column (title, thumbnail, value). */
  /* The beta step (RETOURS-11): the Feedback button sample at the
     header's exact drawing (border, control radius, 13 px),
     inert — the text explains, the drawing shows. */
  .beta {
    display:flex; align-items:center; gap:18px; text-align:left;
    padding:14px 0;
  }
  .echantillon {
    flex:none; display:inline-flex; align-items:center; gap:8px;
    height:32px; padding:0 14px; font-size:13px; color:var(--ink);
    background:var(--surface); border:1px solid var(--border);
    border-radius:var(--r-controle);
  }
  .beta-texte {
    margin:0; font-size:13px; line-height:1.5; color:var(--ink2);
  }

  .recap { display:flex; gap:12px; align-items:stretch; }
  .ligne-recap {
    position:relative; flex:1; min-width:0; display:flex;
    flex-direction:column; align-items:flex-start; gap:10px;
    text-align:left; padding:14px 16px; background:var(--surface);
    border:1px solid var(--border); border-radius:var(--r-surface); cursor:pointer;
  }
  .recap-titre { font-size:13px; font-weight:600; color:var(--ink); }
  .recap-valeur {
    font-size:13px; line-height:1.4; color:var(--ink2); min-width:0;
    overflow:hidden; overflow-wrap:anywhere;
  }
  /* Finding 4 (2nd pass): thumbnails — the layout's screenshot,
     the window in the theme's colors. */
  .mini {
    width:100%; height:auto; flex:none; display:block;
    border-radius:var(--r-controle); border:1px solid var(--border);
  }
  .mini-theme { width:100%; flex:none; display:block; }
  .mini-theme .fenetre { height:64px; border-radius:var(--r-controle); }
  .mini-theme .f-tete { height:8px; }
  .mini-theme .f-liste { padding:3px; gap:2px; }
  .mini-theme .f-liste span { height:5px; border-radius:var(--r-controle); }
  /* The "Back to this step" veil: the attachments' veil rules
     (A70) — absolute overlay, opaque --sel background,
     shown on hover AND on keyboard focus (A8), stable geometry. */
  .ligne-recap .voile {
    position:absolute; inset:0; display:none; align-items:center;
    justify-content:center; gap:6px; font-size:13px; font-weight:600;
    color:var(--ink); background:var(--sel); border-radius:var(--r-surface);
    white-space:nowrap; overflow:hidden;
  }
  .ligne-recap:hover .voile, .ligne-recap:focus-visible .voile {
    display:inline-flex;
  }

  .marche { display:flex; align-items:center; gap:12px; }
  .principal {
    height:40px; padding:0 22px; font-size:14px; font-weight:600;
    color:var(--onAccent); background:var(--accent); border:none;
    border-radius:var(--r-controle); cursor:pointer;
  }
  .principal:hover { background:var(--accentH); }
  .principal:disabled { opacity:.5; cursor:default; }
  .secondaire {
    height:40px; padding:0 18px; font-size:14px; color:var(--ink);
    background:var(--surface); border:1px solid var(--border);
    border-radius:var(--r-controle); cursor:pointer; align-self:flex-start;
  }
  .secondaire:hover { background:var(--sel); }
</style>

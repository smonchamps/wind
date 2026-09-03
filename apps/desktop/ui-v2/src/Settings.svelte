<script>
  // Settings overlay in two panes (A13): on the left the rail of
  // GROUPS (the nav grammar of screen 02 — 36 px rows, active state
  // = surface + accent border + shadow), on the right the content of
  // the chosen group. Signature card widened to 800 px, 48 px header
  // and "Done" footer unchanged. The prototype is silent on this
  // surface: the System fills it in (A6), the gap is logged.
  //
  // Rule: a group ships only with REAL content — no invented setting
  // to pad it out, no empty group.
  import Icon from './Icon.svelte';
  import Menu from './Menu.svelte';
  import Brand from './Brand.svelte';
  import EUFlag from './EUFlag.svelte';
  import { tick } from 'svelte';
  import {
    THEME_CARDS, applyTheme, displayedTheme, osTracking, applyOsTracking,
  } from './lib/theme.js';
  import { t, LANGUAGES, currentLanguage, applyLanguage } from './lib/text.svelte.js';
  import { currentPanes, applyPanes } from './lib/panes.svelte.js';
  import {
    currentSpacing, applySpacing, LEVELS,
  } from './lib/spacing.svelte.js';
  import { activation } from './lib/keyboard.js';
  import { call } from './lib/transport.js';
  import { SCREENED_OUT_LABEL, DESTINATION_LABEL } from './lib/screener.js';
  import { MARKER_ICONS, MARKER_HUES } from './lib/markers.js';
  import { IMPORT_HORIZONS as HORIZONS } from './lib/vocabularies.js';
  import AccountDesk from './AccountDesk.svelte';

  // A11 — the "Accounts" section: v1 offered adding at any time,
  // screen 01 only appears at zero accounts; the permanent gate lives
  // here. Removal lives on the same row: `onsupprime(id)` bubbles up
  // to the App, which reloads nav and list.
  let {
    accounts = [],
    // The addresses holding a session (App.connecter): an account in
    // the registry absent from here has a dead token — it repairs on
    // the spot.
    connected = [],
    // R1 (PLAN-RETOURS-8): the markers set (App loads them); setting
    // or removing bubbles up via `onrepere(id, repere|null)` — the App
    // patches its table on the spot (review: never a full reload on a
    // hue click).
    markers = {},
    onmarker = () => {},
    // PLAN-RETOURS-9 (D3/D4): the custom names (App loads them);
    // setting or clearing bubbles up via `onnom(id, nom|null)` — same
    // regime as the marker.
    names = {},
    onname = () => {},
    onadd = () => {},
    onremove = () => {},
    onreconnect = () => {},
    // RETOURS-14 R5 (review): reinstatement speaks and propagates —
    // the same-gesture contract as the Screener page (toast + views
    // re-served by the App).
    onflash = () => {},
    onrouting = () => {},
  } = $props();

  const GROUPS = [
    { id: 'comptes', icon: 'person', label: 'group.accounts' },
    { id: 'themes', icon: 'bookmark', label: 'group.themes' },
    { id: 'affichage', icon: 'display_settings', label: 'group.display' },
    { id: 'notifications', icon: 'notifications', label: 'group.notifications' },
    // RETOURS-13 R9, field C4: the Screener defaults — the group
    // stays visible REGARDLESS OF MODE (CE field verdict, which
    // overturns the "organized only" choice of the first pass).
    { id: 'screener', icon: 'screener', label: 'group.screener' },
    // R1 (PLAN-RETOURS-6): the signature manager — real content
    // (one editor per account), the group rule is kept.
    { id: 'signature', icon: 'signature', label: 'group.signature' },
    { id: 'raccourcis', icon: 'keyboard', label: 'group.shortcuts' },
    { id: 'apropos', icon: 'info', label: 'group.about' },
  ];

  // The D3 table, for REFERENCE only — no re-mapping. Keys and
  // gestures from the catalogue (`raccourci.touche.*` /
  // `raccourci.geste.*`): "Suppr" / "Échap" become "Del" / "Esc",
  // only the GESTURES translate — keys c/r/f/e don't change from one
  // language to another (A15).
  const SHORTCUTS = ['c', 'r', 'f', 'e', 'delete', 'slash', 'escape'];

  let visible = $state(false);
  let panel = $state(null);
  let group = $state('comptes');
  // The checkmark follows the DISPLAYED card, not the persisted
  // choice (review A42): under OS tracking + dark OS, the screen is
  // in -night — the checkmark too, otherwise the user "corrects" by
  // clicking the -night card and locks themselves into permanent
  // dark. The `wind:theme-affiche` signal keeps it aligned, including
  // when the OS switches while the dialog is open.
  let active = $state(displayedTheme());
  let addOpen = $state(false);
  $effect(() => {
    if (!visible) return;
    const track = () => (active = displayedTheme());
    document.addEventListener('wind:theme-affiche', track);
    return () => document.removeEventListener('wind:theme-affiche', track);
  });

  // Removing an account: the gesture is DESTRUCTIVE locally (local
  // mail erased, connection forgotten — the server itself is never
  // touched), so it confirms on the spot, in a card under the row.
  // `removalTarget` carries the account_id awaiting confirmation.
  let removalTarget = $state(null);
  let removalBusy = $state(false);
  let removalError = $state(null);

  // About: the version is read ONCE (it doesn't change during a
  // session); outside Tauri the rejection leaves the dash — never a
  // silent emptiness that would look like an oversight.
  let version = $state('');
  // null (idle) | 'controle' | 'ajour' | {version} | {erreur}
  let update = $state(null);

  // Display (D6): OS dark tracking, a localStorage boolean like the
  // theme. Notifications (R-D2): incoming bubbles, a preference IN
  // THE DATABASE — the Rust shell is the one that emits them.
  // Language (A15): in the database too, same reason — the shell
  // composes the bubbles in that language.
  let auto = $state(osTracking());
  let bubbles = $state(true);
  let language = $state(currentLanguage());
  // Layout (PLAN-VOLETS, V-D4): the number of panes, a localStorage
  // value like the theme — applied immediately, the theme's gesture;
  // nothing that can fail, so nothing to roll back.
  let panes = $state(currentPanes());
  let spacing = $state(currentSpacing());
  // R1 (RETOURS-11, D4): the "always show images from this sender"
  // rules — read from the core on every open, removed on the spot.
  // The exit gate for "always".
  let imageSenders = $state([]);
  // RETOURS-13 R9: the Screener buttons' defaults — read from the
  // core when the group opens. `null` until the database has
  // answered: the selectors only paint with the PERSISTED state
  // (review — a click before the response would have overwritten the
  // other default with the delivered value, not its own). On a write
  // failure, the interface does not lie: it reverts to the state
  // actually persisted.
  let screenerDefaults = $state(null);
  // RETOURS-14 R5 (D6): the EXHAUSTIVE list of Screener decisions —
  // all destinations (the Screener page's history only shows the
  // ones screened out), alphabetized, filterable, reinstatable.
  // `null` until the database has answered: emptiness is never
  // asserted without proof.
  let routingsList = $state(null);
  let routingsFilter = $state('');
  $effect(() => {
    if (visible && group === 'screener') {
      call('screener_defaults_get')
        .then((d) => (screenerDefaults = d))
        .catch((err) => console.error('screener_defaults_get :', err));
      routingsFilter = '';
      call('routings')
        .then((r) => (routingsList = r))
        .catch((err) => console.error('routings :', err));
    }
  });
  const visibleRoutings = $derived.by(() => {
    if (!routingsList) return null;
    const filter = routingsFilter.trim().toLowerCase();
    return routingsList
      .filter((r) => !filter || r.address.toLowerCase().includes(filter))
      .slice()
      .sort((a, b) => a.address.localeCompare(b.address, currentLanguage(), { sensitivity: 'base' }));
  });
  // The verdicts' vocabulary: ONE copy (lib/screener.js, shared with
  // the Screener page), never copied-out text.
  const routingLabel = (r) =>
    r.destination === 'screened_out'
      ? t(SCREENED_OUT_LABEL[r.rule] ?? 'screener.screenedOut')
      : t(DESTINATION_LABEL[r.destination] ?? r.destination);
  // RETOURS-14 R10 (field 2026-08-31): "Reinstate" becomes "Edit" —
  // the menu re-offers ALL of the Screener's rules (the Yeses, the
  // No rules) plus "Send back to Screener" (the former Reinstate).
  // Same contract as the Screener page: the toast says what just
  // happened, `onrouting` has the App re-serve views and nav, the
  // failure is SAID (never a silence).
  let decisionMenu = $state(null);
  function openEdit(e, r) {
    e.stopPropagation();
    const rect = e.currentTarget.getBoundingClientRect();
    decisionMenu = {
      address: r.address,
      x: rect.left,
      y: rect.bottom + 4,
    };
  }
  const TOAST_NO = {
    spam: 'toast.screenerNoSpam',
    archive: 'toast.screenerNoArchive',
    trash: 'toast.screenerNoTrash',
  };
  const MAILBOX_OF = {
    inbox: 'screener.theInbox',
    feed: 'screener.theFeed',
    paper_trail: 'screener.thePaperTrail',
  };
  async function editRouting(destination, rule = null) {
    const { address } = decisionMenu;
    decisionMenu = null;
    try {
      await call('route_sender', { address, destination, rule });
      routingsList = routingsList.map((r) =>
        r.address === address ? { ...r, destination, rule } : r);
      if (destination === 'screened_out') {
        onflash(t(rule ? TOAST_NO[rule] : 'toast.screenerNoBare', { who: address }));
      } else if (destination === 'inbox') {
        onflash(t('toast.screenerYesBare', { who: address }));
      } else {
        onflash(t('toast.screenerYesTo', { who: address, mailbox: t(MAILBOX_OF[destination]) }));
      }
      onrouting();
    } catch (err) {
      onflash(t('error.preference', { err }));
    }
  }
  async function sendBackToScreener() {
    const { address } = decisionMenu;
    decisionMenu = null;
    try {
      await call('remove_routing', { address });
      routingsList = routingsList.filter((r) => r.address !== address);
      onflash(t('toast.screenerReinstated', { who: address }));
      onrouting();
    } catch (err) {
      onflash(t('error.preference', { err }));
    }
  }
  function changeScreener(field, value) {
    if (!screenerDefaults) return;
    const before = { ...screenerDefaults };
    screenerDefaults = { ...screenerDefaults, [field]: value };
    call('screener_defaults_set', {
      yes: screenerDefaults.yes,
      no: screenerDefaults.no,
    }).catch(() => {
      screenerDefaults = before;
    });
  }

  // ADR 0029 (D3): the import horizon per account — read from the
  // core AT OPENING TIME (review 2026-08-30: an $effect on `accounts`
  // was re-firing every 10 s at chargerNav's pace, and a late read
  // could overwrite an optimistic choice in flight); the selector
  // only paints with the PERSISTED state (screenerDefaults pattern).
  // On a write failure, revert to the state actually persisted — the
  // interface does not lie.
  let horizons = $state({});
  let horizonOpen = $state(null);
  let horizonError = $state(null);
  function loadHorizons() {
    for (const c of accounts) {
      call('horizon_import_get', { accountId: c.account_id })
        .then((v) => (horizons[c.account_id] = v))
        .catch((err) => console.error('horizon_import_get :', err));
    }
  }
  function openHorizon(id) {
    const reopening = horizonOpen !== id;
    closeCards();
    if (reopening) horizonOpen = id;
  }
  function changeHorizon(id, value) {
    const before = horizons[id];
    horizons[id] = value;
    horizonError = null;
    call('horizon_import_set', { accountId: id, value }).catch((err) => {
      horizons[id] = before;
      horizonError = t('settings.horizonFailed', { err });
    });
  }

  // "Never two cards under the same row" (review 2026-08-22): THE
  // single point — the next card gets added here, not in N places
  // (review 2026-08-23: the invariant lived copied in five spots).
  function closeCards() {
    removalTarget = null;
    removalError = null;
    markerOpen = null;
    markerError = null;
    nameOpen = null;
    nameError = null;
    horizonOpen = null;
    horizonError = null;
  }

  export function open() {
    active = displayedTheme();
    auto = osTracking();
    language = currentLanguage();
    panes = currentPanes();
    spacing = currentSpacing();
    addOpen = false;
    closeCards();
    // Reset to zero BEFORE the reload (imageSenders pattern): the
    // gates paint from the DATABASE, never from the previous opening's
    // memory — a choice that didn't persist must not appear persisted
    // (net proven vacant without this line).
    horizons = {};
    loadHorizons();
    reconnection = null;
    reconnectionError = null;
    group = 'comptes';
    update = null;
    visible = true;
    if (!version) {
      call('app_version')
        .then((v) => (version = v))
        .catch(() => (version = '—'));
    }
    call('notif_pref_get')
      .then((v) => (bubbles = v))
      .catch(() => { /* outside Tauri: the default (enabled) stays shown */ });
    // Reset to zero BEFORE the reload: on failure, showing the
    // previous opening's list would be a lie (a rule revoked
    // elsewhere would still look alive); and the failure is said
    // (§9 — never swallowed).
    imageSenders = [];
    call('images_senders')
      .then((list) => (imageSenders = list))
      .catch((err) => console.error('images_senders :', err));
    // PLAN-AUDIT-V2 E11 (D-4 entry): focus comes in WITH the panel —
    // the rail's first control, like `Feedback.svelte`.
    queueMicrotask(() => panel?.querySelector('button, input, select, [tabindex]')?.focus());
  }

  async function removeImageSender(address) {
    try {
      await call('revoke_images_sender', { address: address });
      imageSenders = imageSenders.filter((a) => a !== address);
    } catch (err) {
      console.error('revoke_images_sender :', err);
    }
  }
  export function close() {
    visible = false;
  }
  export function isOpen() {
    return visible;
  }
  function chooseGroup(id) {
    group = id;
    addOpen = false;
    closeCards();
  }
  function requestRemoval(id) {
    const reopening = removalTarget !== id;
    closeCards();
    if (reopening) removalTarget = id;
  }
  // R1 — the marker: the choice card opens under the row (the
  // removal pattern). A marker only exists WHOLE (icon + hue, the
  // Rust allowlist is authoritative): the first choice waits for its
  // twin, then every click applies immediately — the theme's gesture.
  let markerOpen = $state(null);
  let markerChoice = $state({ icon: null, hue: null });
  let markerError = $state(null);
  function openMarker(id) {
    const reopening = markerOpen !== id;
    closeCards();
    if (!reopening) return;
    markerOpen = id;
    const r = markers[id];
    markerChoice = { icon: r?.icon ?? null, hue: r?.hue ?? null };
  }
  async function chooseMarker(id, field, value) {
    markerChoice = { ...markerChoice, [field]: value };
    if (!markerChoice.icon || !markerChoice.hue) return;
    markerError = null;
    try {
      await call('marker_set', {
        accountId: id,
        icon: markerChoice.icon,
        hue: markerChoice.hue,
      });
      onmarker(id, { icon: markerChoice.icon, hue: markerChoice.hue });
    } catch (err) {
      // The database didn't take the choice: the error is said on the
      // spot and the gesture replays — the row's swatch, though, does
      // not lie (it follows `markers`, the state actually persisted).
      markerError = t('settings.markerFailed', { err });
    }
  }
  async function removeMarker(id) {
    markerError = null;
    try {
      await call('marker_set', { accountId: id, icon: null, hue: null });
      markerChoice = { icon: null, hue: null };
      onmarker(id, null);
    } catch (err) {
      markerError = t('settings.markerFailed', { err });
    }
  }
  // PLAN-RETOURS-9 (D3): the custom name — the card opens via the
  // row's LABEL (identity is the gate to its name; no new glyph: the
  // set has no pencil, A3 forbids reusing one). Clearing the field
  // removes the name; the shell normalizes and is authoritative.
  let nameOpen = $state(null);
  let draftName = $state('');
  let nameError = $state(null);
  let nameTaken = $state(false);
  function openName(id) {
    const reopening = nameOpen !== id;
    closeCards();
    if (!reopening) return;
    nameOpen = id;
    draftName = names[id] ?? '';
  }
  async function saveName(id) {
    // Only one flight at a time: Enter and the button go through the
    // same gate (review 2026-08-23 — the button's disabled state
    // wasn't guarding the keyboard path).
    if (nameTaken) return;
    nameTaken = true;
    nameError = null;
    try {
      const name = await call('name_set', { accountId: id, name: draftName });
      onname(id, name ?? null);
      // Close ONLY its own card: a late response must never slam shut
      // the one another account just opened.
      if (nameOpen === id) nameOpen = null;
    } catch (err) {
      // The database didn't take the name: the error is said on the
      // spot and the gesture replays — the row's label follows
      // `names`, the state actually persisted.
      nameError = t('settings.nameFailed', { err });
    } finally {
      nameTaken = false;
    }
  }
  // Reconnecting an account with a dead token (field finding
  // 2026-08-20): the browser consent replays from the row — the
  // failure is said ON THE SPOT and the gesture replays, like removal.
  const isDisconnected = (c) => !connected.includes(c.email);
  let reconnection = $state(null);
  let reconnectionError = $state(null);
  async function reconnect(c) {
    reconnection = c.account_id;
    reconnectionError = null;
    try {
      await call('reconnect_account', { accountId: c.account_id });
      onreconnect();
    } catch (err) {
      reconnectionError = { id: c.account_id, text: t('settings.reconnectionFailed', { err }) };
    } finally {
      reconnection = null;
    }
  }
  async function confirmRemoval() {
    const id = removalTarget;
    removalBusy = true;
    removalError = null;
    try {
      await call('remove_account', { accountId: id });
      removalTarget = null;
      onremove(id);
    } catch (err) {
      // The account is still listed: the error is said on the spot
      // and the gesture replays — removal is repeatable on the shell
      // side.
      removalError = t('settings.removalFailed', { err });
    } finally {
      removalBusy = false;
    }
  }
  function choose(id) {
    applyTheme(id);
    // Never `active = id`: applyTheme silently refuses an unknown id,
    // and under OS tracking the applied theme can be `id-night` —
    // the displayed card is authoritative in both cases.
    active = displayedTheme();
  }
  function toggleAuto() {
    auto = !auto;
    applyOsTracking(auto);
  }
  function toggleBubbles() {
    bubbles = !bubbles;
    const wanted = bubbles;
    call('notif_pref_set', { enabled: wanted }).catch(() => {
      // The database didn't take the choice: the switch must not
      // lie — it reverts to the state actually persisted.
      if (bubbles === wanted) bubbles = !wanted;
    });
  }
  function changePanes(n) {
    applyPanes(n);
    panes = currentPanes();
  }
  function changeSpacing(level) {
    applySpacing(level);
    spacing = currentSpacing();
  }
  function changeLanguage(code) {
    const before = currentLanguage();
    if (code === before) return;
    // Immediate application (the theme's gesture), persistence in the
    // database; if the database didn't take the choice, the interface
    // does not lie — it reverts to the language actually persisted.
    applyLanguage(code);
    language = code;
    call('lang_set', { lang: code }).catch(() => {
      applyLanguage(before);
      language = before;
    });
  }

  // R1 (PLAN-RETOURS-6, D3/D4): the signature per account. A REDUCED
  // rich editor (bold/italic/underline — the vocabulary crosses the
  // same ammonia boundary as the composer, at save time on the Rust
  // side) and the SCOPE (also in replies/forwards?) — a choice per
  // account, applicable to all in one gesture (D4, word for word).
  let signatures = $state({}); // account_id -> { replies, state }
  let signatureFields = {}; // account_id -> contenteditable (outside reactivity)
  async function loadSignatures() {
    for (const c of accounts) {
      try {
        const loaded = await call('signature_get', { accountId: c.account_id });
        signatures[c.account_id] = { replies: loaded.replies, state: null };
        // The node only exists once the group is rendered — setting
        // it before would be lost (same reason as `setBody` at the
        // composer).
        await tick();
        const field = signatureFields[c.account_id];
        if (field) field.innerHTML = loaded.html ?? '';
      } catch (err) {
        console.error('signature_get :', err);
      }
    }
  }
  $effect(() => {
    if (visible && group === 'signature') loadSignatures();
  });
  // `styleWithCSS` off, like at the composer: the output stays the
  // allowlist's exact vocabulary (b/i/u), never generated style.
  function signatureCommand(name) {
    document.execCommand('styleWithCSS', false, false);
    document.execCommand(name, false, null);
  }
  async function saveSignature(c, { replies = null, state = 'ok' } = {}) {
    const sig = signatures[c.account_id] ?? { replies: false };
    const wanted = replies ?? sig.replies;
    try {
      await call('signature_set', {
        accountId: c.account_id,
        html: signatureFields[c.account_id]?.innerHTML ?? '',
        replies: wanted,
      });
      signatures[c.account_id] = { replies: wanted, state };
      return true;
    } catch (err) {
      signatures[c.account_id] = {
        ...sig,
        state: { error: t('error.signature', { err }) },
      };
      return false;
    }
  }
  function clearSignature(c) {
    const field = signatureFields[c.account_id];
    if (field) field.innerHTML = '';
    saveSignature(c);
  }
  // The toggle SAVES (the choice applies right away, like the other
  // Settings switches) — and carries the editor's current text: a
  // single write path.
  function toggleReplies(c) {
    const sig = signatures[c.account_id] ?? { replies: false };
    saveSignature(c, { replies: !sig.replies });
  }
  // D4, clarified on the field (2026-08-21): "apply to all accounts"
  // copies this account's SIGNATURE AND SCOPE onto all the others —
  // and it SHOWS: their editors and their switches update on screen,
  // not just in the database.
  async function applyToAll(c) {
    const html = signatureFields[c.account_id]?.innerHTML ?? '';
    const wanted = signatures[c.account_id]?.replies ?? false;
    await saveSignature(c, { replies: wanted, state: 'tous' });
    for (const other of accounts) {
      if (other.account_id === c.account_id) continue;
      try {
        await call('signature_set', {
          accountId: other.account_id,
          html,
          replies: wanted,
        });
        const field = signatureFields[other.account_id];
        if (field) field.innerHTML = html;
        signatures[other.account_id] = { replies: wanted, state: null };
      } catch (err) {
        signatures[other.account_id] = {
          ...(signatures[other.account_id] ?? { replies: false }),
          state: { error: t('error.signature', { err }) },
        };
      }
    }
  }

  // The same flow as the notification slot (ADR 0013): update_check
  // silently, update_install doesn't return control on success.
  async function checkUpdate() {
    update = 'controle';
    try {
      const info = await call('update_check');
      update = info ? { version: info.version } : 'ajour';
    } catch (err) {
      update = { error: String(err) };
    }
  }
  async function installUpdate() {
    const version = update.version;
    update = 'installation';
    try {
      // Doesn't return control on success; the version goes along —
      // only what was announced gets installed.
      await call('update_install', { version });
    } catch (err) {
      // The launch failed: the update stays available — it's
      // re-offered with the error stated, never a dead end (review
      // PLAN-SIGNATURE). { error } alone remains the CHECK's failure.
      update = { version, error: String(err) };
    }
  }
</script>

{#if visible}
  <div class="scrim" data-testid="reglages-modal" bind:this={panel}>
    <div class="carte" role="dialog" aria-modal="true" aria-label={t('header.settings')}>
      <div class="tete">
        <span class="titre">{t('header.settings')}</span>
        <button type="button" class="fermer" aria-label={t('action.close')} onclick={close}>
          <Icon name="close" /></button>
      </div>
      <div class="milieu">
        <div class="rail" role="group" aria-label={t('settings.groupsAria')}>
          {#each GROUPS as g (g.id)}
            <div class="rang" class:actif={group === g.id}
                 data-testid="reglages-groupe" data-groupe={g.id}
                 role="button" tabindex="0" aria-current={group === g.id}
                 onclick={() => chooseGroup(g.id)}
                 onkeydown={activation(() => chooseGroup(g.id))}>
              <span class="icone" aria-hidden="true"><Icon name={g.icon} /></span>
              <span class="libelle">{t(g.label)}</span>
            </div>
          {/each}
        </div>
        <div class="volet" data-testid="reglages-volet">
          {#if group === 'comptes'}
            <p class="section">{t('group.accounts')}</p>
            <div class="rangees" data-testid="reglages-comptes">
              {#each accounts as c (c.account_id)}
                <div class="compte">
                  <!-- A74: the row's icon becomes THE gate to the
                       marker — it shows the persisted state (swatch or
                       neutral `person`) and opens the choice card. -->
                  <button type="button" class="btn-repere" data-testid="compte-repere"
                          aria-expanded={markerOpen === c.account_id}
                          aria-label={t('settings.markerAccount', { email: c.email })}
                          onclick={() => openMarker(c.account_id)}>
                    {#if markers[c.account_id]}
                      <span class="repere p20"
                            data-teinte={markers[c.account_id].hue}
                            aria-hidden="true"><Icon name={markers[c.account_id].icon} /></span>
                    {:else}
                      <Icon name="person" />
                    {/if}
                  </button>
                  <!-- PLAN-RETOURS-9 (D3/D4): the label is the GATE to
                       the custom name — in Settings the name displays
                       WITH the address (it stays the truth of the
                       connection). -->
                  <button type="button" class="identite" data-testid="compte-nommer"
                          aria-expanded={nameOpen === c.account_id}
                          aria-label={t('settings.renameAccount', { email: c.email })}
                          onclick={() => openName(c.account_id)}>
                    {#if names[c.account_id]}
                      <span class="nom-compte" data-testid="compte-nom">{names[c.account_id]}</span>
                    {/if}
                    <span class="adresse" class:sous-nom={names[c.account_id]}>{c.email}</span>
                  </button>
                  <!-- ADR 0029 (D3): the gate to the import horizon —
                       the VALUE is the gate (no new glyph, A3), the
                       card opens under the row. -->
                  <button type="button" class="btn-horizon" data-testid="compte-horizon"
                          aria-expanded={horizonOpen === c.account_id}
                          aria-label={t('settings.horizonAccount', { email: c.email })}
                          onclick={() => openHorizon(c.account_id)}>
                    {horizons[c.account_id] ? t(`horizon.${horizons[c.account_id]}`) : '…'}</button>
                  {#if isDisconnected(c)}
                    <!-- Dead token: the state is SAID (link_off, the
                         reconnection glyph — same meaning as at the
                         notification slot) and repairs on the spot. -->
                    <span class="deconnecte" data-testid="compte-deconnecte">
                      <Icon name="link_off" />{t('settings.disconnected')}</span>
                    <button type="button" class="reconnecter" data-testid="compte-reconnecter"
                            disabled={reconnection === c.account_id}
                            aria-label={t('settings.reconnectAccount', { email: c.email })}
                            onclick={() => reconnect(c)}>
                      {reconnection === c.account_id
                        ? t('settings.reconnectionInProgress')
                        : t('settings.reconnect')}</button>
                  {/if}
                  <!-- PLAN-RETOURS-9 (D2): the gesture is SAID — icon +
                       text, in the product's vocabulary ("remove",
                       nothing is deleted from the server). -->
                  <button type="button" class="retirer" data-testid="compte-retirer"
                          aria-label={t('settings.removeAccount', { email: c.email })}
                          onclick={() => requestRemoval(c.account_id)}>
                    <Icon name="delete" />{t('settings.remove')}</button>
                </div>
                {#if reconnectionError?.id === c.account_id}
                  <p class="erreur-reconnexion" data-testid="reconnexion-erreur">
                    {reconnectionError.text}</p>
                {/if}
                {#if markerOpen === c.account_id}
                  <!-- A74: the marker card, under the row (the removal
                       pattern). Icons then hues; the first choice waits
                       for its twin, then every click applies
                       immediately (the theme's gesture). -->
                  <div class="carte-repere" data-testid="reglages-repere">
                    <p class="titre-repere">{t('settings.markerTitle')}</p>
                    <div class="choix-repere" role="group" aria-label={t('settings.markerIcons')}>
                      {#each MARKER_ICONS as ic (ic)}
                        <button type="button" class="choix" class:choisi={markerChoice.icon === ic}
                                data-testid="repere-icone" data-icone={ic}
                                aria-pressed={markerChoice.icon === ic}
                                title={t(`marker.icon.${ic}`)}
                                aria-label={t(`marker.icon.${ic}`)}
                                onclick={() => chooseMarker(c.account_id, 'icon', ic)}>
                          <Icon name={ic} /></button>
                      {/each}
                    </div>
                    <div class="choix-repere" role="group" aria-label={t('settings.markerHues')}>
                      {#each MARKER_HUES as te (te)}
                        <button type="button" class="choix" class:choisi={markerChoice.hue === te}
                                data-testid="repere-teinte" data-couleur={te}
                                aria-pressed={markerChoice.hue === te}
                                title={t(`marker.hue.${te}`)}
                                aria-label={t(`marker.hue.${te}`)}
                                onclick={() => chooseMarker(c.account_id, 'hue', te)}>
                          <span class="repere pastille-teinte" data-teinte={te}
                                aria-hidden="true"></span></button>
                      {/each}
                    </div>
                    {#if markerError}
                      <p class="erreur-repere" data-testid="repere-erreur">{markerError}</p>
                    {/if}
                    {#if markers[c.account_id]}
                      <button type="button" class="ajouter" data-testid="repere-retirer"
                              onclick={() => removeMarker(c.account_id)}>
                        {t('settings.markerRemove')}</button>
                    {/if}
                  </div>
                {/if}
                {#if nameOpen === c.account_id}
                  <!-- The name card, under the row (the removal
                       pattern). Clearing the field removes the name;
                       Enter saves. -->
                  <div class="carte-nom" data-testid="reglages-nom">
                    <p class="titre-repere">{t('settings.nameTitle')}</p>
                    <!-- No maxlength: "never silently truncated"
                         (D3 contract) — a name that's too long gets
                         REFUSED with its error, by the shell. -->
                    <input type="text" class="champ-nom"
                           data-testid="nom-champ" bind:value={draftName}
                           placeholder={c.email}
                           aria-label={t('settings.nameTitle')}
                           onkeydown={(e) => { if (e.key === 'Enter') saveName(c.account_id); }}>
                    {#if nameError}
                      <p class="erreur-repere" data-testid="nom-erreur">{nameError}</p>
                    {/if}
                    <div class="boutons-retrait">
                      <button type="button" class="ajouter" data-testid="nom-enregistrer"
                              disabled={nameTaken} onclick={() => saveName(c.account_id)}>
                        {t('action.save')}</button>
                      <button type="button" class="ajouter" data-testid="nom-annuler"
                              onclick={() => (nameOpen = null)}>
                        {t('action.cancel')}</button>
                    </div>
                  </div>
                {/if}
                {#if horizonOpen === c.account_id}
                  <!-- The horizon card, under the row (the name
                       pattern). Immediate application — the theme's
                       gesture; the note says what expanding and
                       reducing DO. -->
                  <div class="carte-nom" data-testid="reglages-horizon">
                    <p class="titre-repere">{t('settings.horizonTitle')}</p>
                    {#if horizons[c.account_id]}
                      <select class="select-horizon" data-testid="horizon-select"
                              value={horizons[c.account_id]}
                              onchange={(e) => changeHorizon(c.account_id, e.currentTarget.value)}>
                        {#each HORIZONS as h (h)}
                          <option value={h}>{t(`horizon.${h}`)}</option>
                        {/each}
                      </select>
                    {/if}
                    <p class="note-horizon">{t('settings.horizonNote')}</p>
                    {#if horizonError}
                      <p class="erreur-repere" data-testid="horizon-erreur">{horizonError}</p>
                    {/if}
                  </div>
                {/if}
                {#if removalTarget === c.account_id}
                  <!-- The confirmation lives UNDER the row, in the
                       signature card: a destructive gesture never fires
                       on the first click, and it says what it erases —
                       and what it doesn't erase (the server). -->
                  <div class="carte-retrait" data-testid="reglages-retrait">
                    <p class="avertissement">{t('settings.removeConfirm', { email: c.email })}</p>
                    {#if removalError}
                      <p class="erreur-retrait" data-testid="retrait-erreur">{removalError}</p>
                    {/if}
                    <div class="boutons-retrait">
                      <button type="button" class="danger" data-testid="retrait-confirmer"
                              disabled={removalBusy} onclick={confirmRemoval}>
                        {removalBusy ? t('settings.removalInProgress') : t('action.remove')}</button>
                      <button type="button" class="ajouter" data-testid="retrait-annuler"
                              onclick={() => requestRemoval(c.account_id)}>
                        {t('action.cancel')}</button>
                    </div>
                  </div>
                {/if}
              {/each}
              {#if addOpen}
                <!-- Signature card: the counter is a deliberate BLOCK,
                     not a floating form (field verdict). Torn down on
                     collapse or on success: it always starts clean
                     again. -->
                <div class="carte-ajout" data-testid="reglages-guichet">
                  <div class="tete-ajout">
                    <span class="titre-ajout">{t('settings.addAccount')}</span>
                    <button type="button" class="fermer" aria-label={t('action.collapse')}
                            onclick={() => (addOpen = false)}>
                      <Icon name="close" /></button>
                  </div>
                  <AccountDesk compact onadd={() => { addOpen = false; onadd(); }} />
                </div>
              {:else}
                <button type="button" class="ajouter" data-testid="reglages-ajouter"
                        onclick={() => (addOpen = true)}>
                  <Icon name="person_add" />{t('settings.addAccount')}</button>
              {/if}
            </div>
          {:else if group === 'themes'}
            <p class="section">{t('settings.sectionThemes')}</p>
            <div class="rangees">
              <!-- R1 (PLAN-RETOURS-13): OS dark tracking lives at the
                   HEAD of Themes — it governs the displayed theme, not
                   the display group. The historical testid stays (two
                   specs and the docs carry it). -->
              <div class="reglage">
                <span class="libelles">
                  <span class="nom">{t('settings.darkAuto')}</span>
                  <span class="desc">{t('settings.darkAutoDesc')}</span>
                </span>
                <button type="button" class="bascule" role="switch"
                        aria-checked={auto} aria-label={t('settings.darkAuto')}
                        data-testid="affichage-auto" onclick={toggleAuto}>
                  <span class="bille"></span>
                </button>
              </div>
              {#each THEME_CARDS as card (card.id)}
                <div class="rangee" class:active={active === card.id}
                     data-testid="theme" data-theme-id={card.id}
                     role="button" tabindex="0" aria-pressed={active === card.id}
                     onclick={() => choose(card.id)}
                     onkeydown={activation(() => choose(card.id))}>
                  <span class="pastilles">
                    {#each card.swatches as color (color)}
                      <span class="pastille" style="background:{color}"></span>
                    {/each}
                  </span>
                  <span class="libelles">
                    <span class="nom">{t(`theme.${card.id}.name`)}</span>
                    <span class="desc">{t(`theme.${card.id}.desc`)}</span>
                  </span>
                  {#if active === card.id}
                    <span class="coche" aria-hidden="true"><Icon name="check_circle" /></span>
                  {/if}
                </div>
              {/each}
            </div>
          {:else if group === 'affichage'}
            <p class="section">{t('group.display')}</p>
            <div class="rangees" data-testid="reglages-affichage">
              <div class="reglage">
                <span class="libelles">
                  <span class="nom">{t('settings.language')}</span>
                  <span class="desc">{t('settings.languageDesc')}</span>
                </span>
                <select class="langue" data-testid="affichage-langue"
                        aria-label={t('settings.language')} value={language}
                        onchange={(e) => changeLanguage(e.target.value)}>
                  {#each LANGUAGES as code (code)}
                    <option value={code}>{t(`language.${code}`)}</option>
                  {/each}
                </select>
              </div>
              <div class="reglage">
                <span class="libelles">
                  <span class="nom">{t('settings.panes')}</span>
                  <span class="desc">{t('settings.panesDesc')}</span>
                </span>
                <select class="langue" data-testid="affichage-volets"
                        aria-label={t('settings.panes')} value={String(panes)}
                        onchange={(e) => changePanes(Number(e.target.value))}>
                  {#each [3, 2, 1] as n (n)}
                    <option value={String(n)}>{t(`panes.${n}`)}</option>
                  {/each}
                </select>
              </div>
              <!-- A83: row spacing, to the EXACT pattern of Layout
                   (A26) — native selector dressed in the row's tokens,
                   no new design (A15: no new group for a single row). -->
              <div class="reglage">
                <span class="libelles">
                  <span class="nom">{t('settings.spacing')}</span>
                  <span class="desc">{t('settings.spacingDesc')}</span>
                </span>
                <select class="langue" data-testid="affichage-espacement"
                        aria-label={t('settings.spacing')} value={spacing}
                        onchange={(e) => changeSpacing(e.target.value)}>
                  {#each LEVELS as n (n)}
                    <option value={n}>{t(`spacing.${n}`)}</option>
                  {/each}
                </select>
              </div>
              <!-- R1 (RETOURS-11, D4): the "always show images from
                   this sender" rules, removable here. No new group for
                   a list (A15); nothing displays as long as no rule
                   exists — never an empty section. -->
              {#if imageSenders.length > 0}
                <div class="reglage">
                  <span class="libelles">
                    <span class="nom">{t('settings.imagesSenders')}</span>
                    <span class="desc">{t('settings.imagesSendersDesc')}</span>
                  </span>
                </div>
                {#each imageSenders as address (address)}
                  <div class="regle-images" data-testid="expediteur-images">
                    <span class="adresse-regle">{address}</span>
                    <button type="button" class="ajouter"
                            data-testid="retirer-expediteur-images"
                            onclick={() => removeImageSender(address)}>
                      {t('settings.removeSender')}</button>
                  </div>
                {/each}
              {/if}
            </div>
          {:else if group === 'screener'}
            <p class="section">{t('group.screener')}</p>
            <div class="rangees" data-testid="reglages-portier">
              <p class="desc-groupe">{t('settings.screenerDesc')}</p>
              {#if screenerDefaults}
              <div class="reglage">
                <span class="libelles">
                  <span class="nom">{t('settings.screenerYes')}</span>
                  <span class="desc">{t('settings.screenerYesDesc')}</span>
                </span>
                <select class="langue" data-testid="portier-defaut-oui"
                        aria-label={t('settings.screenerYes')} value={screenerDefaults.yes}
                        onchange={(e) => changeScreener('yes', e.target.value)}>
                  <option value="inbox">{t('screener.toInbox')}</option>
                  <option value="feed">{t('screener.toFeed')}</option>
                  <option value="paper_trail">{t('screener.toPaperTrail')}</option>
                </select>
              </div>
              <div class="reglage">
                <span class="libelles">
                  <span class="nom">{t('settings.screenerNo')}</span>
                  <span class="desc">{t('settings.screenerNoDesc')}</span>
                </span>
                <select class="langue" data-testid="portier-defaut-non"
                        aria-label={t('settings.screenerNo')} value={screenerDefaults.no}
                        onchange={(e) => changeScreener('no', e.target.value)}>
                  <option value="trash">{t('screener.ruleTrash')}</option>
                  <option value="archive">{t('screener.ruleArchive')}</option>
                  <option value="spam">{t('screener.ruleSpam')}</option>
                  <option value="screened_out">{t('screener.ruleScreenedOut')}</option>
                </select>
              </div>
              {/if}
              <!-- RETOURS-14 R5 (D6): all the decisions, alphabetized,
                   client-side search (a list of verdicts, not a
                   corpus — refused per §2.6), the Screener page's
                   "Reinstate" gesture. -->
              <div class="reglage">
                <span class="libelles">
                  <span class="nom">{t('settings.screenerDecisions')}</span>
                  <span class="desc">{t('settings.screenerDecisionsDesc')}</span>
                </span>
              </div>
              {#if routingsList?.length}
                <div class="recherche-decisions">
                  <input type="search" data-testid="portier-recherche"
                         placeholder={t('settings.screenerSearch')}
                         aria-label={t('settings.screenerSearch')}
                         bind:value={routingsFilter} />
                </div>
              {/if}
              {#if visibleRoutings}
                {#if routingsList.length === 0}
                  <p class="decisions-vide" data-testid="portier-decisions-vide">{t('settings.screenerNoDecision')}</p>
                {:else if visibleRoutings.length === 0}
                  <p class="decisions-vide" data-testid="portier-decisions-vide">{t('settings.screenerNoResult')}</p>
                {:else}
                  <div class="decisions" data-testid="portier-decisions">
                    {#each visibleRoutings as r (r.address)}
                      <div class="regle-images decision" data-testid="portier-decision">
                        <span class="adresse-regle"><b>{r.address}</b>
                          <span class="verdict">{routingLabel(r)}</span></span>
                        <button type="button" class="ajouter"
                                data-testid="decision-modifier"
                                aria-haspopup="menu"
                                aria-expanded={decisionMenu?.address === r.address}
                                onclick={(e) => openEdit(e, r)}>
                          {t('settings.screenerEdit')}</button>
                      </div>
                    {/each}
                  </div>
                {/if}
              {/if}
            </div>
          {:else if group === 'notifications'}
            <p class="section">{t('group.notifications')}</p>
            <div class="rangees" data-testid="reglages-notifications">
              <div class="reglage">
                <span class="libelles">
                  <span class="nom">{t('settings.bubbles')}</span>
                  <span class="desc">{t('settings.bubblesDesc')}</span>
                </span>
                <button type="button" class="bascule" role="switch"
                        aria-checked={bubbles} aria-label={t('settings.bubbles')}
                        data-testid="notif-bulles" onclick={toggleBubbles}>
                  <span class="bille"></span>
                </button>
              </div>
            </div>
          {:else if group === 'signature'}
            <p class="section">{t('group.signature')}</p>
            <div class="rangees" data-testid="reglages-signature">
              <p class="desc-groupe">{t('settings.signatureDesc')}</p>
              {#each accounts as c (c.account_id)}
                <div class="bloc-signature" data-testid="signature-compte">
                  <!-- D4 (PLAN-RETOURS-9): in Settings the name
                       displays WITH the address — here too: this is the
                       surface where editing the wrong account costs
                       (content sent). -->
                  <span class="adresse-signature">
                    <Icon name="person" />{#if names[c.account_id]}{names[c.account_id]}<span class="adresse-sous">{c.email}</span>{:else}{c.email}{/if}</span>
                  <!-- The reduced toolbar (D3): bold/italic/underline —
                       onmousedown neutralized, a format button never
                       steals the editor's selection (idiom A62). -->
                  <div class="barre-signature">
                    <button type="button" class="bouton-format" aria-label={t('compose.bold')}
                            title={t('compose.bold')} data-testid="signature-gras"
                            onmousedown={(e) => e.preventDefault()}
                            onclick={() => signatureCommand('bold')}>
                      <Icon name="format_bold" /></button>
                    <button type="button" class="bouton-format" aria-label={t('compose.italic')}
                            title={t('compose.italic')} data-testid="signature-italique"
                            onmousedown={(e) => e.preventDefault()}
                            onclick={() => signatureCommand('italic')}>
                      <Icon name="format_italic" /></button>
                    <button type="button" class="bouton-format" aria-label={t('compose.underline')}
                            title={t('compose.underline')} data-testid="signature-souligne"
                            onmousedown={(e) => e.preventDefault()}
                            onclick={() => signatureCommand('underline')}>
                      <Icon name="format_underlined" /></button>
                  </div>
                  <div class="editeur-signature" contenteditable="true" role="textbox"
                       aria-multiline="true" tabindex="0"
                       data-placeholder={t('settings.signaturePlaceholder')}
                       aria-label={t('settings.signaturePlaceholder')}
                       data-testid="signature-editeur"
                       bind:this={signatureFields[c.account_id]}
                       oninput={() => {
                         const sig = signatures[c.account_id];
                         if (sig?.state) signatures[c.account_id] = { ...sig, state: null };
                       }}></div>
                  <div class="reglage">
                    <span class="libelles">
                      <span class="nom">{t('settings.signatureReplies')}</span>
                      <span class="desc">{t('settings.signatureRepliesDesc')}</span>
                    </span>
                    <button type="button" class="bascule" role="switch"
                            aria-checked={signatures[c.account_id]?.replies ?? false}
                            aria-label={t('settings.signatureReplies')}
                            data-testid="signature-repliques"
                            onclick={() => toggleReplies(c)}>
                      <span class="bille"></span>
                    </button>
                  </div>
                  <div class="boutons-signature">
                    <button type="button" class="ajouter" data-testid="signature-enregistrer"
                            onclick={() => saveSignature(c)}>
                      <Icon name="signature" />{t('action.save')}</button>
                    <button type="button" class="ajouter" data-testid="signature-effacer"
                            onclick={() => clearSignature(c)}>{t('action.clear')}</button>
                    {#if accounts.length > 1}
                      <button type="button" class="ajouter" data-testid="signature-tous"
                              onclick={() => applyToAll(c)}>
                        {t('settings.signatureAll')}</button>
                    {/if}
                  </div>
                  {#if signatures[c.account_id]?.state === 'ok'}
                    <p class="etat-signature" data-testid="signature-etat">{t('toast.signature')}</p>
                  {:else if signatures[c.account_id]?.state === 'tous'}
                    <p class="etat-signature" data-testid="signature-etat">{t('toast.signatureAll')}</p>
                  {:else if signatures[c.account_id]?.state?.error}
                    <p class="erreur-retrait" data-testid="signature-erreur">
                      {signatures[c.account_id].state.error}</p>
                  {/if}
                </div>
              {/each}
            </div>
          {:else if group === 'raccourcis'}
            <p class="section">{t('settings.sectionShortcuts')}</p>
            <div class="rangees" data-testid="reglages-raccourcis">
              {#each SHORTCUTS as r (r)}
                <div class="raccourci">
                  <kbd>{t(`shortcut.key.${r}`)}</kbd>
                  <span class="geste">{t(`shortcut.gesture.${r}`)}</span>
                </div>
              {/each}
              <p class="note">{t('settings.noteShortcuts')}</p>
            </div>
          {:else if group === 'apropos'}
            <p class="section">{t('group.about')}</p>
            <div class="rangees" data-testid="reglages-apropos">
              <!-- V11: the brand IN TILE form — "About" is one of the
                   four spots under the frozen regime (W-D3). -->
              <span class="marque-bande apropos-bande"><Brand tile size={40} /><b>Wind</b></span>
              <div class="ligne-apropos">
                <span class="cle">{t('settings.version')}</span>
                <span class="valeur" data-testid="apropos-version">{version || '…'}</span>
              </div>
              <div class="ligne-apropos">
                <span class="cle">{t('settings.update')}</span>
                <span class="valeur">
                  {#if update === null}
                    <button type="button" class="ajouter" data-testid="apropos-verifier"
                            onclick={checkUpdate}>{t('settings.checkUpdate')}</button>
                  {:else if update === 'controle'}
                    {t('settings.checking')}
                  {:else if update === 'ajour'}
                    {t('settings.upToDate')}
                  {:else if update === 'installation'}
                    {t('settings.installing')}
                  {:else if update.version}
                    <!-- The INSTALL failure is said under its real
                         name (erreur.maj), and the action stays
                         offered. -->
                    {#if update.error}{t('error.update', { err: update.error })}. {/if}
                    {t('settings.updateAvailable', { version: update.version })}
                    <button type="button" class="ajouter" onclick={installUpdate}>
                      {t('action.install')}</button>
                  {:else}
                    {t('settings.updateFailed', { err: update.error })}
                  {/if}
                </span>
              </div>
              <div class="ligne-apropos">
                <span class="cle">{t('settings.icons')}</span>
                <span class="valeur">{t('settings.iconsValue')}</span>
              </div>
              <!-- R2 (PLAN-RETOURS-11, CE verdict from the visual
                   STOP): the origin mention is WITHOUT a key — a label
                   set alone,
                   detached from the key/value block that precedes it. -->
              <div class="origine" data-testid="apropos-origine">
                <EUFlag />{t('settings.originValue')}
              </div>
            </div>
          {/if}
        </div>
      </div>
      <div class="pied">
        <button type="button" class="principal" data-testid="reglages-termine" onclick={close}>
          {t('action.done')}</button>
      </div>
    </div>
  </div>
{/if}

<Menu isOpen={decisionMenu !== null} x={decisionMenu?.x ?? 0} y={decisionMenu?.y ?? 0}
      testid="decision-menu" onclose={() => (decisionMenu = null)}>
    <p class="titre-menu">{t('screener.yesTo')}</p>
    <button type="button" role="menuitem" data-testid="decision-vers-reception"
            onclick={() => editRouting('inbox')}>
      <Icon name="inbox" />{t('screener.toInbox')}</button>
    <button type="button" role="menuitem" data-testid="decision-vers-kiosque"
            onclick={() => editRouting('feed')}>
      <Icon name="feed" />{t('screener.toFeed')}</button>
    <button type="button" role="menuitem" data-testid="decision-vers-registre"
            onclick={() => editRouting('paper_trail')}>
      <Icon name="paper_trail" />{t('screener.toPaperTrail')}</button>
    <div class="filet-menu"></div>
    <p class="titre-menu">{t('screener.noWillBe')}</p>
    <button type="button" role="menuitem" data-testid="decision-regle-spam"
            onclick={() => editRouting('screened_out', 'spam')}>
      <Icon name="report" />{t('screener.ruleSpam')}</button>
    <button type="button" role="menuitem" data-testid="decision-regle-archive"
            onclick={() => editRouting('screened_out', 'archive')}>
      <Icon name="inventory_2" />{t('screener.ruleArchive')}</button>
    <button type="button" role="menuitem" data-testid="decision-regle-corbeille"
            onclick={() => editRouting('screened_out', 'trash')}>
      <Icon name="delete" />{t('screener.ruleTrash')}</button>
    <button type="button" role="menuitem" data-testid="decision-regle-ecarte"
            onclick={() => editRouting('screened_out')}>
      <Icon name="visibility_off" />{t('screener.ruleScreenedOut')}</button>
    <div class="filet-menu"></div>
    <button type="button" role="menuitem" data-testid="decision-renvoyer"
            onclick={sendBackToScreener}>
      <Icon name="screener" />{t('settings.resendScreener')}</button>
  </Menu>

<style>
  /* The prototype's signature card, widened to 800 px (A13). The
     height is FIXED (640 px, bounded to the screen): the rail must
     not breathe with whatever group is displayed. */
  .scrim {
    position:absolute; inset:0; background:var(--scrim); z-index:2;
    display:flex; align-items:center; justify-content:center; padding:36px;
  }
  .carte {
    width:800px; height:min(640px, 100%); background:var(--surface);
    border:1px solid var(--border);
    border-radius:var(--r-surface); box-shadow:var(--shadow);
    display:flex; flex-direction:column; overflow:hidden;
  }
  .tete {
    height:48px; flex:none; padding:0 16px 0 22px; display:flex;
    align-items:center; gap:14px; border-bottom:1px solid var(--border);
  }
  .titre { font-size:15px; font-weight:600; flex:1; color:var(--ink); }
  .fermer {
    height:32px; width:32px; padding:0; display:inline-flex;
    align-items:center; justify-content:center; color:var(--ink2);
    background:var(--surface); border:1px solid var(--border);
    border-radius:var(--r-control); cursor:pointer;
  }
  .fermer:hover { background:var(--sel); }

  .milieu { flex:1; display:flex; min-height:0; }

  /* The rail: its own grammar since A29 (the nav of screen 02 lives
     at the tracks' design) — 36 px rows, icon + label, active in
     white surface with the single shadow, no left rule. */
  .rail {
    width:220px; flex:none; background:var(--bg);
    border-right:1px solid var(--border); padding:20px 16px;
    display:flex; flex-direction:column; gap:4px; overflow:auto;
  }
  /* R2 (PLAN-RETOURS-13): the rail's glyph aligns like the nav's
     folders — label baseline + 2 px (the CE optical alignment,
     variant C, field 2026-08-27); flex centering was placing the SVG
     lower than in the nav. Same mechanics as Nav.svelte: the drop is
     a transform, outside geometry. The row keeps its 36 px (the
     rail's grammar, A13/A29): the label carries the baseline to
     center via its line-height, the icon hooks onto it. */
  .rang {
    display:flex; align-items:baseline; gap:10px; height:36px; flex:none;
    padding:0 12px; border-radius:var(--r-control); cursor:pointer;
    border:1px solid transparent;
  }
  .rang:hover { background:var(--sel); border-color:var(--border); }
  .rang.actif {
    background:var(--surface); border-color:var(--border);
    box-shadow:var(--shadow);
  }
  .icone { color:var(--muted); }
  .icone :global(.ic) { vertical-align:baseline; transform:translateY(2px); }
  .actif .icone { color:var(--accent); }
  .libelle {
    font-size:13px; line-height:36px; color:var(--ink2); flex:1;
    min-width:0; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
  }
  .actif .libelle { font-weight:600; color:var(--ink); }

  .volet {
    flex:1; padding:22px; display:flex; flex-direction:column; gap:14px;
    overflow:auto; min-width:0;
  }
  .section {
    margin:0; font-size:12px; letter-spacing:.1em; text-transform:uppercase;
    color:var(--muted); font-weight:600;
  }
  .rangees { display:flex; flex-direction:column; gap:6px; }
  .rangee {
    display:flex; align-items:center; gap:16px; padding:14px 16px;
    border-radius:var(--r-surface); cursor:pointer; border:1px solid transparent;
  }
  .rangee:hover { background:var(--sel); }
  .rangee.active {
    background:var(--surface); border:1px solid var(--border);
    box-shadow:var(--shadow);
  }
  .rangee.active:hover { background:var(--surface); }
  .pastilles { display:flex; gap:5px; flex:none; }
  .pastille {
    width:22px; height:22px; border-radius:var(--r-control);
    border:1px solid var(--border);
  }
  .libelles {
    display:flex; flex-direction:column; gap:2px; flex:1; min-width:0;
  }
  .nom { font-size:14px; font-weight:600; color:var(--ink); }
  .desc { font-size:12px; line-height:1.4; color:var(--muted); }
  .coche { color:var(--accent); }
  .compte {
    display:flex; align-items:center; gap:12px; padding:10px 16px;
    font-size:13px; color:var(--ink2);
  }
  /* A74: the marker's swatch keeps its own ink (measured color
     range) — only the row's neutral glyphs are muted. `:where(…)`:
     NULL specificity for the exclusion — otherwise the rule would
     take precedence over `.deconnecte :global(.ic)` and would douse
     the link_off alert glyph (review 2026-08-22). */
  .compte :global(:where(:not(.repere)) > .ic) { color:var(--muted); }
  .adresse {
    color:var(--ink); overflow:hidden; text-overflow:ellipsis;
    white-space:nowrap;
  }
  /* PLAN-RETOURS-9: the name's gate-label — a discreet button, the
     row stays a row; the hover says it opens. min-width:0 + overflow:
     the button shrinks and its text truncates — a long address never
     covers the gestures on the right (the ellipsis the row held
     before, review 2026-08-23). */
  .identite {
    display:flex; flex-direction:column; align-items:flex-start; gap:1px;
    min-width:0; overflow:hidden; padding:2px 6px; margin:0 -6px;
    font-size:13px; text-align:left; color:var(--ink);
    background:transparent; border:1px solid transparent;
    border-radius:var(--r-control); cursor:pointer;
  }
  .identite:hover { background:var(--sel); border-color:var(--border); }
  .identite .nom-compte, .identite .adresse {
    max-width:100%; overflow:hidden; text-overflow:ellipsis;
    white-space:nowrap;
  }
  /* `.name` belongs to the group cards (14px) — the account name has
     ITS OWN class, never a reuse (collision noted in review). */
  .nom-compte { color:var(--ink); font-weight:600; }
  .sous-nom { font-size:12px; color:var(--muted); }
  .champ-nom {
    height:32px; padding:0 10px; font-size:13px; color:var(--ink);
    background:var(--surface); border:1px solid var(--border);
    border-radius:var(--r-control);
  }
  .champ-nom:focus { border-color:var(--accent); outline:none; }
  .ajouter {
    height:32px; padding:0 16px; align-self:flex-start; display:inline-flex;
    align-items:center; gap:8px; font-size:13px; color:var(--ink);
    background:var(--surface); border:1px solid var(--border);
    border-radius:var(--r-control); cursor:pointer;
  }
  .ajouter:hover { background:var(--sel); }

  /* Dead token: the state is said in alert (link_off + "Disconnected"),
     pushed to the right with the repair gesture — a healthy account's
     row, meanwhile, doesn't change. */
  .deconnecte {
    margin-left:auto; flex:none; display:inline-flex; align-items:center;
    gap:6px; font-size:12.5px; font-weight:600; color:var(--alert);
    white-space:nowrap;
  }
  .deconnecte :global(.ic) { color:var(--alert); width:15px; height:15px; }
  .reconnecter {
    height:28px; padding:0 12px; flex:none; display:inline-flex;
    align-items:center; font-size:12.5px; font-weight:600;
    color:var(--ink); background:var(--surface);
    border:1px solid var(--border); border-radius:var(--r-control); cursor:pointer;
    white-space:nowrap;
  }
  .reconnecter:hover:not(:disabled) { background:var(--sel); }
  .reconnecter:disabled { opacity:.6; cursor:default; }
  /* A disconnected account already has its state on the right: the
     removal's trash icon loses its automatic push. */
  .compte:has(.deconnecte) .retirer, .compte:has(.btn-horizon) .retirer { margin-left:0; }
  /* The horizon's gate: the value as text, discreet at rest — the
     removal's design, without the alert. */
  .btn-horizon {
    height:28px; padding:0 10px; margin-left:auto; flex:none;
    display:inline-flex; align-items:center; font-size:12.5px;
    white-space:nowrap; color:var(--muted); background:transparent;
    border:1px solid transparent; border-radius:var(--r-control); cursor:pointer;
  }
  .btn-horizon:hover { color:var(--ink); background:var(--sel); border-color:var(--border); }
  .compte:has(.deconnecte) .btn-horizon { margin-left:0; }
  .select-horizon {
    height:32px; font-size:13px; padding:0 10px; align-self:flex-start;
    min-width:200px; color:var(--ink); background:var(--surface);
    border:1px solid var(--border); border-radius:var(--r-control);
    outline:none; cursor:pointer;
  }
  .note-horizon { margin:0; font-size:12px; line-height:1.5; color:var(--muted); }
  .erreur-reconnexion {
    margin:0; padding:0 16px 6px; font-size:12px; line-height:1.4;
    color:var(--alert);
  }

  /* Removal: discreet at rest (the row stays a row), the alert only
     shows on hover — a permanent red would shout on every healthy
     account. */
  .retirer {
    height:28px; padding:0 10px; margin-left:auto; flex:none;
    display:inline-flex; align-items:center; justify-content:center;
    gap:6px; font-size:12.5px; white-space:nowrap;
    color:var(--muted); background:transparent;
    border:1px solid transparent; border-radius:var(--r-control); cursor:pointer;
  }
  .retirer:hover {
    color:var(--alert); background:var(--sel); border-color:var(--border);
  }
  /* The "card under the row" — ONE rule for removal, marker and
     add (review 2026-08-22: three identical copies were drifting). */
  .carte-retrait, .carte-repere, .carte-ajout, .carte-nom {
    border:1px solid var(--border);
    border-radius:var(--r-surface); padding:14px 16px 16px;
    display:flex; flex-direction:column; gap:12px;
  }
  /* A74 — the marker: the gate is the row's icon (a discreet button,
     the removal's design), the choice card follows the removal
     card's pattern — the SAME rule block, not a copy. */
  .btn-repere {
    height:28px; width:28px; padding:0; flex:none;
    display:inline-flex; align-items:center; justify-content:center;
    background:transparent; border:1px solid transparent;
    border-radius:var(--r-control); cursor:pointer;
  }
  .btn-repere:hover { background:var(--sel); border-color:var(--border); }
  .titre-repere { margin:0; font-size:13px; font-weight:600; color:var(--ink); }
  .choix-repere { display:flex; flex-wrap:wrap; gap:6px; }
  .choix {
    height:32px; width:32px; padding:0; display:inline-flex;
    align-items:center; justify-content:center; color:var(--ink2);
    background:var(--surface); border:1px solid var(--border);
    border-radius:var(--r-control); cursor:pointer;
  }
  .choix:hover { background:var(--sel); }
  .choix.choisi { border-color:var(--accent); background:var(--sel); }
  .pastille-teinte { width:18px; height:18px; }
  .erreur-repere { margin:0; font-size:12px; line-height:1.4; color:var(--alert); }
  .avertissement { margin:0; font-size:13px; line-height:1.5; color:var(--ink2); }
  .erreur-retrait { margin:0; font-size:12px; line-height:1.4; color:var(--alert); }
  .boutons-retrait { display:flex; align-items:center; gap:10px; }
  .danger {
    height:32px; padding:0 16px; display:inline-flex; align-items:center;
    gap:8px; font-size:13px; font-weight:600; color:var(--onAccent);
    background:var(--alert); border:1px solid var(--alert);
    border-radius:var(--r-control); cursor:pointer;
  }
  .danger:disabled { opacity:.6; cursor:default; }
  .tete-ajout { display:flex; align-items:center; gap:14px; }
  .titre-ajout { flex:1; font-size:14px; font-weight:600; color:var(--ink); }

  /* A setting row: label + description, switch on the right. The
     switch stays on tokens — `--bg` track/rule at rest (V3), accent
     when armed; visible focus inherited (A8). */
  .reglage {
    display:flex; align-items:center; gap:16px; padding:14px 16px;
    border-radius:var(--r-surface);
  }
  .bascule {
    width:38px; height:22px; flex:none; padding:2px; cursor:pointer;
    display:inline-flex; align-items:center;
    background:var(--bg); border:1px solid var(--border);
    border-radius:999px; transition:background .12s ease;
  }
  .bille {
    width:16px; height:16px; border-radius:50%;
    background:var(--surface); border:1px solid var(--border);
    transition:transform .12s ease;
  }
  .bascule[aria-checked="true"] {
    background:var(--accent); border-color:var(--accent);
  }
  .bascule[aria-checked="true"] .bille {
    transform:translateX(16px); border-color:var(--accent);
  }

  /* The selectors (Language, Layout): the buttons' grammar
     (32 px, tokens) — a native <select>, keyboard and screen reader
     included. */
  .langue {
    height:32px; padding:0 10px; flex:none; font:inherit; font-size:13px;
    color:var(--ink); background:var(--surface);
    border:1px solid var(--border); border-radius:var(--r-control); cursor:pointer;
  }
  .langue option { background:var(--surface); color:var(--ink); }

  /* R1 (RETOURS-11, D4): one image rule per row — the address and
     its exit gate, on the card's tokens. */
  .regle-images {
    display:flex; align-items:center; gap:12px; padding:6px 16px;
    font-size:13px; color:var(--ink);
  }
  .adresse-regle {
    flex:1; min-width:0; overflow:hidden; text-overflow:ellipsis;
    white-space:nowrap;
  }
  /* RETOURS-14 R5: the Screener's decisions list — row in the design
     of .regle-images, verdict in muted ink behind the address; search
     field on the controls' template (32 px). */
  .decision .verdict { margin-left:8px; color:var(--muted); }
  .recherche-decisions { padding:2px 16px 8px; }
  .recherche-decisions input {
    width:100%; height:32px; padding:0 12px; font-size:13px;
    color:var(--ink); background:var(--surface);
    border:1px solid var(--border); border-radius:var(--r-control);
  }
  .recherche-decisions input:focus-visible {
    outline:2px solid var(--accent); outline-offset:-1px;
  }
  .decisions-vide {
    margin:0; padding:6px 16px 10px; font-size:13px; color:var(--muted);
  }
  /* R10: the "Edit" menu — the product's menu design (D-47 family,
     recorded). Above the Settings overlay (z-index 2). */

  /* Shortcuts: read-only reference, on tokens. */
  .raccourci {
    display:flex; align-items:center; gap:14px; padding:8px 16px;
    font-size:13px; color:var(--ink2);
  }
  kbd {
    min-width:44px; padding:3px 8px; text-align:center; flex:none;
    font-family:inherit; font-size:12px; font-weight:600; color:var(--ink);
    background:var(--bg); border:1px solid var(--border);
    border-bottom-width:2px; border-radius:var(--r-control);
  }
  .geste { color:var(--ink2); }
  .note {
    margin:6px 0 0; padding:0 16px; font-size:12px; line-height:1.4;
    color:var(--muted);
  }

  /* About: key / value, no invented form. */
  /* The shared band (.marque-bande, system.css) — here with its
     own clearance. */
  .apropos-bande { padding:2px 0 6px; }
  .ligne-apropos {
    display:flex; align-items:baseline; gap:14px; padding:10px 16px;
    font-size:13px;
  }
  .cle { width:110px; flex:none; color:var(--muted); }
  .valeur {
    color:var(--ink); display:inline-flex; flex-wrap:wrap;
    align-items:center; gap:10px; min-width:0;
  }
  /* The origin mention (R2, RETOURS-11): without a key, detached from
     the key/value block by a top margin, and ALIGNED to the values
     column (CE verdicts from the visual STOP): 16 px from the edge +
     110 px of key + 14 px of gutter = 140 px. */
  .origine {
    display:flex; align-items:center; gap:10px; margin-top:18px;
    padding:10px 16px 10px calc(16px + 110px + 14px);
    font-size:13px; color:var(--ink);
  }

  /* R1 (PLAN-RETOURS-6): the Signature group — one block per account,
     rich editor reduced to the card's tokens. */
  .desc-groupe {
    margin:0; padding:0 16px 4px; font-size:12px; line-height:1.5;
    color:var(--muted);
  }
  .bloc-signature {
    display:flex; flex-direction:column; gap:10px; padding:12px 16px;
    border:1px solid var(--border); border-radius:var(--r-control);
  }
  .adresse-signature {
    display:flex; align-items:center; gap:8px;
    font-size:13px; font-weight:600; color:var(--ink);
  }
  .adresse-sous { font-weight:400; color:var(--muted); }
  .barre-signature { display:flex; align-items:center; gap:6px; }
  .bouton-format {
    height:32px; min-width:32px; padding:0 6px; display:inline-flex;
    align-items:center; justify-content:center; color:var(--ink2);
    background:var(--surface); cursor:pointer;
    border:1px solid var(--border); border-radius:var(--r-control);
  }
  .bouton-format:hover { background:var(--sel); color:var(--ink); }
  .editeur-signature {
    min-height:72px; padding:10px 12px; font-size:13px; line-height:1.6;
    color:var(--ink); background:var(--surface);
    border:1px solid var(--border); border-radius:var(--r-control); outline:none;
    overflow-wrap:break-word;
  }
  .editeur-signature:focus { border-color:var(--accent); }
  .editeur-signature:empty::before {
    content:attr(data-placeholder); color:var(--muted); pointer-events:none;
  }
  .boutons-signature { display:flex; align-items:center; gap:10px; flex-wrap:wrap; }
  .etat-signature { margin:0; font-size:12px; line-height:1.4; color:var(--accent); }

  .pied {
    flex:none; padding:14px 22px 18px; border-top:1px solid var(--border);
    display:flex; align-items:center;
  }
  .principal {
    height:32px; padding:0 16px; margin-left:auto; display:inline-flex;
    align-items:center; gap:8px; font-size:13px; font-weight:600;
    color:var(--onAccent); background:var(--accent);
    border:1px solid var(--accent); border-radius:var(--r-control); cursor:pointer;
  }
  .principal:hover { background:var(--accentH); border-color:var(--accentH); }
</style>

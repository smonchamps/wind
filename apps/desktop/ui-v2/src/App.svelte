<script>
  // Screen 02 of the prototype (A6): 60 px header, 236/400/1fr grid
  // (236/1fr in two panes — PLAN-VOLETS), 36 px status bar.
  // REAL data and actions through the port.
  // P5: blocking migration first (ADR 0012), notice slot (at most
  // ONE), progress line (at most ONE), wired-up search (D1),
  // shortcuts (D3).
  import Icon from './Icon.svelte';
  import { onMount, tick } from 'svelte';
  import { call } from './lib/transport.js';
  import { t, setDetectedLanguage } from './lib/text.svelte.js';
  import { currentPanes } from './lib/panes.svelte.js';
  import { onboardingDone, markOnboardingDone, onboardingStarted } from './lib/onboarding.js';
  import {
    currentWidth,
    setWidth,
    persistWidths,
    applyWidth,
    defaultWidth,
    BOUNDS,
  } from './lib/widths.svelte.js';
  import { since, whenLong } from './lib/when.js';
  import { mixedView } from './lib/mailbox.js';
  import Nav from './Nav.svelte';
  import List from './List.svelte';
  import Screener from './Screener.svelte';
  import Cleanup from './Cleanup.svelte';
  import SetAsidePile from './SetAsidePile.svelte';
  import Feed from './Feed.svelte';
  import PaperTrail from './PaperTrail.svelte';
  import Reading from './Reading.svelte';
  import Conversation from './Conversation.svelte';
  import Compose from './Compose.svelte';
  import Settings from './Settings.svelte';
  import Feedback from './Feedback.svelte';
  import Onboarding from './Onboarding.svelte';
  import NoticeSlot from './NoticeSlot.svelte';
  import MigrationModal from './MigrationModal.svelte';
  import Toast from './Toast.svelte';
  import Brand from './Brand.svelte';
  import {
    thread, closeThread, shrinkThread, removeMessage, isEcho, msgKey,
  } from './lib/thread.svelte.js';
  // PLAN-MODE-ORGANISE E1: the "Organized" toggle — the state lives
  // in SQLite prefs (D2 amended), the UI reflects it.
  import {
    organizedMode, restoreOrganizedMode, toggleOrganizedMode,
    mailboxLabelKey,
  } from './lib/organized.svelte.js';

  let list = $state(null);
  let reading = $state(null);
  // The conversation REPLACES the screen (prototype): it overlays full
  // screen, the mailbox stays mounted underneath — scrolling, pages and
  // selection are intact on return.
  let conversation = $state(null);
  let compose = $state(null);
  let settings = $state(null);
  // The beta feedback form (RETOURS-11 R3, field 2026-08-28).
  let back = $state(null);
  let migrationModal = $state(null);
  let searchField = $state(null);

  // Nothing touches the database until a legacy database is adopted:
  // the columns only appear after the migration modal (ADR 0012).
  let ready = $state(false);

  let accounts = $state([]);
  // The addresses currently CONNECTED (a session lives on the Rust
  // side) — a subset of the `accounts` registry; the difference is the
  // accounts with a dead token, which Settings can now reconnect.
  let connected = $state([]);
  // Screen 01 only appears once the nav is KNOWN to be empty — never
  // during the first load, otherwise it would flicker on every
  // startup.
  let navReady = $state(false);
  // E2: the Screener badge — the number of MESSAGES waiting at the
  // desk, reloaded with the nav (never on the display path).
  let screenerTotal = $state(0);
  // RETOURS-14 R7: the Feed badge (cards never opened, D8) and the
  // Paper trail badge (IMAP unread) — the pattern of `screenerTotal`.
  let feedTotal = $state(0);
  let paperTrailTotal = $state(0);
  // R2 (PLAN-RETOURS-8, A75): the first-launch journey — null = not
  // yet decided (a single state, review 2026-08-22), decided once at
  // the first nav snapshot, turned off at Finish.
  let onboardingToPlay = $state(null);
  // R1 (PLAN-RETOURS-8): the account markers (icon + hue), indexed by
  // account_id — the nav and the list READ them, Settings sets them.
  // An account missing from the table has no marker.
  let markers = $state({});
  let names = $state({});
  let category = $state('inbox');
  let account = $state(null);
  let tab = $state('tous');
  let search = $state('');
  let resultCount = $state(null);
  let totalCount = $state(null);
  let listTotal = $state(0);
  let sync = $state(null);
  // The cycle in progress, seen by the activity probe (E1): null at rest.
  let activity = $state(null);
  // The clock that ages "N minutes ago": re-paced every 30 s, without
  // anyone clicking.
  let now = $state(Date.now());
  let toast = $state(null);
  let toastTimer;
  // The current selection, for the shortcuts (D3): r/f/e/Delete act
  // on it.
  let selectedRow = $state(null);

  // PLAN-VOLETS (V-D1): the number of panes drives both the grid AND
  // the opening surface — 3: the reading pane; 2 and 1: screen 03,
  // full screen (V-D2). Reading is UNMOUNTED under three panes, hence
  // the `lecture?.` guards everywhere; in one pane the Nav leaves the
  // grid and lives in a DRAWER (E2). On return to three panes, the
  // current selection reopens its pane — the screen does not come
  // back empty when a row is still chosen.
  const panes = $derived(currentPanes());
  let panesBefore = currentPanes();

  // PLAN-RETOURS-V3 R3 (CE verdict D3): the grid boundaries are set
  // with the mouse — nav|list and list|thread in three panes, nav|list
  // alone in two. The handle captures the pointer: the drag follows
  // outside its surface, the thread's iframe never swallows it;
  // double-click restores the default; the arrow keys do the same
  // gesture from the keyboard (A8), 16 px per step. The bounds live in
  // the module; the CAP lives here (the window is UI knowledge): in
  // three panes, a boundary never rises to the point of crushing the
  // thread under RESERVE_FIL — the cumulative maximum bounds
  // (400 + 640) exceed the default window, and a handle pushed off
  // screen would be unrecoverable (review 2026-08-16). The drag SETS
  // (state only); the release PERSISTS — never a write per
  // pointermove. The grab is also released on pointercancel and
  // lostpointercapture (touch, stylus, block unmounted mid-gesture):
  // without this it would stay armed and the next hover would resize
  // with no button pressed.
  const lNav = $derived(currentWidth('nav'));
  const listWidth = $derived(currentWidth('list'));
  const THREAD_RESERVE = 120;
  const handleCap = (pane) =>
    panes === 3
      ? window.innerWidth -
        currentWidth(pane === 'nav' ? 'list' : 'nav') -
        THREAD_RESERVE
      : Infinity;
  let handleGrab = null; // { pane, x0, l0 } — outside $state: only the module state moves
  function grabHandle(pane, e) {
    if (e.button !== 0) return; // only the primary button grabs
    handleGrab = { pane, x0: e.clientX, l0: currentWidth(pane) };
    e.currentTarget.setPointerCapture(e.pointerId);
  }
  function dragHandle(e) {
    if (!handleGrab) return;
    const { pane, x0, l0 } = handleGrab;
    setWidth(pane, l0 + (e.clientX - x0), handleCap(pane));
  }
  function releaseHandle() {
    if (!handleGrab) return;
    handleGrab = null;
    persistWidths();
  }
  function keyHandle(pane, e) {
    const delta = e.key === 'ArrowLeft' ? -16 : e.key === 'ArrowRight' ? 16 : 0;
    if (!delta) return;
    e.preventDefault();
    applyWidth(pane, currentWidth(pane) + delta, handleCap(pane));
  }
  $effect(() => {
    const v = panes;
    if (v === 3 && panesBefore !== 3 && selectedRow && thread.frame !== 'full') reading?.open(selectedRow);
    // Leaving one-pane mode takes the drawer with it — it no longer makes sense.
    if (v !== 1) drawerOpen = false;
    panesBefore = v;
  });

  // The nav drawer (one-pane mode): an overlay under a scrim, the Nav
  // reused as is. Choosing a folder or an account CLOSES it — the
  // completed gesture no longer needs the panel; Escape and the scrim
  // also close it.
  let drawerOpen = $state(false);
  function chooseFromDrawer(what) {
    drawerOpen = false;
    choose(what);
  }

  // --- Notice slot (§6): at most ONE, decreasing priority ----------
  // Drafts no longer live there (PLAN-BROUILLONS): they are in the
  // list — Drafts folder and a mention on the thread.
  let sendNotice = $state(null);
  // PLAN-AUDIT-V1 E3 (D2): quarantined log actions — the server
  // refused them. An incident: right after the send failure, before
  // everything else. No button (wave 2).
  let refusalNotice = $state(null);
  let connectionNotice = $state(null);
  let updateNotice = $state(null);
  let crashNotice = $state(null);
  let telemetryNotice = $state(null);
  // R2 (PLAN-RETOURS-6, D2): the scheduled send is seen and cancelled
  // from here — informational, so LAST in priority (an incident takes
  // precedence).
  let scheduledNotice = $state(null);
  const notice = $derived(
    sendNotice ?? refusalNotice ?? connectionNotice ?? updateNotice ?? crashNotice ?? telemetryNotice ?? scheduledNotice,
  );

  // --- Progress line (§6): at most ONE ------------------------
  let pendingSends = $state(0);
  // R2: the scheduled sends not yet due, and the closest deadline —
  // kept separate from the "pending" ones (those don't wait for the
  // network, they wait for their time).
  let scheduledSends = $state(0);
  let nextScheduled = $state(null);
  let previewBackfill = $state(false);
  let bodyBackfill = $state(null); // remaining, or null if nothing to do
  // R1 (PLAN-RETOURS-3, D1): the % of bodies already there on the
  // corpus in scope — computed by the core (`backfill_percent`), shown
  // in the TEXT next to the remainder (A52: the stroke only loops).
  let backfillPct = $state(null);
  // TOTAL failure of the last sync: in the line, not the slot — §6
  // doesn't put sync there, and "offline" is not an incident.
  let syncFailure = $state(false);
  // E3: the PARTIAL failure is stated — "1 of 2 accounts unreachable".
  // `syncFailure` only covered total failure: one dead account out of
  // two was invisible, and the timestamp was refreshed by the survivor.
  let syncPartial = $state(null);
  // P0-bis: the OS network state, surfaced by the WebView almost
  // instantly (the equivalent of Thunderbird's network observer) —
  // instead of waiting for a cycle to stall on the socket timeout
  // (120 s) to understand we're offline. `navigator.onLine` can lie
  // (Wi-Fi with no internet), but the field has shown the case that
  // matters: cable/Wi-Fi cut, where it switches correctly.
  let online = $state(navigator.onLine);
  // The verdict of a poll report (full cycle OR light pass): total
  // failure, partial failure, or nothing — a single write, the two
  // states cannot diverge.
  function failedUpdates(report) {
    syncFailure = report.accounts === 0 && report.errors.length > 0;
    syncPartial = report.accounts_failed > 0 && report.accounts > 0
      ? { n: report.accounts_failed, m: report.accounts_failed + report.accounts }
      : null;
  }

  // RETOURS-13 R3: a mailbox's label comes out of THE shared rule
  // (cleLibelleBoite, lib/organise) — the old LIBELLES table only
  // copied `boite.${id}`.

  // The ENTIRE status line — text, disc/ring pair (V2: the `thread` ring
  // as soon as an action is running, `stroke` filled disc at rest;
  // A52: the % lives in the TEXT), alert dot — comes out of a single
  // decision: the three cannot diverge. At most ONE progress
  // indicator (System A4); priorities re-sorted by sincerity
  // (PLAN-SYNCHRO E1): the current cycle first — that's what the user
  // is waiting for — then the full sync, the backfills, the send
  // queue, the failure, the timestamped rest.
  const line = $derived.by(() => {
    if (resultCount !== null) {
      // "N of M" when the render is capped (M > N), otherwise "N results".
      const text =
        totalCount !== null && totalCount > resultCount
          ? t('status.searchCap', { n: resultCount, total: totalCount })
          : t('status.search', { n: resultCount });
      return { text, thread: null, alert: false };
    }
    if (category !== 'inbox') {
      // An account not yet PROVEN (null: the source hasn't answered
      // yet, PLAN-DEFILEMENT-PROFOND E2) is not shown — the mailbox
      // name alone, never a waiting "0 items".
      return {
        text:
          listTotal === null
            ? t(mailboxLabelKey(category))
            : t('status.category', { mailbox: t(mailboxLabelKey(category)), n: listTotal }),
        thread: null,
        alert: false,
      };
    }
    // P0-bis: offline, we SAY it — right away. Takes precedence over
    // the sync block: a "Syncing…" or an "up to date" would be false
    // without a network. We live off the stock, and we say since when.
    if (!online) {
      const last = sync?.last ?? null;
      return {
        text: last
          ? t('status.offlineSince', { since: since(last, now) })
          : t('status.offline'),
        thread: null,
        alert: false,
      };
    }
    // The current cycle: never "up to date" again while the machine is
    // working — and EVERYTHING we know is shown (field 2026-08-13:
    // "2/2 · account" stuck for 7 minutes during the folder sweep).
    // Rank, account, current mailbox; the % of the full sync when it
    // exists, because the full sync IS the cycle — hiding it was a
    // regression from the pre-E1 display. Determinate bar with the %,
    // sweep otherwise.
    if (syncing) {
      const pct =
        sync && sync.percent !== null && sync.percent < 100 ? sync.percent : null;
      const parts = [t('status.cyclePrefix')];
      if (activity) {
        if (activity.total > 1) {
          parts.push(`${Math.min(activity.done + 1, activity.total)}/${activity.total}`);
        }
        if (activity.account) parts.push(activity.account);
        // The mailbox in clear text, or the translated step (the shell
        // only sends a key — A15): field observation must be able to
        // NAME what's taking long.
        if (activity.mailbox) parts.push(activity.mailbox);
        else if (activity.phase) parts.push(t(`status.phase.${activity.phase}`));
      }
      let text = `${parts.join(' · ')}…`;
      // A52/D1: the full-sync percentage stays in the TEXT; the
      // stroke itself now only loops while an action is running.
      if (pct !== null) text += ` · ${t('status.percent', { p: pct })}`;
      return { text, thread: true, alert: false };
    }
    if (sync && sync.percent !== null && sync.percent < 100) {
      return {
        text: t('status.sync', { p: sync.percent }),
        thread: true,
        alert: false,
      };
    }
    if (bodyBackfill !== null && bodyBackfill > 0) {
      // The % accompanies the remainder (D1); a safeguard in case the
      // denominator were missing (empty corpus — impossible when
      // bodies remain).
      return {
        text: backfillPct !== null
          ? t('status.bodyBackfill', { n: bodyBackfill, p: backfillPct })
          : t('status.bodyBackfillAlone', { n: bodyBackfill }),
        thread: true,
        alert: false,
      };
    }
    if (previewBackfill) {
      return { text: t('status.previewBackfill'), thread: true, alert: false };
    }
    if (pendingSends > 0) {
      // A queued send is an action in progress (A52): the stroke loops
      // until the flush. Offline is caught higher up — the stroke
      // never spins for nothing.
      return { text: t('status.sends', { n: pendingSends }), thread: true, alert: false };
    }
    if (scheduledSends > 0 && nextScheduled !== null) {
      // R2: a scheduled send doesn't wait for the network, it waits
      // for its time — a dated resting state, not a looping stroke.
      return {
        text: t('status.scheduled', {
          n: scheduledSends,
          when: whenLong(nextScheduled),
        }),
        thread: null,
        alert: false,
      };
    }
    // The prototype's timestamp, at last: "last sync N minutes ago" —
    // and on failure, since when we've been living off the stock.
    const last = sync?.last ?? null;
    if (syncFailure) {
      return {
        text: last
          ? t('status.syncFailedSince', { since: since(last, now) })
          : t('status.syncFailed'),
        thread: null,
        alert: true,
      };
    }
    // E3: the partial failure — the mail of the accounts that are
    // alive is there, but at least one account is dry, and it's
    // stated (alert).
    if (syncPartial) {
      return {
        text: t('status.syncPartial', { n: syncPartial.n, m: syncPartial.m }),
        thread: null,
        alert: true,
      };
    }
    // V2: at the "Up to date" states, the filled disc precedes the
    // text — motionless (the ring only spins during a cycle).
    return {
      text: last
        ? t('status.upToDateSince', { since: since(last, now) })
        : t('status.upToDate'),
      thread: null,
      alert: false,
      stroke: true,
    };
  });

  function flash(message) {
    toast = message;
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (toast = null), 2200);
  }

  // Generation guard (same pattern as List.svelte): two in-flight
  // snapshots can resolve out of order since the commands moved off
  // the pump (PLAN-GELS) — without it, the OLDEST would overwrite the
  // fresh one and the unread badge would drift on its own.
  let navToken = 0;
  // PLAN-AUDIT-V2 E10: after a gesture, the nav used to be requested
  // up to three times in a burst — coalesced (50 ms) on the fetch
  // path; the resting probe (`ui_state`) supplies the snapshot.
  let navRecent = false;
  function loadNav(providedSnapshot = null) {
    if (providedSnapshot !== null) return loadNavNow(providedSnapshot);
    if (navRecent) return Promise.resolve();
    navRecent = true;
    setTimeout(() => {
      navRecent = false;
    }, 50);
    return loadNavNow(null);
  }
  async function loadNavNow(providedSnapshot) {
    const token = ++navToken;
    try {
      const snapshot = providedSnapshot ?? (await call('nav_snapshot'));
      if (token !== navToken) return;
      accounts = snapshot;
      navReady = true;
      // E2: the Screener badge follows the nav. WITHOUT a mode
      // condition (E2 review): at startup, `restoreOrganizedMode()`
      // hasn't answered yet when the first snapshot arrives — a
      // `organizedMode()` guard would leave the badge empty until the
      // next probe (10 s). The command costs 0.26 ms; outside the
      // mode, the value is painted nowhere.
      call('screener_total')
        .then((n) => {
          if (token === navToken) screenerTotal = n;
        })
        .catch(() => {});
      // RETOURS-14 R7 (review): unlike `screener_total` (0.26 ms
      // measured), these two COUNTs probe the entire `threads` — they
      // aren't paid for in Classic mode, where the badges are painted
      // nowhere. The startup gap (mode not yet reread) is closed on
      // the return of `restoreOrganizedMode()`. Real cost on a 200 k
      // database: to be measured in the field.
      if (organizedMode()) {
        call('feed_unopened')
          .then((n) => {
            if (token === navToken) feedTotal = n;
          })
          .catch(() => {});
        call('category_total', { category: 'paper_trail', accountId: null, unread: true })
          .then((n) => {
            if (token === navToken) paperTrailTotal = n;
          })
          .catch(() => {});
      }
      // R2 (A75): the "onboarding to play" decision is made ONCE, at
      // the first snapshot. An existing installation (accounts
      // already there WITHOUT a journey started) is deemed onboarded
      // — the key is set with no journey; a journey STARTED then
      // abandoned (account added at step 1, app quit before Finish)
      // resumes, on the other hand, at the next launch (review
      // 2026-08-22); a blank database plays the whole journey. The
      // e2e seam lives in lib/onboarding.js, not here.
      if (onboardingToPlay === null) {
        if (accounts.length > 0 && !onboardingDone() && !onboardingStarted()) {
          markOnboardingDone();
        }
        onboardingToPlay = !onboardingDone();
      }
    } catch (err) {
      console.error('nav_snapshot :', err);
    }
  }

  // R1: the markers, loaded ONCE at startup — never probed (a
  // preference only moves through a local gesture); a change from
  // Settings PATCHES the table in place (review 2026-08-22: never a
  // full reload on a hue click).
  async function loadMarkers() {
    try {
      const lines = await call('markers_get');
      const map = {};
      for (const l of lines) map[l.account_id] = { icon: l.icon, hue: l.hue };
      markers = map;
    } catch (err) {
      console.error('markers_get :', err);
    }
  }
  function patchMarker(id, marker) {
    const map = { ...markers };
    if (marker) map[id] = marker;
    else delete map[id];
    markers = map;
  }

  // PLAN-RETOURS-9 (D3/D4): the custom names, same regime as the
  // markers — loaded once, patched on gesture.
  async function loadNames() {
    try {
      const lines = await call('names_get');
      const map = {};
      for (const l of lines) map[l.account_id] = l.name;
      names = map;
    } catch (err) {
      console.error('names_get :', err);
    }
  }
  function patchName(id, name) {
    const map = { ...names };
    if (name) map[id] = name;
    else delete map[id];
    names = map;
  }

  // Backfill of previews for bodies written before the `preview`
  // column: in batches, never on the opening path nor on scroll.
  // Converges then goes quiet; the list refreshes once the pass is
  // closed out to show the backfilled previews. Batch of 500
  // (PLAN-GELS D2): at 2,000 bodies (~130 MB read, measured
  // 2026-08-15) the write transaction was lengthening — the short
  // lock protects concurrent UI gestures from BUSY (delete_draft
  // lesson).
  let previewsInProgress = false;
  async function backfillPreviews() {
    // Guarded against reentrancy like `backfillBodies` (PLAN-AUDIT-V2
    // E10): two preview pumps were doubling up on the same queue.
    if (previewsInProgress) return;
    previewsInProgress = true;
    try {
      let remainingOnes = await call('preview_catchup', { limit: 500 });
      previewBackfill = remainingOnes > 0;
      while (remainingOnes > 0) {
        await new Promise((r) => setTimeout(r, 250));
        remainingOnes = await call('preview_catchup', { limit: 500 });
      }
      reloadViews();
    } catch (err) {
      console.error('preview_catchup :', err);
    } finally {
      previewsInProgress = false;
      previewBackfill = false;
    }
  }
  // E4: the mail generation, monotone — bumped by any INBOX poll that
  // reported something (cycle, button, IDLE watcher). When it moves
  // at rest, it's a watcher that has polled: the list reloads on the
  // beat of this probe (5 s), with no new channel (R0-S5).
  let viewGeneration = null;
  async function probeSync(providedSync = null) {
    try {
      sync = providedSync ?? (await call('sync_progress'));
      const generation = sync?.generation ?? null;
      if (generation !== null) {
        if (viewGeneration !== null && generation !== viewGeneration) {
          // E5bis (review): the scenes follow the polls — otherwise
          // an arrival shifts the offsets of the following pages
          // (colliding keys) and a fresh card stays a preview.
          reloadViews();
          loadNav();
          // E4 (PLAN-REACTIVITE): the generation has moved — a batch
          // just came in. Its bodies are already there (poll, R-D2)
          // UNLESS it overflowed the limit: the pump covers the
          // overflow and the stock, guarded against reentrancy — a
          // no-op when everything is there.
          backfillBodies();
        }
        viewGeneration = generation;
      }
    } catch { /* offline or core busy: the status keeps its last value */ }
  }

  // Body backfill (v1 had it in a banner; here it's the progress
  // line): one batch at a time, hard stop if a batch reports nothing
  // — offline, the loop doesn't spin for nothing. Guarded against
  // reentrancy (E4): the generation can trigger it while a pass is
  // already running — one pump at a time, the shell lock stacks
  // nothing.
  let bodyInProgress = false;
  async function backfillBodies() {
    if (bodyInProgress) return;
    bodyInProgress = true;
    try {
      const state = await call('backfill_status');
      if (state.remaining === 0) return;
      bodyBackfill = state.remaining;
      backfillPct = state.percent;
      let remaining = state.remaining;
      while (remaining > 0) {
        const report = await call('backfill_bodies');
        remaining = report.remaining;
        bodyBackfill = remaining;
        backfillPct = report.percent;
        if (report.fetched === 0) break;
        // E4: the backfilled previews appear batch by batch — the
        // reload is invisible since E1, no more need to wait for a
        // lucky refresh. E5bis: the Feed cards gain their bodies at
        // the same pace.
        reloadViews();
      }
    } catch (err) {
      console.error('backfill_bodies :', err);
    } finally {
      bodyInProgress = false;
      bodyBackfill = null;
      backfillPct = null;
    }
  }

  // --- Sources of the notice slot (§6), by priority ------------------

  // 1. Send failure — UI corollary of the golden rules: NEVER invisible.
  //    The blameless wait (queued) lives in the progress line.
  // R2: the departure of a scheduled send. The probe (10 s) rearms this
  // short timer when the deadline approaches (< 60 s) — at the appointed
  // time, ONE flush goes out. Never a long timer: a cancellation in the
  // meantime is seen by the next probe, which disarms it.
  let scheduledTimer = null;
  function armStart(deadline) {
    clearTimeout(scheduledTimer);
    scheduledTimer = null;
    if (deadline === null) return;
    const delay = Math.max(0, deadline * 1000 - Date.now()) + 1000;
    if (delay > 60000) return; // the next probe will rearm, closer to it
    scheduledTimer = setTimeout(async () => {
      try {
        const report = await call('flush_outbox');
        if (report.sent > 0) {
          // The Sent echo was born at the flush (E3) — the copy shows
          // without waiting; reconciliation will follow at the cycle.
          list?.reload();
          loadNav();
        }
      } catch (err) {
        console.error('flush_outbox (scheduled send):', err);
      }
      probeSends();
    }, delay);
  }

  async function probeSends(providedState = null) {
    try {
      const state = providedState ?? (await call('outbox_status'));
      const refused = state.actions_refusees ?? 0;
      refusalNotice = refused > 0
        ? { alert: true, icon: 'error', text: t('notice.refusedActions', { n: refused }), actions: [] }
        : null;
      pendingSends = state.queued;
      scheduledSends = state.scheduled ?? 0;
      nextScheduled = state.next_scheduled_epoch ?? null;
      armStart(nextScheduled);
      // R2/D2: the nearest scheduled send is seen in the slot, with
      // its cancel gesture (back to draft — reversible).
      const scheduledEntry = state.entries.find((e) => e.send_at_epoch != null);
      scheduledNotice = scheduledEntry
        ? {
            icon: 'schedule_send',
            text: t('notice.scheduled', {
              subject: scheduledEntry.subject,
              when: whenLong(scheduledEntry.send_at_epoch),
            }),
            actions: [
              { label: t('action.cancelSend'), primary: true, do: async () => {
                try {
                  const draft = await call('outbox_cancel_scheduled', { id: scheduledEntry.id });
                  flash(draft !== null ? t('toast.sendCancelled') : t('error.cancelLater'));
                } catch (err) {
                  flash(t('error.cancelSend', { err }));
                }
                probeDrafts();
                probeSends();
              } },
            ],
          }
        : null;
      const problem = state.entries.find(
        (e) => e.state === 'interrupted' || e.state === 'rejected',
      );
      if (!problem) {
        sendNotice = null;
        return;
      }
      const noticeKey = problem.state === 'rejected' ? 'notice.sendRefused' : 'notice.sendInterrupted';
      sendNotice = {
        alert: true,
        icon: 'error',
        text: t(noticeKey, {
          subject: problem.subject,
          error: problem.error ? ` : ${problem.error}` : '',
        }),
        actions: [
          { label: t('action.resend'), primary: true, do: async () => {
            await call('outbox_requeue', { id: problem.id }).catch((err) => flash(t('error.resend', { err })));
            await call('flush_outbox').catch(() => {});
            probeSends();
          } },
          { label: t('action.discard'), do: async () => {
            await call('outbox_delete', { id: problem.id }).catch((err) => flash(t('error.discard', { err })));
            probeSends();
          } },
        ],
      };
    } catch { /* the next probe will do */ }
  }

  // 2. Signed update (ADR 0013): a check at startup,
  //    silently — offline, no notice, no noise.
  async function checkUpdate() {
    let update;
    try {
      update = await call('update_check');
    } catch { return; }
    if (!update) return;
    offerUpdate(update);
  }
  // The banner (re)arms itself from KNOWN data — never a network
  // round trip: a rearm that failed would leave "Installing…" stuck
  // with no button at all (PLAN-SIGNATURE review).
  function offerUpdate(update) {
    updateNotice = {
      icon: 'system_update_alt',
      text: t('notice.update', { version: update.version }),
      actions: [
        { label: t('action.install'), primary: true, do: async () => {
          updateNotice.text = t('settings.installing');
          updateNotice.actions = [];
          try {
            // The application restarts on the new version: this call
            // doesn't return control on success. The version goes
            // along with it: we only install what the banner announced.
            await call('update_install', { version: update.version });
          } catch (err) {
            offerUpdate(update);
            flash(t('error.update', { err }));
          }
        } },
        { label: t('action.later'), do: () => { updateNotice = null; } },
      ],
    };
  }

  // 3 and 4. Crash telemetry (ADR 0014): explicit opt-in, off by
  //    default, local reports — nothing is sent without the user.
  async function checkTelemetry() {
    try {
      const reports = await call('telemetry_pending');
      if (reports > 0) {
        crashNotice = {
          icon: 'report',
          text: t('notice.crash', { n: reports }),
          actions: [
            { label: t('action.openReports'), primary: true, do: async () => {
              await call('telemetry_open_folder').catch((err) => flash(t('error.opening', { err })));
            } },
            { label: t('action.dismiss'), do: () => { crashNotice = null; } },
          ],
        };
      }
      const consent = await call('telemetry_consent_get');
      if (consent === 'unset') {
        const decide = async (enabled) => {
          telemetryNotice = null;
          await call('telemetry_consent_set', { enabled })
            .catch((err) => flash(t('error.preference', { err })));
        };
        telemetryNotice = {
          icon: 'volunteer_activism',
          text: t('notice.telemetry'),
          actions: [
            { label: t('action.enable'), primary: true, do: () => decide(true) },
            { label: t('action.noThanks'), do: () => decide(false) },
          ],
        };
      }
    } catch { /* no telemetry available: no notice, no noise */ }
  }

  // --- Drafts in the list (PLAN-BROUILLONS) --------------------------
  // Probed like the rest — the port remains poll-based (R0-S5), no new
  // channel. The probe feeds the Drafts folder AND the mention on the
  // Inbox threads; the composer wakes it on every local gesture
  // (onbrouillon) so the list doesn't lag 10 s.
  let drafts = $state([]);
  // Freshness token: two probes can fly together (the gesture one and
  // the periodic one) — only the LAST one issued has the right to
  // serve, otherwise a stale response re-serves a deleted draft.
  let draftsProbe = 0;
  async function probeDrafts() {
    const mine = ++draftsProbe;
    try {
      const lines = await call('list_drafts');
      if (mine === draftsProbe) drafts = lines;
    } catch { /* the next probe will do */ }
  }
  function resumeDraft(draft) {
    compose.openDraft(draft);
  }

  // --- R1: the sync backbone (PLAN-RETRAIT-V1) --------
  // v1 triggered everything; v2 becomes autonomous — silent
  // reconnection at startup, then an AUTOMATIC cycle (D5: no button):
  // sync, outbox flush (the network may be back — golden rule), draft
  // reflection. v1 sequence kept identical.

  async function connect() {
    try {
      const report = await call('connect_accounts');
      // The addresses holding a session: Settings > Accounts derives
      // the per-account state from it — a dead token is SEEN and
      // repaired in place ("Reconnect", field finding 2026-08-20).
      connected = report.accounts.map((a) => a.email);
      if (report.problems.length > 0) {
        // Say WHICH one is missing and why — an absent badge with no
        // explanation leaves the user helpless (v1 lesson).
        connectionNotice = {
          alert: true,
          icon: 'link_off',
          text: t('notice.connection', { details: report.problems.join(' ; ') }),
          actions: [
            // Field 2026-08-20: "Retry" used to replay the SILENT
            // connection — doomed with a dead token. The useful door
            // is Settings > Accounts, where the state is visible and
            // "Reconnect" relaunches the consent flow (A63). The
            // notice stays displayed: it will fall away on its own at
            // reconnection (connecter() will reset it or clear it).
            { label: t('header.settings'), primary: true, do: () => {
              settings?.open();
            } },
            { label: t('action.dismiss'), do: () => { connectionNotice = null; } },
          ],
        };
      } else {
        connectionNotice = null;
      }
    } catch (err) {
      console.error('connect_accounts :', err);
    }
  }

  // $state: the status bar tells the cycle's story while it's running (E1).
  let syncing = $state(false);
  // The activity probe lives ONLY during the cycle: at the second,
  // purely in-memory on the shell side (atomics) — it costs nothing
  // to the loop and nothing at rest.
  async function probeActivity() {
    try {
      activity = await call('sync_activity');
    } catch { /* the next probe will do */ }
  }
  // P0 (PLAN-SYNCHRO): the cycle watchdog. The mail-imap socket
  // timeouts finish off a network that has stalled; the watchdog
  // covers what they don't see. 5 min: above the longest legitimate
  // silence measured — the full sync in the field advanced in
  // batches of ~75 s, and each batch moves the progress
  // (`sync.local`), hence the signature.
  const STALL_MAX_MS = 5 * 60 * 1000;
  // Sync cadences (PLAN-RETOURS-2, ADR 0021). The FULL cycle
  // (inventory + folder sweep + threads + drafts) is expensive on an
  // account with many folders; since IDLE (ADR 0018) keeps INBOX
  // real-time, it runs every 30 min. The LIGHT pass (STATUS INBOX
  // only) runs every 5 min as a net — if an IDLE watcher dropped
  // without reconnecting, INBOX still stays fresh to within 5 min
  // regardless.
  const FULL_CYCLE_MS = 30 * 60 * 1000;
  const LIGHT_PASS_MS = 5 * 60 * 1000;
  // The token forbids the LATE end of a cycle declared dead from
  // touching the state of a cycle restarted since.
  let cycleToken = 0;
  async function runSyncCycle() {
    if (syncing) return; // reentrancy forbidden: one cycle at a time
    // P0-bis: offline, an automatic cycle would only leave to stall on
    // timeouts — the bar already says "offline", and the network's
    // return will trigger a poll. The manual gesture, though, forces
    // it (see `poll`): the click is an order.
    if (!online) return;
    syncing = true;
    const token = ++cycleToken;
    probeActivity();
    // A cycle where NEITHER the activity NOR the progress moves for
    // 5 min is declared dead: guard rearmed, failure shown. The
    // watchdog kills nothing (an in-flight command isn't cancelled) —
    // it returns control; it's the socket timeout that finishes off
    // the frozen thread.
    let signature = '';
    let lastMove = Date.now();
    // P1: INBOX mail shows PER ACCOUNT, as soon as the cycle counter
    // moves — the list no longer waits for the end of the full cycle.
    // Read from the existing probe: the port remains poll-based
    // (R0-S5), no event channel.
    let mailSeen = 0;
    const monitor = async () => {
      await probeActivity();
      if (activity && activity.mail > mailSeen) {
        mailSeen = activity.mail;
        list?.reload();
        loadNav();
      }
      const trace = JSON.stringify([activity, sync?.local]);
      if (trace !== signature) {
        signature = trace;
        lastMove = Date.now();
        return;
      }
      if (Date.now() - lastMove < STALL_MAX_MS) return;
      clearInterval(probe);
      cycleToken += 1;
      syncFailure = true;
      activity = null;
      syncing = false;
      console.error('sync_inbox: no movement for 5 min, cycle declared dead (P0)');
    };
    const probe = setInterval(monitor, 1000);
    try {
      const report = await call('sync_inbox');
      if (token !== cycleToken) return; // declared dead in the meantime: too late
      failedUpdates(report);
      // The network may be back: the outbox tries its luck again,
      // then the drafts are reflected (push + purge).
      await call('flush_outbox').catch((err) => console.error('flush_outbox :', err));
      await call('sync_drafts').catch(() => { /* offline: the next cycle will retry */ });
      probeSends();
      loadNav();
      probeDrafts();
      if (report.fetched > 0 || report.deleted > 0) {
        list?.reload();
        backfillBodies();
      }
    } catch (err) {
      if (token === cycleToken) syncFailure = true;
      console.error('sync_inbox :', err);
    } finally {
      clearInterval(probe);
      if (token === cycleToken) {
        activity = null;
        syncing = false;
        // The timestamp was just set by the shell: reread it right
        // away, without waiting for the 5 s probe.
        probeSync();
      }
    }
  }

  // E3: the manual gesture (D5 reopened) — the light pass: STATUS
  // INBOX per account, polls only if it moved (E2a), then the outbox
  // tries its luck again (the network may be back — that's often WHY
  // we click). Response in seconds, held by E2a's gate; every command
  // is bounded by the P0 timeouts, the pass always ends — no
  // dedicated watchdog.
  // `force`: the click is an ORDER — it bypasses the per-account
  // backoff (anti-hammering, shell); sleep-wake, on the other hand,
  // respects it.
  async function poll(force) {
    if (syncing) return; // the cycle is already working — button inhibited
    // Offline, only the manual gesture (click, `force`) still tries —
    // sleep-wake and the network's return wait to be online.
    if (!force && !online) return;
    syncing = true;
    const token = ++cycleToken;
    probeActivity();
    const probe = setInterval(probeActivity, 1000);
    try {
      const report = await call('sync_inbox_light', { force: force === true });
      if (token !== cycleToken) return;
      failedUpdates(report);
      await call('flush_outbox').catch((err) => console.error('flush_outbox :', err));
      probeSends();
      loadNav();
      if (report.fetched > 0 || report.deleted > 0) {
        list?.reload();
        backfillBodies();
      }
    } catch (err) {
      if (token === cycleToken) syncFailure = true;
      console.error('sync_inbox_light :', err);
    } finally {
      clearInterval(probe);
      if (token === cycleToken) {
        activity = null;
        syncing = false;
        probeSync();
      }
    }
  }

  // Startup, in the order that protects: migration first (nothing
  // touches the database before), then the loops — and the one-off
  // checks.
  onMount(async () => {
    const dbReady = await migrationModal.ensure();
    // The language detected on first launch is set HERE, not before:
    // `lang_set` opens the database — before the modal, it would
    // silently pay for the adoption (ADR 0012, A41). And only if the
    // migration probe has ANSWERED: otherwise this optional write
    // would itself be the first full opening. Awaited, not fired: on
    // first launch it's the one that creates the schema — serialized
    // before the fleet of probes, as when it lived before mount.
    if (dbReady) await setDetectedLanguage();
    ready = true;
    // THE LIST FIRST (PLAN-DEMARRAGE, E2). `prete = true` doesn't
    // paint right away: Svelte schedules the flush as a microtask.
    // Without this `tick`, the ten calls that follow leave BEFORE
    // `<Liste>` is mounted, and its first page ends up TWELFTH in
    // emission order — behind seven probes that take the global
    // lock. Measured in the field on 2026-08-26, cold run: the page
    // was emitted at 89.6 ms and served at 440.6 ms, for 28 ms of
    // clean work.
    //
    // Returning control to the flush is enough: the list asks for its
    // page FIRST, alone, so it takes the lock uncontested. We DEFER
    // nothing — `loadNav` stays right behind, and that's
    // deliberate: its response carries the mailbox block of every row
    // (A80), delaying it would repaint every visible row.
    await tick();
    loadNav();
    // Organized mode is reread AFTER the first page of the list
    // (never an await before it — PLAN-DEMARRAGE E2 lesson): the nav
    // recomposes on arrival, a PK read.
    restoreOrganizedMode().then(() => {
      // RETOURS-14 (review, race MEASURED on the e2e fixture: page 0
      // served at ~85 ms, mode reread at ~105 ms): the list's last
      // pump then leaves with the mode OFF — the section seam of the
      // Organized Inbox is never requested, and the organized badges
      // missed the first snapshot. The reread therefore RE-SERVES the
      // views and the nav; outside the mode, nothing.
      if (organizedMode()) {
        reloadViews();
        loadNav();
      }
    });
    loadMarkers();
    loadNames();
    // PLAN-AUDIT-V2 E10: ONE resting probe (`ui_state`: nav, sync,
    // sends) every 5 s — three commands, three database openings per
    // 10 s before. Drafts keep their own probe (a list).
    probeState();
    setInterval(probeState, 5000);
    // "N minutes ago" ages on its own: 30 s is enough for a minute's
    // granularity.
    setInterval(() => (now = Date.now()), 30000);
    setTimeout(backfillPreviews, 1500);
    setTimeout(backfillBodies, 3000);
    checkUpdate();
    checkTelemetry();
    probeDrafts();
    setInterval(probeDrafts, 10000);
    // R1 — the sync cycle: AFTER the first renders (the list is
    // usable before, "envelopes first"); never blocking.
    (async () => {
      await connect();
      await runSyncCycle();
    })();
    // The full cycle every 30 min (IDLE holds INBOX, ADR 0018/0021),
    // and a light INBOX pass every 5 min as a net against a dropped
    // watcher. The light pass is cut short during a cycle
    // (`syncing`) — never two polls of the same INBOX.
    setInterval(runSyncCycle, FULL_CYCLE_MS);
    setInterval(() => poll(false), LIGHT_PASS_MS);
    // E3: sleep-wake — a tick running several minutes late signals a
    // sleep (clock jump: timers sleep with the machine), and that's
    // THE moment when the user looks at the screen. The light pass
    // leaves right away, without waiting for the next poll at 5 min.
    // No system API: the clock drift is enough.
    let lastTick = Date.now();
    setInterval(() => {
      const tic = Date.now();
      const lag = tic - lastTick;
      lastTick = tic;
      if (lag > 120000) poll(false);
    }, 15000);
    // P0-bis: the OS network state, live. `offline` switches the bar
    // instantly (no more waiting for the 120 s timeout); `online`
    // polls right away — the mail held back during the outage arrives
    // on return, the way Thunderbird does. Event, not polling: it's
    // the only way to be as prompt as the OS.
    window.addEventListener('offline', () => {
      online = false;
      // E4: IDLE watchers sleep offline — reconnecting in a loop
      // with no network would be pointless.
      call('network_state', { online: false }).catch(() => {});
    });
    window.addEventListener('online', () => {
      online = true;
      // The network's return clears the backoffs (shell side) and
      // wakes the watchers; the immediate poll covers the mail held
      // back.
      call('network_state', { online: true }).catch(() => {});
      poll(false);
      // R-D3 (E3): gestures played offline wait — the after-gesture
      // pass replays their actions and reconciles their echoes. With
      // no work, it costs no connection.
      passAfterGesture(null);
    });
    // Initial state: if the app starts offline, the watchers know
    // right away.
    call('network_state', { online: navigator.onLine }).catch(() => {});
  });

  function choose(what) {
    if ('category' in what) {
      category = what.category;
      tab = 'tous';
      // The Screener isn't a list: nobody will emit a total — the
      // status bar says the view's name, never a stale count from the
      // previous view. Same rule at the Feed in cards, while its own
      // probe hasn't answered yet.
      if (
        what.category === 'screener'
        || what.category === 'feed'
        || what.category === 'cleanup'
        // RETOURS-14 R6: the grouped Paper trail emits its own total.
        || what.category === 'paper_trail'
      ) listTotal = null;
    }
    if ('account' in what) account = what.account;
    search = '';
    selectedRow = null;
    closeThread();
  }
  // E5: the pile toggle — from the thread bar or a row's ⋯. Set
  // aside: the thread leaves its view, the pile keeps it;
  // "Resume"/"Done" returns it to where it came from.
  let pile = $state(null);
  let feed = $state(null);
  // RETOURS-14 R6: the Paper trail's grouped view — same reload
  // regime as the Feed.
  let paperTrail = $state(null);
  async function toggleAside(line, fromThread = false) {
    if (gestureOnEcho(line)) return;
    try {
      const aside = await call('toggle_set_aside', {
        accountId: line.account_id,
        mailbox: line.mailbox,
        uid: line.uid,
      });
      flash(t(aside ? 'toast.setAside' : 'toast.resumedPile'));
      // The store's token discipline (pattern of epinglerFil): the
      // bar's button follows the gesture — a "Resume" that didn't
      // change its label would set aside again on the next click (E5
      // review).
      if (thread.line && msgKey(thread.line) === msgKey(line)) thread.aside = aside;
      reloadViews();
      pile?.reload();
      loadNav();
      // From the reading surface: a thread just set aside has just
      // left its view — screen 03 returns to the mailbox, the pane
      // (Feed/Paper trail) closes (E5 review: it was showing a thread
      // that was gone, lying button included).
      if (fromThread && aside) {
        if (thread.frame === 'full') backToMailbox();
        else closeThread();
      }
    } catch (err) {
      flash(t('error.preference', { err }));
    }
  }
  // "Move to…" (E1): the ENTIRE sender changes destination — the
  // address is resolved from the envelope on the core side, the
  // toast states the gesture, the list re-serves (a Feed or Paper
  // trail view changes under the gesture).
  // RETOURS-14 R6 (review): route by ADDRESS — a Paper trail group's
  // ⋯ doesn't have a row at hand, it has the sender. Same door
  // (route_sender), same toasts, same re-serve.
  async function routeAddress(address, who, destination) {
    try {
      await call('route_sender', { address, destination, rule: null });
      if (destination === 'screened_out') {
        flash(t('toast.screenerNoBare', { who }));
      } else {
        flash(t('toast.senderMoved', { mailbox: t(mailboxLabelKey(destination)) }));
      }
      reloadViews();
      loadNav();
    } catch (err) {
      flash(t('error.preference', { err }));
    }
  }
  async function moveSender(line, destination) {
    if (gestureOnEcho(line)) return;
    try {
      const address = await call('route_sender_from', {
        accountId: line.account_id,
        mailbox: line.mailbox,
        uid: line.uid,
        destination,
      });
      if (address === null) {
        flash(t('error.noAddress'));
        return;
      }
      // E4: "Screen out this sender" — the bare No, from a row's ⋯;
      // the choice replays into the Screener's history.
      if (destination === 'screened_out') {
        flash(t('toast.screenerNoBare', { who: line.sender }));
      } else {
        flash(t('toast.senderMoved', { mailbox: t(mailboxLabelKey(destination)) }));
      }
      reloadViews();
      loadNav();
    } catch (err) {
      flash(t('error.preference', { err }));
    }
  }
  // The "Organized" toggle (PLAN-MODE-ORGANISE E1). Leaving the mode
  // from an organized view returns to the Inbox — never an orphaned
  // view that the classic nav no longer knows how to name.
  async function toggleOrganized() {
    try {
      const active = await toggleOrganizedMode();
      if (
        !active
        && (category === 'feed' || category === 'paper_trail'
          || category === 'screener' || category === 'cleanup')
      ) {
        choose({ category: 'inbox' });
      } else {
        // RETOURS-14 R2 (D3): the Organized Inbox no longer has tabs
        // — an "Unread" filter inherited from Classic would stay
        // stuck there with no way out.
        if (active) tab = 'tous';
        // The displayed Inbox changes CONTENT with the mode (E2:
        // retention and routing) — the list re-serves, as after a
        // "Move to…"; otherwise the screen keeps the other mode's
        // page until the next round trip.
        list?.reload();
      }
      loadNav();
    } catch (err) {
      flash(t('error.preference', { err }));
    }
  }
  function onTab(id) {
    if (id === 'drafts') {
      category = 'drafts';
      return;
    }
    if (category === 'drafts') category = 'inbox';
    // RETOURS-14 R2 (D3, review): the Organized Inbox has no tabs —
    // an "Unread" set from Drafts (or during a search) would stay
    // there with no way out.
    tab = organizedMode() && category === 'inbox' ? 'tous' : id;
    selectedRow = null;
    closeThread();
  }

  // --- Shortcuts (D3): c / r / f / e / Delete / "/" / Escape --------
  // In an input field, letters go back to being letters — only Escape
  // keeps a meaning (leave the field, without discarding the draft).
  // s (star) and v (move) follow D2: cut at the toggle.
  function onKey(event) {
    if (event.ctrlKey || event.metaKey || event.altKey) return;
    // The composer's rich editor (PLAN-COMPOSITION-HTML) is a
    // contenteditable: neither an input nor a textarea, but an INPUT
    // FIELD all the same — without `isContentEditable`, typing "c",
    // "e" or Delete in the body triggered the global shortcuts
    // (Delete used to delete the selected conversation while typing —
    // seen at e2e).
    const typing = event.target instanceof HTMLInputElement
      || event.target instanceof HTMLTextAreaElement
      || event.target.isContentEditable;
    if (typing) {
      if (event.key === 'Escape') {
        if (event.target === searchField) search = '';
        event.target.blur();
      }
      return;
    }
    switch (event.key) {
      case 'c':
        write();
        break;
      case 'r':
        if (selectedRow) reply(selectedRow);
        break;
      case 'f':
        if (selectedRow) forward(selectedRow);
        break;
      // Field 2026-08-27 (R1-8): when a batch is checked, e/Delete
      // act on THE BATCH (the same path as the bar — acting freezes
      // the bar and clears the selection); with no batch, single-item
      // triage A38.
      case 'e':
        if (list?.selecting()) list.act('archive');
        else if (selectedRow) advanceAfter(selectedRow, archive);
        break;
      case 'Delete':
        if (list?.selecting()) list.act('delete');
        else if (selectedRow) advanceAfter(selectedRow, deleteConversation);
        break;
      case '/':
        searchField?.focus();
        break;
      case 'Escape':
        if (compose?.isOpen()) compose.close();
        else if (settings?.isOpen()) settings.close();
        else if (conversation?.isOpen()) backToMailbox();
        else if (drawerOpen) drawerOpen = false;
        else if (search) search = '';
        else return;
        break;
      default:
        return;
    }
    event.preventDefault();
  }

  // E3 (PLAN-REACTIVITE): a local echo is a copy mid-sync — a gesture
  // on it waits for reconciliation (a window of a few seconds), and
  // the toast says so instead of a silent failure. The row is
  // recognized by its synthetic mailbox.
  function gestureOnEcho(line) {
    if (!isEcho(line)) return false;
    flash(t('toast.echoPending'));
    return true;
  }
  // E3: reconciliation runs behind the gesture — the server follows,
  // the echo fades under its real row (invisible to the eye, E1). The
  // report says it all: incidents in the console, mail/sweep → list
  // re-served. `accountId: null` = all the accounts that have work
  // (the trigger for the return online, R-D3).
  function passAfterGesture(accountId) {
    call('sync_after_gesture', { accountId })
      .then((report) => {
        for (const incident of report.errors) console.error('sync_after_gesture :', incident);
        if (report.fetched > 0 || report.deleted > 0 || report.reconciled > 0 || report.swept > 0) {
          loadNav();
          list?.reload();
        }
      })
      .catch((err) => console.error('sync_after_gesture :', err));
  }

  function openConversation(line) {
    // D4 (UI v3): frame exclusivity lives in the store (fil.cadre) —
    // expanding is a size change, never a reload.
    conversation.open(line);
  }
  // R4 (PLAN-RETOURS-7): pin/unpin THE open conversation. The core
  // returns the new state (the thread follows, even if the head has
  // moved), the list re-serves — the pinned section and the flow move
  // together (D5: never the same row twice). The gesture is only
  // offered in the Inbox (D4) and NEVER on a search: a result can
  // live outside the Inbox — the pin would be invisible.
  const pinnable = $derived(category === 'inbox' && resultCount === null);
  // A80/D7, field verdict of 2026-08-25 (point 12): the reading pane
  // only states the mailbox where the LIST states it — same rule, a
  // single expression (lib/mailbox.js). The App is the only one holding
  // both halves: the chosen account and the search state.
  const mixedAccounts = $derived(mixedView(account, resultCount !== null));
  async function pinThread(line) {
    if (gestureOnEcho(line)) return;
    try {
      const state = await call('toggle_pin', {
        accountId: line.account_id,
        mailbox: line.mailbox,
        uid: line.uid,
      });
      // The store's token discipline (review 2026-08-21): the
      // response only dresses the button if the thread STILL shows
      // this conversation — otherwise another row's state would land
      // here.
      if (thread.line && msgKey(thread.line) === msgKey(line)) thread.pin = state;
      list?.reload();
    } catch (err) {
      console.error('toggle_pin :', err);
    }
  }
  function backToMailbox() {
    shrinkThread(panes === 3 && !organizedInbox);
    // E4: on return from screen 03, a READ thread leaves "New for
    // you" — the list and the seam re-serve, the nav follows.
    if (organizedInbox) {
      list?.reload();
      pile?.reload();
      loadNav();
    }
  }

  function write() {
    compose.open('new');
  }
  function reply(line) {
    if (gestureOnEcho(line)) return;
    compose.open('reply', line);
  }
  function replyAll(line) {
    if (gestureOnEcho(line)) return;
    compose.open('reply_all', line);
  }
  function forward(line) {
    if (gestureOnEcho(line)) return;
    compose.open('forward', line);
  }
  // After a flush: the counters (Sent) may have moved.
  function afterSend() {
    loadNav();
  }
  // E2 (PLAN-REACTIVITE): the targeted Sent poll reports mail — the
  // copy is IN the database, the list re-serves right away (the exact
  // case from finding 0.1.5: you send, you look at Sent). E1 makes
  // the re-serve invisible; the generation probe, bumped by the same
  // poll, will pass back over it for free.
  function afterMailSent() {
    loadNav();
    list?.reload();
  }
  // Simple door (D4): the account is added, the nav reloads RIGHT
  // AWAY (local read — never behind the network, review), and the
  // first sync leaves as soon as the reconnection has returned its
  // report. `connected` only fills in on the return of
  // connect_accounts: without the callback, Settings said
  // "Disconnected" for an account that had just been connected, until
  // restart (PLAN-RETOURS-12 R1 — the reason for the "Reconnect"
  // gesture).
  async function accountAdded() {
    flash(t('toast.accountAdded'));
    loadNav();
    await connect();
    runSyncCycle();
  }
  // The counterpart of removal: the account's mail has left the
  // database, so everything that could show it collapses — nav
  // filter, selection, reading pane — before reloading nav and list.
  // At zero accounts, screen 01 comes back on its own
  // (navPrete && comptes.length === 0).
  function accountRemoved(id) {
    flash(t('toast.accountRemoved'));
    // The account's marker dies with it (the shell purges its prefs)
    // — the local table follows suit, otherwise a reused SQLite id
    // would inherit the badge (review 2026-08-22).
    patchMarker(id, null);
    patchName(id, null);
    if (account === id) account = null;
    selectedRow = null;
    closeThread();
    loadNav();
    list?.reload();
  }

  // RETOURS-14 (review): ONE re-serve of the views — the list AND
  // the organized-mode scenes. Gestures no longer target the bare
  // `list`: the grouped Paper trail opens its threads in the pane,
  // the list can be UNMOUNTED at the moment of the gesture (TypeError
  // swallowed by the catch — nav and the after-gesture pass used to
  // skip along with it).
  // PLAN-AUDIT-V2 E10: a gesture made three re-serves in a burst
  // (gesture, reread, after-gesture pass) — coalesced at 50 ms.
  // RISING edge: the first request leaves right away (the old order,
  // which the keyboard triage A38 assumes), the following ones
  // within 50 ms are absorbed — a falling edge made "archive by
  // shortcut from screen 03" flaky (E10 review).
  let reloadRecent = false;
  function reloadViews() {
    if (reloadRecent) return;
    reloadRecent = true;
    setTimeout(() => {
      reloadRecent = false;
    }, 50);
    list?.reload();
    feed?.reload();
    paperTrail?.reload();
  }

  async function probeState() {
    try {
      const state = await call('ui_state');
      loadNav(state.nav);
      probeSync(state.sync);
      probeSends(state.outbox);
    } catch { /* offline or core busy: the next probe will do */ }
  }

  function markSeen(line) {
    if (!(line.thread_unseen > 0)) return;
    call('mark_seen', {
      accountId: line.account_id,
      mailbox: line.mailbox,
      uid: line.uid,
      seen: true,
    })
      .then(() => {
        list?.markRead(line);
        loadNav();
      })
      .catch((err) => console.error('mark_seen :', err));
  }
  // E4: the ORGANIZED Inbox has no reading pane — a click opens
  // screen 03 (the existing overlay), regardless of the panes setting.
  const organizedInbox = $derived(organizedMode() && category === 'inbox');
  // E5bis: the Feed in CARDS — a reading scene, not a list; like the
  // Screener and the Organized Inbox, no pane.
  const feedCards = $derived(organizedMode() && category === 'feed');
  // RETOURS-14 R6 (D7): the Paper trail grouped by sender — the
  // organized view only; the reading pane REMAINS the reader.
  const paperTrailGroup = $derived(organizedMode() && category === 'paper_trail');
  // THE scenes with no reading pane — ONE predicate (review
  // 2026-08-30: the enumeration used to live duplicated across the
  // Reading/handle guards; the next full-scene section is added
  // HERE, not in N places).
  const sceneWithoutReading = $derived(
    category === 'screener' || category === 'cleanup' || organizedInbox || feedCards,
  );
  function onSelection(line) {
    selectedRow = line;
    // V-D2: in two panes, opening IS screen 03 — which knows how to
    // serve a message with no thread (echo included). Read-marking
    // doesn't change: only the destination surface changes.
    if (panes === 3 && !organizedInbox) reading.open(line);
    else conversation.open(line);
    markSeen(line);
  }

  // archive/delete state their success: keyboard triage only advances
  // on a COMPLETED gesture — never on a deferred echo nor a failure.
  async function archive(line) {
    if (gestureOnEcho(line)) return false;
    try {
      await call('archive_message', {
        accountId: line.account_id,
        mailbox: line.mailbox,
        uid: line.uid,
      });
      flash(t('toast.archived'));
      // R1 (RETOURS-10): the row that's gone also leaves the
      // multi-select — the bar never counts a row that no longer
      // exists.
      list?.uncheck(line);
      closeThread();
      // The destination echo is ALREADY in the database (same
      // transaction as the gesture, E3): the re-serve shows it in
      // Archive in < 1 s — the server follows through the pass,
      // silently.
      reloadViews();
      loadNav();
      passAfterGesture(line.account_id);
      return true;
    } catch (err) {
      console.error('archive_message :', err);
      return false;
    }
  }
  // Field R8' (2026-08-23): "Delete" lives PER message — the target
  // is a thread message (or the row of a single-message thread). The
  // open thread stays in place if it still has messages left;
  // returns TRUE when the thread has closed (screen 03 then returns
  // to the mailbox).
  async function deleteConversation(target) {
    if (gestureOnEcho(target)) return false;
    try {
      await call('delete_message', {
        accountId: target.account_id,
        mailbox: target.mailbox,
        uid: target.uid,
      });
      const remainingOnes = removeMessage(target);
      const closed = remainingOnes <= 0;
      list?.uncheck(target);
      if (closed) closeThread();
      flash(t(closed ? 'toast.deleted' : 'toast.messageDeleted'));
      // Same mechanics as archive: the echo is in the database, the
      // Trash shows it right away, the pass reconciles behind it.
      reloadViews();
      loadNav();
      passAfterGesture(target.account_id);
      // TRUE = completed gesture (the avancerApres contract) — screen
      // 03 looks at fil.ligne to know whether the thread has closed.
      return true;
    } catch (err) {
      console.error('delete_message :', err);
      return false;
    }
  }
  // R2 (PLAN-RETOURS-3): report junk / the opposite. Same optimistic
  // mechanics as archive/delete — local disappearance, logged MoveTo
  // action, the server follows. The thread closes, the list and the
  // nav refresh, the pass reconciles behind it.
  async function reportSpam(line) {
    if (gestureOnEcho(line)) return false;
    try {
      await call('report_spam', {
        accountId: line.account_id,
        mailbox: line.mailbox,
        uid: line.uid,
      });
      flash(t('toast.spamReported'));
      list?.uncheck(line);
      closeThread();
      reloadViews();
      loadNav();
      passAfterGesture(line.account_id);
      return true;
    } catch (err) {
      // The only expected failure: the account has no junk folder.
      console.error('report_spam :', err);
      flash(t('error.spamFailed'));
      return false;
    }
  }
  async function markLegitimate(line) {
    if (gestureOnEcho(line)) return false;
    try {
      await call('mark_not_spam', {
        accountId: line.account_id,
        mailbox: line.mailbox,
        uid: line.uid,
      });
      flash(t('toast.notSpam'));
      list?.uncheck(line);
      closeThread();
      reloadViews();
      loadNav();
      passAfterGesture(line.account_id);
      return true;
    } catch (err) {
      console.error('mark_not_spam :', err);
      return false;
    }
  }

  // PLAN-RETOURS-10 R1: the BULK gestures of the selection bar.
  // PLAN-AUDIT-V2 E6: ONE call to the core (`act_on_group`), ONE
  // transaction, all or nothing (D6) — before, the unit commands used
  // to replay in sequence (250 + 50 IPC for 50 conversations, the bar
  // frozen). Then ONE toast, ONE re-serve, ONE pass per account.
  // The ENTIRE thread of a row goes (D6 from RETOURS-10): the core
  // expands it itself, `thread_messages` is no longer requested.
  const GROUP_GESTURES = {
    archive: { toast: 'toast.groupArchived', closed: true },
    delete: { toast: 'toast.groupDeleted', closed: true },
    spam: { toast: 'toast.groupSpam', closed: true },
    not_spam: { toast: 'toast.groupNotSpam', closed: true },
    read: { toast: 'toast.groupRead' },
    unread: { toast: 'toast.groupUnread' },
  };
  // The target of a per-message command — the fifth site that used
  // to spell out this triple by hand (review).
  const targetFrom = (l) => ({ accountId: l.account_id, mailbox: l.mailbox, uid: l.uid });
  async function group(action, lines) {
    const gesture = GROUP_GESTURES[action];
    if (!gesture) {
      console.error(`group: unknown action “${action}”`);
      return;
    }
    // The echoes are set aside by the PURE predicate (estEcho —
    // gesteSurEcho would flash a toast PER echo, instantly
    // overwritten) and stay in the DENOMINATOR: an amputated batch is
    // stated, never a facade success.
    const targets = lines.filter((l) => !isEcho(l));
    const total = lines.length;
    if (targets.length === 0) {
      if (total > 0) flash(t('toast.echoPending'));
      return;
    }
    let done = 0;
    let succeeded = [];
    let spamRefused = false;
    try {
      const report = await call('act_on_group', {
        targets: targets.map((l) => ({ ...targetFrom(l), threadId: l.thread_id ?? null })),
        action,
      });
      done = report.done;
      succeeded = targets;
    } catch (err) {
      // All or nothing (D6): a refusal leaves the batch intact — the
      // only EXPECTED failure is the absence of a junk folder.
      if (action === 'spam') spamRefused = true;
      console.error('act_on_group :', err);
    }
    flash(
      done === total
        ? t(gesture.toast, { n: done })
        : action === 'spam' && done === 0 && spamRefused
          ? t('error.spamFailed')
          : t('error.groupPartial', { done, total }),
    );
    // The local echo of the read-marking, as on the unit path
    // (marquerVue): the weight drops instantly, the re-serve tells
    // the truth behind it.
    if (action === 'read') for (const l of succeeded) list?.markRead(l);
    // The open thread only closes if ITS gesture SUCCEEDED — a
    // failure leaves it in place, as on the unit path.
    if (gesture.closed && selectedRow
      && succeeded.some((l) => msgKey(l) === msgKey(selectedRow))) {
      closeThread();
    }
    reloadViews();
    loadNav();
    if (gesture.closed) {
      for (const id of new Set(succeeded.map((l) => l.account_id))) passAfterGesture(id);
    }
  }

  // Keyboard triage chains (A38): after e/Delete, the row BELOW
  // becomes the selection — captured BEFORE the gesture (the rows
  // shift on the re-serve); last row: nothing advances. In three
  // panes it opens its pane like a click would (viewed, marked read);
  // in 2/1 panes it only lights up — screen 03 never imposes itself
  // on its own. Open conversation: the gesture alone, as before. The
  // mouse gesture (pane buttons) doesn't move the selection.
  async function advanceAfter(line, gesture) {
    // Field finding (2026-08-15): the click leaves focus on a row;
    // the key switches the browser into keyboard mode and the
    // :focus-visible ring would pop up on this RECYCLED node (rows
    // keyed by index — it's already showing another conversation):
    // meaningless accent strokes. The selection (border) states the
    // position — the shortcut removes focus from the row.
    if (document.activeElement instanceof HTMLElement) document.activeElement.blur();
    const next = conversation?.isOpen() ? null : (list?.next(line) ?? null);
    if (!(await gesture(line)) || !next) return;
    list?.select(next);
    selectedRow = next;
    // E5 review: in the ORGANIZED Inbox the pane doesn't exist —
    // opening and marking as read a conversation never shown would
    // lie to the "New for you" section (the disc would leave without
    // a read).
    if (panes === 3 && !organizedInbox) {
      reading?.open(next);
      markSeen(next);
    }
  }

  export function api() {
    return { list, reading };
  }
  export function markStartup() {
    const l = list.snapshot();
    perf = t('status.perf', { total: l.total, ms: l.firstPageMs.toFixed(1) });
    startup = String(Math.round(performance.now()));
  }
  let perf = $state(t('status.startup'));
  let startup = $state('');
</script>

<!-- When the window shrinks, the LIST yields: widths set on a large
     screen must never crush the thread under its reserve nor push a
     handle off screen (review 2026-08-16, same root cause as the
     handle cap). -->
<svelte:window onkeydown={onKey}
               onresize={() => {
                 if (panes === 3) {
                   setWidth('list', listWidth, handleCap('list'));
                   persistWidths();
                 }
               }} />

<div class="ecran">
  <header class="entete" data-testid="entete">
    {#if panes === 1}
      <!-- One-pane mode (PLAN-VOLETS E2): the nav lives in a drawer,
           the button opens it — 32 px, the header-button grammar. -->
      <button type="button" class="btn-tiroir" data-testid="btn-tiroir"
              aria-label={t('nav.openDrawer')} aria-expanded={drawerOpen}
              onclick={() => (drawerOpen = true)}>
        <Icon name="menu" /></button>
    {/if}
    <!-- V1/V11: the brand AS A GLYPH — the envelope in the current
         ink, --marque flap, in front of the word "Wind" (18 px). The
         hitofude stroke is dead (V2); the frozen tile stays for OS
         contexts, onboarding, migration and "About". 28 px since
         PLAN-RETOURS-12 (D2) — 24 px (RETOURS-10) stayed discreet,
         20 px got lost in the 52 px header. -->
    <span class="marque" class:marque--libre={panes === 1}><Brand size={28} />Wind</span>
    <span class="recherche" data-testid="recherche">
      <Icon name="search" />
      <input type="text" bind:this={searchField} bind:value={search}
             data-testid="champ-recherche" aria-label={t('header.search')}
             placeholder={t('header.searchHint')}>
      {#if search}
        <!-- Field verdict (Annex A): clear the search in ONE click. -->
        <button type="button" class="vider" data-testid="vider-recherche"
                aria-label={t('header.clearSearch')}
                onclick={() => { search = ''; searchField?.focus(); }}>
          <Icon name="close" /></button>
      {/if}</span>
    <!-- PLAN-MODE-ORGANISE E1: the "Organized" toggle, to the right
         of the search (form settled at the prototype) — pill + disc,
         the only two legitimate round shapes (V14). -->
    <button type="button" class="organise" data-testid="mode-organise"
            role="switch" aria-checked={organizedMode()}
            onclick={toggleOrganized}>
      <span class="piste" aria-hidden="true"><span class="disque"></span></span>{t('header.organized')}</button>
    <button type="button" class="principal" data-testid="ecrire" onclick={write}>
      <Icon name="edit_square" />{t('header.compose')}</button>
    <!-- The beta feedback (RETOURS-11 R3): with no account, no
         button — the message goes out by email from the workstation's
         first account. -->
    {#if accounts.length > 0}
      <button type="button" data-testid="feedback" onclick={() => back.open()}>
        <Icon name="feedback" />{t('header.feedback')}</button>
    {/if}
    <button type="button" data-testid="reglages" onclick={() => settings.open()}>
      <Icon name="settings" />{t('header.settings')}</button>
  </header>

  <NoticeSlot {notice} />

  {#if ready}
    <div class="colonnes" class:colonnes--2={panes === 2}
         class:colonnes--1={panes === 1}
         class:colonnes--organise={organizedInbox}
         style="--l-nav:{lNav}px; --l-liste:{listWidth}px">
      {#if panes !== 1}
        <Nav {accounts} {markers} {names} {category} {account}
             organized={organizedMode()} screener={screenerTotal}
             feed={feedTotal} paperTrail={paperTrailTotal} onchoose={choose} />
      {/if}
      {#if category === 'cleanup'}
        <!-- Pane B: the Spring cleaning — same scene regime as the
             Screener (column, no reading pane). -->
        <div class="cadre-portier">
          <Cleanup onflash={flash} onchange={loadNav} />
        </div>
      {:else if category === 'screener'}
        <!-- E2: the Screener isn't a list — one row per waiting
             SENDER, a yes/no and nothing else. Its scene takes UP ALL
             the room to the right of the nav (centered column, like
             screen 03) — the reading pane has nothing to read
             there. -->
        <div class="cadre-portier">
          <Screener onflash={flash} onchange={loadNav} />
        </div>
      {:else if feedCards}
        <!-- E5bis: the Feed in cards — the letters already opened,
             the whole scene (CE decision of 2026-08-30). -->
        <div class="cadre-portier">
          <Feed bind:this={feed} {account}
                   onmove={moveSender} onsetaside={toggleAside}
                   ontotal={(t) => (listTotal = t)} />
        </div>
      {:else if paperTrailGroup}
        <!-- R6: the grouped Paper trail takes the list column — the
             reading pane stays on the right, opening goes through the
             list's path (surSelection). -->
        <PaperTrail bind:this={paperTrail} {account}
                  onopen={onSelection} onroute={routeAddress}
                  ontotal={(t) => (listTotal = t)} />
      {:else}
        <List bind:this={list} {category} {account} {accounts} {markers} {names} {tab} {search}
               {drafts} onresume={resumeDraft}
               onselect={onSelection} ontab={onTab} ongroup={group}
               ontotal={(t) => (listTotal = t)}
               organized={organizedMode()} onmove={moveSender}
               onsetaside={toggleAside}
               onresults={(n, total) => { resultCount = n; totalCount = total; }} onflash={flash} />
      {/if}
      {#if panes === 3 && !sceneWithoutReading}
        <Reading bind:this={reading} {drafts} {markers} {names} {accounts} mixed={mixedAccounts} onresume={resumeDraft}
                 onarchive={archive} ondelete={deleteConversation}
                 onconversation={openConversation}
                 onreply={reply} onreplyall={replyAll}
                 onforward={forward}
                 onspam={reportSpam} onnotspam={markLegitimate}
                 isJunk={category === 'junk'} onflash={flash}
                 organized={organizedMode()} onmove={moveSender}
                 onsetaside={(l) => toggleAside(l, true)}
                 {pinnable} onpin={pinThread} />
      {/if}
      <!-- The handles (R3): placed ON the grid boundaries, out of
           flow — the grid gains no column. The ARIA pattern is the
           "window splitter": a focusable separator, aria-valuenow —
           the Svelte linter doesn't know it. ONE single template
           (review 2026-08-16): any hardening of the gesture applies
           to both boundaries by construction. -->
      {#snippet handle(pane, label, left)}
        <!-- svelte-ignore a11y_no_noninteractive_element_interactions, a11y_no_noninteractive_tabindex -->
        <div class="poignee" data-testid="poignee-{pane}" role="separator"
             aria-orientation="vertical" aria-label={label}
             tabindex="0" aria-valuemin={BOUNDS[pane][0]}
             aria-valuemax={BOUNDS[pane][1]} aria-valuenow={currentWidth(pane)}
             style="left:{left}px"
             onpointerdown={(e) => grabHandle(pane, e)}
             onpointermove={dragHandle} onpointerup={releaseHandle}
             onpointercancel={releaseHandle}
             onlostpointercapture={releaseHandle}
             ondblclick={() => defaultWidth(pane)}
             onkeydown={(e) => keyHandle(pane, e)}></div>
      {/snippet}
      {#if organizedInbox && !(thread.frame === 'full' && thread.line)}
        <!-- E5: the pile lives at the bottom right of the Organized
             Inbox (prototype) — fans out on click, full-screen table.
             It hides under screen 03 (E5 review: it used to float
             over the reading pane, z 20 against 1). -->
        <SetAsidePile bind:this={pile} onopen={openConversation} onflash={flash}
                       onchange={() => { list?.reload(); loadNav(); }} />
      {/if}
      {#if panes !== 1}
        {@render handle('nav', t('panes.handleNav'), lNav - 3)}
      {/if}
      {#if panes === 3 && !sceneWithoutReading}
        {@render handle('list', t('panes.handleList'), lNav + listWidth - 3)}
      {/if}
    </div>

    <div class="statut" data-testid="statut">
      <!-- V2: the disc / ring pair — the 9 px filled --marque disc
           says rest (`line.stroke`), the hollow ring of the same
           diameter says an action is running (`line.thread`). The
           hitofude stroke is dead. A52 holds: the % lives in the
           TEXT. -->
      <span class="texte">
        {#if line.alert}<span class="point-alerte" aria-hidden="true"></span>{/if}
        {#if line.thread}
          <span class="anneau" aria-hidden="true"></span>
        {:else if line.stroke}
          <span class="disque" aria-hidden="true"></span>
        {/if}
        <span data-testid="progression">{line.text}</span>
      </span>
      <span id="perf" data-testid="perf" data-startup={startup}>{perf}</span>
      <!-- E3: the gesture lives next to the information it refreshes
           (S-D1, variant A). Inhibited during a cycle (the glyph is
           spinning: the machine is already working); on failure, it
           becomes the lever closest to the outage. -->
      <!-- The button keeps its sync glyph, motionless (A36: the
           animation lives in the line's stroke, never here). -->
      <button type="button" class="btn-statut" data-testid="btn-releve"
              disabled={syncing} onclick={() => poll(true)}>
        <Icon name="sync" />
        {#if syncing}{t('action.syncing')}{:else if syncFailure || syncPartial}{t('action.retry')}{:else}{t('action.sync')}{/if}
      </button>
    </div>

    {#if panes === 1 && drawerOpen}
      <!-- The drawer (PLAN-VOLETS E2): geometry of the prototype
           validated at GO — 268 px, 60 px header (brand tile +
           close), the Nav reused AS IS. The scrim is a button: click
           closes it, so does the keyboard (A8). -->
      <button type="button" class="scrim-tiroir" data-testid="tiroir-scrim"
              aria-label={t('nav.closeDrawer')}
              onclick={() => (drawerOpen = false)}></button>
      <div class="tiroir" data-testid="tiroir" role="dialog" aria-modal="true"
           aria-label={t('nav.aria')}>
        <div class="tete-tiroir">
          <Brand size={28} />Wind
          <button type="button" class="btn-tiroir fermer-tiroir" data-testid="tiroir-fermer"
                  aria-label={t('nav.closeDrawer')}
                  onclick={() => (drawerOpen = false)}>
            <Icon name="close" /></button>
        </div>
        <Nav {accounts} {markers} {names} {category} {account}
             organized={organizedMode()} screener={screenerTotal}
             feed={feedTotal} paperTrail={paperTrailTotal} onchoose={chooseFromDrawer} />
      </div>
    {/if}

    <Conversation bind:this={conversation} {drafts} {markers} {names} {accounts} mixed={mixedAccounts}
                  onresume={resumeDraft} onback={backToMailbox}
                  onarchive={async (l) => { await archive(l); backToMailbox(); }}
                  ondelete={async (l) => {
                    // Thread closed (last message gone) OR gesture refused
                    // (echo pending — the toast said so): back to the
                    // mailbox, the old wiring. The thread only stays open
                    // if it still has messages left.
                    const succeeded = await deleteConversation(l);
                    if (!thread.line || !succeeded) backToMailbox();
                  }}
                  onreply={reply} onreplyall={replyAll}
                  onforward={forward}
                  onspam={async (l) => { await reportSpam(l); backToMailbox(); }}
                  onnotspam={async (l) => { await markLegitimate(l); backToMailbox(); }}
                  isJunk={category === 'junk'}
                  oncompose={write}
                  onflash={flash}
                  organized={organizedMode()} onmove={moveSender}
                  onsetaside={(l) => toggleAside(l, true)}
                  {pinnable} onpin={pinThread} />

    <!-- R2 (A75): the full journey (`onboardingToPlay`, first
         installation — it HOLDS through its four steps, accounts
         added or not); otherwise the original desk alone, at zero
         accounts, which fades on the first addition. -->
    {#if navReady && (onboardingToPlay || accounts.length === 0)}
      <Onboarding complete={onboardingToPlay} {accounts} onadd={accountAdded}
                  onfinish={() => (onboardingToPlay = false)} />
    {/if}

    <Compose bind:this={compose} {accounts} {account} {names}
                 onflash={flash} onsent={afterSend}
                 onmail={afterMailSent}
                 ondraft={probeDrafts} />
    <Feedback bind:this={back} {accounts} onflash={flash} />
    <Settings bind:this={settings} {accounts} {connected} {markers} {names}
              onmarker={patchMarker} onname={patchName} onadd={accountAdded}
              onremove={accountRemoved}
              onflash={flash}
              onrouting={() => { reloadViews(); loadNav(); }}
              onreconnect={async () => { await connect(); runSyncCycle(); }} />
  {/if}

  <MigrationModal bind:this={migrationModal} />

  <Toast message={toast} />
</div>

<style>
  .ecran {
    display:flex; flex-direction:column; height:100vh; position:relative;
    background:var(--bg); overflow:hidden;
  }
  /* A30: the header at the panel token, the search on white. UI v3, E4
     (CE verdict 2026-08-16): the template of the Classic mockup —
     52 px, 14/12 gutters, search capped at 520 px. */
  .entete {
    height:52px; flex:none; background:var(--bg);
    border-bottom:1px solid var(--border); display:flex;
    align-items:center; gap:12px; padding:0 14px;
  }
  .marque {
    font-size:18px; font-weight:600; width:212px; color:var(--ink);
    display:flex; align-items:center; gap:10px;
  }
  .recherche {
    flex:1; max-width:520px; height:32px; display:flex; align-items:center; gap:10px;
    padding:0 14px; font-size:13px; color:var(--ink2);
    background:var(--surface); border:1px solid var(--border);
    border-radius:var(--r-controle);
  }
  .recherche :global(.ic) { color:var(--ink2); }
  .recherche input {
    flex:1; font-size:13px; color:var(--ink); border:none; outline:none;
    background:transparent; min-width:0;
  }
  .recherche input::placeholder { color:var(--ink2); }
  /* The search is capped (520 px): the header controls hold the
     right side, as in the mockup template. */
  .entete [data-testid="ecrire"] { margin-left:auto; }
  /* PLAN-MODE-ORGANISE E1: the toggle — track pill (999px) and disc
     (50 %), the only two legitimate round shapes (V14). Active: the
     track takes the accent, the disc slides to the right. */
  .organise {
    display:inline-flex; align-items:center; gap:8px; flex:none;
    font-size:13px; color:var(--ink2); background:transparent;
    border:none; cursor:pointer; padding:6px 8px;
  }
  .organise[aria-checked="true"] { color:var(--ink); font-weight:600; }
  .organise .piste {
    width:30px; height:16px; border-radius:999px; flex:none;
    background:var(--bg); border:1px solid var(--border);
    display:inline-flex; align-items:center; padding:0 2px;
    transition:background .12s ease;
  }
  .organise[aria-checked="true"] .piste {
    background:var(--accent); border-color:var(--accent);
    justify-content:flex-end;
  }
  .organise .disque {
    width:10px; height:10px; border-radius:50%; background:var(--muted);
  }
  .organise[aria-checked="true"] .disque { background:var(--onAccent); }
  .vider {
    height:22px; width:22px; padding:0; display:inline-flex; flex:none;
    align-items:center; justify-content:center; color:var(--muted);
    background:transparent; border:none; border-radius:var(--r-controle); cursor:pointer;
  }
  .vider:hover { color:var(--ink); background:var(--sel); }
  button {
    height:32px; padding:0 16px; display:inline-flex; align-items:center;
    gap:8px; font-size:13px; color:var(--ink); background:var(--surface);
    border:1px solid var(--border); border-radius:var(--r-controle); cursor:pointer;
  }
  button:hover { background:var(--sel); }
  .principal {
    font-weight:600; color:var(--onAccent); background:var(--accent);
    border-color:var(--accent);
  }
  .principal:hover { background:var(--accentH); border-color:var(--accentH); }

  /* A29: the nav lane lives at 248 px (236 before v2) — since R3
     (PLAN-RETOURS-V3), 248 and 400 are the DEFAULTS: the widths live
     in variables, set at the handle, capped at the module. */
  .colonnes {
    flex:1; display:grid;
    grid-template-columns:var(--l-nav, 248px) var(--l-liste, 400px) minmax(0,1fr);
    min-height:0; position:relative;
  }
  /* PLAN-VOLETS (V-D1): in two panes the list takes the width — row
     template unchanged (V-D3), the preview breathes. In one pane
     (E2) the list is alone: its right hairline no longer has a
     neighbor. */
  .colonnes--2 { grid-template-columns:var(--l-nav, 248px) minmax(0,1fr); }
  .colonnes--1 { grid-template-columns:minmax(0,1fr); }
  /* E2: the Screener's scene extends from the nav to the right edge
     — the reading pane doesn't exist at the desk. */
  .cadre-portier {
    grid-column:2 / -1; display:flex; min-width:0; min-height:0;
    overflow:hidden; background:var(--bg);
  }
  .colonnes--1 .cadre-portier { grid-column:1 / -1; }
  /* E4: the Organized Inbox has no reading pane — the list extends
     from the nav to the right edge (centered column inside). */
  .colonnes--organise > :global([data-testid="liste"]) { grid-column:2 / -1; }
  .colonnes--1.colonnes--organise > :global([data-testid="liste"]) { grid-column:1 / -1; }
  /* The handle (R3): 7 px straddling the hairline, out of flow; on
     hover, drag and keyboard focus, a 2 px accent stroke states the
     boundary — the grid itself doesn't move a pixel. */
  .poignee {
    position:absolute; top:0; bottom:0; width:7px; z-index:1;
    cursor:col-resize; touch-action:none;
  }
  .poignee::after {
    content:''; position:absolute; top:0; bottom:0; left:2px; width:2px;
    background:transparent;
  }
  .poignee:hover::after, .poignee:active::after,
  .poignee:focus-visible::after { background:var(--accent); }
  .colonnes--1 > :global(.colonne) { border-right:none; }

  /* The drawer button (E2): 32 px, the header-button grammar; the
     brand loses its column width in one pane. */
  .btn-tiroir {
    width:32px; height:32px; padding:0; flex:none; display:inline-flex;
    align-items:center; justify-content:center; color:var(--ink2);
    background:var(--surface); border:1px solid var(--border);
    border-radius:var(--r-controle); cursor:pointer;
  }
  .btn-tiroir:hover { background:var(--sel); color:var(--ink); }
  .marque--libre { width:auto; }

  /* The drawer: a 268 px overlay under a scrim, at overlay level
     (the scrim is a BUTTON — click and keyboard both close it). */
  .scrim-tiroir {
    position:absolute; inset:0; height:auto; padding:0; z-index:2;
    background:var(--scrim); border:none; cursor:default;
  }
  .tiroir {
    position:absolute; top:0; bottom:0; left:0; width:268px; z-index:2;
    background:var(--bg); border-right:1px solid var(--border);
    box-shadow:var(--shadow); display:flex; flex-direction:column;
  }
  .tiroir > :global(nav) { flex:1; border-right:none; }
  .tete-tiroir {
    height:60px; flex:none; display:flex; align-items:center; gap:10px;
    padding:0 16px 0 20px; border-bottom:1px solid var(--border);
    font-size:18px; font-weight:600; color:var(--ink);
  }
  .fermer-tiroir { margin-left:auto; }

  .statut {
    position:relative; height:36px; flex:none; background:var(--bg);
    border-top:1px solid var(--border); display:flex; align-items:center;
    gap:14px; padding:0 24px;
    font-size:12px; color:var(--muted);
  }
  #perf { font-variant-numeric:tabular-nums; flex:none; }
  .texte { display:flex; align-items:center; gap:8px; min-width:0; flex:1; }
  .texte span { overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  /* The poll button (E3, S-D1 variant A): 26 px, it fits within the
     bar's 36 px without forcing them — dimensions from the "Status
     bar and sync" section of the System (the mockup, reverted, is
     dead as of GO — DC-D4). */
  .btn-statut {
    height:26px; padding:0 12px; display:inline-flex; align-items:center;
    gap:7px; font-size:12px; font-weight:600; color:var(--ink2);
    background:var(--surface); border:1px solid var(--border);
    border-radius:var(--r-controle); cursor:pointer; flex:none;
  }
  .btn-statut:hover { background:var(--sel); color:var(--ink); }
  .btn-statut[disabled] { opacity:.55; cursor:default; }
  .btn-statut[disabled]:hover { background:var(--surface); color:var(--ink2); }
  .btn-statut :global(.ic) { width:14px; height:14px; }
  .point-alerte {
    width:7px; height:7px; border-radius:50%; background:var(--alert);
    flex:none;
  }
</style>

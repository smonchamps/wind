<script>
  // Windowed list of screen 02 — continuous rows separated by a net,
  // the track drawing (A29/A30), served by `list_category`: the
  // source is (category, account, unread), the tabs live in the
  // footer of this column. Since A44 (PLAN-RETOURS-V3, reverses the
  // "bare row" of A29/A2; field 2026-08-16: height TO CONTENT, no
  // reserved rank) a row that has something to say carries the
  // prototype's chip rank and grows for it: TWO templates (h1 bare, h2
  // carrying), the windowing mechanics from before A29 —
  // chipsParPage, extraPuce, iterative correction — come back into
  // service, identical to what they were (848f286~1).
  //
  // Source change = new generation: in-flight pages from the previous
  // source are discarded on arrival, never mixed in.
  import Icon from './Icon.svelte';
  import Menu from './Menu.svelte';
  import { tick, untrack } from 'svelte';
  import { SvelteMap } from 'svelte/reactivity';
  import { call } from './lib/transport.js';
  import { watchViews } from './lib/views.svelte.js';
  import { when } from './lib/when.js';
  import { invitationChip } from './lib/invitation.js';
  import { mailboxBlock, mixedView } from './lib/mailbox.js';
  import { rowPad } from './lib/spacing.svelte.js';
  import { initials } from './lib/initiales.js';
  import { activation } from './lib/keyboard.js';
  import { t } from './lib/text.svelte.js';
  import { mailboxLabelKey } from './lib/organized.svelte.js';

  let {
    category = 'inbox',
    account = null,
    // A80/D7: the mailbox block is only shown if the accounts REALLY
    // mix — so we need to know how many there are.
    accounts = [],
    // A80: the account markers feed the DRAWING of the mailbox block,
    // which only lives in the unified mailbox and in search (D3/D7:
    // where identifying the account makes sense) — on all rows,
    // marker or not (D8).
    markers = {},
    // PLAN-RETOURS-9 (D4): the custom name is the block's label — the
    // address remains the fallback for an account without a name, and
    // the tooltip's truth.
    names = {},
    tab = 'tous',
    search = '',
    // PLAN-BROUILLONS: the local drafts (rows from `list_drafts`),
    // polled by the App — the Drafts folder shows them, the Inbox
    // mentions the threads that carry one.
    drafts = [],
    onresume = () => {},
    onselect = () => {},
    ontab = () => {},
    ontotal = () => {},
    onresults = () => {},
    onflash = () => {},
    // PLAN-RETOURS-10 R1: BULK gestures go up to the App, which owns
    // the commands (archive, delete, spam, read/unread) — the List
    // owns the selection, never the action.
    ongroup = async () => {},
    // PLAN-MODE-ORGANISE E4: in organized mode, the Inbox is shown in
    // a centered column with SECTIONS and each row carries the ⋯
    // gesture menu (Move to…, Screen out) — passed up to the App,
    // which owns the commands (same rule as the bulk selection).
    organized = false,
    // RETOURS-15 D1: the centered column belongs to the PANELESS
    // organized Inbox (two or one pane(s)); at three panes the list
    // lives in the 400 px column and the pane reads (A99 reversed).
    centered = true,
    onmove = () => {},
    onsetaside = () => {},
  } = $props();

  const PAGE = 200;

  // E4 — the sections of the organized Inbox (verdict S1/A2): the
  // service renders ONE ordered stream "unread first", the seam is
  // the unread COUNT. The headers live OUTSIDE the rows (the
  // windowing geometry gains an offset, the pattern of the invitation
  // chips) — never a row of exceptional height.
  // The organized Inbox's own dressing (RETOURS-14 R2: normalized
  // header, no tabs) — a property of the MODE, whatever the pane
  // setting says. `sections` and `center` both derive from it: the
  // shared clauses live ONCE (review 2026-09-04).
  const organizedInboxView = $derived(
    organized && category === 'inbox'
      && results === null && draftRows === null,
  );
  const sections = $derived(organizedInboxView && tab === 'tous');
  // The centered column (~760 px, prototype) — only while the Inbox
  // is PANELESS (RETOURS-15 D1): at three panes the 400 px list
  // column is the geometry, the pane reads.
  const center = $derived(organizedInboxView && centered);
  // The ⋯ gesture menu per row — organized views only.
  const organizedGestures = $derived(
    organized && ['inbox', 'feed', 'paper_trail'].includes(category)
      && results === null && draftRows === null,
  );
  let seam = $state(0);
  // 52 px: the air ABOVE the label (CE finding at the E4 visual STOP —
  // the last mail of a section and the title of the next one need
  // room to breathe); the label stays anchored to the bottom of its
  // band.
  const H_HEADER = 52;
  const headers = $derived.by(() => {
    if (!sections || total === 0) return [];
    const list = [];
    if (seam > 0) list.push({ index: 0, label: t('list.sectionNew', { n: seam }) });
    if (seam < total) list.push({ index: seam, label: t('list.sectionSeen') });
    return list;
  });
  // The POSITIONED headers — a separate derived: `offset` reads
  // non-reactive Maps (pages/chips), only `version` signals their
  // movements (the channel of `spaceHeight`).
  const headerPositions = $derived.by(() => {
    void version;
    void h1;
    return headers.map((e) => ({ ...e, top: offset(e.index) - H_HEADER }));
  });
  function headersBefore(i) {
    let n = 0;
    for (const e of headers) if (e.index <= i) n += 1;
    return n;
  }
  // RETOURS-14 R2: the name of the current section stays VISIBLE
  // while scrolling — a band stuck at the top of the frame, shown as
  // soon as the real band has gone above. `first` is already the
  // reactive truth of the scroll (windowing): at `first > 0`, the
  // band of the current section is no longer on screen.
  const stuckSection = $derived.by(() => {
    if (!sections || first <= 0) return null;
    let current = null;
    for (const e of headers) {
      if (e.index <= first) current = e;
      else break;
    }
    return current;
  });
  // The ⋯ menu — the Screener's pattern: anchored on click, bounded
  // to the window, closed on an outside click and on Escape.
  let gestureMenu = $state(null);
  const rowKey = (l) => `${l.account_id}:${l.mailbox}:${l.uid}`;
  function openGestures(e, row) {
    e.stopPropagation();
    const r = e.currentTarget.getBoundingClientRect();
    gestureMenu = {
      row,
      key: rowKey(row),
      x: r.left,
      y: r.bottom + 4,
    };
  }
  function gesture(destination) {
    const { row } = gestureMenu;
    gestureMenu = null;
    onmove(row, destination);
  }
  const OVER = 8;

  // R4 (PLAN-RETOURS-MAIL): in the Sent folder, the sender is
  // ONESELF — repeating one's own name on every row teaches nothing.
  // The column shows the RECIPIENT ("To: X"), taken from `to_addrs`
  // stored at sync time. Failing that (an old send not yet backfilled)
  // the previous sender name is kept — never a silent row.
  const toSend = (row) => category === 'sent' && (row.to_addrs?.length ?? 0) > 0;
  const contact = (row) =>
    toSend(row) ? row.to_addrs.join(', ') : row.sender;

  // A80: the mailbox block lives where accounts MIX — the unified
  // mailbox (D3 of A74) and search, D7. Unlike the badge, it does NOT
  // require a marker: the word is enough, and the label falls back to
  // the address when no name is set (D8).
  // The WHOLE rule lives in lib/mailbox.js — view guard included, since
  // the 2026-08-25 field verdict (point 12): the reading pane applies
  // the same one, and two expressions would diverge.
  const mailboxOf = (row) =>
    !mixedView(account, results !== null)
      ? null
      : mailboxBlock({
        accountId: row.account_id,
        address: row.account_email,
        markers,
        names,
        accounts,
      });

  let frame = $state(null);
  let total = $state(0);
  let first = $state(0);
  let version = $state(0);
  // Seeds = the real geometry of the default notch (measured: 88 bare,
  // 115 carrying). They only serve the first frame, before the probes
  // bind — keeping them accurate avoids a needless jump.
  let h1 = $state(88);
  let h2 = $state(115);
  let selection = $state(null);
  let firstPageMs = $state(null);
  // PLAN-DEFILEMENT-PROFOND E2 + field 2026-08-20:
  // `sourceAnswered` — a page of the current SOURCE has arrived;
  // before that proof the screen shows the waiting state, never "No
  // messages here.". `exactTotal` — the displayed total is exact:
  // either a page shorter than its limit said so on its own (end of
  // list — small folders NEVER pay for a count), or `category_total`
  // has answered. In between, `total` is a FLOOR drawn from the
  // served rows: rows display without waiting for the count (~240 ms
  // full scan, more when cold), the scrollbar adjusts as the real
  // total arrives.
  let sourceAnswered = $state(false);
  let exactTotal = $state(false);

  // Two counters (review 2026-08-20): `source` only moves when
  // (category, account, tab) changes — a flight born of ANOTHER
  // source is discarded on arrival; `generation` also moves on every
  // reload — a flight of the SAME source at an earlier generation
  // remains good to DISPLAY (stale-while-revalidate), its page stays
  // mismatched so it gets reserved. Without this distinction, reloads
  // closer together than a deep page's settlement (body backfill: one
  // reload PER BATCH, for days) would condemn every result on
  // arrival — a permanent skeleton.
  let source = 0;
  let generation = 0;
  let pages = new Map();
  let chipsPerPage = new Map();
  let pending = new Map();
  // Stale-while-revalidate (PLAN-REACTIVITE E1): the generation at
  // which each page was served. A reload bumps `generation` WITHOUT
  // discarding `pages` — the displayed rows stay as the backdrop, and
  // a page is only re-served if its generation is mismatched.
  let servedAt = new Map();

  // Two templates (A44, field: height to content): h1 the bare row,
  // h2 the carrying one — the geometry corrects the multiplication by
  // the count of carrying rows BEFORE the index, kept per page.
  // `pinned` only comes from the pinned section (outside windowing):
  // it renders the row as carrying to show its mark, without touching
  // the pages. Field R3'c (2026-08-23): the GESTURES of an invitation
  // occupy a rank OF THEIR OWN — the other chips (messages, files,
  // pin) drop to the rank below and only rise back up when the reply
  // chip joins them. Windowing therefore counts RANKS (0, 1, or 2):
  // the marginal cost of a rank is constant (extraChip = h2 − h1, the
  // grid spaces every rank by the same row-gap) — A44's correction
  // generalizes, still no extra measured template.
  const otherChips = (l) => l.thread_size > 1 || l.attachment_count > 0 || l.pinned;
  const invitationGestures = (l) =>
    l.invitation != null && !invitationChip(l.invitation) && l.invitation.can_reply;
  const chipRanks = (l) =>
    (invitationGestures(l) ? 1 : 0) +
    (otherChips(l) || (l.invitation != null && invitationChip(l.invitation) != null) ? 1 : 0);
  const hasChips = (l) => chipRanks(l) > 0;

  // R10: reply to an invitation WITHOUT opening it — the same path as
  // the card (reply_invitation: log + reply in one transaction), the
  // subject in the product's language, the chip follows locally.
  // stopPropagation: the click does not choose the row.
  let invitationReplies = $state({});
  async function replyInvitation(e, row, reply) {
    e.stopPropagation();
    const key = `${row.account_id}/${row.invitation.mailbox}/${row.invitation.uid}`;
    if (invitationReplies[key]) return;
    invitationReplies[key] = true;
    // OPTIMISTIC (field R3'a, fixed on the 3rd pass): the chip
    // replaces the buttons AT THE INSTANT of the click — the log
    // follows behind; a failure restores the previous state and says
    // so. Rows live in NON-reactive pages (the windowing): it is
    // `version` — the homegrown invalidation channel — that redraws
    // the window, otherwise the chip only appeared at the next
    // invalidation coming from elsewhere (the selection, a probe…).
    const before = row.invitation.reply;
    row.invitation.reply = reply;
    version += 1;
    try {
      const subject = t(`inv.subject_${reply}`, { title: row.invitation.title });
      await call('reply_invitation', {
        accountId: row.account_id,
        mailbox: row.invitation.mailbox,
        uid: row.invitation.uid,
        reply,
        subject,
        body: subject,
      });
      call('flush_outbox').catch(() => {});
    } catch (err) {
      row.invitation.reply = before;
      version += 1;
      onflash(t('error.invitation', { err }));
    } finally {
      invitationReplies[key] = false;
    }
  }

  // R4 (PLAN-RETOURS-7, D4/D5): the PINNED conversations of the
  // Inbox — served SEPARATELY (`pinned_rows`), placed ahead of the
  // stream in the SAME scroll frame; the paginated stream excludes
  // them on the core side (never the same row twice). Their MEASURED
  // height recalibrates the windowing: the stream starts below the
  // section.
  let pins = $state([]);
  let pinsTopMeasured = $state(0);
  // The measurement survives the block's unmount: zero as soon as
  // there is no pin left, without waiting for a measurement that will
  // never come.
  const pinsTop = $derived(pins.length > 0 ? pinsTopMeasured : 0);
  // Emptiness is never asserted without proof (E2) — proof from BOTH
  // sources: page 0 of the stream AND the pins' response. Without
  // this flag, a mailbox entirely made of pins would say "No messages
  // here." during (or after, on failure) the `pinned_rows` flight
  // (review 2026-08-21).
  let answeredPins = $state(false);
  function startPins() {
    if (category !== 'inbox') {
      pins = [];
      answeredPins = true;
      return;
    }
    const capturedSource = source;
    call('pinned_rows', { accountId: account, unread: tab === 'nonlus' })
      .then((rows) => {
        if (capturedSource === source) pins = rows;
      })
      .catch((err) => console.error('pinned_rows :', err))
      .finally(() => {
        if (capturedSource === source) answeredPins = true;
      });
  }
  const extraChip = $derived(h2 - h1);

  function chipsBefore(i) {
    let extra = 0;
    const fullPage = Math.floor(i / PAGE);
    for (const [p, n] of chipsPerPage) {
      if (p < fullPage) extra += n;
    }
    const page = pages.get(fullPage);
    if (page) {
      const bound = i - fullPage * PAGE;
      for (let k = 0; k < bound && k < page.length; k++) {
        extra += chipRanks(page[k]);
      }
    }
    return extra;
  }
  function offset(i) {
    return i * h1 + chipsBefore(i) * extraChip + headersBefore(i) * H_HEADER;
  }

  const spaceHeight = $derived.by(() => {
    void version;
    if (total === 0) return 0;
    let extra = 0;
    for (const n of chipsPerPage.values()) extra += n;
    return total * h1 + extra * extraChip + headers.length * H_HEADER;
  });

  function indexFor(scrollTop) {
    let i = Math.max(0, Math.floor(scrollTop / h1));
    for (let turn = 0; turn < 4; turn++) {
      const corrected = Math.max(
        0,
        Math.floor(
          (scrollTop - chipsBefore(i) * extraChip - headersBefore(i) * H_HEADER) / h1,
        ),
      );
      if (corrected === i) break;
      i = corrected;
    }
    return Math.min(i, Math.max(0, total - 1));
  }

  // PLAN-DEFILEMENT-PROFOND E1: at most VOL_MAX pages in flight
  // (`pending` is the gauge — a single truth), and at every freed
  // flight we launch the most useful page of the CURRENT window —
  // never pages of a position already passed. Before: the effect
  // served every page crossed by every position of a held drag (~161
  // calls for 2 s of scrollbar, measured at the bench); the
  // serialized `off_pump` queue (ADR 0019) took minutes to drain on
  // the real database, and ALL commands waited behind it.
  //
  // A SINGLE flight (field 2026-08-20): the core serializes anyway
  // (global lock) — two flights would parallelize nothing, they would
  // only lengthen the wait for the useful page, once the gesture
  // stops, by one page already passed. The straddling window is
  // served in two successive trips, exactly what the core would have
  // done.
  const FLIGHT_MAX = 1;

  // The most useful page: those in the visible window, closest to
  // `first` first; as a last resort the mismatched page 0 (it carries
  // the fresh total from a reload). Null if everything that matters
  // is served or already in flight.
  // Flights in progress are keyed by (source, page): a flight from
  // ANOTHER source never masks the same page of the new source — the
  // gauge (`pending.size`) counts them all, key lookups only see the
  // current source.
  const flightKey = (p) => `${source}:${p}`;

  function usefulPage() {
    const de = Math.floor(start / PAGE);
    const a = Math.floor(Math.max(0, end - 1) / PAGE);
    const pivot = Math.floor(first / PAGE);
    const candidates = [];
    for (let p = de; p <= a; p++) candidates.push(p);
    candidates.sort((x, y) => Math.abs(x - pivot) - Math.abs(y - pivot));
    candidates.push(0);
    for (const p of candidates) {
      if (servedAt.get(p) !== generation && !pending.has(flightKey(p))) return p;
    }
    return null;
  }

  function pump() {
    if (category === 'drafts') return;
    // Page 0 of a source that has not answered yet jumps AHEAD of the
    // gauge (review 2026-08-20): switching folders starts right away,
    // even if a deep page of the old source is still in flight — the
    // overrun is bounded (only one, `pending` holds it).
    if (!sourceAnswered && servedAt.get(0) !== generation && !pending.has(flightKey(0))) {
      launch(0);
    }
    while (pending.size < FLIGHT_MAX) {
      const p = usefulPage();
      if (p === null) break;
      launch(p);
    }
    // The count — never ahead of rows: only when the pump is at rest,
    // and never if a short page has already stated the total.
    if (
      pending.size === 0 &&
      sourceAnswered &&
      totalServedAt !== generation &&
      !totalInFlight
    ) {
      startTotal();
    }
    if (
      sections &&
      pending.size === 0 &&
      sourceAnswered &&
      seamServedAt !== generation &&
      !seamInFlight
    ) {
      startSeam();
    }
  }

  // The source's total, apart from the pages (field 2026-08-20): a
  // full scan's count costs more than the page — it follows the first
  // render, it never precedes it.
  let totalInFlight = false;
  let totalServedAt = -1;
  function startTotal() {
    const capturedSource = source;
    const capturedGen = generation;
    totalInFlight = true;
    call('category_total', {
      category: category,
      accountId: account,
      unread: tab === 'nonlus',
    })
      .then((n) => {
        if (capturedSource !== source) return;
        total = n;
        exactTotal = true;
        totalServedAt = capturedGen;
      })
      .catch((err) => {
        console.error(`category_total ${category} :`, err);
      })
      .finally(() => {
        totalInFlight = false;
      });
    // E4: the section seam — the unread COUNT, same cadence as the
    // total, never ahead of rows.
  }

  // E4: the section seam — the unread COUNT, its OWN pump: the total
  // can come from a short page without `startTotal` ever running
  // (small mailbox) — grafted onto it, the seam wouldn't run either
  // (proven with the e2e fixture).
  let seamInFlight = false;
  let seamServedAt = -1;
  function startSeam() {
    const capturedSource = source;
    const capturedGen = generation;
    seamInFlight = true;
    call('category_total', {
      category: category,
      accountId: account,
      unread: true,
    })
      .then((n) => {
        if (capturedSource !== source) return;
        seam = n;
        seamServedAt = capturedGen;
      })
      .catch(() => {})
      .finally(() => {
        seamInFlight = false;
      });
  }

  // The component's life flag, lowered by the effect's cleanup (E10):
  // a `.finally` that arrives after unmount no longer pumps.
  let alive = true;
  $effect(() => () => {
    alive = false;
  });

  function launch(p) {
    const capturedSource = source;
    const capturedGen = generation;
    const key = flightKey(p);
    const t0 = performance.now();
    // A failure does not re-pump (review 2026-08-20): the same page
    // would be re-selected instantly — a storm of retries at
    // microtask speed on any persistent error. The next attempt waits
    // for a gesture or an effect, as before.
    let failed = false;
    const promise = call('list_category', {
      category: category,
      accountId: account,
      unread: tab === 'nonlus',
      offset: p * PAGE,
      limit: PAGE,
    })
      .then(async (page) => {
        // Another source: result discarded. Same source, earlier
        // generation (a reload during the flight): the rows remain
        // good to display — logged at THEIR generation, the page
        // stays mismatched and gets re-served (stale-while-revalidate).
        if (capturedSource !== source) return;
        sourceAnswered = true;
        // The page no longer carries a total (field 2026-08-20): the
        // rows themselves say it — a SHORT page marks the exact end
        // of the list, a full page sets a FLOOR that the scrollbar
        // follows while waiting for `category_total`.
        if (page.rows.length < PAGE) {
          total = p * PAGE + page.rows.length;
          exactTotal = true;
          totalServedAt = capturedGen;
        } else {
          total = Math.max(total, (p + 1) * PAGE);
        }
        // The chip delta, not the raw count: a REPLACED page already
        // displayed its own — the scroll anchor must only move by the
        // difference (first served: before = 0).
        const before = chipsPerPage.get(p) ?? 0;
        pages.set(p, page.rows);
        servedAt.set(p, capturedGen);
        let n = 0;
        for (const l of page.rows) n += chipRanks(l);
        chipsPerPage.set(p, n);
        if (firstPageMs === null) firstPageMs = performance.now() - t0;
        const delta = n - before;
        if (delta !== 0 && (p + 1) * PAGE <= first && frame) {
          version += 1;
          await tick();
          frame.scrollTop += delta * extraChip;
        } else {
          version += 1;
        }
      })
      .catch((err) => {
        failed = true;
        console.error(`list_category ${category} page ${p} :`, err);
      })
      .finally(() => {
        pending.delete(key);
        // A flight has freed up: the CURRENT window chooses what's
        // next — if the component is still alive (PLAN-AUDIT-V2 E10:
        // an unmounted list kept pumping, up to four IPC calls queued
        // for a dead component).
        if (!failed && alive) pump();
      });
    pending.set(key, promise);
    return promise;
  }

  function servePage(p) {
    // The Drafts folder is not served here: its page comes from
    // `list_drafts` (PLAN-BROUILLONS, B-D1), not `list_category`.
    // Kept for `goAndServe` (bench P1, e2e): a deliberate jump targets
    // exactly its pages — it serves without waiting for the gauge
    // (the overrun of a jump is accepted, one window at most).
    if (category === 'drafts') return Promise.resolve();
    if (servedAt.get(p) === generation) return Promise.resolve();
    // The qualified key only sees the current source: never a promise
    // from another source, which would settle without writing
    // anything.
    return pending.get(flightKey(p)) ?? launch(p);
  }

  // New source -> restart from the top, discard everything, re-serve.
  // ONLY the source key is a dependency: everything else is under
  // `untrack`, otherwise the effect would depend on what it modifies
  // (a loop).
  $effect(() => {
    void category;
    void account;
    void tab;
    untrack(() => {
      source += 1;
      generation += 1;
      pages = new Map();
      chipsPerPage = new Map();
      servedAt = new Map();
      // `pending` stays: the open flights occupy the gauge until they
      // settle — their result from another source is discarded, and
      // their closure re-pumps the new source's window (its page 0
      // jumps ahead of the gauge, see pump).
      total = 0;
      // E4 (review): the seam is a state OF THE SOURCE — kept, it
      // would paint the N of the old mailbox onto the new one (and
      // hide "Already seen" as long as seam >= total).
      seam = 0;
      seamServedAt = -1;
      sourceAnswered = false;
      exactTotal = false;
      first = 0;
      selection = null;
      // R1: the multi-selection never survives its source — a bulk
      // gesture on rows no longer visible would be a trap (D4: the
      // selection is always in plain sight).
      clearSelection();
      // The first page is re-measured PER source (review 2026-08-20):
      // fixed at the very first, `snapshot().firstPageMs` would have
      // lied to the benches — the startup status, itself, is already
      // captured.
      firstPageMs = null;
      pins = [];
      answeredPins = false;
      if (frame) frame.scrollTop = 0;
      version += 1;
      pump();
      startPins();
    });
  });

  // The pinned section changes the height ABOVE the stream outside
  // any scroll event (async measurement, pin/unpin): `first`
  // recalibrates on every movement of the measurement, otherwise the
  // window would stay anchored to the old origin until the next pixel
  // of scroll (review 2026-08-21).
  $effect(() => {
    void pinsTop;
    untrack(() => {
      if (frame) onScroll();
    });
  });

  // E4: the seam's arrival (0 → n) moves the section headers' height
  // outside any scroll event — same recalibration as the pinned
  // band.
  $effect(() => {
    void headers;
    untrack(() => {
      if (frame) onScroll();
    });
  });

  // A83 — the re-anchoring on a spacing notch change, and it is the
  // INVERSE of the effect above: when the height above the stream
  // moves, the pixels keep their meaning and it is the index that we
  // recompute; when it is a ROW's height that moves, the index <->
  // pixel conversion changes and the TOP ROW must be kept by moving
  // the scroll. Without this, changing the spacing would make the
  // list jump elsewhere — the further one has scrolled, the worse.
  //
  // It takes TWO steps, and the review showed why: we read the
  // position with the OLD geometry and write it with the NEW one.
  //
  // 1) The CAPTURE fires on the notch itself — `rowPad()`, an
  //    UPSTREAM state, which moves before the style is relaid out and
  //    therefore before the probes re-measure. Capturing from the
  //    `h1` effect would be TOO LATE: the pinned-rows effect, created
  //    higher up and therefore run before it in the same flush (the
  //    pinned rows are `.ligne` too, they also grow), has already
  //    rewritten `first` with the new height against the old
  //    scrollTop — 44 rows of drift measured with two pinned
  //    conversations.
  let anchorStep = null;
  $effect(() => {
    void rowPad();
    untrack(() => {
      anchorStep = frame
        ? { row: first, inPins: frame.scrollTop < pinsTop }
        : null;
    });
  });

  // 2) The APPLICATION waits for the probes to render the new height,
  //    then hands its row back to the user. Two guards, each paid for
  //    by a real bug:
  //    — the STREAM only: `go()` speaks the geometry of the windowed
  //      stream; applying it to the Drafts folder (where `total`
  //      stays 0, so `go(0)`) or to a search would scroll the list
  //      back to the top on every notch change;
  //    — not from the PINNED BAND: `go(0)` sets the scroll BELOW it,
  //      meaning it would push it off the screen of someone who was
  //      precisely looking at it.
  $effect(() => {
    void h1;
    const anchor = anchorStep;
    if (anchor === null) return;
    anchorStep = null;
    if (anchor.inPins) return;
    untrack(() => {
      if (frame && results === null && draftRows === null) {
        go(anchor.row);
      }
    });
  });

  // CE decision D3 (2026-08-25) — a PRE-EXISTING bug fixed here: the
  // frame's height was read via `frame.clientHeight`, which is not a
  // signal. The derived value therefore only recomputed when `frame`
  // or `h1` changed, and enlarging the window by more than OVER rows
  // left an empty band at the bottom until the next scroll.
  // `bind:clientHeight` compiles to a ResizeObserver (the pattern of
  // `pinsTopMeasured`): the window follows the window.
  // Fixed WITHIN this job and not left as debt, because the spacing
  // notch would have masked it intermittently — every notch change
  // recomputes `visibleCount` — and made it irreproducible.
  let frameHeight = $state(0);
  const visibleCount = $derived(
    frameHeight > 0 ? Math.ceil(frameHeight / h1) + 1 : 12,
  );
  const start = $derived(Math.max(0, first - OVER));
  const end = $derived(Math.min(total, first + visibleCount + OVER));

  const window = $derived.by(() => {
    void version;
    const arr = [];
    for (let i = start; i < end; i++) {
      const page = pages.get(Math.floor(i / PAGE));
      arr.push({ i, row: page ? page[i % PAGE] : null });
    }
    return arr;
  });

  $effect(() => {
    void start;
    void end;
    untrack(pump);
  });

  function onScroll() {
    // The pinned section lives ABOVE the stream in the same frame:
    // the stream's windowing is computed below it.
    first = indexFor(Math.max(0, frame.scrollTop - pinsTop));
  }

  // D1 — search (FTS5, `search_messages`): the results take the
  // list's place, in the PROTOTYPE'S OWN ROWS — no new UI. Bounded on
  // the core side: no windowing. Below 3 characters, the mailbox
  // returns as it was.
  let results = $state(null);
  let resultsTotal = $state(0);
  let loadingMore = $state(false);
  let searchTimer;
  let searchToken = 0;
  // The batch, mirroring the command's `SEARCH_LIMIT`: the size of a
  // "load more" (the last batch renders the rest, fewer than 100).
  const BATCH = 100;
  // Soft cap (D1): beyond it, the "load more" button gives way to a
  // prompt to refine — the list is not windowed, stacking without end
  // would eventually weigh down the DOM. Ten batches.
  const MAX_RESULTS = 10 * BATCH;
  async function runSearch(q) {
    const mine = ++searchToken;
    try {
      const res = await call('search_messages', { query: q, offset: 0 });
      if (mine !== searchToken) return; // a more recent keystroke
      results = res.rows;
      resultsTotal = res.total;
      // The rendered count (capped) AND the total: the bar says "N of M".
      onresults(res.rows.length, res.total);
    } catch (err) {
      console.error('search_messages :', err);
    }
  }
  // "Load more": the next batch, APPENDED. We don't touch the token
  // (nothing to cancel), but we CAPTURE it: if a keystroke happens
  // during loading, it increments the token and we discard this now
  // stale batch.
  async function loadMore() {
    const q = search.trim();
    // SYNCHRONOUS guard: only one batch in flight at a time. The
    // `disabled` button can lag by one tick — this test blocks a
    // double-trigger before it reads the same offset twice (and so
    // appends the same batch twice).
    if (q.length < 3 || results === null || loadingMore) return;
    const mine = searchToken;
    loadingMore = true;
    try {
      const res = await call('search_messages', { query: q, offset: results.length });
      if (mine !== searchToken) return; // a more recent search took over
      results = [...results, ...res.rows];
      resultsTotal = res.total;
      onresults(results.length, res.total);
    } catch (err) {
      console.error('search_messages (plus) :', err);
    } finally {
      // This batch is no longer in flight, NO MATTER WHAT (superseded
      // included): without an unconditional reset, a keystroke during
      // loading would leave the flag at true and condemn the button
      // for the following searches.
      loadingMore = false;
    }
  }
  $effect(() => {
    const q = search.trim();
    untrack(() => {
      clearTimeout(searchTimer);
      // R1: a keystroke changes the displayed rows — the
      // multi-selection clears in both directions (entering,
      // refining, leaving).
      clearSelection();
      if (q.length < 3) {
        searchToken += 1;
        results = null;
        resultsTotal = 0;
        onresults(null, null);
        return;
      }
      searchTimer = setTimeout(() => runSearch(q), 150);
    });
  });

  $effect(() => {
    // The Drafts folder counts its own rows — the status bar says
    // "Drafts · N items" on the same mechanism. Elsewhere, the count
    // is only stated once EXACT (E2 + field 2026-08-20): null as long
    // as the real total isn't there — the status will never say "0
    // items" on a mailbox that hasn't answered, nor a provisional
    // floor as if it were the count. Pinned rows count: they are ON
    // SCREEN — without them, the bar would say "8 items" in front of
    // 10 rows (review 2026-08-21).
    const n =
      draftRows !== null
        ? draftRows.length
        : exactTotal
          ? total + pins.length
          : null;
    untrack(() => ontotal(n));
  });

  // A83: `sonder()` and `sondees` are dead — the probes are mounted
  // permanently and bind via `bind:offsetHeight`. A one-off
  // measurement could not follow an adjustable spacing notch.

  const key = (l) => `${l.account_id}/${l.mailbox}/${l.uid}`;
  function choose(l) {
    selection = key(l);
    onselect(l);
  }
  const isChosen = (l) => selection === key(l);

  // --- Multi-selection (PLAN-RETOURS-10 R1, D1-D4) --------------------
  // A key -> row set (SvelteMap: the rows live in NON-reactive pages,
  // the reactive map is enough to redraw checkboxes and bar). `anchor`
  // = the last toggled row — Shift-click extends from it, over the
  // DISPLAYED ORDER of the loaded rows (scope refusal §2.6: never
  // "the whole folder", the selection lives in what's loaded).
  let checkedRows = $state(new SvelteMap());
  let anchor = null;
  const isChecked = (l) => checkedRows.has(key(l));
  function toggle(l) {
    // A batch in flight freezes the selection: checking during
    // execution would manufacture rows that were never served
    // (review).
    if (gestureInProgress) return;
    const k = key(l);
    if (checkedRows.has(k)) checkedRows.delete(k);
    else checkedRows.set(k, l);
    anchor = k;
  }
  function orderedRows() {
    if (results !== null) return results;
    const stream = [];
    for (const p of [...pages.keys()].sort((a, b) => a - b)) stream.push(...pages.get(p));
    return [...pins, ...stream];
  }
  function extend(l) {
    if (gestureInProgress) return;
    const order = orderedRows();
    // Field 2026-08-27 (R1-2): without a check anchor, the anchor is
    // the SELECTED row (the chosen message, e.g. the first one at
    // startup) — the range runs from the selection to the target,
    // inclusive.
    const start = anchor ?? selection;
    const ia = start === null ? -1 : order.findIndex((x) => key(x) === start);
    const ib = order.findIndex((x) => key(x) === key(l));
    // Without a visible anchor (never checked nor chosen, or outside
    // the loaded pages), Shift-click amounts to a plain toggle — never
    // a silence.
    if (ia < 0 || ib < 0) return toggle(l);
    for (let i = Math.min(ia, ib); i <= Math.max(ia, ib); i++) {
      checkedRows.set(key(order[i]), order[i]);
    }
  }
  function clearSelection() {
    checkedRows.clear();
    anchor = null;
  }
  // The App unchecks the target of a completed SINGLE gesture (e/Del,
  // thread buttons): the bar never counts a row that's gone — a batch
  // replayed on a departed uid would report a false failure (review).
  export function uncheck(l) {
    checkedRows.delete(key(l));
    if (anchor === key(l)) anchor = null;
  }
  // A row's click, three regimes: Shift extends (Ctrl+Shift too),
  // Ctrl/Cmd toggles AND chooses, bare = choose (the existing
  // behavior, unchanged). Field 2026-08-27 (R1-1): the reading focus
  // FOLLOWS the Ctrl-click — leaving the outline (and the pane) on
  // another row than the one just checked was confusing.
  // Field finding (2026-08-15, follow-up to A38), valid for all THREE
  // regimes: chosen or checked with the mouse (detail > 0), the row
  // gives up focus — otherwise the :focus-visible ring would surface
  // later on a node recycled by index.
  function rowClick(e, l) {
    if (e.detail > 0) e.currentTarget.blur();
    if (e.shiftKey) return extend(l);
    if (e.ctrlKey || e.metaKey) toggle(l);
    choose(l);
  }
  // The bulk gesture: the App acts on the batch's SNAPSHOT; on
  // return, only that batch gets unchecked — a row checked during the
  // flight (blocked today, but the guard doesn't rely on that) would
  // survive.
  // Exported: the App's keyboard shortcuts (e/Del) apply to the
  // checked batch when it exists (field 2026-08-27, R1-8).
  export const selecting = () => checkedRows.size > 0;
  let gestureInProgress = $state(false);
  export async function act(action) {
    if (gestureInProgress) return;
    gestureInProgress = true;
    const batch = [...checkedRows.values()];
    try {
      await ongroup(action, batch);
    } finally {
      for (const l of batch) checkedRows.delete(key(l));
      anchor = null;
      gestureInProgress = false;
    }
  }

  // --- Drafts (PLAN-BROUILLONS) ----------------------------------------
  // Thread -> its most recent draft: the Inbox's mention (validated
  // variant B) shows the prefix and the draft's BODY in the preview —
  // first line and time untouched (B-D3). Never on a search: a result
  // is a message, not a conversation.
  const draftsPerThread = $derived.by(() => {
    const card = new Map();
    for (const b of drafts) {
      if (b.thread_id == null) continue;
      const known = card.get(b.thread_id);
      if (!known || b.updated_epoch > known.updated_epoch) card.set(b.thread_id, b);
    }
    return card;
  });
  const draftOf = (l) =>
    category === 'inbox' && results === null
      ? (draftsPerThread.get(l.thread_id) ?? null)
      : null;
  // The folder: the drafts of the account scoped by the nav, already
  // from most recent to oldest (`list_drafts`). Few by construction:
  // the unwindowed results path is enough.
  const draftRows = $derived(
    category === 'drafts'
      ? drafts.filter((b) => account === null || b.account_id === account)
      : null,
  );

  // R1 (RETOURS-10, D1): the selection bar's gestures — a table, like
  // TABS; in Junk, "Report" gives way to "Not spam", the mirror of
  // the reading pane.
  const BAR_GESTURES = $derived([
    { action: 'read', icon: 'drafts', label: 'action.markRead' },
    { action: 'unread', icon: 'mark_email_unread', label: 'action.markUnread' },
    { action: 'archive', icon: 'archive', label: 'action.archive' },
    category === 'junk'
      ? { action: 'not_spam', icon: 'report', label: 'action.notSpam' }
      : { action: 'spam', icon: 'report', label: 'action.reportSpam' },
    { action: 'delete', icon: 'delete', label: 'action.delete' },
  ]);

  const TABS = [
    { id: 'tous', icon: 'inbox', label: 'tab.all' },
    { id: 'nonlus', icon: 'mark_email_unread', label: 'tab.unread' },
    { id: 'drafts', icon: 'edit_note', label: 'mailbox.drafts' },
  ];
  const tabActive = $derived(category === 'drafts' ? 'drafts' : tab);

  // --- API (App, bench P1, e2e) ---------------------------------------
  export function go(index) {
    frame.scrollTop = offset(index) + pinsTop;
    onScroll();
  }
  export async function goAndServe(index) {
    const t0 = performance.now();
    go(index);
    const de = Math.floor(Math.max(0, index - OVER) / PAGE);
    const a = Math.floor(Math.min(Math.max(0, total - 1), index + visibleCount + OVER) / PAGE);
    const waits = [];
    for (let p = de; p <= a; p++) waits.push(servePage(p));
    await Promise.all(waits);
    await tick();
    void frame.offsetHeight;
    return performance.now() - t0;
  }
  export function snapshot() {
    // `exactTotal`: the benches that jump "over the whole depth"
    // (measure-v2) must wait for the REAL total — the floor drawn
    // from the first rows only covers the screen.
    return { total, exactTotal, first, h1, h2, firstPageMs };
  }
  export function rowAt(index) {
    const page = pages.get(Math.floor(index / PAGE));
    return page ? page[index % PAGE] : null;
  }
  // Keyboard triage (App): the selection is set without going through
  // the click — same key, same outline, no onselect callback.
  export function select(row) {
    selection = key(row);
  }
  // The row BELOW this one. Active search: the next of the results;
  // otherwise the absolute index in the windowed pages — an
  // unserved neighboring page returns null (rare: the window serves
  // wide).
  export function next(row) {
    const id = key(row);
    if (results !== null) {
      const i = results.findIndex((l) => key(l) === id);
      return i >= 0 && i + 1 < results.length ? results[i + 1] : null;
    }
    // From the pinned section: the next one lives there, or the first
    // row of the stream when exiting at the bottom.
    const e = pins.findIndex((l) => key(l) === id);
    if (e >= 0) {
      return e + 1 < pins.length ? pins[e + 1] : (rowAt(0) ?? null);
    }
    for (const [p, rows] of pages) {
      const i = rows.findIndex((l) => key(l) === id);
      if (i >= 0) return rowAt(p * PAGE + i + 1) ?? null;
    }
    return null;
  }
  export function markRead(row) {
    const id = key(row);
    for (const page of pages.values()) {
      for (const l of page) {
        if (key(l) === id) l.thread_unseen = 0;
      }
    }
    for (const l of pins) {
      if (key(l) === id) l.thread_unseen = 0;
    }
    version += 1;
  }
  export function reload() {
    // Stale-while-revalidate (PLAN-REACTIVITE E1): served rows STAY
    // displayed — each page is replaced when its fresh version
    // arrives, never discarded beforehand. The skeleton only exists
    // on a source's first load and when scrolling into the unknown;
    // off-screen pages, kept as they are, get re-served on scroll
    // (mismatched generation).
    generation += 1;
    // The pump re-serves the screen — the whole visible rank page by
    // page, then the mismatched page 0 (the fresh total); open
    // flights keep their places and re-pump as they settle (E1).
    pump();
    // R4: the pinned section follows every reload — a pin moves a row
    // between the section and the stream, never a duplicate.
    startPins();
    // An ACTIVE search also gets re-served: archiving a result must
    // remove it from the results — v1's regression #4, same hole.
    if (results !== null) {
      const q = search.trim();
      if (q.length >= 3) runSearch(q);
    }
  }

  // E7 (PLAN-AUDIT-V3, closes D-48): the view subscribes to the shared
  // invalidation signal — one line, the wiring lives in views.svelte.js.
  watchViews(reload);

</script>


<section class="column" class:center={center} aria-label={t('list.aria')} data-testid="list">
  <!-- UI v3, E1 (CE verdict 2026-08-16): the banner of the Classic
       mockup — the current mailbox's name, ALONE ("Mark all as read"
       ruled out). The mailbox.* keys are the nav's own. -->
  {#if checkedRows.size > 0}
    <!-- R1/D3: the list bar TRANSFORMS as long as the selection is
         not empty — the account, the four bulk gestures (D1), Cancel.
         No new surface: same 52 px, same net. Icon buttons at the
         header's grammar (32 px), the label lives in aria-label AND
         title. In Junk, "Report as junk" gives way to "Not spam" —
         the mirror of the reading pane. -->
    <header class="banner banner-selection" data-testid="bar-selection">
      <h1>{t('list.nSelection', { n: checkedRows.size })}</h1>
      {#each BAR_GESTURES as g (g.action)}
        <button type="button" class="btn-bar" data-testid="bar-{g.action}"
                disabled={gestureInProgress}
                aria-label={t(g.label)} title={t(g.label)}
                onclick={() => act(g.action)}><Icon name={g.icon} /></button>
      {/each}
      <!-- Cancel also freezes during the batch: a bar that would
           collapse while the commands are still running would read
           as a cancellation (review). -->
      <button type="button" class="btn-bar" data-testid="bar-cancel"
              disabled={gestureInProgress}
              aria-label={t('action.cancelSelection')} title={t('action.cancelSelection')}
              onclick={clearSelection}><Icon name="close" /></button>
    </header>
  {:else if organizedInboxView}
    <!-- RETOURS-14 R2 (D2/D3): the organized Inbox takes the mode's
         normalized header (shared .header-view classes from
         system.css, the Feed/Screener pattern R7/R11) — title alone
         (D2), no generic banner nor tabs (D3, further down). -->
    <header class="head-organized" data-testid="list-title">
      <h2 class="display header-view" data-testid="inbox-title">
        <span class="glyph-title" aria-hidden="true"><Icon name="inbox" size={26} /></span>{t(mailboxLabelKey('inbox'))}</h2>
    </header>
  {:else}
    <header class="banner" data-testid="list-title">
      <!-- RETOURS-13 R3: the label comes out of THE shared rule. -->
      <h1>{t(mailboxLabelKey(category))}</h1>
    </header>
  {/if}
  <!-- A83: the spacing notch is set AS A TOKEN on the frame — the
       five `.ligne` instances (probes, waiting, stream, pinned,
       drafts) sit below it and take it all at once, probes included.
       The pattern is that of the pane widths (`--l-nav`); the hyphen
       lets it escape the contract of the 17 theme tokens, and that is
       deliberate: it is a page-layout dimension, not a color. -->
  <div class="frame" bind:this={frame} bind:clientHeight={frameHeight}
       class:selection-in-progress={checkedRows.size > 0}
       onscroll={onScroll}
       style="--rangee-pad:{rowPad()}px">
    <!-- RETOURS-14 R2: the current section, stuck at the top of the
         frame. `height:0`: the band lives OUTSIDE the windowing
         geometry (decalage/indexPour don't know it — the E4 trap of
         the spacers therefore doesn't concern it). -->
    {#if stuckSection}
      <div class="stuck-section" data-testid="stuck-section" aria-hidden="true">
        <span class="header-frame"><span class="lab">{stuckSection.label}</span></span>
      </div>
    {/if}
    <!-- A81: the probes follow the real row — no more tile column; a
         probe rendering a dead object would lie about the geometry.
         A83: they stay MOUNTED and re-measure themselves
         (`bind:offsetHeight` compiles to a ResizeObserver, the
         pattern of `pinsTopMeasured`). Before, they were removed
         after a one-off measurement and `sondees` was never reset to
         false: a notch change would have redrawn the rows at the new
         height while leaving the templates frozen at the old one —
         scrollbar off by 13.6% to 27.3%, and up to 12,000 px of gap on
         a jump (measured, PLAN-ESPACEMENT §3). Mounted permanently,
         the bug class is IMPOSSIBLE, not fixed.
         The cage is POSITIONED, and that's not decorative: without its
         `position:relative` it is not the containing block of the
         `position:absolute` probes, which then anchor to `.cadre` and
         add it up to 85 px of PHANTOM scroll on a short window
         (measured at the bench, variant C).
         Field 2026-09-02 (STOP 2 of wave 2, pass 2): the probe carries
         EVERYTHING that gives its height to the real header row,
         under the same conditions — the mailbox block (mixed view)
         and the organized mode's ⋯ (24 px centered in a 14 px row).
         Without them, 6 px less per row: after twenty rows, the
         "Already seen" band overlapped an entire row. -->
    <div class="probes-cage" aria-hidden="true">
      <div class="probes">
        <article class="row" bind:offsetHeight={h1}>
          <div class="l1">
            <span class="sender">Sonde</span>
            {#if mixedView(account, results !== null)}
              <span class="mailbox"><span class="word">{t('list.on')}</span>
                <span class="bare-marker" aria-hidden="true"><Icon name="work" size={14} /></span>
                <span class="lbl">Sonde</span></span>
            {/if}
            <span class="grow"></span>
            {#if organizedGestures}
              <button type="button" class="gestures" tabindex="-1"><Icon name="more_horiz" size={14} /></button>
            {/if}
            <span class="time">00:00</span>
          </div>
          <p class="subject">Sonde</p>
          <p class="preview">Sonde</p>
        </article>
        <article class="row" bind:offsetHeight={h2}>
          <div class="l1">
            <span class="sender">Sonde</span>
            {#if mixedView(account, results !== null)}
              <span class="mailbox"><span class="word">{t('list.on')}</span>
                <span class="bare-marker" aria-hidden="true"><Icon name="work" size={14} /></span>
                <span class="lbl">Sonde</span></span>
            {/if}
            <span class="grow"></span>
            {#if organizedGestures}
              <button type="button" class="gestures" tabindex="-1"><Icon name="more_horiz" size={14} /></button>
            {/if}
            <span class="time">00:00</span>
          </div>
          <p class="subject">Sonde</p>
          <p class="preview">Sonde</p>
          <div class="chips"><span class="chip"><Icon name="forum" />2</span></div>
        </article>
      </div>
    </div>
    {#snippet waiting()}
      <article class="row pending" data-testid="row-pending">
        <div class="l1"><span class="sender">…</span><span class="grow"></span><span class="time"></span></div>
        <p class="subject">…</p>
        <p class="preview"></p>
      </article>
    {/snippet}
    {#snippet listRow(row, pinned = false)}
      <!-- A80: the mailbox block lives everywhere accounts MIX — the
           unified mailbox (D3/D7) and search (always multi-account,
           even from a single account's view; review 2026-08-22) — and
           on ALL rows, marker or not (D8). -->
      {@const mailbox = mailboxOf(row)}
      {@const checked = isChecked(row)}
      <!-- R1: the click lives in three regimes (rowClick) — Ctrl/Cmd
           toggles the checkbox, Shift extends from the anchor, bare =
           choose (A38's note on focus lives in rowClick). The
           mousedown swallows the TEXT selection of a Shift-click —
           never the gesture. -->
      <div class="row"
           class:unread={row.thread_unseen > 0}
           class:chosen={isChosen(row)}
           class:checked={checked}
           data-testid="row"
           role="button" tabindex="0"
           onmousedown={(e) => { if (e.shiftKey) e.preventDefault(); }}
           onclick={(e) => rowClick(e, row)}
           onkeydown={activation(() => choose(row))}>
        <!-- R1/D4: the checkbox — absolute in the left gutter, the
             row's geometry NEVER moves (the h1/h2 probes measure the
             row without it); opacity 0 at rest, revealed on hover and
             as soon as a selection exists (CSS). tabindex -1: the
             keyboard check goes through Enter/Space on the chosen
             row, the checkbox is a pointer affordance. -->
        <button type="button" class="checkbox" data-testid="row-checkbox"
                role="checkbox" aria-checked={checked} tabindex="-1"
                aria-label={t('list.check')}
                onclick={(e) => {
                  e.stopPropagation();
                  // The A38 guard also applies here: a click doesn't
                  // leave focus on a button of a recycled node.
                  e.currentTarget.blur();
                  toggle(row);
                }}>
          {#if checked}<Icon name="check" size={12} />{/if}
        </button>
        <!-- A81: the initials tile has left the list — the name in
             full letters already said what it said. A80: the header
             row carries the mailbox block in its place, INLINE — no
             reserved column, its absence shifts nothing (D7). The
             drawing is aria-hidden: it DOUBLES the word; the tooltip
             gives "label — address". -->
        <div class="l1">
          <!-- V4: unread is said by the 9 px dot AND the font weight
               (A8 — never color alone); the pinned row carries the
               keep mark on its --tile ground (A73). -->
          {#if row.thread_unseen > 0}<span class="disk"></span>{/if}
          {#if pinned}<span class="brand-pin" aria-hidden="true"><Icon name="keep" size={14} /></span>{/if}
          <span class="sender">{#if toSend(row)}{t('list.dest', { a: contact(row) })}{:else}{row.sender}{/if}</span>
          {#if mailbox}
            <span class="mailbox" data-testid="row-mailbox" title={mailbox.title}>
              <span class="word">{t('list.on')}</span>
              {#if mailbox.marker}
                <span class="bare-marker" data-hue={mailbox.marker.hue}
                      aria-hidden="true"><Icon name={mailbox.marker.icon} size={14} /></span>
              {/if}
              <span class="lbl">{mailbox.label}</span>
            </span>
          {/if}
          <span class="grow"></span>
          {#if organizedGestures}
            <!-- E4: the ⋯ to the LEFT of the time, RESERVED place —
                 opacity only, the geometry never moves. -->
            <button type="button" class="gestures" data-testid="row-gestures"
                    aria-label={t('list.gestures')} aria-haspopup="menu"
                    aria-expanded={gestureMenu?.key === rowKey(row)}
                    onclick={(e) => openGestures(e, row)}>
              <Icon name="more_horiz" size={14} /></button>
          {/if}
          <span class="time">{when(row.epoch)}</span>
        </div>
        <p class="subject">{row.subject}</p>
        {#if draftOf(row)}
          <!-- Variant B (PLAN-BROUILLONS §3): the preview shows the
               draft — prefix and body; the rest of the row doesn't
               move. -->
          <p class="preview"><span class="prefix" data-testid="mention-draft">{t('list.draftPrefix')}</span>{draftOf(row).body}</p>
        {:else}
          <p class="preview">{row.preview ?? ''}</p>
        {/if}
        <!-- PLAN-RETOURS-V3 R1 (CE verdict 2026-08-16, D1/D2): A29's
             "bare row" is reversed — the prototype's chip rank
             returns to the row, under the Thread header's rules ("N
             messages" if the thread has more than one, "N files" if
             attachments). Height TO CONTENT (CE field 2026-08-16,
             reverses D1): the rank only exists on carrying rows and
             enlarges their row — two templates, the windowing
             corrects via chipsBefore. In
             SEARCH, a result is a message, not a conversation (the
             core serves thread_size=1 without joining threads): the
             thread chip does not appear there, by construction. The
             attachment count is the one from BEFORE the body is read:
             0 as long as the body hasn't been backfilled — the chip
             appears as the backfill progresses, never wrongly. -->
        <!-- R10/R3'c (field 2026-08-23): an invitation's GESTURES
             occupy a rank of their own — icon said by color AND by
             text (A8), the chip acts at the instant of the click
             (optimistic). -->
        {#if invitationGestures(row)}
          <div class="chips" data-testid="chips-invitation">
            <button type="button" class="chip tone-accepted" data-testid="list-accept"
                    disabled={invitationReplies[`${row.account_id}/${row.invitation.mailbox}/${row.invitation.uid}`]}
                    onclick={(e) => replyInvitation(e, row, 'accepted')}>
              <Icon name="check_circle" />{t('action.accept')}</button>
            <button type="button" class="chip tone-tentative" data-testid="list-tentative"
                    disabled={invitationReplies[`${row.account_id}/${row.invitation.mailbox}/${row.invitation.uid}`]}
                    onclick={(e) => replyInvitation(e, row, 'tentative')}>
              <Icon name="question_mark" />{t('action.tentative')}</button>
            <button type="button" class="chip tone-declined" data-testid="list-refuse"
                    disabled={invitationReplies[`${row.account_id}/${row.invitation.mailbox}/${row.invitation.uid}`]}
                    onclick={(e) => replyInvitation(e, row, 'declined')}>
              <Icon name="cancel" />{t('action.decline')}</button>
          </div>
        {/if}
        {#if otherChips(row) || (row.invitation && invitationChip(row.invitation))}
          <div class="chips" data-testid="chips-row">
            <!-- R11: the given reply (or the cancellation) joins the
                 common rank — the other chips rise up with it. -->
            {#if row.invitation && invitationChip(row.invitation)}
              {@const chip = invitationChip(row.invitation)}
              <span class="chip tone-{chip.tone}" data-testid="invitation-chip">
                {#if chip.icon}<Icon name={chip.icon} />{/if}{chip.text}</span>
            {/if}
            {#if row.pinned}
              <span class="chip"><Icon name="keep" />{t('chip.pin')}</span>
            {/if}
            {#if row.thread_size > 1}
              <span class="chip"><Icon name="forum" />{t('chip.messages', { n: row.thread_size })}</span>
            {/if}
            {#if row.attachment_count > 0}
              <span class="chip"><Icon name="attach_file" />{t('chip.files', { n: row.attachment_count })}</span>
            {/if}
          </div>
        {/if}
      </div>
    {/snippet}
    {#if results !== null}
      <div class="window-search" data-testid="results">
        {#if results.length === 0}
          <div class="empty-search"><p>{t('list.noResult')}</p></div>
        {/if}
        {#each results as row (`${row.account_id}/${row.mailbox}/${row.uid}`)}
          {@render listRow(row)}
        {/each}
        {#if results.length > 0 && results.length < resultsTotal}
          {#if results.length < MAX_RESULTS}
            <button type="button" class="load-more" data-testid="load-more"
                    disabled={loadingMore} onclick={loadMore}>
              {t('list.loadMore', { n: Math.min(BATCH, resultsTotal - results.length) })}
            </button>
          {:else}
            <p class="refine" data-testid="refine">{t('list.refine')}</p>
          {/if}
        {/if}
      </div>
    {:else if draftRows !== null}
      <!-- The Drafts folder (B-D1): the local drafts, from most
           recent to oldest. The click RESUMES — never mark_seen,
           there is nothing to read here, only to finish. -->
      <div class="window-search" data-testid="folder-drafts">
        {#if draftRows.length === 0}
          <div class="empty-search"><p>{t('list.empty')}</p></div>
        {/if}
        {#each draftRows as b (b.id)}
          <!-- A81: the Drafts folder KEEPS its tile (D9 — it shows the
               recipient there): the `tuilee` class gives it back the
               head column that the list row lost. -->
          <div class="row tiled" data-testid="row-draft"
               role="button" tabindex="0"
               onclick={() => onresume(b)}
               onkeydown={activation(() => onresume(b))}>
            <span class="avatar" aria-hidden="true">{initials(b.to)}</span>
            <div class="l1">
              <span class="sender" class:without={!b.to}>
                {b.to ? t('drafts.to', { a: b.to }) : t('drafts.withoutRecipient')}</span>
              <!-- The spacer pushes the time to the right edge: since
                   A80, .exp no longer grows (flex:0 1 auto), it is
                   the spacer that carries the spring — here as in the
                   stream's row. -->
              <span class="grow"></span>
              <span class="time">{when(Math.floor(b.updated_epoch / 1000))}</span>
            </div>
            <p class="subject" class:without={!b.subject}>{b.subject || t('drafts.withoutSubject')}</p>
            <p class="preview">{b.body}</p>
          </div>
        {/each}
      </div>
    {:else}
      <!-- R4: the PINNED section — the same rows, placed ahead of the
           stream in the same scroll; the stream excludes them (D5).
           Its measured height recalibrates the windowing below. -->
      {#if pins.length > 0}
        <div class="pins" data-testid="pins" bind:offsetHeight={pinsTopMeasured}>
          {#each pins as row (key(row))}
            {@render listRow(row, true)}
          {/each}
        </div>
      {/if}
      {#if total === 0 && sourceAnswered && answeredPins && pins.length === 0}
        <!-- BOTH sources answered zero: emptiness is PROVEN, without
             counting (a short page states the total on its own).
             All-pinned: the stream is empty but the mailbox isn't —
             the section alone, nothing to assert below it. -->
        <div class="empty"><p>{t('list.empty')}</p></div>
      {:else if total === 0 && !(sourceAnswered && answeredPins)}
        <!-- The current source hasn't answered yet: the waiting state
             shows, emptiness is never asserted without proof
             (PLAN-DEFILEMENT-PROFOND E2). -->
        <div class="window-search" data-testid="pending-source">
          {#each Array.from({ length: 6 }) as _, i (i)}
            {@render waiting()}
          {/each}
        </div>
      {/if}
      <div class="space" style="height:{spaceHeight}px">
        {#each headerPositions as e (e.index)}
          <!-- E4: the section header lives OUTSIDE the rows, absolute
               within the space — the rows' geometry stays uniform,
               the offset is carried by decalage/indexPour. The
               positions come from a derived value that LISTENS to
               `version` (review E5): an invitation chip that pushes
               the rows recalibrates the header in the same flush. -->
          <div class="header-section" data-testid="section"
               style="top:{e.top}px">
            <span class="header-frame"><span class="lab">{e.label}</span></span>
          </div>
        {/each}
        <div class="window" style="transform:translateY({offset(start)}px)">
          {#each window as { i, row } (i)}
            <!-- E4: the header band occupies 34 px REAL in the flow
                 (the rows stack in flex — an offset that would only
                 live in decalage/indexPour would make the header
                 overlap and the window drift, a capture finding).
                 When the window STARTS at the boundary, the band is
                 already in the translateY (headersBefore counts
                 e.index <= i). -->
            {#if headers.some((e) => e.index === i && e.index > start)}
              <div class="header-space" aria-hidden="true"></div>
            {/if}
            {#if row}
              {@render listRow(row)}
            {:else}
              {@render waiting()}
            {/if}
          {/each}
        </div>
      </div>
    {/if}
  </div>
  <!-- RETOURS-14 R2 (D3): the organized Inbox has no footer — the
       tabs (and their All / Unread filter) belong to the classic
       view; Drafts stays accessible from the nav. -->
  {#if !organizedInboxView}
  <div class="tabs" data-testid="tabs">
    {#each TABS as o (o.id)}
      <span class="tab" class:active={tabActive === o.id}
            data-testid="tab" data-tab={o.id}
            role="button" tabindex="0" aria-pressed={tabActive === o.id}
            onclick={() => ontab(o.id)}
            onkeydown={activation(() => ontab(o.id))}>
        <Icon name={o.icon} />{t(o.label)}
      </span>
    {/each}
  </div>
  {/if}
</section>

<!-- PLAN-AUDIT-V2 E11: THE product's menu (Menu.svelte) — keyboard,
     focus, closing; the List only supplies its items. -->
<Menu isOpen={gestureMenu !== null} x={gestureMenu?.x ?? 0} y={gestureMenu?.y ?? 0}
      testid="menu-gestures" onclose={() => (gestureMenu = null)}>
    {#each ['inbox', 'feed', 'paper_trail'].filter((d) => d !== category) as dest (dest)}
      <button type="button" role="menuitem" data-testid={`gestures-${dest}`}
              onclick={() => gesture(dest)}>
        <Icon name={dest === 'inbox' ? 'inbox' : dest} />{t('list.moveTo', { mailbox: t(`mailbox.${dest}`) })}</button>
    {/each}
    <div class="net-menu"></div>
    <button type="button" role="menuitem" data-testid="gestures-aside"
            onclick={() => {
              const { row } = gestureMenu;
              gestureMenu = null;
              onsetaside(row);
            }}>
      <Icon name="pile" />{t('pile.put')}</button>
    <div class="net-menu"></div>
    <button type="button" role="menuitem" data-testid="gestures-screen-out"
            onclick={() => gesture('screened_out')}>
      <Icon name="visibility_off" />{t('list.screenOut')}</button>
</Menu>

<style>
  /* Geometry and states of the track drawing (A29/A30): continuous
     rows separated by a net, no card, no shadow. */
  .column {
    display:flex; flex-direction:column; min-height:0;
    background:var(--bg); border-right:1px solid var(--border);
  }
  /* The banner (UI v3, E1 — reworked at PLAN-RETOURS-V3 R2): the SAME
     visual format as the bottom filter banner — 52 px, --bg
     background (V3: the net alone carries the separation); title
     16 px 600. */
  .banner {
    flex:none; height:52px; display:flex; align-items:center;
    padding:0 16px; background:var(--bg);
    border-bottom:1px solid var(--border);
  }
  .banner h1 {
    margin:0; font-size:16px; font-weight:600; line-height:1.3;
    color:var(--ink); flex:1; min-width:0;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
  }
  /* `isolation` (RETOURS-14 R2, review): the stuck section band
     carries a z-index — confined HERE, otherwise it would pass above
     the modal veils (z-index 2) of the root context. */
  .frame { flex:1; overflow:auto; position:relative; isolation:isolate; }
  .space { position:relative; }
  /* RETOURS-14 R2: the organized Inbox's normalized header — same
     page geometry as Feed/Screener (24/28 margin), no net: the view
     is a page of the mode, not a tool banner. */
  .head-organized { flex:none; padding:24px 28px 0; }
  .head-organized .header-view { max-width:760px; margin-inline:auto; }
  /* The stuck current section: NULL height (outside the windowing
     geometry), the label painted over the rows on an opaque
     background — the drawing of the real band (.entete-section). */
  .stuck-section {
    position:sticky; top:0; z-index:3; height:0; overflow:visible;
  }
  .stuck-section .header-frame {
    display:block; background:var(--bg);
    padding:10px 16px 6px; border-bottom:1px solid var(--border);
  }
  /* E4: the section header — the drawing of the Screener's
     rule-label (bare label, uppercase, dimmed ink), anchored to the
     bottom of its 34 px band, the first rank's net acts as a
     separator. */
  .header-section {
    position:absolute; left:0; right:0; height:52px;
    display:flex; align-items:flex-end; padding:0 16px 6px;
  }
  /* The inner frame carries the centering: the auto-margin of an
     over-constrained absolute is fragile — a block in flow isn't. */
  .header-frame { display:block; width:100%; }
  .header-space { flex:none; height:52px; }
  .header-section .lab, .stuck-section .lab {
    font-size:11px; letter-spacing:.1em; text-transform:uppercase;
    color:var(--muted); font-weight:600; white-space:nowrap;
  }
  /* E4: the organized Inbox's centered column (~760 px of the
     prototype) — rows and headers together, to the pixel. */
  /* `width:100%` first: in a flex column, a cross-axis auto-margin
     TURNS OFF the stretch — without it, the row shrinks to its
     content (E4 capture finding). */
  .center :global(.row) {
    width:100%; max-width:760px; margin-inline:auto; box-sizing:border-box;
  }
  .center .header-frame { max-width:760px; margin-inline:auto; }
  /* E4: the ⋯ gesture menu — RESERVED place to the left of the time
     (24 px), opacity only: the row's geometry never moves. */
  .gestures {
    flex:none; width:24px; height:24px; padding:0;
    display:inline-flex; align-items:center; justify-content:center;
    align-self:center; opacity:0; color:var(--muted);
    background:none; border:1px solid transparent;
  }
  .row:hover .gestures, .gestures:focus-visible, .gestures[aria-expanded="true"] {
    opacity:1;
  }
  .gestures:hover, .gestures[aria-expanded="true"] {
    background:var(--hover); border-color:var(--border); color:var(--ink);
  }
  .window {
    position:absolute; top:0; left:0; right:0;
    display:flex; flex-direction:column;
  }
  /* A83 — the probes' cage. `position:relative` is THE line that
     matters: it makes the cage the containing block of the probes,
     which are then clipped by `height:0; overflow:hidden` and drop
     out of the frame's scrolling region. Without it, the probes
     anchor to `.cadre` (also positioned) and add it up to 85 px of
     phantom scroll on a short window — measured at the bench
     (spikes/espacement/sondes.mjs, variants B and C). */
  .probes-cage { position:relative; height:0; overflow:hidden; }
  .probes { position:absolute; visibility:hidden; left:0; right:0; }
  .empty {
    position:absolute; inset:0; display:flex; align-items:center;
    justify-content:center; padding:40px; text-align:center;
  }
  .empty p { margin:0; font-size:13px; line-height:1.5; color:var(--muted); }
  .window-search { display:flex; flex-direction:column; }
  .empty-search { padding:40px; text-align:center; }
  .empty-search p { margin:0; font-size:13px; line-height:1.5; color:var(--muted); }
  /* "Load more": a discreet button centered under the results (pair
     ink/surface, hover ink/sel — validated by the contrast gate).
     Beyond the soft cap, the refine prompt replaces it (dimmed ink,
     like the empty state). */
  .load-more {
    align-self:center; margin:12px 0 20px; height:32px; padding:0 18px;
    display:inline-flex; align-items:center; font-size:13px; font-weight:600;
    color:var(--ink); background:var(--surface); border:1px solid var(--border);
    border-radius:var(--r-control); cursor:pointer;
  }
  .load-more:hover { background:var(--sel); }
  .load-more:disabled { opacity:.6; cursor:default; }
  .refine {
    margin:0; padding:16px 40px 24px; text-align:center;
    font-size:13px; line-height:1.5; color:var(--muted);
  }

  /* Four states (A30): rest transparent, hover in a light tint,
     selection in a tint + 2 px accent outline — never a shadow nor a
     white surface (A29). The outline is reserved in transparent: the
     selection doesn't shift the content. */
  /* A81: the head column (initials tile) has left the list row — the
     grid is ONE column, the content takes the full width. The chip
     rank (A44, field: height to content) only exists on carrying
     rows; BOTH heights are probed (h1/h2). The Drafts folder keeps
     its tile (D9): the `tuilee` class gives it back the head
     column. */
  .row {
    /* A83: the vertical air comes from the notch (--rangee-pad, set
       on the frame); 13 px stays the default, the existing value to
       the pixel. The fallback covers rows rendered outside the frame,
       should one arise. */
    padding:var(--rangee-pad, 13px) 16px; border-top:1px solid var(--border);
    border-left:2px solid transparent;
    display:grid; grid-template-columns:1fr;
    row-gap:3px; align-items:start; cursor:pointer;
    /* R1: the checkbox's containing block (absolute) — no effect on
       the geometry measured by the probes. */
    position:relative;
  }
  .row.tiled { grid-template-columns:auto 1fr; column-gap:10px; }
  .avatar {
    grid-row:1 / span 3; width:28px; height:28px;
    border-radius:var(--r-tile);
    background:var(--tile); border:1px solid var(--border);
    display:grid; place-items:center;
    font-size:11px; font-weight:600; color:var(--tileInk);
  }
  .l1, .subject, .preview, .chips { grid-column:1; min-width:0; }
  .tiled .l1, .tiled .subject, .tiled .preview, .tiled .chips { grid-column:2; }
  /* The chip rank (PLAN-RETOURS-V3 R1): the 24 px template of the
     Classic prototype — present only on carrying rows. */
  .chips {
    height:24px; display:flex; align-items:center; gap:6px;
    overflow:hidden;
  }
  .chip {
    display:inline-flex; align-items:center; gap:5px; height:24px;
    padding:0 9px; font-size:12px; color:var(--ink2);
    background:var(--surface); border:1px solid var(--border);
    border-radius:var(--r-control); white-space:nowrap;
  }
  .chip :global(.ic) { width:14px; height:14px; }
  /* R10: the invitation gestures of the rank — the chip that ACTS. */
  button.chip { cursor:pointer; }
  button.chip:hover:not(:disabled) { background:var(--sel); }
  button.chip:disabled { cursor:default; opacity:.55; }
  /* R9: color says the reply's meaning — carried by the ICON (text
     doubles it, A8), using the System's tokens: accept in accent,
     decline in alert, tentative neutral. Pairs already gated
     (accent/surface 3:1, alert/surface 3:1, and their --sel
     counterparts). */
  .chip.tone-accepted :global(.ic) { color:var(--accent); }
  .chip.tone-tentative :global(.ic) { color:var(--muted); }
  .chip.tone-declined :global(.ic) { color:var(--alert); }
  .chip.tone-cancelled { color:var(--alert); }
  .row:hover { background:var(--hover); }
  .row.chosen {
    background:var(--sel); border-left-color:var(--accent);
  }
  /* R1: the CHECKED row takes the selection tint, without the
     outline (the outline stays the reading position — two ideas, two
     drawings). Field verdict 2026-08-27 (R1-7): PINNED rows take it
     TOO — the checkbox is the only state that displaces A73's --tile
     ground, because it precedes a bulk gesture: what the eye doesn't
     count can leave by surprise. */
  .row.checked { background:var(--sel); }
  /* R1/D4 — the checkbox: absolute in the left gutter (16 px padding
     + 2 px reserved outline: it doesn't enter the grid, the probed
     h1/h2 templates don't see it). Invisible at rest (opacity only —
     it stays blindly clickable in the gutter, and the geometry never
     reflows); revealed on hover over ITS row, and on all rows as soon
     as a selection exists. */
  /* Field 2026-08-27 (R1-3): the checkbox breathes — 8 px from the
     edge, 16 px box, and the CONTENT moves out to 34 px when the
     checkbox shows (hover over ITS row, a checked row, or selection
     mode — there, all rows move out as one block, nothing "jumps"
     while checking). The shift lives in the padding: the rows'
     height doesn't move, the h1/h2 probes stay accurate. The Drafts
     folder (.tuilee) has no checkbox, it doesn't move out. */
  .checkbox {
    position:absolute; left:8px; top:calc(var(--rangee-pad, 13px) + 1px);
    width:16px; height:16px; padding:0;
    display:inline-flex; align-items:center; justify-content:center;
    background:var(--bg); border:1px solid var(--border);
    border-radius:var(--r-control); color:var(--accent);
    cursor:pointer; opacity:0;
  }
  .row:hover .checkbox,
  .selection-in-progress .checkbox,
  .checkbox[aria-checked="true"] { opacity:1; }
  .row:not(.tiled):hover,
  .row.checked,
  .selection-in-progress .row:not(.tiled) { padding-left:34px; }
  /* The transformed bar (D3): same 52 px as the banner, 32 px buttons
     from the header's grammar. */
  .banner-selection { gap:4px; }
  .banner-selection h1 { font-size:14px; }
  .btn-bar {
    flex:none; width:32px; height:32px; padding:0;
    display:inline-flex; align-items:center; justify-content:center;
    background:none; border:1px solid transparent;
    border-radius:var(--r-control); color:var(--ink2); cursor:pointer;
  }
  /* Hover in --sel: the token of the header's grammar (.btn-tiroir,
     .btn-statut) — never a second convention (review). */
  .btn-bar:hover:not(:disabled) { background:var(--sel); color:var(--ink); }
  .btn-bar:disabled { opacity:.55; cursor:default; }
  /* A73, field 2026-08-21: the PINNED row takes the drawing of the
     current mailbox's tile (nav, W2-D5) — --tile background,
     --tileInk ink (pair already measured by the gate): it stands out
     from the stream at first glance. The tint holds through hover
     (the tile has no hover state); the selection keeps its accent
     outline. */
  .pins .row,
  .pins .row:hover,
  .pins .row.chosen { background:var(--tile); }
  .pins .row.chosen { border-left-color:var(--accent); }
  /* Field verdict 2026-08-27 (R1-7): the CHECKBOX displaces the
     --tile ground — declared AFTER the block above to also win on a
     row that is both chosen and checked (same specificity, order
     decides). */
  .pins .row.checked { background:var(--sel); }
  .pins .row .sender,
  .pins .row .subject,
  .pins .row .preview,
  .pins .row .time { color:var(--tileInk); }
  /* A73 holds for the WHOLE row: the mailbox block (A80) takes the
     warm ink like its neighbors — without this rule it kept its two
     cold grays (--ink2/--muted) on the --tile ground, the only cold
     island in the row (review). The drawing, itself, keeps the
     account's hue: that is its identity, and its pair on --tile is
     measured. */
  .pins .row :global(.mailbox),
  .pins .row :global(.mailbox .word),
  .pins .row :global(.mailbox .lbl) { color:var(--tileInk); }
  /* A80 — the header row: gap 6 (the mailbox block adds two gutters;
     at 10 the row lost 12 px for nothing). THE TRUNCATION ORDER IS
     THE DESIGN: the time never gives way (flex:none), the block
     (.boite, system.css) gives way three times faster than the
     sender, the spacer absorbs the slack. */
  .l1 { display:flex; align-items:baseline; gap:6px; }
  .l1 :global(.disk), .l1 .brand-pin { align-self:center; }
  .brand-pin { color:var(--tileInk); display:inline-flex; }
  .sender {
    font-size:14px; color:var(--ink); flex:0 1 auto; min-width:0;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
  }
  .grow { flex:1 1 0; min-width:0; }
  .unread .sender { font-weight:700; }
  .time { font-size:12px; color:var(--muted); flex:none; }
  .subject {
    /* 14 px (A29 — amendment A9): the tracks' template. */
    margin:0; font-size:14px; font-weight:400; line-height:1.3;
    color:var(--ink);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
  }
  .unread .subject { font-weight:700; }
  .preview {
    margin:0; font-size:13px; line-height:1.45; color:var(--ink2);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
    min-height:1.45em;
  }
  /* The "Draft: " mention (variant B, PLAN-BROUILLONS §3): the alert
     token in text form — measured by contrast.mjs on the three row
     backgrounds (rest, hover, chosen). */
  .prefix { color:var(--alert); font-weight:600; }
  /* Empty fields of the folder: dimmed italics say it, never a blank
     ("(no subject)", "(no recipient)"). */
  .without, .subject.without { font-style:italic; color:var(--muted); font-weight:400; }
  .pending { color:var(--muted); }

  .tabs {
    flex:none; height:52px; padding:0 12px; display:flex;
    align-items:center; gap:10px; border-top:1px solid var(--border);
    background:var(--bg);
  }
  .tab {
    height:32px; padding:0 14px; display:inline-flex; align-items:center;
    gap:8px; font-size:13px; border-radius:var(--r-control); cursor:pointer;
    color:var(--ink2); background:var(--surface);
    border:1px solid var(--border);
  }
  .tab:hover { background:var(--hover); }
  .tab.active {
    font-weight:600; color:var(--ink); background:var(--sel);
    border-color:var(--accent);
  }
</style>

# Spike D-53 — Feed memory, options A / B / C measured

PLAN-AUDIT-2026-09-LOT5 § 3.2 (E15b). Throw-away spike, STANDARD §2.2-2.3.
No conclusion on which option wins: the figures go to the main session
and the Chief Engineer (D6). 2026-09-07, worktree at `2ade544`.

## 1. Options as built

| Option | What was measured | S1 (sandboxed iframe, no scripts) |
|---|---|---|
| A | `Feed.svelte` as shipped at `2ade544` — window ±5 (up to 11 live iframes), one `srcdoc` iframe per card, dormant block of the measured height outside the window. `patches/A.patch` (empty). | kept |
| B | Window ±1 (3 live iframes). A leaving iframe is navigated to `about:blank` (`srcdoc` removed, `src = 'about:blank'`, height frozen) one frame BEFORE it unmounts; the same blank runs in an action teardown for every other unmount (Feed left, card collapsed). `autoBody` ignores the blank load so the column does not collapse for a frame. `patches/B.patch` (Feed.svelte + lib/body.js, 80 lines). | kept — same `sandbox="allow-same-origin"`, no `allow-scripts` |
| C | Window ±2 (5 slots). A POOL of at most 5 iframes created once in `.column` (position: relative), never moved (a reparented iframe reloads in Chromium), absolutely positioned over a per-card `.body-slot` block of the measured height. Entering a slot swaps the pooled iframe's `srcdoc`; a released iframe is navigated to `about:blank` and hidden. `patches/C.patch` (Feed.svelte only, 131 lines). | kept — pooled iframes carry the same `sandbox` attribute; `wireLinks` re-armed on every load |
| D | Bodies in the page DOM — REFUSED before measurement (S1, A37). Not built. | — |

B and C compile under Vite without warnings (`raw/npm-build-check-{B,C}.log`)
and render (screenshots; § 6 for C's defect).

## 2. Protocol

- **Machine**: Snapdragon X X1E80100 (12 cores, Windows arm64), 15.6 GB
  RAM, Adreno X1-85 GPU, Windows 11 10.0.26200, **WebView2 152.0.4191.62**.
  This is NOT the CE's x64 workstation of D-53 pass 2 (where the GPU
  process carried 132 MB of 251): here the GPU process stays at ~11 MB
  in every phase and the renderer carries the load. The same-machine
  comparison A/B/C is valid; the absolute figures do not transfer to the
  workstation without a run there.
- **Build**: debug (`cargo build -p wind-desktop`, rustc 1.97.1), ONE
  worktree, ONE target dir, the desktop crate built once
  (`raw/cargo-build-desktop*.log`); per option only the ui-v2 dist is
  rebuilt (`npm run build`, VITE_E2E=1 — the e2e seam flavour, as the
  bench) and re-embedded by `e2e/rebuild-v2.mjs` (fingerprint bump →
  relink of the desktop crate, ~10 s). Svelte 5, Vite 7.3.6.
- **Fixture**: the bench's own — `seed_inbox 200 principal@exemple.fr 200 <ko>`,
  200 letters with synthetic plain-text bodies of 100 KB (series 1) and
  300 KB (series 2), 16 senders routed to the Feed → **160 cards = 8
  pages of 20** (the routed decor caps at 160; "ten pages" below means
  "scrolled to exhaustion, 160 cards").
- **Gesture** (`bench.mjs`, through `e2e/launch.mjs`, CDP): rest 8 s after
  the first row → organized mode → route 16 senders → Feed → (a) first
  page +5 s → scroll `last card into view` until the count stops growing
  (no growth for 4 s ends it in runs 1-2, 12 s in run 3), 700 ms between
  pages → +8 s → (b) → Inbox → +8 s → (c1) → +25 s → (c2) → forced GC in
  the renderer (`HeapProfiler.collectGarbage`, CDP) → +5 s → (c3, diagnostic).
- **Meter**: `e2e/measure-ram.ps1 -AppPid <pid> -Profil target\e2e\webview2`
  — private working set, wind-desktop.exe + the 6 WebView2 processes of
  THAT profile only (7 processes in every sample). `ram-detail.ps1` gives
  the per-process split of the same set. `performance.memory.usedJSHeapSize`
  recorded at each phase from 100 KB run 3 on and on every 300 KB run.
- **Scroll cost**: a rAF loop in the page during the scroll (frame
  intervals; long frame = > 50 ms), plus the wall time from each
  `scrollIntoView` to the next page's arrival.
- **Repetitions**: 3 runs per option at 100 KB, 3 at 300 KB (B 300 KB
  run 1 died mid-scroll, § 5.8; 300 KB runs 1-2 stopped short of 160
  cards, § 4). Each run is a fresh process and a fresh copy of the
  seeded database, same WebView2 profile (HTTP cache purged by the
  launcher).
- **What else ran**: this Claude Code session (4 processes, ~1.3 GB),
  Defender (MsMpEng ~350 MB), Edge, Spark Desktop, OneDrive syncing the
  repository. No cargo other than the launcher's relink. No user gesture
  on the bench window.
- **Rendering proof**: screenshots at (a) and (b) for B3, C3 (100 KB) and
  every 300 KB run (`raw/*-page1.png`, `raw/*-10pages.png`).

## 3. Figures — 100 KB bodies

(a) = Feed page 1, (b) = 160 cards, (c) = back to Inbox + 25 s; the
GC column is a diagnostic beyond the protocol (runs that have it).
First card painted after the Feed click: A 428-450 ms, B 845-850 ms,
C 845 ms (one sample per run).

## 100 KB bodies — private working set, MB (one instance, all processes)

| Option | run | rest | feed-page-1 | feed-10-pages | back-inbox | back-inbox-25s | back-inbox-after-gc |
|---|---|---:|---:|---:|---:|---:|---:|
| A | 1 | 88.6 | 150.6 | 281.8 | 237.5 | 236.0 | — |
| A | 2 | 88.5 | 149.6 | 276.1 | 272.7 | 272.6 | — |
| A | 3 | 87.7 | 150.4 | 358.6 | 231.8 | 229.5 | 208.5 |
| **A** | **median** | **88.5** | **150.4** | **281.8** | **237.5** | **236.0** | **—** |
| B | 1 | 91.3 | 134.1 | 256.5 | 255.8 | 256.1 | — |
| B | 2 | 91.8 | 133.6 | 269.9 | 255.7 | 254.1 | — |
| B | 3 | 89.1 | 133.7 | 257.6 | 221.5 | 217.6 | 196.5 |
| **B** | **median** | **91.3** | **133.7** | **257.6** | **255.7** | **254.1** | **—** |
| C | 1 | 89.9 | 143.9 | 260.0 | 258.1 | 258.5 | 196.2 |
| C | 2 | 88.1 | 140.1 | 371.6 | 243.2 | 233.1 | 229.3 |
| C | 3 | 91.7 | 144.0 | 297.9 | 294.3 | 294.3 | 227.5 |
| **C** | **median** | **89.9** | **143.9** | **297.9** | **258.1** | **258.5** | **227.5** |

### Where (renderer / gpu-process, MB, per run)

| Option | run | page 1 renderer / gpu | 10 pages renderer / gpu | +25 s renderer / gpu | after GC renderer / gpu | JS heap MB (p1 / 10p / +25 s / GC) |
|---|---|---:|---:|---:|---:|---|
| A | 1 | 88.0 / 11.2 | 215.9 / 11.4 | 171.8 / 11.4 | — / — | — / — / — / — |
| A | 2 | 87.5 / 11.2 | 210.0 / 11.4 | 209.3 / 11.4 | — / — | — / — / — / — |
| A | 3 | 88.2 / 11.2 | 217.1 / 11.6 | 166.0 / 11.4 | 145.1 / 11.4 | 18 / 70 / 5 / 5 |
| B | 1 | 73.8 / 11.2 | 192.3 / 11.3 | 194.7 / 11.2 | — / — | — / — / — / — |
| B | 2 | 73.6 / 11.1 | 198.2 / 11.2 | 191.9 / 11.2 | — / — | — / — / — / — |
| B | 3 | 73.6 / 11.1 | 195.1 / 11.3 | 155.7 / 11.3 | 135.4 / 11.3 | 16 / 41 / 5 / 5 |
| C | 1 | 83.2 / 11.2 | 195.4 / 11.6 | 195.5 / 11.6 | 134.2 / 11.6 | 18 / 42 / 42 / 5 |
| C | 2 | 79.2 / 11.2 | 306.7 / 11.6 | 170.7 / 11.6 | 166.9 / 11.6 | 18 / 75 / 5 / 5 |
| C | 3 | 83.6 / 11.2 | 233.1 / 11.6 | 231.4 / 11.6 | 164.6 / 11.6 | 19 / 42 / 42 / 5 |

### Scroll cost (160 cards = 8 pages of 20)

| Option | run | cards / live iframes / dormant at 10 pages | total scroll ms | page arrival ms (each) | frames | long frames (>50 ms) | max frame ms |
|---|---|---|---:|---|---:|---:|---:|
| A | 1 | 160 / 6 / 154 | 11229 | 534, 212, 200, 927, 402, 209 | 753 | 6 (417 ms) | 83 |
| A | 2 | 160 / 6 / 154 | 10596 | 491, 204, 171, 578, 201, 221 | 616 | 2 (117 ms) | 67 |
| A | 3 | 160 / 6 / 154 | 10782 | 514, 193, 217, 808, 176, 191 | 753 | 5 (400 ms) | 100 |
| B | 1 | 160 / 3 / 157 | 10596 | 477, 145, 135, 906, 141, 136 | 640 | 0 (0 ms) | 33 |
| B | 2 | 160 / 3 / 157 | 11132 | 514, 144, 146, 1039, 471, 133 | 669 | 0 (0 ms) | 17 |
| B | 3 | 160 / 3 / 157 | 11340 | 497, 142, 137, 1599, 136, 140 | 725 | 0 (0 ms) | 33 |
| C | 1 | 160 / 5 / 156 | 11313 | 494, 505, 802, 491, 174, 145 | 789 | 0 (0 ms) | 50 |
| C | 2 | 160 / 5 / 156 | 13187 | 510, 474, 1726, 484, 144, 166, 166 | 821 | 0 (0 ms) | 33 |
| C | 3 | 160 / 5 / 155 | 16220 | 504, 469, 1027, 487, 488, 3338, 470 | 963 | 0 (0 ms) | 50 |

The other four processes (browser 27-33, network 7, storage 3, crashpad 2)
and the exe (6) move by less than 5 MB across every phase and option.
The total scroll time is dominated by the harness's own waits (700 ms
between pages, the no-growth cutoff at the end): compare the per-page
arrivals and the long frames, not the total.

## 4. Figures — 300 KB bodies

Runs 1-2 used the 4 s no-growth cutoff, too short at 300 KB (a page
takes ~3 s to arrive): the scroll stopped at 120 cards (A), 140 (B),
100 (C) — their (b) and return figures are NOT at the same card count.
Run 3 (12 s cutoff) reached 160 cards in all three options and is the
comparable one; the medians below mix both and are for the record only.

## 300 KB bodies — private working set, MB (one instance, all processes)

| Option | run | rest | feed-page-1 | feed-10-pages | back-inbox | back-inbox-25s | back-inbox-after-gc |
|---|---|---:|---:|---:|---:|---:|---:|
| A | 1 | 91.0 | 214.9 | 610.5 | 342.1 | 339.1 | 323.6 |
| A | 2 | 89.4 | 213.4 | 471.4 | 329.5 | 325.9 | 313.2 |
| A | 3 | 88.4 | 211.9 | 501.0 | 495.7 | 495.4 | 330.1 |
| **A** | **median** | **89.4** | **213.4** | **501.0** | **342.1** | **339.1** | **323.6** |
| B | 1 | 89.6 | 163.4 | — | — | — | — |
| B | 2 | 88.2 | 161.7 | 440.2 | 301.3 | 297.2 | 293.4 |
| B | 3 | 88.7 | 161.7 | 424.4 | 421.1 | 420.5 | 296.8 |
| **B** | **median** | **88.7** | **161.7** | **440.2** | **421.1** | **420.5** | **296.8** |
| C | 1 | 90.8 | 195.5 | 507.5 | 308.1 | 302.7 | 298.3 |
| C | 2 | 88.7 | 194.2 | 438.5 | 316.3 | 278.5 | 273.8 |
| C | 3 | 88.8 | 193.1 | 862.5 | 503.1 | 335.4 | 335.4 |
| **C** | **median** | **88.8** | **194.2** | **507.5** | **316.3** | **302.7** | **298.3** |

### Where (renderer / gpu-process, MB, per run)

| Option | run | page 1 renderer / gpu | 10 pages renderer / gpu | +25 s renderer / gpu | after GC renderer / gpu | JS heap MB (p1 / 10p / +25 s / GC) |
|---|---|---:|---:|---:|---:|---|
| A | 1 | 151.6 / 11.2 | 541.6 / 11.6 | 275.6 / 11.5 | 261.1 / 11.5 | 25 / 113 / 5 / 5 |
| A | 2 | 150.3 / 11.2 | 403.0 / 11.5 | 263.4 / 11.5 | 249.6 / 11.4 | 25 / 113 / 5 / 5 |
| A | 3 | 148.6 / 11.2 | 432.3 / 11.5 | 431.7 / 11.4 | 267.3 / 11.3 | 25 / 105 / 106 / 5 |
| B | 1 | 103.0 / 11.1 | — / — | — / — | — / — | 17 / — / — / — |
| B | 2 | 101.3 / 11.1 | 376.6 / 11.2 | 234.1 / 11.2 | 232.3 / 11.2 | 17 / 97 / 5 / 5 |
| B | 3 | 101.2 / 11.1 | 361.2 / 11.3 | 358.7 / 11.3 | 235.4 / 11.3 | 17 / 102 / 103 / 5 |
| C | 1 | 134.6 / 11.2 | 348.0 / 11.6 | 241.0 / 11.5 | 236.7 / 11.5 | 23 / 109 / 5 / 4 |
| C | 2 | 132.7 / 11.2 | 372.6 / 11.6 | 218.0 / 10.6 | 213.3 / 10.6 | 23 / 68 / 5 / 5 |
| C | 3 | 131.6 / 11.2 | 793.9 / 11.8 | 271.3 / 11.5 | 271.1 / 11.5 | 23 / 198 / 5 / 5 |

### Scroll cost (160 cards = 8 pages of 20)

| Option | run | cards / live iframes / dormant at 10 pages | total scroll ms | page arrival ms (each) | frames | long frames (>50 ms) | max frame ms |
|---|---|---|---:|---|---:|---:|---:|
| A | 1 | 120 / 11 / 109 | 11313 | 1130, 2987, 451 | 620 | 4 (833 ms) | 233 |
| A | 2 | 120 / 11 / 109 | 11321 | 1128, 2973, 479 | 617 | 4 (883 ms) | 233 |
| A | 3 | 160 / 10 / 150 | 38724 | 1122, 2843, 425, 4923, 9763, 452, 409 | 2472 | 14 (1983 ms) | 225 |
| B | 2 | 140 / 3 / 137 | 11846 | 1080, 2945, 269, 270 | 695 | 2 (117 ms) | 67 |
| B | 3 | 160 / 3 / 157 | 33685 | 1070, 2576, 275, 209, 10549, 169, 294 | 1990 | 2 (150 ms) | 83 |
| C | 1 | 100 / 5 / 95 | 11662 | 1131, 2694, 1139 | 711 | 4 (475 ms) | 125 |
| C | 2 | 100 / 5 / 95 | 12102 | 1307, 2580, 1145 | 687 | 4 (517 ms) | 133 |
| C | 3 | 160 / 5 / 157 | 39712 | 1031, 2593, 1016, 4555, 9976, 1000, 1040 | 2717 | 8 (917 ms) | 117 |

## 5. What the figures say (facts, not a verdict)

1. **No option is under 200 MB on the ten-page protocol on this machine**
   — neither at 160 cards nor after the return + 25 s, at 100 KB or
   300 KB. At 100 KB after the return, B 217.6-256.1, A 229.5-272.6,
   C 233.1-294.3; after a forced GC, B 196.5 (1 run), A 208.5 (1 run),
   C 196.2-229.3.
2. **The live iframes are not where the memory is.** B keeps 3 live
   iframes and blanks the leaving ones; C keeps 5 and never creates a
   sixth; both still reach 192-200 MB of renderer at 160 cards (100 KB)
   against A's 210-217 with 6 live iframes. B's saving over A on the
   Feed pages is 15-25 MB at 100 KB (page 1: 150 → 134; 160 cards: 282
   → 258 medians) and ~50 MB on page 1 at 300 KB (213 → 162).
3. **The instantaneous (b) figure is dominated by V8's GC timing.** JS
   heap: 5 MB at rest, 16-19 after page 1, 41-75 at 160 cards (100 KB),
   68-198 at 300 KB, on identical runs — that spread moves (b) by ±100
   MB at 100 KB (A3 358.6, C2 371.6 vs 257-282) and by ±400 MB at 300 KB
   (C3 862.5 with 198 MB of heap, C1 507.5). Every card holds its
   `document` string in `cards` state (100 KB → ~200 KB UTF-16; 160 ×
   200 KB = 32 MB live at 100 KB, ~96 MB at 300 KB) plus the IPC copies.
4. **After the return, V8 may sit on that heap for more than 30 s**: at
   300 KB, A3 and B3 stayed at 495 / 421 MB for the whole 33 s with
   105 / 103 MB of JS heap, until the forced GC (330 / 297). What a
   user's task manager shows after leaving the Feed depends on when
   V8 decides to collect, in every option.
5. **What survives the forced GC is renderer memory outside the JS
   heap, in every option, and it scales with body size**: renderer
   after GC vs 30-33 at rest — 100 KB: A 145, B 135, C 134-167; 300 KB:
   A 250-267, B 232-235, C 213-271 — with the JS heap back at 5 MB and
   the Feed unmounted. Neither blanking (B) nor pooling (C) releases
   it. Candidates this spike does not separate: Blink/Oilpan heap of
   the 160 detached documents, PartitionAlloc not returning pages,
   font/glyph and paint caches. A DevTools memory snapshot
   before/after unmount (D-53's "Lead") is the next measurement, not
   another window size.
6. **Run-to-run spread on the return phase (±20-40 MB) is larger than
   the A/B gap** there; only the after-GC figures are tight enough to
   order the options, and B leads A by 10-30 MB on them.
7. **Scroll**: B removes the long frames (100 KB: 0 in 3 runs vs 2-6 of
   67-100 ms for A; 300 KB: 2 vs 4-14, max 83 vs 233 ms) with equal or
   faster page arrivals. C has no long frames at 100 KB but slower,
   irregular page arrivals (1.0-3.3 s pages): with an absolutely
   positioned pool the scene's bottom sits on slots of height 0 until
   the pooled iframe has loaded and measured, so "last card into view"
   lands short of the next-page threshold. At 300 KB every option shows
   one ~10 s page (4.5-10.5 s) — a core-side cost of `feed_cards` with
   300 KB bodies or the `moreRequested` chain, not distinguished here.
8. **B at 300 KB, run 1: the page died mid-scroll** ("Target page,
   context or browser has been closed" from CDP at the count poll).
   Runs 2 and 3 of B at 300 KB completed. The launcher only prints the
   app's output on startup failure, so the spike cannot say whether
   the renderer or the app process went; a reproduction needs the
   captured log dumped on any exit. One occurrence in 9 runs of B.

## 6. Defects of the spike implementations (not corrected — time box)

- **C: overlap at 160 cards** (`raw/C-100k-run3-10pages.png`): two
  pooled iframes overlap — the lower one's `top` was taken from its
  slot's `offsetTop` before the slot above finished growing, and the
  re-sync (ResizeObserver → `heights` state → effect → rAF) did not
  land before the screenshot. The pooled element is created by
  `document.createElement`, so it lacks Svelte's scoped `.body` class:
  default iframe border, no white background, 4 px extra height. C's
  memory figures are valid (5 live documents, proven by the DOM count);
  its rendering is not shippable as spiked.
- **B: first card 400 ms later than A** on the Feed entry (845 vs
  430 ms) — not traced within the time box.
- **B: the blank is not observable from the DOM count**: the harness
  counts `iframe.body`; a blanked-not-yet-unmounted iframe still
  counts. The live count (3) is constant across runs, so the unmount
  did follow.

## 7. Limits

- Machine: Snapdragon X / Adreno / WebView2 152 — the GPU process holds
  11 MB here against 132 MB on the CE's workstation. If the
  workstation's GPU share is compositor surfaces of live iframes, B and
  C could show a gain THERE that this machine cannot show. A run of
  `bench.mjs` on the workstation (same script) is the missing figure.
- Debug build of the desktop crate (the bench's own profile); WebView2
  is the production binary, so the renderer figures are representative;
  the exe's 6 MB is not.
- Synthetic plain-text bodies (no images, CSS, tables): a real
  newsletter with granted images would load the GPU process and Blink's
  image cache differently.
- 3 runs per option and size on a machine also running this session,
  Defender and OneDrive: ±20-40 MB on the return phase is the noise
  floor; differences under it are not differences.
- The forced GC is a diagnostic: production has no such button.
- (b) is 160 cards (8 pages), the fixture's maximum; D-53's field
  protocol says ten pages of real mail.
- Reading pane: neither B nor C touches `Thread.svelte`; B's change to
  `lib/body.js` is a guard on `dataset.blank`, which the reading pane
  never sets — unchanged by construction, not re-measured.
- 300 KB runs 1-2 stopped at 100-140 cards (harness cutoff): only run 3
  compares the options at 160 cards.

## 8. Estimated cost of industrialization

### B (window ±1 + blank before leaving)

Code: ~60 lines as in the patch, plus tracing the 400 ms first-card
delay. ~0.5 day.

e2e suite (~1 day incl. two bench runs): (1) window spec — at most 3
live iframes; a leaving iframe carries `src="about:blank"` and no
`srcdoc` before it disappears (a `data-blank` attribute is already
there to assert on); (2) fold/unfold — collapse an in-window card,
expand it: the body comes back with its document and its links wired
(`__e2eLinks` seam); (3) image consent — grant images on a card: the
reload path (`watchImagePermissions` → documents nulled → reload)
re-mounts a fresh iframe with the new document, never a blanked one;
(4) leave the Feed and come back: cards re-mount. S1 check: at every
step, every `iframe` in the document has `sandbox` exactly
`allow-same-origin` and no `allow-scripts` — one shared assertion.
Record (a)(b)(c) from the bench as figures, not asserted.

To name: the gain is 15-25 MB on the Feed pages at 100 KB, within the
noise on the return phase, while ±1 shows dormant blocks one card away
on a fast scroll.

### C (pool of 5 iframes)

Code: the spike's 130 lines are not shippable (overlap, unscoped styling,
first-frame delay). A correct version needs a layout pass that positions
the pool after every layout change of the column (ResizeObserver on the
column and every slot; positions and slot heights committed in the same
frame), keyed identity of a pooled iframe across a reload (image consent
changes the document under the same key), focus/selection when an iframe
is re-targeted, and the release/acquire order within a Svelte flush on
fold/unfold. 2-3 days.

e2e suite (~2 days): everything listed for B, plus (5) never more than 5
iframes in the DOM at any time, including mid-scroll (a MutationObserver
seam recording the max); (6) no overlap — every pooled iframe's rect
inside its slot's rect after every page (this spike's defect, as the
RED first); (7) `wireLinks` re-armed after each `srcdoc` swap (a link
in the 3rd swapped document reaches `__e2eLinks`); (8) keyboard replay
across a swap. The S1 assertion must cover the pooled elements created
outside Svelte.

To name: C's figures are equal to or worse than B's on every phase on
this machine, for 3-4× the code and a fragile absolute-positioning
layout; its only remaining argument would be a GPU-process gain on the
workstation, unmeasured here.

### The common finding

Both options leave 100-130 MB (100 KB) / 200-240 MB (300 KB) in the
renderer after the Feed is gone and the JS heap is collected, and both
leave the `cards` documents to V8's timing. Before spending on B or C,
the D-53 "Lead" (renderer memory snapshot before/after unmount) would
say what that memory is; if it is Blink's detached-document heap, the
lever is not the number of live iframes but the number of documents
ever parsed (not mounting a card the user scrolls past at speed) and
the `document` strings kept in `cards` after mount.

## 9. Files

- `bench.mjs`, `ram-detail.ps1`, `run-option.sh`, `run-all.sh`, `summary.mjs`
- `options/Feed-{A,B,C}.svelte`, `options/body-{A,B}.js`
- `patches/{A,B,C}.patch`
- `raw/*.jsonl` (figures), `raw/summary-{100,300}k.md`, `raw/*.log`
  (raw bench/cargo/npm output), `raw/*.png` (rendering proofs)

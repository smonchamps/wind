# Debt register

Debt assumed KNOWINGLY: each entry states the measured fact, the
choice, and what would reopen it. An entry is closed out by a commit
that strikes it — never by being forgotten. (STANDARD §2.6: a
deferral = one justified line.)

## Open

### D-1 · p95 opening time above budget on very large bodies

- **Fact (2026-08-11, gate R1)**: p95 opening time measured at
  52–55 ms (budget 50 ms) on the real database — p50 ~14 ms. The
  overrun is carried by ONE body > 1 MB from the deterministic
  sample; the database holds 207 of them (up to 28 MB). Identical
  with and without the sync cycle: HTML sanitization cost,
  pre-existing before the redesign.
- **CE decision (2026-08-11)**: recorded as debt, to be handled
  later.
- **Lead**: lazy sanitization or streaming of very large bodies; to
  be investigated as a separate core job.
- **Reopens if**: the field reports perceptibly slow openings, or
  the p50 drifts.

### D-2 · LRU eviction of list pages

- **Fact (P1-P2)**: the windowed list keeps every served page in
  memory; on a very long session with heavy scrolling, RAM climbs
  and never comes back down.
- **Lead**: LRU eviction of pages outside the window.
- **Reopens if**: RAM exceeds the budget (200 MB) in real use.

### D-4 · Focus trap of overlays

- **Amended (PLAN-AUDIT-V2 E11, 2026-09-02)**: Settings now opens on
  its first control (focus enters with the panel, pattern from
  `Retour.svelte`) and menus set then return focus (`Menu.svelte`).
  The Tab trap that EXITS the overlay, though, remains.

- **Fact (A8)**: Tab can exit an overlay (compose, settings) to the
  background; Escape and visible focus cover the essentials.
- **Reopens if**: the field with a screen reader calls for it.

### D-8 · Expensive queries from periodic probes (off pump, real CPU cost)

> ✅ **CLOSED on 2026-08-26 (PLAN-DEMARRAGE).** Its reopening clause
> came true: the database went from 1.3 to 12.8 GB and the 575 ms of
> `pending_total` had become **20,839 ms cold**, holding the global
> lock 8,870 ms on every startup. Fixed — `pending_total` now
> measures **107.9 ms**, `backfill_status` **124.9 ms** in the field
> cold (×71). The 865 ms figure below was **STALE** the moment this
> debt was written: re-measured at ~31 ms cold / ~11 ms hot,
> `nav_snapshot` having been rewritten in the meantime. **The lesson
> to keep is not the figure, it is that a debt entry carries a DATED
> measurement: re-measure it before relying on it.**

- **Fact (2026-08-15, PLAN-GELS)**: `nav_snapshot` **865 ms** per
  Gmail account (Archive counter for a full account, exclusion by
  `message_id`, 87k rows — every 10 s) — **stale figure, see the box
  above**; `pending_total` **575 ms** (COUNT per mailbox, NOT EXISTS
  on `bodies` — on every mail generation). Measured in direct SQL on
  the real database.
- **CE decision (2026-08-15, D4 of the plan)**: from `hors_pompe()`
  they no longer freeze anything or anyone — optimizing them without
  a finding would be work without a measurement. Family D-7
  (responsiveness stopwatches).
- **Lead**: nav counter cache invalidated by generation;
  `pending_total` as one aggregated query.
- **Reopens if**: the field points to the cost (fan, battery, write
  contention, perceived probe latency).

### D-9 · Invariant A41 has no structural guard

- **Fact (review 2026-08-15, A41)**: « nothing touches the database
  before `migration_check` » lives in comments and one probe test
  (`la_langue_se_lit_sans_adopter_la_base`) — nothing stops a future
  pre-modal command from opening the database in full (`Store::open`):
  the whole suite would stay green, the bug would be rediscovered in
  the field.
- **Lead**: an `adopted` flag on `MigrationShared` (or a shared
  opening helper for the 30+ `Store::open` calls in `commands.rs`)
  that makes any full opening before the probe fail LOUDLY; to be
  investigated as a job, not on the fast lane.
- **Reopens if**: a startup command is added before the modal.

### D-10 · Deferred setting of the language has no UI test

- **Fact (review 2026-08-15)**: the Rust half of A41 is held by a
  test that rewinds a real database; the UI-side order (`assurer()`
  before `poserLangueDetectee()`, set only if the probe answered) is
  asserted by no e2e test — `redesign-language.spec.js` never plays
  the first launch on a blank database.
- **Lead**: an e2e spec `vierge: true` that asserts `prefs.lang` set
  after startup, and absent if `migration_check` fails.
- **Reopens if**: a refactor of `onMount` touches the startup order.
- **Reopened and reclosed without a close-out (2026-08-22,
  PLAN-RETOURS-8)**: the onboarding journey (A75) touches the startup
  order — verified on the evidence: the onboarding decision lives in
  `chargerNav`, AFTER `assurer()` and `poserLangueDetectee()`; the
  A41 order is intact. The e2e tests now play the first launch on a
  blank database (full journey), but the `prefs.lang` assertion from
  the lead above is still to be written — the debt remains.

### D-11 · The theme-switch bench stayed calibrated for 7 themes

- **Fact (review 2026-08-16, PLAN-WADA-ELARGI)**:
  `e2e/measure-v2.mjs` keeps 60 iterations and comments saying « the
  7 themes » while A42 now delivers 28 — the sample per theme drops
  from ~8 to ~2, the « per-theme switch cost » figure is no longer
  comparable to the historical baseline.
- **Reason for deferral**: out of scope for the job (file untouched
  by the diff), and recalibrating without re-measuring a baseline
  would be work without a measurement. Family D-7 (responsiveness
  stopwatches).
- **Lead**: at the next measurement pass, recalibrate (28 × N
  iterations) and re-set the baseline in the same reading.
- **Reopens if**: a measurement pass compares theme switching to the
  historical figures.

### D-15 · « To: recipient » display scoped to the Sent category

- **Fact (2026-08-16, PLAN-RETOURS-MAIL R4)**: in the list, the
  switch to « To: X » (instead of the sender = SELF) is guarded by
  `categorie === 'envoyes'` (`Liste.svelte`). A sent message reached
  by a path other than the Sent category — folder navigation — still
  shows the sender. The reading pane, though, is correct everywhere
  (it relies on `propre(m)`, not on the category).
- **Reason for deferral**: the field feedback targeted the Sent
  folder; a per-row detection (« this message is from me ») would
  also switch rows in the Inbox where OUR last reply is at the top,
  a broader behavior change than the feedback called for.
- **Reopens if**: the field views sent messages outside the Sent
  category and the displayed sender is a problem.

### D-16 · Recipient backfill: unindexed leftover probe

- **Fact (2026-08-16, PLAN-RETOURS-MAIL)**: `backfill_recipients`
  calls `recipients_pending_count` (scan `to_addrs IS NULL`, no
  index) on every cycle, INBOX + Sent — even after converging to
  zero. Same class as D-8 (periodic probes off pump: real CPU cost,
  no freeze).
- **Reason for deferral**: aligned with the existing pattern of the
  header pass (`thread_headers_pending_count`, same cost, accepted);
  optimizing it without a finding would be work without a
  measurement. Family D-8.
- **Lead**: skip the probe when the pass reported nothing, or a
  partial index `WHERE to_addrs IS NULL`.
- **Reopens if**: the field points to the cost (with D-8).

### D-17 · Body backfill blind to a Gmail throttle

- **Fact (analysis 2026-08-17)**: on the body-backfill path, a Gmail
  server error (throttling: `[OVERQUOTA]`, « bandwidth exceeded »,
  login rejection / « web login required ») is **caught then
  dropped**. `run_backfill_all` does push it into
  `BackfillSummary.errors` (`commands.rs:4135` on connect, `:4158`
  mid-FETCH), but the UI loop NEVER reads `bilan.errors`
  (`App.svelte:391-417`): it only looks at `remaining` and `fetched`,
  and breaks silently on `fetched === 0`. Symptom in the field:
  « Backfilling messages · N remaining » freezes without a word —
  the worst case for diagnosing a throttle. Backfill is the only one
  of the three that ignores its error channel (sync, for its part,
  surfaces through `synchroEchec`, `App.svelte:181`).
- **Second, coupled defect**: `is_connection_error` (`mail-imap`,
  `lib.rs:897`) recognizes ONLY our errors prefixed `"connexion "`.
  A Gmail server response therefore falls into the `Err(_)` of
  `connect_imap` (`commands.rs:3666`) and is treated as a « dead
  token » → `authenticate_silent` + reconnect. The anti-hammering
  guard from the comment (`commands.rs:3660-3664`) protects only the
  network-outage case: a throttle triggers exactly the refresh +
  reconnect it is meant to avoid.
- **Reason for deferral**: no throttle observed in the field to
  date — the method forbids optimizing against an unmeasured
  problem. But the blindness itself is structural: the day it bites,
  nothing will say so.
- **Lead**: (1) in the UI loop, if `fetched === 0` and
  `bilan.errors.length > 0`, post a notice in the slot (mechanism
  already there, `App.svelte:552-558`) instead of breaking mute;
  (2) recognize the throttle (widen `is_connection_error` or a
  dedicated guard) so that an `[OVERQUOTA]` / login rejection does
  NOT trigger a refresh — « let the account breathe », not « dead
  token ».
- **Reopens if**: the field reports a backfill that freezes without
  explanation, or a Gmail account throttled/locked after a large
  initial sync. Start with (1) — making the error visible is the
  prerequisite for any diagnosis.

### D-18 · « Load more »: the button and the append are not e2e-tested

- **Fact (2026-08-17, PLAN-CHARGER-PLUS)**: the « Show the next N »
  button, the append, the soft cap at 1000 rows and the race guard
  are not covered in e2e. The button only appears past 100 results
  (`resultats.length < total`), yet the gate's fixtures (Clarity,
  inbox with 6-10 messages) fall well below that. Only the CORE is
  tested (`search_capped_pages_without_gap_or_overlap`: pages with
  no gap or duplicate); the UI was validated IN THE FIELD (251k
  database) and by a fresh-eyes review (which incidentally caught a
  race-guard bug).
- **Reason for deferral**: an e2e fixture with >100 messages and a
  common term (`seed_inbox` with a large `nombre` +
  `ko_par_corps ≥ 1`, whose bodies carry the words of `MOTS`) is
  separate infrastructure, disproportionate for this job — the field
  covered the gesture.
- **Lead**: a dedicated spec on a large `seed_inbox` database
  (≈ 250 messages); assertions: button present, click → the list
  grows and « N of M » climbs, ~1000 rows → prompt « Refine your
  search », rare term → no button.
- **Reopens if**: the button's behavior regresses in the field, or
  before touching `chargerPlus`/the cap again.

### D-19 · Cross-device draft resume does not restore Cc/Bcc

- **Fact (2026-08-17, PLAN-RETOURS-2 #4)**: Cc and Bcc are persisted
  **locally** (columns `drafts.cc_raw`/`bcc_raw`) — autosave, close
  and resume on the SAME machine keep them, and the mirror pushed to
  the Gmail Drafts folder (`draft_bytes`) carries the Cc and Bcc
  headers. But pulling a draft written on ANOTHER device
  (`import_remote_draft`, `commands.rs:1449`) only reads
  `to_raw`/`subject`/`body` from the remote message: the Cc/Bcc of a
  pulled-back draft come back **empty**.
- **Reason for deferral (§2.6)**: the remote parsing path (parse the
  Cc header, decide the fate of the pulled-back Bcc) is a separate
  slice; the loss only affects cross-device resume, never sending
  nor local resume.
- **Lead**: extend `RemoteDraft` + `convert.rs` to extract Cc (and, a
  decision still to make, Bcc) from the remote message, then
  `import_remote_draft` populates `cc_raw`/`bcc_raw`.
- **Reopens if**: a user reports lost Cc while resuming a draft
  started elsewhere.

### D-20 · Gmail cycle: per-cycle cost still high when many views move

- **Fact (2026-08-17, PLAN-RETOURS-2 #1, ADR 0021)**: the 30-min
  cadence fixed the FREQUENCY (from ~45% of the time in sync to
  ~7%). But a full Gmail cycle still costs up to **~135 s** when many
  views have changed (release measurement: 22 folders polled, ~5
  s/changed folder — Gmail throttle likely, cf. **D-17**). Excluding
  the virtual views (Important, Followed) was **dropped**: marginal
  after the cadence fix, and costly (a new field on the core's
  `Folder` type, detecting the `\Important`/`\Flagged` flags in the
  adapter, logic close to ADR 0010). « All messages » is deliberately
  KEPT (Archive, mail archived elsewhere — ADR 0010).
- **Reason for deferral (§2.6)**: the cadence captures most of the
  gain; the rest is not worth the code surface unless the field asks
  for it again.
- **Lead**: (1) exclude Important/Followed by IMAP flag (not
  fragile); (2) attack the ~5 s/folder (throttle — crosses D-17);
  (3) LIST-STATUS does not help Gmail (not advertised), the inventory
  stays at ~52 STATUS calls.
- **Reopens if**: the 30-min cycle becomes a problem in the field
  again, or if the throttle is confirmed (crosses D-17).

### D-21 · Backfill percentage: double COUNT of the corpus per batch

- **Fact (2026-08-18, PLAN-RETOURS-3 R1)**: the denominator of the
  backfill % (A55) adds `corpus_total` (a COUNT of the corpus per
  mailbox) next to `pending_total`, both recomputed on **every
  batch** of `backfill_bodies` (50 bodies) during the backfill
  loop — i.e. two-plus full scans per batch over ~256k messages ×
  ~40 mailboxes. Caught at the `/code-review high` review.
- **Reason for deferral (§2.6)**: the probe is off pump (does not
  freeze the window) and the field judged the app **perfectly
  smooth** during backfill (2026-08-18) — the budget is held.
  Optimizing without a finding would be work without a measurement.
  **Family D-8** (periodic probes off pump, real CPU cost).
- **Lead**: a single query per mailbox returning `(total, missing)`
  together; or cache the total (near-stable within a loop) and only
  refresh it on a clear change.
- **Reopens if**: the field points to the backfill's CPU cost (with
  D-8, D-16).

### D-22 · « Report as spam » on a spam message reached via search

- **Fact (2026-08-18, PLAN-RETOURS-3 R2)**: `report_spam` returns
  `Ok(())` without moving anything when the message is **already**
  in the Junk folder (`spam == mailbox`), but the UI flashes
  « reported as junk » and closes the thread. Reachable only via
  **search** (a spam message opened with `categorie !=
  'indesirables'` still shows the « Report » button).
- **Reason for deferral (§2.6)**: edge case (search→spam path),
  cosmetic (no data corrupted); fixing it properly would require the
  UI to know each account's Junk folder (the Thread does not have
  it) — disproportionate for the frequency.
- **Reopens if**: real usage shows the false success being a
  problem.

### D-49 · Cleanliness deferred from the PLAN-AUDIT-V1 review (audit wave 3)

- **Fact (fresh-eyes review, 2026-09-02)**: nine cleanliness
  candidates checked but not taken up, wave 1 only fixing S1 issues:
  `into_inner` copy-pasted seven times (one `verrou_repris` helper);
  `hors_pompe(app, |app| auth_for(&app, id))` ×4 (`session_de`);
  `trace::trace` and `trace_maj` — two dated-line writers, only
  `wind.log` is capped at a megabyte; `is_connection_error`
  duplicated IMAP/SMTP on the « connexion » prefix;
  `instance::dossier_de_la_base` recomputes the `db_path` rule (two
  sources of the path, with no test tying them together);
  `sync_inbox`/`sync_inbox_light` still twins; `remove_local` with
  two paths (`is_autocommit`); `compose()` with no `references` (set
  after the fact in two places); `SEUIL_QUARANTAINE` hardcoded in the
  Store; `reply_*`/`forward_context` take the lock three times (rare
  paths).
- **Reason for deferral**: none is an observable defect; audit wave
  3 (`docs/AUDIT-2026-09-01.md` §5) reorganizes these files.
- **Reopens if**: a job touches one of these sites — fix it in
  passing, not as a gratuitous refactor (§2.6).

### D-50 · Two stated limits of wave 1 to confirm in the field

- **Fact (PLAN-AUDIT-V1 E8, 2026-09-02)**: (1) the refresh token
  renewed by Microsoft is now stored when it changes — not proven in
  test (the vault cannot be simulated); to confirm on a Microsoft
  account beyond 90 days; (2) the « open manually » fallback
  (`BrowserFallback`) returns control without waiting for the
  redirect — rare case (no browser), unchanged.
- **Reopens if**: a silent Microsoft disconnection after 90 days, or
  a tester with no default browser.

### D-51 · An account without CONDSTORE never resyncs its flags

- **Fact (audit 2026-09-01 §2.1, CE decision D3 of PLAN-AUDIT-V2 on
  2026-09-02)**: without the CONDSTORE announcement, `changes_since`
  returns `None` and the engine only re-reads the UID differential —
  a message read on the phone stays unread here, forever (`sync.rs`
  promised a « full resync » that does not exist). Gmail, Microsoft
  365 and Dovecot all announce it; the case is theoretical in beta.
- **Reason for deferral**: a `FETCH FLAGS` window on every cycle
  would cost everyone for a server we have never seen. A line in
  `wind.log` names the account without CONDSTORE at each poll: the
  field will say whether the case exists.
- **Reopens if**: the line appears for a tester.

### D-52 · Stated limits of audit wave 2

- **Fact (PLAN-AUDIT-V2, 2026-09-02)**: (1) an edit MADE INSIDE the
  forwarded block is lost on send (the block is replaced by a render
  of its source with its images, D8); a forward whose source is
  ANOTHER account goes out as-is, at neutral pixel; (2) the « RAM
  after five Feed pages » measurement cannot be run on the e2e
  fixture — the windowing is in place, its gain will be read in the
  field; (3) `list_drafts` remains a WHOLE list (bodies included)
  polled every 10 s, outside the single `etat_ui` probe (wave 3,
  with command pagination); (4) `decode_header` still parses one
  synthetic message per subject — not measured as a cost; (5) the
  « archive by shortcut from screen 03 » test flaked twice after the
  resent-mail coalescing (E10) — passed on a rising edge at review,
  79/79 since; (6) the `RFC822.SIZE` probe costs one round trip per
  batch of 50 bodies for a bound (32 MB) rarely reached — the
  alternative is to store the size when envelopes are polled (a
  job); (7) `etat_ui` at 5 s doubles the cadence of nav and sends
  (the watcher's poll imposes 5 s; accepted); (8) `__e2ePanne` is a
  fifth e2e seam compiled into production, without
  `import.meta.env` (wave 3, with the other four); (9) the E1
  fast-gate registry is keyed by PATH, not by file identity — safe
  under single-instance, not guarded by code.
- **Reopens if**: a tester edits a forward and loses the edit; the
  « flaky: N » counter names the same test twice.

### D-53 · Feed RAM: one page of letters costs 70 to 136 MB, and 94 to 167 MB remain after returning

- **Finding** (field STOP 2 PLAN-AUDIT-V2, 2026-09-02): 249 MB of
  private working set across 6 WebView2 processes after ten Feed
  pages on the CE's workstation — STANDARD §3 budget « < 200 MB »
  (at rest 95.5 MB). Bench `e2e/tests/bench-ram-feed.spec.js` (200
  letters of 100 KB, debug build): window 12 → +136 MB on the first
  page, +217 at 160 cards, +167 RETAINED on returning to Inbox;
  window 1 → +70, +96, +94, stable at +25 s. Window width (E10)
  bounds it, it does not cure it: a letter's `srcdoc` iframe is
  worth dozens of MB, and unmounted documents do not release their
  memory (`corpsAuto` and `brancherLiens` clean up — the retention is
  elsewhere: documents of removed iframes, render heap that does not
  shrink?).
- **Pass 2 (window 5, CE's workstation)**: 251.5 MB across 6
  processes — GPU 132.3 MB, renderer 69.6, manager 36.3, network
  8.1, storage 3.2, crashpad 1.8. The window changes nothing to the
  total: the GPU process carries more than half (composited surfaces
  of the iframes and their granted images), the DOM is not the
  lever.
- **Reason for deferral**: CE decision D9 on the window (an
  immediate setting, held — costs nothing); the root cause — one
  iframe per card — is a design question (a single iframe for the
  read card? cards collapsed by default? rendering without an
  iframe?): a set-based job, not a setting. The budget itself needs
  clarifying: « private working set » at rest, or after the
  product's heaviest gesture?
- **Lead**: memory profile of the WebView2 renderer (DevTools,
  before/after-unmount snapshot) to name what is retaining; then
  options measured on the bench.
- **Reopens if**: the clarified budget is exceeded on the CE's
  workstation after D9, or a freeze appears while scrolling the
  Feed.

### D-54 · `multi-select:173` (« the `e` shortcut archives the CHECKED batch ») flakes one gate in three

- **Finding** (2026-09-02, PLAN-AUDIT-V2 gates): passed on the second
  try in three gates out of six (D4: counted, never red). The
  gesture: several rows checked, `e` on the keyboard, a single toast
  and the threads leave. On this machine only so far (CI is green
  every time) — STANDARD §7.5, CI is the reference.
- **Reason for deferral**: out of scope for the day's field work; a
  flaky test that passes on the second try does not block, but three
  occurrences the same day stand out from the noise.
- **Lead**: replay the spec alone twenty times (`--repeat-each`),
  read the trace of the first failure — a race between the checkbox
  and the shortcut (focus left on the checkbox, A38) or between the
  toast and the assertion.
- **Reopens if**: a fourth occurrence, or a red run in CI.
## Closed

### ~~D-36 · The ghost column of `echos` is born on every fresh database~~ — closed 2026-09-01

- **Fact (PLAN-DEMARRAGE, 2026-08-26)**: the `SCHEMA` literal in
  `store.rs` carries a `backslash-n` **inside a SQL comment
  `--`** of an ordinary Rust string. Rust turns it into a real line
  break: the comment stops there, and SQLite swallows the rest as a
  **column**. Reproduced on a fresh database — a column named
  `) — the list o` whose type absorbs the declaration of `to_addrs`.
  The real `to_addrs` exists only because `add_missing_columns` adds
  it back later. The fleet's databases are sound (created before this
  comment) ; **any fresh database is not**.
- **Reason for the deferral**: this is not a performance defect, and
  removing a column from an existing database requires a table
  rewrite. Out of scope for a startup job (refusal §2.6).
- **Lead**: fix the literal, plus a test asserting the column names of
  `echos` on a FRESH database — that is what is missing, and its
  absence is the real cause.
- **Reopens if**: a fresh database shows a defect tied to `to_addrs`,
  or at the first job that rewrites `echos`.
- **Closed at wave 0 of the [2026-09-01 audit](AUDIT-2026-09-01.md)**
  (S1-11): the literal fixed (« joined by a line break », no more
  escape sequence in a SQL comment) AND the missing net —
  `une_base_neuve_n_a_aucune_colonne_fantome`: every column of every
  table of a fresh database carries a sound name. Proven by breaking
  it: RED on the prior database (« ) — the list o »), then RED again
  when the fix itself reintroduced a `backslash-n` in the explanatory
  comment — the net caught its own fix.
  The five databases of beta wave 1 (installed before) carry the
  ghost column; harmless (`to_addrs` exists via
  `add_missing_columns`), it only comes off by rewriting `echos`.

### ~~D-6 · e2e v1 flake: "starring" (parcours-critiques)~~ — closed 2026-08-15

- **Fact (2026-08-13)**: the v1 "starring" test flakes one run in
  three, a path unrelated to the jobs under way; logged, not
  investigated — v1 dormant.
- **Closed at B2** (PLAN-RETRAIT-V1): the `parcours-critiques` spec is
  removed along with the v1 interface, the flake dies with it. Would
  reopen if an equivalent symptom touched a v2 journey.

### ~~D-3 · Weekday dates (2 to 6 days)~~ — closed 2026-08-12

- **Fact (P3)**: the prototype displays « Monday, 18:20 » for
  messages of the week ; `quand()` displayed « August 8 ». Minor visual
  gap, noted at the P3 delivery.
- **Closed at Settings E2** (R-D1, PLAN-REGLAGES): `quand()` extended
  — 2 to 6 days → weekday, `quandLong()` composes « Monday,
  18:20 » with no rework. Without a setting: the prototype's form is
  not opted into.

### ~~D-5 · Upstream charset (U+FFFD in stored bodies)~~ — closed 2026-08-11

- **Fact**: bodies from the real database carried U+FFFD as early as
  the stored HTML — MIME charset decoding at sync time.
- **Closed by `0f7f059`** (separate session, PR #1 merged): the
  `full_encoding` feature of mail-parser (gb2312…), a windows-1252
  fallback when the bytes are not valid UTF-8, and a one-time repair
  of mangled bodies (purge flagged, re-download at backfill, preview
  and index rebuilt along the way).

### D-12 · Opening the thread: `thread_messages` → body cascade, P1 series to re-baseline

- **Fact (UI v3, review of 2026-08-16)**: selection now opens the
  THREAD — `thread_messages` then the last one's body, in series (two
  round trips where v2 made only one), and the P1 "opening" stopwatch
  has changed definition (selection → thread displayed, attachments
  excluded). The historical series (< 50 ms, ADR 0015) is no longer
  comparable.
- **Accepted deferral**: parallelize the head row's body with the
  thread list (one lost fetch in the rare case of a fresher head), and
  re-baseline the `measure-v2` bench on the new definition. Deferred
  to keep the v3 commit on CE verdicts ; to be investigated with
  D-7/D-11 (bench family).

### D-13 · Expand/collapse remounts the thread's iframes

- **Fact (v3 review)**: the frame change unmounts then remounts the
  `srcdoc` iframes of expanded messages — the network replays nothing
  (shared state), but the render re-parses each document and loses
  internal scroll. Noticeable on a long "all expanded" thread.
- **Accepted deferral**: keeping both frames mounted (`display:none` +
  `inert`) would cost duplicated testids — exactly what the v3 review
  just fixed ; the remedy requires re-scoping the e2e suite first. To
  be investigated if the field feels it.

### D-14 · Re-baseline the P1 bench on the A44 geometry

- **Fact (PLAN-RETOURS-V3, 2026-08-16)**: two geometry changes in the
  same job — the overlay bars render ~10 px of width on every
  scrolling pane (0 px reserved vs 10 px webkit), and the list has two
  templates (bare h1 / carrying h2, ~+27 px per bulleted row):
  `visible`, the number of rows rendered per jump and the cost of a
  reflow have moved. The "page" percentiles of the `measure-v2` bench
  are no longer comparable to the series from before A44.
- **Accepted deferral**: re-measure and re-baseline the P1 budgets on
  the delivered geometry, in a dedicated pass — to be investigated
  with the bench family (D-7/D-11/D-12), so as not to mix a budget
  re-baseline with a feature job. The benches ALREADY measure the
  right geometry (browser-args.mjs) ; only the reference series is
  dated.

### D-23 · Downloading an attachment: network path not covered in e2e

- **Fact (PLAN-RETOURS-4, R1, 2026-08-18)**: the new "Save as" gesture
  (click chip → `chemin_enregistrement_suggere` →
  `plugin:dialog|save` dialog → `save_attachment(dest)`) has its e2e
  seam (`__e2eDestination`) but no test exercises it. Success requires
  the IMAP fetch of the bytes — field-only by construction
  (§7.5) ; only the cancel path (`!dest → return`, no toast nor
  fetch) is playable offline.
- **Accepted deferral** (code-review high, gap accepted): R1 is
  validated in the field (CE, 2026-08-18). Adding an e2e for the
  cancel path alone brings little ; the success path stays field-only.
  To be investigated if a composition/attachments bench is set up.

### D-24 · PASSATION.md stub to be removed

- **Fact (PLAN-DOCUMENTATION, 2026-08-19, CE decision D3)**:
  PASSATION.md is split into STANDARD.md (the working standard) and
  ETAT.md (the handover snapshot) ; a few-line stub remains at the
  historical path — a poka-yoke for old memories and the former
  resumption ritual.
- **Removal condition**: two consecutive cold resumptions with nothing
  tripping over the old path ; then the stub gets removed (`docs:`
  commit) and this entry gets struck.
- **Progress**: first resumption counted 2026-08-19 (E4 of the
  job — an ordinary clean resumption, and the stub caught the old
  ritual deliberately stuck to it). One more clean resumption and it
  falls.

### D-25 · Rich composer: four accepted gaps from the review

- **Fact (PLAN-COMPOSITION-HTML, review of 2026-08-20)**: four gaps
  noted by the fresh-eyes review, accepted with no fix —
  1. the JS mirror `texteEnHtml` (Composition.svelte) duplicates
     `mail_core::texte_en_html` (8 lines, documented on both sides):
     removing it would require serving the conversion through Rust,
     which would recreate the text→rich churn the anti-churn guard
     just killed ; an escaping divergence would stay invisible to the
     tests ;
  2. `mail_smtp::draft_bytes` carries 8 positional arguments including
     six consecutive `&str` (clippy allow posted): a Cc/Bcc inversion
     at the call site would compile — the least-tested path (Drafts
     mirror) ;
  3. `DraftContent` has no `Default`: the next field reworks ~45 test
     literals mechanically ;
  4. the e2e triptych for emptying a contenteditable (click + Ctrl+A +
     Delete) is copied three times across the specs.
- **Reopening condition**: at the next job touching drafts or
  sending, fix 2 and 3 (group the parameters, derive `Default`) ; 1
  reopens only if a third converter appears ; 4 at the next spec that
  needs it (shared helper).

### D-26 · Deep category pagination: O(offset) cost accepted

- **Fact (PLAN-DEFILEMENT-PROFOND, 2026-08-20, CE decision D1)**:
  outside Inbox, `category_page` pays `LIMIT offset+limit` per mailbox
  plus a merged sort — the page of 200 costs 10 ms at offset 0, 66 ms
  at 10 000, 157 ms at 40 000, **247 ms at 80 000** (raw SQL, seeded
  base of 120 000, release). The "list page < 100 ms" budget
  (STANDARD §3) is blown past ~20 000 ; Inbox itself holds up (the
  `threads` pattern + `idx_threads_date_globale`, 14.6 ms at offset
  200 000). The exclusion clause for the full Gmail archive costs
  little (partial index `idx_envelopes_message`).
- **Why accepted**: since A64, only one deep page flies at a time
  (bounded queue, VOL_MAX = 1) and the screen shows loading — the
  latency of ONE isolated page is livable ; it was the burst ×
  serialization that caused the multi-minute outage. Field tightening
  (2026-08-20): the count (`category_totals`, a NOT EXISTS probe per
  archive row, ~240 ms on 200 k) is now paid only at page 0 — the bare
  deep page goes from 368 to ~129 ms on the archive fixture.
- **Reopening condition**: if the field measures a deep page beyond
  ~1 s on the real database (256 k, 4 accounts), or if a "list without
  limit" job opens (the existing deferral of the virtualized search)
  — the two-phase pattern of the search (A51) is the starting point.

### D-27 · The outbox only retries at cycle end or on gesture

- **Fact (field PLAN-RETOURS-5, 2026-08-21)**: at first launch, two
  sends clicked right after opening stayed "pending" for the whole
  session — the flush triggered by the "Send" click ran before the
  accounts' sessions were ready, and the outbox has NO clean retry of
  its own: it only resumes at the end of a full cycle (long on the
  real database), at the light pass of the "Sync" click, or on
  network return. The messages went out at the first flush actually
  triggered (Sync click of the next session) — never lost, never
  doubled: the golden rules held.
- **Why accepted**: the case only occurs when sending within the very
  first seconds of a launch ; in steady state, the post-send flush
  fires and succeeds (verified in the same field visit). The status
  bar honestly says "N sends pending".
- **Reopening condition**: if the field sees a send stay pending
  beyond one cycle, or at the first user report in beta.
  Lead: a bounded retry triggered when sessions are established (the
  online-return trigger, R-D3, already exists — hook it up there),
  never a hammering timer.

### D-28 · The pin is carried by the gesture's single envelope alone

- **Fact (review PLAN-RETOURS-7, 2026-08-21)**: pinning stores ONE
  `(mailbox_id, uid)` key — that of the thread head at the moment of
  the gesture. The thread is found again by join (`PINNED_THREADS`),
  and unpinning frees the whole thread ; but if THAT exact message
  leaves its mailbox (deletion by another client, retention policy,
  partial move of the thread), the conversation unpins silently and
  the orphaned `pins` row stays in the database (no FK on `envelopes`
  — only deleting the MAILBOX cascades). A UID reused after an
  UIDVALIDITY reset could pin an unrelated message.
- **Why accepted**: the case requires a third party to delete exactly
  the key message while the rest of the thread stays in Inbox — rare
  ; the join means an orphaned pin is never SERVED (no false display,
  just a lost pin and a dead row) ; and re-pinning is one click. The
  full fix (a key by account `message_id`, or re-anchoring at sync) is
  a robustness job disproportionate for a local v1.
- **Reopening condition**: if the field (or beta) reports pins that
  "jump", or at the first observed UIDVALIDITY reset.
  Lead: re-anchor the pin on the thread's current head at each service
  (`pinned_rows` knows how), and sweep orphans at flush.

### D-29 · A message whose root IS the calendar has a permanently empty body

- **Fact (review PLAN-INVITATIONS, 2026-08-22)**: a message with no
  text/HTML part whose root is `text/calendar` (case C of the
  finding) is now DISPLAYABLE — empty body, the invitation card is the
  content. Before, it fell into "message not found" and stayed an
  eternal backfill candidate. Trade-off: the `""` body is cached
  (`scanned = 1`) — full-text search does not see the meeting title's
  words, and FORWARDING this message produces an empty quote (the ICS
  does not follow), with no warning.
- **Why accepted**: the card shows the essentials (title, time, place,
  organizer) ; the old behavior (hard error) was worse on both counts
  ; the shape is rare (Google/Outlook emit multipart/alternative with
  an HTML part).
- **Reopening condition**: if the field or beta forwards bare
  invitations or searches them by title. Leads: index the invitation
  title in FTS at `save_body_full` ; attach the ICS on forward.

### D-30 · A legacy invitation WITHOUT a calendar attachment row has no card

- **Fact (review PLAN-INVITATIONS, 2026-08-22)**: adopting existing
  data goes through the `pieces-calendrier` repair (bodies of messages
  carrying a calendar `attachments` row are re-read). A message scanned
  BEFORE the feature whose calendar part was NOT classified as an
  attachment by mail-parser (e.g. an exotic `inline` disposition) is
  invisible to the criterion: its card is only born on a chance re-read
  (UIDVALIDITY reset, body re-fetched).
- **Why accepted**: rare shape (the major producers use
  multipart/alternative, classified as an attachment), and the only
  possible local criterion would be re-reading ALL bodies — the
  opposite of a targeted repair.
- **Reopening condition**: a field finding of "this old message is an
  invitation with no card". Lead: widen the repair to messages whose
  BODY contains a BEGIN:VCALENDAR marker (SQL LIKE criterion, one
  pass).

### D-31 · `drafts` does not carry `ics_reply` — the draft round trip would lose it

- **Fact (review PLAN-INVITATIONS, 2026-08-22)**: `Draft.ics_reply` and
  `outbox.ics_reply` exist, `drafts` does not. Currently unreachable:
  an invitation reply is never scheduled (cancelling a scheduled send
  is the only outbox → draft path) nor edited in the composer. But the
  "the recreated draft is COMPLETE" contract (annuler_envoi_programme)
  is false for this field.
- **Why accepted**: the path does not exist ; adding a dead column
  would be worse.
- **Reopening condition**: the day an invitation reply becomes
  schedulable or editable in the composer — the `drafts` column and
  its copy in both directions are part of the same job.

### D-32 · The gate lives in TWO encodings — pre-push (sh) and gate.ps1 (PowerShell)

- **Fact (review PLAN-KAIZEN-CLAUDE wave 2, 2026-08-23)**: the 9 steps
  exist in sh in `.githooks/pre-push` (with the docs-only fast path)
  and in PowerShell in `scripts/gate.ps1` (without that path — by
  design: the gate before commit is always full). Any step added or
  changed must be done twice, with no safeguard.
- **Why accepted**: the two homes have different needs (the hook
  redirects silent steps, the script shows everything ; the hook
  carries the docs-only path, the script never does) ; unifying today
  would cost more than the risk run.
- **Reopening condition**: the first OBSERVED divergence between the
  two verdicts, or the addition of a 10th step.

### D-33 · A stale dist is only fixed in JS — `build.rs` has no `rerun-if-changed`

- **Fact (review PLAN-KAIZEN-CLAUDE wave 2, 2026-08-23)**: the trap
  "generate_context! only embeds the dist when main.rs compiles" is
  held by `e2e/rebuild-v2.mjs` (fingerprint + bump) and
  `scripts/build-wind.mjs` — but a bare `cargo build`, outside these
  two gates, stays exposed.
- **Why accepted**: adding the dependency in
  `apps/desktop/build.rs` would touch shipped code with a tauri_build
  semantic to prove (a build-script rerun does not imply main.rs
  recompiles) — out of scope for the tooling.
- **Reopening condition**: a stale dist observed OUTSIDE the two
  gates (release or field), or a job that touches build.rs.

### D-34 · The "pref per account" pattern is duplicated at every table (loaders, commands, release script)

- **Fact (review PLAN-RETOURS-9, 2026-08-23)**: `chargerNoms`/
  `patcherNom` (App.svelte) and `noms_get` (commands.rs) are structural
  clones of the markers duo — second occurrence ; each table also pays
  its own `Store::open` on the serialized queue at startup (~a few
  ms). And the `$oauth` table of `make-release.ps1` duplicates the
  `option_env!` of `provider.rs` (cross comments posted on both sides,
  but no safeguard).
- **Why accepted**: at two occurrences the factoring would cost more
  than the duplication (lesson from the repository's patterns) ; the cost
  startup is negligible measured at the workstation scale.
- **Resume condition**: the THIRD pref per account (merge
  into `identites_get` + generic loader), or the addition of an OAuth
  provider (check the script's table BEFORE its first release).

### D-35 · The 16 icon tier is not drawn (V9 debt)

- **Finding (PLAN-ELEMENTS, 2026-08-24)**: the 78 delivered glyphs are
  the 24 masters reduced — only 37% of the coordinates survive the
  24 → 16 pass (multiples of 3), the stroke is 1.33 px at 16 px and
  0.83 px at 10 px (account markers, below the 16 tier itself).
  System costing (V9): 74 16-tiers + 12 10-12-tiers to draw, aligned
  rectangle by rectangle — 86 drawings.
- **CE decision (D4, 2026-08-24)**: deliver the reduced masters; the
  sharpness judged sufficient at the E2 visual STOP on the actual
  render (anti-aliasing, work screen).
- **Reopen if**: the field or the beta sees blur at 16 px or on the
  10-12 px markers — then a dedicated drawing job, glyph by glyph
  (`format_list_numbered` pleads first).

### D-37 · `sync_progress` recounts every mailbox, every 5 s, forever

- **Finding (PLAN-DEMARRAGE, 2026-08-26)**: `store.sync_progress()` is a
  `SUM` of correlated `COUNT`s over every mailbox — **152 ms cold**,
  8.6 ms warm, replayed **every 5 seconds forever**, under the
  global lock. Over the first 60 seconds of a startup, ~1.8 s
  of lock.
- **Reason for deferring (CE decision D6, 2026-08-26)**: its fix is a
  counter kept up to date on write — same family and same drift risk
  as the missing-bodies counter, for ~1.8 s against the ~26 s of the
  main defect. A counter that lies is worse than a slow count.
- **Lead**: a per-mailbox counter kept on write, or a single aggregated
  query instead of the loop (the pattern measured at E1-bis:
  it was the index that carried the cost, not the round trips).
- **Reopen if**: the field points to the cost — fan, battery,
  or perceived latency of the probes at rest.

### D-38 · The preview backfill reloads the list even when it did nothing

- **Finding (contre-expertise PLAN-DEMARRAGE, 2026-08-26)**:
  `rattraperApercus` (`App.svelte`) calls `liste?.recharger()`
  **unconditionally**, outside its loop — so **even when
  `restants === 0` on the first call**, which is the case for any
  up-to-date database. Now `recharger()` bumps the generation, relaunches
  `pomper()` and `lancerEpingles()`: one more `list_category` page, one
  `pinned_rows` and one `category_total`, at t + 1.5 s, for nothing.
- **Reason for deferring**: found during E2, outside the reduced scope
  the CE cut (the `tick` alone). Two lines, but they change a reload
  behavior that deserves its own net.
- **Lead**: capture `const aFaire = restants > 0;` on the first call
  and reload only if `aFaire`. Net: count the `list_category` calls
  in `__e2eJournal` after the tier on a fixture with no missing preview
  — the expected value is zero.
- **Reopen if**: the next job touches the backfill, or the bench sees
  these three commands in the budget.

### D-39 · The Authenticode signature is frozen — installation on an SAC workstation stays a lottery

- **Finding (spike `spikes/maj-x64/`, CE readings of 2026-08-26/27)**:
  Smart App Control (`On` by default on recent Windows 11) judges
  Authenticode-unsigned exe **binary by binary** (cloud verdict by
  hash): 0.10.0 launches, 0.10.1 is refused, on the same workstation.
  Any unsigned release can be blocked for any SAC user — and the
  verdict can change over time.
- **Reason for deferring (CE decision D2, 2026-08-27)**: E1 failed — the
  Azure Trusted Signing individual identity validation is closed
  outside the US/Canada (CE address in France). Costed fallbacks at
  PLAN-SIGNATURE §2 (Certum open source ~€69/year, OV cloud
  ~$200-400/year): "wait + net only" decided. The Azure account
  `rg-fcts` is to be deleted (Basic bills $9.99/month).
- **Lead**: watch for the individual Trusted Signing reopening
  (or Certum if the wait weighs), E2/E3 of PLAN-SIGNATURE are written
  and FROZEN — workstation tooling, `signCommand` injected only by
  `make-release.ps1`, an Authenticode check added to
  `verify-release.ps1` (18 → 20).
- **Beta readings (PLAN-BETA §3 bis register)** — the measurement that
  will reopen this job; one verdict per line, workstation/version/date,
  without identity:
  - 2026-08-31, **T1, x64, SAC workstation `On` (outside CE): 0.15.0
    installs** — first FAVORABLE verdict recorded outside the
    development workstation. It closes nothing (the verdict is
    rendered by hash, so it is silent on the next version), but it
    gives the **bench** the PLAN-SIGNATURE net's due measurement was
    missing: the day this workstation refuses an update, the failure
    must be VISIBLE.
- **Reopen if**: a beta return hits SAC, or the Azure door reopens,
  or the public launch nears (ADR 0013 ties it to the public).

### D-40 · The upstream tauri-plugin-updater issue — OPENED on 2026-08-27 (CE GO)

> **SETTLED as an action**:
> https://github.com/tauri-apps/plugins-workspace/issues/3555
> (title: "updater: ShellExecuteW result is never checked on
> Windows — app exits silently when the installer fails to launch").
> What stays alive is the WATCH: at the crate's next bump (pinned
> `=2.10.1`), check whether upstream has fixed it — the local
> workaround (PLAN-SIGNATURE E4) then becomes a candidate for
> removal.

- **Finding (sources 2.10.1, `updater.rs:854-865`)**: the return of
  `ShellExecuteW` is never tested and the process exits via
  `exit(0)` — any Windows refusal closes the host application without
  a word. Wind is protected by its own launch (PLAN-SIGNATURE E4),
  the rest of the Tauri ecosystem is not.
- **Reason for deferring**: outbound action (publish under the CE's
  GitHub account) — draft to be validated at the job's STOP 2.
- **Lead**: short issue with the lines at fault and the fix (test the
  return > 32, otherwise return the error instead of exiting).
- **Reopen if**: at the crate's next bump (pinned `=2.10.1`) — if
  upstream has fixed it, the local workaround becomes a candidate for
  removal.

### D-41 · The multi-select checkbox has no dedicated keyboard gesture

- **Finding (PLAN-RETOURS-10, 2026-08-27)**: multi-select in the list
  is a pointer gesture — Ctrl-click, Shift-click, hover checkbox. On
  keyboard, e/Delete do apply to the checked batch, but NOTHING lets
  you CHECK without a mouse (no Ctrl+Space, no Shift+Arrows).
- **Reason for deferring**: out of the job's scope (§2.6) — the CE's
  statement targeted the three pointer gestures, and a keyboard
  vocabulary for multi-select deserves its own design (interaction
  with A38's e/Delete triage and the :focus-visible ring on recycled
  nodes).
- **Reopen if**: the field or the beta asks for it — then design the
  full vocabulary (check, extend, clear all) in one pass.

### D-42 · The PER-MESSAGE image memory has no exit door

- **Finding (PLAN-RETOURS-11, fresh-eyes review of 2026-08-28)**: a
  message's "Show images" choice is written to the database
  (`images_messages`, envelope key) but is neither listed nor
  revoked anywhere — the Settings list (D4) only covers sender
  rules (`images_expediteurs`). An inadvertent click on a suspect
  message reloads its remote pixel on every reopening, with no
  visible way to re-block it.
- **Reason for deferring**: scope assumed by the job — CE decision D4
  only settled revocation for senders; a per-message exit door needs
  its own form (where would the gesture live? an inverted banner?).
- **Bound**: the consent dies with its mailbox (CASCADE), on the
  message's local removal (`remove_local`) and on UIDVALIDITY change
  (`reset_mailbox`, purge proven by test) — never inherited by a
  recycled UID.
- **Reopen if**: the field or the beta reports "I want to re-block
  this MESSAGE's images."

### D-43 · The local echo has no Cc column — the header changes at reconciliation

- **Finding (PLAN-RETOURS-12, fresh-eyes review of 2026-08-28)**: the
  `echos` table only copies `outbox.recipients` (the To) even though
  `outbox.cc_addrs` exists; the "Cc: …" line of the A92 header
  therefore never appears during the echo window. A send with Cc,
  opened right away in Sent, shows "To: …" alone, then gains its Cc
  line once the server envelope replaces the echo — two headers for
  the same message depending on timing.
- **Reason for deferring**: a column + migration + recopy for a window
  of a few seconds in normal use; the RETOURS-5 net ("the echo states
  its recipients") stays true for the To.
- **Reopen if**: the field reports a header that "changes on its own,"
  or a job already touches the echo schema.

### D-44 · `connectes` is refreshed by no cycle — a revoked token with Wind open still says "Connected"

- **Finding (PLAN-RETOURS-12, fresh-eyes review of 2026-08-28)**: the
  `connectes` array (App.svelte) is only rehydrated on gestures —
  startup, "Reconnect," and now the add (R1). No cycle (30-min sync,
  5-min poll, opening Settings) sets it straight, and
  `accounts_failed` from the sync summary is never reflected in it: an
  OAuth token revoked while Wind is running leaves Settings > Accounts
  saying "Connected" until restart — the mirror symptom of R1.
- **Reason for deferring**: out of R1's scope (fixed at the gesture,
  every add path covered); the right level is a refresh driven by the
  cycle or a state derived from the core's summary — its own form to
  design.
- **Reopen if**: a field finding of "disconnected shown as connected"
  (the opposite of R1), or at the first job that touches the sync
  cycle.

### D-45 · The System theme visual swatches are the only hex copy outside the gate

- **Finding (PLAN-MONA, fresh-eyes review of 2026-08-29)**: each theme
  lives in four copies — `systeme.css`, `FICHES` (theme.js), the
  System's contract table and its visual swatches. The first three are
  kept in sync with each other by the gates (checks 1 and 3 of
  `system-coherence.mjs`); the swatches are NOT: check 1 only reads
  `data-theme`/`data-jeton` cells, never the `style="background:#…"`
  or `title="--bg #…"` of the swatches. A token retouched after a
  field finding would leave the swatch at the old color, gate green,
  forever. ~80 hex values exposed (4 themes), the
  `title="--jeton #hex"` format is already machine-readable.
- **Reason for deferring**: pattern pre-existing to Elements (PLAN-MONA
  only doubled the exposure); extending check 1 to the swatches is a
  gate job in its own right, out of scope for a theme addition.
- **Reopen if**: a token retouch in the field (the case that
  materializes the drift), or at the next job that touches
  `system-coherence.mjs`.

### D-46 · The Screener's row anatomy is a hand copy of the List's

- **Finding (PLAN-MODE-ORGANISE E2, fresh-eyes review of 2026-08-30)**:
  `Portier.svelte` recopies the central pane's row drawing
  (`l1`/`exp`/`essor`/`heure`/`objet`/`apercu`, unread boldness,
  centered disk) into its own `<style>` — the component promises "THE
  row format" but no mechanism holds it: the next retouch of the
  template in `Liste.svelte` (A83's padding, optical alignment) will
  not reach the desk, silent pixel drift. Only `.disque` already lives
  globally (`systeme.css:100`).
- **Reason for deferring**: promoting the shared anatomy to
  `systeme.css` touches `Liste.svelte` (the hottest component of the
  UI, 8 fragile tests recorded at the e2e audit) — out of E2's scope;
  the current form is the one validated at the visual STOP.
- **Reopen if**: a retouch of the row template (spacing, typography) —
  the finding "the Screener did not follow" materializes it —, or at
  job E4 (the organized Inbox reuses these rows as sections).

### D-47 · Three context menus and two thread toggles are hand copies

- **Amended (PLAN-AUDIT-V2 E11, 2026-09-02, A108) — the MENUS are
  settled**: `Menu.svelte` is THE product's menu (eight surfaces —
  List, Feed, Screener, Cleanup, Paper trail, section sort, Settings >
  Screener, the thread's "Move to…"), drawing AND mechanics in one
  copy (keyboard included, A8 held), 24 CSS copy rules removed, the
  `--ombre` token that did not exist along with them. **Still open**
  is the "core" half of this debt: `toggle_mis_de_cote`/
  `etat_mis_de_cote`, twins of `toggle_pin`/`pin_state`, and the Paper
  trail's stack/rank recopied from the Feed — audit wave 3.

- **Amended (RETOURS-14, 2026-08-31)**: two more copies — the grouped
  Paper trail's `.menu-groupe` (`Registre.svelte`), and the FAMILY
  extends to the STACK drawing (`.empile`/`.rang-groupe` recopied from
  `Kiosque.svelte` to `Registre.svelte`) and to Cleanup's two-line row
  (`.l1/.l2` recopied). The verdict vocabulary, meanwhile, was
  factored out along the way (`lib/portier.js`).
- **Finding (E4/E5 review, 2026-08-30)**: the product menu's drawing
  lives in three CSS copies (`Portier.svelte` `.menu`, `Liste.svelte`
  `.menu-gestes`, `PileMisDeCote.svelte`'s fan) — the `0 8px 24px`
  shadow is already written three times there, `min-width` diverges by
  10 px for no reason, and only Screener goes through
  `var(--ombre, …)`. On the core side, `toggle_mis_de_cote`/
  `etat_mis_de_cote` are the structural twin of `toggle_pin`/
  `pin_state` (~80 lines, only the table changes), and
  `pile_mis_de_cote` is the twin of `pinned_unified_scoped`.
- **Reason for deferring**: factoring the menu = a shared component
  touching three surfaces validated at the visual STOP; the core
  twins are each covered by their own tests — the refactor brings
  nothing to the field for the release underway.
- **Reopen if**: a shadow/menu token enters the theme table (the copy
  would drift at the first retouch), at the third twin (E6 groups), or
  at the next retouch of the thread resolution contract (the RED
  "never the head" would need to be carried twice).
- **REOPENED on 2026-08-30 (PLAN-HORIZON-NETTOYAGE, review)**: Spring
  cleaning is the announced twin — `Nettoyage.svelte` recopies the
  Screener's ⋯ menu whole (markup, `ouvrirMini` and its 250/170 bounds
  hard-coded, `BOITE_DE`/`TOAST_NON` cards, `.btn-portier`/`.mini`/
  `.menu` CSS), a 4th copy of the drawing. Not factored within the job
  (three surfaces validated at the visual STOP, same reason as at the
  deferral) — **to be handled as a dedicated debt**: a shared
  `MenuVerdict.svelte` for Screener/Cleanup, and the common classes in
  `systeme.css` (the earlier `.entete-vue`). Add to it the `select`
  style pair born in two copies (AccountDesk 40 px / Settings 32 px).

### D-48 · The list does not follow an external write

- **Finding (RETOURS-13 review, 2026-08-30)**: the Inbox only reloads
  on a poll generation's beat or through its own gesture handlers. A
  `retirer_routage` (or any write outside the List's paths — second
  workstation, replay, e2e command) leaves the list stale until a
  manual navigation. The e2e step of `organized-mode.spec.js` that
  "passed" lived off a FORTUITOUS reload of the probe — it now holds
  honestly through the folder round trip. Same family as D-44
  (`connectes` with no refresh cycle).
- **Reason for deferring**: the proper fix is a generic invalidation
  signal (bump the generation on any core write that changes a view),
  not one more `liste.recharger()` wired per surface — a job, not a
  retouch.
- **Reopen if**: a field finding of "the list does not move" on a
  gesture outside the List, or at the multi-window/second-workstation
  job.

### D-55 · The database, the disk files, the `prefs` keys and the localStorage keys stay French

- **Finding** (PLAN-BASCULE-ANGLAIS, Chief Engineer decision D3 of 2026-09-02;
  GLOSSARY §1.6): the SQLite schema (26 tables, ~30 French columns),
  the six `prefs` keys, the files on disk (`wind.db`, `wind.log`,
  `maj.log`, `telemetry.json`, `discovery.db`) and the browser
  `localStorage` keys (`wind-theme`, `wind-volets`, `wind-largeurs`,
  `wind-espacement`, `wind-accueil-*`) keep their French names while
  every identifier around them is English. The PLAN and the glossary
  cite this debt as “D-54” — that number was already the flaky spec
  above; it is D-55 from here.
- **Why deferred**: renaming a column is a migration on every tester's
  database; renaming a storage key silently resets their layout. Not a
  behavior change the switch may embed (§5).
- **Done on the way (E5b, 2026-09-03)**: two persisted VALUES did move,
  with a read-side legacy map and no reset — the pane width shape
  (`{ nav, liste }` → `{ nav, list }`) and the row spacing levels
  (`faible|moyen|eleve` → `low|medium|high`). The keys themselves are
  untouched.
- **Reopen if**: a schema migration is scheduled for another reason
  (rename the columns in the same migration), or the storage keys get a
  versioned envelope.

### D-56 · Shell-composed text stays French while the UI may be English

Opened on 2026-09-03 (PLAN-BASCULE-ANGLAIS E5, CE decision D17). The size
units of `human_size` (`o`, `Ko`, `Mo` — attachments, drafts, the outbox),
the two native dialogs of `main.rs` (second instance, failed relocation)
and the one shell error string a spec asserts are composed by the shell in
French, marked `lang:fr`, whatever the UI language. The clean fix is a
behavior change the switch refuses to embed (§5): send bytes on the wire
and format in the UI per language; give the dialogs an English text when
`prefs.lang` is `en`. A small dedicated job once the switch is closed.

**Seen in the field on 2026-09-03 (E6b)**: in the English interface the
compose weight reads `2.8 Mo / 25 MB` — the total from `human_size`
(shell, French), the limit from the catalogue (English). The spec
asserts it as shipped (`redesign-screen02.spec.js`).

### D-57 · The onboarding illustrations are French screenshots inside an English default UI

Opened on 2026-09-03 (PLAN-BASCULE-ANGLAIS E6b, Chief Engineer decision
D28). `assets/accueil/disposition-{1,2,3}.png` are screenshots of the
French interface, captured by `e2e/capture-onboarding.mjs` (pinned to
`lang: 'fr'` so a replay does not change a visible asset without a
decision). The rule the Chief Engineer set: **every screenshot shown to
the user is in the language the user chose** — one set per language,
selected with the catalogue. To do at the next onboarding job: capture
both sets, select per `lang`, unpin the script.

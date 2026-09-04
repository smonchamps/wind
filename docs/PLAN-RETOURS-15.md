# PLAN-RETOURS-15 — the organized Reception gains the reading pane in 3-pane mode; the pending measures settled

> Opened 2026-09-04. First job sourced from the **beta** (wave 1).
> Statement (Chief Engineer): *on Reception in organized mode, add a visualization
> pane when the 3-pane mode setting is active. It reverses a decision I
> took earlier. The decision is reversed based on feedback from the
> beta.*
> Folded in by the Chief Engineer: the four pending items of STATE — the S4
> measure, the SAC net, the D-50 confirmation, the D-51 call — and the
> GitHub purge record.

---

## §1 Finding (the facts, verified 2026-09-04)

**The decision being reversed.** PLAN-MODE-ORGANISE E4 (2026-08-29,
journal **A99**): *"the Organized Inbox: WITHOUT a reading pane — the
list spans from nav to right edge, rows in a centered 760 px column …
a click opens SCREEN 03"*. It was written as the E4 spec (archive
table row and A99), **not** as a numbered D1-D9 decision, and no ADR
covers it. The reversal is therefore recorded here (§ Chief Engineer decisions,
D1) and in a new System journal entry — nothing to amend in an ADR.

**Where the rule lives in the code** (all verified in place):

- `App.svelte:1478-1480` — `organizedInbox` derived, comment stating
  the rule ("regardless of the panes setting").
- `App.svelte:1491-1493` — `sceneWithoutReading` = screener ∪ cleanup
  ∪ organizedInbox ∪ feedCards; ONE predicate (review 2026-08-30).
- `App.svelte:1499` — `onSelection`: `panes === 3 && !organizedInbox`
  routes to `reading.open(row)`, else `conversation.open(row)`.
- `App.svelte:1838` — `Reading` mounted only when
  `panes === 3 && !sceneWithoutReading`.
- `List.svelte:77-85, 1562` — the `center` derived paints the
  centered ~760 px column; the two sections ("New for you · n" /
  "Previously seen") are built at lines 97-103 and exist regardless
  of centering.
- The pane setting: `lib/panes.svelte.js`, `localStorage['wind-volets']`,
  values 3/2/1 (PLAN-VOLETS, A26). In classic mode, 3 panes =
  nav 248 / list 400 / reading 1fr (System, screen 02).
- **The net that proves the old rule**: `organized-mode.spec.js:385-395`
  asserts `reading-pane` count 0 in the organized Inbox. It will be
  the RED of E1 — rewritten to prove the NEW rule, plus screen 03
  still served at 2/1 panes.

**The four pending items** (register quotes verified):

- **S4** (PLAN-MODE-ORGANISE §5): daily FLOW of senders with no
  routing line, on both Chief Engineer workstations, one week engaged 2026-08-30
  — the week is over; the figures are with the Chief Engineer. Sizes the
  Screener's ergonomics.
- **SAC net** (D-39/D-40): the due measurement is an update failure
  VISIBLE under a real Smart App Control refusal. T1 (x64, SAC On)
  gave the bench on 2026-08-31; the proof needs a refusal that has
  not happened. **Not closable by us** — it stays armed; this job
  only re-records it.
- **D-50**: Microsoft renewed-refresh-token storage, to confirm
  beyond 90 days of 2026-09-02 (≈ 2026-12-01) — a calendar wait, not
  closable now; re-recorded with the absolute date.
- **D-51**: the reopening condition FIRED (Chief Engineer account 3 is
  CONDSTORE-less, `wind.log` line, PLAN-AUDIT-V3 field). The call —
  pay a bounded `FETCH FLAGS` window for CONDSTORE-less accounts, or
  keep the log line — is the Chief Engineer's (D4 below).

**GitHub purge**: resolved. `gh api repos/smonchamps/wind/commits/051bb01`
answers **422 "No commit found for SHA"** on 2026-09-04 (STATE
predicted a 404; the API returns 422 for a short SHA — same meaning:
the object no longer resolves). To record in STATE (E4).

## §2 Scope

**In**: the reading pane on the organized Reception at `panes === 3`;
the e2e nets rewritten and extended; the System amended (A-n); STATE,
DEBT, the S4 figures recorded; D-51 per the Chief Engineer call (D4).

**Refusals (§2.6)**:
- **Screener, Feed cards, Cleanup keep no pane** — they are reading
  scenes or verdict scenes, not lists; the beta feedback names
  Reception only. Reopen on a new field reason.
- **The Paper trail is untouched** — it already uses the reading pane
  (R6/D7 of RETOURS-14).
- **2-pane and 1-pane unchanged** — screen 03 remains their reader
  (V-D2); the feedback targets the 3-pane setting.
- **No new preference** — the existing `wind-volets` setting decides;
  organized mode stops overriding it, that is the whole feature.
- **SAC net and D-50 produce no code** — waits re-recorded with dates.

## §3 Options on the one hard point — the 3-pane organized layout

The centered 760 px column and a 1fr reading pane cannot coexist in
the screen-02 grid (nav 248 + 760 + a useful pane exceeds nothing at
wide widths but starves the pane at common widths; and the centered
column was designed for a list that spans to the right edge).

- **Option A — the standard screen-02 grid** (recommended): at
  `panes === 3`, the organized Inbox renders in the 400 px list
  column, sections kept, centering off (`center` gated on
  `panes !== 3`); the reading pane takes 1fr, identical to classic
  mode. Zero new geometry, zero new CSS surface, the sticky section
  band (RETOURS-14) already works in a scrollport. At 2/1 panes the
  centered column and screen 03 survive unchanged.
- **Option B — centered column + pane**: a bespoke grid. New
  geometry, new contrast/windowing surface, and it contradicts the
  reason the column exists (it was the compensation for having NO
  pane). Rejected unless the Chief Engineer wants it (D1).

No spike: the two options differ by a gate on an existing derived,
not by an unknown to measure.

## §4 Steps

- **E1 — the reversal** (TDD): rewrite the organized-mode net RED
  (pane present at 3 panes, sections intact, screen 03 still the
  reader at 2/1 panes), then: `organizedInbox` leaves
  `sceneWithoutReading` when `panes === 3`; `onSelection` drops the
  `!organizedInbox` guard; `List.svelte` `center` gated on the pane
  count; the A99 comment blocks rewritten. Re-section semantics per
  D2. Inner loop: `organized-mode.spec.js`, `redesign-panes.spec.js`,
  `feedback-14-inbox.spec.js` as whole files.
- **E2 — early visual STOP**: first render in the Chief Engineer's hands (the
  sort bar lesson, A58) — verdict on the look before rolling out the
  rest (sticky band, badges, return path).
- **E3 — D-51 per D4** (if GO): bounded `FETCH FLAGS` window for
  CONDSTORE-less accounts only, RED first on a mail-imap/mail-core
  test; else: DEBT re-worded, call recorded.
- **E4 — records**: S4 figures written where the Chief Engineer hands them
  (PLAN-MODE-ORGANISE close-out + STATE), GitHub purge recorded in
  STATE (ticket closed), D-50 re-dated (≈ 2026-12-01), SAC net
  re-stated as armed-and-waiting.
- **E5 — quality**: fresh-eyes review (`/code-review high`), full
  gate, System journal A-n in the same commit as the UI change
  (DC-D2), STOP 2 field checklist, commit, push, CI.

## §5 Chief Engineer decisions

| # | Question | Answer (date) |
|---|---|---|
| D1 | Reversal confirmed + layout: Option A (standard 400 px list column + 1fr pane) or Option B (centered column + pane)? | **Option A — standard grid** (Chief Engineer, 2026-09-04) |
| D2 | When does a thread read in the pane leave "New for you"? (a) when the selection moves on / the list is next re-served — the row never jumps under the open reading; (b) immediately on click. | **(a) when the selection moves on** (Chief Engineer, 2026-09-04) |
| D3 | Version: the change is user-visible behavior → next release MINOR (0.18.0), carrying audit wave 3 with it (its D5). Confirm? | **Yes — 0.18.0 MINOR**, this job + audit wave 3 ship together (Chief Engineer, 2026-09-04) |
| D4 | D-51: pay the bounded `FETCH FLAGS` window for CONDSTORE-less accounts (account 3 gets flag resync; cost bounded, gated to those accounts), in this job (E3) — or keep the log line and close the call as "declined for now"? | **Yes, in this job (E3)** (Chief Engineer, 2026-09-04) |
| D5 | S4: hand over the figures from both workstations (flow of unknowns/day since 2026-08-30) so they are recorded — or extend the measure? | Chief Engineer asked *where* the figures live — answer: in `wind.db`, nothing recorded them elsewhere. **Workstation 1 MEASURED 2026-09-04** (query anchored on `routage_expediteurs.epoch > arrival`): 5 / 18 / 18 / 9 / 19 / 11 per day 08-30 → 09-04, **~13/day, peak 19** (80 senders / 6 days; 186 routing lines decided). Workstation 2: same read-only query handed to the Chief Engineer, figures due before STOP 2 (E4 waits). |

## §6 Delivery record

- **2026-09-04 — STOP 1 played, D1-D4 settled, GO.** S4 workstation 1
  measured in session (D5); the naive "still unrouted today" count
  (~6/day) undercounts — verdicts erase their senders; the honest
  query tests "no routing line whose `epoch` precedes the arrival".
- **2026-09-04 — E1 delivered (TDD)**: RED shown on the rewritten
  organized-mode net (pane absent), then GREEN — `organizedInbox`
  leaves `sceneWithoutReading` at 3 panes, `onSelection`/keyboard
  advance/`backToMailbox` share the ONE predicate, the grid class and
  the List centering gate on the pane count. Trap found and fixed:
  `center` conflated the geometry with the R2 dressing (normalized
  header, no tabs) — split into `organizedInboxView` (mode property)
  and `center` (paneless geometry); `feedback-14-inbox.spec.js`
  caught it. Two spec sites still encoding the old rule updated (the
  Set-aside toggle test, the mixed-thread badge test — the badge now
  asserted in the pane). Specs green in isolation: organized-mode
  17/17, redesign-panes 9/9, feedback-14-inbox 7/7. System **A117**
  (reversal recorded; A99/A104 rules held at 2/1 panes).
- **2026-09-04 — E2 played**: three captures (3-pane light/dark,
  2-pane control) handed to the Chief Engineer. Flagged for the
  verdict: the Set-aside pile's placement beside the pane — settled by
  the review fix (hidden behind an OPEN reading, visible on an empty
  pane) and **accepted at STOP 2**.
- **2026-09-04 — E3 delivered (TDD)**: RED shown (2 sync tests + the
  store guard test), then GREEN — `MailServer::fetch_flags` (ImapServer:
  ONE `UID FETCH … (UID FLAGS)`; FakeServer records batches;
  ShellServer delegates), `Store::apply_flags` (changed rows only,
  thread counters refreshed, a queued local intent WINS over the
  window), engine window = 500 most recent UIDs (`DEFAULT_FLAG_WINDOW`,
  proven bounded with a window of 1), CONDSTORE servers pay nothing
  (proven). `wind.log` D-51 line reworded. mail-core 465/465,
  mail-imap + desktop green. **DEBT D-51 closed** with its stated
  limit (stale beyond the window).
- **2026-09-04 — fresh-eyes review** (five finder angles, verified):
  ten findings, **nine fixed in session**, one skipped. Fixed: (1) the
  D-51 blind spot the review EXPOSED beyond its own finding — on a
  CONDSTORE-less server a flag-only change moves neither EXISTS nor
  UIDNEXT, `must_poll` rightly skipped the mailbox and the window
  never played; a **light flags pass** now runs when the guarded poll
  skips a CONDSTORE-less INBOX (`SyncEngine::flags_pass`: window from
  the STORE's own UIDs, one `(UID FLAGS)` round trip, zero inventory —
  ADR 0017's sobriety holds, proven by the new cycle net); (2) flags
  applied now bump the UI generation (`SyncReport.flags_applied`,
  cycle bump — without it the database updated and the row stayed
  bold); (3) the `apply_flags` guard filters `refusee = 0` (a
  quarantined action no longer freezes the window forever — RED
  proven); (4) `apply_flags` in ONE transaction, touched threads
  refreshed once apiece; (5) the Set-aside pile hides behind an OPEN
  pane (it floated over the reading, z 20 vs 1); (6) ONE
  `organizedPaneless` derived (three raw copies removed); (7)
  `sections` derives from `organizedInboxView`; (8) seen/star parsing
  shared in `convert::seen_flagged`; (9) the window selection is a §4
  pure decision (`flag_window_uids`, partial selection, unit-tested)
  and the stale `without_condstore` rustdoc reworded. **Skipped
  (stated limit)**: the CONDSTORE delta path (`upsert_envelopes`)
  writes flags without the pending-intent guard — pre-existing, the
  head-of-sync replay narrows the race to gestures made mid-poll; to
  fix at the next job touching `upsert_envelopes`, not by a special
  case here. Tests: mail-core 465 → 469.
- **2026-09-04 — E4 records**: GitHub purge resolved in STATE (422 on
  the short SHA — same meaning as the predicted 404); S4 workstation 1
  figures in STATE; D-50 re-dated ≈ 2026-12-01; SAC net re-stated as
  armed. **Workstation 2's S4 measured on 2026-09-04** (Chief
  Engineer, same query): 4 unknown senders on 2026-08-30, none after —
  a quiet workstation; the Screener's sizing rests on workstation 1's
  ~13/day, peak 19. **S4 is CLOSED.**
- **2026-09-04 — ⛔ STOP 2: field validation by the Chief Engineer,
  "All ok", zero KO** — the 3-pane organized Reception with its pane
  (sections, the D2 hold-then-reserve, the pile placement accepted),
  2/1 panes unchanged, the D-51 light pass on account 3, both themes.

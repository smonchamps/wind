# PLAN-INPROGRESS-2026-09 — the backlog items marked "In progress"

> **JOB CLOSED on 2026-09-09 — full field validation.** STOP 1 GO on
> the eight Chief-Engineer decisions D1–D8 + D3-bis; STOP 2 round one field verdict
> (workstation, real accounts): items 2,3,4,5,7 OK, item 6 untestable
> without a live cross-client invitation, three field follow-ups
> asked; STOP 2 round two verdict **"All ok, commit and push"** (window
> size/position remembered, junk "Empty" inline confirmation, offline
> reply leaves no draft). One commit `dcf5112` (39 files, +1158/−659),
> **CI GREEN run 34404369730** (all seven legs — Windows x64/arm64,
> macOS x64/arm64 clippy+tests, cargo audit, ui-v2 build, Node suites).
> Review: 10 findings, 7 fixed, 3 accepted-and-recorded. System
> A141–A149; ADR 0014 retired.
>
> **Kaizen (PLAN-KAIZEN-CLAUDE):** T1 ≈ **59.2 M** input equiv. (35.1 +
> 24.1 M over the two session IDs a context reset split the job across;
> 8.8 M of it in the review's 9 Sonnet agents). **W3 full gates: 7**
> (four andons round one — fmt, dead doc links, maximized-window e2e
> geometry ×1; and round two — language ratchet CE-abbreviation trap,
> the junk-confirm banner overflow) — well over the ≤3 target: an
> eight-item plan plus three field follow-ups is a *batch*, not a job,
> the same T1/W3 blow-out the multi-item audits showed (kaizen D5).
> **Quality guard: 0 KO on the delivered increments at both STOP 2
> passes**; the three follow-ups were field-originated (two adjustments
> + one pre-existing offline-reply bug), each fixed the same session.

> Statement (Chief Engineer, 2026-09-09): *"Process Wind backlog items
> marked as 'In Progress' in Notion's 'Backlog Wind' database."*

Opened: 2026-09-09. Closed: 2026-09-09. Baseline: `a8f999f` (main,
0.21.0 published). Delivery: `dcf5112`.
Method: [WORKFLOW](WORKFLOW.md), [job](../.claude/skills/job/SKILL.md).

---

## 1. Finding (what was verified on the evidence)

The database holds **eight** items with status "In progress" (queried
2026-09-09; IDs are the backlog's auto-increment IDs). Code
reconnaissance ran on this workstation; every claim below carries its
file and line.

### Item 85 — remove the bar proposing to send information

- The bar is the **telemetry notice** (`App.svelte:818-850`), shown
  while consent is `unset`. Its wording promises only a **local**
  crash report: `telemetry.rs` (ADR 0014) writes crash files to the
  app data dir; **no upload code path exists anywhere** — the user
  opens the folder and sends the file by hand.
- It sits second-to-lowest in the single-notice priority chain
  (`App.svelte:215`), so it lingers for days on a fresh install and
  resurfaces whenever higher notices clear — the tester reads it as
  nagging.
- Settings already exposes nothing for it: the only way to answer is
  the bar itself. No e2e clicks its actions; startup specs mock it
  away (`startup.spec.js:53-54`).

### Item 97 — fold everything in the Feed for a compact unread view

- Fold state is per card, in memory (`Feed.svelte:185-187`):
  unread arrives unfolded, read arrives folded (R10). No section-level
  action. Section headers (`Feed.svelte:423,438`) each carry a
  `SectionSort`.
- **The read-dwell invariant already holds**: the read witness is only
  rendered on an unfolded unread card (`Feed.svelte:405`); folding
  removes it and cancels the dwell timer (`Feed.svelte:298-303`).
  A folded card can never be auto-marked read.

### Item 107 — select all, and empty the junk folder

- Multi-selection exists (`List.svelte:874-937`: click modifiers,
  checkboxes, batch bar with delete) but **no select-all** anywhere;
  the main key handler ignores modifier combos (`App.svelte:1330+`).
- Ranges only ever cover the **served** rows (`orderedRows()`,
  `List.svelte:887`) — the list is windowed, so "select all" can honestly
  mean "all served conversations", not "the whole folder".
- Batch delete is one IPC call, all-or-nothing
  (`act_on_group`, `commands.rs:2289-2322`).

### Item 109 — adapt the shortcuts to macOS

- **No platform detection exists in the UI** (zero hits for any
  platform probe in `ui-v2/src`). Only the literal `Delete` key
  deletes (`App.svelte:1358`); `Backspace` — the Mac delete key — does
  nothing. The shortcut reference (`Settings.svelte:94`,
  `catalog.*.js` `shortcut.key.*`) shows fixed Windows labels on every
  platform. First report from a Mac seat; PLAN-MACOS never touched
  shortcuts.

### Item 100 — an answer sent from another client does not show on the invitation

- Root found, two halves:
  - `mail-core/src/invitation.rs:191` — `scheduling_state()` compares
    a request against peers filtered to `request|cancel`; **reply rows
    are excluded**, so an incoming reply never updates the original
    invitation card.
  - `ui-v2/src/lib/invitation.js:113-121` — the request card shows
    only *our last answer sent from Wind* (or the request's own
    participation field); the "X accepted" line renders only on the
    reply message itself.
- So the two messages stay visually independent by construction. The
  fix is a reconciliation (reply rows folded into the request's shown
  status), testable with fixtures on this workstation — but the exact
  expected behavior needs the Chief Engineer's reading of the report
  (whose answer: ours from another client, other attendees', both?).

### Item 86 — first Gmail add-account says permissions were refused

- `ensure_mail_scope` (`mail-auth/src/flow.rs:232-250`) fails when the
  token's granted scopes miss the mail marker; Google issues a token
  even on partial consent. Nothing in the code differs between a first
  and a second attempt — any asymmetry comes from browser/Google
  session state. **Not reproducible here without a throwaway Google
  account and a virgin browser profile**; needs the exact on-screen
  message and, ideally, one reproduction.
- Adjacent defect, certain and fixable now: the raw English Rust error
  is interpolated into the French UI (`AccountDesk.svelte:62`, no
  dedicated catalog key).

### Item 114 — the window always opens maximized

- The config asks for a plain 1000×700 window (`tauri.conf.json:13-20`,
  no maximize/fullscreen flag), no window-state persistence exists,
  and no code calls maximize. **The code does not do what the report
  says** — the fact is unverified: which machine, which OS, maximized
  or macOS full-screen? A measurement is needed before any hypothesis.

### Item 106 — P0, the tester's inbox synchronization stalled at 72%

- Already sequenced in STATE as the next `/field`, **on the tester's
  trace** (T2, Apple Silicon; T3 as same-architecture witness).
  Blocked on the trace arriving; §7.1 — no way to reproduce her
  account from here. Not part of this plan's build scope.

## 2. Scope

**In (buildable and provable on this workstation now):**

- E1 — item 85: drop the telemetry consent feature entirely (D1).
- E2 — item 97: fold-all/unfold-all on the Feed's unread section.
- E3 — item 107: select-all (Ctrl+A / Cmd+A) over served rows + an
  "empty" gesture on junk and trash heads.
- E4 — item 109: platform-aware delete key and shortcut labels
  (coded here, field-verified on the Air, as STATE already plans).
- E5 — item 100: invitation reply reconciliation, per D5.
- E6 — item 86's certain half: localize the missing-scope error into
  a proper catalog message telling the user which box to tick.
- E7 — item 114 (per D7): the window opens **maximized** at launch,
  every platform.

### Refusals (what we do not do, and why)

- **Item 106**: it is a `/field` on the tester's trace, not a build —
  starting it blind would violate the method's first rule. It stays
  the queue's head for the day the trace lands.
- **Item 86's root**: no blind fix on an unreproduced first-run
  asymmetry; we ask for the measurement (D6) and fix the message now.
- **No window-state persistence** (item 114): D7 asks for maximized
  at launch, not remembered geometry; no plugin is added.
- **No whole-folder select-all**: selection stays over served rows;
  "empty junk/trash" is its own explicit gesture, so the honest
  primitive and the expected gesture each exist without pretending
  the windowed list is the folder.
- **Notion is not written back by this session** (the register is the
  Chief Engineer's, out-of-repo rule in PLAN-BETA § 3 bis); statuses
  move at `/close` time, by hand or on an explicit instruction.

## 3. Design sketches

- **E2**: one button in the unread section header (beside
  `SectionSort`), toggling a section-wide override; a manual per-card
  fold still wins afterwards. Folded stays unmarked — the witness
  mechanism already guarantees it; the e2e proves it by breaking (a
  folded unread card must never auto-mark).
- **E3**: Ctrl+A / Cmd+A outside any input selects all served rows into
  the existing `checkedRows`; the existing batch bar does the rest.
  "Empty" on junk/trash heads runs the existing all-or-nothing group
  delete over the served set, with a count in the confirm wording.
- **E4**: one `isMac` probe (user-agent platform, no new plugin);
  `Backspace` = delete only on macOS (never on Windows); catalog keys
  render per platform (Cmd/Backspace/Escape glyphs on the Mac side).
- **E5**: fold reply rows into `scheduling_state()` (or a sibling
  aggregate) so the request card's shown status reflects the latest
  answer; the reply message keeps its own line. Exact rule per D5.

## 4. Steps and gates

Strict TDD per step; targeted inner loop (whole spec files); one
full `/gate` before the commit; early visual STOP for E2/E3 (first
rendering increment goes to the Chief Engineer before rollout).
E4's field check runs on the Air at STOP 2.

| Step | Item | Test first (RED) | Gate |
|---|---|---|---|
| E1 | 85 | startup spec: no telemetry notice path per D1 | targeted |
| E2 | 97 | feed spec: fold-all folds, unfolds, never marks read | targeted |
| E3 | 107 | multi-select spec: Ctrl+A serves rows; empty gesture on junk | targeted |
| E4 | 109 | keyboard spec: Backspace deletes only when the Mac probe is on; label snapshot per platform | targeted |
| E5 | 100 | Rust: `scheduling_state` folds a reply; e2e: request card shows the reply's status | targeted |
| E6 | 86 | ui spec: missing-scope failure renders the catalog message | targeted |
| E7 | 114 | Rust: the deferred window is created maximized (seam-level assert) | targeted |
| — | all | full `/gate`, review `/code-review high`, commit, push, CI | full |

E7 field note: the gate's e2e step proved review finding 9 for real —
three geometry-sensitive suites went red under the maximized window
(pane widths scale with window width). The e2e build now pins
`maximized: false` through `rebuild-v2`'s existing `windowOverride`
seam (launch + the two bench scripts); the product keeps D7's
maximized launch.

Honesty notes on the REDs: E1's removal has no meaningful RED (the
e2e harness forces telemetry off by `is_e2e()`; the IPC-contract net
is the removal's net) and E7 is config-only (a config-echo test would
teach nothing — the field proves it). Both stated per the method's
never-fake-a-RED clause.

## 6. Delivery record (2026-09-09)

- RED wave A played and shown: the two new e2e tests failed exactly at
  the missing `feed-fold-all` / Ctrl+A surfaces (11 prior tests green
  around them). Rust RED shown: the two invitation tests failed at the
  reconciliation asserts.
- E1 telemetry removed (telemetry.rs, crash.rs, five commands, notice,
  catalogue keys, e2e allowlists; ADR 0014 retirement note).
- Early visual STOP passed 2026-09-09 (four screenshots): one finding
  — the section button said "Fold all" while the cards say "Collapse"
  — fixed on the spot ("Collapse all / Expand all", one vocabulary),
  then *"OK — continue"*.
- E2 fold-all (`feed-fold-all`, A142), E3 select-all + junk Empty
  (`empty-folder`, A143), E4 platform probe + Backspace + labels
  (A144), E5 wire-reply reconciliation in `refresh_invitation_group`
  (A145, mail-core 592 tests green), E6 `missing_mail_scope` marker +
  localized message (A146), E7 `maximized: true` (A147).

## 5. § Chief Engineer decisions

- **D1 (item 85)** — the telemetry bar: retire it entirely and move
  the opt-in to Settings (recommended: the report is local-only, the
  bar has no urgency to justify a notice), or keep the bar but show it
  only once, or drop the telemetry consent feature altogether?
- **D2 (item 97)** — fold-all: on the unread section only (the
  reported need), or on both sections?
- **D3 (item 107)** — the "empty" gesture: junk and trash both
  (recommended — the reporter names the expectation "everywhere
  else"), or junk only, or select-all alone with no head button?
- **D4 (item 109)** — GO to code E4 here with field proof on the Air
  at STOP 2?
- **D5 (item 100)** — whose answer must the invitation card reflect:
  only our own sent from any client (recommended reading of the
  report), any attendee's latest answer, or both (ours + a per-attendee
  line)?
- **D6 (item 86)** — measurement request: the exact message shown at
  the failed first run, and whether the second run ticked boxes that
  the first did not. Meanwhile, GO on E6 (the localized message)?
- **D7 (item 114)** — measurement request: which machine and OS shows
  the always-maximized launch, and is it maximize or macOS
  full-screen? (No build until the fact exists.)
- **D8 (item 106)** — has the tester's trace arrived? If yes, the
  `/field` takes priority over this whole plan; if no, this plan runs
  while we wait.

### STOP 2 round two (2026-09-09) — "All ok, commit and push"

The three follow-ups field-validated the same day: window size/position
remembered across launches (maximized first launch), the junk "Empty"
inline confirmation, and — tested offline — a reply sent immediately
leaves no draft behind and no "stale" notice. Item 6 (invitation
status) stays unverified: it needs a live invitation accepted from
another client. Full gate GREEN (290 e2e, 0 flaky).

### Field verdict (STOP 2, 2026-09-09) and its follow-ups

Workstation pass: items 2,3,4,5,7 OK; 6 (invitation) untestable without
a live invitation; two adjustments + one fresh field bug:

- **D7-bis (item 114)**: the Chief Engineer wants the window to REMEMBER the size
  the user leaves. `tauri-plugin-window-state` added: first launch
  maximized (the config), then the user's geometry restored on every
  later launch, saved on close. Skipped under the e2e harness (the
  suite pins the window size). A148/A147.
- **Empty confirmation (item 107)**: the Chief Engineer asked for a confirmation
  before the junk "Empty" deletes. Inline in the banner, the Compose
  R3 grammar (alert warning + red confirm + cancel), armed on the
  first click. A148.
- **Offline-reply draft zombie (new field finding)**: reply offline,
  send immediately → the message is sent but a draft remains, with
  "draft editing session is stale". Root traced and reproduced with a
  store test: consuming a draft-edit session deleted the source draft
  gated on a matching `updated_epoch`; a drift orphaned it while its
  token was removed. Fixed: the send removes the local draft by its id
  alone. A149.

### Answers (STOP 1, 2026-09-09)

- **D1**: *"Drop telemetry consent entirely"* — the notice and the
  consent go; crash reports stay off. ADR 0014 gets a retirement note.
- **D2**: *"Unread section only (Recommended)"*.
- **D3**: *"Junk and Trash (Recommended)"*.
- **D4**: *"GO (Recommended)"*.
- **D5**: *"Only our own answer (Recommended)"*.
- **D6**: *"GO on E6, measurement asked (Recommended)"* — the item-86
  measurement request stands (exact message at the failed first run;
  did the second run tick boxes the first did not).
- **D7**: *"It's the opposite: I want the app to open fullscreen at
  launch, which is not the case today"* — item 114 is a **request**,
  not a bug. Follow-up answered: *"Maximized (Recommended)"* — the
  window opens maximized (title bar and taskbar stay), not borderless
  full-screen. Becomes step **E7**, in scope.
- **D8**: *"Not yet — run this plan"* — the P0 `/field` stays queued
  at the head for the day the trace lands.
- **D3-bis (2026-09-09, hard point at E3)**: Wind's `delete` is
  move-to-trash by construction (`mail-imap/src/lib.rs:1306`); an
  "Empty" on the Trash itself would be a visible no-op — true emptying
  is a new permanent-delete capability (IMAP `\Deleted` + EXPUNGE,
  offline queue, echoes). Chief Engineer: *"Junk now, Trash as its own
  job (Recommended)"* — this plan ships Ctrl+A everywhere and the Junk
  "Empty" button; the Trash capability goes to the backlog.

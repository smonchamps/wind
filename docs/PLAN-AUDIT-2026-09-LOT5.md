# Lot 5 — Architecture, offline/recovery and measured scale

Opened: 2026-09-07. Baseline: `2ade544` (Lot 4 committed locally on top of
Lot 3's `9f4a9b1`; neither pushed nor closed — see D0).
Parent: [audit program](PLAN-AUDIT-2026-09.md), E13–E15.
Method: [WORKFLOW](WORKFLOW.md), [job](../.claude/skills/job/SKILL.md).

**Status: STOP 1 passed on 2026-09-07 (D0–D5, D7 answered; D6 pending
figures). Sub-lot E13 implemented, reviewed, full gate GREEN (322 s);
field pass 1: items 1, 3–7 OK, item 2 KO fixed the same day (reads off
the commands' lock, net `reads-under-write.spec.js`), gate GREEN again
(300 s); second field pass “2 OK”. **E13 committed locally as `2e096d6`;
E14 field-validated (“1 à 9 ok”, 12.49 GB database) and committed locally as `516180e` <!-- lang:fr -->
on 2026-09-07; not pushed (D0). E15 next.**

## 1. Findings

Read-only reconnaissance on this workstation, 2026-09-07, three agents then
hand-checked at HEAD. Every figure below is a count or a line read today; the
historical performance figures are quoted from the register with their date
and are **not** current measurements.

### E13 — architecture (A01–A09, D-9/D-25/D-34/D-46/D-47/D-48/D-49)

- **A01, business rules in the shell.** `commands.rs` holds 120
  `#[tauri::command]` functions over 7,374 lines (3,033 lines of command
  bodies). Lot 3 moved the recipient rule into `mail_core::reply_to` /
  `reply_all_split`; Lot 4 moved error categories into the core. What still
  decides in the shell: own-address detection and the Reply-To/From fallback
  order (`commands.rs:3427-3483`, `:3585-3643`), the empty-recipient
  self-fallback (`:3624-3630`), and in the UI the reply/forward signature
  scope rule (`Compose.svelte:348`). `reply_invitation` (`:1861-1941`) already
  delegates to `mail_core::compose` and `participation_de_stable`; it is
  orchestration, not a rule.
- **A02, one global lock.** `state.commands: Arc<Mutex<()>>` (`main.rs:157`)
  is taken by `off_pump` (`commands.rs:5896-5909`) around every blocking
  command body: SQLite read-decide-write pairs, file IO and CPU-bound
  sanitizing of bodies up to tens of MB (`:3558-3560`) all serialize behind
  it, while the per-account poll locks (`poll.rs:431-433`) are separate. **No
  p95 gesture-latency measurement exists** anywhere (no script, no bench, no
  figure newer than the register's historical ones).
- **A03, the blocking guard is textual.** `e2e/main-thread-guard.mjs:55-92`
  keeps a hand-maintained allowlist of pure commands and a marker list
  (`Store::`, `db_path(`, `keyring`, …). The path the audit cites,
  `add_oauth_account` (`commands.rs:407-450`), already goes through
  `store_off_pump` after its await. Nothing structural prevents a new helper
  under an unlisted name from reaching the Store off the guarded path.
- **A04, identity.** No `MessageKey`/`DraftKey` type exists. The core has
  `MailboxIdentity` (`store.rs:651-656`, with `uid_validity`) and the shell a
  parallel `MessageVersion` (`commands.rs:61-79`) converted per call; the
  loose tuple still crosses the IPC in **65** `account_id`, **32** `mailbox`
  and **37** `uid` parameters. D-59 (attachment rank tied to the adapter's
  inline filters) is untouched in code.
- **A05, error strings.** Resolved in substance by Lot 4 (`fault.rs`,
  `Error::Connection`); the remaining `starts_with`/`contains` hits are in
  test modules. A count of commands still returning a bare `String` (losing
  `code`/`retryable`) is the only residue.
- **A07 / D-9, adoption.** `Store::pending_adoption` (`migrations.rs:784-819`)
  is a proper read-only probe, but `MigrationShared` (`main.rs:45-49`) has no
  `adopted` flag, `db_path()` caches the path in a `OnceLock` never
  invalidated (`commands.rs:5969-5987`), and 30+ sites call
  `Store::open(&db_path(&app)?)` directly — only UI ordering keeps them after
  the migration screen. The two existing tests are probe-level.
- **A08, sizes.** `commands.rs` 7,374 (audit baseline 6,206), `store.rs` 3,261
  (2,956), `App.svelte` 2,217, `List.svelte` 1,843: the two big files grew by
  1,168 and 305 lines across Lots 1–4. Eleven shell modules (3,904 lines) and
  eight `store/` submodules (9,331 lines incl. 6,248 of tests) already exist.
- **A09 and the register.** D-25: `mail_smtp::draft_bytes` still takes ten
  positional parameters, six consecutive `&str` (`mail-smtp/src/lib.rs:337-348`).
  D-47 core half: `toggle_pin`/`pin_state`/`thread_pin_state`
  (`store.rs:2809-2863`) and `toggle_set_aside`/… (`:2869-2921`) differ by the
  table name only; the three-span fan is written twice
  (`PaperTrail.svelte:160`, `SetAsidePile.svelte:94`). D-48: only
  `drafts_revision` exists, no generic view revision. C08: `PRAGMA
  foreign_keys = ON` is executed twice per connection (`store.rs:52`,
  `migrations.rs:925`). D-34 (two per-account pref loaders) and D-46
  (Screener row anatomy) have **not** met their own reopening conditions.
  D-49's poisoned-lock item is done (`recovered()`, `commands.rs:5872`).

### E14 — offline, recovery, retention (G01, G02, G04)

- **What "offline" means today.** Envelopes are always local; a body is local
  if opened once or inside the per-account import horizon
  (`backfill.rs:30-71`); attachment bytes are **never** cached (ADR 0007,
  `store.rs:161-172`) and are refetched on every open; nothing is ever
  evicted. The UI has one generic failure, "This message could not be
  loaded." (`Thread.svelte:588-592`, `catalog.en.js:332`), for both "no
  network" and "the server no longer holds it", plus the global Offline disc
  (A116). FTS indexes bodies only when they are local (`search.rs:188-217`):
  a message outside the horizon is searchable by subject/sender only, and
  nothing says so. No "keep offline" gesture exists.
- **Backup/restore.** None of the 120 commands exports, imports or restores;
  Settings has eight sections, none for storage. The database is one WAL file
  (`migrations.rs:890-903`, ADR 0011); secrets live in the OS keyring
  (`mail-auth`), never in SQLite. Local-only state: drafts and their bytes,
  the outbox (`queued`/`sending`/`interrupted`/`rejected`, `outbox.rs`),
  prefs (`prefs.rs:11-34`), sender routing, Screener and cleaning sessions,
  learned contacts with provenance, image consent. **A restored database with
  queued rows would be delivered by the next automatic cycle**
  (`poll.rs:905`, `:932` call `flush_outbox` unconditionally): G02's
  acceptance criterion fails today by construction. Schema versioning is
  forward-only (`add_missing_columns` at every open, `user_version` for
  threading only).
- **G04 residue.** Lot 3 delivered contact provenance (ADR 0040) and
  per-message image revocation. Surviving a single-account removal by design:
  sender routing, per-sender image trust, contacts of unknown provenance, the
  Screener's history; ADR 0040 says in so many words that the erase-all
  facility belongs to Lot 5. Deletion goes to the server's Trash, never
  permanent (A98); there is no undo control and no "empty trash" gesture.

### E15 — measured scale, platforms, invitations, language (G05–G07)

- **Budgets and tools.** STANDARD §3 has eleven budgets; the one currently
  exceeded is RAM in the Feed: **249–251.5 MB private working set against
  200 MB** (D-53, field 2026-09-02; bench `e2e/tests/bench-ram-feed.spec.js`,
  the "optional benchmark" every gate skips). The Feed mounts one sandboxed
  iframe per card inside a window of ±5 rows (`Feed.svelte:128-158,347`); the
  memory of unmounted documents is retained by the GPU process. The list
  keeps every served page in plain `Map`s cleared only on a generation change
  (`List.svelte:222,551,607-609`, D-2). `sync_progress` recounts every
  mailbox every 5 s under the global lock (`store.rs:972-984`; D-37: 152 ms
  cold / 8.6 ms warm, 2026-08-26). Deep pages pay `LIMIT/OFFSET`
  (`nav.rs:964-1004`; D-26: 247 ms at offset 80,000, 2026-08-20, accepted).
  `e2e/measure-v2.mjs` still measures "60 switches across the 7 themes" while
  four ship (D-11) and an "opening" that predates the thread cascade (D-12,
  D-14): its series are not comparable to anything current. No criterion
  bench exists; `telemetry.rs` is crash telemetry, not timing.
- **Platforms.** CI proves Windows x64 (tests), macOS x64 and arm64 (clippy
  + tests), and no e2e anywhere (`ci.yml:52-54`; the 205-spec suite runs in
  the pre-push hook only). **Windows arm64 has no CI job at all**: it is
  built at release time only. macOS has no vault-across-update proof in the
  script or the doc (D-60 names the Keychain re-prompt as "to watch at the
  second mac release"). e2e on WKWebView is impossible with this harness
  (CDP), D-61.
- **Invitations.** `mail-ical` handles REQUEST/CANCEL/REPLY, SEQUENCE and
  RECURRENCE-ID, flags an RRULE without expanding it; Lot 3 delivered the
  version/occurrence guard (B16). D-29 stands: a calendar-only message has an
  empty indexed body, and `forward_context` (`commands.rs:3677-3706`) builds
  the quote from the HTML only — the ICS does not follow, silently.
- **Language.** D-56's stated scope is `human_size` (`o`/`Ko`/`Mo`,
  `attachment.rs:44-46`) and the native dialogs of `main.rs:245-276`. **New
  finding**: every reply and forward inserts a French attribution and header
  (the quote's "a écrit :" line and the forward's "Message transféré" <!-- lang:fr -->
  header, `compose.rs:229-249`) whatever the UI language — visible to every recipient
  of an English user. D-57: three French PNGs imported unconditionally
  (`Onboarding.svelte:28-30`). D-35 was decided by the Chief Engineer (ship the reduced
  masters). D-54: 3 local retries in 6 gates, CI never red.

## 2. Scope and acceptance

The lot is delivered as **three sub-lots**, each with its own review, gate,
field STOP 2 and commit (D1): the parent plan's gates require "use cases
without Tauri", recovery fixtures and long-session benchmarks, which do not
share a field checklist.

| Step | Work and proof | Coverage | State |
|---|---|---|---|
| E13a | **Adopted database handle**: `Store::open` reachable from the shell only through one `adopted()` helper that refuses before the probe/migration has adopted (`MigrationShared.adopted`), invalidates the path cache and checks file identity (inode/size/`user_version`) on reuse; probe stays read-only. Tests: an early command fails loudly; a file replaced at the same path is re-adopted, never skipped. | A07, A03, D-9 | Implemented |
| E13b | **Structural blocking boundary**: the Store handle is only constructible inside `off_pump`/`store_off_pump` (a private token type), so an async helper cannot touch SQLite off the blocking path; the text guard keeps its markers as a second net. Test: a deliberately wrong helper fails to compile (trybuild or a doc-test `compile_fail`). | A03 | Implemented |
| E13c | **Use cases into the core**: reply sender selection (own-address, Reply-To order, self-fallback) and the reply/forward signature-scope rule as pure `mail_core` functions with tables of cases; the shell converts DTOs. `MessageRef {account_id, mailbox, uid, uid_validity}` as the one identity DTO on the commands that take the tuple today, validated at the boundary (wrong account or generation → typed refusal). | A01, A04, A05 residue | Implemented (wire DTO refused, see § 5) |
| E13d | **Critical sections, measured**: spike first (§3.1): p95 latency of three gestures during a body backfill and a 25 MB file attach, before/after moving file IO and sanitizing out of the lock. The move is implemented only if the spike shows a gain; otherwise the figure is written down and A02 closes on it. | A02 | Spike measured, B implemented |
| E13e | **Duplicates still present**: one `Mark {Pin, SetAside}` enum behind `toggle_pin`/`toggle_set_aside`; one fan component for the two piles; a named `DraftContent` DTO for `draft_bytes`; one `PRAGMA foreign_keys` per connection; a generic `views_revision` bumped by every core mutation and polled with the sync activity (D-48). Domain moves in `commands.rs` limited to the files this lot touches (compose/reply, accounts/adoption), pure moves protected by the existing suites. | A08, A09, D-25, D-47, D-48, C08 | Implemented (no domain move yet) |
| E14a | **Availability made visible**: per-message body and attachment locality exposed in the thread DTO; distinct states "not downloaded, offline" (retryable) and "no longer on the server"; a search-coverage note when the query's scope has bodies outside the horizon. No new retention. | G01 | Planned, early visual STOP |
| E14b | **Backup and restore**: `VACUUM INTO` snapshot of the database (WAL-consistent, secrets excluded by construction), restore by replacement at the adopted path with the E13a identity check, and **outbox quarantine on restore** — every `queued`/`interrupted` row becomes `held` until the user releases it. Fixture: a synthetic install with drafts, files and two queued sends restores and delivers nothing. Export formats beyond the snapshot are refused (§3.4). | G02 | Awaiting D3 |
| E14c | **Forget derived data**: at single-account removal, a second choice "also forget what Wind learned from this account" removing exclusively-owned contacts, sender rules and image trust derived from it, keeping anything shared; a synthetic removal leaves no exclusively-owned remnant. | G04 | Awaiting D4, early visual STOP |
| E15a | **Rebaseline**: `measure-v2.mjs` recalibrated (four themes, thread opening as shipped, A44 geometry, cold/warm p50/p95, process scope stated); a dated baseline table written into STANDARD §3 replacing the historical figures; the RAM bench promoted from "optional" to a gate step with its own budget line. | G05, D-11, D-12, D-14, D-1 | Planned |
| E15b | **Feed memory, set-based** (§3.2): three measured alternatives on the RAM bench; the winner implemented only if it brings the Feed under 200 MB or the Chief Engineer dates an exception. Early measured STOP after the first increment. | D-53, D-13 | Spike planned |
| E15c | **Bounded growth**: LRU on distant list pages preserving selection and scroll (D-2); `sync_progress` from counters maintained at write time, with a drift check against the recount at startup (D-37). Each with a before/after figure. | D-2, D-37, D-21 | Planned |
| E15d | **Invitations and language**: calendar title/location indexed at `save_body_full`; the ICS attached on forward of a calendar-only message; attribution/forward header, `human_size` units and the two native dialogs rendered in the UI language (the core returns structured values, the shell/UI formats); language-specific onboarding captures. | G07, D-29, D-56, D-57 | Planned |
| E15e | **Platform proof**: a `windows-arm64` CI job (`cargo clippy`/`check` on `aarch64-pc-windows-msvc`); a dated support matrix (OS / WebView / arch / install / open / vault / update, with the evidence link per cell) in MACOS-BUILD and STANDARD §2.10; the vault-across-update line becomes a field item of the next mac release. | G06, D-60, D-61 | Planned |
| E15f | **D-54**: `multi-select.spec.js` replayed 20× with `--repeat-each`, the first failure captured with per-test counters; fixed if a race is proven, otherwise the rate is recorded. | D-54 | Planned |

Each row needs a maintained RED before its GREEN, whole-file e2e verdicts
when the UI is touched, the System journal amended in the same commit, and
the sub-lot's full gate. The measured rows carry their protocol and figures
in § 5.

Explicit refusals (§2.6), asked as D5:

- **No offline attachment cache** (ADR 0007 stands): a "keep offline"
  retention with bytes on disk is a product feature with a budget of its
  own, not an audit remedy; E14a makes the limit visible instead of hiding it.
- **No EML/mbox export, no import from another client**: the server mirror
  is the portable copy of mail; the snapshot covers what only Wind holds.
- **No undo of deletion and no "empty trash"**: deletion already goes to the
  server's Trash (A98) and is reversible there; a local undo would duplicate
  a reversible action.
- **No cursor pagination** (D-26 stays accepted on its 2026-08-20 figures,
  `VOL_MAX = 1`): re-measured in E15a, reopened only if the figure moved.
- **No e2e on macOS, no Windows arm64 runner tests** (D-61 stands; a
  clippy/check job is the proof hosted runners can give).
- **No redraw of the 16 px icon tier** (D-35, Chief Engineer decision of 2026-08-24).
- **D-34 and D-46 stay deferred**: their own reopening conditions have not
  fired (still two pref loaders; no row-template retouch).
- **D-59 structural MIME-part identity**: only if the E13c spike shows the
  part path can be captured at fetch time without a data migration;
  otherwise it stays open with the reason written (asked as D2).
- **No SQLite replacement, no repository framework, no line cap** (A08).

## 3. Design and measured options

### 3.1 A02 — the global lock (spike, one worktree)

Protocol: release build, the gate-3 fixture (3 accounts, 200,000 messages),
`freeze-probe.py` running; three gestures replayed 30× each (open a thread,
next page, toggle pin) **while** (a) a body backfill batch runs and (b) a
25 MB attach is in progress; p50/p95 per gesture, before and after:

| Option | What changes | Risk |
|---|---|---|
| A | Nothing (baseline) | — |
| B | File IO and sanitizing leave the critical section; the lock covers only the SQLite read-decide-write pair | The pair must be re-validated after the unlocked step (the same "look again" rule as A41) |
| C | B + a second connection for read-only commands | Two connections on one WAL: the §9 lesson on periodic readers applies; only if B leaves a gesture above 100 ms |

The alternative must beat A *clearly* (a p95 gain visible to the hand, not a
few ms) to be implemented.

### 3.2 D-53 — Feed memory (spike, three worktrees)

Bench: `bench-ram-feed.spec.js` as it stands (200 letters, 100 KB bodies),
plus the field protocol of D-53 (ten pages, return to the list, 25 s idle),
private working set per process from `measure-ram.ps1`.

| Option | What | Keeps S1 (iframe isolation)? |
|---|---|---|
| A | Window ±5 as shipped (baseline) | yes |
| B | Window ±1 and `srcdoc` reset to `about:blank` before unmount, so the compositor surface is released | yes |
| C | One pooled iframe per visible slot (3 to 5), documents swapped by `srcdoc`, cards outside the pool hold a measured placeholder | yes |
| D | Bodies rendered in the page DOM inside a shadow root (no iframe) | **no** — refused before measurement (S1, A37) |

The winner is the one under 200 MB on the ten-page protocol with the smallest
scroll cost (`measure-scroll.mjs`); if none passes, the figures go to the Chief Engineer
with a dated exception request (G05).

### 3.3 E13a/E13b — the adopted handle

`Store::open` becomes `pub(crate)`; the shell gets `Db::adopted(&app)`
returning a handle only after `MigrationShared.adopted` is set by the
migration pass (or by the probe when nothing is pending). The handle carries
the file identity read at adoption and re-checks it on every open (cheap:
one `stat`); a mismatch (file replaced under the app, relocation) re-runs the
probe instead of opening blindly. The blocking token is a zero-size type
constructible only inside `off_pump`; `Db::adopted` requires it. Both are
compile-time nets, proven by a `compile_fail` doc-test each, and the text
guard stays.

### 3.4 E14b — the snapshot

`VACUUM INTO 'path'` from the adopted connection: consistent under WAL, no
secrets (the keyring is outside), one file the user names in a native save
dialog; restore = close every session, replace the file at the adopted path,
re-adopt through E13a, then **hold** the outbox: rows in `queued`/
`interrupted` get `held` and the Outbox view shows them with a single
"Send now" per row. `flush_outbox` skips `held` rows. A restore into a newer
Wind migrates forward as today; a restore into an older Wind is refused by
`user_version`/schema check with a message naming both versions.

### 3.5 E15d — language at the boundary

The core stops composing text: `attribution()` returns `{date, sender}` and
`forward_header()` a struct; the UI catalogs carry the sentences in both
languages; `human_size` returns `(value, unit_key)`. The language ratchet's
baseline drops accordingly (the count only falls).

## 4. Chief Engineer decisions

Asked one by one at STOP 1 on 2026-09-07; answers verbatim (the dialog was
in French):

- **D0, 2026-09-07:** “On fera le push groupé après tous les lots et après une review du diff avec /code-review ultra.” <!-- lang:fr -->
  Neither A nor B: Lot 5 proceeds on the local commits; the grouped push of
  Lots 3, 4 and 5 comes after the whole program's diff has been reviewed
  with `/code-review ultra`. No push before that.
- **D1, 2026-09-07:** “Oui, trois sous-lots (recommandé)”. <!-- lang:fr -->
- **D2, 2026-09-07:** “Oui, conditionnel (recommandé)”. <!-- lang:fr -->
- **D3, 2026-09-07:** “A : snapshot + restauration avec mise en attente de l'outbox (recommandé)”. <!-- lang:fr -->
- **D4, 2026-09-07:** “Oui (recommandé)”. <!-- lang:fr -->
- **D5, 2026-09-07:** first “Non, à amender”, then “Quelles seraient tes recommandations pour chacun des éléments ?”; after the per-item recommendations (all: keep the refusal; D-26 re-measured in E15a): “Aucun : liste approuvée telle quelle”. <!-- lang:fr -->
- **D7, 2026-09-07:** “Oui, langue de l'UI (recommandé)”. <!-- lang:fr -->
- **D6:** not asked yet (needs the § 3.2 figures).

The questions as asked:

- **D0 — order with Lots 3 and 4.** STATE plans: grouped push → green CI →
  `/close` of Lots 3–4 → Lot 5. Options: (A) push Lots 3–4 now in the
  background, close them on green CI, then start Lot 5's code; (B) start
  Lot 5 on the local commits and push everything grouped later.
  *Recommended: A* — the new CI jobs of Lot 4 prove themselves alone, and a
  Lot 5 red would not be confused with them.
- **D1 — three sub-lots.** Deliver E13, E14, E15 as three commits with three
  field STOPs, in that order (E14 restore depends on E13a's adopted handle).
  *Recommended: yes.*
- **D2 — D-59.** Include structural MIME-part identity only if the fetch path
  can capture the part path without migrating stored rows (spike inside
  E13c); otherwise leave it open with the reason. *Recommended: yes, conditional.*
- **D3 — backup/restore shape.** (A) snapshot + restore with outbox hold, no
  other format; (B) also an EML export of a message; (C) defer backup
  entirely. *Recommended: A.*
- **D4 — forget derived data.** Add the second choice at account removal
  (E14c) with an early visual STOP on the dialog. *Recommended: yes.*
- **D5 — the refusals of § 2.** Approve the list as written. *Recommended: yes.*
- **D6 — Feed memory budget.** If no option of § 3.2 passes 200 MB, accept a
  dated exception at the measured figure or keep the line red. *Asked only
  once the figures exist.*
- **D7 — D-56 wording.** The reply attribution and forward header follow the
  UI language (the recipient sees the writer's language). *Recommended: yes.*

## 5. Verification and field

Record RED/GREEN figures here as they occur. Spikes § 3.1 and § 3.2 run
after STOP 1 (they cost a build each).

### Spike A02 — 2026-09-07

Isolated worktree (at `a58c1cf`: created before Lots 3–4 landed on the
main tree; the lock model is identical at `2ade544`), release build,
`banc200k.db` copy, gestures driven through the UI's own transport, 30
reps each; full report and raw figures in
[`spikes/global-lock/REPORT.md`](../spikes/global-lock/REPORT.md).

| Load | Gesture | A p50 / p95 (ms) | B p50 / p95 (ms) |
|---|---|---:|---:|
| idle | open / page / pin | 7.5 / 15.3 · 5.6 / 8.3 · 17.5 / 25.6 | 9.7 / 25.4 · 6.4 / 9.6 · 16.2 / 24.5 |
| 10 MB sanitize | open | 465.5 / 691.6 | **12.0 / 68.2** |
| 10 MB sanitize | page | 149.1 / 673.9 | **8.0 / 26.6** |
| 10 MB sanitize | pin | 168.8 / 654.3 | **18.6 / 25.0** |
| 25 MiB attach | open | 858.7 / 1473.6 | 845.6 / 1140.7 |
| 25 MiB attach | page | 85.2 / 828.5 | 9.8 / 826.9 |
| both | open | 1414.0 / 1922.4 | 421.6 / 881.3 |

The backfill could not be driven offline and, decisively, does not hold
the commands' lock (it holds `bodies_backfill`): the load is the sanitize
path that IS under the lock. Verdict on the figures: B beats A clearly
where the lock is the cause (a sanitize in flight); it buys nothing where
SQLite's writer is (the 25 MiB blob insert) — option C (a second reader
connection) is not needed for the gestures measured, and the write-bound
wait is a different question, left open in the register. **E13d
implemented on B**, sanitize sites and the attach file reads.

### Implementation increments — 2026-09-07

- **E13a.** `apps/desktop/src/adoption.rs`: `Adoption` records the
  database file's identity (`same_file` handle hashed, no handle kept
  open) at `migration_check` when nothing is pending and at
  `migration_run` after the pass; `adopted_db` refuses `NotAdopted` before
  and `Replaced` when the file at the path is not the adopted one (record
  cleared, the probe must run again); a first install is adopted `Fresh`
  and bound to the file the first open creates. Six unit tests (early
  command refused; the file grows in place; replaced at the same path →
  refused once then needs adoption; first install; deleted file;
  forget). The module and its tests were written together — no RED shown
  for them; the RED of the lot is E13b's.
- **E13b.** `Blocking` token: `off_pump`'s closure receives it (derefs to
  the `AppHandle`, so the ~125 closures keep their shape); `adopted_db`
  takes `&Blocking`; `db_path` is gone, `probe_path` remains for the three
  read-only probes. **RED measured: 17 compile errors**, 15 of them
  indirect helpers reaching the database through a bare `&AppHandle`
  (`auth_for`, `queue_removal`, `connected_jobs`, `light_pass_account`,
  `job_for_email`, and eight async commands computing the path in their
  glue) — precisely what the text guard could not see (A03). All now
  carry the token; the dedicated threads (scheduler, watcher, the two
  removal/repair workers) build theirs with `on_dedicated_thread`, the one
  name the text guard adds to its markers. Shell 66/66, startup and
  screen-02 specs 62/62.
- **E13c.** Core: `is_own_message` (one trimmed, case-insensitive rule for
  reply and reply-all — the shell had two), `reply_all_sender` (Reply-To
  precedence on received mail only), `reply_all_recipients` (the whole
  decision with the self-only fallback and the Cc-only case), `ComposeMode`
  and `signature_applies`. RED shown (five missing functions), then 7
  tests GREEN. The shell's `reply_context`/`reply_all_context` delegate;
  `signature_get` takes the composer's `mode` and returns `applicable`,
  the composer inserts what the core decided (two decision sites in
  `Compose.svelte` gone). Four compose specs 75/75. The `MessageRef` DTO
  at the wire is NOT done: the IPC shape (65/32/37 loose parameters)
  would change on every call site for no user-visible gain; the core's
  `MailboxIdentity` + `verify` stays the boundary. Recorded as a limit.
- **E13e.** `Mark {Pin, SetAside}` behind the four pin/set-aside
  functions (one implementation, the table name the only variable);
  `Stacked.svelte` for the Feed's and the Paper trail's fan; `DraftMessage`
  named DTO for `mail_smtp::draft_bytes` (six test callers converted);
  `PRAGMA foreign_keys` once per connection (removed from `SCHEMA`, kept
  in `init_with` ahead of the fast door — the belt test unchanged);
  `views_revision` in `prefs`, bumped by routing verdicts and removals,
  pins, set-asides and cleanup verdicts, read by `ui_state`, the UI
  reloading its views when it moves (RED shown: missing method; then
  GREEN). Core 576, smtp 47; UI build and lint green.
- **E13d.** `unlocked(...)`: a bare blocking worker with no `Blocking`
  token — no database reachable from it. The sanitize left the lock in
  `message_body` (both paths), `echo_body`, `feed_cards` (a page of
  sanitizes), `citation_reply`, `forward_context` — pure reads, no
  second look (the next gesture verifies again); the attach gesture
  reads its files unlocked against the room read under a first lock, the
  core re-checking the budget at every insert. **Net**: the text guard
  gains a rule — inside every `off_pump` family call, `mail_render::
  sanitize(_with)`, `attachment_file::read` and `std::fs::read` are a
  red, transitively through the file's helpers (`body_view` hid the first
  one from a flat scan). RED shown: 7 findings; GREEN after. The
  composer's own boundary (`sanitize_composition`, `sanitize_for_composer`)
  stays under the lock on purpose (the user's typed content). Not done:
  `save_attachment`/`install_downloaded` (`fs::write` paths, off the
  gesture path) — recorded.

### Fresh-eyes review — 2026-09-07

Eight finder angles over the working-tree diff (line-by-line, removed
behavior, cross-file tracer, reuse, simplification, efficiency, altitude,
conventions), about twenty-five candidates, each read against the code.
Confirmed and corrected before the gate:

1. **`ComposeMode` did not know `reply_all`** (three angles): the wire's
   fourth mode parsed to `None`, so every "Reply all" lost its signature
   whatever the account's scope — a regression of E13c against the JS rule
   it replaced. `reply_all` now signs like a reply; the test names it.
2. **A replaced file left the app refusing everything until a restart**:
   `adopted_db` now probes again on `Replaced` — nothing pending → adopted
   anew, the command goes on; a pass pending → refused with the count and
   the migration screen is the way back. `migration_check`'s startup failure
   retries once after 500 ms.
3. **One unreadable file dropped the files read before it**: the read pass
   keeps the first hard failure for after the files that read fine are
   stored — the gesture fails as before, the good files stay attached.
4. **Two commits per gesture**: the views' revision bump moved inside the
   write's own transaction (pins and set-asides now run in one; routing,
   removal and cleanup verdicts bump before their commit).
5. **`exists()` then open** in the identity read: a file deleted in between
   read as an I/O failure; one syscall now, `NotFound` is `Fresh`.
6. **The second lock take of `message_body`** (the spike's re-verify): a
   pure read gets no second look, same rule as the Feed — the hottest
   command takes the lock once again.
7. Cleanups: one `render_document` behind the reading pane, the echo and
   the Feed (it was written three times); `reply_all_sender` private; the
   `repliesSig` alias gone; the guard's one parenthesis scanner and its
   heavy-under-lock rule over the whole shell, not one file; `views_revision`
   read through `text_pref`; `forget()` (unused until E14b) removed.

Recorded, not changed: the Feed's dropped second verify (a stale card was
already possible between a served page and a click minutes later; the
gesture commands take no version — the A04 wire DTO, refused in this lot,
is where that closes); `on_dedicated_thread` inside the repair and removal
workers (bare `spawn_blocking` that take the lock by hand — the doc now says
so); the identity check's one `CreateFile` per command beside a
`Store::open` that costs milliseconds; two store opens per attach gesture
(the trade the spike measured); the four hand-placed `note_view_change`
calls (an `update_hook` would tax every synced row). After the corrections:
core 576, smtp 47, shell 65, clippy clean, guard green, UI build and lint
green.

## Full gate and STOP 2 handoff (sub-lot E13) — 2026-09-07

Two gates: the first was red at step 7 (the plan's own quote of the French
attribution strings and four two-letter Chief Engineer abbreviations read as French — reworded, one
`lang:fr` marker on the deliberate quote); the second ran unchanged,
`scripts/gate.ps1`, **exit 0 in 322 s**.

| Step | Verdict and figures |
|---|---|
| 1. Rust format | GREEN |
| 2. UI build and lint | GREEN; no build or lint warning |
| 3. Contrasts | GREEN; four themes, 440 pairs |
| 4. System coherence | GREEN; four themes, 68 token values |
| 5. Main-thread guard | GREEN; 126 commands, heavy-under-lock rule over the whole shell |
| 6. Script syntax | GREEN |
| 7. Language ratchet | GREEN; 1,967 markers against a 1,974 baseline, no rise |
| 8. IPC contract | GREEN; 125 commands defined, registered, 112 called by name |
| 9. Markdown links | GREEN; 80 files, 432 links |
| 10. Clippy | GREEN; zero warnings |
| 11. Rust tests | GREEN; **911 passed** (core 576, imap 116, shell 65, smtp 47, auth 42, ical 35, others), 3 ignored |
| 12. Doc tests | GREEN |
| 13. e2e | GREEN; **43 Node, 265 UI passed**, 1 optional benchmark skipped, **0 flaky** |

Earlier in the day, `organized-mode.spec.js` "Move to… routes the whole
sender" timed out on its first attempt in two targeted runs (first-card
hover, 180 s) and passed on retry each time — the same scenario Lot 3
recorded as a loaded-suite timeout; it passed first try in the full gate.
No product cause was found; recorded, not claimed fixed.

### STOP 2 — field checklist (E13)

Nothing new to see; what must not have changed, and three things that must
have. Real accounts, the installed release build from the sources.

1. **Startup.** Wind opens on the real database as before (no migration
   screen unless one is due). The trace carries no "database not adopted"
   and no "replaced under the application" line.
2. **A heavy letter, then a gesture.** Open the heaviest newsletter you
   have (the reading pane takes its time); while it renders, press ↓ to the
   next message or turn a page. The second gesture answers at once — it no
   longer waits behind the sanitize (A02).
3. **Signature scope.** Settings › Signature with "also in replies and
   forwards" ON: Reply, **Reply all** and Forward all carry the signature;
   New too. Scope OFF: New only. (The review caught Reply all — check it.)
4. **Reply all on a list message** (a Reply-To that differs from the
   sender): the To is the list address; on one of your own sent messages,
   Reply all writes back to the same group, never to yourself alone.
5. **Attach.** Several small files in one gesture: all attached. A file
   above the room left: refused with the remaining room named, the others
   stay. (The read of the files no longer sits behind the lock.)
6. **The list follows the Screener (D-48).** From the Screener, route a
   sender to the Feed; go back to the Inbox without clicking anything else:
   within about five seconds its threads leave the Inbox on their own.
7. **Pin and set aside**: unchanged behavior; the pinned section moves at
   once (same as before), the Feed's and the Paper trail's sender groups
   show the same stacked glyph as before.
8. **Feed page.** Ten pages of letters scroll as before (the sanitize of a
   page runs off the lock; the memory figure is E15's, not this STOP's).

### STOP 2 field findings — 2026-09-07

Verdict: **1, 3–7 OK**; item 8 asked what "ten pages of Feed" meant (the
D-53 memory protocol's wording — for this STOP it only meant "scroll the
Feed a while"; answered, nothing to test); **item 2 KO**: a few seconds
after launch, "Always show images" on a newsletter blanked the reading
pane until the initial synchronization ended — more than ten seconds. The
trace of that launch (`poll account 1: INBOX 2.3s · inventory 0.9s · 52
folders 21.6s · threads 6.2s · drafts 5.0s`) carried no line for the
wait.

Root, read in the code: the grant is a write (`allow_images_sender`, one
SQLite transaction) that waits SQLite's writer out behind the sync's
batches (`busy_timeout` 30 s) while HOLDING the commands' lock; when it
returns, `refreshThreadImages` deletes the body and re-reads it, and that
pure read — like the next message, the list, the probe — queues behind any
other write command waiting under the lock. **Reproduced**: a new
maintained net, `e2e/tests/reads-under-write.spec.js`, holds a writer from
outside (Python `BEGIN IMMEDIATE`, 12 s) and opens the next unread message
— its own mark-as-read is the write that queues (the fixture has no remote
images, so no grant is needed to show the mechanism) — RED on the morning's
code (the frame stayed empty for the 3 s allowed), GREEN 2/2 after. Fixed
the same day:

1. **Pure reads take no commands' lock**: `read_off_pump` /
   `read_store_off_pump` (own connection, `&Store`, no action-effects
   recovery); the twenty read commands converted (`message_body`,
   `thread_messages`, `list_category`, `category_total`, `echo_body`,
   `feed_cards`, `ui_state`, `sync_progress`, `list_drafts`, `markers_get`,
   `names_get`, `search_messages`, `backfill_status`, `screener_waiting`,
   `paper_trail_groups`, `list_folders`, `cleanup_groups`, `images_senders`,
   `signature_get`, `outbox_status`). The text guard refuses a writing name
   under a read helper. This is the spike's option C without the second
   connection type: reads simply do not queue.
2. **A body being re-read keeps its frame** until the new one lands
   (`loadMessage(…, refresh)`); the grant's wait is then invisible.
3. **Observable**: a command slower than a second traces its lock wait and
   its work under the enclosing function's name (`slow command
   allow_images_sender: lock wait 0 ms, work 11,842 ms`).

Not changed: the sync's write batches (500 envelopes per transaction, the
threads pass) still hold SQLite's writer for seconds; a WRITE gesture
during a batch still waits its turn (late beats dead, ADR 0011). That wait
is now visible in the trace and bounded by the batch — E15c's measured
work if the field asks for shorter batches. Second field pass requested on
item 2 alone.

Gate replayed after the fix, unchanged: **GREEN in 300 s** — 911 Rust,
43 Node, **266 UI** (the new net included), 1 optional benchmark skipped,
0 flaky.

### STOP 2 verdict — 2026-09-07

Second field pass on item 2 alone: **“2 OK”** (verbatim). Sub-lot E13 is
field-validated: items 1–7 OK, item 2 fixed the same day and re-validated.
Committed locally as `2e096d6`; not pushed (D0). <!-- lang:fr -->

Commands, ready to copy — the committed scripts first:

```powershell
powershell -ExecutionPolicy Bypass -File scriptsield.ps1
```

```powershell
powershell -ExecutionPolicy Bypass -File scripts
un-wind.ps1
```

(release build from the sources with the trace at `trace-terrain.log` at
the repository root; `-DebugBuild` for a faster build with inflated CPU
durations). Verdict as a numbered list, 1–8 OK/KO, with what you saw.
 Each sub-lot ends with one fresh
review, the full `scripts/gate.ps1`, then STOP 2 with the committed
`scripts/field.ps1` and `scripts/run-wind.ps1` commands and a numbered field
checklist. The E14 restore fixture and the E13 compile-fail nets are proven
by tests; the Feed figure and the gesture p95 are proven on this
workstation's fixtures and then in the field on the real database.

## Sub-lot E14 — 2026-09-07

Started after E13's commit. A disk full to the byte (target caches of a
hundred gigabytes, spike worktrees) stopped the line once; the Chief
Engineer cleaned 85 GB and the work resumed.

### Implementation increments

- **E14b core (TDD).** RED shown (`Held` state, `hold_pending_sends`,
  `release_held`, `snapshot_into`, `delete_account_forgetting`,
  `inspect_copy` absent), then GREEN: an outbox held after a restore
  delivers nothing until each send is released, a held send can be
  discarded; `VACUUM INTO` writes a complete copy that opens on its own
  while the live database keeps working, and refuses an existing target;
  the copy probe refuses a text file, a foreign SQLite file and a copy
  from a newer Wind. Core 581.
- **E14c core (TDD).** `delete_account_forgetting`: the sender rules and
  the image trust whose address no remaining account receives mail from
  go with the account; a shared rule stays; a contact of unknown
  provenance stays (ADR 0040). One test.
- **E14b shell.** `apps/desktop/src/restore.rs`: a copy is STAGED while
  Wind runs (validated by the core probe, placed as `wind.db.restore`) and
  APPLIED at the next start under the instance lock, before anything opens
  the database — the replaced file kept as `wind.db.replaced-<epoch>`, its
  WAL and shm removed with it, a marker asking the adoption to hold the
  outbox; `hold_if_marked` runs after both adoption paths. Three tests
  (a text file refused, nothing staged touches nothing, the swap and the
  hold). Commands: `backup_snapshot` (a read, no commands' lock — a large
  copy takes seconds), `restore_snapshot`, `restart_app` (pure, listed),
  `outbox_release`; `remove_account` gains `forget`; `OutboxStatus.held`.
- **E14a.** `Error::MissingOnServer` (code `gone`, not retryable) replaces
  the bare string; the reading pane says offline-never-downloaded (retry)
  or gone (no retry); the search field carries a coverage line while
  bodies are still on their way.
- **UI.** Settings › Your data (two rows, the removal card's form for the
  restore confirmation), the held-send notice with Send now / Discard, the
  forget checkbox. Catalogs en/fr. `chooseSource` in the transport with
  its e2e seam. The `storage` glyph un-reserved (System icon table).
- **Nets.** `storage.spec.js` (the copy written where asked and a SQLite
  file, a second copy never overwrites, the restore confirmation, the
  staged file next to the database, a foreign file refused) and the
  removal spec's second choice. 10/10 with the image-revocation spec.
  **Early visual STOP: “OK, déroule” on 2026-09-07** (three captures). <!-- lang:fr -->

### Fresh-eyes review (E14) — 2026-09-07

Three finder passes (line-by-line; removed behavior and cross-file; the
cleanup angles together), about fifteen candidates. Confirmed and
corrected before the gate:

1. **A failed swap kept its hold marker**: the marker is written first
   (a failure after the swap must never leave the copy's outbox free), and
   a swap that fails takes it back — nothing restored, the database's own
   sends stay its own; the database steps back in if the copy could not
   take its place (never a path that opens as a first install). Windows
   test with an exclusive handle on the file.
2. **Removal and forgetting in ONE transaction** (a crash between the two
   stranded the rules forever).
3. **A hand copy of a live database refused**: a `-wal` sidecar next to the
   picked file means transactions the file lacks; `quick_check` runs at
   staging, not after the swap.
4. **The forget checkbox outlived its card** (a cancelled tick reached the
   next account's card): reset with the card.
5. Efficiency: the copy probe read the whole file for a sixteen-byte
   header; the staging copy ran under the commands' lock; the orphan
   subquery recomputed `lower(trim())` per row instead of the indexed
   `sender_norm`. All three corrected.
6. Cleanup: the `validate` wrapper inlined. Kept: the `Applied` type, the
   per-module scratch helpers of the tests.

Stated limits: an e2e session cannot survive `restart_app`, so the swap
and the hold are proven by the shell's tests and by the field; the
attachment bytes are not in the copy (never cached, ADR 0007); a restore
into an older Wind is refused only when the threading version says so
(other schema drift migrates forward as any legacy database, D-55).

### Full gate and STOP 2 handoff (sub-lot E14) — 2026-09-07

Two gates: the first red at step 13 on one legitimate count (the settings
navigation spec expected eight groups, "Your data" makes nine — corrected,
the file replayed whole in isolation 61/61); the second unchanged,
`scripts/gate.ps1`, **exit 0 in 490 s** — 920 Rust (core 583, shell 69,
others), 43 Node, **267 UI**, 1 optional benchmark skipped, one flaky
scenario passed on retry (recorded below), zero failed. Contrasts 440
pairs, System coherence 68 values (the `storage` glyph in use), guard 130
commands, language ratchet no rise, IPC contract 129 commands. The flaky
scenario is `organized-mode` "Move to… routes the whole sender" (first-card
hover), the same as E13's day and Lot 3's — passed on retry.

### STOP 2 — field checklist (E14)

Real accounts, the release build from the sources. The restore path is
proven end to end HERE (the e2e cannot survive the restart).

`scripts\field.ps1` on the workstation, 2026-09-07 evening: `wind.db`
**12.49 GB**, 64 GB free after the day's cleanup. A copy is the whole
database, bodies included, compacted by little: **about 12 GB and one to
three minutes per copy** (button greyed meanwhile, reading still
possible — no commands' lock); a restore copies the file once more (a
minute) then walks it with `quick_check` (one to two minutes, no
progress bar — a stated limit). The sequence below keeps the disk under
40 GB: ONE copy, made after the queued send of item 3, restored once,
never a second restore.

1. **A copy.** Settings › Your data › Save a copy… → name a file on the
   Desktop. The toast names the path; the file is there (~12 GB, bodies
   included, no attachment bytes); Wind kept working meanwhile.
2. **A second copy under the same name** is refused (the toast says so);
   the first file is intact.
3. **A queued send.** Go offline (network off), send yourself a message:
   "queued". Then Save a copy… again (this copy carries the queued send).
   Go online, let it deliver.
4. **Restore that copy.** Settings › Your data › Restore a copy… → pick the
   second file → the confirmation card names it → Restart on this copy.
   Wind restarts. Expected: the mail is as it was at the copy; the notice
   slot says “…was waiting to be sent in the copy you restored” with
   **Send now / Discard** — press **Discard** (it was already delivered
   before the restore). Nothing goes out on its own. The trace carries
   `restore: copy in place, previous database kept as wind.db.replaced-…`
   and `restore: 1 send(s) held`.
5. **No second restore**: the restored copy is today's mail minus what
   arrived in between (the sync catches up). The replaced file sits next
   to `wind.db` in `%APPDATA%\dev.elements.wind`, named
   `wind.db.replaced-<epoch>` (12 GB); delete it and the Desktop copy by
   hand once satisfied (Wind never deletes them).
6. **A foreign file** (a .txt renamed .db): refused with the reason, no
   restart.
7. **Forget at removal** — only if you have a disposable account to
   remove: tick “Also forget what Wind learned from this account”, Remove;
   a Screener rule for a sender that only wrote to that account is gone
   from Settings › Screener; a rule shared with another account stays.
   Without a disposable account: open the card, see the checkbox, Cancel.
8. **Bodies.** Offline, open a message never downloaded: the frame says
   “You are offline and this message was never downloaded…” with Retry.
   (“No longer on the server” needs a message deleted from the webmail
   between two syncs — only if convenient.)
9. **Search coverage.** During a body backfill (a fresh account, or the
   first minutes after a restore), type a query: one muted line beside
   the field says how many messages are searched by subject and sender
   only; it goes away once the bodies are in.

Commands, ready to copy — the committed scripts first:

```powershell
powershell -ExecutionPolicy Bypass -File scriptsield.ps1
```

```powershell
powershell -ExecutionPolicy Bypass -File scripts
un-wind.ps1
```

Verdict as a numbered list, 1–9 OK/KO, with what you saw (and the two
`restore:` trace lines of item 4).

### STOP 2 verdict (E14) — 2026-09-07

**“1 à 9 ok”** (verbatim), on the 12.49 GB real database with the <!-- lang:fr -->
tightened sequence (one copy, one restore, the held send discarded).
Sub-lot E14 is field-validated. Committed locally as `516180e`; not pushed (D0). <!-- lang:fr -->


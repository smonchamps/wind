# Standard — Wind's working standard

> **This document is the project's permanent instruction**: method
> (§2), product (§3), architecture (§4), frozen decisions (§5),
> invariants (§6), environment (§7), lessons (§9). It is **amended by
> kaizen** — one finding, one amendment — and is not rewritten. **The
> current state** (shipped version, next job, field figures) lives in
> [STATE.md](STATE.md), the handover snapshot.
>
> Born from the split of HANDOVER.md on 2026-08-19
> (PLAN-DOCUMENTATION, CE decisions D1-D2). **The numbering §2-§10 is
> frozen**: any external reference (« §2.9 », « §7.1 ») stays true.
---

## 0. How to open the conversation

Since 2026-08-15, `CLAUDE.md` (root) loads the role and the pointer
to this document at every session: **nothing left to paste**. The
standardized workflows live in `.claude/skills/` (committed,
CE decision the same day): `/job` runs a bug or a feature
end to end with its two manual validations (plan, field),
`/field` handles a field finding the same day, `/gate` replays the
full gate, `/close` closes a job. The `spike` agent
(`.claude/agents/`) carries the set-based exploration in an isolated
worktree. The full user guide: [WORKFLOW.md](WORKFLOW.md).

If context is lost anyway, the old ritual still holds:

> Resume Wind's development. You are the Chief Engineer of the
> project and you apply the method described in `docs/STANDARD.md` §2 —
> it is a permanent instruction, it takes precedence over everything. Read
> this document in full first, then read `docs/STATE.md`.

Reading order, once:

1. **this document** — method, invariants, traps;
2. [`docs/STATE.md`](STATE.md) — where things stand, what to do first;
3. [`docs/PLAN.md`](PLAN.md) — the concept paper, product source of truth;
4. the ADRs in [`docs/adr/`](adr/) — **frozen decisions**, not to be
   reopened without a measurement to the contrary.

Do not read the code beforehand. It is large and heavily commented; the
comments explain *why*, and assume the context below.

---

## 1. Where things stand → [STATE.md](STATE.md)

The current state — shipped version, next job, field
figures, open trade-offs — lives in [STATE.md](STATE.md),
the handover snapshot, rewritten at every job: that is its
function.

---

## 2. The method — permanent instruction

Development follows Toyota's *shusa* (Chief Engineer) discipline.
**It takes precedence over everything else**, including the urge to move fast.

### 2.1 The user is the Chief Engineer, not a customer
He decides the product decisions and **validates every increment on his
real accounts**. You propose, you measure, you recommend; he arbitrates.
Never make a scope decision in his place.

### 2.2 Front-loading — hard points get settled BEFORE coding
Through a **throw-away, measured spike**, outside the production workspace. Done
for: the sync engine, the web bridge, HTML rendering, OAuth, the search engine.

### 2.3 Set-based — explore, then eliminate on figures
Several options are compared and decided **on measurements, not
opinions**. Tie-breaking rule: the alternative must beat the hypothesis
*clearly* to unseat it. Model to imitate: [ADR 0004](adr/0004-fts5-search-engine.md).

### 2.4 Jidoka — quality built into the process
- **TDD**: the test fails (RED) before the implementation (GREEN). When a
  RED can teach nothing (a trivial pure function), say so, don't
  fake it.
- **Gate mandatory before every commit** — and a `pre-push` hook
  replays it (§7.4). One clippy warning = red build.
- **Targeted inner loop**: during implementation, only the impacted
  spec(s) are run, whole file (never `-g` on an e2e),
  runs grouped by wave (grouped RED, grouped GREEN); the full gate is
  run ONCE before the commit — not at every increment (kaizen
  2026-08-23: up to 10+ gates per job measured, ~100 min).
- Zero `unwrap()`/`expect()` in production. Typed errors (`thiserror`)
  in the crates, `anyhow` in the apps.

### 2.5 Genchi genbutsu — go see in the field
**That is where the defects are found.** See §9. An increment not validated
on a real account is not delivered. Feedback is fixed **the same
day** — the WAL (ADR 0011) is the latest example of it: a defect on the first
field trial, fixed and committed within the day.

### 2.6 Explicit scope refusals
When a feature would be a phantom (invisible result, missing
brick), it gets **deferred, and why is written down**. Saying no is the
default behavior: every addition is paid for in speed and reliability.

### 2.7 Traceability
- A structuring decision = **a short ADR** in `docs/adr/`.
- End of phase = **a closing review** `docs/PHASEn.md`: delivered against
  the plan, budgets re-measured, lessons, deferrals owned, GO/NO-GO.

### 2.8 Language and commits

**Everything is in English** — code, identifiers, comments, commits,
documentation, the System. Amended on 2026-09-02 by
[PLAN-ENGLISH-SWITCH](PLAN-ENGLISH-SWITCH.md) (CE decision D2): until
then everything was in French, and the switch is carried by the
language ratchet (`e2e/language-gate.mjs`, gate step 7) — the French
markers of every file can only go down. What stays French, by decision,
is listed in [GLOSSARY.md](GLOSSARY.md) §1: the SQLite schema and the
persisted keys (debt D-54), the French UI catalogue (delivered word for
word), the archives, `BETA.fr.md`.

Commits: `type: description` (`feat`, `fix`, `refactor`, `docs`, `test`,
`chore`, `perf`, `ci`), in English; the body carries the figures and the
reasoning. **Never a `Co-Authored-By`.** The old "no accents" rule of
the commit messages is void with the language.

### 2.9 Version numbering

**Wind follows an `x.y.z` format, where `x` = MAJOR, `y` = MINOR, `z` =
PATCH.** Wind exposes no public API: the "contract" whose breakage counts
as MAJOR is redefined on the **only two things the user cannot
fix on their own** — the auto-update chain and their inbox's survival.

We go down the list, stopping at the first "yes":

1. **MAJOR** (`x`+1, then `y` and `z` → 0) — if **any** of these is true:
   - the version **cannot be reached by auto-update** from the previous one
     (manual reinstall: signing-key rotation, installer/format
     change — **it happened in 0.1.3**). Since the return
     of the x64 channel (PLAN-RETOURS-8, ADR 0023), there are **two
     auto-update chains** (arm64 and x64): the criterion is evaluated
     **per channel**, and a break on just ONE channel is enough to trigger
     MAJOR. Adding a channel breaks nothing (each workstation's updater
     only reads its own `{os}-{arch}` key) — removing or breaking a channel does;
   - it carries a **non-reversible data migration** (contrary
     to [ADR 0012](adr/0012-visible-interruptible-migration.md));
   - \+ the **single** `0.x → 1.0.0` transition at the "past initial
     development" milestone (leaving beta) — a shusa product decision.
2. **MINOR** (`y`+1, then `z` → 0) — if the release **adds at least one
   new capability** visible to the user.
3. **PATCH** (`z`+1) — if the release includes only fixes,
   adjustments to existing behavior, perf, internal streamlining, cleanups.

The release is published via `scripts/make-release.ps1 <version>`
([ADR 0013](adr/0013-nsis-installer-signed-update.md), bi-arch since
[ADR 0023](adr/0023-x64-channel-return.md): two `--target` builds,
native arm64 + local x64 cross-build, **all-or-nothing** — a failed
build blocks the whole release, never a channel left behind); the GitHub
tag stays the **bare version**.

⚠️ **User-facing notes FIRST, systematically**: write (and
commit) the `## [<version>]` entry of `CHANGELOG.md` **before**
launching the script — it flatly refuses without it ("CHANGELOG.md has
no '## [x.y.z]' entry…"), that's its first check. Missed **at least three
times** during a session (last: 0.2.1, 2026-08-20): the reflex is
part of release prep, not an afterthought.

### 2.10 Verifying a published release

Since 0.1.10 (2026-08-18), `scripts/make-release.ps1 <v>` does
**the entire** release (field-validated) — provided the
`## [<v>]` CHANGELOG entry already exists (§2.9, its first check):
bump of the sole
`version` line of `apps/desktop/tauri.conf.json`, **two signed builds**
(native arm64 + x64 cross, bi-arch since PLAN-RETOURS-8/ADR 0023;
key at the **path** `C:\Keys\wind.key` — `TAURI_SIGNING_PRIVATE_KEY`
accepts a path; password entered once), `latest.json` with no
BOM at **two platform keys**, then — after `OUI` confirmation —
commit `release: version <v>`, push (gate replayed), BARE tag + GitHub
Release `--latest` with **five assets**, notes pulled from the CHANGELOG.

Control **after the fact**, before announcing it green:
**`scripts/verify-release.ps1 <v>` runs every form check**
(the friction is encoded once — with two platforms, the
manual checks were doubling). What it verifies, and which stays the
norm when checking by hand:

- **The Release is "Latest"** — the updater endpoint is
  `…/releases/latest/download/latest.json`:
  `gh api repos/smonchamps/wind/releases/latest --jq '.tag_name'`
  must return the new version.
- **Five assets at the BARE tag** (never `v<x>`), named exactly:
  `Wind_<v>_arm64-setup.exe` + its `.sig`, `Wind_<v>_x64-setup.exe` +
  its `.sig`, `latest.json`. ("Five" is not enough by itself: two exes of the
  same architecture would pass a simple count.)
- **`latest.json` with no BOM** (first bytes `7b` = `{`, not
  `ef bb bf` — serde_json silently rejects it).
- **BOTH platform keys present** (`windows-aarch64` AND
  `windows-x86_64`): a missing key is a **silent failure**
  — the mute channel's updater concludes "no update available",
  without error. Same family as the BOM and the `v` tag (ADR 0013).
- **Per platform**: manifest signature == `.sig` file of the
  SAME architecture; URL at the BARE tag (`/releases/download/<v>/…` — the
  404 trap) toward the exe of the SAME architecture; the URL resolves
  (302 then 200, `Content-Length` = asset size).
- **DISTINCT arm64 and x64 signatures** (anti-cross-wiring guard):
  a signature copied under the wrong key passes every form
  check and only breaks at the user's end.
- **minisign crypto is NOT locally verifiable** (no
  `minisign` on this workstation; `tauri signer` has no `verify`). Never
  fake a PASS: the definitive proof is the **`<n-1> → <n>` auto-update
  observed in the field, PER CHANNEL** — arm64 on this
  workstation; x64 on the second x64 workstation (CE decision D5,
  PLAN-RETOURS-8). The first x64 auto-update can only be observed at
  the release FOLLOWING the first bi-arch release (no n-1 x64
  exists before it); the x64 install itself is observable from the
  first one.
- `CHANGELOG.md` (root) carries the `## [<v>] - <date>` entry and the
  link to the Release at the bottom.

---
## 3. The product

**Promise:** *"Your mail, instantly."* An email client that
starts in under a second, where every action responds in under
100 ms, and that works offline as well as online.

**Target:** demanding professional or individual, 1 to 4 accounts
(Gmail, Microsoft 365, generic IMAP — all three shipped and
validated).

**What it IS:** fast (performance is THE feature), simple (read,
sort, search, write — nothing else), reliable (never a loss, never
a phantom send), safe (credentials in the OS vault, HTML sanitized,
remote images blocked). Since ADR 0010: **complete** — the whole
mailbox is local and searchable, spam and trash included.

**What it is NOT (v1):** no calendar, no chat, no built-in AI, no
plugins, no mobile.

### Budgets — these are BLOCKING gates

Re-measured on 2026-07-26 after ADR 0010, on the gate 3 fixtures (3
accounts, 200,000 messages):

| Metric | Target | Last measured |
|---|---|---|
| Cold start | < 1 s | 337 ms on the gate 3 fixture ✅ — and **384.6 ms on the REAL database** (12.84 GB, 251,524 envelopes, 64 mailboxes), first launch after a machine restart, 2026-08-26: the project's first honestly cold measurement ✅ |
| Opening a message | < 50 ms | 1–3 ms ✅ |
| List page | < 100 ms | 0.58 ms ✅ |
| RAM (**private** working set) | < 200 MB | 95.5 MB · 7 processes ✅ |
| Database size | **lifted** (ADR 0010 §2) | disk-space guard at ~50 KB/message |
| Data loss | 0, proven by crash recovery | ✅ |
| **Mail pump freeze** | no freeze > 150 ms (window always movable) | 0 freezes over 40 s, fixture 251k envelopes (PLAN-GELS, `e2e/freeze-probe.py`) ✅ |
| **Search** | < 100 ms | **~66 ms ✅** (field, real database 251k / 7 GB, worst case 3-char prefix, 36k matches; held by the **sort-by-date relief valve** past 10k matches, since the BM25 floor is exceeded otherwise — `WIDE_QUERY_THRESHOLD`, A50/PLAN-RECHERCHE) |
| **Adopting a legacy database** | < 1 s | **3.66 s — accepted** (ADR 0012: once only, visible, cancelable, reversible) |
| **Rebuilding the search index** | no silent freeze | **~4 min cold on 7 GB — accepted** (ADR 0012: once only at update time, visible, cancelable, reversible; PLAN-RECHERCHE E3) |
| **Rebuilding the envelope date index** | no silent freeze > 2 s | **1.77 s cold — accepted WITHOUT a screen** (PLAN-DEMARRAGE, CE decision D9: once only at update time, and it only reads `envelopes` — 47 MB — never the bodies. A screen that appears and disappears in 1.8 s is more annoying than the wait) |

An exceeded budget = **we stop the line** (andon). The "database
< 1 GB" gate is not an oversight: it is **explicitly lifted** by
ADR 0010 §2, replaced by the disk-space guard.

⚠️ **Measurement tools are checked like everything else.** Three of
them lied at gate 3 (RAM summed across every instance, a
non-isolated WebView2 profile, a fixture that did not exercise the
partial index). Fixed — but the reflex still has to be there.

---

## 4. Architecture — "a single brain"

`mail-core` contains **100% of the business logic**, the sync, and
the storage. The desktop app embeds it as a process; the web app
(Phase 4) will run it server-side. The UI is "dumb": it displays a
state, it emits intents.

```
wind/
├── crates/
│   ├── mail-core/     # domain + sync + storage + search + threads
│   │                  # (ZERO UI or network dependency)
│   ├── mail-imap/     # IMAP adapter (implements MailServer)
│   ├── mail-auth/     # OAuth2 PKCE loopback + Windows vault (keyring)
│   ├── mail-ical/     # iCalendar/iTIP invitations (calcard), PURE (ADR 0024)
│   ├── mail-render/   # HTML sanitization (ammonia) + text + CSP
│   └── mail-smtp/     # SMTP adapter (lettre, XOAUTH2)
├── apps/desktop/      # Tauri 2: commands.rs (IPC) + main.rs + ui/ (vanilla JS)
├── e2e/               # Playwright driving the REAL window via CDP WebView2
├── spikes/            # throw-away prototypes, outside the prod workspace
└── docs/              # PLAN, phase reviews, ADRs, this document
```

**The only abstract boundary** is the `MailServer` trait (read) and
the `MailTransport` port (send). **SQLite is NOT behind a trait**:
frozen decision; `Store` is a concrete struct, tests use an
in-memory database, and the journal is **WAL** on file (ADR 0011).

**A recurring pattern, worth imitating.** The decision is **pure and
testable**, the execution (I/O) is elsewhere: `thread::plan`
(conversations), `plan_draft_pull` (drafts), `convert::sent_folder`
(sent folder), `notify::arrivals_to_notify` (bubbles), and since ADR
0010: `sync_order` (mailbox order), `sync_percent` (progress),
`disk_shortfall` (disk-space guard). This is what makes it possible
to test field scenarios without a network.

---

## 5. Frozen decisions — do not reopen without measurement

| ADR | Decision | Takeaway |
|---|---|---|
| [0001](adr/0001-workspace-structure.md) | Multi-crate Cargo workspace | `mail-core` with no UI/network dependency |
| [0002](adr/0002-tauri-desktop-shell.md) | Desktop shell = Tauri 2 (WebView2) | The RAM that counts = **private** working set |
| [0003](adr/0003-smtp-outbox.md) | SMTP outbox + golden rules | Journal BEFORE network; anti-phantom quarantine |
| [0004](adr/0004-fts5-search-engine.md) | Search = SQLite **FTS5** | The index lives INSIDE the database (transactional) |
| [0005](adr/0005-e2e-gate-outside-hosted-ci.md) | E2E outside hosted CI | A GitHub runner cannot open WebView2 — hence the `pre-push` hook |
| [0006](adr/0006-microsoft-imap-oauth2.md) | Microsoft via IMAP+OAuth2, not Graph | Graph remains the encrypted plan B |
| [0007](adr/0007-body-backfill.md) | Bounded, resumable, grouped body backfill | **Horizon lifted by ADR 0010**; the shape (bounded/resumable/grouped) remains |
| [0008](adr/0008-conversation-threading.md) | Conversations = union-find on RFC 5322 headers | **Never a subject-line fallback**; recomputed aggregate; an identifier requires an at sign |
| [0009](adr/0009-thread-scope-per-account.md) | Thread scope = the **account** | "Sent" synced; **partial index** or gate 3 is lost |
| [0010](adr/0010-full-synchronization.md) | **Full synchronization** — everything, no horizon or quota | Gate < 1 GB **lifted**; **storing ≠ grouping** (scope = INBOX + Sent); disk-space guard; progress in % |
| [0011](adr/0011-wal-journal.md) | SQLite journal in **WAL** | A read no longer blocks a long sync; persistent, legacy databases converted |
| [0012](adr/0012-visible-interruptible-migration.md) | **Visible and interruptible** migration | Adoption is ONE reversible transaction — canceling leaves `user_version` unchanged, never a partial adoption; read-only `pending_adoption` probe, which announces the **scope** |
| [0013](adr/0013-nsis-installer-signed-update.md) | **NSIS** installer + signed update | **Not MSIX** (would virtualize `%APPDATA%`, orphan the database); minisign-signed updater, driven from Rust; Windows signature deferred; GitHub tag = **bare version**, `latest.json` without BOM (`scripts/make-release.ps1`) |
| [0014](adr/0014-local-crash-telemetry.md) | **Local, opt-in** crash telemetry | Local file only (no network/third party); panics only; **panic message dropped** (the only PII vector); a hook that never touches the database; a main-thread crash does a **double panic** (`SEQ` counter + `cannot unwind` filter) |
| [0015](adr/0015-ui-v2-svelte-foundation.md) | **UI v2 foundation = Svelte**, single web frontend carried everywhere (Tauri 2 desktop+mobile + browser) | Set-based decision (vanilla / Svelte / WASM) **on measurement**: 256k list + theme toggle, two engines (desktop Blink, Android-class CPU ×6) — rendering neutralized by windowing + CSS theme. **System written once** (Strategy A); WASM ruled out, vanilla as fallback; **iOS/WKWebView: field validation still due**; UI↔core boundary = transport port; `mail-core` untouched (ADR 0001) |
| [0019](adr/0019-commands-off-the-main-thread.md) | **Blocking commands off the main thread**, one at a time (`off_pump` = spawn_blocking + global lock) | The pump only pumps (measured freeze: 25.2 s/40 s → 0); the previous serialization is KEPT; `main-thread-guard.mjs` gate + "no freeze > 150 ms" budget (`freeze-probe.py`) |
| [0024](adr/0024-icalendar-parser-calcard.md) | iCalendar invitations = **calcard** in pure `mail-ical` | Decided by spikes (shared corpus) on **cost of ownership**; native Windows TZIDs; ⚠️ `resolve()` never `resolve_or_default` — an unknown TZID renders a FLOATING time stated as such (guard D1), never wrongly converted |

Phase 0 decisions ([PHASE0.md](archives/PHASE0.md) §2): local SQLite;
CONDSTORE; MIME parsing by `mail-parser`; OAuth2 PKCE loopback + OS
vault; defense-in-depth HTML rendering.

---

## 6. Non-negotiable invariants

Easy to break **silently**. Checked at every review.

1. **Outbox — the two golden rules** (ADR 0003): never a lost send
   (the intent is journaled BEFORE any network); never a phantom
   send (quarantine, never an automatic resend). *"A duplicate is
   worse than a delay."*
2. **Message identity = `(account_id, mailbox, uid)`** everywhere,
   down to the UI selection. UIDs are assigned per mailbox and
   restart at 1 — and since ADR 0010, an account carries DOZENS of
   mailboxes. The compiler does not protect this invariant; a test
   holds it (`chaque_ligne_dit_dans_quelle_boite_elle_habite`).
3. **Indexes and aggregates live INSIDE the database**, maintained
   in the SAME transaction as the message: FTS5 index, `threads`
   table.
4. **Rendering security**: HTML sanitized by `ammonia`, remote
   images blocked, sandboxed iframe + CSP, `textContent` never
   `innerHTML`. **Single, bounded exception (A62)**: the composer's
   rich editor sets via `innerHTML` — that is its job — but accepts
   ONLY HTML that has passed the ammonia boundary on the Rust side
   (`frontiere_corps`, quotes included). Remote images there are
   decided BY GESTURE (field verdict D5, 2026-08-20): a REPLY quotes
   at neutral pixel — the 2026-08-20 review showed the exact trap, a
   quote sanitized in `AllowRemote` loaded the spy pixels of the
   quoted message on a plain "Reply" click (the main document's CSP
   allows `img-src https:`); a FORWARD, however, KEEPS the images —
   the recipient receives the whole message, and composing the
   forward implicitly means "show images," it is the gesture that
   says so.
5. **Credentials never in plain text**: Windows Credential Manager
   via `keyring`.
6. **UIDVALIDITY**: if it changes, the mailbox restarts from zero
   and **the whole account** rebuilds its threads
   (`thread::rebuild_account`). Drafts: *"a duplicate is acceptable,
   deleting the wrong UID never."*
7. **A new feature must ADOPT the old data** — the trap has shown up
   four times (§9). Migration written at the same time as the
   feature, proven by a test that rewinds a real file database.
8. **Diagnostics disclose nothing**: no subject, no sender, no
   content; **masked** identifiers (shape only).
9. **We store everything, we group only the scope** (ADR 0010 §3).
   A message outside INBOX + Sent keeps `thread_id = NULL` forever:
   without that, spam attached to a thread would bump it to the top
   of the list (corrupting `size`, `unseen`, `last_epoch`). Carried
   by `mailboxes.threaded` + `accounts.sent_mailbox`, held by
   `un_message_hors_portee_ne_rejoint_pas_le_fil`. **The scope is
   declared on the account BEFORE the mailboxes exist** — the sync
   loop creates them
   (`une_portee_declaree_avant_la_creation_de_la_boite_vaut_quand_meme`).
10. **Nothing touches the database before `migration_check`** (ADR
    0012, A41). Any command played before the migration modal must
    be a probe that does not adopt (`Store::pending_adoption`,
    `Store::text_pref_readonly`); a deferred write only fires once
    the migration probe has answered, and a read failure means a
    session fallback — never a write. Held by
    `la_langue_se_lit_sans_adopter_la_base` (rewound file
    database); the UI-side order (`main.js` → `assurer()` →
    `poserLangueDetectee()`) has no structural guard — to be
    checked at every startup command added.

---
## 7. Environment & commands

Windows 11. Two shells: **PowerShell 5.1** (primary) and **Bash** (Git
Bash). Different syntaxes.

### 7.1 Traps that cost dearly

- **PowerShell 5.1 has no `&&`.** Two lines, or Bash.
- **NEVER use `Get-Content`/`Set-Content` on the sources**:
  UTF-16 BOM re-encoding, corrupted accents. Edit via the `Edit` tool,
  Python, or Bash. Everything is **UTF-8**.
- For non-ASCII output from Python: `PYTHONIOENCODING=utf-8`.
- **The assistant does NOT see the real database.** The Claude
  application is packaged as MSIX: its shell reads a **redirected**
  `%APPDATA%`, and `wind.db` resolves there to a stale private copy.
  **The §9 diagnostics are run by the user**, who pastes the output.
  Corollary: announce first what one expects to read there, so the
  round trip is a measurement and not a collection — and pass on
  each figure **with its exact definition** (a "~1,650 remaining"
  read as a leftover when it was actually a total cost a false
  prediction).
- **Since ADR 0011, the database has two companions**: `wind.db-wal`
  and `-shm`. A hot copy must take all three.
- **The app in `--release` is SILENT on console** (`main.rs`:
  `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]`):
  *windows* subsystem, no console attached, `eprintln` (the
  `run_sync` trace) has nowhere to write to. To read a trace in the
  field: either **debug** (`cargo run -p wind-desktop`, console
  attached, but inflated CPU durations), or **redirect via a launcher
  that WAITS** (`cargo run … --release 2> file` — cargo, a console
  app, holds the handle to the end; exact release timing). ⚠️
  **Launching the BARE exe traces NOTHING** (`& …\wind-desktop.exe 2>
  file` from PowerShell): PowerShell does not wait for a windowed
  executable and stops reading its stderr as soon as the prompt
  renders — file created, empty FOREVER, even when the traces do get
  written. Paid twice: PLAN-RETOURS-2 (a "no trace" taken for "no
  sync"), PLAN-RETOURS-5 (two field passes burned on an empty file,
  2026-08-21).
- **A commit cannot be chained with `git --no-pager …`**: the
  `block-no-verify` hook blocks the `--no-` prefix. Separate the
  commands.
- **`prefers-color-scheme` is DEAD in Tauri's WebView2**: never
  dark, zero event, even under a real Windows toggle (measured at
  the probes, field A42 of 2026-08-16). Listening to the OS theme
  goes through the Tauri window API (`theme()` + `onThemeChanged`);
  `matchMedia` is only the fallback outside Tauri and the bench's
  handle (emulateMedia). Corollaries: `Set-ItemProperty` on
  `AppsUseLightTheme` notifies NO ONE (no `WM_SETTINGCHANGE`) — a
  field check goes through Windows Settings or
  `e2e/dark-toggle.ps1`; and the suite's WebView2 profile
  (`target/e2e/webview2`) PERSISTS between runs — a test that dies
  after arming a localStorage setting poisons local reruns (fix:
  purge the folder; CI, for its part, always starts clean).
- **The reading thread is ONE object, TWO frames** (UI v3, A43,
  2026-08-16): `Fil.svelte` + module state `lib/fil.svelte.js`, and
  frame exclusivity lives in the store (`fil.cadre`:
  null/pane/full) — never a local visibility boolean in a frame
  (three booleans reconciled by hand went out of sync on the first
  forgotten path, v3 review). Corollaries: every purge goes through
  `fermerFil()` (importable everywhere — `lecture?.fermer()` was a
  no-op in 1-2 panes); every `ouvrirFil` RELOADS (memoization was
  hiding its own sent response); the P1 "opening" stopwatch now
  measures selection → thread displayed (thread_messages included,
  attachments excluded) — the pre-v3 series is not comparable. Bench
  lesson: a testid `sed` can DISARM discriminating assertions —
  re-scope to the frame (`[data-testid="volet-lecture"] …`) and
  assert uniqueness (`toHaveCount(1)`).
- **Scrollbars are NATIVE overlays** (A44, 2026-08-16): Chromium
  trait `OverlayScrollbar`, set by tauri.conf.json's
  `additionalBrowserArgs` — this field is spelled WITHOUT
  "uments", and setting it REPLACES wry's default
  `--disable-features` (carried over into the value). Three traps
  measured: the `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` variable
  OVERWRITES the conf at the loader level — any launcher that sets it
  must reuse the prod args (`e2e/browser-args.mjs`, single source:
  launch, measure-v2, diag-v2); a REPEATED `--enable-features` is not
  merged, the last one wins; `scrollbar-width:auto` does NOT disarm
  webkit rules (the default value — a non-default value is needed to
  probe the native path). ONE `::-webkit-scrollbar` /
  `scrollbar-width` / `scrollbar-color` rule anywhere falls the
  element back to the classic path (~15 px gutter) — guard #5 of
  `system-coherence.mjs` blocks it; the `color-scheme` (light handle
  in night mode) lives in CSS next to the tokens AND baked into the
  body's iframe (mail-render, background luminance).
- **The list has had TWO templates since A44** (field 2026-08-16:
  content-fit height — the chip row only exists on carrier rows): the
  pre-A29 windowing mechanics is back (h1/h2 probed, `chipsParPage`,
  `chipsAvant`, iterative index, anchoring on the delta of a
  resupplied page). Every new row variant must enter BOTH probes, and
  the P1 bench is read with h1 AND h2 (D-14: re-base).
- **A Tauri command without `async` runs on the MAIN THREAD** — the
  one of the message pump: the window freezes for its whole duration
  (finding 2026-08-15, freezes of 2 to 4.6 s at startup). Every
  command that opens the database, touches a file or the keyring is
  `async fn`; the `e2e/main-thread-guard.mjs` gate holds it (named
  exemption for pure state reads). Measuring the symptom:
  `python e2e/freeze-probe.py <base.db>` (database OUTSIDE the repo).

- **The embedded SQLite (3.50) does not have the planner of the tool
  used to measure.** A query costing 0.2 ms under python cost 116 ms
  in Wind: the date index chosen instead of the sender's.
  `INDEXED BY` on the Cleanup queries, and an execution PLAN test
  that guards it (PLAN-AUDIT-V2 E4).
- **While a gate is running in the background, the sources are NOT
  touched.** The e2e recompiles `ui-v2` and the examples when they
  change — an edit mid-flight gives a verdict on a state that no
  longer exists. Reading, writing docs: yes; Rust, Svelte, scripts:
  after the verdict (PLAN-AUDIT-V2).
- **A frame `sandbox="allow-same-origin"` WITHOUT `allow-scripts`
  (S1) is not evaluable by Playwright** — `click`, `dispatchEvent`,
  `evaluate` hang there until timeout (3 min paid). To strike a key
  "inside the body": focus the iframe from the parent
  (`locator('iframe').focus()`) then `keyboard.press` — the real key,
  delivered to the frame's document.
- **Replacing by substring means catching everything that contains
  it.** `reply_to: None,` lives INSIDE `in_reply_to: None,`; `Nom {`
  catches the `-> Nom {` of a signature. An editing script steers by
  POSITION landmarks (block start/end, matched braces) and checks
  itself against the compiler (E0063 gives the exact list of
  incomplete literals). And `python -` under PowerShell decodes the
  input as cp1252: `python -X utf8`, or the script in a file — an
  `assert old in s` that lies for no reason is often the encoding or
  a stray `\r` mixed in (PLAN-AUDIT-V2, six repair passes paid).

- **A `.ps1` without a BOM must contain only ASCII in its strings.**
  PowerShell 5.1 (the one from `powershell -File`, in the field)
  reads a file without a BOM as ANSI: an em dash "—" becomes
  `â€”`, and that `”` is a closing quote for its parser — the string
  closes, the script no longer parses (`verify-release.ps1`,
  2026-09-02, a release verified by hand). A `.ps1` carrying
  non-ASCII carries a UTF-8 BOM (`make-release.ps1`); the gate
  (step 6) parses every `.ps1` with that PowerShell's parser.

### 7.2 Notifications require the INSTALLED application

`tauri-winrt-notification` requires an application identity
(AppUserModelID), carried by a Start menu shortcut. So:
`cargo tauri build`, install, launch from the Start menu;
`GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET`, `MICROSOFT_CLIENT_ID` set
at the user level. ⚠️ Windows only registers the application in
Settings → Notifications AFTER its first successful notification.

### 7.3 Commands

```bash
cargo test --workspace --all-targets           # everything, EXAMPLES INCLUDED
cargo test --workspace --doc                   # the doc-tests, excluded above
cargo build -p wind-desktop --release     # binary
cargo run -p wind-desktop --release       # launch (without notifications)

cargo fmt
cargo clippy --all-targets -- -D warnings

cd e2e
npm test                                       # PowerShell: two lines

# Test set — <db> <count> <email> [body] [kb/body] [mailbox] [senders]
cargo run -p mail-core --example seed_inbox --release -- <db> 33000 un@exemple.fr 0 0 INBOX
# Benches (PLAN-AUDIT-V2): diag_opening, bench_search, bench_indexing,
# bench_cleanup (MUTES the database: fixture only), on C:/mesure/banc200k.db and
# banc5000.db (200k envelopes; 8 or 5,000 senders)

# Installer (needed for notifications)
cd apps/desktop
cargo tauri build
```

Measurements: `node e2e/measure-v2.mjs` (startup, page, RAM —
`MEASURE_DB`, `MEASURE_ACCOUNTS`, `MEASURE_REUSE`),
`e2e/measure-ram.ps1`, and `python e2e/freeze-probe.py <base.db>`
(message pump freeze, budget "no freeze > 150 ms", PLAN-GELS —
requires Python 3, the only tool in the repo that does).

⚠️ **The measurement database goes OUTSIDE the repo** (OneDrive
would disturb the measurement). The three gate-3 databases
(`gate3.db`, `gate3-corps.db`, `gate3-envoyes.db`) are **kept** in a
session's temporary scratchpad and were **migrated to the ADR 0010
schema** on 2026-07-26 — they remain valid and comparable. It is a
Temp folder: check they exist before use, regenerate with
`seed_inbox` otherwise (several minutes; do not do it "just to be
safe").

### 7.4 The pre-push gate

`.githooks/pre-push` DELEGATES to `scripts/gate.ps1` (PLAN-AUDIT-V2
E9, debt D-32 closed: a single gate, no more copied-out commands).
Ten steps: `fmt` → build ui-v2 → contrasts → System coherence →
main-thread guard → `node --check` on the scripts → `clippy -D
warnings` → `cargo test --workspace --all-targets` → `--doc` → `npm
test` (e2e); at the verdict, "flaky: N" with the names (Playwright
JSON report, `e2e/flaky.mjs` — D4: a flaky is counted, it does not
turn the gate red). **Fast documentary path** (PLAN-KAIZEN-CLAUDE,
E4; `gate.ps1 -DocsOnly`): if everything going out is ⊆
`docs/**` + `*.md` — excluding `docs/design/**`, the System is
tooled normative material — the slow steps (clippy, Rust tests, e2e)
are skipped, the six steps that take seconds remain; a new ref or a
removed ref ⇒ full gate. CI remains the complete net — its actions
are pinned by SHA (Dependabot keeps them up to date).

**`--all-targets` is not decorative**: without it, cargo ignores the
EXAMPLES' tests — the field diagnostics live there and carry their
tests. `--no-verify` exists; using it is a decision, not a shortcut.

**Two gates can run at the same time** (two worktrees, two pushes):
since PLAN-ISOLATION-E2E (2026-08-15), each e2e suite gets a free
CDP port chosen by the OS (`e2e/port-cdp.mjs`, one port per suite —
the browser arguments of the same WebView2 profile must stay
identical), and the zombie sweep of `rebuild-v2.mjs` is bounded to
the current worktree's `target/`. Before: a shared port 9222 + a
global sweep = applications struck down with `0xFFFFFFFF` and no
output, and suites steering one another (`connectOverCDP` recognizes
its window on the sole criterion `tauri.localhost`). Field: 73 + 73
green simultaneously.

**The gate only reflects CI on the SAME toolchain.** The Rust
version is **pinned** in [`rust-toolchain.toml`](../rust-toolchain.toml)
(single source: local + hook + CI). The CI job, for its part, does
not read this file: its action ref is pinned by hand in
[`ci.yml`](../.github/workflows/ci.yml) — **bumping the version is
done in BOTH places**, then clippy is replayed (a new lint can
appear). Lesson paid: CI was tracking "the latest stable" and the
hook was running on a local toolchain that lagged behind (1.94 vs
1.97); a new clippy lint broke CI without the local gate seeing it.

### 7.5 E2E determinism

Watertight by construction: a throwaway database (`WIND_DB_PATH`),
fake accounts (`WIND_E2E_ACCOUNT`), `GOOGLE_CLIENT_ID`/`SECRET`
removed, a dedicated WebView2 profile. **The E2Es talk to no
server**: the whole real network path (OAuth, folders, background
passes, STATUS) is covered only by unit tests on the pure part and
is proven only in the field.

---
## 8. What remains → [STATE.md](STATE.md)

Recent jobs, the long tail, and deliberate deferrals live in
[STATE.md](STATE.md); the detailed debt, in [DEBT.md](DEBT.md).

---

## 9. Lessons — read before resuming

They cost dearly. Ignoring them will make them cost again.

### Defects are found in the field, not in tests

Never logic errors: always **false assumptions about the environment
or usage**. A test suite shares the assumption. Latest example:
"database is locked" on the first trial of full sync (ADR 0011).

### A periodic reader alongside long writes requires WAL

Rollback mode held up as long as writes took seconds. Full sync
stretched them into minutes, and the progress poll (800 ms) expired
writers' `busy_timeout` on the very first trial. **The risk had been
named in review; it should have been addressed then.** When adding a
periodic reader, check the journal mode.

### An inherited bound is not a decided bound

The header pass borrowed the 12-month horizon of the body backfill —
a bound that existed for the **disk budget**, whereas a header block
weighs ~3 KB and does not strain the disk. Reused because the
function had the same *shape*, not the same *reason*. The diagnostic
showed it converged at 1 656/1 656 with 78 % of the database
permanently out of reach. **When inheriting a parameter, re-examine
its reason for being.**

### A scope declared before its object is created is stored on the parent

The sync loop **creates** the "Sent" mailbox: at the moment the
grouping scope is declared, there is no row to update, and the
mailbox would be born out of scope — threadless messages until the
next startup, with no signal. Hence `accounts.sent_mailbox`, read by
`create_mailbox`. **A declaration that precedes its object is carried
by the parent, not by call order.**

### A diagnostic written for one scenery is re-read in the next scenery

On the full database, "never read: 250 864" mixed real backlog with
the deliberately-ignored out-of-scope — a figure that no longer
points to anything makes the diagnostic get rerun for nothing. Broken
down by scope the same day. **When the scenery changes (ADR), re-read
every diagnostic through the eyes of the new scenery.**

### A new feature must ADOPT old data

The trap showed up **four times**: attachments, conversations, thread
headers, schema. `CREATE TABLE IF NOT EXISTS` does not touch an
existing table, but a new partial index fails on a missing column —
and the app stopped starting. **Write the migration together with the
feature, prove it with a test that rewinds a real file database.**
(The ADR 0010 migrations — three columns — followed this rule and
passed silently on both the real database AND the gate 3 databases.)

**Recurrence (2026-09-02, PLAN-AUDIT-V2 STOP 2)**: the `reply_to`
column placed in the `CREATE TABLE` without its line in
`add_missing_columns` — six green gates, the e2e scenery freshly
seeded, and on the real database every watcher pass failing. Rule:
**one new column = three lines** — the `CREATE TABLE`, the list of
migrated columns, and a REOPEN test on a file database that has had
the column removed (`une_base_d_avant_la_vague_2_…`).

### Measure before fixing — including your own assumptions

On the false grouping, three assumptions were wrong; the diagnostic
pointed to the cause in one command. On adoption, the announced
"dominant" cause accounted for only a quarter of the cost. Seven
tools exist, same model — read-only, **no content disclosed**:

| Tool | Answers |
|---|---|
| `diag_index` | are messages in the search index? |
| `diag_threads` | what identifier joins a thread? (broken down by scope since ADR 0010) |
| `diag_drafts` | does the draft pull do its job? |
| `bench_list_page` | does a page's cost depend on mailbox size? |
| `bench_thread_migration` | what does adopting a legacy database cost? (copies via `VACUUM INTO`, does not mutate the target database) |
| `bench_search` | do search and opening hold their budgets? |
| `seed_inbox` | build a scenery (the 500 most recent get a body) |

Writing a new one costs 40 lines and saves a round trip.

### A green test can encode a FALSE model of the other writer

Draft conflict detection simulated the pull with an in-place
rewrite; the real pull **replaces**. **Simulate the other writer by
calling ITS REAL PATH.** Same family: a fake server must report
exactly what it serves (`FakeServer::exists` and `message_count`
return the real scenery, never a constant).

### An index's promise holds only for the query you had in mind

ADR 0008 §4 reasoned about one mailbox; the product queries the
unified mailbox — 987 ms of materialized sort, invisible at field
scale. **A QUERY PLAN test catches this class of regression.**

### A measurement scenery can never exercise what you think it validates

The partial index lived for several days without a single thread
ever being excluded: the scenery had only one mailbox per account.
**Check that the scenery produces the condition the code claims to
handle.** Corollary from ADR 0011: testing WAL on an IN-MEMORY
database would have validated a false model — it answers "memory".

### A test that does not run is not a test

`cargo test --workspace` ignores example tests — hence
`--all-targets` in the gate (§7.4).

### The compiler does not protect an identity made of strings

`account_id` and `mailbox_id` are `i64`, a mailbox is a `String`.
After a signature change, the code compiled while targeting the
wrong message. Hold the invariant with a test.

### A requested signal must be OBSERVABLE

Check in the code that every signal requested during validation is
actually displayed — and not overwritten one line later. Textbook
case: a progress bar must never say "0%" when it does not know, nor
"100%" until it is actually done (`sync_percent`, degenerate cases
tested).

### A status set without looking erases another

Three times. When a function sets a status message, the caller
decides its own from its own outcome. (That's why sync progress has
its own banner, separate from the status line.)

### Never swallow an error

`let _ = …show()` destroyed the proof of a notification defect.
Non-blocking failures surface in the sync outcome — full sync logs
the failure of EACH folder without ever blocking the others, and the
disk-space guard **says how much** is missing rather than
"insufficient space".

### A measurement tool is checked like everything else

`measure-ram.ps1` summed every instance; `measure-v2.mjs` did not
isolate its profile; a diagnostic disclosed identifiers by splitting
a whole header on its first `@`. Fixed — the reflex remains.

### A "hidden" element can stay rendered — and steal focus

`#detail { display: flex }` overrode the browser's `[hidden]` (an
ID's specificity beats the default stylesheet): the reading pane
stayed rendered at all times, its sandboxed iframe covered half the
window, and the first click into it lost the keyboard — shortcuts
dead until you clicked elsewhere. **Invisible to E2E**, which injects
its keystrokes over CDP without going through the Windows window's
focus; found by the Chief Engineer during the field validation of
ANOTHER job (ADR 0012). Two lessons: every ID rule that sets a
`display` needs its `#id[hidden]` safeguard (the whole class was
combed through); and a session's first gestures — clicking anywhere,
`/` right away — are a field pass of their own.

### Validating a fast screen requires the scenery that slows it down

On the real mailbox, the migration screen lasts under a second: the
scope to adopt (~7 500 messages) is 30× smaller than the gate 3
scenery. Cancelling mid-pass is only exercised on a rewound
`gate3.db` (`user_version = 0`), where the bar takes ~4 s to climb.
**Choose the scenery for the property you are validating, not for its
realism.**

### The release chain has its own false assumptions

Updater validation (ADR 0013) paid for two traps, neither in the
Rust code — both in the **release tooling**. A hand-written
`latest.json` got corrupted (multi-line PowerShell paste, then a BOM
risk that `serde_json` rejects). And the package URL pointed to
`releases/download/v0.1.2/…` while the GitHub tag is the **bare
version** (`0.1.2`): the banner appeared — detection worked — but the
install returned 404. **The path between `cargo tauri build` and the
user's app is field territory too; it is diagnosed by looking at the
actually published assets (GitHub API), not by assuming.** Both are
now held by `scripts/make-release.ps1`.

A third trap, same family (CE finding on 2026-08-22): the **release
notes came out as mojibake** ("Ã©" for "é") on nine versions (0.1.10
to 0.6.0). Root cause: the script read the UTF-8 CHANGELOG with
`Get-Content -Raw` **without `-Encoding UTF8`**; invoked by
`powershell` (Windows PowerShell 5.1, default encoding cp1252), it
decoded the UTF-8 as Latin-1, then `WriteAllText` re-encoded it as
UTF-8 — double encoding. The Rust code is off the hook, again: the
app's body was clean, only the GitHub Release notes were affected —
invisible to the gate, visible in the field (here on the Releases
page). Root fix: `-Encoding UTF8` on the script's three UTF-8 file
reads (including `tauri.conf.json`, the same latent trap the moment
an accent enters it); the nine Releases repaired by hand from the
clean sections of the CHANGELOG, via `gh release edit --notes-file`
through a path that does not re-encode. **A publishing script that
reads UTF-8 under PowerShell 5.1 must say so — the shell's default is
not UTF-8.**

### A command's thread is a decision, not a detail

In Tauri 2, a command without `async` runs on the main thread — the
message pump. Thirty-four commands opened the database from this
thread; all was fine as long as they stayed under ~100 ms, then a
130 MB backfill batch froze the window for 4.6 s straight (CE finding
on 2026-08-15: "the window cannot be moved"). The cost of the queries
was not the root cause — their PLACE was: 865 ms is acceptable on a
background thread, unacceptable on the pump. Root fix: every blocking
command is `async`, a gate holds it (a named and justified exemption
for pure state reads), and the symptom has its own instrument —
`freeze-probe.py` measures the pump the way Windows judges it
(`SendMessageTimeout`). Before/after on the same scenery: 25.2 s of
cumulative freezes → zero.

### A panic on the main thread makes TWO panics

Crash capture (ADR 0014) proved itself right in tests, but the field
showed a behavior no unit test could see: a panic on the main thread
tries to unwind, crosses the WebView2 FFI boundary (nounwind), and
triggers a SECOND panic `cannot unwind` that aborts. The hook runs
for both, in the same second — the second overwrote the first (the
only useful one). Fixed with a counter in the filename and a filter
on the secondary panic. **The environment's behavior at the moment of
a crash is only seen by actually crashing.**

### A third-party library delivers what it delivers, not what you assume

PLAN-RETOURS-MAIL paid for two false assumptions about libraries, and
a field capture settled the third. `imap-proto` strips the quotes
from an IMAP `quoted-string` but **leaves the backslash escapes in
the content** (`\"`, `\\`) — proven by its own tests; our quoted
subjects showed up polluted. `ammonia`, for its part, strips a
forbidden tag but **unwraps its text** by default (outside
`clean_content_tags`): a newsletter's `<head><title>` leaked at the
top of the body. Neither is guessable — you **read them in the
crate's source** (or its measured behavior). And on the duplicate-
subject case, my first two hypotheses (the body's `<h1>`, the
unmasked preheader) were wrong: it was the **CE's Gmail-vs-Wind
capture** that named the real culprit, the `<title>`. **When a render
differs from a reference client, a side-by-side capture is worth ten
hypotheses.**

### A decoding fix does not repair data already decoded

The new unescaping only cleaned NEW envelopes; subjects already in
the database kept their escapes (incremental sync does not re-read
existing rows). As with previews (D-5) and threads, **a decoding
change requires a pass over what already exists** — here a migration
that unescapes the stored value (equivalent to the new decoding: the
content is already RFC 2047-decoded, only the IMAP escape layer
remains). The reflex of the four adoption traps (§6.7), in another
form.

### A disk I/O measurement is only valid cold

Measuring a disk-bound reconstruction or migration on a freshly
written copy (`Copy-Item`) is a lie: the copy leaves its pages in the
RAM cache, so the re-read is served from memory. Measured fact
(PLAN-RECHERCHE, 2026-08-17): FTS5 rebuild on 7 Go / 130 k bodies —
**0.7 s** on a fresh copy, **~4 min** cold in the field, a **×340**
gap (announced as "×5-10 at worst"). The dominant cost is not the
computation but **re-reading the bodies** from disk — invisible on a
warm cache. Product corollary: any FTS5 schema change forces a
rebuild that re-reads the bodies; on a supplied database, take it out
of the startup path (ADR 0012 modal, detected by `pending_adoption`).
**Never conclude "budget held" from a lab measurement when the real
path is disk-bound.**

### A contenteditable is neither an input nor a textarea — three traps paid for

Paid for the same day (PLAN-COMPOSITION-HTML, e2e on 2026-08-20):

1. **Playwright's `fill('')` is a no-op** on it — an empty-string
   `insertText` does not clear the selection in Chromium. Clear it
   the way a user would: Ctrl+A then Delete. And `fill(text)` writes
   into whichever element is **focused** at the moment of insertion
   (not atomic like on an input): any programmed focus
   pre-positioning can hijack the keystrokes into another field — the
   guard "a focus already set wins" is product, not test.
2. **Keyboard routers do not see it**: a guard of `instanceof
   HTMLInputElement || HTMLTextAreaElement` lets its keystrokes leak
   into global shortcuts (Delete deleted the conversation while
   typing). Add `isContentEditable` to every input-detection check.
3. **Its re-serialization is never faithful**: reading back the
   `innerHTML` of content you just placed there returns normalized
   styles and entities — any "identical content" check that compares
   against the stored value fires wrongly (churn). Absent user
   keystrokes, re-emit the stored values, never the DOM.

---

### An audit remedy is admitted on a measurement, not on its apparent obviousness

Two remedies from the 2026-09-01 audit seemed obvious and were
REJECTED once measured: search's "COUNT per keystroke" was worth
1.5 ms out of 57 (the cost is the date-sorted page — the proposed
bound saved 1 ms), and `withGlobalTauri: false` protected nothing
(`__TAURI_INTERNALS__.invoke` is still injected into every window —
the CSP is the boundary). Conversely, the variance of a Cleanup
verdict (35 to 580 ms for the same 40 messages) was not a scan
needing an index but FTS5 segment merging on delete. **Every batch
carries its own before/after measurement; a measurement that does not
move withdraws the remedy** (PLAN-AUDIT-V2 E2, E4, E8).

### An opening click reaches the listener its own effect just set

A menu that sets a `click` listener on the window inside a `$effect`
sets it WHILE the click that opened it is still propagating (the
effect runs at microtask time, before the click reaches `window`):
the menu closes instantly. The List's `stopPropagation` masked it;
the Feed and the thread did not have one. Deferring the arming to a
macrotask misses the next keystroke (a race with Playwright); the
rule that holds: **a click on the trigger is never "outside"**
(PLAN-AUDIT-V2 E11, `Menu.svelte`).

### A review fix is an increment like any other

At PLAN-AUDIT-V2, the fresh-eyes review replaced a `</div>` search
with a count of nested blocks — validated by an ASCII unit test and
by only the "flaky" specs replayed. The final gate came back red: the
real body said "transféré", the loop advanced byte by byte with
`str[i..]`, the index landed inside an "é", and the function
PANICKED. Two rules in one:

- **Advance over bytes (`as_bytes()`), never over a `str` indexed
  byte by byte** — `body.rs` advances by character, which is the only
  other admitted form. A test string with no accents proves nothing
  about a path that will see French.
- **A pure decision called from an async command runs under
  `hors_pompe`**: `spawn_blocking` reports a panic as a stated error;
  bare in the task, it leaves the invoke without a response and the
  UI frozen without a word — no toast, no trace, no test that names
  it.

And the method rule: a review fix replays the spec of the path it
touches, not just the specs being watched.

## 10. File map

| File | Role |
|---|---|
| [`docs/STATE.md`](STATE.md) | The handoff snapshot — current state, rewritten with every job |
| [`docs/PLAN.md`](PLAN.md) | Concept paper — product source of truth |
| [`docs/adr/`](adr/) | The 15 frozen decisions |
| [`docs/archives/`](archives/) | Closed-out plans and phase closing reviews |
| [`crates/mail-core/src/store.rs`](../crates/mail-core/src/store.rs) | SQLite storage (WAL), schema, migrations, unified mailbox, grouping scope |
| [`crates/mail-core/src/sync.rs`](../crates/mail-core/src/sync.rs) | Sync engine + `sync_order`, `sync_percent`, `disk_shortfall` |
| [`crates/mail-core/src/thread.rs`](../crates/mail-core/src/thread.rs) | Conversations: pure union-find + persistence, account scope |
| [`crates/mail-core/src/drafts.rs`](../crates/mail-core/src/drafts.rs) | Drafts: push, pull, edit conflict |
| [`crates/mail-core/src/outbox.rs`](../crates/mail-core/src/outbox.rs) | Outbox + golden rules |
| [`crates/mail-core/src/search.rs`](../crates/mail-core/src/search.rs) | Contentless, transactional FTS5 index |
| [`crates/mail-core/src/backfill.rs`](../crates/mail-core/src/backfill.rs) | Body backfill AND header pass — `NO_HORIZON` since ADR 0010 |
| [`crates/mail-core/src/test_support.rs`](../crates/mail-core/src/test_support.rs) | `FakeServer` — replays field oddities |
| [`crates/mail-core/examples/`](../crates/mail-core/examples/) | 3 diagnostics + 3 benchmarks + `seed_inbox` |
| [`crates/mail-imap/src/convert.rs`](../crates/mail-imap/src/convert.rs) | IMAP → domain translation; archive and sent discovery; calendar extraction (`extract_ics`) |
| [`crates/mail-ical/src/lib.rs`](../crates/mail-ical/src/lib.rs) | iCalendar/iTIP invitations: parser + REPLY generator, pure (ADR 0024) — spike corpus as tests |
| [`crates/mail-auth/src/provider.rs`](../crates/mail-auth/src/provider.rs) | OAuth providers described **as data** |
| [`apps/desktop/src/commands.rs`](../apps/desktop/src/commands.rs) | Tauri commands (IPC), all-mailboxes loop, disk guard, progress |
| [`apps/desktop/ui-v2/src/App.svelte`](../apps/desktop/ui-v2/src/App.svelte) | The UI (Svelte 5, sole framework since B2/PLAN-RETRAIT-V1): screens 01-04, notice slot, automatic sync cycle |
| [`e2e/README.md`](../e2e/README.md) | Deterministic E2E harness (CDP) |
| [`scripts/make-release.ps1`](../scripts/make-release.ps1) | **All** of the release (ADR 0013, dual-arch ADR 0023): bump, two signed builds arm64 + x64 (all-or-nothing), two-platform BOM-free `latest.json`, commit + push + Latest Release at the bare tag |
| [`scripts/verify-release.ps1`](../scripts/verify-release.ps1) | The §2.10 verification, scripted — 5 named assets, BOM, two platform keys, signatures == `.sig` and distinct, URLs that resolve |
| [`crates/mail-core/src/crash.rs`](../crates/mail-core/src/crash.rs) | PURE redaction of a crash report — discards the message (PII) (ADR 0014) |
| [`apps/desktop/src/telemetry.rs`](../apps/desktop/src/telemetry.rs) | Panic hook, file-based consent, local report write (ADR 0014) |
| [`spikes/ui-socle-v2/`](../spikes/ui-socle-v2/RAPPORT.md) | Tie-breaking spike for the UI v2 foundation — evidence for ADR 0015, **throwaway** |

---

*Your mail, instantly. Performance and reliability are not options —
they are the features.*

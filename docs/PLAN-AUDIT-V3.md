# PLAN-AUDIT-V3 — audit wave 3, architecture

> Statement: deliver wave 3 of [AUDIT-2026-09-01.md](AUDIT-2026-09-01.md)
> §5 — the seven architecture jobs (3.1–3.7), each carrying its ADR
> where the audit marks one. Opened 2026-09-04. Status: **draft,
> awaiting STOP 1**.

---

## 1. Finding — the code on 2026-09-04 (post wave 2, post English switch)

Reconnaissance replayed on the current tree (the audit's French names
are stale since PLAN-ENGLISH-SWITCH). All seven items still stand;
none was absorbed by waves 1–2. Measured:

### 1.1 Poll orchestration lives in the shell (audit 3.1)

There is no `releve.rs`; everything sits in
`apps/desktop/src/commands.rs` (**7,225 lines**): `run_sync`
(commands.rs:1233, ~320 lines), `poll_inbox` (:711), `body_view` /
`fetch_body` (:2096, :2157), `invitation_view` (:2283), and a dozen-plus
pure helpers (`must_poll` :649, `settle_marker` :685,
`wait_after_failures` :840, `note_outcome` :951, `body_horizon` :3015,
`without_condstore_first_time` :2643…). The watcher
(`apps/desktop/src/watcher.rs`, 207 lines) imports `crate::commands`
(watcher.rs:25) and calls seven of its symbols — the poll policy cannot
be tested without the Tauri shell.

### 1.2 The cadence is owned by the UI (audit 3.2)

Scheduling is JS `setInterval` in `App.svelte` (**2,161 lines**):
`runSyncCycle`/light pass at App.svelte:1052-1053, `probeState` at 5 s
(:1032), `probeDrafts` at 10 s (:1041), plus timers at :908, :956,
:1035, :1060. The shell's tokio usage is 18 `spawn_blocking` sites in
`commands.rs`, per-command offloading only — no shell-side loop. The
audit's premise shifted (it said "tokio scheduler on the shell side" as
the *remedy*; the disease is now "JS setInterval scheduler"): if the
UI is closed to a tray or busy, no cadence exists but the watcher's.
`sync_progress` is a command read at App.svelte:548; the D-52 item 3
`list_drafts` whole-list 10 s poll rides these timers.

### 1.3 `store.rs` is one file of 10,568 lines (audit 3.3)

`crates/mail-core/src/store.rs`: `mod tests` from line 5216 to EOF —
**≈5,352 test lines (51 %)**, ≈5,216 of code. The `SCHEMA` +
`user_version` fast door is the intended architecture (store.rs:24,
:824, `migrate` :4217) and stays. Screener, Cleanup, prefs, SQL all
inline (`portier_attente` :404, `nettoyage_session` :413 — table names
stay French per D-55; `CleanupSession` :744, `screener_defaults`
:3164, `set_text_prefs` :1134).

### 1.4 `commands.rs` boilerplate (audit 3.4)

- `Store::open(&db_path(&app)?).map_err(|err| err.to_string())?`
  repeated **108 times**; no `with_store` helper.
- **147** signatures return `Result<_, String>`; no typed error.
- `sync_inbox` (:981, ~110 l.) and `sync_inbox_light` (:1092, ~119 l.)
  are near-parallel account loops with separate `SyncShared`
  bookkeeping.
- `queue_send` (:4176) takes flat `to/cc/bcc/…` strings, not a
  `DraftContent`.
- Four pref families keyed in the shell (`PREF_ARRIVAL_BUBBLES` :1629,
  `PREF_LANG` :1630, `PREF_LAST_SYNC` :1633 — stored key still
  `"derniere_synchro"`, D-55 — screener defaults :3166-3193); **D-34
  still open** (DEBT.md:707).
- The nine D-49 cleanliness items (DEBT.md:287) live in these files.

### 1.5 The front (audit 3.5)

`App.svelte` 2,161 l. with **11 `bind:this`** (D-48's `generation`
store not built); `List.svelte` 1,808; `Compose.svelte` 1,704;
`Settings.svelte` 1,613. No `lib/commands.js`; theming is plain
`lib/theme.js` (186 l.); `.btn` appears at 20 sites (D-47 core half
open). **Eight** `__e2e*` seams compiled into production, none behind
`import.meta.env` (`transport.js`: `__e2eHold`, `__e2eLog`, `__e2eAdd`,
`__e2eFailure`, `__e2eAttachments`, `__e2eDestination`; `links.js`:
`__e2eLinks`; `onboarding.js`: `__e2eOnboarding`) — the audit counted
five (D-52 item 8); the count **grew** since.

### 1.6 Two TLS stacks (audit 3.6)

IMAP = `native-tls` (mail-imap/Cargo.toml:13, workspace Cargo.toml:47);
SMTP = rustls via `lettre` `rustls-tls` (Cargo.toml:42); OAuth = rustls
via `oauth2` `reqwest-blocking`+`rustls-tls` (Cargo.toml:48); the
updater uses `rustls-platform-verifier`. Audit fact: a corporate CA in
the Windows store works in IMAP and fails in SMTP/OAuth. **Chief-Engineer decision
#6 of the audit — still unmade**; no ADR exists.

### 1.7 `MailServer` trait (audit 3.7)

`crates/mail-core/src/remote.rs:181-310`. No separate
`capabilities()`; `Folder` (remote.rs:106-119) carries
`special_use: Option<SpecialUse>` (wave 2, E5) but **no `delimiter`**;
`fetch_body_html` (:200) and `fetch_recipients` (:251) still on the
trait; attachments are index-based (`fetch_attachment` :242,
`attachment_bytes` convert.rs:598) but **D-30 stays open**
(DEBT.md:647 — legacy invitations without an attachment row).

### 1.8 The two audit Chief-Engineer decisions still open

#6 (one TLS stack) and #7 (confirm the Google OAuth client is the
"Desktop" type — ADR 0025 only *assumes* it). Neither is recorded
anywhere in STATE, DEBT or `docs/adr/`.

---

## 2. Scope

Everything in §4's steps, and nothing else. Wave 3 is
**behavior-neutral by contract**: no visible feature, no vocabulary
change, no schema change (D-55 holds — French SQL/prefs keys stay).
The proof of neutrality is the existing net: 433 mail-core tests, the
full e2e suite, the contract nets (IPC, DOM, language ratchet) must
pass **unchanged** at every step — a step that needs to rewrite a
test's expectation must say why in its commit.

Closures aimed at: **D-30** (stable MIME index), **D-48**
(`generation` store), the **D-47 core half** (`.btn`), **D-49** (in
passing, in the files each step reorganizes), **D-52 items 3 and 8**
(`list_drafts` poll folded into `etat_ui`'s cadence; the e2e seams
gated), **D-34** (pref keys to the core).

Beta wave 1 stays open: a tester report interrupts this plan and goes
first in `/field` on `main` (STATE). Steps are sized so `main` is
releasable after every commit.

---

## 3. Options — settled on facts

### 3.1 One plan or seven

The audit says "one job each, with an ADR". Wave 2's precedent (D1,
"all in one job") ran ten batches under one plan, one commit per
batch, and closed in a day. The seven jobs here are coupled: 3.4's
`with_store`/typed-error rewrite touches the same 7,225 lines 3.1
carves up; 3.2 (scheduler) presupposes 3.1 (poll policy callable from
the shell without the UI); 3.3 (store split) is where 3.1's core-side
code lands. Sequencing them as one plan avoids re-opening the same
files seven times. → proposed: **one plan, E1-E7, one commit (or few)
per step, each ADR written at its step** (D1).

### 3.2 Order of attack

Dependency-driven, small and decidable first:

1. **E1 TLS** (P) — one Cargo surface, needs D2; done first so every
   later gate replays it.
2. **E2 store split** (G) — mechanical, pure moves; gives 3.1's code a
   destination.
3. **E3 commands.rs** (M) — `with_store` + typed error shrink the file
   before E4 carves it.
4. **E4 poll policy → core** (G, ADR) — depends on E2/E3.
5. **E5 scheduler → shell** (G, ADR) — depends on E4.
6. **E6 MailServer trait** (M, ADR) — independent, after the core has
   settled.
7. **E7 front** (G) — independent; last because E5 already rewrites
   App.svelte's timer block and doing E7 first would be re-done.

### 3.3 TLS: which stack (feeds D2)

Facts: `native-tls` on Windows = SChannel = the Windows certificate
store (corporate CA works); plain `rustls-tls` in lettre/oauth2 =
webpki roots (corporate CA fails); the updater already ships
`rustls-platform-verifier` (rustls with the Windows store as verifier —
both properties at once). Three candidates:

| Option | Corporate CA | Stacks in the binary | Cost |
|---|---|---|---|
| A. native-tls everywhere | yes | 1 (+ updater's) | lettre/oauth2 feature flip; oauth2's reqwest on native-tls |
| B. rustls + platform-verifier everywhere | yes | 1 | wiring the verifier into lettre and oauth2's reqwest — feature support to prove by spike |
| C. status quo | IMAP only | 3 | zero — and the audit's finding stays |

B matches the updater and removes OpenSSL-adjacent C code from the
supply chain; its unknown is whether lettre and oauth2 expose a
connector hook for the platform verifier — **a one-hour spike proves
or kills it** before the step is written. A is the certain fallback.
→ proposed: **spike B; if the hook exists, B, else A** (D2).

### 3.4 Scheduler ownership (feeds D4)

The audit's remedy ("tokio scheduler on the shell side; the UI keeps
`sync_progress` + the button") predates the finding that the cadence is
wholly in JS. Moving it shell-side means: tokio interval tasks own the
full cycle / light pass / state probe cadence, the UI subscribes
(event or probe) and keeps the manual button. Gains: cadence survives
a hidden window; the D-52 item 3 drafts poll dies with the other
timers. Risk: the pump discipline (`off_pump`, main-thread guard) must
hold from a task context — the guard net exists and is proven. The
alternative (keep JS timers, only coalesce) keeps ~250 l. of timer
code and the audit finding open. → proposed: **shell-side, as the
audit says** (D4).

---

## 4. Steps

Every step: its own commit(s) on `main`, full `/gate` green before the
commit, behavior-neutral (§2), fresh-eyes review before the final
commit of the wave. ADRs at E1 (TLS), E4 (poll policy), E5
(scheduler), E6 (trait).

### E1 — one TLS stack (P; spike + code)

Per D2. Spike first (`spikes/tls-stack/`, throw-away): prove the
platform-verifier hook in lettre and oauth2, or fall back to
native-tls. Then: one workspace TLS dependency, ADR written, the three
mail crates' suites green. Field: both real accounts (Gmail OAuth +
the other) connect, send, poll.

**Delivered 2026-09-04.** Spike verdict: **B feasible as-is** at the
pinned versions, 9/9 live handshakes on the Windows store. Swap: lettre
on features `rustls`+`ring`+`rustls-platform-verifier`; oauth2's
reqwest via `use_preconfigured_tls`; mail-imap's `tls_stream()`
(handshake completed at connect; STARTTLS path unchanged);
`native-tls` out of the workspace; ring sole provider (the spike
measured the dual-provider panic). Net
`the_workspace_ships_one_tls_stack` proven RED before the swap.
**ADR 0032**; D3's "Desktop app" confirmation recorded in ADR 0025.
Full gate green (13 steps, 203 e2e, flaky 0) after two andons: a fmt
wrap, and the Chief-Engineer abbreviation in the ADR 0025 note counting as a French marker (the scanner reads the two-letter abbreviation as a French function word).

### E2 — `store.rs` split into submodules (G)

`tests.rs` out first (−51 %), then `migrations`, `screener`,
`cleanup`, `prefs`, `sql`; `SCHEMA` stays the single fresh-database
source of truth, `user_version` the single gate; the pure decisions of
`upsert_envelopes` extracted. Pure moves — `cargo test -p mail-core`
count identical (451 at measurement, not the plan draft's stale 433),
zero test rewritten beyond `use` paths.

**Delivered 2026-09-04.** `store.rs` 10,568 → 2,890 lines; `tests.rs`
5,421 (the module body moved verbatim, fmt-dedented); `migrations` 860,
`screener` 611, `cleanup` 399, `sql` 369, `prefs` 156. Blanket
`pub(crate) use` re-exports at the store root keep every external path
(`nav.rs`, `search.rs`, `backfill.rs`, `thread.rs` untouched);
`SCHEMA` and `write_invitation` stayed in `store.rs` (shared);
visibility bumps to `pub(super)` only. Extraction by a Sonnet agent
under five oracles — suite EXACTLY 451/0/2, clippy zero, fmt,
per-name item conservation, diff confined to the store files. Then the
three pure decisions of `upsert_envelopes` extracted RED-first
(`needs_reindex`, `no_rule_action`, `arrived_after_verdict`): suite
451 → 454. Full gate green (203 e2e, flaky 0). Two ratchet lessons
paid: the scanner reads `git ls-files`, so a gate played before
`git add` of new files lies on that step; and a file SPLIT
redistributes per-file baselines — `--update` legitimate only because
the total fell (1991 → 1974).

### E3 — `commands.rs` de-boilerplated (M)

`with_store` (108 sites), typed `CommandError` implementing
`Into<String>` at the Tauri boundary (147 signatures — the wire keeps
strings, the IPC contract net must stay green), the account loop
unified (`sync_inbox`/`sync_inbox_light` share one cycle driver),
`mailbox_id()` resolved core-side, `queue_send(DraftContent)`, the
four pref families' keys declared in the core (D-34). D-49 items fixed
in passing where the line is already under the pen (`verrou_repris`…
now under their English names).

**Delivered 2026-09-04** (`a850cfd`, CI 33870746895). `CommandError`
in `fault.rs` (104 signatures; the wire unchanged — serialized as its
message, and no error substring is matched by UI or e2e);
`with_store`/`store_off_pump` (open sites 93 → 21, deliberate fusions
kept); the twins share ONE `poll_cycle` + `CycleTally`; `queue_send`
packs `SendContent` at the boundary (flat IPC keys kept); D-34 closed
(the three global pref keys in the core); D-49 partial: `recovered()`
replaces twelve poison matches — left open: the
reply/forward-context triple `off_pump`, `compose()` without
references (E4's neighborhood). Andon story: three local gates red on
three DISJOINT e2e sets, every red green in isolation (the machine's
engraved WebView2/OneDrive flake; the -1 silent externals) — no
bypass, the pre-push gate decided: green, 203 e2e, flaky 0. Root of
the "asset not found" reds found by the parallel session: a live Wind
window locks the exe and cargo silently replays a stale binary — its
guard (`frontendFailure` in `launch.mjs`) now names it.

### E4 — poll policy into the core (G, ADR "poll policy lives in the core")

`run_sync`, `poll_inbox`, the pure helpers of §1.1 →
`mail-core` (new `sync_account`/`poll` module, landing in E2's layout);
`watcher.rs` loses `use crate::commands` — it calls the core. The
shell keeps I/O wiring only. RED first: the pure functions get direct
unit tests as they land (they had none reachable outside the shell).

### E5 — the scheduler on the shell side (G, ADR)

Per D4. Tokio interval tasks own full cycle, light pass, state probe;
`list_drafts` polling folds into the single `etat_ui` probe (D-52
item 3); the UI keeps `sync_progress` reads + the manual sync button;
App.svelte's timer block (~250 l. with its state) dies.
`main-thread-guard.mjs` and `freeze-probe.py` are the nets — the probe
played OUTSIDE the gate (memory: never during a gate).

### E6 — `MailServer` trait cleaned (M, ADR)

`capabilities()` separated; `Folder` gains `delimiter`;
`fetch_body_html`/`fetch_recipients` removed (their callers move to
the composed fetch paths); the stable MIME part index finished and
**D-30 closed** (the legacy-invitation gap gets its repair or an
explicit kept-debt line).

### E7 — the front (G)

`generation` store (11 `bind:this` die, **D-48 closed**);
`lib/commands.js` (one `invoke` doorway); global `.btn` (D-47 core
half); `Editor.svelte` extracted from Compose; `theme.svelte.js`;
App/List/Compose/Settings each split along their existing seams; the
**8** `__e2e*` seams behind `import.meta.env` (D-52 item 8) — the e2e
build keeps them, the release build loses them; proven by grepping the
built bundle both ways. DC-D2: any UI-visible drift is a defect, not
an amendment — the System should need **no** A-n from this step.

### Estimate

E1 P, E3+E6 M, E2+E4+E5+E7 G — the audit's own sizing. Wave 2 (ten M
batches) closed in a day with parallel agents; wave 3's G steps are
bigger and serialized by dependency (§3.2). Expect **2–3 sessions**;
`main` releasable after every step.

---

## 5. Explicit refusals (§2.6)

- **No behavior change, no new feature** rides this wave — anything a
  tester would notice belongs to `/field` or its own plan.
- **No schema or stored-key rename** (D-55 stands; `"derniere_synchro"`
  keeps its key even as its constant moves to the core).
- **No D-53 work** (Feed RAM / iframe-per-card): it is an architecture
  subject but it changes rendering — its own job.
- **No `svelte-check` adoption** (rejected at E1 of the switch — 1,059
  pre-existing errors); the front step leans on the existing nets.
- **The archives and the v1 System stay frozen** (D-58).

## 6. Chief-Engineer decisions — to be settled one by one at STOP 1

| # | Question | Proposal | Answer |
|---|---|---|---|
| D1 | Scope: one plan E1-E7 in the §3.2 order, or a subset now (e.g. E1-E5 core only, front later), or one plan per job as the audit wrote? | One plan, E1-E7, §3.2 order | **2026-09-04: "One plan, E1-E7"** |
| D2 | TLS (audit decision #6): spike rustls+platform-verifier, fall back to native-tls everywhere if the hook is missing? | Spike B, fallback A | **2026-09-04: "Spike B, fallback A"** |
| D3 | OAuth (audit decision #7): the Chief Engineer confirms in the Google console that the client is the **Desktop** type — recorded here and in ADR 0025 when done | Chief-Engineer action | **2026-09-04: "Desktop app confirmed"** — checked in the Google console; audit decision #7 closed |
| D4 | Scheduler ownership: shell-side tokio cadence (UI keeps the button + progress), per §3.4? | Yes | **2026-09-04: "Shell-side tokio"** |
| D5 | Release: wave 3 is behavior-neutral — publish a release at the end (PATCH per §2.9, nothing user-visible) or let it ride with the next feature release? | Ride with the next release | **2026-09-04: "Ride with next release"** |
| D6 | Field cadence: one STOP 2 at the end, or an intermediate field pass after E5 (the scheduler is the riskiest behavioral seam)? | Intermediate after E5 + final | **2026-09-04: "Intermediate after E5 + final"** |

## 7. Field checklist (STOP 2) — what the Chief Engineer plays

At minimum (final pass; the E5 intermediate replays 1-4):

1. `scripts\field.ps1` then `scripts\run-wind.ps1` — launch with trace.
2. Both accounts connect (E1 TLS); send one mail from each; it lands.
3. Arrival cadence: a mail sent from the phone appears within the full
   cycle; the light pass ticks in `wind.log` (E5 — cadence now
   shell-side).
4. `freeze-probe.py` outside any gate: 0 freeze > 150 ms.
5. Screener, Cleanup, Settings, Feed, Paper trail each opened once
   (E2/E7 moved their code — nothing may look different).
6. Drafts: type, wait 10 s, the draft is on the server (E5 folded the
   drafts poll).
7. Attachments on an old invitation mail (E6, D-30).
8. RAM at rest ≤ the STATE figure; `wind.log` bounded, no PII.

## 8. Named risks

- **A refactor of this size during an open beta**: mitigated by
  releasable-`main`-per-step and D6's intermediate field pass; a
  tester report preempts the wave.
- **The typed-error rewrite touching the IPC wire**: the contract net
  is the tripwire; the wire keeps `String`.
- **E5 and the pump**: any `Store::open` from a task context outside
  `off_pump` — the main-thread guard is proven red on exactly this.
- **The seams behind `import.meta.env`**: the e2e suite must still
  find them — proven by running one seam-dependent spec against the
  gated build before trusting the wave.
- **Parallel-agent translation-era lesson** (English switch): whole-file
  mechanical moves get a structure oracle (test count, symbol
  inventory) before and after.

## 9. Expected debt

- D-30, D-48, D-34, D-52(3, 8), D-47 core half, D-49: **closed or
  explicitly re-scoped** by their steps.
- New debt only if E6's legacy-invitation gap is kept rather than
  repaired (then a D-n with its reopening condition).

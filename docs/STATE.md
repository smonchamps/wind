# State — Wind's handover snapshot

> **This document is rewritten at every job — that is its function.**
> Version delivered, next job, field figures, open trade-offs,
> deferrals: everything volatile lives here. The method, the
> invariants and the traps live in [STANDARD.md](STANDARD.md) —
> they are not rewritten, they are amended.
>
> Extracted from PASSATION.md (§1 + §8) on 2026-08-19 — PLAN-DOCUMENTATION.

---

## Where things stand, and what to do first

🔧 **Job in progress: [PLAN-BASCULE-ANGLAIS](PLAN-BASCULE-ANGLAIS.md)** — switch all the code and all the documentation from French to English (CE command). Plan written and measured on 2026-09-02 (~375 000 words of prose, ~1 200 definitions, ~115 files to rename; layer by layer, one commit per layer, three new nets). **STOP 1 played on 2026-09-02, fourteen decisions settled** (D1 archives frozen, D3 French SQL kept → debt D-54, D4 English by default → next release MINOR, D9 normative documents read in full by the CE, D11 `BETA.fr.md` kept, D14 glossary validated by the CE), **GO from the CE on 2026-09-02** after the publication of 0.16.0 and 0.17.0 and the close-out of AUDIT-V2. **E0 delivered and validated on 2026-09-02** ([GLOSSARY.md](GLOSSARY.md) « Validé tel quel », `scripts/rename/`: 1 210 identifiers, 480 keys, 542 DOM ids derived from `tokens.csv`). **E1 delivered on 2026-09-02**: four nets proven by breaking them — the language ratchet (142 113 French markers at baseline, `e2e/language-baseline.json`, any rise = red), the IPC contract (`queue_send` invisible until then), markdown links, eslint `no-undef` (set-based: svelte-check rejected at 1 059 pre-existing errors); a 13-step gate. **E2 delivered on 2026-09-02**: scripts renamed and rewritten in English (`make-release.ps1`, `verify-release.ps1`, `run-wind.ps1`, `install-workstation.ps1`, `build-wind.mjs`, `measure-sessions.mjs`, `field.ps1`, `make-icon.ps1`), skills `/job` `/field` `/close` `/gate`, agent, CLAUDE.md, WORKFLOW.md, gate.ps1, hook, CI; STANDARD §2.8 amended — everything new is written in English from here on. **E3a delivered on 2026-09-02**: `mail-ical`, `mail-render`, `mail-smtp`, `mail-auth` in English (3 476 lines, 96 tests green, mail-ical public API renamed with its two dependents). **E3b delivered on 2026-09-02**: `mail-imap` in English (3 398 lines, 79 tests green, `Veille`/`veiller` → `Watch`/`watch` with the shell's watcher updated). **E3c delivered on 2026-09-02, in three commits**: `store.rs` (10 504 lines, 140 tests — split in seven chunks translated in parallel by Sonnet agents against a fixed rename table, reassembled, dependents updated by a string-literal-aware whole-identifier pass), then `sync`/`thread`/`search`/`backfill` (4 822 lines, 120 tests), then the rest of the crate and its 13 examples renamed (`correspondants.rs` → `contacts.rs`, `bench_*`, `diag_*`, `seed_arrival`). Every Rust crate is now in English; `mail-core` keeps 124 French markers, all deliberate (`lang:fr`: notification texts, quoting and forward labels composed into bodies, the size units `o`/`Ko`/`Mo` the e2e specs assert, the French fixtures that ARE the test). Baseline 134 436 → ~110 100. Two traps caught by the nets and fixed the same hour: the mechanical rename reached the shell — 14 Tauri command names (IPC contract) and the serialized `dernier_epoch`/`dernier_objet` payload fields the UI sorts on (e2e `feedback-14`) — both reverted, they belong to E4. **E4 delivered on 2026-09-03**: the shell (`apps/desktop/src`, 8 552 lines) in English — the 36 command names of GLOSSARY §5.3 renamed together with the UI `appel()` calls, the specs' `invoke()` calls and the two e2e tools that name commands; `veilleur.rs` → `watcher.rs`, `demenagement.rs` → `relocation.rs`; `hors_pompe` → `off_pump` with the main-thread guard's literal. Kept for E5 on purpose, because they are the IPC contract with the UI: the command PARAMETER names (JSON keys) and the serialized payload FIELDS; the two native dialogs stay French (`lang:fr`). Shell: 33 French markers, all deliberate. Baseline → 102 688. **Field validated on 2026-09-03 (E3c + E4, no finding).** **E5 STOP 1 played on 2026-09-03** (D15 DOM contract at E5d, D16 value vocabularies mapped at the shell boundary in `wire.rs` — the database keeps the French value, the wire carries the English one —, D17 shell-composed text kept French, debt D-56). **E5a delivered on 2026-09-03**: the IPC keys (18 parameters, ~45 fields) and the five value vocabularies in English on the wire, `--rep-*` → `--mk-*` (A110); `d384724`, CI green 33742728494, **field validated on 2026-09-03, no finding**. **E5b delivered on 2026-09-03**: the UI in English — 38 files renamed, ~850 identifiers (the E0 dictionary plus 282 definitions it had missed, found by seven read-only Sonnet agents), the 518 catalogue keys and the `{placeholders}` of the catalogue values, ~1 000 comment blocks translated by eight Sonnet agents under a mechanical oracle (files stripped of comments byte-identical to the snapshot: 0 code difference); **D4 applied** (English is the reference: `lib/language.js`, `text.svelte.js` falls back to EN, `Lang::from_pref` defaults to `En`, ADR 0016 amended, A111 — **the next release is MINOR**); two persisted UI preferences migrate at read without a reset (`wind-largeurs` key `liste` → `list`, `wind-espacement` `faible|moyen|eleve` → `low|medium|high`); the applier is committed (`scripts/rename/apply-ui.mjs`, a tokenizer with a `--report` mode, GLOSSARY §6). Baseline 102 638 → 87 926; the UI keeps 96 markers, all deliberate. Fresh-eyes review: ten findings, eight fixed (a legacy map the applier had rewritten, a duplicate catalogue key, two nets that matched nothing since E5a, the chip tone classes, D4 now proven in a real launch, a placeholders net), two deferred to E5d (`line` → `row`, the handle test id). Lesson: the catalogue-value `{placeholders}` are a bridge too (a param key renamed on one side rendered empty names, organizers and dates — nine specs caught it); an e2e wave played while passes are still running is worth nothing. Full gate green in 167 s (198 e2e, flaky 0); commit `59c6ee1`, CI green 33759685493; **field validated on 2026-09-03, no finding** (ten steps OK, trace clean). **E5c delivered on 2026-09-03**: `--brand`, `--r-control`, `--r-tile`, `--tile`/`--tileInk` in the components, `system.css` (renamed), the System (A112, contract table cells included) and the three nets; the four sort glyphs renamed `sort_*`; nothing visible changes; full gate green in 145 s (198 e2e, flaky 0); commit `29c6a68`, CI green 33765915506, **field validated on 2026-09-03, no finding**. **E5d delivered on 2026-09-03**: the DOM contract in English from one table — `dom.csv` completed 539 → 654 rows by `scripts/rename/derive-dom.mjs` (the E0 inventory had missed 124 classes and the `data-*` attribute names), 49 words entered in `tokens.csv` (D20 + five listed in the plan's annex for STOP 2), 22 E0 rows corrected; the applier `scripts/rename/apply-dom.mjs` (RED then GREEN on fixtures, `e2e/apply-dom.test.mjs`) applied to 67 files in one run — the components, `system.css`, 30 specs, 5 e2e tools; `line` → `row` for the list row (D18, `listRow` snippet, `thread.row`); the two test-id collisions split (D19: `write`/`compose`, `desk-continue`/`onboarding-continue`); the 12 `data-*` names (D21: `data-hue`, `data-category`…); the eight seams (`__e2eHold`, `__e2eLog`, `__e2eRelease`…); System A113. Seven spec-side reds on the first e2e wave (selector forms the pass did not reach — each a rule now), ten review findings fixed (one field-visible: the cancelled-invitation chip's alert ink), a permanent net added (`e2e/dom-contract.test.mjs`: every id a spec selects is rendered by the UI). Full gate green in 150 s (198 e2e, flaky 0); baseline 87 925 → 87 900; commit `2c30cea`, CI green 33781288186, **field validated on 2026-09-03, no finding** (ten screens, both themes; the five words added after D20 validated). **E6 STOP 1 played on 2026-09-03** (D22 the suite runs in English — the D4 default, D23 the 201 titles translated, D24 the e2e README rewritten, D25 the living docs' path pointers updated names-only, D26 the four names the glossary lacked). **E6a delivered on 2026-09-03**: the 30 specs and 13 tools renamed (`redesign-screen02.spec.js`, `contrast.mjs`, `system-coherence.mjs`, `main-thread-guard.mjs`, `tokens.mjs`, `freeze-probe.py`, `measure-*`…), identifiers, comments and titles in English (the applier `scripts/rename/apply-e2e.mjs` on the shared scanner `scripts/rename/lib.mjs`, ten Sonnet agents under a token-level oracle), the 21 dependents and living docs re-pointed, System A114; baseline 87 900 → 77 283 (the layer 10 981 → 364, all E6b anchors); gate green 209 s (196 e2e, 2 flaky), review twelve candidates / ten fixed (three `file:line` pointers had drifted by one line — a comment pass moves lines). **E6b delivered on 2026-09-03**: the suite launches in English by default (D22), ~200 anchors rewritten from the catalogues, 95 fixture lines `lang:fr`, two French-only tests added to `redesign-language.spec.js` (the R3 short name, the plural), `capture-onboarding.mjs` pinned French; the e2e layer at **0 French markers**, baseline → 76 919; one field-visible symptom found by the wave: the compose weight reads `2.8 Mo / 25 MB` (shell unit vs catalogue limit, debt D-56). **Field (STOP 2) on 2026-09-03: E6 validated by the Chief Engineer, no finding** (benches under their new names, `freeze-probe` 0 freeze > 150 ms, `measure-v2` page p50 15.1 ms, gate green 157 s); **D27 applied** (`MEASURE_DB`, `MEASURE_ACCOUNTS`, `MEASURE_REUSE`, `MEASURE_NO_ACTIVITY`), **D28 applied** (the French sweep: three more tests in `redesign-language.spec.js`; rule: every screenshot in the language the user chose → debt D-57, the onboarding illustrations at the next onboarding job). CI green 33802706071 (E6a) and 33806065399 (E6b). **Next: E7** (the living documents, in this order, one commit each: README, STANDARD, WORKFLOW, STATE, DETTE, PLAN, BETA, AUDIT, PASSATION, CHANGELOG, the 31 ADR, the PLAN-*.md — the closed ones to `archives/` first). **E5, the UI** (`apps/desktop/ui-v2/src`: 25 components and 24 `lib/` modules renamed per GLOSSARY §5.1 — on NTFS a case-only rename needs two `git mv`; identifiers, comments; the catalogue KEYS in `catalogue.fr.js`, `catalogue.en.js` and the 569 `t()` calls per §5.4 with `redesign-language.spec.js` as the oracle; English becomes the reference and the fallback (D4, ADR 0016 amended, `Lang::from_pref` in `notify.rs` follows); the 15 CSS tokens of §5.5 in `systeme.css` + `theme.js` + `systeme.dc.html` in the same commit (DC-D2)). Handover, in order: (1) read this paragraph, GLOSSARY §2, §4, §5.1, §5.4–§5.6 and `scripts/rename/dictionary.csv` rows `layer=ui`, `keys.csv`, `dom.csv` — the words are decided; (2) the E4 leftovers come first, in ONE commit with their UI side: the shell's command parameter names and payload field names (`#[derive(Serialize)]` structs of `commands.rs`, some `rename_all = "camelCase"`) change with every Svelte/JS read and every `appel('…', { … })` argument object — the IPC contract net does not see keys, only the e2e and the field do, so play the whole e2e wave before the commit; (3) the DOM contract (`data-testid`, CSS classes, `__e2e*` seams, §5.6) changes with the specs — either at E5 with E6 in the same commit, or kept French until E6: decide at the start and write it in the PLAN; (4) the French UI text (`catalogue.fr.js` VALUES) does not change, only the keys (D3/§1.6); the browser `localStorage` keys stay (D-54); (5) method: copies in the scratchpad, one agent per component, a fixed table for keys/ids/seams, `apply-renames` passes; the Vite build, `eslint no-undef`, `catalogs.test.mjs`, `system-coherence` and the e2e are the oracles; (6) open CE points: the language of shell-composed text (`human_size` units, the two native dialogs) once the UI language is known to the shell; (7) after the commit: `node e2e/language-gate.mjs --update`, full gate, commit in English, push and CI watch as a background tool call. Then E6 (e2e/scripts), E7-E10 (docs, System, archives, memory).

🔎 **Full code audit on 2026-09-01** —
**[AUDIT-2026-09-01.md](AUDIT-2026-09-01.md)**: six auditors per
layer + cross-cutting tooling, on 0.15.0. **11 S1**, ~90 S2, exhaustive
S3, a four-wave plan, **seven CE decisions** pending
(history rewrite, single-instance, images on « Transférer »,
missing CONDSTORE, `failOnFlakyTests`, one TLS stack, the Google OAuth
client type). **Wave 0 DELIVERED the same day**: field TRACE logs
(9.1 MB, 99 third-party addresses) removed from the PUBLIC repository
and global `*.log`/`target/` added to `.gitignore` — **history
REWRITTEN the same day** (CE decision, `git filter-repo`, main + the
26 tags force-pushed; verified: `journaux` unfindable from any tag,
release 0.15.0 and `latest.json` intact). What the rewrite does NOT
purge: the `refs/pull/1-4/head` refs and dangling objects (the old
`051bb01` still answers by hash) — **GitHub support request due**
(the « remove sensitive data » form). Two traps engraved along the
way: the pre-push hook was dying on a missing `remote_sha` (`git
diff` invalid under `set -e`) — fixed, the full gate runs instead;
and `main`'s protection (`allow_force_pushes: false`) is lifted and
restored via `gh api PUT …/protection`; `open` with
`shellexecute-on-windows` (without it, `open_link` launched
`powershell.exe` synchronously on the pump — **MEASURED on
2026-09-01**: `freeze-probe.py` 60 s on the Clarity fixture, repeated
clicks on the Vantis thread link, **0 freeze > 150 ms**); trap
engraved along the way: the probe is NEVER played during a gate, the
e2e launcher kills every `wind-desktop` under `target\` (code -1,
false crash — two false alarms that day); CI in `--all-targets` +
`--doc` (it was not running the examples' tests, including the one
for credential non-disclosure); hook without `> /dev/null` on the
three text gates; **D-36 closed** with the net
`une_base_neuve_n_a_aucune_colonne_fantome` (proven by breaking it,
twice); `make-release.ps1` refuses a branch ≠ `main`;
`spikes/web-bridge` taken out of the workspace; `mail-render`:
`img-src` without `http:` + `no-referrer` even with images granted.
**Next topic: audit wave 1** (core S1: pure `plan_sync` against the
« initiale » sync of an emptied mailbox, quarantine of refused
actions, atomic purges, the main-thread guard extended to `async`
commands, bounded IDLE watch, single-instance) — or the beta feedback
if it arrives first. **Two CE decisions taken on 2026-09-01
(evening)**: (1) **single-instance via an `fs4` file lock** next to
`wind.db` (already a dependency, zero new plugin), a clear message if
a second instance starts; (2) **no 0.15.1** — wave 0 ships with wave
1 in **0.16.0** (MINOR: behavior changed), reopening clause: a freeze
on link click or an HTTP pixel reported by a tester ⇒ 0.15.1 the same
day.

**Last job closed: [PLAN-AUDIT-V2](PLAN-AUDIT-V2.md)**
(2026-09-02, full field validation in six passes, CI green run
33642403656, **ADR 0031**, journal A107-A109) — audit wave 2 (ten
measurable S2 batches + the front end, D1: « tout en un chantier »),
opened and closed the same day. **Eleven steps**: second
`Store::open` 36 → 0.9 ms (E1); 28 MB body indexed 401 → 338 ms and
210 → 133 MB (E2); `HEADER.FIELDS`, bounded batches, one `LIST` and
one `CAPABILITY`, single MIME parse (E3); `nettoyage_groupes`
380 → 67 ms on 200 k / 5 000 senders (E4); resumable initial sync,
SPECIAL-USE, `Reply-To`, send echo (E5); bulk gesture in ONE
all-or-nothing call (E6); send refused on the 5th failure (E7); CSP
and bounded paths (E8); CI actions pinned by SHA, « flaky : N », hook
→ `gate.ps1` (E9); retryable body, Feed merged and windowed,
`etat_ui` probe, forward without remote image (E10, A107);
single-`Menu.svelte` keyboard handling (E11, A108). Fresh-eyes review
(14 fixes), gate andon (UTF-8 panic in `fin_du_bloc`). **Field: 4
findings fixed the same day** — `reply_to` migration missing on the
real database (the watcher was failing every pass; lesson §9 «
adopter les données anciennes » recurring), Feed image guard beyond
page 0, editable line after the forwarded block, List probes
faithful to the row (« Déjà consulté » overlapped a row) — then **the
thread bars redrawn per CE verdicts** (A109: stuck under the header
in the pane, in the header bar on screen 03 aligned on the column,
floating reply bar at the bottom of the message). D9: Feed window at
5; **RAM after ten Feed pages: 251 MB, of which 132 for the GPU
process, 200 budget exceeded → D-53** (root cause: one iframe per
card; the budget itself remains to be pinned down: at rest, or the
heaviest gesture). Debt D-51 (missing CONDSTORE), D-52 (stated
limits), D-53, D-54 (flaky `multi-select:173` ×3). **0.16.0
PUBLISHED on 2026-09-02** (`343c8d0`, audit waves 0 and 1;
`verify-release.ps1`: everything passes, minisign crypto not proven
for lack of the tool on PATH; **auto-update proven on both
workstations**). Along the way, two tooling nets: the verify script
stopped parsing under PowerShell 5.1 (an em dash in a string of a
BOM-less `.ps1`, §7.1) — the gate now parses every `.ps1`; and
`thread-bars:25` waits for the card before reading it. **0.17.0
PUBLISHED on 2026-09-02, the same day** (wave 2;
`verify-release.ps1`: everything passes, minisign crypto not proven
for lack of the tool on PATH; **auto-update proven on both
workstations**). The GitHub support ticket (purge of `051bb01`,
personal data) was **sent on 2026-09-02** — response pending, proof:
`gh api repos/smonchamps/wind/commits/051bb01…` ⇒ 404.

Last closed before it:
**[PLAN-AUDIT-V1](PLAN-AUDIT-V1.md)** (2026-09-02, CE field verdict
« ok », 0 findings, CI green run 33568895402, **ADR 0030**) — audit
wave 1 (the core and shell S1s), opened on the evening of
2026-09-01, GO from the CE at STOP 1 (D1-D4 settled), **nine steps
delivered**: single-instance via a file lock (E1), pure `plan_sync`
against the « initiale » sync of an emptied mailbox (E2), quarantine
of refused actions + a line in the slot (E3, A106), atomic purges and
a single table list (E4), the main-thread guard extended to `async`
commands + 17 commands migrated + `VolGarde` + `into_inner` (E5),
bounded IDLE watch via `FluxBorne` (E6 — two false leads killed by
the tests: on Windows neither the timeout nor a `shutdown` via a
socket clone takes effect), transient SMTP 53x + complete
`References` + OAuth refresh reserved for auth refusals (E7), renewed
refresh token stored + OAuth wait bounded to 5 min + `Debug` masked
(E8), `wind.log` bounded to one meg without PII (E9). Fresh-eyes
review 8 angles / 10 checked / **10 fixed** (including the
relocation-lock race, the eternally refused item, the address in
`wind.log`). Six full gates green. Tests: mail-core 422 → 433,
mail-imap 70 → 72, mail-smtp 26 → 29, mail-auth 21 → 24, desktop
27 → 31, e2e 187. Debt: **D-49** (cleanup deferred from the review,
wave 3) and **D-50** (Microsoft refresh token and `BrowserFallback`
to confirm). **Next topic, the CE's choice: the 0.16.0 release**
(decision B — the CHANGELOG entry is written, `scripts\make-release.ps1
0.16.0` from `main`; then `verify-release.ps1` and proof of the
n-1→n update on both workstations, `maj.log` readable afterwards),
**the beta feedback** (follow-up with the silent ones on 2026-09-03),
or **audit wave 2** (ten measurable S2 batches, `AUDIT-2026-09-01.md`
§5). Outside the job: the GitHub support ticket (purge of
`refs/pull/1-4` and of the `051bb01` object) — **sent on 2026-09-02**,
response pending; verify
`gh api repos/smonchamps/wind/commits/051bb01…` ⇒ 404.

Previous job:
**[PLAN-RETOURS-14](PLAN-RETOURS-14.md)** (2026-08-31, opened and
CLOSED the same day, commit `18a9e61`, CI green run 33408211506, CE
field validation in THREE passes the same day — 1-7 OK, R8-R10
requested and delivered in session, final verdict « ok », **0 KO
findings**). Ten findings from real use, journal **A104**:

- **thread bar AT THE TOP, sticky** to the scrollport of both frames
  (D1), « Déplacer vers… » menu opening downward;
- **Organized inbox**: header normalized to title only (D2), neither
  banner nor tabs (D3), **current section name stuck** to the scroll
  (zero-height band, outside the windowing geometry);
- **the R4 "bug" is the golden rule** (mixed thread, intended and
  tested) — now SAID: a « En attente au Portier » badge (D5,
  `portier_adresses` command, intrusion scenario proven in e2e via
  `seed_arrivee`, which knows how to reply to a thread);
- **Settings > Screener**: exhaustive list of decisions (alphabet,
  client-side search), « **Modifier** » re-offers every rule +
  « Renvoyer au portier » (R10); verdict vocabulary in one copy
  (`lib/portier.js`);
- **Paper trail GROUPED by lead sender** (D7, recency;
  `registre_groupes`/`registre_groupe_page`), a ⋯ group menu
  (Déplacer, Écarter), « Voir plus » — the reading pane stays the
  reader;
- **nav badges** for the Feed (cards never opened, `kiosque_lus` —
  D8) and the Paper trail (unread IMAP), bounded to organized mode;
- **zero em dash** in the shown texts (D4, net `catalogs.test.mjs`
  proven by breaking it);
- **a Yes at the Screener counts as trust** (R8): the sender's image
  rule is set INSIDE the verdict transaction (every path), revocable
  at Settings > Display;
- **section sort** (R9): a four-sort dropdown menu, each entry its
  own glyph — **4 new `tri_*` glyphs, set 87 → 91**; ONE component
  (`TriSection.svelte` + `comparateurTri`), four surfaces (Feed ×2,
  Paper trail, Screener history, Cleanup).

Fresh-eyes review 5 angles / 10 fixed — including the **mode-reread /
first pump race, MEASURED on the e2e fixture** (page 0 at ~85 ms,
mode reread at ~105 ms: the section seam was never asked for — the
reread now re-serves the views) and the sticky elements that passed
over the modal veils (isolation). Tests mail-core 419 → **422**, e2e
177 → **187** (+2 node). **D-47 amended** (Paper trail
pile/menu/rank copies). Limits stated in the PLAN (global badges
under an account filter; badge Unicode divergence; transient overlap
of the sticky band). Two tooling traps engraved in memory: a pattern
replacement that swallowed the definition of the targeted helper
(infinite recursion, swallowed exception), and an un-sabotaging `git
checkout` that carried off the file's uncommitted edits.

The release due before the beta is
**DONE: 0.15.0 published on 2026-08-30** (D12 held — it ships
RETOURS-13 AND PLAN-HORIZON-NETTOYAGE, the CHANGELOG entry for both
written at publication; detail under « Dernière version livrée »
below). **The way is clear for the next topic on the list: the first
beta wave** (PLAN-BETA — invite 5-10 close contacts, D9; CE action:
the invitations). Then, the CE's choice: **E6 — Groups** (deferral
written, folder and S1 spike ready) or the beta feedback.

**Beta wave 1 OPENED on 2026-08-31** (PLAN-BETA §3 bis): five close
contacts, **ANONYMOUS** register per the plan (T1-T5: workstation
posture, targeted account, dates, feedback — the repository is
PUBLIC, CE decision of 2026-08-31: names, addresses and matching stay
with the CE, outside the repository; no copied feedback carries a
name or message content), sample invitation wording at §3 ter — the
sending stays with the CE, who handles it. **The FIVE invitations
went out on 2026-08-31** (CE finding): the wave is running. **Both
mandatory postures are covered as of T1** (x64, Smart App Control
`On`, Gmail, installed on 2026-08-31): both assumed risks of
PLAN-BETA §2 are being tested. Two new facts, of unequal weight: (1)
**0.15.0 x64 passes on a SAC `On` workstation outside the CE** —
logged as **D-39**; it closes nothing (verdict rendered BY HASH,
silent on the next version) but supplies the bench that the
**PLAN-SIGNATURE net's due measurement** was missing (an update
failure visible as a real SAC refusal); (2) the Google « application
non validée » screen was crossed — whether it caused hesitation
remains open, the guide does not prove it was read. Follow-up with
the silent ones planned for **2026-09-03** (the three-day rule).
Prerequisites verified the same day: PUBLIC repository and `main`
pushed (`f8cc0f4`), **0.15.0** the latest release with its two
installers (x64 + arm64), their `.sig` and `latest.json`,
`feedback-wind@fcts.io` receiving (finding of 2026-08-29), a Feedback
button in place in the header (`App.svelte`, conditional on at least
one account). Two GUIDE defects fixed before sending: (1) « quatre
étapes » of onboarding when the journey actually counts **cinq** (the
beta step A91) — exactly the staleness fixed in the spec on
2026-08-29, BETA.md had been forgotten; (2) mode **« Organisé »**,
the novelty of 0.14/0.15 and a VISIBLE toggle in the header, which
the guide did not mention — CE decision of 2026-08-31: a short §
(BETA.md §3, sections renumbered 3→4→5→6, the « voir §4 » reference
fixed to « §5 »). Two MANDATORY postures within the set of five,
without which the wave tests neither of the two §2 risks: at least
one **Smart App Control `On`** workstation, at least one **Gmail
account**. Measurements under way: **S4** (flow of unknowns to the
Screener, one week, started on 2026-08-30). **Two due measurements
closed on 2026-09-02**: the update banner, read from `maj.log`
(poka-yoke `13c7681`) at the 0.16.0 → 0.17.0 update — manifest
verified in 79 ms, 5.9 MB downloaded in 1.8 s, installer written in
5 ms and launched in 8.0 s (the time of the Windows install check
before UAC, to watch on the x64 workstation under Smart App Control);
and **the cost of `nettoyage_groupes` on the real database** (249 k
envelopes): 159 groups in 86 ms cold, 17 ms warm (`wind.log`, STOP 2
of PLAN-AUDIT-V2), the 200 ms budget held.

Last closed:
**[PLAN-HORIZON-NETTOYAGE](PLAN-HORIZON-NETTOYAGE.md)** (2026-08-30,
opened and CLOSED the same day, commit `f66d1e6`, CI green run
33333151630, CE field validation **12/12 « Tout OK »**, zero
findings). Two tracks, D1-D12 settled at STOP 1:

- **Track A — the import horizon** (ADR 0029, amendment 0010, A102):
  when an account is added, the history depth is chosen (1 month →
  all, default 1 year — D2); only BODIES are bounded (D1/A2: full
  envelopes, list and subject/sender search all included, body
  beyond the horizon fetched on click); pref `horizon_import.{id}`
  (PREFS_PAR_COMPTE), the bound derived at READ time, adjustable at
  Settings > Accounts (D3 — text-based control, styled on the name
  card), written at the FIRST add only (a re-add does not overwrite
  a choice), existing accounts deemed « tout » (D4).
- **Track B — Spring cleaning** (A103): 5th section of organized
  mode, a new `nettoyage` glyph (a draft of air, CE verdict « D » on
  a board of six, set 86 → 87). Intro (3m→all range + D6 scope
  choice, CE texts word for word), sort by sender GROUPS in the
  Screener's vocabulary: the verdict applies to the group and covers
  both the range's existing stock AND the future (D5), via
  `poser_verdict` — THE gate shared with `router_expediteur` — stock
  actions INSIDE the transaction (pattern E3), trash never final
  (D4), local removals in ONE transaction. A single persisted
  session (`nettoyage_session`, D8, bound FROZEN at start), gauge
  derived from the REMAINING groups, Screener defaults shared (D9),
  routed items excluded (D7), navigation within a group.

Fresh-eyes review 8 angles / ~35 candidates / 10 retained /
**8 fixed** (including the anti-duplicate that removed the local
copy without posting the action — the message would have come back
at the next poll); **D-47 REOPENED** (4th copy of the ⋯ menu, logged
to DEBT). Limits stated in the PLAN (dateless in any range —
precedent A98; full archive out of scope; stock untouched if an
action is already queued or for junk with no resolved folder; e2e
fixture dates frozen in 2020). Tests mail-core 412 → **419**, e2e
169 → **177**. UI vocabularies in one copy (`lib/vocabulaires.js`),
exhaustiveness net `horizon_epoch` × vocabularies.

The job closed before it:
**[PLAN-RETOURS-13](PLAN-RETOURS-13.md)** (2026-08-30, opened and
CLOSED the same day, commit `5ab1f15`, CI green run 33323808766, CE
field validation in two passes — 5 findings fixed the same day,
verdict « tout ok »). **Twelve CE findings on the Organized mode**
(post-0.14.0), journal **A101**: Sombre Automatique first among the
Themes; the Settings rail set at baseline+2px (nav pattern); «
Réception » in organized mode (a SINGLE `cleLibelleBoite` rule — four
dead copies removed) + a nav net after the Screener; the Screener
header on the left (glyph thinned to 1.5), a three-sentence CE
subtitle, empty history rewritten, section title always visible;
**a bare click at the Screener follows adjustable DEFAULTS** (shipped
Yes → Inbox, No → Trash; a Settings > Screener section in BOTH
modes, vocabulary derived from the routing tables, refused on the
core side); **the Feed gains a "read" memory** (reverses A100: table
`kiosque_lus` on the pins pattern, IntersectionObserver witness
rearmed on failure, "Unread" sections expanded / "Previously read"
grouped by sender collapsed into a pile, sectioning IN SERVICE of the
page, mark-all-read via the checkmark); Feed header in the Screener
format (shared classes `.entete-vue` from systeme.css); **the `feed`
glyph redrawn** (a news kiosk, scalloped — a board of 7, CE verdict
« B »); « Déplacés automatiquement dans la corbeille ». Fresh-eyes
review 8 angles: 8 fixed (including the selector race and the witness
that never rearmed), new debt **D-48** (the list does not follow an
external write — the lucky e2e net made honest). Tests mail-core
410 → **412**, e2e 166 → **169**. Trap engraved along the way: a
"fortunate" reload of a probe can pass an e2e step for months —
target the product gesture.

The job closed before it:
**[PLAN-MODE-ORGANISE](PLAN-MODE-ORGANISE.md)** (2026-08-29 → 30,
**CLOSED on 2026-08-30 — full field validation, E1-E5bis DELIVERED
and PUBLISHED in 0.14.0**; E6 — Groups — deferred, a future job).
**Organized mode** — the second sort mode inspired by HEY, D1-D9
settled at STOP 1, reversible, the golden rule "never lose mail"
proven RED at every step:

- **E1 — the foundation** (`f6a81f6`, ADR 0028, A96): pref
  `mode_organise` + activation epoch, Feed/Paper trail nav, table
  `routage_expediteurs`, « Déplacer vers… ».
- **E2 — the Screener** (`fe33b51`, A97): retention of unknowns
  "arrivées seules" (D3) — materialized `portier_attente`, flag
  `threads.organise_hors` maintained by `thread::refresh` (S2-bis
  verdict: any form computed at request time collapses at deep
  offset), generated column `sender_norm` (trap engraved: a SQLite
  EXPRESSION index never serves a join); Yes/No page, history,
  reinstatement.
- **E3 — the rules of No** (`59a8378`, A98): action logged INSIDE
  the batch transaction (never a crash window between mail and
  intent), `pending_actions` replayed at the head of sync, anti-
  duplicate guard, trash never final (D4), shutdown with the mode
  (D2).
- **E4 — the organized Inbox** (`5493bef`, A99): "Nouveau pour vous"
  / "Déjà consulté" sections in ONE ordered flow (bench 200 k:
  0.03/1.6 ms), centered column, screen 03; two windowing traps paid
  for (the REAL spacer required by the absolute header; the auto
  margin that kills the flex stretch).
- **E5 — Set aside** (same commit): table `mis_de_cote` (pins
  pattern), exclusion shared by ONE write (`exclusion_organisee()`),
  pile + fan + table, SEEDED state.
- **E5bis — the Feed in cards** (`0f61506`, A100, CE decision
  "before the release"): cards already opened, FULL body from the
  CACHE by pages of 20 (D5/S3: 12 ms), R1 image guard per card, fold
  on the subject line (visual STOP in three passes), a ⋯ gesture
  menu, dedup on append, S1 scriptless iframe.

Fresh-eyes reviews by step: 8/8, 10/10, 10/9, 6/5 fixed — including
six "mail loss" defects proven RED. Tests mail-core 383 → **410**,
e2e 153 → **166**. New debt **D-46** (Screener row anatomy copied)
and **D-47** (⋯ menus ×3, pins/pile twins). Limits stated in the
PLAN (falsified Date, junk with no resolved folder, CONDSTORE
inventory during a replay, no Feed windowing, Escape precedence).
**0.14.0 is PUBLISHED on 2026-08-30** (`40771ab`, CI green run
33316584480, bare tag, Latest, **verified 18/18** and **auto-update
0.13.0 → 0.14.0 proven on BOTH workstations the same day** — CE GO).
It carries E1-E5bis AND the **Innamoramento** theme, light + night
(PLAN-MONA, A94/A95). Kaizen figures at the closed PLAN (≈ 80 M, 5
gates, 0 KO in the field).

That same 2026-08-29/30, two CE findings settled outside a job:
**"Mona" renamed "Innamoramento"** (A95, ids migrated, commit
`0a25105`) and **the beta blocker lifted** (`feedback-wind@fcts.io`
receiving — DNS propagation delay).

The job closed before it:
**[PLAN-MONA](PLAN-MONA.md)****[PLAN-MONA](PLAN-MONA.md)** (2026-08-29, opened and closed the same
day, commit `409c8ae`, CI green run 33270609284, CE field verdict
« Terrain OK sur les deux thèmes, GO » — zero findings). **Two new
themes, "Mona" and "Mona · nuit"** — **renamed "Innamoramento" /
"Innamoramento · nuit" on 2026-08-29 (A95, CE feedback, before any
release; ids migrated, migration proven at the net)** — (CE colors:
accent `#AD204C` —
6.80:1 on white, both accent AND brand of the light theme unchanged
— and the tile hue `#A0868F` declined by polarity
`#EFDFE4`/`#2C2126`: the raw value is impossible at the thresholds,
2.04:1 under ink2, 1.88:1 under the worst marker, 3.33:1 at best at
night). **V7 is AMENDED (A94, ADR 0027)**: the theme table is short
and LIVING — additions/removals possible, never a return of the 28
Wada. Mechanics: the light marker table served by
`[data-theme$="-nuit"]` (zero hex copied), its parser centralized in
`tokens.mjs` (`lireReperes` — the two gates each carried their own
copy of the regex), the migration guard of `theme.js` derives from
`THEMES` (the hardcoded list removed — proven RED: without it a
persisted `mona-nuit` choice was rewritten on every start),
`NOMBRE_ATTENDU` 2 → 4. Contrast **440 pairs (220 → 440), 0
failures**; coherence 4 themes / 68 tokens; e2e **153/153** (two
specs extended, accounts 2 → 4, migration proven by breaking it).
Fresh-eyes review 6 angles / 10 retained / **9 fixed** (including the
wrong engraved figure 166 → 220 and the theme leak in the migration
test); new debt **D-45** (System swatches = the only hex copy outside
the gate). **Innamoramento ships with the next release** — the
CHANGELOG entry will be written at that point, under this name
(§2.9). **The beta blocker is LIFTED on 2026-08-29**:
`feedback-wind@fcts.io` receives — the mails from the 28th arrived,
it was a DNS propagation delay, not an alias outage. **The first
beta wave (5-10 close contacts, D9) is open** — CE action at
PLAN-BETA.
---

The previous closed job:
**[PLAN-RETOURS-12](PLAN-RETOURS-12.md)** (2026-08-28 → 29, commits
`60225b0`/`331832d`, CI green run 33216010954, field CE **4/4 on
2026-08-29, zero finding**). Five feedback items: (1) **an account added
while Wind is open reads as connected** — `compteAjoute()` calls
`connecter()` (the `connectes` array was only filled at startup),
the nav reloads BEFORE the network ; e2e seam `__e2eAjout` ;
(2) **the package size is FLAT, a fact measured over 12 releases**
(arm64 5.04 → 5.66 MB, a single +0.44 MB step at the bi-arch 0.6.0 ;
x64 ±1 %) — the longer update banner comes from the honest path of
0.10.2, not from bytes ; instrumented path (manifest / download /
write / spawn durations), **traces visible ONLY via
`run-wind.ps1`** (windowed app with no stderr) ; (3) **header
marker 28 px** (A93 — in passing, the V11 fiche said « Wind
15 px », false: 18 px real, corrected) ; (4) **the Cargo workspace
versions follow the product version** (0.1.0 frozen since
origin → 0.12.0 ; `make-release.ps1` now bumps both,
validations before any write) ; (5) **the two-line message
header** (A92): "Name <address> on Mailbox" (rule D7 kept) then
"To: Name <address>, …" and "Cc: …" if present — the recipient
names come from the **contacts directory**
(command `noms_adresses`, PK lookup bounded to the thread's To/Cc, ~0.2 ms ;
`cacheNoms` cache surviving the frame switch). Fresh-eyes review
8 angles / 10 retained / **8 fixed** ; new debts
**D-43** (echo without Cc) and **D-44** (`connectes` with no refresh
cycle — the mirror symptom of R1). e2e 150 → **153**.
**Tooling trap paid and recorded**: the e2e seed fixture expires at
MIDNIGHT even "fresh" on TTL (two red specs at the 00:00 pre-push ;
`launch.mjs` now requires the same calendar day). **DELIVERED in
0.13.0, PUBLISHED on 2026-08-29** (commit `9599b31`, CI green run
33217432151, bare tag, Latest, **verified 18/18** by
`verify-release.ps1` — arm64 exe 200 / 5,667,616 b, x64
200 / 6,405,481 b, distinct signatures — and **auto-update 0.12.0 →
0.13.0 proven on BOTH workstations the same day**, CE GO: "Autoupdate OK
on both workstations"). First release where `make-release.ps1` also
bumped the Cargo workspace (0.13.0 everywhere — E4 proven under real
conditions). One flaky logged at the release pre-push gate
(redesign-panes:86, a compositing scrim intercepting the click —
retry green, 152 passed). NB: the `maj :` traces were not
captured at this update (workstations launched normally) — the banner
measurement is waiting for an update accepted from a Wind launched by
`run-wind.ps1`. **The next subject remains the first beta
wave** (PLAN-BETA — CE blocker: get `feedback-wind@fcts.io`
receiving ; then invite 5-10 close contacts, D9).

**e2e suite audit (2026-08-29, commit `84651bb`, CI green run
33217676308)**: the 20 specs (153 tests) checked statically against
the `apps/desktop/ui-v2/src` source — **0 stale, 0 vacant, 0 duplicate** ;
every selector, testid and catalogue text traced back. Only defect:
the title and header of `redesign-feedback-8.spec.js` said "four
steps" when the body proves 5 (beta 4/5, A91) — two strings
fixed (CE GO), zero behavior touched. ~10 FRAGILE tests
logged (assertions on `__e2eJournal`, `outbox_status`,
`getComputedStyle`, focus, 2 px pixel alignment) — accepted and
documented trade-offs in the specs, left as is ; to watch at the
next CSS/IPC refactor. Scope refusal §2.6: nothing sound was
rewritten.

---

The previous closed job:
**[PLAN-RETOURS-11](PLAN-RETOURS-11.md)** (2026-08-27 → 28, commits
`a562fdd`/`a9f93e0`, CI green runs 33127472066/33127940550, **DELIVERED
in 0.12.0 PUBLISHED on 2026-08-28, verified 18/18 and auto-update proven
on BOTH workstations the same day** — CE GO: "Release ok, auto update ok
on both workstations"). Three feedback items: (1) **the image guard
has a memory** — "Show images" persists per MESSAGE (envelope
key, `pins` pattern; reverses invariant A43, decision D1) and
"Always show images from this sender" sets a workstation-wide
global rule (normalized exact address, authority in the CORE in
`message_body`), revocable at Settings > Display (D4) — A89 ;
(2) **"Made in EU"** + EU flag (SVG frozen outside themes, outside
the registry) in About — A90 ; (3) **the beta is launched**:
[PLAN-BETA.md](PLAN-BETA.md) (dated actions), [BETA.md](BETA.md)
(tester guide), and — field findings from the 28th, fixed the same day —
the **Feedback button** in the header (new glyph, sent via `queue_send`
to feedback-wind@fcts.io, immediate `flush_outbox`) and the **onboarding
step 4/5 "Wind is in beta"** (A91, CE text verbatim).
⚠️ The feedback address **does not receive yet** (fcts.io alias, proven
outside Wind — blocking CE action before any invitation, at
PLAN-BETA). Fresh-eyes review 8 angles / 10 retained / 9 fixed
(including the `reset_mailbox` purge of the image memory — a recycled
UID would have inherited a consent, TDD) ; new debt **D-42**
(no per-message revocation). e2e 148 → **150**, 4 full gates
(2.2-2.6 min). Nothing more is owed on this version. **The
next subject: the first beta wave** (PLAN-BETA — CE blocker:
get `feedback-wind@fcts.io` receiving, proven down on the 28th from
any client ; then invite 5-10 close contacts, D9).

---

The previous closed job:
**[PLAN-RETOURS-10](PLAN-RETOURS-10.md)** (2026-08-27, opened and closed
the same day, commit `a72f341`, CI green run 33111561147) — four
CE feedback items: **multi-select** (Ctrl-click which checks AND moves the
reading focus, Shift-click from the anchor or the selection, checkbox on
hover in an 8 px gutter with content pushed to 34 px, list bar
transformed — read/unread/archive/junk/delete —,
e/Del shortcuts on the batch, and **the whole thread moves** — D6, settled
on the Vantis example), **Windows icon** restored to the Elements brand
(it still carried the pre-2026-08-24-adoption "W-badge" ;
`make-icon.ps1` rewritten), **header marker 28 px** (D2), **optical
alignment of the nav glyphs** (a board of three variants, verdict
C — baseline + 2 px, D7). Field validated in TWO passes the same day
(8 findings on the first pass, all fixed within the session) ; fresh-eyes
review 8 angles, 10 findings, 9 fixed ; e2e 137 →
**148** ; new `check` glyph (79) ; A86-A88. New debt: **D-41**
(keyboard checkbox). **0.11.0 is PUBLISHED on 2026-08-27** (commit
`d0f9c8c`, CI green run 33113349707), **verified §2.10 the same day:
18/18 PASS** (Latest on bare tag, 5 assets, `latest.json` with no BOM
1,590 b on BOTH platform keys, signatures == `.sig` and
distinct, arm64 exe 200 / 5,630,211 b, x64 exe 200 / 6,354,494 b) and
**proven in the field: auto-update 0.10.2 → 0.11.0 confirmed on
BOTH workstations** (CE GO: "release ok, auto update ok on the 2
workstations"). Nothing more is owed on this version. NB: no SAC
refusal was reported at this update — proof of the update-failure net
**under refusal conditions** (PLAN-SIGNATURE) remains owed, at
the next occasion where SAC genuinely refuses.

**0.10.2 is PUBLISHED on 2026-08-27**, verified §2.10 (everything passes,
2 channels), **auto-update proven in the field on BOTH workstations** (CE
GO: "release ok auto update ok on the 2 workstations"). Then: the
**closed beta** — with, blocking its path, the debt **D-39**
(Authenticode signature frozen: on any Smart App Control workstation,
installing an unsigned exe is a lottery by binary AND by
day — proven on 26-27/08) and **D-40** (upstream
tauri-plugin-updater issue, CE GO pending).

---

✅ **[PLAN-SIGNATURE](PLAN-SIGNATURE.md)** (2026-08-26 → 27, CLOSED) :
an update installation failure now **is visible** (banner
that rearms, Settings with no dead end, 10 min timeout, announced
version = installed version, marker in a new directory, e2e guard,
crate pinned `=2.10.1`) instead of closing the app silently. The
Authenticode signature is pending (D2): individual Trusted Signing
validation closed outside the USA/Canada. Proof of the net under
refusal conditions: at the first update from 0.10.2 on an SAC
workstation.

The original finding: "Install" **closed Wind without installing
anything** — Smart App Control (`On`) refused the unsigned exe and the
updater plugin exited via `exit(0)` without reading the return of
`ShellExecuteW` (spike `spikes/maj-x64/`). Full detail in the plan
(finding, E1-E5, decisions D1-D5) and in the journal **A85**.

---

✅ **[PLAN-DEMARRAGE](PLAN-DEMARRAGE.md)** (2026-08-26, CLOSED on the 27th) :
delivered, field 6/6, commits `b94d63b`/`385ee64`, CI green, **0.10.1
published** (18/18) ; the x64 auto-update proof (D5) landed on the 27th
(0.10.0 → 0.10.1 applied, after an initial SAC refusal).

The CE's finding — "freezes and slowness at startup, once the
window is open" — was a **SERVICE freeze, not a window freeze**:
`backfill_status` started at t + 3 s, held the **global commands
lock for 8,870 ms**, and during that time no application command
was served. Measured in the field, first launch **after a
machine restart** (the project's first honestly cold measurement):

| | before | after |
|---|---|---|
| `backfill_status`, lock held | **8,867.8 ms** | **124.9 ms** (×71) |
| window → full list | **1,157.3 ms** | **384.6 ms** |
| the part outside the WebView2 slice | 406.4 ms | **119.0 ms** (×3.4) |

**Three fixes, each of one line or nearly, each measured before
being written:** the `AND b.scanned = 1` criterion leaves the
backfill queries (it forced 251k fat-row recalls in 11.4 GB to
protect **zero** rows — measured on both workstations) ; `idx_envelopes_date`
gains `uid` (without it SQLite would look up the envelope row to
read the polled uid — `pending_total` 521.9 → 107.9 ms) ; and an
`await tick()` starts the first page of the list **before**
the probes, where it ranked twelfth.

**Four hypotheses were overturned by measurement or by
counter-expertise, and none was written before being tested** — the
fix in the investigation file (a bare index SQLite would never have
chosen: it needed `INDEXED BY` or UNIQUE), contention, the
64 round trips, and plan step E2 (deferring the probes would have
**manufactured** a repaint of every row on the first screen). Savings:
one useless index at an 18 s migration, one query grouping with no
effect, and a visible defect at the CE's.

**CE decisions: D1-D9** (§5 of the plan), including D1 "painted-list
milestone", D8 "remove the `scanned` criterion" and D9 "accept the
1.77 s index-rebuild cost at first launch after an update, with no
screen" — recorded in STANDARD §3.

**Debt reopened then CLOSED: D-8.** New debts: **D-36**
(phantom column of `echos`), **D-37** (`sync_progress`), **D-38** (the
preview backfill reloads the list for nothing).

**Two tooling defects paid for and fixed along the way**:
`depouiller.py` died outside PowerShell 7 on its own arrow, and the
bench wrote `$n` in its loop bounded by `$N` — **in PowerShell
that's the same variable** ; a `-N 3` ran for ~550 rounds. This is
also what explains the "19 launches" of the 26/08 campaign.

**Remaining:** GitHub's answer to the ticket (check the 404 on
`051bb01`) ; PLAN-BASCULE-ANGLAIS in progress in another session.

**Last closed job: [PLAN-ESPACEMENT](PLAN-ESPACEMENT.md)**
(2026-08-25, field CE **7/7 zero finding**, gate green 2 min, e2e
129 → **137**) — **three air steps between messages** (A83):
"Low" (the existing pixel-for-pixel, 13 px padding, row 88),
"Medium" (19, 100), "High" (25, 112), at Settings > Display
(native selector, A26 pattern). The step is set as a **token
`--rangee-pad`** on the list frame (pattern of `--l-nav`): all
rows take it at once, probes included. **The air lives in the
padding and nowhere else** — a margin or a `row-gap` would give
12.375 px per row invisible to `offsetHeight`, hence to windowing.
**The height probes become permanent**, in a **positioned
cage**: `sondees`/`sonder()` are dead, `bind:offsetHeight` replaces
them. Measured on the bench (`spikes/espacement/`, msedge = WebView2, 4
variants × 5 heights): without the `position:relative` of the cage, the
probes add up to **85 px of phantom scroll** ; with it, **zero**
at every height. Pre-existing defect fixed along the way (decision
D3): `visibles` read `clientHeight`, which is not a signal —
enlarging the window left an empty band. Fresh-eyes review: 7
angles, **29 findings retained, all fixed** — including the order of
effects (the re-anchoring read a position already rewritten by the
pinned items' effect: **44 rows of drift measured**) and the `in`
trap on the prototype chain. **Paid-for lesson**: three of the five
tests in the first net could not fail; the net was rewritten (8 tests) to
read what the user SEES and was **proven not vacant** by deliberately
breaking the code. **Delivered in 0.10.0** (published on 2026-08-25).

**The previous closed job: [PLAN-REPERE-LIGNE](PLAN-REPERE-LIGNE.md)**
(2026-08-25, field CE **15/15**, gate green, e2e 124 → **129**) — **the
mailbox is spelled out in full, on the sender's line**
(A80-A82). The marker badge under the avatar is replaced by a **text
block** on the header line — "on" in muted ink, the marker's
glyph **in bare outline** at the account's hue, the label (custom
name A78, otherwise the address) — CE's motivation: *the sentence
reads, it avoids having to permanently remember a color or a
logo*. Three truncation rules measured (the time never yields, the
block yields three times faster than the sender, cap at the
**third** — plateau 33-36 %, measured over 22 drawings and 5 throw-away
boards). **The initials tile leaves the LIST** (A81) — it survives in
the thread and the Drafts folder, where it works. **The marker dot
leaves the nav** for a 16 px outline (A82): measured across the whole
window, the disc is now **the only round shape on screen 02**, which
V4 aimed at without reaching it ; the dot survives in Settings, as a
*choice* dot. The 24 hex values in the palette used twice
(background and color) move to **`--rep-*` tokens** — the contrast
gate was amended accordingly, and the coherence gate now checks
BOTH tables plus the tokens of both polarities (proven red on the
three failures it targets). **No new contrast pair**, no new glyph,
A44's two templates measured unchanged.
Fresh-eyes review: 8 angles, 40 candidates, **14 distinct defects —
all fixed** (including a block that could paint over the time,
and a D7 rule that gave a chorus to single-account workstations).
One field finding (point 12: the pane spoke when the list
stayed silent) fixed the same day. **Delivered in 0.10.0** (published on 2026-08-25).

**The previous closed job: [PLAN-ELEMENTS](PLAN-ELEMENTS.md)**
(2026-08-24, `fb32238` → `0de3689`, field CE **8/8 zero KO** the same
day, CI green run 32752449754) — the "Elements" System became
THE reference System
(`docs/design/systeme.dc.html`, ADR 0026, journal A79, the old one archived
as `docs/archives/systeme.v1.dc.html`) and the UI delivers it in full: five
steps committed gate-green on 2026-08-24 (E1 the 2-theme base and
`--panel` dead, E2 the 78 glyphs in SVG and the Material font dead,
E3 zero radius / initials tile / unread disc, E4 the Elements
brand and the death of the hitofude stroke, E5 the registry 340), four
visual STOPs validated by the CE the same day, fresh-eyes review passed
(6 angles, 10 findings — 7 fixed), 124/124 e2e, the Fluent reservation
from V14 lifted at the real window. **Delivered in 0.9.0, PUBLISHED on
2026-08-24** (tag `0.9.0` on `f135791`, Latest) — release **verified
18/18** and **all field proofs done** on 2026-08-25, detail further
below. New debt: **D-35** (icon tier 16 — the reduced masters are
delivered, decision D4). Bench from 2026-08-24 (256k): page
p50 85.8 / p95 180.9 ms (P1: p95 307.6), theme 0.3 ms, RAM 8.1 MB.

⚠️ **Trap paid for on 2026-08-25 — this document lied for a whole
day.** It announced "Remaining: the 0.9.0 release" when it had
already been published the evening before, and the CHANGELOG still
carried "[0.9.0] - upcoming" (like "[0.8.0] - upcoming", published two
days before). Consequence: two jobs were written under an
ALREADY-DELIVERED version entry, and they had to be moved to 0.10.0. **The
rule that comes out of it: date the CHANGELOG entry at the moment of
publication, in the same move** — an "upcoming" entry on a published
version is a lie that spreads. The check that costs nothing:
`gh release list` before writing a release note.

**0.8.0 is published** (tag `0.8.0` on `a3d04fb`, 2026-08-23) —
it carries PLAN-RETOURS-9 (OAuth compiled in, "Remove the account" said,
account names). Its deferred field proof is **DONE on
2026-08-25**: an account connected on the second workstation from a
published release, **with no `setx` at all** — **ADR 0025 is CLOSED**. It
will have slipped by two versions (expected at 0.8.0, came after
0.9.0) ; the decision itself stayed unchanged in the meantime.

**Subject DROPPED on 2026-08-25: marker glyphs in solid fill.**
Requested by the CE that morning, investigated on the evidence, put on a
board that afternoon (`spikes/glyphes-pleins/`, the twelve markers in
outline vs. filled+outline at three sizes and two polarities) —
CE verdict in front of the board: **"The outline is enough."** Scope
refusal §2.6, **no production code touched**. The measured facts
are kept in the spike's README so they need not be remeasured: 9 glyphs
out of 12 fill without a redraw, the fill alone thins three,
and above all it **brings the silhouettes closer together** (overlap 0.24 → 0.47)
when the job of a marker is to distinguish twelve accounts. Not to
be re-proposed without a new reason.

**Next: the closed beta of 20-50 users** ([PLAN.md](PLAN.md)
§4, last step before gate 5) — **ENGAGED on 2026-08-28**
(PLAN-RETOURS-11 R3): action plan at [PLAN-BETA.md](PLAN-BETA.md),
tester guide at [BETA.md](BETA.md), Feedback button in the app (A91).
First wave D9: 5-10 close contacts, as soon as the feedback address
receives (blocking CE action).

**perf-lecture is off** (CE decision D1 of 2026-08-21): the
symptom (on-demand body throttled to ~7 s at launch, field finding of
2026-08-19) has been dead in the field since 0.2.1 — the counts have
left the display path (A64). To be reopened only if the field says
so again ; the WIP from back then was removed (CE decision of
2026-08-20), its material stays in the review section of
[PLAN-COMPOSITION-HTML](PLAN-COMPOSITION-HTML.md).

**Last delivered version: 0.15.0** (published **2026-08-30**, bare tag
on `52db74f`, marked Latest, CI green run 33334129022, **verified**
by `verify-release.ps1` and **auto-update 0.14.0 → 0.15.0 proven
on BOTH workstations the same day** — CE GO: "release ok verification
ok auto update ok on both workstations"). It carries PLAN-RETOURS-13
(twelve Organized mode feedback items, A101) and PLAN-HORIZON-NETTOYAGE
(per-account import horizon ADR 0029/A102, Spring cleaning
A103). Nothing more is owed on this version.

**The previous version, 0.14.0** (published **2026-08-30**, bare tag
on `40771ab`, marked Latest until 0.15.0, **verified 18/18**
and **auto-update 0.13.0 → 0.14.0 proven on BOTH workstations the same
day**). It carries
PLAN-MODE-ORGANISE E1-E5bis (Organized mode: sectioned Inbox,
Screener, No rules, Set aside, Feed in cards, Paper trail) and the
Innamoramento light + night theme (PLAN-MONA).

**The previous version, 0.13.0** (published **2026-08-29**, bare tag
on `9599b31`, marked Latest, **verified 18/18** and **auto-update
0.12.0 → 0.13.0 proven on BOTH workstations the same day**). It carries
PLAN-RETOURS-12: the two-line message header with recipient
names (directory), the added account reading as connected, the logo
at 28 px, the workspace versions aligned, the instrumented update path.

**The previous version, 0.12.0** (published **2026-08-28**, bare tag
on `a9f93e0`, marked Latest, **verified 18/18** by
`scripts/verify-release.ps1` and **auto-update 0.11.0 → 0.12.0
proven on BOTH workstations the same day**). It carries PLAN-RETOURS-11:
the image guard's memory (per message and per sender,
revocable), the Feedback button and the beta onboarding step, "Made in
EU" in About.

**The previous version, 0.11.0** (published **2026-08-27**, bare tag
on `d0f9c8c`, marked Latest, verified 18/18 and proven on both
workstations the same day — detail further above). It carries PLAN-RETOURS-10:
multi-select (whole thread, D6), the Windows Elements icon, the
header marker at 24 px, nav alignment C.

**An earlier version, 0.10.0** (published **2026-08-25 at 21:02**,
bare tag on `f94a008`, marked Latest). It carries the two jobs from
2026-08-25: **PLAN-REPERE-LIGNE** (the mailbox spelled out on the
line, A80-A82) and **PLAN-ESPACEMENT** (the three air steps, A83).
**Release verified by `scripts/verify-release.ps1 0.10.0` the same
day: 18/18 PASS** — Latest on bare tag, 5 named assets, `latest.json`
with no BOM 1,590 b on BOTH platform keys, URL on bare tag, signatures
== `.sig` and distinct, arm64 exe 200 / 5,632,535 b, x64 exe
200 / 6,350,877 b. **Field proof DONE the same day: auto-update
0.9.0 → 0.10.0 confirmed on BOTH workstations** — the signed
bi-arch chain (ADR 0013/0023) is proven alive in both directions for the
**second consecutive version**. Nothing more is owed on this version.

**The previous version, 0.9.0** (published 2026-08-24 at 16:59, bare
tag on `f135791`, marked Latest). It carries PLAN-ELEMENTS: the entire
"Elements" direction. **Release verified by
`scripts/verify-release.ps1 0.9.0` on 2026-08-25: 18/18 PASS** —
Latest on bare tag, 5 named assets, `latest.json` with no BOM 1,581 b on
BOTH platform keys, URL on bare tag, signatures == `.sig` and
distinct (anti-crossover guard), arm64 exe 200 / 5,629,324 b, x64
exe 200 / 6,351,726 b. **ALL its field proofs are done**
(2026-08-25): **auto-update confirmed on BOTH channels** — the
signed bi-arch chain (ADR 0013/0023) is proven alive in both directions,
as with 0.7.0 — and the **OAuth proof on the second workstation WITHOUT
`setx`**, which **closes ADR 0025**. Nothing more is owed on this version.

**The previous version, 0.8.0** (published 2026-08-23, bare tag on
`a3d04fb`, release **verified** by `scripts/verify-release.ps1
0.8.0` on 2026-08-24: **everything passes** — Latest on bare tag, 5 assets
named, manifest on both platform keys, signatures == `.sig`
and distinct, x64 exe 200 / 6,397,182 bytes). 0.8.0 carries
PLAN-RETOURS-9 (compiled-in OAuth credentials — ADR 0025, "Remove the
account" said, account names).

**The previous version, 0.7.0** (published 2026-08-23, bare tag on
`68384d2`, release **verified** by `scripts/verify-release.ps1
0.7.0` the same day, **18/18 PASS**: Latest on bare tag, 5 assets
named, manifest with no BOM 1,278 b on both platform keys,
signatures == `.sig` and distinct, arm64 exe 200 / 5,668,094 bytes,
exe x64 200 / 6 390 669 bytes; **field proof PER CHANNEL on
2026-08-23: auto-update 0.6.0 → 0.7.0 confirmed on this workstation (arm64)
AND — FIRST ever on the channel — x64 auto-update confirmed on the
second workstation**: the signed bi-arch chain (ADR 0013/0023) is proven
alive in BOTH directions). 0.7.0 carries meeting invitations and
"Delete" per message (PLAN-INVITATIONS, MINOR — decision
D7). **Publication in TWO passes, a paid-for lesson**: a first
run of `make-release.ps1` (night of 2026-08-22 to 23) committed and
pushed the bump then died before the tag; the morning run rebuilt
and signed, but failed on "nothing to commit" — publication
finished by hand, identical to the script (tag `0.7.0` anchored on
`68384d2`, the exact tree of binaries), and the script is now
**resumable** (the empty commit is skipped, publication continues).

**The previous version, 0.6.0 — the FIRST bi-arch release**
(published 2026-08-22, `4a72a53`, CI green run 32584117219; release
**verified** by `scripts/verify-release.ps1 0.6.0` the same day,
**18/18 PASS**: Latest at the bare tag, **5 named assets** (2 exe, 2
`.sig`, `latest.json`), BOM-free manifest 1 581 b for **both
platform keys**, signatures == `.sig` and **distinct** (anti-cross
guard), arm64 exe resolves 200 / 5 504 084 bytes, x64 exe
200 / 6 215 897 bytes; **field proof PER CHANNEL on 2026-08-22:
auto-update 0.5.0 → 0.6.0 confirmed on this workstation (arm64) AND
0.6.0 x64 install confirmed on the second workstation** (decision D5) — the
signed ADR 0013 chain remains proven alive, the x64 channel was BORN
proven at install; its first auto-update will only be observable at
the next release). 0.6.0 carries the three feedback items from PLAN-RETOURS-8
(MINOR, D8), below.

**The previous version, 0.5.0** (published 2026-08-21, release
**verified** STANDARD.md §2.10 on 2026-08-22: Latest, 3 assets at the
bare tag, `latest.json` BOM-free 876 b, URL at the bare tag,
signature == `.sig`, exe resolves 200 / 5 066 813 bytes; **auto-update 0.4.0 → 0.5.0
confirmed in the field**). 0.5.0 carries the four feedback items from
PLAN-RETOURS-7 (MINOR, D6).

**The previously closed job: [PLAN-RETOURS-9](PLAN-RETOURS-9.md)**
(2026-08-23, `19e39cf`, A77-A78 + **ADR 0025**, CE field **6/6** the
same day — zero KO —, CI green run 32647649916, **to ship in 0.8.0**
MINOR, decision D5). Three subjects: (1) **OAuth credentials compiled
into the release** (D1, ADR 0025) — `option_env!("WIND_RELEASE_*")`
set by `make-release.ps1` alone for only the duration of the two
builds (`finally` — the review killed the release that would have
locked itself at pre-push), the runtime variable takes precedence
(dev/e2e), test "a dev build carries nothing", failure message rewritten for
both readers; proof deferred at the time, **done on 2026-08-25**
on the second workstation WITHOUT setx — ADR 0025 closed. (2) **"Remove account"** as icon + text (D2 —
"Delete" refused: nothing is deleted from the server), WCAG 2.5.3
aria. (3) **Custom name per account** (D3/D4): pref
`account_name.{id}` purged on removal via THE
`PREFS_PER_ACCOUNT` constant (the cross-crate hardcoded list is dead), door =
the row's label (no new glyph, A3), 60 characters max
refused never truncated, surfaces: nav, list badge, Settings
(Accounts AND Signature), composer "Name — address"; the name never
touches the `From:`. Fresh-eyes review 8 angles: 10 findings, all
fixed before the field. Reports: DEBT **D-34**. e2e: 121 →
**124**; gate green 2 min 13 s.

**The previously closed job: [PLAN-KAIZEN-CLAUDE](PLAN-KAIZEN-CLAUDE.md)
wave 2** (2026-08-23, `ceb59c4` + `a3ed285`, CE field 3/3 the same
day, CI green run 32642956082) — the technical countermeasures of the
kaizen, order D3, **constant quality** (121/121 e2e, 0 KO in the field).
Figures: **full gate 4 min 34 s → 1 min 43 s** (e2e 256 → 86 s;
rebuild memoized by fingerprint + seed templates copied per spec, 30 min
TTL — the seeders freeze the clock, red paid and fixed); **one
e2e spec 74 s → 13-30 s**; gate in ONE call `scripts/gate.ps1` (9
steps, fail-fast); `retries: 1` (flaky logged, two failures =
andon); docs-only fast path of the pre-push; `scripts/field.ps1` +
`scripts/run-wind.ps1` (the workstation state and the launch traced, PS
5.1, no more one-liners at STOP 2); **nextest rejected on the figure**
(the whole workstation = 9.3 s). Reports: DEBT **D-32** (gate in two
encodings), **D-33** (stale dist held in JS only, not in build.rs).
Rest of the kaizen: wave 3 out of window, PDCA review on 2026-09-06 (D4).

**The previously closed job: [PLAN-INVITATIONS](PLAN-INVITATIONS.md)**
(2026-08-23, `1c159bc`, A76 + **ADR 0024**, **field complete in
FOUR passes on 2026-08-22/23** — each finding fixed in the
session —, CI green run 32605745661, **shipped in 0.7.0** — published on
2026-08-23, verified 18/18, decision
D7). A received meeting invitation is HANDLED in Wind — scope
held: an email feature, NOT a calendar (refused: calendar
view, CalDAV/Graph, event creation, RRULE expansion
beyond "Repeats", COUNTER/delegation). (1) **Pure crate
`mail-ical`** on calcard 0.3.11 (`default-features = false`, spikes
measured — ADR 0024): REQUEST/CANCEL/REPLY, IANA AND Windows TZID
("Romance Standard Time"), guard D1 **per endpoint** — unknown
TZID ⇒ floating time STATED, never a lying conversion;
the responder of a REPLY = first ATTENDEE besides the organizer (Exchange
echo). (2) **Card in the thread**: it travels with the body
(`BodyView.invitation` — zero round trip on open), title,
local time, organizer, location, status, three NEUTRAL gestures with
icons (D4 — "Accept in accent" REJECTED); **cross-cancellation
in BOTH orders** (CANCEL before or after its REQUEST); a
forwarded invitation IS answerable (R8 field: being invited is
not required); the inline calendar part disappears from the chips when the
card is rendered (D3), a named `.ics` remains savable. (3)
**Transactional iTIP reply**: `enqueue_reponse_invitation` —
email and reply in ONE transaction, nothing goes out if the row has
disappeared; MIME `text/calendar; method=REPLY` as alternative;
change of mind allowed (D6); subject in the UI language (D5).
(4) **The list answers** (R10-R12 field): `enrichir_lignes`, a
pass bounded to the PAGE (never the hot request — the
DEEP-SCROLL lesson); gestures at the row rank of chips (windowing generalized to
N rows at constant marginal cost), face-swap of the row when the
reply is posted (R11: the original invitation retakes the front —
the only exception to "last message of the thread"), sum of the thread's
attachments (R12), **instant optimism via `version`** (the windowed
pages are NOT reactive; 3rd/4th passes — write AND rollback).
(5) **Adopting the existing**: one-shot repair
`pieces-calendrier` (`corps-fffd` pattern) — invitations already in
the database gain their card, misaligned attachment indexes from a
legacy database get repaired along the way; `reset_mailbox`/`remove_local`
purge `invitations`. (6) **"Delete" per message** (2nd pass): the
thread bar keeps archive/spam/pin, screen 03 only returns
to the mailbox if the thread closes. Glyphs 76 → **78** (`cancel`,
`question_mark`, `?v=78`, proof **79/79**). Fresh-eyes review 8
angles: 11 findings, all handled before the field. Reports:
DEBT **D-29** (case C: empty body if the invitation is the only
part), **D-30** (legacy invitation without an attachment row), **D-31**
(`drafts` without `ics_reply`). e2e: 117 → **121**; Rust
workspace tests: **547** (new `mail-ical` crate, 16 corpus tests on
real Google/Outlook fixtures).

**The previously closed job: [PLAN-RETOURS-8](PLAN-RETOURS-8.md)**
(2026-08-22, `cbf795a`, A74-A75 + **ADR 0023**, **field complete in
FIVE passes the same day** — 16 R2 findings, each fixed in the
session —, CI green run 32576771340, **shipped in 0.6.0** — the first
bi-arch release, verified 18/18 and proven in the field on the TWO
channels, above). (1) **Account marker** (icon + hue):
DEDICATED set of 12 glyphs (subset 64 → 76, `?v=76`, proof
77/77; A3 held by reservation) + measured swatch **12 families × 2
variants** (measured fact: no single hue holds 3:1 on both
light backgrounds AND `-nuit` — toggle `[data-theme$="-nuit"]`, contrast gate
2 716 → **3 052 pairs**, `tuile` backgrounds included, hex and
inks READ from the shipped CSS); chosen in Settings > Accounts (the
row's icon is the door, a marker only exists WHOLE — transactional
write `set_text_prefs`), replaces `person` in the nav,
badge under the avatar in **unified inbox (D3) and in search**
(always multi-account), tells screen readers; the suffixed prefs
**die with the account** (`delete_account` — the SQLite id
is reused, signature included); consistency gate: the dedicated set,
ONE list across four carriers. (2) **First-launch journey**
in four steps (accounts / layout / theme / summary),
form settled in the field over five passes: title → "Step n/4" →
text, **Continue never grayed out** (absent with no account — D4, hidden
under the generic desk), REAL app screenshots at step 2
(`e2e/capture-onboarding.mjs`, replayable), theme thumbnails in the
CHOSEN layout, summary in side-by-side card-doors
(text above the thumbnails, "Back to this step" veil under A70's
rules); `wind-accueil-fait`/`-commence` marks (localStorage,
V-D4): an existing install is deemed onboarded, an
abandoned journey RESUMES, zero accounts → desk only; seam
`__e2eAccueil` in `lib/accueil.js` (never in the product
decision). (3) **Bi-arch release** (ADR 0023): the x64 channel RETURNS
(removed in 0.1.3) — local cross-build proven (1 min 45 s, `lld-link`
override extended to the x64 triple), `make-release.ps1` with two
`--target` builds **all-or-nothing** (D7), `latest.json` with TWO
keys built per platform + anti-cross guard on signatures
(the silent failure encoded, never left to vigilance),
5 assets derived from `$cibles`, UTF-8 BOM restored (PS 5.1 pitfall);
**new `verify-release.ps1`** (§2.10 scripted ×2 platforms,
checks at the version TAG, failure as verdict — proven on
0.5.0); STANDARD §2.9 (MAJOR evaluated **per channel**) and §2.10 (five
named assets) amended. Fresh-eyes review 8 angles: 10 findings
confirmed, all fixed before the field. Reports: D-10
reopened-reclosed without settlement (A41 order verified on record,
the `prefs.lang` assertion still to write). e2e: 108 → **117**; Rust
tests: mail-core 357 → **358**, wind-desktop 18 → **20**.

**The previously closed job: [PLAN-RETOURS-7](PLAN-RETOURS-7.md)**
(2026-08-22, `2cb9460`, A70-A73, **field complete in two passes on
2026-08-21** — the visual finding fixed in the session —, CI green,
**shipped in 0.5.0**). (1) **Descriptive hover on attachments**:
a veil covers the chip on hover and keyboard focus — `download` glyph
+ "Save" (D1: the product's vocabulary, not
"download") — same geometry, the row does not reflow; never
on an echo's inert chip. (2) **Attachments at the head of the
message**, between the header and the body (the image guard stays
attached to the body); first e2e to lock the DOM order. (3) **Screen 03 flat**
(reverses A46's "screen 03 keeps its full card") — no more
enclosing card, scene in a single flow, 960 px centered column
(D2) — the flat form is now THE only form of the Thread component
in its two frames. (4) **Pin a conversation** (Inbox
only D4, thread bar D3, at the top ONLY D5): local `pins`
table keyed by envelope (survives thread rebuilds;
NEVER `flagged`, overwritten by sync), pinned section
prepended to page 0 — both the paginated flow AND the totals exclude it
(shared exclusion, extended plan guard: the subquery starts from `pins` via
a directive `CROSS JOIN`, otherwise SQLite without ANALYZE scanned the whole
`envelopes` on the hottest path, ~24 ms measured at 200k); the button's
state is SEEDED from the served row (zero round trip on
open); the pinned row carries the nav tile's look
(`--tuile`/`--tuileInk`, field finding fixed the same day); the
empty list requires both sources to answer (flow + pins).
3 new glyphs (61 → 64, `?v=64`, proof 65/65). Reports: DEBT
**D-28** (orphan pin if the key message leaves its mailbox — accepted edge
case). e2e: 103 → **108**; Rust tests 355 → **357**.

**The previously closed job: [PLAN-RETOURS-6](PLAN-RETOURS-6.md)**
(2026-08-21, `13d4bed`, A66-A69, **field complete in three passes the
same day** — each finding fixed in the session —, CI green,
**shipped in 0.4.0**). (1) **Signature per account** (Settings >
Signature): reduced rich editor (B/I/U, ammonia allowlist at THE
boundary), scoped per account ("also replies/forwards", default
new messages only), "Apply to all" copies signature AND scope
visibly; in the composer, insertion at the template (new: under two
blank lines; reply/forward: between the lead-in and the quote) and the
signature **follows the sending account** (recomposable body
template — quote regenerated identically); anti-churn guard `corpsAuto`
(closing without typing seeds NOTHING). Storage `prefs`, no migration.
(2) **Deferred send**: `outbox.send_at_epoch`, due-only filter
IN `outbox_to_send` (single gate), transactional
`enqueue_outbox_full`; cancellation = the WHOLE draft recreated
(attachments with bytes; scheduled not-yet-due entries only — race with a
concurrent flush locked, reviewed); status bar "N
scheduled · departs {when}", notice slot "Cancel send",
departure by a short timer armed by the 10 s probe (~1 s). Local
semantics STATED (D1): it goes out if Wind is open, otherwise at the next
launch. (3) **"Important"**: icon button on the formatting bar
(aria-pressed, tooltip "Mark the message as
important"), columns `drafts.important`/`outbox.important` (toggling
alone advances the timestamp), SMTP headers `X-Priority: 1` +
`Importance: high` (none on ordinary mail). (4) **Composer header**
on `--panel` (Wind's footer). 3 new glyphs
(58 → 61, `?v=61`, proof 62/62). Report: display of RECEIVED
important messages (§ reports). e2e: 99 → **103**.

**The previously closed job: [PLAN-RETOURS-5](PLAN-RETOURS-5.md)**
(2026-08-21, `6f94922`, A65, field complete — five points, point 2
investigated then replayed —, CI green, **shipped in 0.3.0**). (1) **The
temporary Sent entry tells the truth**: the local echo of a send
(PLAN-REACTIVITE E3, mechanism unchanged) showed "To: sent" (the
destination slug served as the recipient by the nav echo
slice) and an empty "Attachments" title (metadata never
fetched back). Now: column `echos.to_addrs` filled at
birth (send: `outbox.recipients`; gesture: `envelopes.to_addrs`),
attachments as name + size from the send log (`echo_attachments`),
INERT chips during the window (D2); the gesture echo no longer shows
an empty section (reviewed). (2) **Address autocomplete** (D3-D5):
contacts directory — dedicated table, NEVER a scan
of envelopes per keystroke (A64 lesson) — learned from mail seen
(senders outside junk/trash with their name, recipients
of our sends), backfilled ONCE on the existing data at open
(142 ms/200k, `prefs` mark); suggestion "Name + address", inserted
as a **bare address** (D3, send path and anti-injection guard
untouched); 22 ms worst case (budget < 50 ms). (3) STATE squared
away again (read-perf turned off D1, "attachment sending" report
removed — shipped since PLAN-PIECES-JOINTES). Reports: **D-27** (the
outbox only retries at cycle end or on gesture — sends never
lost, golden rules held). Traps engraved: STANDARD §9
(`2> file` on the windowed exe launched bare captures nothing — trace via a
launcher that waits). e2e: 97 → **99**.

**The previous version, 0.4.0** (published 2026-08-21, release
**verified** §2.10: Latest, 3 assets at the bare tag, `latest.json` BOM-free
876 b, signature == `.sig`, exe 200 / 5 055 194 bytes;
**auto-update 0.3.0 → 0.4.0 confirmed in the field on 2026-08-21**). 0.4.0
carries the four feedback items from PLAN-RETOURS-6: per-account
signatures, deferred send, "important" marking, composer header.

**The previous version, 0.3.0** (published 2026-08-21, release
**verified** §2.10: Latest, 3 assets at the bare tag, `latest.json` BOM-free,
signature == `.sig`, exe 200 / 5 038 998 bytes; **auto-update
0.2.1 → 0.3.0 confirmed in the field on 2026-08-21**). 0.3.0 carries
**address autocomplete** for To/Cc/Bcc and the **send echo that
tells the truth** (PLAN-RETOURS-5).

**The previous version, 0.2.1** (published 2026-08-20, release
**verified after the fact** STANDARD.md §2.10: Latest, 3 assets at the bare
tag, `latest.json` BOM-free, URL at the bare tag, signature == `.sig`, exe
resolves 200 / 5 014 053 bytes; **auto-update 0.2.0 → 0.2.1 confirmed
in the field on 2026-08-20** — the signed ADR 0013 chain remains proven
alive). 0.2.1 carries the **deep scroll fixed**
(PLAN-DEFILEMENT-PROFOND, FIX §2.9): the list no longer freezes
when the scrollbar is dragged, the empty screen no longer lies,
startup and first displays are immediate. Lesson engraved along the way (STANDARD §2.9 ⚠️,
oversight made three times): **user notes for the CHANGELOG are written
BEFORE `make-release.ps1`** — the script refuses without them.

**The previous version, 0.2.0** (published 2026-08-20, **auto-update
0.1.11 → 0.2.0 confirmed in the field on 2026-08-20** — the signed
ADR 0013 chain remains proven alive; release **verified after the fact**
STANDARD.md §2.10: Latest, 3 assets at the bare tag, `latest.json` BOM-free,
URL at the bare tag, signature == `.sig`, exe resolves 200 / 5 008 012 bytes).
0.2.0 carries the **rich HTML composer** (PLAN-COMPOSITION-HTML,
the first new capability of 0.x → MINOR §2.9) and the **reconnection
of an account with a dead token** from Settings > Accounts.

**0.1.11** (published 2026-08-19, `6977778`,
auto-update **0.1.10 → 0.1.11 confirmed in the field on 2026-08-19** — update
from the app, the signed ADR 0013 chain remains proven alive;
release **verified after the fact**: Latest, 3 assets, `latest.json` BOM-free,
URL at the bare tag, signature == `.sig`). 0.1.11 carries the **three
feedback items** from PLAN-RETOURS-4 (R1-R3, a **FIX** — no new
capability, STANDARD.md §2.9): downloading an attachment via "Save
as" dialog; name + size of an attachment in a single chip; message
bodies always on a light slab (dark themes readable again).

**0.1.10** (2026-08-18, `a25c566`, auto-update 0.1.9 → 0.1.10 confirmed)
carried the four feedback items from PLAN-RETOURS-3 (backfill %; spam /
not spam; delete a draft; reply per message). **Publication
is driven end to end by `scripts/make-release.ps1`**: bump of
tauri.conf.json + signed build (key `C:\Keys\wind.key`, password by
hand) + manifest + — after `YES` confirmation — release commit, push
(gate replayed), bare tag + GitHub Release marked Latest, notes drawn from
the CHANGELOG. Fact proven: `TAURI_SIGNING_PRIVATE_KEY` accepts the **path**
of the key file (not only its content); publication is therefore
**no longer manual** (ADR 0013 described it that way; the script does it,
behind a confirmation).

**0.1.9** (2026-08-17) carried the four feedback items from PLAN-RETOURS-2
(Cc/Bcc; Gmail sync cadence 5→30 min; loading bar; "Make
independent" removal). **0.1.8** (2026-08-16) carried the four
mail fixes from PLAN-RETOURS-MAIL.

**0.1.7** (2026-08-16) remains the line of the whole redesign — the
System v2 "Wada" and its widening (28 themes, PLAN-WADA /
PLAN-WADA-ELARGI), UI v3 and the CE feedback (A44-A47, PLAN-UI-V3 /
PLAN-RETOURS-V3), the three display modes (PLAN-VOLETS), the v1
interface removed (PLAN-RETRAIT-V1), on a window that no longer freezes
(PLAN-GELS, ADR 0019); auto-update 0.1.6 → 0.1.7 confirmed in the field.
All the plans on this line are closed.

**Previous closed job: PLAN-DEFILEMENT-PROFOND** (2026-08-20,
`70e44e3`, A64, field validation complete — three passes the same day —, CI
green, run 32382945877). The field bug of the drag held in Archives
(blocks « .. », then « No messages here. » in ALL folders
for minutes) died at the root: the list was asking for **one
page per position crossed** (~161 calls for 2 s of the bar, measured)
in the serialized queue of `hors_pompe`, and the source change
was showing an unproven blank. From now on: **only one page flight at a
time** (the last window wins, the page 0 of a new source moves
ahead of the gauge), an **honest empty screen** (skeleton until the source
has responded), and — field of the 2nd/3rd passes — **the counts have
left the display path**: the page no longer carries a total (a
short page says its own exact end; `category_total` separate,
asked at rest from the pump), and `nav_snapshot` now only pays for the
two unread the nav DISPLAYS (it recalculated every 10 s eight
counters per account, including the archive total at ~240 ms per probe —
the most expensive computation of the application, dropped). Figures: end-to-end
wait p50 2,408 → **17 ms** (bench `measure-scroll.mjs`,
checked into the repository, decision D2); Archives' first display 253 →
**14 ms** of core (SQL, full archive fixture 200 k). Reports: **D-26**
(deep page O(offset) accepted, decision D1 — ~129 ms at offset
80,000, only one flight, screen that says loading). e2e: 94 → **97**.
Published in **0.2.1** (CORRECTIVE, §2.9).

**Previous closed job: PLAN-COMPOSITION-HTML** (2026-08-20,
`537a1e4`, A62-A63 + ADR 0022, field validation complete, CI green — **delivered
in 0.2.0**). The composer moves to a **rich body end to end**:
`body_html` column next to the text (drafts + outbox, rewindable
migration, NULL on existing rows), `multipart/alternative` send
(text derived from the same HTML by THE single boundary `frontiere_corps`),
Drafts reflection and print alike (a rich draft re-fetched
keeps its formatting), Sent echo in HTML, `blockquote` quoting,
`contenteditable` editor + legacy `execCommand` (output = exact ammonia
allowlist), strict R4 bar (D1-D3: without Link/Quote, generic
families + 4 notches, 12-hue swatch), icons 46 → 58. **Remote images
by gesture (D5, field)**: response with the neutral pixel (an
`AllowRemote` quote loaded spy pixels on Reply click —
reverted), forwarding keeps the images. **Two field findings fixed
the same day (A63)**: reconnecting an account with a dead token
(`invalid_grant`) from Settings > Accounts (`reconnect_account`, identity
guard, dedicated e2e); the disconnect notice leads to Settings.
Fresh-eyes review: 10 findings confirmed, fixed (including three
contenteditable traps engraved in STANDARD §9). Reports: DEBT D-25.
e2e: 92 → **94**.

**Previous closed job: PLAN-DOCUMENTATION** (2026-08-19, `78a2a91`
→ `8cf8ac3`, CI green, field E4: clean cold resumption and stub
test). The documentation is restructured in three kaizen moves:
**the method lives in [STANDARD.md](STANDARD.md)** (numbering
§2-§10 frozen, amended by kaizen, never rewritten), **the state in
this document** (rewritten with every job), the 24 closed plans and
5 phase reviews in [archives/](archives/), the orphan normative content
repatriated to the repository (release verification → STANDARD §2.10, the
hot-cache trap → §9); Claude's memories now carry only machine
facts and pointers. Temporary PASSATION.md stub (D-24: one
clean resumption of the two required, counted).

**Previous closed job: PLAN-RETOURS-4** (2026-08-18, `52aec3e`, A59-A61,
field validation complete, CI green — **R1-R3 delivered in 0.1.11** (`6977778`, auto-update
confirmed on 2026-08-19); **R4 deferred to a dedicated job**, CE decision D1).
Three feedback items, all **corrections / adjustments to the existing** (no
new capability → **CORRECTIVE**, STANDARD.md §2.9). (1) **Downloading an attachment
via dialog**: the click was saving SILENTLY to Downloads (« nothing
happens »); it now opens the native « Save As » — folder AND
name chosen, default Downloads + sanitized name; new command
`chemin_enregistrement_suggere` (the name comes from the UI; `safe_file_name` remains
the sanitizing authority), `save_attachment(dest)` writes to the chosen path,
`dialog:allow-save` capability, e2e seam `__e2eDestination` (A59). (2) **Name
+ size of an attachment in the SAME chip**: reading aligns with the composer,
an accepted exception to « 1 chip = 1 piece of information »; `storage` glyph withdrawn
from use, kept reserved for the subset (prior A53, A60). (3) **Body
always on a light slab**: measured in the field that only text with sender
COLORS (newsletters designed for a white background) was black on dark — the
color-less text was already legible — the body now bakes
`mail_render::Palette::default` (white background) whatever the theme, like
mature clients, **reversing A42's dark slab**; the front no longer transmits a
palette (`paletteLecture` removed, `palette` params of
`message_body`/`echo_body` removed — A61). Report: DEBT D-23. **Trap engraved
(A61)**: NEVER re-transmit a theme palette to a message body —
the body is intentionally light everywhere (sender text is composed
for a white background); e2e guard « the body stays on a light slab even under a
dark theme ».

**Previous closed job: PLAN-RETOURS-3** (2026-08-18, `8819090`, A55-A58,
field validation complete, CI green — **delivered in 0.1.10**, auto-update confirmed in the
field on 2026-08-18). Four field feedback items. (1) **Backfill
percentage**: the status bar now shows « N remaining · P % »; `P` = bodies
present / corpus in scope, pure function `backfill_percent` (sister of
`sync_percent`, capped at 99 as long as a body is missing), denominator
`bodies_total_count`; the % lives in the TEXT (A55). (2) **Spam / not spam**:
`report_spam`/`mark_not_spam` reuse `MoveTo` — the junk
folder is resolved per account (`canonical_folders`), the provider is the one that learns;
per-thread gesture, unavailable if there is no Junk folder (A56). (3) **Deleting a
draft** from composition — destructive gesture with inline confirmation,
distinct from « Cancel » which keeps it (A57). (4) **Reply per message**:
Reply/Reply-all/Forward move to the bottom of each message; the thread
bar now keeps only sort + spam. **Field finding fixed the same day**:
the 3 gestures on our OWN messages too — replying then targets the
original recipients (To for Reply, To+Cc for Reply-all; pure
function `reply_to`), never oneself (A58). Reports: **D-21** (double COUNT of
the backfill, family D-8, budget held in the field), **D-22** (report_spam
already-spam via search). Trap confirmed: the field trace (`… 2> file`)
fails if the path doesn't exist — the « Desktop » is redirected under OneDrive
(`C:\Users\<u>\Desktop` absent); write at the repository root.

**Previous closed job: PLAN-RETOURS-2** (2026-08-17, `dfa6224`, A52-A54
+ ADR 0021, field validation complete, **delivered in 0.1.9**). (1) **Gmail sync « too
long »**: measured in the field (`run_sync` trace, ~135 s in release when
22 Gmail views moved — ~5 s per changed folder, likely throttling). The
sobriety principle (ADR 0017) holds; it was the **cadence** that was costly. With the
IDLE watcher (ADR 0018) holding INBOX in real time, the **full cycle
goes from 5 to 30 min** (+ a light INBOX pass every 5 min as a net) — S-D4
settled, **ADR 0021**. All Mail STAYS synchronized (Archives intact, ADR
0010 preserved); excluding virtual views is deferred (STANDARD.md §2.6). (2)
**Loading bar**: the « percentage » mode (frozen under Chromium,
A40) dies; the bar now runs its full loop as soon as any action runs, the
% stays in the TEXT (A52). (3) **« Make independent » removed** —
inert placeholder, multi-window deferred to a dedicated job (A53). (4)
**Working Cc/Bcc** — slice compose→Draft→outbox→SMTP→UI; **Bcc
in the SMTP envelope ONLY** (`send_raw`, never a Bcc header served),
« Reply all » puts the original Cc back in Cc (`reply_all_split`);
local drafts carry cc/bcc (A54). Trap paid for: the **release** app is
a *windows* subsystem → `eprintln` **silent** in console (measure in
debug or `2> file`).

**Previous closed job: PLAN-RETOURS-MAIL** (2026-08-16, `19ea16a`,
A48, field validation complete, delivered in 0.1.8). Four CE feedback items on
real mail: subjects/names stripped of IMAP `quoted-string`
escapes that `imap-proto` leaves behind (fix + migration of existing rows),
the « Sent » folder finally showing the real recipient, instant « Reply to
all », and the `<head><title>` of certain newsletters no longer
leaking at the top of the body. **State fact to remember: the stored envelope
now carries the recipients** (`envelopes.to_addrs`/`cc_addrs`,
pulled from the same ENVELOPE as the sender) — the earlier « the envelope
carries only the sender » is reversed; `reply_all_context` reads them
first (offline), the server poll is only a fallback. Reports:
DEBT D-15/D-16.

### The state of the field — figures from 2026-07-26, real mailbox

The full synchronization (ADR 0010) brought everything back: **256,312
messages** (7,539 before), 4 accounts, all folders — spam and trash
included, an explicit Chief Engineer decision.

**The header pass has converged to zero**: `diagnostic_fils` shows
`never read: 0` within the grouping scope. This figure is final —
nothing is still moving on the thread side. Grouping result:

| | before ADR 0009 | before ADR 0010 | **final** |
|---|---|---|---|
| threads of 2 to 5 | 15 (all combined) | 242 | **577** |
| threads of 6 to 20 | — | 6 | **35** |
| threads of more than 20 | — | 0 | **1** |

**The scope holds at scale**: 248,771 out-of-scope messages created no
thread and surfaced no conversation — this is the
STANDARD.md §6.9 invariant, held by test.

**What's still moving: the body backfill.** ~250,000 messages
await their body, at 200 per batch, along with usage — a long
tail of several days or weeks, resumable, visible in
the app's ochre banner. **The database will grow toward ~13 GB**
(256,312 × ~50 KB); the « < 1 GB » budget is lifted (ADR 0010 §2) and
the disk-space guard watches before every commit.

**First reflex of a new session:** ask the user where
the backfill banner stands and how much
`%APPDATA%\dev.elements.wind\wind.db` weighs (with its `-wal` and
`-shm` companions). Reminder of STANDARD.md §7.1: you cannot read its database yourself.
(Before PLAN-WIND E3: `dev.discovery.app\discovery.db` — the move
is automatic on Wind's first launch.)

### Budgets not held, with their remedy

| Item | Measurement (2026-07-26) | Lever |
|---|---|---|
| Deep page of a category (outside inbox) | ~129 ms at offset 80,000 (core only, 200 k fixture, 2026-08-20) | **accepted** (CE decision D1, DEBT D-26): O(offset) index scan; only one flight at a time, screen says loading, count out of the path (A64) — to reopen if the field exceeds ~1 s per page |
| Adopting a legacy database | 3.66 s at 200,000 messages, once | **settled by shape, via ADR 0012**: visible, cancelable, rewindable — the duration is accepted, the pass is one-time |
| Search | ~~113–210 ms~~ → **~66 ms ✅** (2026-08-17) | **settled**: `prefix='2 3'` + indexed recipients + **date-sort valve armed** beyond 10 k matches (`WIDE_QUERY_THRESHOLD`); measured on the REAL database (251 k / 7 GB), worst case a 3-char. prefix (36 k matches) at ~66 ms (PLAN-RECHERCHE, A50) |

The search budget is **held in the field**. Lesson from the field: the
wall is not the render ceiling (hydration costs only ~0.2 ms/row)
but the **BM25 floor** — ranking 36 k matches for a 3-char. prefix
takes ~80 ms, whatever the cap, and this floor rises with the corpus.
The date-sort valve of ADR 0004 solves it: beyond 10 k matches,
`search_capped` sorts by date (the best order for such a broad query
anyway), ~66 ms. Now indexed: recipients (`to:`/`à:`),
the most common relevance gap.

### Trade-offs — settled and open

**Settled** (do not reopen without a measurement):
- ~~Synchronize the archive?~~ → **Everything is synchronized** (ADR 0010),
  spam and trash included, no quota. The question is settled.
- ~~Scope of Phase 5?~~ → The visible and interruptible migration
  first — **done** (ADR 0012). Next, in order: installer,
  telemetry, beta.
- ~~OAuth credentials for the distributed app?~~ → **CLOSED on 2026-08-25**
  (ADR 0025, decision D1 of PLAN-RETOURS-9). The client ids are
  compiled into the release by `make-release.ps1` alone
  (`option_env!("WIND_RELEASE_*")`, all-or-nothing); the runtime
  variable keeps priority in dev and in e2e, and the
  `dev_builds_embed_no_credentials` test screams on a poisoned build. **The
  field proof that closed the trade-off is done**: an account
  connected on the second workstation from a published release, with no
  `setx`. It slipped two versions — expected at 0.8.0, came
  after 0.9.0.

**Open** (to the Chief Engineer):
- **Search without a practical limit** (2026-08-17) — the cap itself is
  settled (`SEARCH_LIMIT = 100`, « N of M » bar with the real total;
  A50/PLAN-RECHERCHE). What remains open is only the true « see it all »: a result
  list **virtualized + cursor pagination** (the wall: `SELECT_UNIFIED`
  hydration per row + unwindowed list). Separate job.
- **Search sorted by date** — **armed** in the field (2026-08-17): the
  BM25 floor for a very broad 3-char. prefix (36 k matches) exceeds the budget
  whatever the cap. `search_capped` switches to date beyond
  `WIDE_QUERY_THRESHOLD` (10 k matches); below it, BM25. Threshold set for this
  machine — to be re-measured if the budget tightens in beta.
- **Multi-mailbox duplicates in search** — observed in the field: the
  same message lives copied in several mailboxes (« 19 messages share
  a Message-ID »), and search returns each copy. Deduplicate at
  display time? To observe in real usage before deciding (D2, kept open).

### Next — Phase 5

Hardening and beta ([PLAN.md](PLAN.md) §4). Order settled: visible and
interruptible migration **✓ done (ADR 0012)** → signed installer + update
**✓ done (ADR 0013)** → opt-in crash telemetry **✓
done (ADR 0014)** → **closed beta 20-50 users (next)**.
Gate 5: two weeks with no critical defect.


---

## What remains

### The job done: visible and interruptible migration (ADR 0012)

Finished and **field-validated** on 2026-07-26, on copies. Adoption
is a single transactional unit (from the conditional DROP of the thread
tables to `user_version`): canceling rewinds everything, the pass
replays entirely on next launch. Modal screen at startup — each
command opens its own connection, without a gate the first arrival
would pay for the pass silently. Proof: rewind test on a
real file database, bench (3.66 s, no regression), cancellation
exercised mid-pass at gate 3 scale.

### The job done: NSIS installer + signed update (ADR 0013)

Finished and **field-validated** on 2026-07-26: the 0.1.1 → 0.1.2 loop
applies on the installed app, database intact. **Re-validated on 2026-08-16 at
the scale of the redesign**: the 0.1.6 → 0.1.7 auto-update (the whole
redesign) applies on the installed app, signed chain proven alive.
NSIS (**not MSIX** — it
would virtualize `%APPDATA%` and orphan the database); Tauri updater
signed with minisign, driven from Rust (capabilities at a minimum); Windows code
signing deferred to beta. Publishing a version:
`scripts/make-release.ps1 <version>` does the WHOLE release — since
PLAN-RETOURS-8/ADR 0023 in **bi-arch** (native arm64 + cross x64,
all-or-nothing, 5 assets, two-key `latest.json`, tag = bare
version); scripted verification by `scripts/verify-release.ps1`.

### The job done: local, opt-in crash telemetry (ADR 0014)

Finished and **field-validated** on 2026-07-26. Local file only
(no network, no third party), backend panics only, opt-in off by
default; the **panic message is stripped** (the only vector of personal
data), proven at two levels (memory and written file). The hook
never touches the database (consent in a file + `AtomicBool`).
Field finding fixed: a crash on the main thread produces a
**double panic** at WebView2's FFI boundary — `SEQ` counter (unique
names) + filter of the secondary `cannot unwind`.

### The job done: no more commands on the main thread (ADR 0019)

Finished and **field-validated** on 2026-08-15 (PLAN-GELS, `e32280b`,
A39/A40). The startup freeze (25.2 s of cumulative freezes over 40 s,
measured) died at the root: every blocking command goes through
`hors_pompe()` — spawn_blocking + global lock, the earlier
serialization kept — held by the `main-thread-guard.mjs` gate and the
« no pump freeze > 150 ms » budget (`freeze-probe.py`). Along the way, the
field surfaced and got fixed the same day: progress frozen at
99% by pending replay departures (the denominator adjusts), and
the stillborn hitofude stroke loop (CSS animation in an unrendered
`<mask>` → SMIL). Open debt: D-8 (expensive probes, off pump).

### The next job: closed beta with 20-50 users

Last step before gate 5 ([PLAN.md](PLAN.md) §4). Weekly kaizen
on **observed** friction. **Started on
2026-08-28** (PLAN-RETOURS-11 R3): actions in
[PLAN-BETA.md](PLAN-BETA.md), guide in [BETA.md](BETA.md), in-app feedback
channel (Feedback button, A91) — remaining blocker: getting
`feedback-wind@fcts.io` to receive mail (fcts.io alias, on the CE side).

### The long tail in progress

The full body backfill (~250,000 messages remaining) proceeds at
200 per batch along with usage. Nothing to code; watch the disk and the
banner. Search deepens as it goes.

### Accepted deferrals

- **Expensive queries from periodic probes** (PLAN-GELS D4): off
  the pump they no longer freeze anything, but their CPU cost stays real —
  register **D-8** of [DETTE.md](DETTE.md), figures and leads inside.
- **Multi-mailbox duplicates in search** (new, ADR 0010): the
  same message copied in several mailboxes surfaces several times in
  the results. To observe in beta before deciding on deduplication.
- **Search sorted by date** — **armed** (A50, PLAN-RECHERCHE): beyond
  10 k matches (`WIDE_QUERY_THRESHOLD`), ranking switches to
  date, since the BM25 floor would otherwise exceed the budget. No longer a deferral.
- **Deep scrolling of the LIST** — the FAILURE is dead
  (PLAN-DEFILEMENT-PROFOND, A64, 2026-08-20): queue bounded to ONE flight,
  honest empty screen, counts out of the display path
  (`category_total` separate, nav lightened to its two unread). Still
  ACCEPTED (CE decision D1, DEBT **D-26**): the O(offset)
  index scan of a deep page outside the inbox (~129 ms at offset
  80,000 out of 200 k, core only) — only one flight at a time, the screen says
  loading; to reopen if the field exceeds ~1 s per page.
- **Display of important RECEIVED messages** (flag, sort, filter) —
  explicit scope refusal of PLAN-RETOURS-6: the composer sets
  priority headers on SEND; reading them on incoming messages
  is a separate job. Useful finding: Gmail web
  shows NO indicator at all for `X-Priority`/`Importance` (a homegrown
  algorithmic marker) — Outlook/Thunderbird show the « ! »;
  the header is checked via « Show original ».
- **Orphan pin if the key message leaves its mailbox** (DEBT
  **D-28**, PLAN-RETOURS-7): the pin is carried by the gesture's own
  envelope alone — a third party deleting exactly that message
  makes it vanish silently. Accepted edge case; never a false display
  (the join discards orphans). To reopen if the field or the
  beta reports pins that « vanish ».
- **« Has an attachment » filter**. (**Sending attachments**
  is DELIVERED — PLAN-PIECES-JOINTES closed, `38cd812`/`27ed056`; the
  **`to:` in search** is DELIVERED — A49.)
- **Real CONDSTORE, IDLE/push** — Phase 1 deferrals unchanged.
- **Google CASA folder** — critical path for the public launch, on the
  product-owner side.

### Known debt, not fixed

`apps/desktop/ui/style.css`: the element rule `header { display: flex }`
also applies to `#detail-header`. Any full-width child added there
becomes a flex item collapsed to 0 px. (The ADR 0010 progress banner
and the ADR 0012 migration screen were placed **outside**
of any `<header>` for this reason.)

Cousin of this debt, now **held by a rule**: any ID rule that
sets a `display` overrides the browser's `[hidden]` and requires
its guard `#id[hidden] { display: none }`. Eight occurrences to
date; the latest (`#detail`) let the sandboxed iframe capture the
first click and kill keyboard shortcuts (STANDARD.md §9). An E2E holds the case.

### Phase 5

MSIX/NSIS installer + signed update, opt-in crash telemetry,
closed beta with 20-50 users, weekly kaizen on **observed**
friction. Gate 5: two weeks with no critical defect.

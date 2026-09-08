# PLAN-BATCH-2026-09 — docs cleanup, the "What's new" window, feedback pictures, and the next action

> **JOB CLOSED on 2026-09-08 — full field validation.** STOP 2 the
> same day, Chief-Engineer verdict **"All ok"** (workstation's real
> database, run-wind.ps1 launch: the What's-new window on first
> launch, absent on relaunch, the feedback pictures and the
> three-image cap, the cleaned docs root — zero KO findings). One
> commit `6aa2a49` (50 files, +1360/−81), **CI GREEN run
> 34229853815**. Full gate GREEN twice (283 e2e, 0 flaky), two andon
> stops on the way (both the language ratchet: a French word in a test
> fixture address, then the plan file's own bare Chief-Engineer
> abbreviations — the pre-push hook caught the second, exactly its
> job). System A138/A139.
> Kaizen: 2 full green gates (+2 andon-stopped at the ratchet), 0 KO
> findings at STOP 2, review 8 angles / 8 findings fixed / 2 accepted.

> Statement (Chief Engineer, 2026-09-08): *"1 Clean the /docs folder by
> archiving and updating what needs to be. 2 Implement a feature that
> adds a 'What's new on Wind version X.Y.Z' window after updating the
> app. Inside this window is the changelog for the new version.
> 3 Allow attachments of pictures (jpg, png) on the feedback form.
> 4 Execute the next action in the PLAN.md."*

Opened: 2026-09-08. Baseline: `7ef8975` (main, clean, 0.20.0 published).
Method: [WORKFLOW](WORKFLOW.md), [job](../.claude/skills/job/SKILL.md).

---

## 1. Finding (what was verified on the evidence)

### Item 1 — the docs folder

- **Ten closed plans and two closed audit reports still sit at the
  root** of `docs/`: PLAN-ENGLISH-SWITCH (closed 09-04), PLAN-AUDIT-V3
  (09-04), PLAN-RETOURS-15 (09-04), PLAN-MACOS (09-05),
  PLAN-APPLE-SILICON (09-06 — one field proof still open: the
  Rosetta-or-native check on T2/T3), PLAN-AUDIT-2026-09 + LOT2–LOT6
  (09-08), PLAN-SWEEP-2026-09 (09-08), AUDIT-2026-09-01,
  AUDIT-2026-09-06. STANDARD's map says `docs/archives/` = "closed-out
  plans and phase closing reviews"; nothing automated the move.
- **HANDOVER.md** is the D-24 stub; its removal condition ("two
  consecutive clean cold resumptions") has been met many times over —
  every session since 2026-08-19 resumes on STATE.md directly.
- **PLAN.md §9 "Immediate next actions"** is the founding list
  (restructure into a workspace, launch the Phase 0 spikes, create the
  cloud projects, interviews, CI) — all five delivered long ago. The
  living queue is STATE.md's "Next, in order". The rest of PLAN.md is
  the concept paper, still normative (STANDARD's map lists it as
  "product source of truth").
- Open/living documents that stay put: STANDARD, STATE, DEBT,
  GLOSSARY, WORKFLOW, MACOS-BUILD, BETA/BETA.fr (tester guides),
  PLAN.md (concept paper), PLAN-BETA (running), PLAN-KAIZEN-CLAUDE
  (standing kaizen).

### Item 2 — "What's new" window after an update

- `CHANGELOG.md` (repo root, English, Keep a Changelog form) is the
  only changelog; no machine-readable variant.
- The updater (`commands.rs:6922-7281`, ADR 0013): `update_install`
  ends in `app.restart()`; **there is no post-restart hook and no
  "last run version" persisted anywhere** — the feature needs a new
  `prefs` key (SQLite `prefs` table, `crates/mail-core/src/store/prefs.rs`,
  globals declared as constants there) and a typed IPC pair, mirroring
  `lang_get`/`lang_set` (never a generic prefs command).
- Natural trigger: `App.svelte`'s startup path (`onMount`, right where
  `checkUpdate()` already runs). The frontend has no version constant;
  it always asks Rust (`app_version`).
- The consistent delivery of the changelog text is Rust-side:
  `include_str!` the CHANGELOG into the binary and expose one command
  that returns the parsed `## [X.Y.Z]` section for the running version
  — the webview never touches the filesystem (same doctrine as the
  updater comment at `commands.rs:6925`).
- Modal template exists: `MigrationModal.svelte` + the `use:modal`
  action (focus trap, aria, scrim); `ConsentNotice` is the lighter
  one-time-notice precedent. New strings go in **both** catalogs
  (`catalog.en.js`/`catalog.fr.js`), gated by the catalog-diff e2e.

### Item 3 — pictures on the feedback form

- `Feedback.svelte` sends via `queue_send` → `queue_send_content` →
  outbox → `mail-smtp`. **The MIME layer already does attachments
  end-to-end** (`multipart/mixed`, `file_part`, lettre base64); the
  gap is only at the IPC boundary — `queue_send` has no attachments
  parameter, and Feedback has no draft to hang `attach_files` on.
- Reusable, already-proven pieces: `chooseFiles()` (Tauri dialog,
  `transport.js:135` — accepts a `filters` option not used yet),
  `read_attachment_files` (absolute-path + regular-file + growth-capped
  reads, `attachment_file.rs`), `mime_for_name` (jpg/jpeg/png already
  mapped), the 25 MB `MAX_ATTACHMENTS_BYTES` budget enforced in the
  core, and the `Permanent` refusal on purged bytes.
- Extension filtering today is cosmetic (dialog filter) — a jpg/png
  allowlist must be enforced Rust-side on this new path.
- System journal: the feedback feature is A91; the journal is at A137
  — this job takes the next numbers.

### Item 4 — "the next action in PLAN.md"

PLAN.md's own next actions are all delivered; the living queue
(STATE.md) reads, in order: **(1) `/field` on beta T2's P0 — sync
stalled at 72 % — on the tester's trace, with T3 as witness**; (2) the
backlog bugs needing other machines (86, 100, 109); (3) the
Rosetta-or-native check on T2/T3. Action (1) is blocked on material
only the Chief Engineer can supply (T2's `~/Library/Application Support/Wind/`
trace). Which "next action" is meant is a Chief Engineer decision (D8).

## 2. Scope

- **E1** — docs cleanup per D1–D3.
- **E2** — "What's new" window: prefs key + IPC + changelog parser
  (Rust, TDD), modal (Svelte, both catalogs), e2e; early visual STOP.
- **E3** — feedback pictures: image-filtered picker, chips in the
  form, `queue_send` extended with an attachment-paths parameter,
  jpg/png allowlist + 25 MB budget Rust-side (TDD), e2e.
- **E4** — quality: `/code-review high`, full `/gate`, STOP 2.
- **E5** — documentation (System A-n, STATE, DEBT if any), commit,
  push, CI watch.

### Refusals (what we do not do, and why)

- No changelog browser / history window — the window shows only the
  section of the version just reached; the full history is the file.
- No "What's new" on a fresh install — the first run seeds the
  last-seen key silently; the window is an *update* artifact.
- No paste-from-clipboard or drag-and-drop images on feedback — the
  picker is enough for a beta feedback loop; either can come as its
  own job on observed friction.
- No arbitrary file types on feedback — the statement says pictures;
  jpg/png only, enforced Rust-side.
- No parallel machine-readable changelog — one source (CHANGELOG.md),
  parsed; a second source would drift.

## 3. Design

### E2 — What's new (design sketch)

1. `PREF_LAST_SEEN_VERSION` in `prefs.rs`; commands
   `whats_new_check` (compares the pref to `package_info().version`;
   empty pref → seed with current, return `None`; equal → `None`;
   different → `Some { version, notes }` with `notes` = the parsed
   CHANGELOG section, falling back to an empty string when the section
   is missing) and `whats_new_ack(version)` (writes the pref). Both
   off the pump (ADR 0019), registered in `generate_handler!` +
   capabilities, passing the IPC-contract gate.
2. Changelog section parser: a pure function in the desktop app
   (input: changelog text + version; output: the section's markdown
   body), unit-tested RED→GREEN on the real `CHANGELOG.md` embedded
   via `include_str!`.
3. `WhatsNewModal.svelte` on the MigrationModal pattern: title
   "What's new in Wind X.Y.Z", the section rendered as simple
   markdown-to-HTML (headings + bullets only — same restraint as the
   rest of the UI), one "Continue" button → `whats_new_ack`.
   Triggered from `App.svelte` startup after `ready`.
4. E2E: with the throwaway DB, call `whats_new_ack('0.1.0')` (seam:
   plain IPC, no new `__e2e` hook needed), reload, assert the modal
   shows the running version and the changelog text, dismiss, reload,
   assert it does not return.

### E3 — feedback pictures (design sketch)

1. `chooseFiles({ filters: [{ name: 'Images', extensions:
   ['jpg','jpeg','png'] }] })` — extend the helper to pass filters.
2. Chips under the textarea (name + size + remove), no preview.
3. `queue_send` gains an optional `attachmentPaths: string[]`;
   `queue_send_content` reads them via `read_attachment_files`,
   **refuses any extension outside jpg/jpeg/png on this path**, holds
   the 25 MB budget, and threads the parts into the outbox draft the
   same way Compose's attachments reach `mail-smtp`. Rust tests
   RED→GREEN on the allowlist and budget; the SMTP layer is already
   proven and stays untouched.
4. E2E on the redesign-screen02 feedback spec's pattern: attach a
   fixture png, send, assert the outbox row carries the attachment;
   assert a `.txt` path is refused.

Set-based note: the alternative for E3 (anchor a real draft and reuse
`attach_files` wholesale) was weighed and set aside — it drags the
draft-edit machinery (draftId, editToken, chips store) into a form
that has no draft; the chosen path reuses the same read/budget/MIME
helpers with a fraction of the surface. No spike needed: no hard
point, all pieces measured in place.

## 4. Steps and gates

| Step | Content | Gate |
|---|---|---|
| E1 | Docs cleanup per D1–D3 | markdown-links gate; `docs:` commit |
| E2a | Prefs key + commands + parser (TDD) | Rust tests green |
| E2b | Modal + catalogs + startup wiring | **early visual STOP (Chief Engineer)** |
| E2c | E2E what's-new | spec file green |
| E3a | Picker filter + chips + IPC extension (TDD) | Rust tests green |
| E3b | E2E feedback attachment | spec file green |
| E4 | `/code-review high`, then full `/gate` | all green |
| E5 | System A-n, STATE, PLAN updated; commit, push, CI | CI green |

## 5. § Chief Engineer decisions

- **D1 — Archive the closed plans?** Move the ten closed plans + two
  audit reports to `docs/archives/` (English, no banner needed — D-58
  concerns the French era), fixing inbound links. Sub-call:
  PLAN-APPLE-SILICON still carries one open field proof (Rosetta
  check) — archive it too, or keep it at root until the proof lands?
  *Recommendation: archive all; the open proof is tracked in STATE.*
- **D2 — Remove HANDOVER.md (pays D-24)?** The removal condition is
  met. *Recommendation: remove, strike D-24.*
- **D3 — PLAN.md §9?** Replace the stale founding list with a short
  pointer: "the living queue is STATE.md". *Recommendation: yes.*
- **D4 — What's-new presentation**: blocking modal on first launch
  after an update (recommended — matches "window" in the statement),
  or a dismissible banner like the update notice?
- **D5 — What's-new language**: the CHANGELOG is English-only. Show
  it as-is in both UI languages (recommended — one source, no standing
  translation cost per release), or start maintaining a French
  changelog too (a per-release cost forever)?
- **D6 — Feedback attachment limits**: jpg/png only, 25 MB total
  budget reused, no count cap (recommended), or a tighter cap?
- **D7 — GO for the E3 IPC shape**: extend `queue_send` with an
  optional attachment-paths parameter (recommended), versus anchoring
  a real draft and reusing `attach_files`.
- **D8 — Item 4, "the next action"**: PLAN.md's own list is
  exhausted; STATE.md's next is the `/field` on T2's P0 (sync stalled
  at 72 %) — blocked on T2's trace, which only you can supply. Does
  item 4 mean: (a) run that `/field` when you provide the trace
  (recommended — it opens as its own `/field` session, not inside this
  batch), (b) another action you have in mind, or (c) nothing beyond
  items 1–3 for now?

### Answers (STOP 1, 2026-09-08)

- **D1**: "Archive all except APPLE-SILICON" — PLAN-APPLE-SILICON
  stays at root until the Rosetta-or-native proof lands; the other
  nine plans + two audit reports move to `docs/archives/`.
- **D2**: "Remove it" — HANDOVER.md deleted, D-24 struck.
- **D3**: "Replace with pointer" — §9 becomes a pointer to STATE.md.
- **D4**: "Modal".
- **D5**: "Bilingual changelog" — a French changelog is maintained
  alongside the English one; the modal picks by UI language.
  *Execution note: `CHANGELOG.fr.md` is created seeded with the
  translated 0.20.0 entry (the only one the window can show today);
  future releases add both entries (§2.9 amended). A missing French
  section falls back to English.*
- **D6**: "25 MB, 3 images cap" — jpg/png only, the shared 25 MB
  budget, and at most three images, enforced Rust-side.
- **D7**: "Extend queue_send".
- **D8**: "T2 /field when trace arrives" — item 4 executes as its own
  `/field` session once the Chief Engineer supplies T2's trace; nothing more in
  this batch.

**GO recorded — the eight decisions settled 2026-09-08.**

## 6. Delivery record (2026-09-08)

- **E1 delivered**: eleven closed docs + two audit reports moved to
  `docs/archives/` (APPLE-SILICON kept at root per D1), HANDOVER.md
  removed (D-24 struck), PLAN.md §9 now points at STATE.md, every
  inbound link rewritten (STATE, STANDARD, GLOSSARY, DEBT, 8 ADRs,
  evidence README, CHANGELOG, architecture page), the language
  baseline refreshed (3 stale entries dropped, total 1735 → 1662).
- **E2 delivered**: `whats_new.rs` (embedded bilingual changelogs,
  pure section parser, `decision()`), `prefs.whats_new_seen`,
  commands `whats_new_check`/`whats_new_ack`, `WhatsNewModal.svelte`,
  `CHANGELOG.fr.md` (0.20.0 translated; ratchet exemption per D5,
  GLOSSARY §1 + STANDARD §2.9 amended), catalog keys in both
  languages. TDD: grouped RED shown (5 parser tests + compile
  refusal), then GREEN; e2e `whats-new.spec.js` (3 shapes). Release
  net added: a version without its English entry fails the tests.
- **E3 delivered**: image-filtered picker, chips on the feedback form,
  `queue_send` gains `attachmentPaths` (jpg/png, 3 max, Rust-side),
  `enqueue_outbox_full` journals in-memory files in the same
  transaction. TDD: RED on the core tests, then GREEN; e2e
  `feedback-pictures.spec.js` (send + refusal + cap — the net proven
  by its negative legs). E2e specs were written after the UI
  increment (the Rust waves carried the strict RED); noted honestly.
- **Early visual STOP**: two captures sent to the Chief Engineer 2026-09-08 (the
  modal and the form with chips); one defect seen at the pass — the
  bold lead ran into its text — fixed before the captures went out.
  Verdict rides STOP 2.
- **E4 review** (`/code-review high`, 8 angles, 10 findings): FIXED —
  the debut window swallowed for the existing fleet (absent pref now
  reads as "update" when accounts exist; e2e fixtures excluded by the
  `WIND_DB_PATH` guard, `update_check`'s idiom), editToken+pictures
  silently dropped (now refused), the combined draft+files 25 MB hole
  (budget now counted inside the transaction over both channels),
  `Some("")` masking English notes (heading-only = no section),
  downgrades reopening the window (numeric compare, newer-only), the
  duplicated picture read loop (reuses `read_attachment_files`), the
  architecture page's dead HANDOVER links, fmt. ACCEPTED — the notes
  parser is scoped to the house changelog idiom; the one extra store
  open at startup (batch into `ui_state` if probes consolidate again).
- **D8**: nothing executed here; the T2 P0 `/field` opens in its own
  session when the Chief Engineer supplies the trace.

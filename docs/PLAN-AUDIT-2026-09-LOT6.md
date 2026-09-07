# Lot 6 — Documentation, comments and reconciliation

Opened: 2026-09-07. Baseline: `46c1463` (Lot 5 E15 committed locally on
top of E13/E14; Lots 3–5 neither pushed nor closed — D0 of Lot 5 stands).
Parent: [audit program](PLAN-AUDIT-2026-09.md), E16.
Method: [WORKFLOW](WORKFLOW.md), [job](../.claude/skills/job/SKILL.md).

**Status: STOP 1 passed on 2026-09-07 — D0 "Confirmed, no push",
D1 "Archive the history", D2 "Refresh in place, dated cells",
D3 "Resolve by record", D4 "Keep decision citations" (all as
recommended). E16a–E16e implemented; fresh-eyes review done (three confirmed findings fixed: the D-54 duplicate, this plan’s own ratchet markers, the 265.2 MB misattribution); full gate GREEN, exit 0 in 681 s — 927
Rust, 43 Node, 266 e2e passed, 1 optional bench skipped, 2 known flaky
(`redesign-feedback-3`, the recorded in-suite flake, and
`organized-mode` green on retry), ratchet 1974 → 1735 markers (baseline
updated the sanctioned way), 428 links live. STOP 2 on 2026-09-07:
the Chief Engineer replayed the ratchet and the links net and read the
checklist — verdict "OK", zero findings. Committed locally (no push,
D0).**

## 1. Findings

Read-only reconnaissance on this workstation, 2026-09-07, three agents
then hand-checked at HEAD. E16 is the audit's consolidation step: every
C finding verified against the code as it stands after Lots 1–5, every
debt entry against its evidence. Nothing here changes behavior.

### C01–C08, verified one by one

| Finding | Verdict at HEAD | Evidence |
|---|---|---|
| C01 (sanitize boundary comments) | **FIXED** (Lot 1) | Reading and editor policies split (`sanitize.rs:40-46,163,222-223`, `style.rs:1`); `received_html_cannot_supply_forward_authority` (`sanitize.rs:350`) proves the incoming marker is stripped. |
| C02 (save/send promises) | **HALF FIXED** | Compose close now refuses to hide a failed save, two e2e nets (`composer-integrity.spec.js:63,130`). Remaining: the `queue_send` doc (`commands.rs:3785-3786`) still says "BEFORE any network attempt" while forward preparation fetches the source attachment from IMAP first (`fetch_source_attachment`, `commands.rs:5610-5678`). |
| C03 (Feed header narrative) | **FIXED** | Header rewritten (`Feed.svelte:2-3`); the windowing comment (`:123-129`, `WINDOW = 5`) matches the code. |
| C04 (review narratives over contracts) | **OPEN** | `commands.rs:6083-6086` and `Editor.svelte` body comments unchanged since the audit; the Editor *header* already reads as a contract — the residue is the reviewer/date shorthand (`R4, Chief-Engineer decisions D1-D3`) sprinkled through the body. |
| C05 (keyboard comments aspirational) | **FIXED** (Lot 3 E10) | `rowKeyboard()` implements choose/toggle/range (`List.svelte:915-939`); `keyboard-selection.spec.js`; D-41 payment recorded in the Lot 3 plan. |
| C06 (Windows-only staleness) | **HALF FIXED** | The support matrix exists ([MACOS-BUILD.md § Support and proof matrix](MACOS-BUILD.md), delivered `46c1463`). Remaining: no STANDARD §2.10 pointer (the E15e row promised one); `vite.config.js:9-11` still asserts, in a French comment, that WebView2 is the only browser; `ci.yml:13` still names Windows "the product's target" above a file that also builds three other triples. |
| C07 (active docs mix eras) | **OPEN** | STANDARD §10-adjacent tree says `ui/ (vanilla JS)` where ADR 0015 decided Svelte; STANDARD §3's "Last measured" column is dated 2026-07-26 while E15a's dated baseline (2026-09-07: RAM at rest 89.1 MB, thread open p95 30 ms, page jump p95 27–31 ms) landed only in DEBT.md; PLAN.md:37 keeps the `< 1 GB` database target ADR 0010 explicitly lifted; STATE.md is 1,609 lines, mostly narrative history; the Lot 5 plan's E14a–E15f status cells still read "Planned" for delivered steps. |
| C08 (duplicate FK PRAGMA) | **FIXED** (Lot 5 E13e) | One activation (`migrations.rs:1025`) before the fast path (`:1027-1031`), with reason; net `the_fast_path_keeps_foreign_keys_enabled` (`store/tests.rs:4408`). |

### The debt register, checked entry by entry

61 real entries (D-1…D-63; D-7 and D-11 were never given standalone
entries). Every closure claimed by Lots 1–5 is present and dated
(D-9, D-12, D-14, D-25 item 2, D-29, D-32, D-33, D-37, D-47, D-48,
D-56, D-57). The register's defects are **structural**, not factual:

- **~27 live debts sit under `## Closed`** — appended chronologically
  after the banner, never closed: D-13, D-23, D-24, D-25 (items 1, 4),
  D-26, D-27, D-28, D-31, D-34, D-35, D-38, D-39, D-41–D-46, D-49*,
  D-55, D-58–D-61, and **D-63** (opened 2026-09-06, still open, filed
  straight under Closed).
- **D-8 and D-51 are closed under `## Open`** (✅ CLOSED 2026-08-26 and
  2026-09-04 respectively).
- **D-54's latest update is a stray bullet** sitting right after the
  `## Closed` banner with no heading — an explicitly still-open item
  reading as if filed closed.
- **D-40** reads as settled while its own text says the WATCH stays
  alive — a partial status stated ambiguously.
- **Orphan references**: "Family D-7" (benches, DEBT.md:77) and STATE's
  "D-11/D-12/D-14 closed" point at numbers with no entry to close.
- **One unnumbered debt**: D-47's 2026-08-30 reopening deferred a
  "4th Spring-cleaning menu copy" to "a dedicated debt" that was never
  given a number. Lot 5 E13e shared the pile fan; whether a menu copy
  remains is checked at E16c against the code.
- **Reserved, untouched here**: striking D-4 and D-41–D-44 belongs to
  the `/close` of Lots 3/4 (STATE, Lot 5 D0). This lot re-files
  sections; it strikes nothing.

### Also pending in the working tree

`e2e/language-gate.mjs` carries the comment-only edit recording the
Chief Engineer's ratification of the `docs/evidence/` exemption
(STATE says: fold it into the next commit that runs a full gate).

## 2. Scope, and what we refuse

In scope: the comment residues of C02/C04/C06, the C07 document
reconciliation, the DEBT.md re-filing, the program/plan/STATE records.

Refused, per the program's own constraints (§ Lot 6):

- **No global comment sweep** — only the sites the audit names, plus
  what their verification exposes. Useful rationale stays.
- **No cosmetic persisted-schema migration** (D-55's French identifiers
  stay; ADR-protected decisions stay).
- **No debt strikes** — statuses move only where the entry itself
  already says so; the Lots 3/4 strikes stay with `/close`.
- **No rewriting of dated evidence** — AUDIT-2026-09-06.md is the
  audit's frozen baseline snapshot; it is not amended, the
  reconciliation lives here and in the program plan.
- **No behavior change anywhere** — if a comment and the code disagree,
  the comment moves; a code defect found on the way is its own finding,
  reported before it is touched.

## 3. Steps

| Step | Implementation and acceptance | Coverage | Status |
|---|---|---|---|
| E16a | **Comment residues.** `queue_send` doc scoped honestly: enqueue-before-network holds for the send itself; forward preparation's earlier IMAP fetch named (C02). `commands.rs:6083` and `Editor.svelte` body narratives: contracts kept (ownership, async completion, effects), reviewer/date stories trimmed per D4 (C04). `vite.config.js` build-target comment rewritten in English naming both real engines (WebView2, WKWebView) and why `esnext` holds for both; `ci.yml` job name says what the job is (the Windows leg), not that Windows is "the" target (C06). No RED is possible for a comment — the acceptance is the gate plus reading the diff against the code it describes. | C02, C04, C06 | Implemented (commands.rs, vite.config.js, ci.yml; Editor.svelte inspected and KEPT unchanged per D4 — its header already reads as the ownership/effects contract, its citations point at live decision records) |
| E16b | **Normative documents.** STANDARD tree: `ui-v2/ (Svelte, ADR 0015)`. STANDARD §3: the "Last measured" column refreshed with the dated 2026-09-07 baseline per D2, historical figures kept where nothing newer exists, each cell dated. STANDARD §2.10: the promised pointer to the MACOS-BUILD support matrix. PLAN.md targets table: the database row states the ADR 0010 lift instead of the dead `< 1 GB`. Lot 5 plan: E13a–E15f status cells brought to what STATE already records; the E15e cell notes the §2.10 pointer landed here. The `language-gate.mjs` comment edit folded in; STATE's paragraph about it deleted. | C07 | Implemented |
| E16c | **The debt register.** Every entry re-filed under its true section, statuses untouched: D-8/D-51 → Closed; the live entries → Open; D-54's stray bullet reattached under its heading; D-40 restated as "action settled, watch open" under Open. Orphans resolved per D3. The unnumbered menu-copy debt checked against the code first, then numbered or recorded closed with the evidence, per D3. A short header note states the filing rule (an entry lives under its status, updates go under the entry). Acceptance: 61 entries before and after, the D-n multiset unchanged, `grep -c "^### D-"` per section matches the table in §1. | C07, register | Implemented (61 entries before and after, D-n multiset unchanged; a re-filing duplicate of D-54’s bullet was caught by the fresh-eyes review and removed) |
| E16d | **STATE.md.** Per D1: the current-state summary (active lots, next actions, open field proofs, budgets) stays; the closed-job narrative moves to `docs/archives/STATE-HISTORY-2026-09.md` under the archives' frozen-English banner, linked from STATE. Every link re-checked by the docs-links net. | C07 | Implemented (archive byte-identical after its banner — proven by the review’s hash check) |
| E16e | **Records.** Program plan E16 row updated; this plan's delivery record; STATE amended; memory updated. The C01–C08 verdict table (§1) is the consolidation record the program asked for. | program §5 | Implemented |

Gate: the full `/gate` (the ratchet will move DOWN when the French
vite comment goes — the baseline is updated the sanctioned way);
docs-links proves every moved link; fresh-eyes `/code-review high` on
the diff before the commit. One commit, local only (D0).

## 4. Chief Engineer decisions

Answered 2026-09-07, each the recommended option, word for word:
D0 "Confirmed, no push"; D1 "Archive the history"; D2 "Refresh in
place, dated cells"; D3 "Resolve by record"; D4 "Keep decision
citations".

- **D0 — carry-over, confirm**: no push. Lot 6 commits locally on top
  of `46c1463`; the grouped push after your `/code-review ultra` of the
  whole diff stands as ordered.
- **D1 — STATE.md history**: (a) move the closed-job narrative to a
  frozen archive file, STATE keeps the current summary (recommended —
  it is what C07 asks and what STATE's own banner says its function
  is); (b) leave STATE whole and only fix the factual staleness.
- **D2 — STANDARD §3 baseline form**: (a) refresh the existing budget
  table's measured column in place, each cell dated (recommended —
  one table, no duplicate truth); (b) add a separate dated 2026-09-07
  baseline table alongside the historical one.
- **D3 — the orphan debts**: (a) resolve by record — a dated note in
  DEBT.md's header naming D-7 and D-11 as never-created entries,
  pointing D-7 at its archive origin and recording D-11's closure
  evidence inline; the menu-copy deferral gets a real entry (numbered
  D-64) ONLY if the code still shows the copy, otherwise a dated note
  closes the deferral with the E13e evidence (recommended — nothing
  silently disappears, no retroactive renumbering); (b) create
  standalone numbered entries for all three regardless.
- **D4 — C04 trimming depth**: (a) keep the R4/D-n shorthand citations
  where they point at a live decision record, trim only the
  conversational narrative (recommended — the citations are cheap
  provenance); (b) strip to pure contracts, provenance only in git.

# PLAN-AUDIT-2026-09 — reliability, security and product completion

Opened: 2026-09-06.
Statement: implement the recommendations of AUDIT-2026-09-06 while following WORKFLOW.md.
Baseline: 7cdc4495c68c52a2a7d02702d52f86d368ae0653, Wind 0.19.0.

**Status: Lot 1 DELIVERED on 2026-09-06 — implementation commit `e95adb4`, field validated, CI 34038794672 green. D1, D2, D3 and the first visual increment were approved on 2026-09-06. Lot 2 is DELIVERED in `0374e43`, field “1-4 OK” and CI 34053044227 green on 2026-09-06. Lots 3–6 remain open; this program is not closed.**

## 1. Finding and evidence

Source: [audit report](../docs/AUDIT-2026-09-06.md).
Operating method: [WORKFLOW](../docs/WORKFLOW.md), [job skill](../.claude/skills/job/SKILL.md), [STANDARD section 2](../docs/STANDARD.md).
At the audit baseline, the System ended at amendment A119 and STATE recorded the Apple Silicon job as closed. Lot 1 adds A120–A121 and ADR 0038.

The audit distinguishes 5 security findings, 31 behavioral defects, 10 architecture recommendations, 7 product recommendations and 8 comment/documentation findings. Its debt table covers all 61 entries present in the baseline DEBT.md. These are overlapping views of the same work, not 122 independent bugs.

The baseline's Rust tests, clippy, UI lint/build and small Node suites passed during the audit. Passing them did not exercise the following failures. The synthetic probes were replayed on the unchanged baseline on 2026-09-06:

| Scenario | Baseline result | Required result |
|---|---|---|
| Received HTML contains an internal forward marker | Marker accepted; replacement inserts a different synthetic body | No authority or local content read derived from received HTML |
| Escaped CSS URL in the composer | 0 blocked-image count; 1 request intercepted by the browser test | 0 unconsented network requests |
| Positioned content in the composer | fixed element covers the 1280 px test viewport | Content stays within the editable document |
| Save fails while closing | Panel closes, no draft ID, saved toast | Content remains recoverable, explicit failure |
| Ambiguous SMTP acceptance followed by another flush | 2 simulated deliveries | 1 attempt followed by an explicit unknown state |
| Body fetch after UIDVALIDITY changed | New unrelated body stored under old generation | Refetch rejected and mailbox generation reconciled |
| Draft conflict with attachment | New draft has 0 attachments | Complete fork of the appropriate version |
| Saved draft changes sender | Stored account remains account 1 | Consistent account ownership and mirror |
| Edited forward sent | Edited content overwritten | Edited document preserved |
| Bcc-only message | Rejected | Accepted without exposing Bcc |
| Initial sync interrupted, message deleted, sync resumed | Deleted envelope survives | Inventory converges |
| Older calendar cancellation followed by newer request | Newer request is cancelled | Correct version and occurrence state |

The probes use synthetic data, in-memory SQLite and intercepted network requests. They are not real-account field validation and are not yet maintained regression tests. Each implementation slice must first turn its relevant probe into a failing test.

## 2. Scope and completion contract

All recommendations remain in scope. Work is sequenced into six delivery lots so every field verdict concerns a reviewable increment.

- Fix confirmed privacy, integrity and delivery defects first.
- Reproduce static findings before claiming their fixes. A disproved finding is corrected in the audit, with evidence.
- Integrate architecture improvements with the fixes that need them; avoid moving large files before behavior is protected.
- Design the proposed new product capabilities before implementing them. Their cache limits, recovery behavior and screens are not silently invented from the audit.
- Re-measure historical performance debts before choosing an optimization.
- Keep closed or intentionally accepted debts classified honestly. Implementing the audit does not mean reverting valid decisions or removing compatibility data.

An item is **implemented** only when its acceptance test and relevant gate pass. A delivery lot is **delivered** only after review, full gate, the Chief Engineer's STOP 2 field verdict, documentation, commit and green CI. A successful unit test is not a field verdict.

### Existing decisions that do not need to be asked again

- ADR 0003: a duplicate is worse than a delay; unknown delivery must not be resent automatically.
- ADR 0012: data adoption is visible, interruptible and reversible.
- The current System: remote images stay blocked when starting a reply/forward; the recipient should retain the permitted forwarded images, without restoring text the author removed.
- Rust, SQLite, Tauri, Svelte, OS credential vaults and mandatory TLS remain.
- French persisted keys remain compatible. Code, comments and committed working documents follow the English standard.
- Every UI change amends the System in the same commit and receives the required early visual verdict.
- No real-account sending, destructive mailbox test, signing purchase or release publication is implied by a synthetic test.

### Explicit sequencing limits

No public web/mobile server, full calendar product, generic repository framework, unconditional schema renaming or mass code-format rewrite. These are not remedies required by this audit.

Paid signing/enrollment and real-platform field measurements remain Chief Engineer/external actions. The implementation prepares their scripts, checks and concrete instructions; it cannot claim those external outcomes without evidence.

The French audit remains the user-facing working report. Before committing living documentation, preserve that French artifact for the user and provide the repository's English version; do not weaken the language gate to admit it.

## 3. Options and design rules

### 3.1 One broad patch or sequenced delivery

One broad patch would simultaneously change MIME fidelity, persistence, network state, UI behavior and release scripts. It would make a failed field check hard to attribute. Six lots retain one program and one traceability map, while allowing small commits and field checks.

**Proposed D1:** accept the six-lot sequence, beginning with confidentiality and message integrity. No wall-time estimate is asserted before the remaining static reproductions and spikes.

### 3.2 Forward provenance and rendering

| Option | Evidence / consequence | Treatment |
|---|---|---|
| Keep the current HTML marker as authority | S01 and B08 reproduced | Rejected |
| Strip the marker and stop source replacement everywhere | Removes the dangerous lookup but loses the existing forwarded-image contract | Diagnostic/control option, not the final behavior silently shipped |
| Persist trusted provenance separately and preserve the edited document | Can preserve the product contract; needs legacy-draft and image mapping proofs | Starting hypothesis |

The implementation must never restore the entire source block at send time. It must not silently trust markers already stored in legacy drafts. Source generation and mapping of individual image references must be checked.

CSS options: use the installed sanitizer's parsed property allowlist with a separate editing policy, or a dedicated CSS value parser if that API cannot enforce the required policy. Dropping all styles is a measurement control, not an unapproved fidelity regression.

**Hard-point protocol before selecting the implementation:** isolated throw-away spikes, one per viable option using the repository's spike-agent workflow; common synthetic HTML/CSS corpus; measure allowed-format preservation, blocked network attempts, containment, serialized size and sanitize cost. Include escaped functions, CSS comments, rich paste, image-only bodies and edited forwards. Keep the reading iframe isolation in every option. Present a product tradeoff if neither option preserves the contract.

### 3.3 Delivery uncertainty

Retain the outbox and add an explicit unknown-delivery result from the adapter. Certain failures before submission may retry. Ambiguous submission is quarantined using the existing user-decision vocabulary.

If the SMTP library cannot expose the precise send stage, choose the conservative classification and state its availability cost. Do not infer non-delivery from an absent Sent-folder echo alone.

### 3.4 Identity and transactions

Carry account, mailbox, UIDVALIDITY and UID across remote operations; verify before consuming the fetched bytes and again before committing where a reset can race. Use typed identities internally and explicit versioning at persistence/IPC boundaries.

Draft forks, account transfer and enqueue/consume operations are core transactions. A retry identifier makes replay of a committed send request safe. Migration proof covers existing drafts, attachments, outbox rows and a rollback path.

### 3.5 Product and performance choices

No automatic optimization based on old numbers. Each candidate has baseline, alternative, representative fixture and before/after measurements with identical definitions.

Offline attachment caching and backup/restore need separate concrete UI/state contracts before their implementation slice. Keep their IDs open in this program rather than declaring a documentation sentence to be implementation.

## 4. Delivery steps and gates

### Lot 1 — Confidentiality and message integrity

| Step | Implementation and acceptance | Audit coverage |
|---|---|---|
| E1 | Promote hostile HTML/edited-forward probes to regression tests. Run rendering/provenance spikes; select safe received/editing policies and trusted draft metadata. Preserve edits and image intent, reject forged/legacy authority, filter paste before insertion. | S01, S02, B08, B13; A01/A04; C01 |
| E2 | Make save failure observable and keep the composition recoverable. Bind all async completions to an edit session. Fork complete drafts. | B02, B06, B10; C02 |
| E3 | Validate generation on direct body/backfill/attachment fetch. Make remote draft import complete and reversible, retaining recipients, MIME parts and reply metadata. | B03, B05; A04; D-19/D-59 |

Inner gates: mail-render/core/imap unit files, whole impacted composer/attachment E2E files, IPC and System checks. Migration tests run on synthetic current and legacy databases. Early visual STOP after the first UI increment. Full gate + review + STOP 2 before lot delivery.

**Field checklist:** reply/forward with images blocked; delete text inside a quote then inspect the delivered content using the Chief Engineer's own chosen recipient; save/close/reopen a draft with files; resume a draft from another client. Faults and mailbox resets stay synthetic unless the Chief Engineer chooses disposable test accounts.

### Lot 2 — Irreversible effects and account lifecycle

Started on 2026-09-06 after the Chief Engineer's go. The [focused Lot 2 plan](PLAN-AUDIT-2026-09-LOT2.md) records reproductions, measured SMTP alternatives and the implementation contract. D4 was answered “A” and D5 “Ok go” on 2026-09-06. E4–E6 are implemented; the one fresh review is complete and its three confirmed findings are corrected. Full gate GREEN in 476 seconds: 776 Rust, 28 Node, 228 UI passed, one UI flaky (passed on retry), four Rust ignored and one optional UI benchmark skipped. STOP 2 was validated on 2026-09-06, verbatim “1-4 OK”, with zero reported KO. No removal duration was supplied. Delivered in `0374e43`; [CI 34053044227](https://github.com/smonchamps/wind/actions/runs/34053044227) is green on Windows and both mac architectures. The required pre-push gate passed in 227 seconds, with 229 UI passes and zero flaky results. The Rust total includes two example tests omitted from the initial summary.

| Step | Implementation and acceptance | Audit coverage |
|---|---|---|
| E4 | Classify unknown SMTP delivery and quarantine it; use a fake SMTP dialogue cutting at defined stages. Require single-instance exclusivity for writing. Show queued/sent/unknown accurately. | B01, B22, B26; A05 |
| E5 | Consume a draft atomically with enqueue and a request ID; transfer drafts between accounts with files/mirror tombstones; coordinate removal with active jobs. | B07, B09; A06 |
| E6 | Use safe targeted IMAP actions, native MOVE for archive, and explicit unsupported/deferred behavior without a safe expunge. Reconcile interrupted COPY before retry. | B04, B28 |

Gates: transport and core state-machine tests, process/crash fault injection on fixtures, relevant outbox/account E2E files, full gate. Field: offline queue, reconnect, correct sender after restart and archive on disposable messages. Existing unknown-delivery warning and resend choice must remain explicit.

### Lot 3 — Daily workflows and bounded work

| Step | Implementation and acceptance | Audit coverage |
|---|---|---|
| E7 | Fix Reply-To/reply-all and Cc/Bcc-only sending, Feed scope/page reset, initial-sync inventory reconciliation and calendar version/occurrence handling. | B11, B12, B14, B15, B16 |
| E8 | Fix OAuth legacy rotation/manual fallback/listener bounds; edit generic credentials in place and test both protocols without sending mail. | B17, B18, B19, B24, B25 |
| E9 | Bound allocations, attempted work and disk writes; expose per-account/folder errors and retry state; guarantee bounded eventual flag convergence. | S04, B20, B21, B23, B27 |
| E10 | Add modal focus containment and keyboard selection, image-consent revocation and accurate connection/local-echo state. Preserve shared data when removing a single account. | G03/G04; D-4/D-41/D-42/D-43/D-44; C05 |

Gates: whole impacted Rust and E2E files, synthetic multi-account/error corpus, connection throttling tests, no-write credential probes, keyboard-only journeys. New controls require a study prototype, System update and early visual verdict. Full gate and field at lot completion.

### Lot 4 — Dependencies, build and release guarantees

| Step | Implementation and acceptance | Audit coverage |
|---|---|---|
| E11 | Upgrade Vite/plugin dependencies coherently, add npm audit and autonomous Node tests to CI; retain all current Rust/platform checks. Implement structured/redacted diagnostics. | S03, S05; A05/A10 |
| E12 | Require build source/tag identity, stage the complete release matrix before Latest, decode/verify every signature, fail the final verification mode when proof is absent. Express build-input dependencies and share gate step definitions where useful. | B29, B30, B31; A09/A10; D-32/D-33 |

Gates: npm/Rust audit where tools are available, build/lint, release-script fixtures with fake git/gh/artifacts, signature tamper tests, interrupted-publication simulation, full gate. No real upload is needed for unit validation. Final installation/update proof follows the four supported platforms and remains STOP 2 evidence.

### Lot 5 — Architecture, offline/recovery and measured scale

| Step | Implementation and acceptance | Audit coverage |
|---|---|---|
| E13 | Extract core use cases, introduce validated identity/error/IPC contracts, enforce adopted Store access and handle file replacement. Move the indirect OAuth Store opening off the async worker. Shorten critical sections after contention measurements. Split domains and remove only the duplicates still present. | A01–A09; D-9/D-25/D-34/D-46/D-47/D-48/D-49 |
| E14 | Design and implement offline-availability/cache controls, coherent backup/restore/export and data-retention choices. Validate restoration without unintended sending; provide coverage visibility for incomplete search/import. | G01, G02, G04 |
| E15 | Rebaseline opening/search/pagination/memory/idle work; investigate Feed memory and page eviction; implement winning measured alternatives. Complete invitation fidelity, localization and platform-support proof/tooling. | G05, G06, G07; D-1/D-2/D-11–D-16/D-18/D-20/D-21/D-26/D-29/D-35/D-37/D-53/D-54/D-56/D-57/D-60/D-61 |

Gates: use cases without Tauri, schema compatibility and migration cancellation tests, recovery fixtures, long-session benchmarks, platform jobs. Each performance increment has an early before/after STOP. Each new UI capability has its own prepared design and field checklist before its production implementation.

### Lot 6 — Documentation, comments and closure

| Step | Implementation and acceptance | Audit coverage |
|---|---|---|
| E16 | Correct misleading comments and stale normative statements, keep useful rationale, reconcile debt statuses with evidence, update audit/PLAN/ADR/System/STATE. Preserve compatibility-oriented decisions and historical archives. | C01–C08; remaining debt-register recommendations |

No global removal of comments, no cosmetic persisted-schema migration. Documents state implemented, verified, field-validated and externally pending separately. Check every finding ID and every debt entry against delivery records; nothing silently disappears.

## 5. Traceability and quality procedure

Coverage of all primary audit IDs:

- S01–S05: E1, E9, E11.
- B01–B31: E1–E9 and E12.
- A01–A10: E1/E3–E5, E11–E13.
- G01–G07: E10, E14–E15.
- C01–C08: corrected at the affected step, consolidated E16.

The audit's full 61-entry debt reconciliation is the secondary checklist. Closed entries stay closed unless a new test disproves the closure. Conditional limits are not called fixed until their relevant path exists and passes.

Each delivered slice records:
1. RED command and observed failure against the old behavior.
2. GREEN command and changed result.
3. Whole-file targeted E2E verdicts when relevant.
4. Fresh-eyes review and dispositions.
5. Full gate verdict for the exact candidate, with no source changes during it.
6. Early visual/performance verdict where required.
7. STOP 2 field checklist, ready-to-copy committed PowerShell launch/measurement commands and the Chief Engineer's actual result.
8. System amendment, ADR/migration notes, commit and CI result.

Use scripts/gate.ps1 for the full gate. Resolve the workstation npm launcher issue before invoking the full gate; do not replace a failed required step with a claimed success. Keep source edits paused while a gate runs. No bypass of hooks.

For field work, prepare commands using scripts/field.ps1 and scripts/run-wind.ps1 and the relevant measurement tools. Do not operate on an assumed real-database path. Fixtures and simulations can be run independently.

## 6. Chief Engineer decisions — STOP 1

### D1 — Program scope and first delivery lot

**Question:** approve this six-lot program and start Lot 1 (E1–E3: confidentiality, safe composition persistence and message/draft integrity)?

**Recommendation:** yes. Keep one program and deliver reviewable lots with the workflow's field validations. Start from the reproduced failures, preserve all remaining recommendations in the tracker, and prepare later product/performance choices at their respective steps.

**Alternative:** select a different first lot or smaller initial slice; its remaining findings stay explicitly pending.

**Status: APPROVED on 2026-09-06.** The Chief Engineer answered: "Je valide". This approves the program and the start of Lot 1; later field validations remain due. <!-- lang:fr -->

Further decisions are requested one at a time only when their concrete alternatives and evidence are ready: rendering fidelity if the spike cannot preserve it, offline-cache/backup UX and limits, performance-budget exceptions, external signing/enrollment and real-platform proof. Existing ADR decisions listed in section 2 are not asked again.

### D2 — Composer fidelity at the untrusted HTML boundary

**Question:** approve a conservative editing policy that keeps ordinary typography, colors, lists, tables and permitted images, but simplifies unsupported complex CSS/layout in replies, forwards and pasted content?

**Recommendation:** approve the token/value policy (option B). Keep the reading iframe's separate policy; apply the stricter editing boundary before content enters the live composer. Never restore deleted text to recover appearance. Common formatting passed all 12 positive spike cases; complex functions/layout are outside the measured preserved subset. This is a deliberate fidelity limit, not a claim that all rich email formatting remains identical.

**Alternative:** require preservation of advanced CSS layouts in the composer. That needs a further isolated-editor design and measured prototype before E1 implementation; the current property-list API cannot meet that requirement alone. Removing all formatting remains only a diagnostic control.

The reports establish functional limits, not a speed winner: the two synthetic corpora and repetition protocols differ. Option A loses uppercase declarations and misserializes nested functions; option B preserves the tested ordinary formatting but deliberately rejects complex functions and positional declarations. Per-property/unit bounds, clipboard integration, image provenance and browser regression coverage remain production acceptance work.

**Status: APPROVED on 2026-09-06.** The Chief Engineer answered: "Je valide". This decision concerns the newly demonstrated fidelity tradeoff. D1 and the first visual verdict are already approved and are not requested again. <!-- lang:fr -->

### D3 — Preserve the edited attachment version across remote replacement

**Finding:** the pull currently deletes stale draft rows and their cascading files before fetching replacements. Copying the current row on conflict cannot recover files after that deletion. A pull planned before an autosave can also delete the newly saved version unless deletion rechecks its identity/revision and clean state.

| Option | Evidence | Consequence |
|---|---|---|
| A: in-process edit lease | 14 deterministic scenarios; 10,000 begin/end pairs in 143.789 ms, 20,000 SELECTs and zero writes on an in-memory fixture | Defers deletion while editing and permits autosave/push, but requires every writer to share coordination. An abandoned begin response leaks protection until cleanup; the lease does not retain a durable version. |
| B1: durable snapshot copying attachment bytes | 12 grouped lifecycle scenarios; near-25-MiB open median 1,029.999 ms and fork 815.816 ms | Preserves the edited version independently of the mirror, but copies payloads on every open/fork. |
| B2: durable snapshot referencing immutable attachment payloads | Same 12 lifecycle groups plus incarnation reuse, attachment-only edits, shared-file cleanup and interrupted migration checks; near-25-MiB open median 1.604 ms and fork 4.714 ms | Preserves the version without recurring payload copies; requires persistent session/reference tables, migration and recovery. |

The lease uses in-memory SQLite; the snapshot models use file-backed WAL/FULL SQLite. These are Python prototypes with different operation protocols, not an application speed ranking. B2's latest medians use three fresh processes; near-cap open observations are 1.591 / 1.604 / 4.774 ms. Earlier observations under other work were noisier. Production UI latency and database contention remain to be measured.

**Recommendation / question:** approve B2: keep one durable editing snapshot with immutable file references, validate the draft incarnation and revision at acquisition/save, and atomically fork the edited version when its mirror was replaced. Opening/closing without editing must neither dirty nor repush a draft. Recover committed editing changes after a process interruption; release an abandoned untouched snapshot without creating a draft. This cannot recover browser typing that never reached an autosave. Do not introduce an unbounded draft history.

**Migration cost:** in the near-25-MiB prototype, one-time upgrade median 333.089 ms, reversal 326.219 ms. Before checkpoint, the upgrade occupied 52,518,912 bytes of main database plus 26,442,192 bytes of WAL, versus about 26 MiB initially. This is temporary copy/journal headroom, not a recurring opening cost. Cancellation, SQLite progress interruption and process termination before commit restore the original schema and bytes. Production must integrate visible progress/cancellation, disk headroom, quiescent writers and full-schema migration/rollback tests under ADR 0012. The production migration and its reversal are implemented; validation evidence follows below.

**Required integration:** session-scoped add/remove/save/send/discard paths; immutable draft incarnation across SQLite rowid reuse; guarded stale deletion; complete atomic remote MIME import; recovery before another composer opens; candidate-based collection of unreferenced files (the prototype's whole-table scan is not the production design). Keep account transfer/removal semantics aligned with Lot 2 rather than copying the prototype's experimental account-fork policy. Saved drafts still retain their actual attachments; releasing a snapshot must never collect a file referenced by a draft.

Reports: [lease](C:/Users/smonc/.codex/visualizations/2026/09/06/01a075d7-45cf-74a0-aff0-dc04a5e13e67/spike-render-allowlist/spikes/draft-edit-lease/REPORT.md), [copied and referenced snapshots](C:/Users/smonc/.codex/visualizations/2026/09/06/01a075d7-45cf-74a0-aff0-dc04a5e13e67/spike-render-parser/spikes/draft-version-snapshot/REPORT.md). These reports include executable protocols, raw measurements and limits; no real accounts were accessed.

**Status: APPROVED on 2026-09-06.** The Chief Engineer answered: "Je valide". GO for B2 durable editing snapshots with immutable file references and the reversible migration. D1, D2 and the centered-banner verdict remain approved. <!-- lang:fr -->

### Decision log

| Decision | Date | Chief Engineer's exact answer | Effect |
|---|---|---|---|
| D1 | 2026-09-06 | Je valide | GO for the program and Lot 1 (E1–E3) | <!-- lang:fr -->
| D2 | 2026-09-06 | Je valide | GO for the conservative composer token/value policy | <!-- lang:fr -->
| D3 | 2026-09-06 | Je valide | GO for referenced editing snapshots and reversible migration | <!-- lang:fr -->

## 7. Current delivery record

| Phase | Status |
|---|---|
| Investigation | Existing audit reviewed; synthetic probes replayed on unchanged baseline |
| Design | Program and acceptance criteria written; hard-point protocols defined |
| STOP 1 | D1 approved on 2026-09-06 |
| Implementation | E1 HTML boundary and E2 save/session increments pass targeted tests; E3 body/backfill/attachment generation guards added. Complete remote import and editing-version retention are implemented under approved D3; final review, full gate and field validation remain pending. |
| Review/full gate | Not started for this job |
| STOP 2 | Not reached |
| Documentation/commit/push/CI | Plan only; no implementation delivery claimed |

Continue Lot 1 through its required evidence and validations. D1 is settled; do not ask it again or restart the completed audit.

### First E2 increment — 2026-09-06

- RED: `cargo test -p mail-core --offline conflict_fork` failed both new cases on the baseline: fork attachments were empty, and an attachment-copy failure was not surfaced.
- GREEN: `cargo test -p mail-core --offline drafts::` passed all 42 draft tests. Saving now runs in one SQLite transaction, copying existing source attachments in order to the conflict fork; a copy error rolls the operation back.
- RED: the complete `tests/composer-integrity.spec.js` file failed in the real Tauri fixture: the composer disappeared after the injected save failure.
- GREEN: the same complete file passed (1 test, 37.4 s including build). It checks retained text, a visible error, no saved toast and successful persistence on retry.
- UI ESLint, build, System coherence (4 themes / 68 values) and `git diff --check` passed. These are targeted checks, not the full gate.
- System amendment A120 documents the first visual increment. The screenshot is a synthetic test fixture, not a real account.
- Early visual STOP approved on 2026-09-06 with the requested vertical centering: "Oui, mais centre le texte verticalement dans le bandeau". The save-error banner now uses equal 7 px top/bottom padding, preserving its height. No field verdict, full review/gate, commit or CI delivery is claimed. <!-- lang:fr -->

Still open in E2: completing coverage of asynchronous session transitions and preservation of an editing snapshot when a remote pull has deleted/replaced its original row. The transaction fix only copies attachments from an existing source; it does not yet establish complete version provenance.

### Rendering spike evidence — 2026-09-06

Two isolated throw-away spikes completed without modifying production rendering code.

| Option | Observed behavior | Indicative sanitize cost |
|---|---|---|
| A: Ammonia property allowlist | Zero network attempts on 15 browser cases; standard typography/table formatting retained. Uppercase properties lost; nested functions misserialized; extreme dimensions unbounded. | Mean 0.043 / 3.638 / 44.391 ms for about 1 KB / 100 KB / 1 MB, 1,000 repetitions |
| B: cssparser token/value policy | 14/14 hostile cases removed, 12/12 formatting cases retained, zero browser network attempts. Per-property units/size limits still need production design; complex CSS deliberately rejected. | Median of three means 0.040 / 2.611 / 30.770 ms for 1 KB / 100 KB / 1 MB, 200 / 30 / 5 repetitions |

Both used Edge 152 on Windows ARM64, synthetic inputs and interception aborting all potential requests. Their corpora and timing protocols differ: these numbers cannot establish a speed ranking. Neither is a WebKit, clipboard, MIME or real-account proof. Option B adds a direct cssparser API dependency but zero new transitive packages; option A needs no dependency change. The exact option A API alone fails the required fidelity/containment criteria. The production policy and any product tradeoff remain to be settled from these reports before E1 integration.

### E2 asynchronous persistence increment — 2026-09-06

The visual adjustment requested by the Chief Engineer is implemented and verified in the real Tauri fixture: equal 7 px top/bottom padding centers the save error text/icon in the existing banner. The complete original regression file passed again, as did lint and System coherence.

Two additional regression cases were then observed RED on the preceding implementation: closing hid the composer while an attachment write was held; text edited during a held close-save was lost (stored body remained "Before the save."). After serialization of draft saves/attachment mutations and revision checking before close, the complete three-test composer integrity file passed (34.9 s including build). These are synthetic fault/latency fixtures, not field evidence.

Saving returns attachment metadata from within the same transaction, allowing the editor to adopt the fork's actual chip IDs without a second, fallible IPC read. This DTO projection is mechanical and does not claim an independent meaningful RED; the fork's byte integrity and rollback are covered by the existing RED/GREEN core regressions. The 42 core draft tests still pass.

Session guards now cover queued mutations, picker completion, save completion and close transitions; new/open-draft waits for the current composition to close successfully. A stale queued removal cannot delete a file from the preceding fork source. More transition tests and remote replacement/version retention are still due before E2 is complete.

The four affected whole E2E files passed all 79 tests in 1.0 minute: composer-integrity, redesign-screen02, redesign-gated-journeys and redesign-feedback-6. UI ESLint and System coherence passed again. No full gate, field verdict, commit or CI delivery is claimed at this point.

### E1 implementation design refinement — D2 approved, 2026-09-06

The CSS token/value policy is approved. Initial renderer RED covered five cases (escaped loads, relative images, oversized legacy layout, formatting after a rejected nested function and received private markers); all passed after the first renderer implementation.

For forward images, remove the local-source lookup altogether. The earlier trusted-source-row hypothesis existed to make that lookup safe; retaining a lookup is unnecessary when only image URLs need preservation. Preparation now produces inert image placeholders and a separate per-editor map of placeholder to original HTTP(S) URL. Save/send restores only surviving image nodes through that map and persists the edited HTML with its permitted URLs. Resuming a draft prepares a new map from its stored HTML. No account/mailbox/UID authority is ever decoded from received HTML, and no body is reread to recover an image. Legacy source markers are stripped rather than trusted. This avoids adding a schema and lifecycle for obsolete body-replacement authority, while retaining the accepted edit/image contract.

The map is content, not a capability: its values are validated as HTTP(S) image URLs, never interpreted as local identities or fetched by the renderer. Incoming placeholder fragments are normalized before new mappings are generated. A renderer regression proves deleting text and one of two images keeps only the edited text and remaining URL; an unsafe mapping cannot introduce JavaScript or a local-source marker.

The renderer alone was not sufficient verification; the subsequent browser evidence is recorded below. The full forward-to-outbox/MIME path and final review/gate remain due.

### E1 HTML boundary and asynchronous signatures — 2026-09-06

- The live composer and signature editors share HTML preparation before DOM insertion, including paste/drop. A per-editor URL map restores surviving images only. Scripted clipboard integration is exercised in the Tauri fixture; native OS clipboard and other rendering engines remain field/platform checks.
- RED: rich-paste/signature tests initially found no interception; image-only `body_boundary` returned no rich body. GREEN followed boundary integration. Subsequent signature-load races were reproduced and corrected rather than accepted as flaky tests.
- The obsolete forward marker parser and whole-block source replacement API were removed. This removes a local body lookup and supersedes its old tests asserting replacement; renderer/UI tests instead assert that deleted text/images stay deleted. Mechanical API removal has no independent meaningful RED.
- RED: a signature saved while its initial read was pending became empty after Settings closed (reproduced twice). GREEN: saving captures the originating editor node before waiting, and a late result cannot update another editor's visible state. Applying a signature to other accounts also captures its source and target accounts before waiting.
- RED: a protocol-relative image URL was lost on a compose/save roundtrip. GREEN: it is blocked in the editor and retained as an HTTPS URL when saved. Unresolvable relative paths remain blocked.
- Latest whole-file E2E wave: **14 passed, 22.1 s**, across composer-html, composer-integrity and redesign-feedback-6. Cases cover rich paste, image-only persistence, reopen/edit with removed text and images, pending paste on close, intervening typing, native undo, signatures and save/attachment races.
- Renderer: **34 tests passed**. The prior combined core/renderer wave passed 469 core tests (2 ignored) and 33 renderer tests before the generation increment. Later verification counts belong to the next record, not to an unchanged baseline.
- Last remote CI checked after the local RED: baseline `7cdc4495c68c52a2a7d02702d52f86d368ae0653` completed successfully. It does not cover this working tree.

### E3 mailbox generation increment — 2026-09-06

- RED: four body/backfill cases accepted a changed generation before or during download, or assigned a returned body to an unrelated requested UID. GREEN: generation checks bracket each body batch, unsolicited/duplicate UID results are excluded, and cache writes check identity inside their SQLite transaction.
- A typed local mailbox identity carries account, wire mailbox name, local mailbox id and UIDVALIDITY. An additional RED showed that checking only local id/generation allowed a reassigned mailbox to receive another account's old bytes; checking the complete identity fixes it.
- Direct attachment download/retrieval uses the same before/after generation rule and rechecks local identity before consuming the result. The IMAP attachment adapter accepts only the requested returned UID. Additional tests cover a local reset before cache commit and attachment reset before/during download. These supplemental tests passed when added; no artificial RED is claimed for them.
- Focused core body wave: **39 passed**. Clippy for mail-core/mail-render/mail-imap/wind-desktop, all targets, passed with warnings denied. IPC contract: 112 defined/registered; main-thread guard: 113 commands checked, no blocking command.
- Cost: two fresh SELECTs per body batch/direct attachment operation. Cached body reads still avoid network access. This is a correctness cost; no production round-trip latency improvement is claimed.
- E3 remains open: complete remote draft recipients/files/reply metadata and atomic import, editing snapshot integration, and end-to-end propagation of identities from a displayed message/session across stale UI requests. The new generation guards do not claim to solve all identity races or constitute the final field verdict.
- Final targeted Rust wave: **476 core + 80 IMAP + 34 renderer tests passed**, with 3 pre-existing ignored tests. UI ESLint, System coherence (4 themes / 68 values) and whitespace checks passed. The shell's `npm` shim pointed to a missing user-level npm-cli; invoking the installed ESLint entry point directly ran the same lint without changing machine configuration. Full gate/review/field/commit/push remain pending for the completed lot.


### D3 and E3 implementation evidence — 2026-09-06

- Durable single-slot snapshots, immutable file references, incarnation/epoch validation, recovery, token-scoped file edits and atomic outbox consumption are implemented. Sender transfer preserves files and tombstones the former account's mirror. See [ADR 0038](adr/0038-durable-draft-editing-snapshots.md).
- Three remote-import regressions first failed on lost Cc, ignored file insertion and deletion of a locally edited mirror; their fixes passed. Additional RED/GREEN cases cover reply metadata lost across a fork, sender duplication, late reply context, stale displayed-message cache reads, and a draft discarded during remote fetch.
- The parser retains Cc/Bcc, priority, reply headers, binary files, calendars and embedded images. Draft MIME construction now retains Bcc (lettre otherwise removes it), priority and reply headers. Actual SMTP recipient privacy remains covered by its existing send test.
- Displayed rows carry mailbox ID and UIDVALIDITY. Body/cache reads, thread opening, reply/forward contexts and source-file retrieval validate that identity. Local draft conversation links retain their source generation; sending uses frozen reply headers. UI body caches include the generation.
- Import commits one complete message at a time and delays obsolete-mirror removal until all required imports succeed. Guarded deletion rechecks account/incarnation/revision/remote UID and clean state. Earlier successful imports may coexist with old mirrors on failure; no partial draft is presented.
- Full-schema synthetic migration tests cover cancellation, reversal/re-upgrade and actual SQLITE_FULL. The incompatible file migration runs last; earlier independent migrations may already have committed. The old file layout survives a failed file migration.
- Production Rust WAL fixture, 26,214,399 bytes: upgrade 358.411 ms; reversal 329.211 ms; opening median 2.540 ms across seven samples; conflict fork 3.997 ms. Payload count stays one and all bytes are checked. These are local synthetic timings, not field/UI latency.
- Targeted Rust tests and Clippy passed during the increments. Expanded UI runs exposed an obsolete forward-marker assertion, then an incorrect replacement selector targeting the header rather than the body. The corrected whole screen02 file passed all 61 scenarios. The new source-failure scenario and the other five composer-integrity scenarios passed.
- Full gate, final review and STOP 2 are still pending. This entry does not mark the lot or the six-lot program delivered.

### Final review — 2026-09-06

The requested `/code-review high` command/skill is not installed in this environment. A manual review of the production diff and new modules was performed instead, concentrating on untrusted HTML, generation checks, snapshot ownership, transaction failure, migration compatibility and asynchronous UI completion. This is not claimed as execution of that unavailable command or an independent-agent review.

Confirmed review findings and fixes:

- Native-window closing had no composer persistence handler. The first added test accidentally passed because the 2 s autosave later failed; constraining the assertion before that timer exposed the actual RED. The Tauri close-request callback now waits for close/save and prevents destruction while the composer remains open. Its required `core:window:allow-destroy` permission applies to the main window. The six integrity scenarios passed, including immediate native-close failure and ordinary retry.
- Legacy replies had source identity but no frozen outgoing reply headers. A migration test failed with `None` instead of the parent Message-ID. Migration now freezes identity, In-Reply-To and the full References chain once, together with a transactional completion marker. A source reset afterwards does not erase those headers. The full core library run passed **499 tests, 2 ignored**.
- The audit's English edition retains all 61 findings, all 61 actual debt entries and 148 baseline code references. The original French artifact is preserved outside the repository. Findings remain dated baseline observations, not assertions that later implemented defects are still present.

Scope boundaries retained after review:

- D-59's structural MIME-part identity redesign remains open under A04/Lot 5. The reading attachment filter/order is unchanged in this lot; complete remote-draft parsing does not reuse those persisted reading ranks. Generation guards alone do not close D-59.
- Atomic enqueue and sender transfer are integrated because snapshot consumption requires them. E5 is not delivered: account-removal quiescence and the UI response to an acknowledgement lost after enqueue remain pending in Lot 2. Retrying a consumed editing session currently reports stale state instead of creating another outbox item.
- Recovery preserves committed edits, not keystrokes that never reached storage before a process crash. Legacy reply provenance is captured from the local source at migration time; already-misbound historical data cannot be repaired retrospectively.
- Migration progress is reported per independently committed adoption pass. Reversal is exposed by the tested consuming Store API and requires stopped application writers; it is not a new user-facing downgrade control.

The full gate and real-account field verdict remain due. No real account or database was used during these checks.

### Full gate verdict and STOP 2 — 2026-09-06

The first full run stopped at the language check: an original French approval quotation and the heading abbreviation were detected in this new plan. The quotation now uses the repository's intentional-French marker and the heading spells out Chief Engineer. No baseline increase or gate bypass was used. A second **single-call full gate passed in 243 seconds**, without source edits during execution.

| Step | Result and evidence |
|---|---|
| 1 — Format | GREEN, cargo fmt check |
| 2 — UI build/lint | GREEN, 195 Vite modules; no Svelte/ESLint warnings |
| 3 — Contrast | GREEN, 4 themes / 440 pairs |
| 4 — System coherence | GREEN, 4 themes / 68 token values |
| 5 — Main-thread guard | GREEN, 117 commands |
| 6 — Script syntax | GREEN, Node/PowerShell/bash checks |
| 7 — Language | GREEN, 299 files; 1,968 markers against baseline 1,974 |
| 8 — IPC | GREEN, 116 defined / 116 registered commands; 108 called by literal name |
| 9 — Markdown links | GREEN, 69 files / 391 relative links |
| 10 — Clippy | GREEN, workspace/all targets, warnings denied |
| 11 — Rust tests | GREEN, 725 passed / 4 ignored, including examples |
| 12 — Doctests | GREEN, 0 tests |
| 13 — Node and UI | GREEN, 28/28 Node; 217 Playwright passed, 1 flaky, 1 skipped |

The flaky case is `redesign-feedback-3.spec.js:26`: expansion initially exposed one card instead of three; the automatic retry passed. It remains a recorded intermittent failure, not proof that a race is absent. The workflow permits a green gate with counted flaky results. No additional full-suite replay was used to conceal it.

STOP 2 checklist for the Chief Engineer, using their selected real accounts:

1. If attachment migration is offered, record its duration and verify existing drafts/files afterwards. Cancellation and disk-full faults have been tested synthetically; do not fill a real disk for this pass.
2. Create a draft with Cc/Bcc, priority, text and **2 files**; close/reopen, then close the native window immediately after editing and relaunch. Expect all fields and **2 unchanged files**.
3. Reply/forward rich content: images remain blocked while composing. Remove one sentence and one image, keep another image, then optionally send to the Chief Engineer's chosen test recipient. Expect **0 restored deletions**, permitted retained content and no missing retained files.
4. Import a draft created in another client with Cc/Bcc and **2 files**. Change one word, close and reopen in both clients. Expect complete recipients/files/body and preserved reply threading where applicable.
5. With two accounts, switch the sender of a draft carrying **2 files**, close/restart and verify the selected account and both files. During concurrent edits from another client, verify that any reported conflict retains complete versions.

Run the committed `scripts/field.ps1`, then `scripts/run-wind.ps1 -Trace trace-audit-lot1.log` after closing existing Wind windows. The latter builds the release UI/binary and launches with tracing. These commands are handed to the Chief Engineer; the session has not run them on real data. Field results, Phase 4 documentation and commit/push/green CI remain pending. The later lots are not marked delivered by this gate.

### STOP 2 field return and corrections — 2026-09-06

The Chief Engineer approved items 1, 2 and 4 of the handed-over checklist. Item 3 reported a Gmail web draft with two attachments absent after more than a minute and repeated manual syncs. Item 5 reported no observed migration; this is not a measured migration verdict. The attached screenshot also shows the sender selector above its From label. The follow-up answer states that manual synchronization completed without an error: "Terminée sans erreur". <!-- lang:fr -->

Confirmed mechanisms and remedies:

- The manual command called `poll_inbox`, while remote-draft import ran only in the full cycle, scheduled every 30 minutes. The manual command now calls portable `run_light` through the existing IMAP adapter. It imports complete drafts on the same connection after INBOX polling. Wake and five-minute scheduled light passes use that path too; IDLE arrivals remain INBOX-only. The UI probes drafts after the manual report even with zero INBOX changes. Only unknown remote UIDs are fetched; complete attachment parsing and generation guards remain shared with full sync. Import time depends on remote payloads and network latency; this change does not promise one-minute automatic discovery.
- The shared select wrapper sets `align-self:flex-start`, overriding the composer row's centered alignment. The composer now overrides the wrapper with `align-self:center`; the chevron follows it. The System's composition rule and journal A121 record this correction.
- Review found that an import problem could reach the report but remain invisible when INBOX polling succeeded. Such errors now keep the retry button active and show the existing sync failure state in Inbox. Mixed reachable/unreachable accounts retain the existing numbered partial-failure state; a clean retry clears the error. Other folders retain their category status text and expose retry through the button.

TDD evidence:

- Two new core cases failed before the behavioral fix: zero imported drafts instead of one, and an empty problem list instead of the simulated import failure. After correction, all six `cycle::tests` pass, including byte-for-byte preservation of two files, repeated-sync idempotency and preserved INBOX arrivals on draft failure. The initial mechanical extraction retained the old behavior; its shell wiring was not presented as a failing assertion. A misspelled Store test method was corrected before measuring RED.
- The sender geometry case failed twice at a 13 CSS-pixel center offset, then passed with a one-pixel maximum tolerance. Its WebView2 capture was inspected. The original six composer integrity cases remained green.
- The sync-report UI case failed with a normal Sync button despite a draft-import error, then passed for failure, clean retry and mixed account failure. Its first harness attempt used an ineffective Tauri override and an incorrect English label; that failure is excluded as proof. The test now uses a narrowly scoped report fixture compiled out of release builds. An intermediate run passed only on retry because a prior test left the Drafts view selected; explicit Inbox navigation removed that test-order dependency. The final whole-file run passed **8/8 in 15.9 seconds, with no retries**.

The follow-up review inspected command routing, connection logout on errors, report aggregation, UI state clearing, the release-only removal of test branches and CSS scope. The unavailable `/code-review high` command remains the previously stated limitation; this is a manual review, not an independent review. Full gate after these changes and a targeted real-account replay remain due. Prior approved field items are retained; the replay targets Gmail draft import with both files and sender alignment.

### Field correction gate verdict — 2026-09-06

A single-call full gate passed in **200 seconds**, with no source edits while it ran. All 13 steps passed: format; UI build/lint (195 modules, no warnings); contrast (4 themes, 440 pairs); System coherence (4 themes, 68 values); main-thread guard (117 commands); script syntax; language ratchet (299 files, 1,968 markers against 1,974); IPC (116 defined/registered, 108 literal callers); links (69 documents, 391 links); clippy; **727 Rust tests passed / 4 ignored**; doctests (0); and **28 Node tests plus 220 Playwright tests passed / 1 skipped / 0 flaky**. The skipped case is the opt-in RAM benchmark. The production bundle was checked before the E2E rebuild: it contains no `__e2eSyncSummary` fixture branch.

Only the targeted real-account replay remains for this field correction: after closing Wind, run `scripts/run-wind.ps1 -Trace trace-audit-lot1.log`; manually sync the Gmail draft, open it and verify both attachments, then check From alignment in composition/forwarding. The prior approvals remain recorded. No real database was read by the session, no message was sent, and no commit/push was performed before the renewed STOP 2 verdict.

### Final STOP 2 verdict — 2026-09-06

The Chief Engineer answered **"1 et 2 ok"** to the targeted replay: (1) after manual sync, the Gmail draft appears with both attachments present and readable; (2) the From label, selected account and chevron align in a forward. This closes both reported field defects. The earlier approvals remain valid. No migration was observed on this installation; migration and reversal evidence remains synthetic, not a claimed real-account measurement. <!-- lang:fr -->

Lot 1 has its field GO. Commit/push and the CI verdict follow; the complete audit program is not closed. Next is Lot 2, E4–E6, with the remaining account lifecycle and irreversible-delivery behavior described above. The E5 transaction work already integrated for editing snapshots must be reused, not repeated.

### Lot 1 publication and CI — 2026-09-06

Implementation commit `e95adb4c8335208816005c5bc57765ce0d19700f` is published on `origin/main`. Automatic approval review initially blocked publication; the Chief Engineer subsequently authorized the push explicitly. The mandatory pre-push full gate passed in **179 seconds**: 727 Rust tests, 28 Node tests and 220 UI tests passed; 4 Rust tests ignored, 1 optional UI benchmark skipped, 0 flaky UI tests. No source edits occurred during the gate.

[CI 34038794672](https://github.com/smonchamps/wind/actions/runs/34038794672) passed on that exact commit: Windows 3m55s, macOS Intel 2m53s, macOS Apple Silicon 1m38s, UI checks 14s and dependency audit 25s. Field validation, publication and CI are complete for Lot 1. The final documentary commit records this verdict; it does not implement or close Lots 2–6.

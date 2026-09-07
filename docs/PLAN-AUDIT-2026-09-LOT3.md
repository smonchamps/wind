# Lot 3 — Daily workflows and bounded work

Opened: 2026-09-06. Baseline: `a58c1cf` (Lot 2 delivered, clean checkout).
Parent: [audit program](PLAN-AUDIT-2026-09.md), E7–E10.
Method: [WORKFLOW](WORKFLOW.md), [job](../.claude/skills/job/SKILL.md).

**JOB CLOSED on 2026-09-08 — full field validation** ("1-11 OK",
zero KO, 2026-09-07; commit `9f4a9b1`); the grouped push landed 2026-09-07/08, CI GREEN 34164333355 (one red first: the seam guard caught an unguarded `__e2eNoRestart` read from Lot 5 E14, fixed in `dfd734c`); shipped in **0.20.0**, published 2026-09-08 and field-proven the same day ("release ok and autoupdate ok on all computers").
At close: D-4 and D-41–D-44 struck; the E9 residue and ADR 0041's
open cycle measurement entered as **D-65**; the two accepted limits
(two body-pump drivers §5.9, the renamed Google address refusal)
stand as written below. Kaizen: the lot's figures live in its own
sessions; 0 KO at STOP 2.

*(Original status: authorized investigation and implementation; no delivery claim.)*
The Chief Engineer's “Go Lot 3” authorizes the existing E7–E10 scope. <!-- lang:fr -->
New product tradeoffs are asked separately; existing approvals stay valid.

## 1. Findings

The current code still rejects empty To even with valid Cc/Bcc, ignores Reply-To
in reply-all, keeps stale Feed pages when scope or inventory changes, and omits
absence reconciliation after resumed initial synchronization. These findings
will become maintained regression tests before their fixes.

Read-only reconnaissance confirms the five E8 authentication findings. Generic
account SQL already preserves the id on update: the missing work is an explicit
repair journey, independent IMAP/SMTP probes, safe credential publication and
coordination with the account-incarnation guards delivered in Lot 2.

A synthetic SQLite reproduction of the current invitation statements produced
four incorrect cancellations in four cases (both arrival orders, older CANCEL
or unrelated organizer). Parsing/storage also omit DTSTAMP and RECURRENCE-ID;
response generation loses occurrence identity. Historical HTML cannot recover
these fields without bounded re-fetching of the calendar source.

The three backfill pumps count successful fetches rather than attempted work.
The IMAP dependency accumulates responses before returning them; checking MIME
sizes afterwards cannot bound allocations. Non-CONDSTORE flag passes visit only
recent UIDs and skip unchanged folders. At 500 UIDs per 30-minute cycle, visiting
200,000 UIDs would already take 200 hours before folder rotation (arithmetic,
not a performance measurement).

## 2. Scope and acceptance

| Step | Work and proof | Coverage | State |
|---|---|---|---|
| E7a | Reply-To reply-all; Cc/Bcc-only composition and transport privacy; synthetic core, SMTP and shell/UI regressions | B11/B12 | Implemented; targeted GREEN |
| E7b | Authoritative inventory on resumed initialization; Feed scope reset and all served-page reconciliation | B14/B15 | Implemented; targeted GREEN |
| E7c | Version/organizer/occurrence-aware invitation state; preserve occurrence in replies; guarded enqueue and legacy extraction recovery | B16 | Implemented; targeted GREEN |
| E8 | Preserve OAuth rotation, live manual-browser consent, absolute bounded callbacks; generic repair and both protocol probes | B17–B19/B24/B25 | Implemented; targeted GREEN |
| E9 | Bound bytes, attempts, elapsed work and disk writes; retain partial operation errors; fair eventual flag reconciliation | S04/B20/B21/B23/B27 | Local attachment reads and rotating flags implemented; other resource/retry work pending |
| E10 | Modal focus/inert/restore, keyboard selection, revoke image consent, connection and local-echo accuracy, shared-data provenance | G03/G04, D-4/D-41–D-44, C05 | Implemented; targeted GREEN |

Each row needs maintained failing assertions, targeted whole-file GREEN,
integration review and the final gate. No real mail, credentials or database
are used by the session. Account lifecycle, SMTP uncertainty and immutable
message identities from Lots 1–2 must remain intact.

## 3. Design and open measurements

- Composition validates all three recipient lists, then requires a nonempty
  union. Bcc remains solely in the delivery envelope.
- Reply-all uses the received Reply-To in place of From, keeps To/Cc separate
  and removes duplicate/self recipients. It does not infer address aliases.
- Initial sync retains the complete server inventory separately from missing
  UIDs, then removes absent local messages only after all fetch batches succeed.
- Feed scope changes clear page, scroll, window, fold and read-section state.
  Same-scope refresh reconciles the served prefix while retaining the original
  read-section assignment of surviving cards. Stale async responses are ignored.
- Generic probes stage submitted values in memory. Publishing a repaired
  credential needs a versioned vault entry and an atomic SQLite configuration
  reference, preserving the old reference across failures. Endpoint identity
  changes need an explicit policy; equal UIDVALIDITY on two servers is not proof
  of the same mailbox.
- Callback limits use a single consent deadline, bounded request lines,
  per-client deadlines and cancellation. Wrong-state clients cannot consume the
  legitimate consent; browser-open failure does not destroy its listener.
- Invitation correlation uses account, UID, organizer and occurrence, ordered
  by SEQUENCE then DTSTAMP. Unsupported scheduling shapes must not silently
  become a series-wide action. Full scheduling fidelity remains Lot 5.
- Allocation bounds and flag reconciliation require measured throwaway options
  before choosing resource budgets. No historic performance number is presented
  as a fresh benchmark.

New controls require a scratch study, System amendment and the workflow's early
visual verdict. No framework rewrite, mailbox migration, release publication,
full calendar product or changes to accepted delivery uncertainty are implied.

## 4. Chief Engineer decisions

- **Scope GO, 2026-09-06, verbatim:** “Go Lot 3”. <!-- lang:fr -->
- **D6, 2026-09-06, verbatim:** “Exiger les deux connexions avant d’enregistrer la configuration (recommandé). Une réparation échouée conserve l’ancienne configuration.” <!-- lang:fr -->
  Both IMAP and SMTP authentication must succeed before registration/repair is
  published. No test message is sent. A failed repair keeps its old configuration.
- **D7, 2026-09-06, same-mailbox scope retained:** the Chief Engineer answered
  the final two-item validation request with “Oui, par contre comment l’utilisateur est censé utiliser ce champ ?” <!-- lang:fr -->
  The yes covers the recommended bounded repair and the visual proposal; the
  follow-up question concerns the field's explanation. Repair keeps IMAP host,
  username and email; password, port and SMTP settings may change. A migration
  to another mailbox remains outside this lot. Do not ask this scope again.
- **First visual verdict, 2026-09-06, approved with usability feedback:**
  “Oui, par contre comment l’utilisateur est censé utiliser ce champ ?” <!-- lang:fr -->
  The user approved the shared form's presentation and asked how users should
  interpret Username. The label is now Login username, with persistent help
  explaining that email is the default unless the provider specifies another
  username. The input references the help via aria-describedby. The current
  geometry remains. Actual synthetic onboarding/Settings captures were inspected;
  the scratch study `lot3-account-study.html` stays outside the repository.
- **D8, 2026-09-06, verbatim:** “je valide”. <!-- lang:fr -->
  Approved the proposed recurrence subset: single-occurrence changes and
  whole-series cancellation; complex scheduling messages remain readable without
  reply controls. This does not authorize a full recurrence engine.
- **D9, 2026-09-07, verbatim:** “On part sur 32Mio”. <!-- lang:fr -->
  Approved a 32 MiB raw complete-message ceiling (33,554,432 bytes), including
  headers, body and encoded attachments. Larger messages keep their envelope;
  body and attachment access goes through webmail. This is an input limit, not a
  whole-application memory guarantee. Implement actual stream admission and
  maintain explicit refusals without discarding previously cached content.
- **D10, 2026-09-07, verbatim:** “1-11 OK, par contre on va au commit mais pas au push, on fera un push de l'ensemble des modifications liées à l'audit en une seule fois.” <!-- lang:fr -->
  Field verdict: all eleven items OK, zero KO. The lot is committed locally;
  the push (and therefore the CI run and the closure) is deferred to one
  grouped push of every audit-related change. No CI claim until then.


## 5. Verification and field

Record RED/GREEN figures here as they occur. At completion: one fresh review,
full `scripts/gate.ps1`, then STOP 2 with the committed `scripts/field.ps1` and
`scripts/run-wind.ps1` commands and a numbered field checklist. Field validation
precedes commit/push; green CI is required before delivery. No full gate, field
verdict, commit or CI has yet been performed for Lot 3.


### First implementation increment — 2026-09-06

- Core RED: 510 passed, two new failures, two existing ignored. Empty To rejected
  valid copies; resumed initial sync kept removed UID 6. SMTP RED: 45 passed,
  one new failure because a blind-only message had no visible envelope recipient.
  GREEN: **512 core + 46 SMTP passed**, two existing ignored, 20.60 s + 0.73 s.
  The transport receives an explicit envelope; its formatted message contains no
  Bcc header or hidden address. Inventory cleanup runs after successful batches.
- Shell/UI RED: Reply-To was replaced by From; an own Cc-only reply added the
  unrelated Reply-To fallback. The latter assertion initially ran before context
  arrived; waiting for Cc first exposed the real failure. Both paths now pass.
- Feed RED: account switch retained 25 old cards; deleting a page-two card kept
  it visible. Same-scope refresh now reads the whole served prefix. Two further
  deterministic cases exposed the two orders of pagination/refresh overlap
  (20 cards remained instead of 25). Pending scroll demand and requested prefix
  extent now survive refresh; scope changes reset both. Surviving read-section
  assignments, card image grants and the existing organized-mode journeys pass.
  An initial race probe froze the status poll as well as Feed requests, and a
  later probe returned before the scroll event ran: neither is counted as RED
  evidence. The final probes hold only Feed requests and dispatch scroll before
  releasing the held response.
- Generic form RED: Username absent in onboarding and Settings. GREEN: explicit
  username follows email until edited, blank submits email, a custom username is
  preserved, all connection fields are disabled during a held check and restored
  after failure. Password is cleared on success/Back, retained for a failed-check
  retry. Tests inject failure before network access and never read vault secrets.
  **At this increment, dual probes and repair persistence were pending; E8 below completes them.**
- Final targeted UI wave: **47 passed in 47.5 s, zero retries**, eight whole files
  (Feed scope/images, recipient routing, generic username, original onboarding,
  organized mode, feedback-3 and feedback-14-inbox). Build and lint pass. Seven
  Node catalog/launcher cases pass. Format, System coherence (68 values), contrast
  (440 pairs), main-thread guard (119 commands), IPC (118 commands), language
  (1,968 markers) and documentation links (402) pass. This is an increment's
  targeted verification, not the final full gate. Clippy for core, SMTP and the
  desktop app (all targets) passes with warnings denied in 6.99 s.
- The sandbox blocked esbuild before assertions on the first UI attempt; the
  authorized unsandboxed rerun uses only the isolated test accounts. `npx` also
  had a broken user installation; calling the installed Playwright CLI directly
  resolves it. These environment failures are not behavioral RED evidence.

Remaining after that increment: E7c, E8 backend and repair/OAuth controls, E9, E10, final fresh review,
full gate, STOP 2, documentation closure, commit and green CI. System A123 records
this increment; the first visual verdict and its usability correction are recorded above. No real-account field
result, message delivery or publication is claimed.


### Username clarification — 2026-09-06

The new accessible-name assertions failed on both surfaces before the label/help
change. After correction, the complete generic-username file passes **2/2 in
13.9 seconds, without retries**, including the accessible help, defaults, explicit
username, held verification and failure recovery. UI build/lint, both catalog
checks, System coherence and the language ratchet pass. This is the approved
form's usability correction; no additional account capability is claimed.

### OAuth increment — 2026-09-06

- Callback RED: four maintained regressions failed (silent client extending the
  global deadline, ambiguous request parsing, oversized request and wrong-state
  callbacks consuming consent). GREEN uses one absolute deadline, 8 KiB request
  lines, two-second client budgets and 10 ms cancellation polling. A validated
  callback survives a failed browser acknowledgement. Token/identity HTTP calls
  have a 30-second timeout; cancellation during such a blocking call is observed
  on return, not claimed instantaneous.
- Legacy-token RED: five cases exposed the overwritten rotated token and the
  cross-provider fallback. Effective refresh tokens are now published once,
  identity checked first, and legacy cleanup follows successful publication.
  Seven memory-vault cases include failed writes/cleanup and provider isolation.
- Manual-consent RED: browser-open failure dropped the listener, and cancellation
  failed to interrupt an accepted silent client. The synthetic loopback token
  server now receives the code and PKCE verifier after a failed opener. Two more
  RED cases exposed credential publication after cancellation or wrong identity.
  GREEN: **39 mail-auth passed, one existing ignored, 2.01 s**. The real OS vault
  test remains ignored. Reconnection validates identity before writing any token.
- Shell consent preparation has one bounded active owner, a non-reused id,
  30-second unclaimed preparation expiry, one claim and RAII cleanup. Three
  lifecycle tests pass. The existing account Registration/Lease stays in place.
  Cancellation is serialized with the start of credential publication: accepted
  cancellation prevents publication; a late cancellation reports saving instead.
- UI RED: manual-link controls absent on onboarding and Settings. Shared notice
  now supports copying, keyboard fallback, cancellation and close/retry on all
  three surfaces including reconnect. Four Node controller tests cover closing
  during preparation, accepted/refused cancellation and stale status. A further
  RED showed that closing during publication suppressed the successful account
  refresh: committed completion now still refreshes the parent after close.
- Targeted UI GREEN: **17 passed in 21.5 s**, five whole files, zero retries.
  Captures revealed the disconnected account address collapsed to zero width;
  a new maintained assertion reproduced that defect. Rows now wrap and preserve
  identity space: **6 passed in 16.5 s**, three whole files, zero retries. Actual
  captures inspected; System A124 updated. One intermediate run was interrupted
  after a Svelte rune/property naming conflict was detected; the alias is fixed,
  and the completed builds have no such warnings.
- Mail-auth/desktop all-target Clippy passes with warnings denied (5.95 s).
  UI lint, Node catalog checks, System coherence, IPC (121 registrations),
  main-thread guard (122 commands) and language ratchet pass. No final review,
  full gate, real-account field, commit, push or delivery claim for this lot.

The recurrence subset question is pending with the Chief Engineer; no dependent
calendar implementation has started. E9/E10 remain.

### Generic repair and dual probes — 2026-09-06

- Probe RED: the SMTP closure never ran. GREEN exercises all four success/failure
  combinations, always collecting both diagnostics and requiring both successes.
  The SMTP transcript witness authenticates and sends NOOP, with no MAIL, RCPT
  or DATA. Whole SMTP suite: **47 passed, 0.86 s**.
- SQL RED: four cases reproduced identity changes, conversion of OAuth accounts,
  partial settings after failed slot publication and a phantom account after a
  failed first preference write. GREEN commits settings, slot reference and the
  initial history together; repairs retain their id and history. Six targeted
  core tests include a ten-table snapshot (mailbox, draft, attachment/blob,
  interrupted delivery, action, edit session and preferences) and idempotent
  legacy-slot migration. Five were included in the full core run: **517 passed,
  two existing ignored, 21.26 s**; the sixth then passed in the six-test wave.
- Password publication uses two bounded inactive/active vault slots, retaining
  the legacy read path. Three memory-vault tests cover failpoints and repeated
  cleanup failure. Whole auth suite: **42 passed, one existing ignored, 2.01 s**.
  This storage decision is recorded in [ADR 0039](adr/0039-generic-credential-publication.md).
- A repair captures the original account generation during probes, retires it
  only after success, releases its own lease before draining, checks the stored
  configuration again and publishes under the command lock. Failure preserves
  old settings and restores watcher admission. Successful repair clears stale
  backoff/work state. Whole desktop suite: **49 passed, 0.85 s**.
- UI RED: generic repair card absent. The shared editor now loads only nonsecret
  settings from SQLite, freezes identity per D7, accepts an empty password as
  unchanged and clears edited secrets on close/success. Generic repair remains
  available when a session exists. A dedicated synthetic seeder exercises the
  real settings IPC without any vault secret; failed verification is injected
  before network access. System A125 and the external scratch study document it.
- Final targeted six-file UI wave: **19 passed in 53.9 s, zero retries**. One
  preceding wave exposed a stale assertion for the old French IMAP-only error;
  it now requires both protocol labels and still rejects IPC deserialization
  errors. Eleven Node controller/catalog/launcher cases pass. All-target Clippy
  passed after collapsing one nested condition. No full gate or field claim.


### Local echoes, keyboard and attachment reads — 2026-09-06

- Echo RED: two maintained core cases returned empty Cc after sending/moving a
  message. GREEN preserves Cc in both echo paths and the category row, with no
  Bcc exposure: 518 core passed, two existing ignored (20.62 s). A subsequent
  migration failpoint exposed a non-retryable partial ALTER/update; one transaction
  now rolls both back. The targeted migration case passes and rejects recovery
  from a reused send id with a different account/message identity. Old gesture
  echoes whose deleted source is unavailable cannot recover unknown Cc.
- Modal RED: Tab escaped Settings; the remaining serial cases did not execute.
  The shared action isolates background siblings, wraps current focusable controls,
  restores the opener and supports stacked owners. App shortcuts are suspended
  while a modal exists. Compose busy/failed-close rules and menu ownership remain.
  GREEN: 17/17 in 30.7 s across modal-focus, generic-repair, oauth-consent and
  redesign-panes; no retries. This covers Settings, compose, feedback and drawer
  behavior, not a real legacy-migration field run.
- Selection RED: Ctrl+Space opened instead of checking in two-pane mode. The first
  scenario initially expected eight rows from eight fixture messages, although
  threading produces seven; that fixture assertion is not behavioral RED evidence.
  Ctrl+Space now checks without opening, Shift+Space extends the existing loaded
  range, Escape clears an idle batch and Settings documents the keys. The stale
  checkbox comment was replaced (C05); the count is announced politely. A duplicate
  handler name was corrected after a build error. The new two-pane test initially
  leaked its profile preference into multi-select; it now purges its local settings
  before/after, as existing layout tests do. The three latest remote CI runs are
  successful (including baseline a58c1cf); no Lot 3 CI claim. Final targeted wave:
  **18/18, 24.1 s, zero retries**, four whole keyboard/modal/multi-select files.
- Local attachment RED: two stream tests reproduced unlimited reads despite the
  existing remaining budget. Both legacy draft and editing-session paths now open
  once, check that handle's file metadata, and admit at most remaining bytes plus
  one refusal sentinel. Known excess reads zero content; growth after metadata is
  still bounded. A refusal preserves prior files and allows later eligible files.
  Three reader tests pass; whole desktop suite **52 passed, 0.58 s**. Two synthetic
  real-IPC cases and the whole composer-integrity file pass **10/10, 38.1 s, zero
  retries**. This enforces the existing 25 MiB decoded-file budget, not an IMAP
  ceiling, file-read deadline or whole-app heap guarantee.
- UI lint, six Node consent/catalog cases, IPC (123 registrations), main-thread
  guard (124 commands), System coherence (68 values), language ratchet (1,968
  markers) and documentation links pass. System A126 records the keyboard and
  echo behavior. No full gate, final review, field verdict, commit or push.

### Measured IMAP options — 2026-09-06; no ceiling approved

Job-required isolated spike worktrees at a58c1cf produced 34 raw-quota cases,
29 partial-read cases and 51 additional candidate-ceiling cases. Sources and
full measurements stay in ignored `target/lot3-spikes/{raw-cap,parts}/spikes/`;
no spike code is production code. All mail/server data are generated locally.

- A quota above the pinned alpha15 parser stops lying/missing-size literals,
  newline-free responses and aggregated bodies at the actual admitted-byte cap.
  An incomplete response poisons its connection; an independent eligible message
  succeeds on a replacement. Honest advertised oversize can be rejected earlier.
- With an experimental 2 MiB raw ceiling plus 8 KiB response allowance, abusive
  responses stop at exactly 2,105,344 bytes. This proves the input boundary,
  not a 2 MiB heap boundary. Ordinary full-MIME content needs one size FETCH and
  one body FETCH; real TLS reconnect/timeouts remain unmeasured by this spike.
- Partial reads reconstructing whole raw MIME retain an O(message-size) Vec.
  Selected base64 parts decoded directly to files keep bounded working buffers:
  the nested 8 MiB attachment takes 178 commands and approximately 7.6 MB Windows
  peak working set. Six selected-part hashes match the current parser's small/large
  fixture outputs. Current alpha15 public getters hide the returned offset:
  a peer repeating offset zero produced false success with a corrupted hash.
  A production parts option therefore requires a verified origin/section/UID
  adapter or dependency change, plus broader MIME and persisted-index equivalence.

Candidate quotas were also measured with current mail-render sanitization in
fresh release processes, with conservative overlapping owned text/HTML/file forms:

| Raw message ceiling | Parser-only plain-text peak working set | With conversion/sanitization |
|---|---:|---:|
| 8 MiB | 14,745,600 B | 57,004,032 B |
| 16 MiB | 23,117,824 B | 107,315,200 B |
| 32 MiB | 40,017,920 B | 208,003,072 B |

Every candidate tested cap−1/exact/+1, honest/missing/lying size and 64 MiB abuse.
Mixed MIME decoded 4/8/16 MiB attachments exactly. These are Windows process peak
working sets, not exact private-working-set peaks or Wind's whole-app budget.
The 32 MiB simple-HTML conversion alone approached 200 MiB; neither lower option
proves whole-app compliance. Raw ceilings also count headers/base64 expansion and
would withhold large-message body/draft/attachment access through the capped path.
The Chief Engineer must choose that product tradeoff before implementation; no budget inferred.

The recurrence subset (D8), pending during these measurements, is now approved:
single-occurrence updates and whole-series cancellation; multi-event or
THISANDFUTURE changes stay readable but non-actionable, with an explanation.
The Chief Engineer answered “je valide” on 2026-09-06. <!-- lang:fr -->
Implementation is resuming with maintained parser and correlation regressions.
E7c, E9 remote quotas/fairness/diagnostics/disk guards, E10 image revocation,
connection accuracy/shared-data provenance, final review/gate/field remain open.

Final checks for this increment: **519 core + 52 desktop passed** (22.00 s and
0.87 s), two existing core ignores. All-target Clippy for core and desktop passes
with warnings denied (12.91 s); diff whitespace, documentation links and language
ratchet pass. This remains targeted verification; the lot's full gate is pending.

### Calendar increment — 2026-09-06 to 2026-09-07

- D8 authorizes original-occurrence updates and whole-series cancellations;
  multi-event and THISANDFUTURE forms remain readable without response actions.
  Parser RED exposed missing DTSTAMP/occurrence and accepted ambiguous shapes.
  GREEN: **23 mail-ical tests**, including validated recurrence-property round
  trips, timezone equivalence, original dates after moves, property injection,
  duplicate methods/dates and invalid version timestamps. Missing DTSTAMP remains
  tolerated for a lone otherwise valid message; conflicting equal/missing
  timestamps stay unverified rather than guessing the winner.
- Stored states correlate account, organizer, event UID and original occurrence;
  SEQUENCE then DTSTAMP replace arrival ordering. Three maintained enqueue REDs
  reproduced cancellation after display, re-extraction/namespace reuse and lost
  occurrence identity. The transaction now rechecks the displayed revision and
  namespace, generates the calendar reply from verified metadata, and records
  the response and outgoing message together. Corrupt scope queues nothing.
  List badges carry their invitation's own mailbox namespace and revision.
- Legacy metadata adoption preserves cached HTML and the local reply/date. Cards
  are unverified until an explicit one-message refresh succeeds; failed/missing
  remote sources retain offline content and permit retry. A successful extraction
  preserves the local reply and avoids repeat fetching. Historical blanket
  cancellation flags are not presented as verified scheduling evidence.
- Final Rust wave: **528 core + 23 calendar + 52 desktop passed**, two existing
  core ignores (24.27 s core, 0.91 s desktop). All-target Clippy for the three
  packages passes with warnings denied. UI lint, IPC/main-thread guards, System
  coherence, language ratchet, catalog/placeholders and documentation links pass.
- UI RED exposed the absent explanation and legacy refresh control. One failed
  assertion used `flash` instead of the existing `toast` selector; that failure
  is not counted as a product regression. A further maintained RED caught a late
  failed reply accessing a closed card and suppressing its error. The captured
  card now gates local mutation. Final whole-file wave: **9/9 in 35.5 s, zero
  retries**, invitation-scheduling and redesign-invitations. Actual unsupported
  and legacy card captures inspected. Verification errors use their own wording.
  System A127 records the controls; the scratch study stays outside the repo.

This completes the targeted E7c increment, not Lot 3. E9 remote bounds/fairness/
partial failures/disk admission and E10 image revocation, live connection state
and shared-data provenance remain. The measured remote ceiling choice is pending.
Final fresh review, full gate, real-account field, commit/push and CI are still due.

### Image permission increment — 2026-09-07

- Core RED: revocation left the message grant in place, and stale/missing message
  identities could create consent. GREEN: two maintained tests cover independent
  message/sender grants, idempotent removal, missing envelopes and namespace reuse.
  Grant/revoke and sender lookup validate the selected namespace in one transaction.
- UI RED: no message-level exit on the pane or Feed; Settings sender removal left
  open documents allowed. A first race fixture captured the other message's body;
  that fixture error is not counted as a product defect. After targeting the real
  allowed document, its late arrival reproduced the revoked images returning.
- Reading and Feed share the permission notice. Successful writes advance a
  permission generation, discard loaded documents and reject older in-flight
  reads. Sender permission remains independent and the removal result says when
  it still applies. A failed grant keeps images blocked and reports failure.
  The original screen02 suite exposed shared callback ownership during pane/full
  remount: each subscription now owns its own teardown callback.
- Five image-revocation, one Feed-images, four Feed-scope and nine invitation
  tests pass (19 total). Screen02 then exposed its old seven-keyboard-shortcut
  assertion, obsolete after the approved keyboard selection increment. Updated
  to ten with explicit selection-key checks: **61/61 screen02 in 56.5 s**, zero
  retries. Thus all six targeted files pass across these final waves; this is
  not a claim of an uninterrupted 80-test run. Actual permission notice inspected.
- Core **530 passed**, two existing ignored, 22.36 s. Core/desktop all-target
  Clippy, UI lint, System coherence, language/catalog/placeholders, IPC (125),
  main-thread (126), links and whitespace checks pass. System A128 records the
  contract; scratch study remains outside the repository.

Remaining: E9 remote admission/fairness/errors/disk, E10 live connection state
and contact provenance, then the full-lot review/gate/field. No commit or delivery.


### Contact provenance increment — 2026-09-07

- Three maintained REDs reproduced retained exclusive suggestions, incomplete
  rollback and loss of the original historical name. GREEN keeps observations
  per account and derives the small autocomplete table atomically with triggers.
  Account removal removes only its observations; shared sources and unknown
  historical ownership remain. New outgoing recipients and initial backfill
  carry the same provenance. A failed migration rolls its schema back and retries.
- Additional checks cover newer unnamed mail, migration idempotence, restored
  names/frequencies, sent recipients and backfilled contact removal. Latest Rust:
  **535 core + 52 desktop passed**, two existing core ignores; all-target Clippy
  passes with warnings denied. Account-removal E2E **3/3, 42.9 s, zero retries**.
- Sequential release measurements after warmup: 200k-envelope directory backfill
  150.65 ms baseline / 226.41 ms with provenance; 50k-contact autocomplete
  27.19 / 25.91 ms (existing budget below 50 ms). These are single synthetic
  samples, not real-account measurements. ADR 0040 records the retention policy.
- The existing removal paragraph explains retained shared rules/contacts and
  unknown historical sources. No new control; System A129. Five catalog checks,
  System coherence, language ratchet, links and whitespace checks pass.

E9 and E10 live connection state remain, followed by final review/gate/field.
The remote ceiling question remains unanswered. No commit, push or delivery.


### Rotating flags and connection state — 2026-09-07

- A workflow-required isolated spike compared rotating 500 with recent100 plus
  rotating400 on synthetic 200k-row SQLite data. Selection median/p95:
  0.634/1.239 ms versus 0.738/1.410 ms; 400 versus 500 passes to cover the corpus.
  The chosen recent/rotating split preserves the existing total of 500; latest100
  remain checked every opportunity, ranks101–500 now join the rotating schedule.
  The effective descending cursor needs one SQL bound; a redundant pair caused
  a depth-dependent scan in the spike. See ADR 0041 for costs and limits.
- Four maintained REDs: old flags starved by arrivals, quiet archive skipped,
  successful retries never advanced, and UIDVALIDITY replacement was accepted.
  GREEN persists one mailbox cursor and commits it with flags, protects local
  pending actions, visits quiet archives, and checks the actual remote namespace
  before/after retrieval. Missing UIDs advance only after a successful command;
  unexpected or duplicated returned UIDs are refused. Restart, rollback, cursor
  reuse, sparse/max UID and remote-reset checks pass. Latest core: **542 passed,
  two existing ignored, 32.64 s**. The superseded pure newest-only selection test
  was removed because that policy no longer exists.
- A stable 200k-row mailbox needs 500 successful opportunities, nominally
  41 h 40 in INBOX / 10 d 10 in archives. This is not a wall-clock guarantee.
  The two production SELECT fences cost more than the spike's SELECT estimate:
  two SELECTs plus one FETCH per nonempty opportunity. Global operation admission
  must preserve folder fairness; real cycle latency remains a field measurement.
- Connection UI RED: Settings did not react to a failed connection after startup.
  State now follows the latest authenticated connection attempt and current
  session registry through the five-second local probe. It distinguishes absent
  session from unavailable connection without inferring dead credentials from a
  folder error. Cancelled/older attempts and retired account generations cannot
  replace a newer result. UI probe tokens protect completed reconnection.
- Two concurrent-lifecycle regressions pass; whole desktop **54/54, 0.42 s**.
  Core/desktop all-target Clippy passes with warnings denied. Whole four-file UI
  wave **15/15, 36.3 s, zero retries** (connection state, generic repair, OAuth
  consent, onboarding). A preceding run spent its 90-second scenario timeout in
  compilation; fixture setup now has its own beforeAll timeout. That failure is
  not recorded as a product defect. Actual Settings capture inspected; System
  A130, external scratch study. UI lint, IPC125/main-thread126, System, language
  and links pass. Final full-lot review/gate/field have not run.

E7, E8 and E10 have targeted implementation coverage. Remaining E9 work is the
remote resource ceiling and admission, fair backfill retry with spent budgets,
operation-specific durable partial failures/retry policy and disk guards.
The previously asked 8/16/32 MiB product ceiling is still awaiting a choice;
no restriction on remote message size is enabled. Then final review/gate/field,
commit/push and CI are due. The lot is not delivered.


### Remote ceiling decision — 2026-09-07

D9 is now approved: 32 MiB per raw message. The earlier pending-choice entries
above are historical checkpoints. Implementation resumes with the measured
stream quota option; the 200 MiB whole-app memory budget remains unproven.


### Remote raw ceiling implementation — 2026-09-07

- D9 is implemented in the IMAP adapter: 32 MiB per complete raw message,
  shared by body, attachment and draft retrieval. Honest sizes refuse before
  BODY; absent sizes are admitted conservatively. Actual plaintext reads are
  capped below the IMAP parser, with 8 KiB protocol allowance and a 120-second
  absolute command deadline. Size metadata has its own smaller allowance.
- Four maintained initial REDs covered oversized admission, missing-size
  omission and duplicate/unrequested body UIDs. A further RED proved that
  successive commands accumulated more than one body call's 32 MiB input budget;
  the call now returns its completed prefix and defers the next estimated batch.
- Exact cap, cap+1, 64 MiB literal/unterminated-line abuse, metadata abuse,
  aggregate bodies, slow drip, poisoned-session refusal and nested attachment
  byte/index parity pass. Whole IMAP: **111 passed, one existing ignored, 2.88 s**.
- Size refusals retain a typed command code while ordinary errors retain their
  string wire shape; whole desktop **55/55, 0.55 s**. The reader states the
  webmail fallback without a futile retry. The first E2E RED showed generic
  retry text; screenshot inspection then revealed the floating action bar
  covering the notice. A geometric RED confirms that defect. Failed-body action
  bars now remain in normal flow; loaded-message behavior stays unchanged.
- Whole two-file E2E wave **8/8, 24.7 s, zero retries**, including ordinary retry
  and message actions. Actual corrected capture inspected. System A131; scratch
  study outside the repository; [ADR 0042](adr/0042-raw-imap-download-ceiling.md)
  records the ceiling, measured trade-offs and remaining integration limits.

This is a targeted increment, not delivery of Lot 3. Permanent per-UID refusal
handling must still prevent one oversized message from blocking its backfill
batch. Shared spent budgets, fair retry, durable partial-operation diagnostics
and disk guards remain E9 work. Final review, full gate and field have not run.


### Persisted size refusals — 2026-09-07

The preceding permanent-refusal checkpoint is resolved. Two REDs showed that the
next pass retried the same oversized UID and that a refusal could belong to a
replaced remote namespace. The checked storage path now fences the remote
namespace after a complete refusal, verifies local identity in a transaction,
and records only the requested UID. Following passes exclude recorded refusals;
an account with only such pending bodies does not open a backfill connection.
The missing-content count still includes them, so it cannot assert full download.
Cache reads take precedence; known refusals are explained offline. A reset or
message/account deletion purges the markers. A future larger ceiling retries them.

The first persistence shape failed the existing covering-index plan guard:
reading an envelope column forced row lookups. A separate small WITHOUT ROWID
table fixed that regression without changing either existing covering index.
Reopen, failed writes, next-pass progress, namespace replacement and cache
preservation are covered. Whole core **546 passed, two existing ignored, 34.03 s**;
whole IMAP **111 passed, one existing ignored**; desktop **55 passed, 0.66 s**.
Core/IMAP/desktop all-target Clippy passes with warnings denied.

E9 shared admission, general fair retries/partial diagnostics and disk guards
remain. This checkpoint is not the final whole-lot review, full gate or field.


Final targeted checkpoint for D9 (2026-09-07): IMAP **112 passed, one existing
ignored, 6.16 s**. A further RED/GREEN preserves ordinary unsolicited flag-only
FETCH updates during size discovery; they must not become invalid-size errors.
A complete size refusal permits the namespace fence, while incomplete stream
responses remain poisoned. Latest two-file reader E2E **8/8, 35.4 s, zero retries**
after persistence integration. UI lint, IPC125/main-thread126, System68, language
1967 <= 1974, docs links405 and whitespace checks pass. No running test jobs,
commit, push or real-account access. General E9 admission/retry/disk work remains.

### Shared admission, fair retry and disk guards — 2026-09-07

Resumed from the previous session's uncommitted tree. The general E9 work was
found implemented and wired end to end (core pumps, cycle, poll, IPC, Settings,
System A132, [ADR 0043](adr/0043-backfill-admission-and-retry.md)) but not
recorded here; that session's RED observations for this increment were not
written down and are not reconstructed. What is recorded below was measured
in this session.

- Spent budgets: candidate allowances are debited before I/O, shared across
  folders and accounts, with 64 MiB of actual IMAP plaintext input and a
  120-second fetch deadline per caller; inner raw-message admission keeps D9.
  Scan positions persist per mailbox, UIDVALIDITY and pump (date DESC, UID
  DESC); a claimed position records an attempt, never a downloaded body, so
  missing content returns on a later sweep. Arrival previews request their
  new UIDs directly. The first scheduled mailbox rotates between passes.
- Partial-operation diagnostics persist per account, mailbox and operation,
  separately from connection state. Temporary failures retry after 30 s,
  doubling to 30 min; refusals and corrupt scopes wait for a manual retry;
  a manual retry releases the delay but keeps the diagnostic until success.
  The notice slot links the count to Settings > Accounts, which lists folder,
  operation, escaped reason and retry condition per affected account.
- Disk admission keeps a 64 MiB reserve for local intentions and checks
  actual payload estimates before every background write (envelopes, bodies,
  recipients, remote drafts, sync batches); a refused write settles the
  operation and preserves drafts. Admission estimates only; SQLite arbitrates.
- One RED was still open in the tree: a repaired generic connection did not
  release refusals waiting for a manual retry (`store/generic.rs`, core
  **559 passed, 1 failed**). GREEN releases them in the repair transaction
  and after a successful OAuth reconnection, keeping the diagnostic.
- Whole Rust after GREEN: **560 core** (two existing ignored), **114 IMAP**
  (one existing ignored), **56 desktop, 42 auth, 47 SMTP, 23 ical**, all
  passed (36.25 s core). `cargo check --workspace --all-targets` clean.
  UI build and lint pass. Whole three-file E2E wave (sync-issues,
  attachment-budget, remote-size-limit): **5/5 in 42.7 s, zero retries**.
  Actual Settings capture inspected: alert slot, per-account card with
  folder, operation, reason, automatic-retry time and Try again.

E7–E10 now all have targeted implementation coverage. Next: one fresh review
of the whole lot, full gate, STOP 2 field checklist. No commit or delivery.

### Fresh-eyes review of the whole lot — 2026-09-07

One review at high effort over the complete uncommitted diff (58 tracked files,
39 new): eight finder angles, then one verifier per confirmed group. Ten findings
survived, ranked; all were confirmed by reading the code, none refuted at the
top of the list. Dispositions:

1. **A click paid the background disk reserve** (`body.rs`): below 64 MiB free,
   an uncached message was refused before any FETCH with a generic error.
   Fixed: admission moved to the pumps (once per window or pass), out of the
   write primitives and the click path; the legacy invitation refresh now goes
   through the checked path and records size refusals. Regression:
   `a_click_reads_a_message_without_paying_the_background_reserve`.
2. **Any IMAP NO/BAD parked a folder until a gesture** (`operations.rs`): mail-imap
   maps every tagged refusal to `Refusal`, which meant `retry_at = NULL`. Fixed:
   refusals back off like other failures (30 s to 30 min); only a corrupt local
   scope waits for a manual retry. This also makes the policy robust to errors
   crossing the shell as strings. Regression in `operations.rs`.
3. **Draft pull budget recorded as a failure** (`poll.rs`): a spent 50-draft
   allowance and a draft above 32 MiB both failed the pass with backoff and
   skipped stale-mirror pruning. Fixed: spent allowance ends the pass normally,
   an oversized draft is skipped and traced, the cursor is persisted once on a
   failed fetch, admission once per pass. No unit seam for the shell pull;
   covered by the whole-crate compile and the field.
4. **Batch planner blind to the shared byte scope** (mail-imap): a batch larger
   than the scope's remainder was cut mid-literal and poisoned the session.
   Fixed: batches planned against `min(scope left, 32 MiB)`; a pass stops
   admitting candidates below one maximal message. Regressions:
   `a_scope_smaller_than_the_batch_defers_the_rest_without_poisoning`,
   `a_shared_allowance_below_one_maximal_message_admits_nothing`.
5. **Absolute 120 s per FETCH** (mail-imap): a 30 MiB body on a slow link could
   never complete and was retried at every wrap. Fixed: the command deadline is
   120 s plus the announced size at 100 KiB/s; the pass deadline refuses new
   commands without poisoning a response in flight. ADR 0042 amended.
6. **Diagnostics never cleared** (`backfill.rs`, `cycle.rs`, `commands.rs`): a
   sweep that fetched nothing recorded "still unavailable" forever; a size
   refusal at arrival left a `bodies` issue no pass could settle. Fixed: an
   error-free sweep settles its diagnostic, a per-UID size refusal is not an
   operation failure, a mailbox with nothing eligible settles its stale row.
   Regressions: `content_the_server_no_longer_holds_is_missing_not_a_failure`,
   `an_error_free_sweep_settles_an_earlier_diagnostic`,
   `an_oversized_arrival_keeps_its_marker_without_an_operation_failure`.
7. **Arrivals beyond ten waited for a full wrap** (`backfill_cursor.rs`): the
   persisted cursor never looked above its position. Fixed: the cursor keeps a
   head UID and scans arrivals above it before the sweep resumes. Regression:
   `arrivals_are_served_before_the_historical_sweep_resumes`.
8. **Reconnect per folder on a disk refusal** (`commands.rs`): fixed, the pass
   ends on `InsufficientDisk`.
9. **Two body-pump drivers** (scheduler and UI burst): confirmed, **kept as a
   stated limit** in ADR 0043 — each pass is bounded and serialized; up to two
   passes per opportunity, UI figures can wait behind the scheduler's pass.
   Restructuring the driver is not a gate-time change.
10. **Hard-coded English "no sender address"** on Feed and reading pane: fixed
    with the existing catalogue key; Feed reuses the reading pane's grants.

Also corrected from the cleanup angles: the Accounts retry time uses the
product's time grammar; the Feed's redundant permission token is gone; typing
inside a modal no longer re-isolates the background (records inside the modal
are ignored, siblings are diffed); a consent cancelled during its preparation
is acknowledged and honored (Node regression); the generic repair reuses
`retry_operations`; three language-ratchet markers in new files. Recorded, not
changed: the Google identity check refuses an address renamed by an
administrator (intended strictness, remove and re-add is the only path — a
product question for the Chief Engineer); Reply-To handling stays in the shell
(A01, Lot 5); the six backfill wrappers and the triple `validate_uids`; every
quiet non-CONDSTORE folder pays its flags pass per full cycle (ADR 0041's
design; full-cycle latency is the pending field measurement); the two probes
with two thresholds (hook shortfall and pump reserve) coexist by design note.

The review's regressions were written with their fixes; their RED evidence is
the verifiers' reading of the pre-fix code quoted above, not a recorded failing
run. Rust after corrections: **566 core** (two existing ignored), **115 IMAP**
(one existing ignored), **56 desktop**, all passed. UI build and lint, five Node
consent cases, System coherence (68 values), language ratchet (1967 <= 1974) and
documentation links (405) pass. System A133; ADR 0042 and 0043 amended.

### First full gate — 2026-09-07, RED at step 13 (andon)

`scripts/gate.ps1` ran green through the twelve first steps and stopped on the
e2e suite: **188 passed, 5 failed, 1 skipped, 72 not run** in 19.1 minutes.
Three of the five shared one root cause exposed only by the whole suite: a
failed **connection** in the backfill pass was recorded as a `bodies` issue for
every mailbox of the account (six on the synthetic accounts), and the resulting
"Synchronization incomplete · 6 issue(s)" took priority over "Sync failed" in
the progress line and over the scheduled-send notice in the slot
(redesign-screen02 "Retry on failure", composer-integrity "draft import failure
after a successful poll", redesign-feedback-6 "scheduled send"). A connection
failure is the account's state, already carried by the connection notice and
the account backoff; it now records no per-folder issue. The desktop test that
asserted the opposite was corrected (`…_is_reported_without_a_folder_issue`);
ADR 0043 states the rule. The two other reds (feedback-14 Screener menu
intercepted by the search box, organized-mode first-card hover) timed out at
180 s in both attempts under the loaded suite; the extra notice shifting the
layout is the suspected common cause. Per the gate rules they are replayed as
whole files in isolation once, then the full gate is replayed unchanged.

Isolation replay of the five files, once each: composer-integrity 8/8,
redesign-feedback-6 4/4, redesign-screen02 61/61, organized-mode 17/17 (its
first-card hover passed in 633 ms: the loaded-suite timeout is recorded as such,
not as a product defect). **feedback-14-inbox "Settings > Screener" stayed red
in both attempts, deterministically**: the Screener's "Edit" menu is rendered
beside the Settings scrim, so the modal isolation introduced in this lot made
it `inert`, transparent to hit-testing; the click landed on the search box
beneath. This second finding of the gate is a real regression that the whole
suite alone could catch. Correction: a sibling `role="menu"` is never blocked
by the modal isolation (the keyboard handler already delegated Escape/Tab to
the shared menu). Replayed as whole files after the fix, then the full gate.

## Full gate and STOP 2 handoff — 2026-09-07

After the two andon corrections above, the full gate ran again in one call,
`scripts/gate.ps1`, unchanged, and exited 0 in **653 seconds**. No source change
followed it; this section only records its result and hands the lot to the field.

| Step | Verdict and figures | Seconds |
|---|---|---:|
| 1. Rust format | GREEN | 0.8 |
| 2. UI build and lint | GREEN; no build or lint warning | 7.7 |
| 3. Contrasts | GREEN; four themes, 440 pairs | 0.1 |
| 4. System coherence | GREEN; four themes, 68 token values | 0.1 |
| 5. Main-thread guard | GREEN; 126 commands checked | 0.1 |
| 6. Script syntax | GREEN; JavaScript, PowerShell and shell | 4.4 |
| 7. Language ratchet | GREEN; 347 files, 1967 markers against 1974 baseline | 0.5 |
| 8. IPC contract | GREEN; 125 defined/registered, 112 called by name | 0.2 |
| 9. Documentation links | GREEN; 76 files, 416 relative links | 0.2 |
| 10. Clippy | GREEN; all targets, warnings denied | 7.2 |
| 11. Rust tests | GREEN; 885 passed, four ignored | 51.0 |
| 12. Rust documentation tests | GREEN; no runnable doctest | 3.6 |
| 13. Node and UI tests | GREEN; 33 Node passed; 264 UI passed, one flaky, one optional benchmark skipped | 577.2 |

Rust totals: auth 42, core 566, iCal 16 + 7, IMAP 115, render 34, SMTP 47,
desktop 56, core examples 2. The UI flaky is recorded, not hidden: the same
Screener scenario of feedback-14-inbox (its first attempt timed out at 180 s
with an empty toast after the corrected menu click, the automatic retry passed
in under a second; the whole file had passed 7/7 in isolation after the menu
fix). It is put on the field list below rather than erased by another gate.
The whole lot remains staged and uncommitted; no CI claim.

Field handoff (real accounts remain the Chief Engineer's responsibility; every
step is reversible and touches only disposable messages):

1. **Compose and reply routing (E7a).** Reply-all to a message carrying a
   Reply-To: the Reply-To lands in To, the other recipients in Cc, oneself
   absent. Send a disposable message with only a Cc, then only a Bcc: both
   leave the queue; the received copy shows no Bcc.
2. **Feed scope (E7b).** Switch account in the Feed after scrolling two pages,
   then back: no stale card from the other account; delete a page-two card and
   refresh: it is gone.
3. **Calendar (E7c, D8).** Open an invitation with a single-occurrence change
   and one recurring series: the single occurrence answers with its original
   date; a THISANDFUTURE or multi-event message stays readable with the
   explanation and no reply buttons. A legacy invitation card offers "Verify"
   and keeps its cached message if the refresh fails.
4. **Generic account repair (E8, D6/D7).** Settings > Accounts > Repair on the
   IMAP account: host, login and email frozen, password/ports editable; an
   empty password means unchanged; a wrong password fails on both protocol
   labels and keeps the old configuration; a right one reconnects without a
   test message.
5. **OAuth manual link (E8).** Reconnect a Google or Microsoft account, choose
   the manual link, copy it, cancel: the notice closes, nothing is saved;
   retry to completion.
6. **Keyboard and modals (E10).** In the list, Ctrl+Space checks without
   opening, Shift+Space extends, Escape clears; Tab inside Settings, Compose
   and Feedback never reaches the background; closing restores the trigger.
   Settings > Screener > "Edit" on a decision: the menu opens and its choice
   applies (the gate's one flaky scenario).
7. **Image consent (E10, D-42).** "Show images" on one message, then the new
   per-message "Block again": images stay blocked on reopening; a sender rule
   removed in Settings blocks an open message at once.
8. **Connection state (E10, D-44).** With Wind open, revoke or break one
   account's password provider-side: Settings shows the account unavailable
   within the next attempt, without a restart; the other account keeps syncing.
9. **Partial diagnostics (E9).** If a folder fails while INBOX succeeds, the
   status bar reads "Synchronization incomplete · n issue(s)", "View details"
   opens Accounts with folder, operation, reason and the retry time in the
   product's grammar; "Try again" releases it. Going offline must NOT produce
   such issues (only "Sync failed"/offline).
10. **Bounded work (E9, D9).** Watch one full cycle and one manual sync on the
    largest account: record the cycle duration from the trace (this is the
    pending field measurement of ADR 0041's quiet-folder flags pass). A message
    above 32 MiB, if one exists, keeps its envelope and explains the webmail
    fallback; clicking a large but admissible message on a slow link completes.
11. **Contacts (E10, ADR 0040).** Remove a disposable secondary account:
    autocomplete still offers addresses seen through the remaining account.

Run the committed scripts, in order, after closing Wind:

```powershell
powershell -ExecutionPolicy Bypass -File "C:\Users\smonc\OneDrive\Documents\Repositories\wind\scripts\field.ps1"
```

```powershell
powershell -ExecutionPolicy Bypass -File "C:\Users\smonc\OneDrive\Documents\Repositories\wind\scripts\run-wind.ps1"
```

Await the per-item field verdict before commit, push and candidate CI. Lot 3 is
not delivered; Lots 4–6 of the audit remain open.

## STOP 2 verdict — 2026-09-07

The Chief Engineer replayed the eleven items on the real accounts: **“1-11 OK”**,
zero KO (D10). No cycle duration figure was supplied with the verdict, so the
full-cycle latency of ADR 0041 stays an open field measurement. Per D10 the lot
is committed locally and **not pushed**: CI, and with it the lot's closure, wait
for the single grouped push of the audit's changes. Until that push, `main`
on GitHub still points at `a58c1cf`.

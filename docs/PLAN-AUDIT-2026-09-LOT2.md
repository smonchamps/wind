# Audit Lot 2 — irreversible effects and account lifecycle

Opened: 2026-09-06. Baseline: `d13811c340568b79ffadb17f9d4ffa600a46b509`.
Parent: [six-lot audit program](PLAN-AUDIT-2026-09.md), E4–E6.

**JOB CLOSED on 2026-09-06 — full field validation.** E4–E6 delivered in `0374e43fd29c00acb3e1c6e3e3da77ecf89366d7`; [CI 34053044227](https://github.com/smonchamps/wind/actions/runs/34053044227) is green on Windows, macOS Intel and Apple Silicon. The Chief Engineer authorized starting Lot 2
on 2026-09-06. The program, Lot 1 field approvals and its delivery remain valid.

## Reproduced findings

All experiments use synthetic local data and loopback servers. No real account,
credential or mailbox is used. These are investigation proofs, not a production
gate or evidence that the candidate is already integrated.

| Finding | Evidence on the current implementation | Required outcome |
|---|---|---|
| B01, ambiguous SMTP | Lost final acknowledgement: two submission opportunities produce two complete payloads, one duplicate. Core flush repeats the same behavior in 3/3 runs. | Persist uncertainty; no automatic second submission. |
| A06, account removal | Hold `MailTransport::send`, delete its account through a second Store, then release: 3/3 runs report one accepted send after deletion, with zero retained outbox rows. | Close admission, drain existing work before credential removal/purge, preserve the delivery decision until resolved. |
| B09 follow-up, lost enqueue IPC acknowledgement | Core repeated enqueue with the same edit token returns the same ID, 3/3. But `save_draft_edit` fails after consumption, and Compose always calls that save before retrying enqueue. | Resolve an uncertain enqueue by token before another save or send; one durable outbox entry. |
| B26, failed instance lock | The actual lock function returns an error for a synthetic file used as a directory, 3/3. `main` explicitly continues on this error or an unavailable path (static startup continuation, no real app launched). | No writers or window without a successfully acquired guard. |
| B04, unsafe purge | Existing `mail-imap` test `a_server_without_uidplus_never_gets_uid_expunge` passes while expecting COPY followed by global EXPUNGE. | No global EXPUNGE and no preparatory mutation if safe targeting is unavailable. |
| B28, repeated archive copy | Archive and trash bypass MOVE; transient action replay repeats the whole COPY/delete operation. The pending-action schema has no confirmed-copy phase (static). | Shared MOVE path; durable fallback phases prevent repeated confirmed COPY. |
| B22, false send announcement | Compose announces the sent toast immediately after `queue_send`, before the SMTP flush (static). | Say queued until a confirmed acceptance exists; keep scheduled and uncertain states distinct. |

The core proof is `target/lot2-proof`, built against the actual core source and
the repository lockfile after an initial standalone dependency-resolution run.
Its sources are preserved in
[`docs/evidence/audit-2026-09/lot2-proof/`](evidence/audit-2026-09/lot2-proof/)
with the command that restores and reruns them: `target/` is ignored and any
`cargo clean` erases the copy that ran.
The lockfile-pinned rerun gives the same 3/3 results. It uses the core flush and
two SQLite connections, not the Tauri command scheduler: the missing shell
coordination must still be covered by integration tests. Its two unused-code
warnings belong to the standalone inclusion of the instance module, not a gate.

Additional confirmed boundary defects discovered during this investigation:

- Lettre accepts any positive SMTP response, including intermediate 3xx. Wind
  treats any successful return as sent: an unexpected final 354 becomes false
  success. Require final DATA acceptance 250; quarantine unexpected replies.
- QUIT failure after final 250 does **not** turn the pinned transport into an
  error: lettre deliberately ignores it. Preserve this behavior.
- Rejection of the second recipient aborts before DATA. Do not introduce partial
  delivery while changing the transaction handling.
- The pinned IMAP crate's `uid_copy` interpolates an unquoted destination, unlike
  `uid_mv`. A replacement must quote/escape mailbox names and reject CR/LF/NUL.
- Generic special-use `All` does not guarantee Gmail's archive semantics.
  Expunge-only archive needs a verified provider capability; unknown servers
  with only an All folder must not silently lose source content.
- `connect_accounts` can adopt a captured successful connection after removal.
  `reset_sessions` already avoids resurrection, but does not protect this path.

## SMTP hard point — measured alternatives

Pinned dependency: lettre 0.11.22, current features, no pool. Its high-level
transport reopens a connection per send. A response-less error exposes no
transaction stage: greeting EOF and a lost final DATA acknowledgement can have
the same public predicate. Diagnostic string parsing is not an acceptable fix.

Two isolated worktrees contain standalone throw-away spikes. Both were measured
on Windows ARM64, Rust 1.97.1, on ten scenarios repeated three times. Each uses a
tiny synthetic payload, ephemeral loopback TCP and bounded timeouts. Their own
standalone lockfiles are retained; only the lettre version and enabled features
are asserted identical. Neither experiment exercises TLS or authentication.

| Option | Result | Cost and limits |
|---|---|---|
| A — keep SmtpTransport; conservative typed classification | 30/30 scenario results stable. Lost final ACK replay: baseline 2 payloads, A 1. Greeting/MAIL/RCPT EOF: Unknown despite zero payload. Final 354 becomes Unknown. | 19 formatted classifier lines; 253-line complete fixture. 33 connections including replay in 61 ms. Some safe-to-retry failures need manual verification. Public client/TLS exceptions rely on pinned-source inspection and need maintained tests. |
| C — drive public SmtpConnection command/message phases | 30/30 pass. Greeting/MAIL/RCPT EOF: Retry with zero payload; post-DATA EOF and unexpected final 354: Unknown. Recipient refusal sends no DATA. | 44 lines of transaction/classification in a 126-line fixture. Saved per-case medians 0.416–0.582 ms. Requires taking responsibility for secure setup, required STARTTLS, authentication, capability checks, SMTPUTF8/8BITMIME and refresh integration. |

The latency figures are fixture diagnostics with different harnesses, **not a
performance ranking**. Neither option proves end-to-end storage recovery or real
provider compatibility. The difference is automatic recovery coverage versus
adapter responsibilities. The intermediate option (separate connection setup,
keep lettre's send transaction) still hides MAIL/RCPT failures and adds setup
responsibility; it was inspected but not measured as a third candidate.

Artifacts, outside the production workspace:

- `.claude/worktrees/lot2-smtp-a/spikes/lot2-smtp-a/MEASUREMENT.md`, `results.txt`.
- `.claude/worktrees/lot2-smtp-c/spikes/lot2-smtp-c/REPORT.md`, `measurements.csv`.

**Recommendation: A for this lot.** It meets ADR 0003's preference for delaying
an uncertain send over duplicating it, retains the existing secure connection
stack, and keeps the change bounded. Its manual-verification cost is explicit.
Option C remains feasible if the Chief Engineer chooses precise phase handling.

## Implementation contract

### E4 — truthful delivery and exclusive writers

1. Add typed unknown-delivery and connection/authentication outcomes. Distinguish
   setup failure, explicit refusal, accepted submission and uncertain submission;
   behavior must not depend on localized or prefixed error strings.
2. Persist unknown sends as interrupted with their reason, Message-ID and files.
   Subsequent flushes, including after restart, submit zero additional copies.
   Reuse the existing check-Sent/Resend/Discard decision. Never interpret an absent
   Sent-folder echo as proof of non-delivery.
3. Preserve final 250 success despite failed QUIT. Reject unexpected final 3xx as
   success. Keep second-recipient refusal ahead of DATA.
4. Fail startup before any writer when the database location or exclusive lock
   is unavailable. Retain patient locking for updater relaunch and the existing
   second-instance behavior. Do not introduce an unguarded fallback.
5. Announce queued after local enqueue, sent only after acceptance; scheduled
   sends still name the deadline. Use existing notice, status and toast surfaces.

### E5 — operation lifetime and enqueue acknowledgement

Reuse Lot 1's atomic enqueue and sender transfer. They are not reimplemented.

- A captured job carries the account incarnation, not just an email or reusable
  row ID. It obtains an active-work lease immediately before execution, not when
  collecting a queue of jobs for all accounts. Removal atomically closes admission
  and stops watchers/new tasks before waiting for active leases to drain.
- Check closing state between network operations. An SMTP handoff already in
  progress cannot be recalled: let its bounded transport finish and persist its
  result before removal may purge. No global command lock is held while waiting.
- Vault removal and Store purge happen only after quiescence. Timeout or failure
  keeps the account/data and reports the problem. An unresolved interrupted send
  blocks purge until the existing explicit delivery decision is resolved; removing
  an account must not silently erase that warning.
- Reject late connection/reconnection publication and stale work after removal
  or re-addition. Cover sync, IDLE, direct body/file reads, gesture passes, outbox,
  draft push and body backfill, including tasks waiting behind existing locks.
- On lost enqueue acknowledgement, query the existing token result before saving
  again. Freeze the submitted version while this check is pending. A committed
  result closes the composer truthfully; an uncommitted result permits retry;
  unavailable verification retains the content and an explicit pending state.
- A cancellation button must cancel admission closure if offered, not merely hide
  an ongoing destructive operation. Keep the removal surface honest while busy.

### E6 — scoped IMAP mutations and durable uncertainty

| Server capabilities | Move/archive-to-folder/trash | Pure purge, including draft tombstone |
|---|---|---|
| MOVE, with or without UIDPLUS | Shared UID MOVE primitive | UIDPLUS required |
| UIDPLUS, no MOVE | Journal COPY, then targeted deletion | UID STORE + UID EXPUNGE |
| Neither | Unsupported before any COPY/STORE | Unsupported before any STORE |

Persist source incarnation, fixed destination and an in-flight phase **before**
mutating the server. A confirmed COPY advances to a durable copied phase; recovery
from that phase only removes the source. A lost confirmation becomes uncertain,
never an automatic repeat of COPY or MOVE. Persist uncertainty across disappearance
of the source envelope and UIDVALIDITY changes, while forbidding mutation of old
UIDs. A reset cannot make stale operation identities current again.

COPYUID is useful corroboration, not a cure for a lost acknowledgement; it may be
absent, and Message-ID search alone cannot distinguish preexisting copies. Parse
tagged and untagged COPYUID through the existing parser with strict identity
validation. An inconsistent result remains uncertain. No generic marker-keyword
scheme, no protocol fork, no global EXPUNGE fallback.

The notice for an uncertain mutation explains that the move may already have
occurred and directs the user to inspect source/destination before an explicit
new action. Confirmed stages can resume automatically; uncertain stages cannot.
Unsupported-server incidents retain the source and explain the missing capability.

Protocol references: [UID EXPUNGE](https://www.rfc-editor.org/rfc/rfc4315.html#section-2.1),
[COPYUID limitations](https://www.rfc-editor.org/rfc/rfc4315.html#section-3),
[MOVE failure semantics](https://www.rfc-editor.org/rfc/rfc6851.html#section-3.3),
[All special use](https://www.rfc-editor.org/rfc/rfc6154.html#section-2).

## Verification and delivery

Promote the diagnostic scenarios to maintained RED tests before each implementation
wave. Add process/restart and held-transport tests for uncertainty and removal,
including another account continuing to work. Test lost IPC acknowledgement after
the real enqueue commits, not a failure injected before it.

For IMAP, use a stateful fake server retaining synthetic source/destination state
between connections: MOVE-only, UIDPLUS-only, neither; a third-party Deleted UID;
cuts before/after COPY effect, acknowledgement, copied persistence, STORE and
EXPUNGE; missing/inconsistent COPYUID; source/destination generation reset; quoted
destination names. Repeated recovery must create no accumulating copies and must
preserve every unrelated message.

Play relevant E2E specs as whole files, RED then GREEN; amend System A122 and
ADR 0003/0030 where their decisions evolve. Inspect the first visible increment
before extending it. Final review, one full gate, STOP 2 on chosen disposable
messages/accounts, then documentation, commit, push and green CI. No real sending
or destructive mailbox experiment is authorized to the agent by the probes.

## Chief Engineer decision D4

**2026-09-06 — answer, verbatim: “A”.** Keep SmtpTransport and conservatively
quarantine submissions without a conclusive acknowledgement. The measured
manual-verification cost is accepted. STOP 1 is complete.
The prior go for Lot 2 authorizes its scope and investigation; D4 selects this
newly measured behavior. Earlier D1–D3 and Lot 1 approvals are not reopened.


## E4 implementation checkpoint — 2026-09-06

Option A is integrated in the production SMTP adapter and core outbox. Unknown
submissions become interrupted in one state/reason/counter write, keeping files
and Message-ID. Connection checks return typed connection/authentication errors;
the shell only refreshes OAuth after an authentication refusal. The existing TLS
transport is retained. Startup lock errors and missing data locations now exit
before opening a Store or Tauri window; the existing pre-lock legacy-folder rename
remains in its required order (creating the destination first would defeat it).

Maintained SMTP/core integration tests ran RED before implementation: 6 failed,
5 passed. Failures demonstrated lost-ACK retry after reopening, missing quarantine
at greeting/MAIL/RCPT EOF, lost final DATA ACK and false success on final 354.
GREEN: 45 SMTP tests, including 16 wire/recovery scenarios, 28 core outbox tests,
and 4 desktop instance tests. The startup branch is a direct error-to-exit change;
its already measured lock failure and existing lock tests are the evidence, not
an invented failing assertion on a new pure wrapper. No failure dialog was opened
on the real workstation. TLS negotiation/certificate failure and real OAuth remain
outside the loopback fixture's claims; required STARTTLS refusal without cleartext
submission and pre-MAIL SMTPUTF8 refusal are covered.

The two UI regressions ran RED against the old wording, then the four impacted
spec files ran GREEN: **78 passed in 1.2 minutes, no retry/flaky result**. The queued
case reads a real disposable outbox row. The unknown case substitutes the outbox
part of IPC status responses and renders the actual notice; it does not claim a
real SMTP-to-Tauri failure injection. An initial esbuild sandbox restriction and
two ineffective attempts to replace immutable Tauri internals were harness issues,
excluded from the functional RED evidence. The final injection wraps fetch and
asserts that a status response was actually substituted.

System A122 records this first visible increment. The early visual question was
sent on 2026-09-06 with native screenshots of both French surfaces. The reply was:

> Pour "message mis en file d'envoi", remonte le glyphe de quelques pixels pour donner une impression d'alignement. OK pour l'autre. <!-- lang:fr -->

The uncertain-delivery notice is approved. The shared toast checkmark moves up
2 CSS px for optical alignment; the text and box keep their positions. This is
a cosmetic adjustment verified by a fresh native capture and the existing whole
spec, without inventing a RED assertion that only mirrors a CSS declaration.
Targeted clippy passes for mail-core, mail-smtp and wind-desktop, all targets,
with warnings denied. Five catalogue/placeholder checks, language ratchet
(302 files, 1968 markers against 1974), 397 relative documentation links,
System token coherence and whitespace checks pass. Rustfmt moved the language
annotation off two French dialog strings; placing it on each literal fixed that
text-gate failure without changing runtime behavior.
The enqueue-acknowledgement part of E5 is implemented below; account lifecycle and E6 remain pending. Fresh-eyes final review, full gate, STOP 2,
commit/push and CI remain due for the complete lot. No delivery claim yet.

Additional E6 integration observation: App's refused-action notice reads the old
`actions_refusees` field while OutboxStatus serves `refused_actions`. Cover the
actual status schema when implementing the durable IMAP incident surfaces.


## E5 enqueue acknowledgement checkpoint — 2026-09-06

The requested toast adjustment is visually inspected: 2 CSS px upward, with the
text and surface unchanged. Both delivery-state tests pass (38.2 seconds).
The uncertain-delivery notice retains its explicit visual approval.

A new real-IPC failure fixture lets queue_send commit, consumes its success
response, then reports a missing acknowledgement to the composer. RED: the first
two tests failed (composer left open despite one durable entry; no verification
state); the pre-commit failure control passed. The core now resolves an enqueue
by its existing edit token. A missing receipt is retryable only while the same
account still owns a live editing snapshot; stale/foreign tokens remain errors.
The composer queries this result before further saving. An unavailable lookup
keeps the submitted version inert and offers a status check; Escape/native close
cannot discard it. A successful lookup closes without a second queue_send.

GREEN: 12 draft-edit core tests and 13 UI tests across the complete enqueue
recovery, composer integrity and delivery states files. The new core check also
covers a receipt after the send state becomes sent and wrong-account/token
lookups. A missing glyph name on the new verification button was caught in
inspection and replaced with the existing sync icon; its capture replay follows.

Account-removal admission/draining and E6 remain open. Final review, full gate,
field validation and publication still belong to the complete lot.

## Account-removal and IMAP safety checkpoint — 2026-09-06

The maintained Store regression failed on Sending before implementation: deletion
silently erased the account and its delivery decision. The Store now refuses
removal for Sending or Interrupted, including attachment bytes, and permits
removal of another account. The explicit discard of an interrupted send permits
subsequent removal. A typed UnresolvedDelivery error drives this check; the shell
preflights before touching the vault and the Store rechecks in its transaction.
GREEN: all 29 outbox tests. This guard alone does not close admission to queued
jobs or drain active operations; the lifecycle coordination remains due.

Six maintained IMAP wire tests failed before the adapter change: unsupported move
and draft purge mutated anyway; archive/trash ignored MOVE; COPY did not quote
destinations; CRLF in a destination injected an extra command into the synthetic
server; a generic All role permitted destructive archive. The adapter now refuses
unsupported mutation before COPY/STORE, shares MOVE for archive/trash, uses only
targeted UID EXPUNGE and quotes/escapes COPY destinations after rejecting CR/LF/NUL.
Expunge-only archive requires INBOX, All Mail and X-GM-EXT-1; it is refused for
other selections. The extension is documented by
[Google](https://developers.google.com/workspace/gmail/imap/imap-extensions).
The follow-up control exposed a refused generic archive despite a usable named
Archive folder: one RED. Filtering the All role by the advertised Gmail extension
before strategy selection preserves this safe destination. Positive Gmail and
negative Trash/no-UIDPLUS controls also pass.
GREEN: 89 IMAP tests, one optional benchmark ignored. These wire checks prove
commands, not durable recovery: COPY/MOVE phase journaling, COPYUID validation,
generation-reset incidents and the status-notice schema fix remain due in E6.
Targeted clippy passes for desktop/core/IMAP/SMTP with warnings denied; language,
IPC, documentation links, System coherence and whitespace checks pass.
The complete core crate also passes: 503 tests, two optional tests ignored,
17.51 seconds. This remains an implementation checkpoint, not the final lot gate.

## IDLE cancellation hard point — Chief Engineer decision D5

The existing IDLE heartbeat is 180 seconds. A closed admission flag cannot wake
its blocking read. The isolated spike tested the actual imap 3.0.0-alpha.15 and
rustls 0.23.41 on Windows ARM64, with synthetic loopback data only. Ten attempts
each with a cloned socket and a shared original handle stayed blocked until the
server's emergency signal at about three seconds: shutdown is not an effective
cancellation mechanism on this host.

Local TCP read slices of 100 ms, with the logical 180-second heartbeat preserved,
did interrupt IDLE. TLS trials gave 60/60 cancellations in 69–90 ms across silent
IDLE, final DONE reply and partial TLS record, above/below the TLS wrapper. Twenty
fragmented-record controls resumed correctly without cancellation. Each silent
10-second witness incurred 90 local read timeouts and zero extra DONE/IDLE traffic.
These counters do not measure CPU or battery use. Cancelled connections are
terminal; they are never resumed. The IDLE destructor can swallow its cancellation
error, so the token must also be checked after watch returns.

Evidence remains in the isolated worktree:
`.claude/worktrees/lot2-smtp-c/spikes/lot2-idle-cancel/REPORT.md` and
`REPORT-TLS.md`, with CSVs and runnable fixtures. macOS, a blocked write, DNS and
connection setup were not measured; these figures are not a whole-account drain
guarantee. SMTP is outside this cancellation mechanism.

| D5 option | Observable behavior and cost |
|---|---|
| Fast IDLE stop — recommended | Check the watcher-only stop token through 100 ms local read slices; measured 69–90 ms stop, roughly nine local wakeups/second/account, no added IMAP commands. Other work still drains or produces an explicit timeout without purge. |
| Keep passive IDLE waits | No extra local wakeups. Account removal waits for the natural end of IDLE and may take several minutes; its progress surface must state that wait. |

Recommendation: fast stop, with a token below TLS so ordinary slice timeouts stay
inside the transport. The spike found no material latency difference between the
two placements; the lower placement also covers TLS-internal I/O checks. This is
a local cancellation mechanism, not a shorter polling heartbeat. Its production
implementation is authorized by D5.
**2026-09-06 — D5 answer, verbatim: “Ok go”.** The Chief Engineer accepts the
recommended fast stop with watcher-only local read slices below TLS. D4 remains
unchanged; no additional scope or publication approval is requested.


## D5 and lifetime integration checkpoint — 2026-09-06

The watcher now checks its stop token through 100 ms TCP read slices below TLS.
Five maintained cancellation tests pass: terminal raw cancellation, logical
heartbeat preservation, ordinary poll timeouts, quiet/partial/DONE TLS stop and
fragmented TLS resumption. Synthetic certificate fixtures use a fixed test clock;
production trust and hostname verification are unchanged.

Per-account tickets identify the captured incarnation. Leases cover network work
and its Store outcome; retirement closes admission, stops IDLE, waits outside the
command mutex for up to 30 seconds, then preflights unresolved sends before vault
and Store removal. Timeout keeps the account and reopens a fresh incarnation while
sharing the old active-work counter: a second removal must still wait for an old
SMTP handoff. Unknown-identity OAuth flows also drain before vault removal. Local
saves, enqueues and group mutations reject closing accounts. Late refreshes, echo
cleanup, backoff publication and gesture-flight coalescing retain the old ticket.
Four lifecycle tests pass, including a held SMTP transport: accepted submission
persists before purge; unknown submission retains the account; a second queued
message is never attempted after closure. All 42 desktop tests pass.

The removal-dialog RED found an enabled Cancel button during an operation that
cannot be cancelled. The implementation keeps the progress card visible and
disables Cancel/Remove/Done until completion; its whole-file GREEN is pending.

## Durable IMAP phase checkpoint — 2026-09-06

Two stateful loopback regressions fail with two copies before implementation:
COPY acknowledgement loss, and a cut at STORE after a confirmed COPY. The first
fixture run failed at LOGIN because accepted sockets inherited nonblocking mode
on Windows; that fixture failure is excluded from functional RED evidence.

The action journal now resolves capabilities/destination before transfer, persists
source and destination generations, and writes in-flight before COPY/MOVE. A
confirmed copy advances durably before targeted source removal; that removal can
resume without copying. Lost replies remain uncertain. Separate effect rows retain
incidents when the source/action disappears or its generation resets. A nullable
foreign key prevents an orphan attaching to a reused pending-action rowid.
COPYUID is parsed through imap-proto for tagged and untagged replies, accepting an
absent mapping but rejecting mismatched generations, source/destination sets,
zero UIDs or contradictory mappings without expanding unbounded ranges.

GREEN: 506 core tests and 100 IMAP tests; two core benchmarks and one IMAP benchmark
ignored. Coverage includes repeated reconnections, cuts before COPY/after effect/
at STORE/after EXPUNGE, MOVE without UIDPLUS, neither capability, unrelated Deleted
UID retention, source/destination generation changes, contradictory COPYUID,
persisted phases reopened through another Store and orphan rowid reuse. Existing
transient-retry assertions now exercise repeatable flags; ambiguous moves instead
retain an incident without automatic replay. Incident surfaces and final gates
remain due; this is not a delivery claim.


## Final review corrections in progress — 2026-09-06

One independent fresh-eyes review examined the complete diff. Three confirmed
findings are retained: discarding Sending could erase an active SMTP handoff,
a cancelled queued snapshot could still send (or claim a recycled ID), MOVE NO
could lose evidence of a partial effect, and the uncertain-move notice lacked
identifying context (the two SMTP races form one finding). The proposed Compose
double-click issue was withdrawn: the containing scrim is inert during sending
and verification, so the alleged native-click path was not reachable.

Maintained RED proves both SMTP races, then GREEN passes all 32 outbox tests:
Sending refuses discard; each snapshot entry must atomically claim its own id,
Message-ID, account and queued state before the transport. The stateful MOVE-NO
regression also fails before correction (zero incidents despite a copied message),
then all seven mutation-wire tests pass. RFC 6851 section 3.3 allows an effect
despite MOVE NO; COPY NO remains a refusal because RFC 3501 section 6.4.7 requires
restoring the destination on unsuccessful COPY. Those are distinct guarantees.

The prior complete targeted UI wave passed 18/18 across composer integrity,
delivery states, enqueue recovery and account removal. The review's new UI RED
checks concurrent decision blocking and visible account/message context. This
fixing wave and the final full gate remain open. No second review is requested.


The review UI RED completed with three passes and two expected failures: missing
incident context and enabled concurrent send decisions. The core now snapshots
subject/sender when a removing action is enqueued, before its envelope disappears;
that context survives generation reset and orphaning. The shell adds the account
address, and the notice displays all available identifying fields. The additional
persistence regression passes, along with all 510 core tests (two ignored).
The shell checks Message-ID on resend, discard and scheduled cancellation so a
stale notice cannot target a recycled outbox ID. Both send decisions remain
unavailable through the awaited operation, including during periodic status reads.
The complete delivery-state UI file passes 5/5 with retries disabled (60 seconds).
The actual WebView2 incident capture shows the account, subject and sender, with
vertically centered text and acknowledgement button. The final full gate is due.


## Full gate and STOP 2 handoff — 2026-09-06

The one fresh review is complete; all three confirmed findings are corrected.
The final full gate ran in one call, `scripts/gate.ps1`, and exited 0 in 476 seconds.
No production change followed this gate; this checkpoint only records its result.

| Step | Verdict and figures | Seconds |
|---|---|---:|
| 1. Rust format | GREEN | 0.6 |
| 2. UI build and lint | GREEN; no build-plugin or lint warning | 15.7 |
| 3. Contrasts | GREEN; four themes, 440 pairs | 0.1 |
| 4. System coherence | GREEN; four themes, 68 token values | 0.1 |
| 5. Main-thread guard | GREEN; 119 commands checked | 0.1 |
| 6. Script syntax | GREEN; JavaScript, PowerShell and shell | 3.3 |
| 7. Language ratchet | GREEN; 308 files, 1968 markers against 1974 baseline | 0.4 |
| 8. IPC contract | GREEN; 118 defined/registered, 110 called by name | 0.1 |
| 9. Documentation links | GREEN; 70 files, 398 relative links | 0.2 |
| 10. Clippy | GREEN; all targets, warnings denied | 11.6 |
| 11. Rust tests | GREEN; 776 passed, four ignored | 51.0 |
| 12. Rust documentation tests | GREEN; no runnable doctest | 8.2 |
| 13. Node and UI tests | GREEN; 28 Node passed; 228 UI passed, one flaky, one optional benchmark skipped | 384.4 |

Rust totals: auth 24, core 510, iCal 16, IMAP 101, render 34, SMTP 45, desktop 44, core examples 2.
The UI retry is recorded rather than hidden: organized-mode.spec.js:129 timed out
waiting to hover the first Feed card at line 138; the full serial file passed on
its automatic retry (the gesture took 634 ms). The final report has zero unexpected
failures. The latest three main CI runs are green, including baseline d13811c,
CI 34039110994. That comparison does not constitute CI validation of this uncommitted
lot. No additional full gate was run to erase the flaky result.

Field handoff (real accounts remain the Chief Engineer's responsibility):

1. Send a disposable message to oneself with two files: queued wording and the
   approved raised glyph; one received message with two readable files.
2. Schedule a disposable message, then cancel before its deadline: one complete
   draft returns, with its files; no later send of that cancelled entry.
3. On disposable messages, archive, move and trash, then manually synchronize:
   expected destination, no duplicate in that destination or source resurrection.
   If an uncertainty notice occurs naturally, verify the displayed account/message
   context before acknowledging; do not deliberately provoke a real ambiguous send.
4. If a disposable secondary account is available, remove it: progress stays
   visible, concurrent decisions are disabled, the rest of the window responds.
   Active-work waiting has a 30-second bound; timeout keeps the account. Record the
   elapsed time and confirm the other account still synchronizes.

Run the committed scripts, in order, after closing Wind:

```powershell
powershell -ExecutionPolicy Bypass -File "C:\Users\smonc\OneDrive\Documents\Repositories\wind\scripts\field.ps1"
```

```powershell
powershell -ExecutionPolicy Bypass -File "C:\Users\smonc\OneDrive\Documents\Repositories\wind\scripts\run-wind.ps1"
```

Await the per-item field verdict before commit, push and candidate CI. Lot 2 is
not delivered and the remaining lots of the audit are not closed.


## STOP 2 verdict — 2026-09-06

The Chief Engineer answered verbatim: **“1-4 OK”**. All four field checks are
accepted: send with two readable files and the approved glyph alignment, scheduled
cancellation restoring a complete draft, archive/move/trash after synchronization,
and secondary-account removal with the other account continuing to work.
Zero KO findings were reported. No elapsed removal time was supplied; the 30-second
bound is an implementation/test fact, not a newly measured field figure.
No implementation change follows the field GO. Commit, push and candidate CI are
now authorized by the workflow; the audit's later lots remain open.


## Delivery and closure — 2026-09-06

Implementation commit: `0374e43fd29c00acb3e1c6e3e3da77ecf89366d7`, pushed to
origin/main after the Chief Engineer explicitly authorized that destination.
[CI 34053044227](https://github.com/smonchamps/wind/actions/runs/34053044227) completed
successfully: Windows 4m20s, macOS Intel 2m04s, Apple Silicon 2m03s; UI/coherence and
dependency audit also green. The tree and origin/main matched at closure.

The required pre-push full gate passed in 227 seconds: 776 Rust, 28 Node and
229 UI tests passed, four Rust tests ignored, one optional UI benchmark skipped,
zero flaky results. The earlier 476-second gate's one UI retry remains recorded
above. The initial summary and implementation commit message omitted the two core
example tests; 776 is the corrected all-targets Rust total, not an added test wave.

Field GO: **“1-4 OK”**, 2026-09-06. Zero KO findings. The earlier visual request
raised the queued-toast glyph by 2 CSS px (System A122); no touch-up followed this
final field verdict. No real-account removal duration was supplied.

Owned limits: SMTP option A can require manual verification for a response-less
failure before DATA; precise transport phase control remains an explicitly
unchosen alternative, now indexed as D-63 in [DEBT](DEBT.md). IDLE read cancellation
does not promise cancellation of DNS, connect, writes or an already started SMTP
handoff. IMAP operations lacking safe capabilities are refused before mutation.
Existing D-27 retry scheduling and D-59 structural attachment identity remain open;
this lot does not silently close the remaining audit program.

Kaizen: **W3 = two full local gates** (476 s and the required pre-push 227 s);
**STOP 2 KO = 0**. T1 input-equivalent usage is unavailable for this Codex task:
`scripts/measure-sessions.mjs` reads Claude Code transcripts, not this runtime's
usage. No substitute or zero consumption is asserted. This plan and STATE retain
the durable handover. Next audit work: Lot 3, E7–E10, in the parent plan.

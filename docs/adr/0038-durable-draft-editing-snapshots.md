# ADR 0038 — Durable draft editing snapshots

Date: 2026-09-06 · Status: accepted and delivered in `e95adb4`, field validated, CI green.
Chief Engineer: D3 of [PLAN-AUDIT-2026-09](../PLAN-AUDIT-2026-09.md).

## Context

The remote draft pull could delete the row and cascading files that an open
composer still represented. Copying attachments from that row during a later
conflict save was too late. A timestamp alone also cannot distinguish two
incarnations of a reused SQLite row ID.

## Decision

- One persistent editing session owns the opened content, its immutable draft
  incarnation and revision, and references to immutable attachment payloads.
  Opening acquires this version transactionally. It does not copy attachment
  bytes or mark the draft for another remote push.
- Add, remove, save, send and discard belong to the session token. A conflict
  saves the edited version separately and reports the fork. A sender change
  moves an unchanged current version and records the old account's remote UID
  for deletion; a concurrent different version is preserved.
- Closing releases the session. Startup releases an abandoned untouched session
  or materializes its committed changes if the source mirror disappeared.
  Browser keystrokes that never reached autosave cannot be recovered.
- Enqueue copies the session's files into the durable outbox and consumes the
  exact draft version in one transaction. The outbox remembers the request token.
  Frozen reply headers survive UID resets, conflict forks and sender changes;
  local conversation links also check the source mailbox generation.
- Deleting a file reference collects only its payload when neither a draft nor
  the session refers to it. There is no retained revision history or full-table
  garbage collection on every gesture.
- Remote import persists recipients, priority, HTML, reply headers and all
  retained MIME files atomically. Embedded images are inlined into the HTML.
  An over-budget message is refused as a whole. Imports run one message at a
  time; old mirrors are removed only after all required imports succeed, and
  only if their incarnation, revision and remote UID still match. Completed
  imports may remain alongside old mirrors after a later failure; retry is
  idempotent.

## Migration and recovery

Legacy attachment rows become immutable payloads and references in one SQLite
transaction. Payloads are copied with SQL, one file at a time. The UI uses the
existing migration screen and cancellation checkpoint between files. The shell
checks conservative copy/journal headroom before opening for migration.

This incompatible storage change runs after the older schema/adoption passes
and contact backfill. A failure before this unit leaves legacy file rows usable.
Cancellation or disk exhaustion inside this unit rolls it back; completion is
reported after commit. Independently committed earlier migrations are retained.

`Store::revert_draft_file_storage` reconstructs legacy attachment rows in a
transaction, recovers committed editing changes first, and consumes the Store.
It must run with application writers stopped before a binary downgrade. Extra
additive columns remain compatible with the earlier binary. A cancelled reversal
retains the reference layout. A later upgrade is supported.

## Evidence

The approved Python alternatives and their limitations are recorded in D3.
Production Rust tests cover incarnation reuse, unchanged open/close, conflict
recovery, file-only changes, rollback, ownership, sender transfer, generation
reset, atomic enqueue, discard and garbage collection. Synthetic current/legacy
file databases exercise cancellation, reversal, re-upgrade and `SQLITE_FULL`.

One local Rust WAL measurement with 26,214,399 attachment bytes: upgrade 358.411 ms,
reversal 329.211 ms, seven openings with median 2.540 ms, conflict fork 3.997 ms.
The payload count remains one and every byte survives. These are fixture timings,
not UI latency or a real-account field verdict.

## Remaining validation

Manual review and the full 13-step gate passed on 2026-09-06; the plan records
the unavailable review command, corrected findings, test figures and one flaky
UI scenario. The first STOP 2 return exposed manual sync omitting remote drafts
and sender-row misalignment; both are corrected locally. The new full gate
passed in 200 seconds (727 Rust and 220 UI tests passed, no flaky result).
The Chief Engineer approved the targeted field replay on 2026-09-06.
Migration/reversal were verified synthetically; no migration was observed in
the real installation. Implementation commit `e95adb4` is published, with
[CI 34038794672](https://github.com/smonchamps/wind/actions/runs/34038794672)
green on Windows and both macOS architectures.
Broader SMTP outcome reporting, account removal coordination and attachment
caching remain in their separately planned lots.

# ADR 0017 — Poll guarded by STATUS (the lean cycle)

**Date**: 2026-08-13 · **Status**: accepted (Chief Engineer's GO on
the measurements from the 3rd field session, PLAN-SYNCHRO §3)

## Context

Measurement on the Chief Engineer's real mailbox (per-phase traces,
2026-08-13): the **recurring** sync cycle cost ~38 minutes on a Gmail
account with ~50 folders — INBOX 34 s, inventory 660 s, folders
1,540 s — because every folder paid `SELECT` + `UID SEARCH ALL` on
every cycle, even at rest, and `SyncEngine::sync` replayed a full
`LIST` per folder. The cycle never rested; the felt latency was the
cycle's own duration. Probable Gmail throttling made it worse (the same
commands twice as slow from one cycle to the next): the volume of
commands was itself the problem.

## Decision

1. **One `STATUS (MESSAGES UIDNEXT UIDVALIDITY)` reading per folder and
   per cycle** — the same one the space guard (ADR 0010 §4) already
   paid for, now enriched — ALSO serves the poll decision: `faut_relever`
   (a pure decision, mail-core, held by a test table) **skips** the
   folder when nothing has moved. INBOX is guarded like the others.
2. The reference point compared is the **UIDNEXT seen at the reading
   preceding the last completed poll** (`mailboxes.remote_uidnext`, NULL
   on a legacy database → conservative first cycle), not `last_uid`: a
   server never lowers its UIDNEXT, while `last_uid` falls back when the
   most recent item is deleted.
3. **Any uncertainty triggers a poll**: a folder never polled, UIDNEXT
   or UIDVALIDITY withheld by the server, an unreadable reference point,
   a refused reading, **local actions awaiting replay** (skipping them
   would abandon them).
4. `folders()` is **hoisted out of `SyncEngine::sync`**: one LIST per
   account and per cycle, at inventory time, with the list already in
   hand.

## Consequences

- Cycle at rest: ~51 STATUS + 1 LIST per account, no more SELECT at
  all — gate measured: **< 60 s on the field's Gmail account** (versus
  38 min), readable in the trace (`n folders (k skipped)`).
- **Flag-only** changes do not wake a folder — they were already NOT
  resynchronized (`changes_since` absent): nothing served before is
  lost. The true reflection of flags is the CONDSTORE job (E2b of
  PLAN-SYNCHRO), where `HIGHESTMODSEQ` will join the STATUS reading.
- A skipped folder does not refresh `remote_total`: full progress
  (ADR 0010 §5) does not move for idle mailboxes — which is correct.

# ADR 0019 — Blocking commands live off the main thread, one at a time

**Date**: 2026-08-15 · **Status**: accepted, delivered (`e32280b`,
PLAN-GELS), held by a gate.

## Context

Field session of 2026-08-15: a multi-second freeze at startup — the
window responds to neither click nor drag. Measurement (`SendMessageTimeout`
probe, real database, 251,062 envelopes): **25.2 s of cumulative
freezes over 40 s**, worst freeze 4.6 s. Root cause: in Tauri 2, a
command declared without `async` runs on the **main thread** — the
Windows message pump. Thirty-four commands were opening the database
from there; it all held as long as they stayed under ~100 ms, until a
130 MB catch-up batch went over.

Two architectural facts constrain the remedy:

- Tauri's async runtime has **no blocking pool**: `async` alone moves
  the blocking onto a tokio worker (workers = cores) — on two cores,
  two slow commands starve the whole IPC queue;
- the main thread was giving away **serialization** of commands for
  free: one at a time, in order. Losing it opens real races
  (local-state/action-queue pairs of `mark_flagged`, `save_draft`/
  `delete_draft` TOCTOU, `SQLITE_BUSY_SNAPSHOT` that `busy_timeout`
  does not cover).

## Decision

1. **Any command that opens the database, touches a file or the vault
   is `async fn`, and its body goes through `off_pump()`**:
   `spawn_blocking` (the pump only pumps) **+ a global command lock**
   (`AppState.commandes` — the prior serialization, kept). The two
   halves are inseparable.
2. **Exemptions are named and justified one by one** (pure state:
   atomics, detached, ADR 0014 self-test panic) in the
   `e2e/garde-thread-principal.mjs` gate — run at pre-push, in CI and
   in `/gate`. Cross-count of attributes/traces: zero trace = red.
3. **The symptom has its budget and its instrument**: no pump freeze
   > 150 ms (HANDOVER §3), measured by `python e2e/sonde-gel.py
   <db.db>` on a database outside the repository.

## Consequences

- Proof at delivery: zero freeze > 150 ms over 40 s (fixtures with
  251k envelopes, with and without a preview stock) and over 60 s on a
  copy of the real database (4.75 GB); background work keeps running
  (15,000 previews recomputed during the measurement).
- Commands stay one at a time: a long write batch still makes a gesture
  wait — as before, but with a free window. Hence the short batches
  (`preview_catchup` at 500, D2).
- The CPU cost of expensive probes remains (D-8): off the pump, it no
  longer freezes — it will reopen on a finding, not on a hunch.
- A panic inside `off_pump` does not condemn the following commands
  (poisoned lock recovered, the same choice as `verrou_compte`).

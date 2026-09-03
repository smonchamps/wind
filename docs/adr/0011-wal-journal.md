# ADR 0011 — SQLite journal in WAL

Date: 2026-07-26 · Status: accepted
· Direct consequence of [ADR 0010](0010-full-synchronization.md)

## Context

First field trial of the full synchronization: a 77 s sync ended with
**"database is locked"** on the header pass of the Microsoft account.

The mechanism is structural, not accidental. In rollback mode, a reader
blocks the writer for the duration of its read. Each command opens its
own connection, and ADR 0010 added a **periodic** reader — the progress
poll, every 800 ms — precisely while writes stretch from a few seconds
to several minutes. The writer's 5 s `busy_timeout` eventually expires:
the error comes from the product itself, not from an exotic usage.

The risk had been named during the review of the progress banner; the
field turned it into a defect the very day of the first full sync.
Decision arbitrated by the Chief Engineer on this finding.

## Decision

`PRAGMA journal_mode = WAL`, set at database opening (`Store::init`).

- **Readers no longer ever block the writer, nor the reverse** — that is
  the property that was missing, and the whole reason for the choice.
- **Persistent**: written into the file header, re-read at every
  opening. Legacy databases in rollback mode are converted on their
  first opening — proven by a test on a file-backed database
  (`une_base_fichier_s_ouvre_en_wal`), not in memory: an in-memory
  database answers "memory" to this PRAGMA and would have validated a
  false model.
- **The `busy_timeout` stays**: two writers still serialize, only it
  makes them wait.
- In-memory tests are unchanged: the PRAGMA answers "memory" there,
  without error.

## Consequences

**Positive** — no more read/write lock at all; the gauge, the list and
the search keep being served during a long synchronization.

**Negative, accepted**

- Two companion files (`-wal`, `-shm`) next to `discovery.db`. No effect
  on cold backups; a hot copy must take all three (which was already
  true of the rollback journal).
- The `-wal` grows during a long burst of writes and shrinks at
  automatic checkpoints. No manual tuning unless a measurement shows the
  need.

## Alternatives ruled out

| Option | Why not |
|---|---|
| Lengthen the `busy_timeout` | Moves the expiry, does not remove the blocking — and freezes the interface for that much longer. |
| Space out the gauge poll | Treats ONE reader; the list and the search block just the same. And progress that refreshes once a minute reassures no one. |
| A single serialized shared connection | Rebuilding a scheduler in front of SQLite, which already has one — maximum complexity for the same result. |

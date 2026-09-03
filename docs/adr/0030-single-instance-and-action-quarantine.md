# ADR 0030 — A single instance, and refused actions quarantined

Date: 2026-09-02 · Status: accepted
· Amends [ADR 0003](0003-smtp-outbox.md) (the transient/permanent
  distinction of the outbox extends to the action log) and
  [ADR 0019](0019-commands-off-the-main-thread.md) (the
  main-thread guard now also checks `async` commands).

## Context

The full audit of 2026-09-01 (`docs/AUDIT-2026-09-01.md`) found two
structural silences: no single-instance guard even though `main.rs`
named the risk (two concurrent pumps quarantining each other's sends),
and an action log where the server's first PERMANENT refusal
(`NO`/`BAD` — folder gone) blocked an entire mailbox's queue, forever,
without a word, because the network port did not distinguish a refusal
from a disconnection.

## Decisions (CE, 2026-09-01 — D1 and D2 of PLAN-AUDIT-V1)

**Single instance via a file lock**, no plugin: `wind.lock` next to
`wind.db`, taken exclusively by the first process (`fs4`, already a
dependency), released by the OS on its death — never a sticky lock.
The second instance says "Wind is already open." and exits (D1:
message then exit, no bring-to-front). The lock is taken BEFORE any
database and any window — but AFTER the Discovery → Wind relocation,
which it would break by creating the target folder; the relocation is
therefore race-tolerant (`rename_tolerant`).

**The action log distinguishes refusal from failure**: `Error::Refus`
(NO/BAD) alongside `Error::Server` (transient by default, retried). A
refusal quarantines the action on the spot and the replay continues;
five transient failures also lead there. A quarantined action is not
eternal: a fresh user gesture on the same message replaces it. The
notice slot counts refused actions (D2: no button — the decision UI
awaits wave 2).

## Consequences

- Two postures stated to the field: double launch ⇒ one message, one
  window; folder deleted server-side ⇒ one line in the notice slot,
  the following gestures go through.
- The `garde-thread-principal.mjs` guard now refuses the database, the
  vault and files in the glue of an `async` command (outside
  `hors_pompe`/`spawn_blocking`) — 17 commands migrated.
- Traps recorded: on Windows, neither a timeout nor a `shutdown` set on
  a socket CLONE acts on the original handle — the IDLE watch is
  bounded by a stream where `set_read_timeout(None)` is worth a floor
  (`FluxBorne`); `REFERENCES` is a reserved SQLite word.

# ADR 0045 — The database boundary: adoption, the blocking token, unlocked work

Date: 2026-09-07. Status: implemented for audit lot 5 (E13a–E13d); field
validation pending. Chief Engineer decisions D1 and D5 of 2026-09-07
(PLAN-AUDIT-2026-09-LOT5).

## Finding

The 2026-09-06 audit named three weaknesses of the shell's relation to the
database (A02, A03, A07; D-9). Every command opened the database at a memoized
path, so only the UI's discipline — `migration_check` before anything else
(ADR 0012) — kept a legacy database from being adopted silently, in a freeze,
by the first command to come along; a file replaced at that path after the
check was reused as if nothing had happened. The rule "no database off the
blocking path" was a text guard with a hand-maintained marker list, blind to
an indirect helper. And one global mutex serialized every blocking command
body: the SQLite read-decide-write pairs it exists for, but also the CPU-bound
sanitize of a body and the reads of attached files. Measured in
`spikes/global-lock` (release build, 200,000-envelope fixture, 30 gestures per
cell): an open gesture waited 465 ms p50 / 692 ms p95 behind a 10 MB sanitize
held under the lock.

## Decision

1. **Adoption is a record, not an ordering.** `apps/desktop/src/adoption.rs`
   remembers the identity of the database file (the file system's own, hashed;
   no handle kept open) when the probe finds nothing pending or when the
   visible pass completes. `adopted_db` hands the path out only while the file
   at the path is that one: before adoption it refuses (`NotAdopted`); a
   replaced file clears the record; the probe runs again on the spot —
   nothing pending, the file is adopted anew and the command goes on; a pass
   pending, the command is refused with the count and the migration screen
   is the way back. A first install is adopted before its file exists and bound to
   the file the first open creates. The three read-only probes keep the raw
   path (`probe_path`).
2. **The path needs a token.** `adopted_db` takes a `Blocking` token that only
   `off_pump` builds, on its `spawn_blocking` thread, for its closure; the
   token derefs to the `AppHandle` so the closures keep their shape. An async
   body cannot name the database by construction; the first build refused 17
   sites, 15 of them indirect helpers reaching the database through a bare
   `&AppHandle`. Threads the shell spawns itself build their token with
   `Blocking::on_dedicated_thread` — the one escape hatch, and the one name
   the text guard still watches.
3. **Heavy work leaves the lock.** `unlocked(...)` runs CPU or file work on a
   bare blocking worker with no token: no database reachable from it. Under
   the lock stay the SQLite reads; the sanitize of a body (reading pane, echo,
   Feed page, reply and forward quotes) and the reads of attached files run
   unlocked; when the result feeds a write, the lock is re-taken to look
   again (the A41 rule) — the core re-checks the attachment budget at every
   insert; a pure read gets no second look (the next gesture verifies).
   The text guard gains the matching rule: a sanitize or a file read inside
   an `off_pump` family call is a red, transitively through helpers.

4. **Pure reads take no lock** (field finding of the same day, STOP 2 of
   E13). A read command runs through `read_off_pump`/`read_store_off_pump`:
   off the pump, its own connection, no commands' lock, `&Store` only. In
   WAL a reader never waits for a writer; what made the reading pane wait a
   synchronization batch out was a write command holding the lock while it
   waited on SQLite's writer. Twenty commands are reads today (the reading
   pane, the list, the thread, the Feed, the resting probe, search, the
   drafts list, …); the text guard refuses a writing name under a read
   helper; `reads-under-write.spec.js` proves the next message opens within
   three seconds while a writer is held and its mark-as-read is queued.

## Consequences and limits

The lock is now what it claims to be: the serialization of writes and of
the read-decide-write pairs; a pure read does not queue behind them. With the sanitize unlocked the same gestures measured 12 ms p50 /
68 ms p95 under the same load. What the lock did not cause, this does not
cure: a 25 MiB attachment's SQLite insert, the backfill's writes and a pin
behind a writer are SQLite-writer-bound (the spike's attach load: 859 → 846 ms
p50 on open) — a different question, left open in the register. The
composer's own boundary (`sanitize_composition`, `sanitize_for_composer`)
stays under the lock on purpose: the user's typed content, small by
construction. The spike ran warm and on a twelve-core machine (the unlocked
sanitize had a free core); it measured at `a58c1cf`, whose lock model is the
one of `2ade544`. The text guard remains a second net for the escape hatch;
the compile-time net has no maintained `compile_fail` test (the app is a
binary crate) — its proof is the 17-error build recorded in the plan.

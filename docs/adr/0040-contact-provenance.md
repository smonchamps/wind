# ADR 0040 — Retain account provenance for learned contacts

Date: 2026-09-07. Status: implemented for Lot 3; final review and field pending.
Scope: [Lot 3 E10](../archives/PLAN-AUDIT-2026-09-LOT3.md).

## Context

The global autocomplete directory did not record which account supplied an
address or name. Removing an account retained exclusive contacts and could not
restore a shared contact's name from the accounts that remained.

## Decision

Record one `contact_origins` row per normalized address and source account.
Keep `correspondants` as the small autocomplete projection: summed frequency,
latest observation and latest nonempty name. Name time is independent of the
latest unnamed observation. SQLite triggers update the projection atomically
with each source write. Account removal deletes its sources in the same
transaction as its local mail; another account's evidence remains.

Create the table, capture the existing directory and install triggers in one
transaction. Historical entries use source 0, meaning unknown ownership;
positive ids refer to accounts. No ownership is inferred from an incomplete
mail cache. The completed table is the migration marker. Failure rolls back
schema and captured data together and permits retry.

## Consequences and validation

New exclusive contacts disappear on account removal. Shared entries fall back
to the remaining sources. Unknown historical contacts and explicit shared
sender rules remain; the account-removal confirmation explains this retention.
This is not an erase-all facility (retention controls belong to Lot 5).

Maintained tests cover exclusive/shared names, sent recipients, backfill,
historical adoption, unnamed recent observations and transactional failures.
Sequential release measurements on synthetic in-memory data, after warmup:
200,000 envelopes to 20,000 contacts take 150.65 ms at a58c1cf and 226.41 ms with
provenance. One-letter autocomplete over 50,000 contacts takes 27.19 / 25.91 ms,
below the existing 50 ms budget. Single samples establish cost, not a latency
distribution or a real-account startup guarantee.

# ADR 0029 — Body import horizon, per account

Date: 2026-08-30 · Status: accepted
· Amends [ADR 0010](0010-full-synchronization.md) (bodies gain a
  chosen horizon; envelopes stay full)

## Context

The CE asks, when adding an account, for a choice of the length of
history imported locally for search (PLAN-HORIZON-NETTOYAGE, wing A).
ADR 0010 had decided "everything, no horizon" — but its plumbing kept
the bound: every backfill pump takes a `since_epoch`, production
passed `NO_HORIZON`, and `backfill.rs`'s comment expected that "a
future user setting would find it as is". This is that setting.

## Decision (CE, 2026-08-30 — D1-D4 of the PLAN)

**Envelopes stay full; only BODIES are bounded to the chosen
horizon** (D1, option A2). A message's weight is its body (~49 of the
~50 kB/message measured at ADR 0010 §4); the envelope (~1.2 kB) stays
cheap and carries the list, grouping, and search by subject/sender.

- **Closed vocabulary**: `1m, 2m, 3m, 6m, 1a, 2a, tout`
  (`HORIZONS_IMPORT`, `backfill.rs`). Translated to epoch by
  `horizon_epoch()` — full days, derived at READ time: the bound
  follows the clock, never a date fixed at add time.
- **Per-account pref** `horizon_import.{id}` (the signature/marker
  pattern, `PREFS_PAR_COMPTE` — purged on removal). Absent or
  corrupted: "everything" — the SAFE default, never a silent
  amputation (D4: accounts predating the setting import everything,
  nothing changes for them).
- **Selector default at add time: "1 year"** (D2).
- **Adjustable afterwards** (D3, Settings > Accounts): extending it
  makes bodies eligible again — the pump catches them up on its next
  pass (it is resumable, the state is the database);
  **shrinking it erases nothing**.
- **Scope of the bound**: the body pump and its counters
  (`bodies_pending_count` / `bodies_total_count` — numerator AND
  denominator talk about the same corpus, else the bar never reaches
  100%) and arrival bodies (uniform, no effect in practice).
  **Out of scope**: thread headers and recipients (from the envelope,
  ~3 kB — ADR 0010 §Context's reason still stands) and loading on
  click — a message out of horizon stays readable on open, its body
  comes from the server on demand (existing path).

## Consequences

- Body full-text search covers the chosen horizon; subject and sender
  search covers everything (the FTS index follows the database, no new
  search logic).
- What ADR 0010 promised as "everything searchable" becomes a USER
  CHOICE of which "everything" remains one value of the vocabulary —
  and the factual default for existing accounts.
- The disk-space guard (0010 §4) now overestimates for a bounded
  account (it counts ~50 kB/message for messages of which only
  ~1.2 kB will be stored beyond the horizon). Accepted: the estimate
  was "deliberately high" on principle.

## Alternatives dismissed

| Option | Why not |
|---|---|
| A1 — bound everything (envelopes included, `UID SEARCH SINCE`) | Old messages would exist nowhere (no list, no reading, no subject search); overturns 0010 head-on; a new bound to write into `SyncEngine` when the body plumbing already exists. |
| Purge bodies when shrinking the horizon | We don't erase what we have — a hole manufactured after the fact is worse than an occupied disk (same spirit as 0010, "quota with purge" refused). |
| Date fixed at account add time | A bound that doesn't follow the clock would drift: "1 year" would become "1 year from 2026". |

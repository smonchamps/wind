# ADR 0007 — Body backfill: recency cap, stored bodies

Date: 2026-07-21 · Status: accepted

## Context

The field validation of the generic IMAP account incidentally revealed a
design flaw far larger than the one it was looking for.

The user searched for a word present in the **body** of a message never
opened. No result. The diagnostic
([`diagnostic_index`](../../crates/mail-core/examples/diag_index.rs))
cleared the index — 100% complete on all three accounts — and pointed to
the real cause:

| Account | Envelopes | Bodies cached |
|---|---|---|
| gmail #1 | 537 | 18 (3%) |
| gmail #2 | 2193 | 1 (0.05%) |
| zoho | 3 | 1 |

The **"envelopes first"** sync ([PLAN.md](../PLAN.md) §3) downloads a
body only on click. The real corpus is therefore **almost bodyless**:
"full-text" search, in practice, only reaches **subjects and senders**.

Two unacceptable consequences as things stand:

1. one of the product's four verbs (*read, sort, **search**, write*,
   §1) is crippled without anything saying so;
2. [ADR 0004](0004-fts5-search-engine.md) settled on FTS5 against
   a corpus **with bodies** — a premise production would never reach.

## Measurements ([`spikes/body-backfill`](../../spikes/body-backfill/README.md))

200 bodies repatriated from a real Gmail account, through the core's
public API, on a **copy** of the database:

| Measurement | Value |
|---|---|
| HTML transferred | 59.2 KB per body |
| Database growth | **61.9 KB per body** |
| Duration | 192 ms per body |

### The structuring fact: the index is nearly free

61.9 KB of disk for 59.2 KB of HTML. The gap — **~2.5 KB per
message** — is the entire cost of the FTS5 index. The "size" vigilance
of ADR 0004 is **lifted by the measurement**: the *contentless* table
keeps its promise, 25× lighter than the content it indexes.

**So it is not the indexes that cost, it is the stored bodies.**

### Scaling to gate 3 (200,000 cumulative messages)

| Policy | Disk |
|---|---|
| Store all bodies | 12.4 GB |
| Index without storing | 500 MB |
| **12-month cap, 3 accounts** | **~370 MB** |

On the real mailbox measured (2,730 messages), storing everything would
cost ~170 MB and ~9 min: perfectly bearable. **It is gate 3 that
breaks, not everyday use.**

## Decision

**Backfill and STORE the bodies of the last 12 months**, in a
background task, after the envelope sync.

This is **not** a retreat from "envelopes first": the list stays usable
immediately, the backfill comes after and blocks nothing. The two
decisions complement each other.

Why this choice over the alternatives:

- **against "index without storing"** (25× less disk): this would make
  offline reading of old messages disappear, while the product promises
  to be *offline-first* (§1) — and would require decoupling indexing
  from the cache, the index today being rebuilt from the `bodies` table
  on every `upsert_envelopes`. A lot of complexity for a problem the
  recency cap already solves;
- **against the status quo**: the measurement made the amputation
  indefensible;
- **unsought benefit**: the backfill also repairs **offline reading**,
  today almost non-existent (18 bodies out of 537).

### Disk budget — the plan had none

Set here, derived from the measurement: **local database < 1 GB in
everyday use (3 accounts)**. At 62 KB per body, this allows ~16,000
stored bodies, consistent with 12 months across three active accounts.

The 12-month horizon is a **setting**, not a dogma: the per-message cost
now being known, any N converts directly to megabytes.

## Field validation (2026-07-21, three real accounts)

The full backfill ran on the real mailbox: resumed after a stop, clean
interruption, and — the test that triggered this whole job — **the word
searched in the body of a message never opened finally comes back**.

| | Predicted by the bench | Measured in production |
|---|---|---|
| Final database (~2,730 messages) | ~170 MB | **97 MB** |
| Cost per stored body | 61.9 KB | **~34 KB** |

**The bench overestimated by 45%, and the gap is explained.** It
sampled the 200 *most recent* messages — that is, the layer most loaded
with heavy HTML newsletters. The full corpus also contains personal
exchanges, much lighter. The bench figure was an **upper bound**, like
the duration figure; both confirmed themselves upper bounds.

Consequences for the decisions made above:

- the **< 1 GB** budget is not tight, it is comfortable: the real
  mailbox uses **10%** of it;
- the gate 3 extrapolation (~370 MB at 12 months on 3 accounts) is also
  an upper bound — at ~34 KB/body it falls back toward ~200 MB;
- the "index without storing" lever moves **further away** by the same
  amount. It stays documented, it is no longer on the table.

Still open: **real batched throughput** against the 192 ms/body upper
bound. Not measured here, the duration not having been recorded. No
bearing on any decision in progress — to instrument the day the
backfill becomes a nuisance.

## Consequences

- **Implementation**: a background pump, resumable after an
  interruption, that never competes with the sync nor with the flushing
  of the outbox (same locks as `outbox_flush` / `drafts_push`).
- **Group the `FETCH`s**: the 192 ms/body measures the current path of
  `load_body`, that is **one IMAP round trip per message**. A real
  backfill must batch (50 per command). The measured figure is an
  **upper bound**, and the gap between the two is to be re-measured once
  the pump is written.
- **Visibility**: progress must be visible and interruptible — an
  invisible background download is a bad network surprise.
  **Precision (2026-07-21)**: this requirement is about VISIBILITY, not
  about triggering. The backfill therefore starts on its own, after the
  first sync — background work the user must request is work that will
  never happen, and the field confirmed it. The banner, the progress and
  the stop button remain in full; a voluntary stop suspends automatic
  resumption until the next session.
- **Known lever if gate 3 tightens**: "index without storing" beyond the
  horizon, costed here at 500 MB for 200,000 messages. An informed
  decision, not a panic rewrite.
- **Re-measure at gate 3**: startup/RAM budgets with a ~1 GB database —
  SQLite reading is insensitive to volume (Phase 1), but that is to be
  verified rather than assumed.

# ADR 0004 — Search: SQLite FTS5 confirmed, Tantivy as a priced plan B

Date: 2026-07-18 · Status: accepted

## Context

Phase 3 opens with full-text search: < 100 ms over 100,000 messages
([PLAN.md](../PLAN.md) §1), budgets held at 200,000 messages at gate 3. The
Phase 0 set-based grid (§2.4) set SQLite + FTS5 as the frozen hypothesis,
Tantivy as the alternative. The spike
[`spikes/search-engine`](../../spikes/search-engine/README.md) settled it on
a deterministic corpus of 100,000 then 200,000 documents, identical
protocol, cross-checked hit counts.

## Decisive measurements (p95, top-50 with ranking, 200,000 docs)

| | FTS5 | Tantivy |
|---|---|---|
| Widest realistic query (16.7% hits) | **37.4 ms ✅** | 0.68 ms |
| Rare / AND / phrase queries | 0.3–4.7 ms ✅ | < 1 ms |
| Degenerate query (90% of corpus matches) | 188 ms ❌ | 0.15 ms |
| Incremental 500 docs (sync's recurring path) | **25 ms** | 350 ms |
| `reunion` finds `réunion` (French) | **yes, native** | no (needs configuring) |
| Second store to reconcile | **no** | yes |

## Decision

**FTS5 is confirmed** for v1 search. The set-based rule was: Tantivy had
to beat the hypothesis *clearly* — it is faster on every query, but it
loses on what structures an offline-first client:

1. **Transactionality**: the index lives INSIDE the database — deleting the
   message and its index entry in the same transaction. Tantivy is a
   second store: tombstones, guards, post-crash reconciliation — Phase 2
   just paid this price for drafts, it should not be paid a second time
   for a rebuildable index.
2. **The recurring path**: every sync inserts messages. FTS5 absorbs 500
   documents in 25-36 ms; each Tantivy commit costs ~350 — and a deferred
   commit policy would be needed.
3. **French is native** (`unicode61 remove_diacritics 2`); the default
   Tantivy tokenizer returns **zero** results for "reunion" → "réunion".
4. **Zero new dependency**: FTS5 is already in the bundled SQLite.

By the plan's criterion, FTS5 holds gate 3 with a ×2.7 margin on the most
unfavorable realistic query (37 ms for a 100 ms budget).

## Watch points and guardrails (measured, not assumed)

- **FTS5's cost follows the number of matches** (`ORDER BY rank` = BM25
  over all of them): a query matching 69-90% of the corpus exceeds the
  budget at 200,000 messages. This case is an artifact of the spike's
  synthetic vocabulary (~150 words), but the mechanism is real. Product
  guardrails: search-as-you-type triggered from 3 characters + debounce;
  FTS5's `prefix=` option to evaluate at implementation.
- ***External content* table** (`content=`) in production: the spike
  stored the content in the FTS table (595 MB); the index alone should be
  much smaller — to be measured at implementation.
- **Priced, documented plan B**: if the wide-query wall materializes on a
  real corpus (measured in beta, Phase 5), Tantivy is sub-millisecond
  everywhere (block-max WAND pruning), an 8× smaller index — the switch
  would be an informed decision, not a panic rewrite. Its costs are known:
  second store, slow commits, diacritics folding to configure.

## Consequences

- Production search is implemented in `mail-core` on FTS5, after the
  multi-account foundation (the index will carry `account_id` from the
  start).
- The spike stays in `spikes/search-engine` (outside the workspace —
  Tantivy must not enter the production lock), re-runnable to re-measure
  at other volumes.

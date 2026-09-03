# ADR 0012 — Visible and interruptible migration: adoption rewinds

Date: 2026-07-26 · Status: accepted (validated in the field the same day)

## Context

Adopting a legacy database — attaching each message to its thread —
costs **3.69 s at 200,000 messages** (gate 3), paid silently by the
first command that opens the database, in a frozen interface. Already
brought down from 11.1 s by two measured fixes; the rest is the
intrinsic cost of the union-find. [ADR 0010](0010-full-synchronization.md)
made it a **prerequisite of Phase 5**: the real mailbox (256,312
messages) exceeds gate 3's scale, and every future change to the
threading rule will replay this pass.

The precedent is [ADR 0007](0007-body-backfill.md): a long task
must be **visible and interruptible**. But the asymmetry is total, and
it is what decides the shape:

| | Body backfill (0007) | Thread adoption (here) |
|---|---|---|
| Does the list depend on it? | no — it lives without bodies | **yes — it starts from `threads`** |
| Splittable? | yes, in batches of 200 | **no**: a partially persisted adoption is a half-empty mailbox |
| "Interrupt" | stop recalling the command | **undo everything**, and replay it whole later |

## Decision

**Adoption becomes a single transactional unit, reported and
cancellable.**

1. **A single transaction** in `Store::init`, from the conditional
   `DROP` of the thread tables to `PRAGMA user_version`: deletion,
   schema recreation, detaching the envelopes, adoption, version. Before,
   `user_version` advanced in its own transaction *before* adoption —
   this is precisely what forbade the rewind. Cancel = `ROLLBACK`: the
   database returns to its state before opening, `user_version`
   unchanged, and the whole pass replays at the next launch. The `BEGIN`
   is *deferred*: on an up-to-date database nothing writes, the
   transaction stays a reader and never meets the writer of a long sync
   (lesson of [ADR 0011](0011-wal-journal.md)).

2. **`Store::open_with_progress(path, on_progress)`**: the callback
   receives `(done, total)` every 1,000 steps and answers `ControlFlow`
   — `Break` = cancel. The total is an **upper bound declared upfront**
   (attach each orphan + consolidate at most as many threads): it never
   moves along the way, a bar going backward being worse than an
   imprecise bar. `(total, total)` is announced only after `COMMIT` —
   never "100%" before it is true. Display reuses `sync_percent`, which
   already carries the degenerate cases.

3. **`Store::pending_adoption(path)`**: a **read-only** probe (no file
   created, no migration triggered) so the desktop knows whether to show
   the screen BEFORE the first real opening.

4. **On the desktop side**, four commands (`migration_check`,
   `migration_run`, `migration_progress`, `migration_cancel`) and a
   modal screen that blocks the whole startup until the database is
   adopted. Necessary because **every command opens its own
   connection**: without a gate, the first one in would pay for the
   pass. After "Cancel", the screen offers "Resume" — showing the
   mailbox without the pass is impossible, by construction.

## Proof

- **Rewind** (`annuler_l_adoption_defait_tout_et_laisse_user_version_inchangee`):
  cancellation at the 1,000th message of a real file-backed database →
  `user_version` intact, v1 tables intact (the `DROP` is undone too),
  zero message lost; reopening replays the whole pass and the list is
  complete.
- **Observability** (`l_adoption_annonce_son_avancement_du_depart_a_la_fin`):
  total announced upfront, progress never decreasing, "done" said
  exactly once, at the end.
- **Silence of the common case** (`une_base_a_jour_s_ouvre_sans_annoncer_de_migration`,
  `la_sonde_dit_quand_une_adoption_attend_sans_la_declencher`): no false
  banner, no trace left by the probe.
- **Bench** (`banc_migration_fils`, gate3.db, 200,000 messages):

  | | before (reference) | after |
  |---|---|---|
  | adoption (legacy database) | 3.69 s | **3.66 s** |
  | up to date (common case) | ~2.5 ms | **2.5 ms** |
  | threading | 160,000 threads, 0 orphan | identical |

  The cost of the reporting steps is invisible; the single transaction
  does not change the price of the pass.

## Field validation (2026-07-26, on copies — never the real database)

- **Copy of the real database** (256,312 messages) rewound to
  `user_version = 0`: screen shown for **under a second** — the scope to
  adopt (~7,500 messages, INBOX + Sent) is small against the database;
  complete list on arrival. At this scale, the migration is nearly
  imperceptible: it is gate 3's decor that justifies the screen.
- **Copy of gate3.db** (200,000 messages, all in scope): screen with a
  bar rising ~4 s; **"Cancel" mid-pass** → cancellation message, no
  list; application closed then relaunched → **the screen shows again
  from the start**, proof of the rewind at scale; pass left to finish →
  complete list.

**Collateral finding of this validation** (pre-existing defect, fixed
the same day): `#detail { display: flex }` overrode `[hidden]` (ID
specificity against the browser stylesheet) — the reading pane stayed
permanently rendered and its sandboxed iframe captured the first click:
keyboard focus went into the iframe and shortcuts died until a click
elsewhere. Three safeguards added (`#detail`, `#detail-note`,
`#compose-from-row`), held by an E2E test.

## Consequences and accepted limits

- **A second instance** of the application launched DURING a migration
  would have its commands fail after the `busy_timeout` (5 s).
  Pre-existing risk, unchanged — the unit does not worsen it, it bounds
  it in time.
- Cancellation has a **one-step latency** (1,000 steps, ~20 ms at the
  bench's pace): imperceptible, and the button disarms on first click.
- The displayed total is an upper bound: the bar jumps forward at the
  end (consolidation shorter than the estimate), never backward.
- The pass stays **blocking for use** for a few seconds at real scale —
  that is the choice: splitting it is impossible without lying to the
  list (§ Context).

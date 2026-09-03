# ADR 0010 — Full synchronization of the mailbox

Date: 2026-07-25 · Status: accepted
· Revises [ADR 0007](0007-body-backfill.md) (horizon)
· Refines [ADR 0009](0009-thread-scope-per-account.md) (threading scope)

## Context

Three field measurements, taken the same day, say the same thing.

**1. The header pass has converged, and its result is capped.**
`diagnostic_fils` on the real mailbox: `read, without References`
(1,399) + `read, with References` (257) = **1,656**, exactly the number
of messages `within the horizon (12 months)`. The pass read 1,656
messages out of 1,656 eligible. It has nothing left to read.

**2. The 5,883 messages outside the horizon — 78% of the database —
will never be read.** They stay threaded by `In-Reply-To` alone, of
which measurement 2 of [ADR 0008](0008-conversation-threading.md)
establishes that it "threads almost nothing" in an inbox.

**3. The header pass's horizon was an inheritance, not a decision.**
[`commands.rs`](../../apps/desktop/src/commands.rs) passes it
`backfill_horizon()` — the bound from ADR 0007, which exists to hold
the disk budget (< 1 GB, [PLAN.md](../PLAN.md) §1). But a header block
weighs ~3 KB against ~50 KB for a whole message, and it does not sit on
disk like a body: it fits in one column. The bound was reused because
the function had the same *shape*, not the same *reason*.

### The costing, against the options already ruled out

| Option | Network | Verdict |
|---|---|---|
| `References` in the sync | 150 MB / 50,000 msg | ruled out (ADR 0008) |
| Headers carried by the body backfill | 137 MB measured | ruled out (ADR 0008) |
| **Extend the headers-only horizon** | ~17.6 MB | never examined |

Two orders of magnitude below the two rejected options.

## Decision

The Chief Engineer decides more broadly than the finding required: **on
adding an account, Discovery downloads the whole mailbox**, every
folder, with no horizon and no volume quota.

### 1. Every mailbox, no exception

INBOX, Sent, Archive, Trash, Spam, and every user folder. **No
exception**, including Spam and Trash: the software does not judge what
deserves to be kept. Searching for a message thrown away by mistake is
a real need; an excluded folder is a hole the user cannot fill.

### 2. The "database < 1 GB" gate is LIFTED — explicitly

This is a Chief Engineer decision, not a drift: it is written here so
no future review mistakes it for an oversight. The budget of
[PLAN.md](../PLAN.md) §1 becomes moot, replaced by a **disk-space
guard** (§4).

A lifted gate is not a forgotten gate. The others — startup, opening,
list page, RAM, zero loss — **remain blocking**, and this decision puts
them under strain: they will be re-measured (§6).

### 3. Store and index ≠ thread

**This is the core of this ADR**, and it protects a frozen decision.

- **Storage and search**: every message, from every mailbox. Everything
  becomes readable and searchable.
- **Threading**: the scope stays **INBOX + Sent**
  ([ADR 0009](0009-thread-scope-per-account.md)). A message outside
  this scope keeps `thread_id = NULL`.

Without this separation, ADR 0009 falls through a path no current test
watches. `thread::adopt` operates **per account**: as soon as Archive
and Spam enter the account, their messages join the threads, and three
aggregates silently corrupt —

| Aggregate | What breaks |
|---|---|
| `size` | "12 messages" on a thread showing 3 |
| `unseen` | a thread forever unread because of a spam message never opened |
| `last_epoch`, `last_mailbox_id` | **a conversation jumps to the top of the list because a spam message latched onto it** |

The third is a **correctness** flaw, not an ergonomics one: the list
would lie about the order of exchanges, and the user would have no way
to undo it. This is the same reason for refusal as threading by subject
(ADR 0008 §2).

The alternative ruled out by ADR 0009 — "scope = every mailbox,
including Archive and Trash" — therefore stays ruled out, **for its
original reason**: it would resurrect conversations the user filed away
or discarded. Downloading the archive does not reopen it.

### 3 bis. `inbox_size` survives, and the partial index with it

[`thread.rs`](../../crates/mail-core/src/thread.rs) warned it would need
to "rethink rather than parameterize" `RECEIVED_MAILBOX` the day the
list would show several mailboxes. **That day has not come**: the list
still shows threads with at least one message in INBOX.

`inbox_size > 0` therefore keeps its meaning, the partial index of ADR
0009 §4 stays valid, and the execution-plan test
(`la_boite_unifiee_ne_materialise_pas_son_tri`) keeps guarding it. Gate
3 is not threatened by this door.

### 4. Space check BEFORE, clear message if insufficient

No quota, but no full disk either. Before starting an account's sync,
the volume is estimated and compared to free space. If it is short,
**it refuses and says so** — we do not start only to stop halfway.

The estimate rests on two project measurements, not on an invented
figure:

- **~49 KB per message**, attachments included — the full backfill of
  the real mailbox was measured at 137 MB for 2,801 messages;
- **~1.2 KB of envelope and index** — derived from `gate3-corps.db`
  (778.9 MB for 200,000 envelopes + 16,002 bodies).

That is **~50 KB per message**, multiplied by the sum of the `EXISTS`
the server announces per folder (free, already in the protocol). Orders
of magnitude: 50,000 messages → 2.5 GB; 100,000 → 5 GB.

The estimate is **deliberately high**: announcing too much and holding
to it is better than starting and failing halfway.

### 5. In the background, with percentage progress

Same shape as the body backfill (ADR 0007): bounded per cycle,
resumable — the state is the database — and batched. It never blocks
reading: the list is usable from the first envelopes.

Progress is shown as a **percentage**: denominator = sum of the
`EXISTS`, numerator = messages in the database. A long task that does
not say where it stands is indistinguishable from a stuck task — this
is the "never swallow an error" lesson applied to duration.

### 6. What must be re-measured, and what is going to hurt

**Search is already over budget** (118–208 ms, target < 100 ms) and is
paid **by the number of matches** (~2.9 µs per unit, ceiling around
35,000 — ADR 0009). Multiplying the corpus by the entire archive
mechanically brings this ceiling closer.

Both levers are costed and available: **sort by date** (×2, four
queries out of six fall back under budget) and FTS5's **`prefix=`**
option (−73 ms of expansion). The Chief Engineer had chosen to decide
the first in beta, on real mailboxes; **this decision puts it back on
the critical path**.

To be re-measured on `gate3-corps.db`: search, cold start, adoption of
a legacy database.

## Consequences

**Positive**

- The 5,883 messages outside the horizon become searchable — "full
  text" search finally reaches the whole correspondence.
- Threading is uncapped: the `References` of the whole mailbox enter the
  registry, and the convergence of ADR 0008 §5 does the rest without any
  acquired information being lost.
- An archived, deleted or spam-filtered message can be found. A real
  need no fallback covered.

**Negative, accepted**

- **The disk is no longer bounded.** Explicit decision (§2).
- **Search moves away from its budget** before its levers are applied
  (§6).
- **Adopting a legacy database grows with it**: 3.7 s at 200,000
  messages, on a path already over budget. The "visible and
  interruptible migration" job earmarked for Phase 5 becomes a
  **prerequisite**, not merely an improvement.
- **Spam enters the search results.** Direct and intended consequence of
  §1.
- The first sync of an existing account is long and heavy. It announces
  itself (§5) rather than being endured.

## Alternatives ruled out

| Option | Why not |
|---|---|
| Extend only the headers horizon (~17.6 MB) | Would have been enough to uncap **threading**, but left 78% of the mailbox outside **search**. Ruled out by the Chief Engineer in favor of the full scope. |
| Exclude Spam | The largest folder of an old mailbox, and never searched — but that is the software judging what deserves to be kept. Refused. |
| Exclude Trash | Searching for a message deleted by mistake is precisely the case where the user needs the software. |
| Volume quota with pruning of the oldest | Makes a search's result depend on the date it is run. A silent hole is worse than an announced full disk. |
| Extend thread scope to every mailbox | Would resurrect filed-away or discarded conversations, and corrupt three aggregates (§3). Already ruled out by ADR 0009. |

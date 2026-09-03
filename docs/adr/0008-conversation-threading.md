# ADR 0008 — Conversation threading: headers only, two-speed acquisition

Date: 2026-07-24 · Status: accepted

## Context

Threading messages into conversations is the last major functional item
of Phase 3. It touches three things at once: the model (which headers
attach a message to a thread), storage (how to paginate a threaded list
without aggregating the whole mailbox), and **rendering the list**,
which is the product's hot path — 60 fps, virtualization
([PLAN.md](../PLAN.md) §1).

Two measurements were taken before a line was written, and the second
overturned the initial plan.

### Measurement 1 — `In-Reply-To` is already in the bytes we download

The IMAP ENVELOPE (RFC 3501 §7.4.2) carries `In-Reply-To`. The sync was
already receiving it and discarding it. The first level of threading
therefore costs **zero network bytes**.

### Measurement 2 — `In-Reply-To` alone threads almost nothing in an inbox

The normal case of an exchange, seen from INBOX:

1. I receive **A**;
2. I reply **B** — which goes into "Sent", **not** into INBOX;
3. I receive **C**, which carries `In-Reply-To: B`.

INBOX contains A and C. C points to an absent message. Without
`References` — which also carries the **root** A — the two remain two
separate threads.

**The "missing link" is therefore not an edge case: it is the majority
case of a real correspondence.** `References` is mandatory, not a
refinement.

### Measurement 3 — `References` cannot travel with the ENVELOPE

`References` is not in the ENVELOPE. The `imap` crate only exposes
`Fetch::header()` for `BODY[HEADER]` — the **entire** block, ~3 KB —
not for `HEADER.FIELDS (REFERENCES)`, which would weigh ~150 B. Putting
the entire block into the sync would multiply the cost of "envelopes
first" tenfold: ~150 MB for the 50,000 messages of Phase 1.

## Decision

### 1. Threading by union-find on RFC 5322 identifiers — and nothing else

Every `Message-ID` encountered — that of the message **and those of its
ancestors, even absent from the mailbox** — is entered into a registry
that points to a thread. A message citing two identifiers attached to
two different threads **merges** them.

The algorithm is **pure and I/O-free**
([`thread.rs`](../../crates/mail-core/src/thread.rs)), tested against
field cases: out-of-order arrival, absent ancestor, a message linking
two threads, self-reference, `References` with several thousand
entries, malformed `Message-ID: <>`.

### 1 bis. An identifier must contain an at sign — revision of 2026-07-24

*Added after the field validation of the first version.*

The user reported a conversation gathering 17 unrelated messages. The
diagnostic
([`diagnostic_fils`](../../crates/mail-core/examples/diag_threads.rs))
ruled out the three expected causes — no reused `Message-ID`, no
campaign anchor — and pointed to the real one:

| thread | messages | most-cited anchor |
|---|---|---|
| #1991 | 43 | cited by **43/43**, no angle brackets, **no at sign**, 11 characters |
| #484 | 17 | cited by **17/17**, no angle brackets, **no at sign**, 11 characters |

These 3-to-11-character "identifiers" that nobody carried were
**words**. The first version accepted, as a fallback, a header without
angle brackets and split it on spaces — a compromise made "for real
life", without measuring what it let through. It then only takes a
header written in prose (`In-Reply-To: Your message of January 3`, an
RFC 822 form that some autoresponders still produce) to manufacture as
many false anchors as there are words. Every message carrying the same
phrase latches onto it, and the union-find merges them — correctly, on
false data.

**Decision: a token is an identifier only if it contains an at sign and
no space** (RFC 5322 §3.6.4: `msg-id = "<" id-left "@" id-right ">"`).
The rule also applies between angle brackets: `<1234567890>` is
rejected.

Accepted consequence: a message with a non-conforming `Message-ID`
forms its own thread and the replies it receives do not attach to it.
This is a **local and silent** loss, against a **massive and visible**
merge — the trade-off is decisively favorable.

Databases already threaded under the old rule carry false threads that
no code fix alone repairs. A version marker (`PRAGMA user_version`)
makes them **redo at opening**: purely local, the raw headers being
intact in the database — only their interpretation was at fault.

**Residual risk, named.** Transitive merging still has no safeguard: a
single wrong anchor does not slightly damage the threading, it collapses
it step by step. The filter above removes the observed cause, not the
class. A legitimate ancestor cited by dozens of messages (a mailing
list's original announcement) would produce the same effect — and would,
itself, be RFC-compliant. No arbitrary cap is set: it would break
genuine long conversations. The chosen safeguard is **measurability** —
`diagnostic_fils` names the anchor in one command — rather than a
heuristic that would fail silently.

### 2. Explicit refusal: never thread by subject

The JWZ algorithm proposes, as a fallback, threading messages with the
same subject once "Re:" is stripped. **We refuse it.**

Such a fallback merges messages with no real link as soon as the
subject is common — "Hello", "Invoice", "Question". In a mail client,
this is a **correctness** fault, not an ergonomics one: the user sees a
conversation that never existed, and nothing in the interface lets them
undo it. A thread split in two is repairable and honest; two strangers'
messages merged are not.

Accepted consequence: correspondents whose software emits neither
`In-Reply-To` nor `References` will not be threaded. That is the right
side of the trade-off.

### 3. A thread only groups what the mailbox contains

Threads are **relative to a mailbox** (`mailbox_id`). The counter shown
is therefore that of **received** messages: our own replies live in
"Sent", which v1 does not sync. This is consistent with what the list
shows — it does not display our sends either.

### 4. Materialized aggregate, never incremented

A `threads` table carries, per thread, its last message, its date, its
size and its unread count. The list starts from **this** table:

```sql
FROM threads t JOIN envelopes e ON e.uid = t.last_uid ...
ORDER BY t.last_epoch DESC LIMIT ? OFFSET ?
```

A `GROUP BY thread_id` with `MAX(date)` would force SQLite to scan then
sort the 200,000 envelopes **on every scroll page**. Here the index
carries the sort and the pagination: the cost of a page no longer
depends on the mailbox's size.

The aggregate **recomputes**, it does not increment. A counter
maintained by additions drifts at the first forgotten path (merge,
UIDVALIDITY, replayed action), and drift shows on screen forever: "4
messages" on a thread showing 3. The recompute is bounded by the
thread's size.

Like the search index ([ADR 0004](0004-fts5-search-engine.md)),
the aggregate is maintained **within the transaction** that writes the
message.

### 5. Two-speed acquisition, and convergence

| Header | Source | Cost | When |
|---|---|---|---|
| `In-Reply-To` | ENVELOPE | 0 B | at sync |
| `References` | `BODY.PEEK[HEADER]` | ~3 KB | bounded background pass |

The header pass reuses the connection **already open** by the sync: it
costs no extra round trip. It is bounded (2,000 messages per account
per sync), resumable (its state is the database) and batched, like the
body backfill ([ADR 0007](0007-body-backfill.md)).

Delivering acquisition in two stages is only possible thanks to a
property of the algorithm: merging makes it **convergent**. A thread
born in two pieces reattaches as soon as the missing link appears,
without any acquired information being lost. Conversations therefore
thread progressively, never backward.

`refs = NULL` means "never read", `refs = ''` means "read, and there are
none". Confusing the two would make the same messages be requested
forever.

### 6. An emptied thread disappears with its registry

Archiving every message of a conversation removes the thread and its
links. A later reply will open a fresh thread — which is honest, since
the mailbox no longer contains anything of that exchange. Keeping empty
threads would force the list to filter them, at the cost of the index
that makes it fast.

## Consequences

**Positive**

- One line per conversation, with its counter; a thread stays unread as
  long as it has an unread message left, even if the last one is read.
- The cost of a list page stays independent of the mailbox's size.
- No network round trip added; opening a thread is **purely local**,
  like choosing a destination folder.
- Existing databases are **adopted** at opening: without this pass,
  every legacy message would have kept `thread_id` NULL and the list —
  which starts from `threads` — would have been empty. This is the trap
  attachments fell into, this time handled from the start and proven by
  test.

**Negative, accepted**

- Correspondents without thread headers are not threaded (§2).
- The counter ignores our own replies (§3).
- Messages outside the recency horizon (12 months, ADR 0007) do not have
  their `References` repatriated: they stay threaded by `In-Reply-To`
  alone.
- A message that is not at the head of its thread **no longer has its
  own line** in the list. That is the very point of threading, but it
  changes navigation: it is reached through the conversation bar.

## Alternatives ruled out

| Option | Why not |
|---|---|
| `X-GM-THRID` (Gmail native thread) | Specific to Gmail. The product now serves Microsoft 365 and generic IMAP; a per-provider path is exactly what the `MailServer` trait exists to avoid. |
| Fall back to subject | Merges unrelated messages, with no recourse for the user (§2). |
| `References` in the sync | ~150 MB on 50,000 messages: would destroy "envelopes first" (measurement 3). |
| Carry the headers via the body backfill | Free in bytes — bodies contain the headers — but would require **re-downloading the whole mailbox** (137 MB measured for the user against 8 MB for headers alone), and would not cover messages outside the body horizon. |
| `GROUP BY` on the fly | Scan + sort of the whole mailbox on every page (§4). |

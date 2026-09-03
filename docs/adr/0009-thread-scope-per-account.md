# ADR 0009 — A thread's scope is the ACCOUNT, not the mailbox

Date: 2026-07-25 · Status: accepted
· Revises [ADR 0008](0008-conversation-threading.md) §3 and §4

## Context

[ADR 0008](0008-conversation-threading.md) §3 froze a rule:
*"threads are **relative to a mailbox** (`mailbox_id`) […] our own
replies live in "Sent", which v1 does not sync"*.

The rule was **conditioned on this premise**, and the premise has just
fallen: the Chief Engineer decided to sync "Sent". This decision had
itself been deferred until after gate 3, to know the cost at scale
before opening a second front ([PHASE3.md](../archives/PHASE3.md) §5) —
it is now known.

This is therefore not reopening a frozen decision against a
measurement: it is removing its foundation. The rule of §5 of the
handover is respected.

### The finding that makes the job necessary

Syncing alone would bring **nothing**. The sync engine is already
parameterized by mailbox name (`commands.rs` simply sets
`MAILBOX = "INBOX"`): adding "Sent" is plumbing. But threading, itself,
is walled off:

```sql
threads.mailbox_id  NOT NULL
thread_links        PRIMARY KEY (mailbox_id, message_id)
```

A reply filed in "Sent" would form **its own thread, in its own
identifier space**, never joining INBOX's. We would pay for the sync,
the disk and the search index for zero additional threading.

It is this walling-off, and only this, that kept threading from paying
off: 40 messages threaded into 15 conversations out of 2,813 real
messages (ADR 0008, field finding).

## Decision

### 1. A thread belongs to an ACCOUNT

`threads.account_id` replaces `mailbox_id`; `thread_links` is re-keyed
on `(account_id, message_id)`. A received message and the reply made to
it belong to the same thread, since they belong to the same exchange.

The scope stops at the account: two accounts never merge their threads,
even if the same message appears in both. The invariant *"identity =
`(account_id, uid)`"* (handover §6.2) requires it, and a thread crossing
accounts would make the unified mailbox impossible to explain.

### 2. What the list shows

A thread has a line in the list **as soon as it contains at least one
received message**. It is represented by its **most recent** message,
**wherever it comes from** — including our own replies.

Replying therefore brings the conversation back up, and the displayed
excerpt becomes our reply. This is what Gmail does, and it is
consistent with the question the list answers: *"where does this
exchange stand?"*, not *"when was I last written to?"*.

**A purely outgoing thread has no line**: writing to someone who never
replies does not create a conversation in the inbox. This is the exact
counterpart of the rule "a thread only shows what the mailbox
contains" — the inbox stays what was received.

### 3. The counter covers the whole exchange

"3" on a line means three messages in the conversation, received and
sent combined.

This is not a refinement, it is a **mandatory coherence**: the
conversation bar shows the full exchange. A counter that only announced
received messages would contradict on screen what opening the thread
displays — exactly the "two contradicting numbers" flaw that ADR 0008
§4 sought to avoid by recomputing the aggregate rather than
incrementing it.

### 4. A PARTIAL index, without which gate 3 is lost

Gate 3 just fixed a materialized sort that cost up to 987 ms per page
([PHASE3.md](../archives/PHASE3.md) §2). The rule in §2 above would
bring it back through another door: filtering "threads with at least
one received message" **while sorting by date** forces SQLite to scan
then discard every purely outgoing thread.

The `threads` aggregate therefore carries a received-message counter,
and the index serving the list is **partial**:

```sql
CREATE INDEX idx_threads_date_globale
    ON threads(last_epoch DESC, last_uid DESC, account_id)
    WHERE inbox_size > 0;
```

The filter thus enters the index instead of being evaluated after it.
The promise of ADR 0008 §4 — *the cost of a page does not depend on the
mailbox's size* — is upheld **by construction**, and the execution-plan
test (`la_boite_unifiee_ne_materialise_pas_son_tri`) keeps it so.

### 5. The aggregate must name its mailbox

`threads.last_uid` is no longer enough: a thread's last message can be
in INBOX or in "Sent", and *"a UID alone identifies nothing"* (handover
§6.2). The aggregate therefore carries `last_mailbox_id` alongside
`last_uid`.

### 6. The migration goes through the version marker

The tables change key: SQLite forces a rebuild. The mechanism already
exists — `PRAGMA user_version` lower than `THREADING_VERSION` triggers
the erasure of the threads and their full recompute at opening (ADR
0008 §1 bis). It is therefore enough to **drop the two tables** on this
path and bump the version.

Measured cost of the rebuild: **4.22 s for 200,000 messages**, once. It
is already over budget and already logged as a carry-over
([PHASE3.md](../archives/PHASE3.md) §4); this job does not worsen it by
an order of magnitude, but it **makes handling it more urgent** — the
migration will this time touch every existing database, not only the
legacy ones.

### 7. Discovery of the "Sent" folder

`\Sent` attribute announced by the server, then a **fallback by name**
(`sent`, `envoyés`, `éléments envoyés`) — same priority order and same
deliberate exception to the "never hard-code a name" rule as archiving
([ADR 0006](0006-microsoft-imap-oauth2.md)), and for the same reason: a
real server does not always announce what it holds. Modified UTF-7
decoding (`mutf7`) serves here too.

If no folder is found, the account works as before: threads only group
received messages. A local and silent degradation, never an error.

## Consequences

**Positive**

- Threading finally pays off for what it was written for: a complete
  exchange in one line, in the order it unfolded.
- Reading a thread requires no extra network round trip.
- "Sent" becomes synced, which settles a Phase 3 carry-over.

**Negative, accepted**

- **The search corpus grows**, and search is paid for by the number of
  matches (~2.9 µs per unit, ceiling around 35,000). Adding "Sent" thus
  brings this ceiling closer. To be re-measured.
- **The disk grows**: envelopes, FTS index, and backfilled bodies from
  the "Sent" folder.
- **All existing databases rebuild their threads** on first launch —
  4.22 s at 200,000 messages, on a path already flagged as over budget.
- The list **changes order** before the user's eyes on first launch:
  conversations move up because they had been replied to. Expected, but
  not to be discovered without warning.

## Alternatives ruled out

| Option | Why not |
|---|---|
| Sync "Sent" without changing the scope | Cost paid, zero gain: threads would stay walled off by mailbox. |
| Scope = every mailbox of an account, including Archive and Trash | Would resurrect conversations the user filed away or discarded. INBOX + Sent is the scope of the **live** exchange; widening it will be decided on an observed need. |
| Scope = the account, but the list sorted on the last RECEIVED message | A conversation just replied to would stay frozen at its earlier date — the opposite of "where does this exchange stand". Ruled out by the Chief Engineer. |
| Filter outgoing threads after the index (without a partial index) | Reintroduces the scan gate 3 just removed (§4). |
| Counter limited to received messages | Would contradict on screen the conversation bar, which shows the whole exchange (§3). |

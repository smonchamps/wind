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


## Amendment — audit Lot 2, 2026-09-06

The Chief Engineer approved [Lot 2](../PLAN-AUDIT-2026-09-LOT2.md), SMTP option A
and fast IDLE stop (D5). Lock failure now prevents any Store/window startup.

Account jobs capture an incarnation ticket and acquire a lease immediately before
work. Removal closes admission, stops the watcher and drains active work outside
the command mutex before vault/Store deletion. A timeout keeps the account; a
fresh incarnation reopens admission without reviving old jobs. Old and new
incarnations share outstanding-work counters after an aborted removal, so a retry
cannot bypass an unfinished SMTP handoff. Unknown-identity authentication flows
also drain before vault removal. Started SMTP persists its result before release;
Sending or Interrupted blocks purge. Local saves/enqueues reject closing accounts,
and late refresh/publication cannot act on a replacement account.

The watcher stop token is checked below TLS through 100 ms local TCP read slices.
The 180-second heartbeat and ordinary poll read timeouts remain unchanged. Cancelled
streams are terminal. This implements measured D5; it does not promise immediate
cancellation of DNS, connect, writes or a started SMTP submission.

IMAP removal now crosses a portable plan/step boundary. The adapter resolves safe
capabilities and a fixed destination before mutation: MOVE when advertised;
otherwise COPY plus targeted UID EXPUNGE only with UIDPLUS. Pure purge also needs
UIDPLUS. No global EXPUNGE fallback. Generic All is not evidence of Gmail archive
semantics; label-removal archive requires INBOX, X-GM-EXT-1 and UIDPLUS.

A separate action_effects journal freezes source/destination generations. In-flight
is committed before COPY/MOVE; confirmed COPY advances before deleting the source.
Only source deletion can repeat. A missing or inconsistent confirmation is retained
as uncertain across restart, disappearing source rows and generation changes.
The action link uses ON DELETE SET NULL and a separate non-reused effect ID, so an
orphan cannot attach to a new action with a recycled rowid. Startup recovery runs
once before new work through the Store command boundary. A pre-mutation refusal or an unsuccessful COPY remains a refusal; after MOVE
emission, NO is uncertain because [RFC 6851 section 3.3](https://www.rfc-editor.org/rfc/rfc6851.html#section-3.3)
permits partial effects. [RFC 3501 section 6.4.7](https://www.rfc-editor.org/rfc/rfc3501.html#section-6.4.7)
requires destination restoration for unsuccessful COPY. Repeatable flags retain
the five-failure policy. COPYUID is corroboration,
validated for both tagged/untagged replies through the pinned parser; absent mapping
is allowed, mismatched identities are uncertain, Message-ID alone proves nothing.

The notice now consumes the actual refused_actions schema and includes the refusal
reason. An uncertain move directs inspection of source/destination before a new
gesture. I checked acknowledges the incident; it never repeats the old operation.


Removal actions capture subject and sender when enqueued, before local envelope
removal. The retained incident exposes that snapshot with its account address so
acknowledgement follows an identifiable check, including after a generation reset.
Outbox decisions additionally carry Message-ID; an old notice cannot operate on a
recycled rowid. Sending refuses discard, and queued snapshots must atomically claim
the same id, Message-ID, account and queued state before entering the transport.

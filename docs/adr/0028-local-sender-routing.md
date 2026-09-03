# ADR 0028 — Organized mode's routing is LOCAL, by sender

Date: 2026-08-29 · Status: accepted

## Context

Organized mode (PLAN-MODE-ORGANISE, prototype validated over six
passes) sorts mail into destinations — Inbox, Feed, Paper trail,
screened out — on the HEY model. The structuring question: where does
this destination live? Server side (moving IMAP messages) or
workstation side (a presentation datum)?

## Decision (CE, D1 of STOP 1, 2026-08-29)

**Local routing only, by sender.** The table
`routage_expediteurs(address PK, destination, regle, epoch)` — keyed
on the exact lowercase address, THE same normalization authority as
the image guard (A89), global to the workstation like it. Never an
IMAP move: the account's other clients see the mail unchanged, and
"Reinstate" (DELETE of the row) undoes everything — the rollback is
total.

- A thread belongs to a destination if **any one of its messages**
  comes from a sender routed there — never the head alone, which is
  the last message across all mailboxes: replying to it moved it to
  Sent and the thread would eject itself (proven RED, E1 review).
- The "Move to…" gesture resolves the address **on the core side**:
  the thread's last message that does not come from the account —
  never the user's own address (E1 review, proven RED).
- The vocabulary is closed and validated in Rust before any write
  (`valider_routage`); the SQLite CHECKs are only the belt.
- Only the **No rules** (E3: spam / archive / trash — D4: never a
  permanent deletion) will touch the server, through the existing
  `pending_actions` queue.

## Consequences

- Hot queries filter through an `EXISTS` probe
  (`idx_envelopes_thread` then the routing PK — spike S2: 0.209 ms at
  200 k, no directive `CROSS JOIN` needed), a plan guard "never an
  envelopes scan" in the net.
- Organized views do NOT exclude pinned threads (their dedicated
  section exists only in Inbox — excluding them would make a pinned,
  routed thread disappear from every view).
- Accepted and stated limit: the SQL comparison `lower(trim(...))`
  diverges from the Rust normalization on non-ASCII uppercase and
  Unicode whitespace — a real address is ASCII (punycode domain).

## Reversibility

`DROP TABLE routage_expediteurs` + removing the views: the classic
view stays intact by construction (the "zero diff with the mode off"
e2e guard).

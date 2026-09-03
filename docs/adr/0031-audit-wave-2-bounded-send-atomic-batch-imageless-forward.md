# ADR 0031 — Audit wave 2: bounded send, atomic batch, imageless forward, one single menu

Date: 2026-09-02 · Status: accepted
· Amends [ADR 0003](0003-smtp-outbox.md) (a transient send is no
  longer retried forever), lifts the §6.4 exception of the field
  session of 2026-08-20 (the forward's images), and closes the "front"
  half of D-47 (the product's menu).

## Context

Wave 2 of the 2026-09-01 audit (`docs/AUDIT-2026-09-01.md` §5,
`docs/PLAN-AUDIT-V2.md`) handles the measurable S2 items. Most are
technical remedies with no product decision — opening the database,
Cleanup's indexes, IMAP batches, resumable sync. Five points belonged
to the Chief Engineer: what becomes of a send that keeps failing, what
a bulk gesture that fails midway is worth, what "Forward" loads, what
to do with a server lacking CONDSTORE, and whether a flaky test should
turn the gate red.

## Decisions (CE, 2026-09-02 — D1-D8 of PLAN-AUDIT-V2)

**A poisoned send is REFUSED on the fifth transient failure** (D5,
`SEUIL_ENVOI = 5`, like action quarantine): it leaves the queue with
its reason, the user decides (resend, delete), and the next message
gets its turn. Before, `attempts` was counted but never read — one
poisoned message held the account's queue forever.

**A bulk gesture is ALL OR NOTHING** (D6): `Store::agir_groupe`
expands threads on the core side and chains the batch in ONE
transaction; a failure midway leaves nothing half-done, and the UI
says so. One single call replaces N × k unit commands in series (250 +
50 IPC calls for fifty conversations).

**"Forward" loads NO remote image** (D8): the composer receives the
forwarded block at the neutral pixel, marked with its source
(`data-wind-transfert="account/uid/mailbox"`, allowlisted at the
boundary); at send time, the block is replaced by the source's render
WITH its images — the recipient gets the same message, the tracking
pixel no longer fires on a "Forward" click. Stated limit: an edit
INSIDE the forwarded block is lost (we transmit, we don't annotate
line by line); what is typed BEFORE it stays.

**A server without CONDSTORE is a stated debt, not a job** (D3): its
flags do not resync; a line in `wind.log` names it at poll time, to
know whether the case exists in beta (Gmail, Microsoft 365 and Dovecot
all announce it).

**A flaky test is COUNTED, it does not turn the gate red** (D4,
confirms PLAN-KAIZEN E3): Playwright's JSON reporter and
`e2e/flaky.mjs` print "flaky: N" in the verdict — the figure the
`failOnFlakyTests` decision expected did not exist.

**One single menu** (D1, visual STOP of 2026-09-02 on the List):
`Menu.svelte` carries the drawing and the mechanics (keyboard
included) of the eight surfaces; the front entered the same wave as
the core.

## Consequences

- Measurements recorded in the PLAN: second `Store::open` 36 →
  0.9 ms (200 k), indexing a 28 MB body 401 → 338 ms and 210 → 133 MB,
  `nettoyage_groupes` 380 → 67 ms (200 k / 5,000 senders), MIME
  analysis 18.2 → 11.1 ms for 50 bodies, idle probes 5 → 2 per 10 s.
- Two audit remedies REFUSED on measurement or evidence: the COUNT per
  keystroke (1.5 ms out of 57: the cost is the sorted page) and
  `withGlobalTauri: false` (`__TAURI_INTERNALS__` stays injected; the
  CSP is the boundary, completed).
- Traps recorded: the embedded SQLite (3.50) can prefer the date index
  where another tool picks correctly — `INDEXED BY` and a query-plan
  test; a scriptless frame (S1) is not assessable by Playwright — the
  iframe is focused from the parent and the real key is struck.

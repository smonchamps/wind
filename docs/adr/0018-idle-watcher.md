# ADR 0018 — The IDLE watcher (real time, per account)

**Date**: 2026-08-14 · **Status**: accepted on the architecture,
**activation pending the spike gate** (field measurements: p50 ≤ 5 s,
p95 ≤ 30 s, reconnection after a cut and after Windows sleep/resume,
three providers — PLAN-SYNCHRO §7 "E4 spike"). Production wiring starts
only once this gate is green and the budgets re-measured.

## Context

Field session of 2026-08-13/14: E2a/E2c brought the cycle down from
~38 min to ~4 s, but the latency of a mail's arrival remains **the
polling cadence (5 min)** — the model is polling, not event-driven. Beta
complaint #1 ("no real real-time") targets this latency. P0-bis already
put an event-driven model on the network state; IDLE (RFC 2177) puts an
event-driven model on mail arrival: the server pushes an `EXISTS` as
soon as a message lands in INBOX, without the client asking again.

The spike (`spikes/idle/`) established feasibility and delivered a
first integration finding: **the crate's `idle()` handle sets its own
read timeout during the watch, then clears it back to `None` on
exit** — it would erase the P0 guard (120 s) set at connection time.

## Decision

1. **One watcher per account, a dedicated shell thread, on a DEDICATED
   IMAP connection** — never the cycle's own. Two reasons: the cycle's
   connection is short-lived (opened/closed per poll), a watcher must
   HOLD; and the `idle` handle mishandles the socket timeout — isolating
   it on its own connection protects the P0 timeout's lifecycle.

2. **`idle` is a capability of the ADAPTER (`mail-imap`), not of the
   `MailServer` trait.** The "envelopes first" engine (`mail-core`)
   knows nothing of IDLE: it is a BLOCKING operation that lives outside
   its command flow. The shell orchestrates the watchers; the engine
   stays pure. (Alternative ruled out: `idle` on the trait — it would
   force blocking onto every implementation and leak a transport detail
   into the core.)

3. **Watcher lifecycle**:
   - **SHORT IDLE renewal: ~3 min, not 28** — 1st field session of the
     spike (2026-08-14): the read timeout of the watch is ALSO the only
     detector of a dead connection. A Wi-Fi cut or Windows sleep produce
     NO error at all — the read blocks silently until the deadline (the
     Thunderbird "hang" on IP change, Mozilla bug 284152). At a 28 min
     renewal, the watcher would be blind for 28 min; at 3 min, death is
     detected within ≤ 3 min at the DONE/re-IDLE, for 2 commands per
     cycle — zero cost, and well under RFC 2177's 29 min;
   - **reconnection with doubling delay** (2 s → 60 s, rearmed after
     2 min of a stable session), carried over from the spike;
   - **OAuth token reread from the vault on every reconnection** — a
     session that drops after expiry restarts on its own;
   - **read timeout reset after every exit from the watch** (the
     crate's default, §Context) — or the socket would be left unguarded.

4. **An `EXISTS` wakes the light pass for the affected account** (E3,
   `sync_inbox_light` targeted at that account): it polls INBOX if
   something moved (E2a), counts the mail (P1) and emits the bubbles —
   **phone parity**. Then a **Tauri event** pushes the UI to reload the
   list and the nav. The watcher NEVER touches the database itself: it
   only signals, the light pass does the work (a single poll path).
   **AND the light pass ALSO runs on every (RE)CONNECTION of the
   watcher** — 2nd field session (2026-08-14): a mail that arrived
   DURING a cut is already in the mailbox at reconnection time, no
   `EXISTS` will ever signal it; without the reconnection pass, it would
   wait for the 5-min cycle.

5. **Interaction with P0-bis**: offline (`navigator.onLine` false), the
   watchers are stopped (a dead IDLE connection is useless and would pay
   for reconnections in a loop); on `online`, restarted. The UI drives
   start/stop through the network events already wired.

6. **Interaction with backoff (complement to P0)**: an account backing
   off after repeated failures does not restart its watcher before its
   delay ends — the same anti-hammering discipline as the cycle and the
   light pass.

7. **The full cycle (5 min) remains** for what IDLE does not cover:
   folders, drafts, deletion diffing, flags (CONDSTORE). IDLE watches
   ONLY INBOX. The full cycle's cadence will be re-discussed once IDLE
   is active (S-D4, open).

## Consequences

- **Phone parity on arrival**: latency targeted at p50 ≤ 5 s. Beta
  complaint #1 is closed for good.
- **Budget re-measured, mandatory**: a watcher = a persistent connection
  + a thread. RAM already at 184-187/200 MB. A broken budget is an
  **andon** — possible fallback: IDLE on the foreground account only, or
  watchers paused past a cap.
- **Complexity assumed**: threads, persistent connections, network
  lifecycle — this is the product's hard point (front-loading, HANDOVER
  §2.2). Hence the order: measured spike, THEN wiring.
- P0's socket timeouts remain the net for the "network up, server
  silent" case; P0-bis remains the fast detection of a clean cut. IDLE
  adds to these two nets, it does not replace them.

## What the spike must confirm before wiring

Protocol and gates in `spikes/idle/README.md`: p50/p95 latency over
10 arrivals, held for 60 min, reconnection after a network cut and
after Windows sleep/resume, behavior at OAuth token expiry — on Gmail,
Microsoft and a generic IMAP. The line stops if a gate breaks.

**1st field session (2026-08-14, Gmail)**: **held ✅** (2 h 42, one
EXISTS still served at the end); **latency ⚠️ to re-measure** — the
~30 s observed were counted from SEND, which includes Gmail delivery;
the right measurement is parity with the phone's bubble; **reconnection
❌ unproven** — the 28-min renewal made a cut or sleep invisible (hence
decision 3 amended: short renewal). Spike fixed the same day, cut/
sleep/OAuth to be replayed.

**2nd field session (2026-08-14, Gmail, 3-min renewal)**:
- **reconnection on a cut ✅** — detected in 1 min 48 (≤ 3 min, the
  short renewal at work), resumed 2 s after the network returned,
  doubling delay visible (2→4→8→16 s, OAuth failures logged during the
  outage);
- **resume from sleep ✅** — Windows aborts the socket on wake (10053):
  detected immediately, reconnected in 2 s;
- **the token rereads fine from the vault** on every reconnection
  (offline failures, success on return); the ~1 h expiry remains to be
  observed;
- **latency ✅ — bubble PARITY ACHIEVED: gap between the phone bubble
  and the EXISTS ≤ 3 s** (measured over 5 sends). The ~60 s tick observed
  (four EXISTS at :35 of the minute within 0.3 s) lives in Gmail's
  DELIVERY pipeline, upstream: it delays the phone as much as the IMAP —
  both are notified within the same second. IDLE has no handicap against
  the proprietary push; the "at the same time (< 30 s)" product gate is
  held with ~27 s of margin, light pass included;
- **finding: mail that arrived during the cut never emits an EXISTS**
  (already in the mailbox at the re-SELECT) — hence decision 4 amended:
  light pass on every (re)connection.

# ADR 0041 — Rotate bounded flag checks through cached mail

Date: 2026-09-07. Status: implemented for Lot 3; field timing pending.
Scope: [Lot 3 E9 / B23](../archives/PLAN-AUDIT-2026-09-LOT3.md).

## Finding and measured options

Without CONDSTORE, only the newest 500 flags were checked. Quiet archive folders
skipped even that pass. Four maintained regressions reproduced starvation,
quiet-folder omission, absent progression after retry, and application to an
obsolete UIDVALIDITY namespace.

An isolated synthetic SQLite spike compared 500 rotating UIDs with 100 recent
plus 400 rotating. On 200k rows, selection median/p95 was 0.634/1.239 ms versus
0.738/1.410 ms; complete sweeps required 400 versus 500 passes. One effective
keyset predicate takes approximately 2,500 SQLite VM instructions at all depths;
two redundant upper bounds accidentally caused 801,500 near the tail.

## Decision

Preserve the existing 500-UID total per mailbox opportunity: the newest 100,
then up to 400 older UIDs below a persisted exclusive cursor. Use the existing
(mailbox_id, uid) index and one effective bound. The descending cursor itself
keeps new arrivals from restarting the sweep. Wrap after reaching the tail.
Successful missing-UID responses advance; they do not imply local deletion.

Both incremental and quiet paths use this window. Quiet non-CONDSTORE archives
now perform it during the full cycle without paying a UID inventory. CONDSTORE
keeps its delta path. Apply returned flags and cursor advancement in one
transaction, retaining queued local intentions and refreshing affected threads.
Reject stale mailbox identities, stale windows and unexpected/duplicate UIDs.
Verify remote UIDVALIDITY with SELECT before and after FETCH. Failures consume
no cursor progress. Mailbox reset clears the cursor; account deletion cascades.

## Costs and limits

The newest 100 remain checked each successful opportunity. Ranks 101–500 now
share the rotating schedule with older mail. At nominal 5-minute INBOX and
30-minute full cycles, a stable 200k-row sweep takes 41 h 40 / 10 d 10. This is
arithmetic over successful opportunities, not a real-time guarantee: outages,
backoff, suspension, mailbox growth and long cycles extend it. IDLE arrivals and
manual checks may shorten it. No cadence or larger admission budget is added.

Production fencing costs two SELECTs plus one bounded FETCH per nonempty pass,
more than the spike's selection-only command estimate. A quiet archive adds
48 FETCHs and 96 SELECTs per nominal online day. Full-cycle network duration
must be measured in the field; the spike does not prove the cycle latency budget.
Global time/byte/disk admission and durable operation issues remain separate E9
work and must preserve fairness when they defer folders.

The maintained tests cover arrival churn, restart, sparse/deleted UIDs, maximum
UID, wrap/reset, partial responses, failed FETCH, failed flag/cursor writes,
late windows and namespace replacement before/during retrieval.

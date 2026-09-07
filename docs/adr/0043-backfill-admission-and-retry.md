# ADR 0043 — Spent backfill budgets and persistent scan positions

Date: 2026-09-07. Status: implemented for Lot 3 E9; amended the same day by the
lot's fresh-eyes review (retry policy, budget floor, arrivals head, draft pull,
click-path admission). Field validation pending.

## Finding and measured alternatives

The old pumps counted saved rows. Empty responses spent no allowance and could
walk the entire corpus. A failed pass restarted at the newest missing messages.
Returned-row LIMIT also hid unbounded scans through cached or refused entries.

Two isolated synthetic SQLite spikes compared row-value continuation with
date-head probes followed by exact-date UID ranges. Both reused the existing
`idx_envelopes_date`; neither added an index. A row-value query was fast for
distinct dates but still scanned large equal-date groups. Exact-date ranges
avoid that sort and limit examined envelopes before testing eligibility.

On SQLite 3.50.4, 200,000 synthetic rows, 128 candidates and 15 warm repetitions:

| Case | Baseline median | Exact-date ranges median |
|---|---:|---:|
| Distinct dates, first page | 0.235 ms | 3.723 ms |
| Distinct dates, depth 199,000 | 121.354 ms | 3.967 ms |
| All dates equal, first page | 90.146 ms | 0.251 ms |

At depth 199,000, VM instructions fell from 4,981,092 to 8,730. Distinct dates
pay about two small statements per examined envelope. These are warm in-memory
SQL measurements, not cold disk or whole-application latency measurements.

## Decision

Persist one scan position per mailbox, UIDVALIDITY and pump. Traverse date DESC,
UID DESC; NULL dates follow numeric dates and remain distinct from `i64::MIN`.
Claim examined positions in a short transaction before network I/O. A position
records an attempt, never a downloaded body. Missing or failed content returns
on a later sweep; only checked cache writes count as fetched. Preserve the
position through restart, invalidate it on namespace replacement and enforce a
narrower import horizon when resuming. The cursor also keeps the highest UID it
has examined: every claim first scans the UIDs above that head (arrivals since
the last claim, oldest first) before the historical sweep resumes, so a burst of
new messages never waits for a full wrap.

Share candidate allowances across folders and accounts. Debit before I/O,
including misses and errors. Each caller also shares 64 MiB of actual IMAP
plaintext input and a 120-second pass deadline. Inner raw-message admission
retains [D9's 32 MiB ceiling](0042-raw-imap-download-ceiling.md). A pass stops
admitting candidates once fewer bytes remain than one maximal message, and the
IMAP adapter plans its batches against the remaining scope as well as the
ceiling: a window claimed against a scope too small for its content would be
scanned without being served, and a batch planned past the scope was cut
mid-literal. The pass deadline refuses new commands; a response already flowing
is bounded by its own command deadline. Clearing an outer allowance never
revives a session whose parser response was truncated. Neither deadline is an
end-to-end bound on DNS, authentication, SQLite or the whole sync pipeline.

Disk admission is taken by the background pumps once per window or pass, before
the network. The user's own reads and writes never pay the 64 MiB reserve: a
click reads a message whenever the server can deliver it, and a draft is saved
on a full disk. Remote draft pulls end a pass normally when their allowance is
spent, skip a draft above the raw ceiling (it stays in webmail) and persist their
cursor only after a failed fetch.

Rotate the first scheduled mailbox between passes. Arrival previews request
their new UIDs directly, independently of historical scan positions. A scanned
window with no downloaded body is not a completion signal. The UI limits one
automatic pumping burst to two minutes and resumes on subsequent opportunities.

Persist partial-operation failures separately from connection state: a failed
connection is the account's state (connection notice and backoff) and records
no per-folder issue. A success in another mailbox cannot erase them. Failures retry after 30 seconds, doubling
up to 30 minutes; server refusals (any tagged NO/BAD, transient or not) follow
the same backoff, because one "NO [UNAVAILABLE]" must not park a folder until a
gesture. Only a corrupt local scope waits for a manual retry. A manual retry
releases the delay but keeps the diagnostic until successful work; the Sync
button runs the full cycle for the account while a diagnostic exists. A sweep
that completes without error settles the diagnostic even when it downloaded
nothing: content the server no longer holds is missing, not a failure, and stays
counted as missing. Per-message size refusals retain their separate UID-scoped
markers, are not operation failures and do not disable a whole folder.

Stated limits: the body pump has two drivers (the scheduler after every due
cycle, and the UI burst after a generation change), each bounded per pass and
serialized on one lock, so up to two passes can run per opportunity and the UI's
figures can wait behind the scheduler's pass. Errors crossing the shell as
strings are settled as ordinary failures, which the uniform backoff tolerates.

## Validation and limits

Maintained regressions cover empty responses, shared spent allowances, failed
continuation, cached prefixes, NULL/minimum dates, horizon changes, arrival
previews, oversized header responses, absolute deadlines and poisoned sessions.
The pre-diagnostic whole-core checkpoint passed 552 tests with two existing
ignores. Final integration, whole-lot review, gate and field are still due.

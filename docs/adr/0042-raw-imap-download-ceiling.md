# ADR 0042 — Bound complete raw IMAP downloads

Date: 2026-09-07. Status: implemented for Lot 3; integration and field pending.
Scope: [Lot 3 E9 / S04](../archives/PLAN-AUDIT-2026-09-LOT3.md), decision D9.

## Finding and options

RFC822.SIZE previously guided grouping but did not limit actual reads. Oversized
messages travelled alone; attachment and draft reads skipped even that estimate.
A missing size response could silently omit a body. Four maintained REDs proved
oversized admission, omitted unknown-size mail and unexpected/duplicate UID
acceptance. Additional REDs exposed accumulated raw inputs across commands and
the floating action bar covering a body-failure explanation.

Isolated synthetic spikes compared bounded complete-message reads with partial
MIME retrieval. Partial attachments reduced memory, but the pinned IMAP library
does not expose the returned partial offset: a server repeating offset zero
could corrupt an apparently successful download. Adopting that path requires a
protocol adapter/dependency change and MIME-index parity work.

The Chief Engineer chose 32 MiB after comparing 8/16/32 MiB. Synthetic fresh-process Windows
working-set deltas for conversion were approximately 54/102/198 MiB. These are
not measurements of Wind's entire working set or a proof of its 200 MiB budget.

## Decision

Allow at most 33,554,432 bytes per complete raw MIME message, including headers,
body and encoded attachments. Preserve envelopes and existing cached content.
Bodies, attachment extraction and drafts share the same checked retrieval path.

Query sizes in groups of at most 50 UIDs. Treat missing sizes as unknown, with a
full-ceiling weight; reject an advertised oversize before requesting its body.
Wrap the plaintext stream below the IMAP parser with an armed byte allowance and
an absolute command deadline of 120 seconds plus the announced wire size at
100 KiB/s (about 7.5 minutes for a 32 MiB message; review correction of
2026-09-07: a fixed 120 s abandoned admissible bodies on slow links forever,
without recording any refusal). Limit each raw reply to its admitted
raw budget plus 8 KiB of protocol allowance. Bound size replies separately to
8 KiB plus 128 bytes per requested UID. Reject duplicate/unrequested body UIDs.
Check actual per-message and aggregate lengths before MIME conversion.

One body-loading call shares 32 MiB of actual raw input across its successive
commands. When the next estimated batch no longer fits, return completed bodies;
the remaining UIDs stay eligible for a later call. A server understating sizes
cannot enlarge the admitted remainder. BODY.PEEK preserves unread state, and
complete-message parsing preserves attachment indices and bytes.

After an incomplete response or invalid UID/batch response, poison the session: reads, writes and
later operation admission fail, including logout. Its owner must drop/reconnect;
the stream wrapper does not independently close the underlying socket. A complete
size refusal (from metadata or a fully read literal) permits the namespace
fence and other UIDs on the same session. Existing TLS timeout-floor and cancellation tests remain green.

The command boundary carries a stable size-refusal code; ordinary errors retain
their string wire shape. The reader explains the 32 MiB ceiling and webmail
fallback without offering retry for this refusal. Ordinary loading errors retain
retry. Error notices keep their action bar in normal flow to prevent overlap;
loaded messages retain the floating bar. System A131 records the UI amendment.

## Limits and remaining integration

This bounds raw input, not MIME allocations, HTML expansion, sanitized output,
parallel jobs or the whole-app working set. The command deadline does not bound
DNS resolution, connection establishment, every other IMAP operation, or a whole
synchronization cycle. Larger unsolicited protocol traffic can exhaust the
allowance even when the message itself would fit.

Size refusals confirmed in the same remote and local mailbox namespace are
persisted by (mailbox_id, UID) in a small WITHOUT ROWID table. Later backfill
passes exclude these UIDs, and on-demand reading explains the refusal offline.
The first refused batch still returns an error; its unrelated UIDs remain
eligible for the next pass. An account with only refused pending bodies does not
open a backfill connection. Pending counts continue to include refused content,
so they never claim it was downloaded. Cache reads take precedence. Message,
mailbox and account removal purge the markers; a larger future ceiling permits
re-evaluation. No body or attachment bytes are erased by a refusal.

The first persistence implementation added an envelope column. The maintained
SQL-plan guard caught the resulting loss of covering index reads. The separate
small table preserves covering access to both envelopes and bodies without
rebuilding an existing index. Reopen, failed writes, namespace replacement,
next-pass progress and cached-content preservation are covered by maintained tests.

E9 still needs shared spent attempt/time/byte admission across backfill calls,
durable per-operation failures and fair retries, and disk admission before each
batch. These remaining integrations require tests before final review, full gate
and real-account field validation. This increment is not delivery of Lot 3.

Maintained coverage includes exact/over-limit input, honest/missing/lying sizes,
64 MiB synthetic literals and unterminated lines, bounded metadata, aggregate
and successive-command budgets, session poisoning, a slow drip deadline, nested
attachment parity, stable IPC error shape, envelope visibility, ordinary retry
and notice/action-bar geometry.

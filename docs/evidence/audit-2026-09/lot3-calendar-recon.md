# Lot 3 calendar reconnaissance (B16)

Date: 2026-09-06. Read-only production investigation; no real account, database, MIME, or credentials used. No maintained test changes and no full gate. Workflow/job read. Named exploration model unavailable; inherited model used.

## Confirmed current defects

1. `crates/mail-core/src/store.rs:524-592`: both cancellation paths join only account and event UID. Neither stored `sequence` nor organizer participates. The boolean is set monotonically, so even a newly received REQUEST sequence 2 is cancelled by an earlier CANCEL sequence 1.
2. `crates/mail-ical/src/lib.rs:74-99,132-205`: parser/model omit DTSTAMP and RECURRENCE-ID, and choose the first VEVENT only. The storage schema likewise cannot distinguish occurrences, a master series, or partial/ranged cancellation. Consequently adding `sequence >=` alone cannot complete B16.
3. `ReplyRequest` and `itip_reply` omit RECURRENCE-ID (`lib.rs:101,212`). `apps/desktop/src/commands.rs:1555` builds a reply from these fields. Replying to an occurrence currently generates a reply without its occurrence identity.
4. `invitation_view`/`reply_invitation` only check method, organizer presence and cancellation. They do not recognize outdated REQUEST revisions. This is a related policy gap: an old update can still offer an RSVP using its old SEQUENCE.
5. Current historical cards retain only extracted fields plus rendered HTML (`store.rs:472`); raw calendar source is absent. A schema-only change cannot recover DTSTAMP or occurrence identity. `annule` may already encode the old wrong cross-cancellation.

## Synthetic SQL measurement

Ran Python SQLite `:memory:` with the exact current SCHEMA and the three SQL strings extracted from `write_invitation`. Two fresh synthetic stores per condition; current cancellation decision ported verbatim. This is a SQL-level reproduction, not a Rust end-to-end RED.

| Condition | Arrival orders | Actual | Expected |
|---|---:|---|---|
| REQUEST 2 / CANCEL 1, same organizer | Both | REQUEST cancelled in 2/2 | Never cancelled by older revision |
| REQUEST 2 / CANCEL 3, different organizer | Both | REQUEST cancelled in 2/2 | No cross-organizer action |

Output: `target/lot3-calendar-sql-probe.json`. Four incorrect cancellations out of four. No recurrence runtime claim: that defect is established by missing fields, not measured with a parser probe here.

## Protocol facts checked

[5546 section 2.1.5](https://www.rfc-editor.org/rfc/rfc5546.html#section-2.1.5) orders same UID/occurrence revisions by SEQUENCE, then DTSTAMP. [Section 3.2.5](https://www.rfc-editor.org/rfc/rfc5546.html#section-3.2.5) distinguishes whole-series cancellation (no RID), individual cancellation (RID), and THISANDFUTURE or multiple VEVENTs. CANCEL comes from the organizer. [Section 3.2.2.4](https://www.rfc-editor.org/rfc/rfc5546.html#section-3.2.2.4) permits an organizer change after agreement outside the protocol; silently treating all organizers as interchangeable does not implement that agreement.

[5545 section 3.8.4.4](https://www.rfc-editor.org/rfc/rfc5545.html#section-3.8.4.4) defines RECURRENCE-ID as the original occurrence position, including its DATE/DATE-TIME representation and timezone. It is not the moved occurrence's new DTSTART. The identifier must survive a reply. Keep DATE distinct from midnight DATE-TIME, and keep unresolved timezone names distinct.

## Recommended bounded implementation

Keep `mail-ical` pure and calcard-based (ADR 0024). Add parsed DTSTAMP as optional UTC epoch, a typed occurrence identity, an explicit supported/ambiguous scheduling shape, and an extraction version to stored rows. Preserve an exact writer-safe recurrence entry (value type, TZID, date/time and relevant parameters) for reply generation; do not serialize arbitrary raw ICS lines from the UI. Use a normalized comparison key separately if necessary. Do not reuse `When::Floating` as the only occurrence key: it loses timezone and seconds, and can alias unrelated occurrences.

Make cancellation a pure function with explicit outcomes applicable / stale / unrelated / indeterminate. Account, UID, and organizer must match (existing product comparison for addresses can use ASCII case-insensitive matching; no dot/plus alias heuristics). Missing organizer must never match another missing organizer. Matching content is correlation, not authentication: do not claim it prevents a forged organizer address.

A cancellation with a known master scope can apply to a known individual scope; an individual cancellation cannot cancel the master or another individual. Then compare SEQUENCE; use DTSTAMP only for equal sequence. Never substitute IMAP UID, message Date, arrival time, or a default current timestamp. Missing DTSTAMP at equal sequence and exact conflicting ties need an explicit conservative policy. Recompute request state from applicable evidence on relevant writes; do not keep the old monotone `annule` update. Index the account/UID lookup via mailboxes and a suitable invitation index; no full-table scan on each message write.

For this mail-client scope, recommend supporting a single unambiguous occurrence and whole-series cancellation first, while showing multi-VEVENT and ranged/unknown occurrence messages as non-actionable rather than silently reducing them to the first event or whole series. Implementing complete multi-event/range state is a larger scheduling engine decision. Existing forwarded-invitation approval must remain: do not require the local account to be an original ATTENDEE merely as a shortcut.

For stale REQUEST cards, prefer a distinct superseded state that disables RSVP, rather than calling them cancelled. Revalidate eligibility inside the same transaction that records `reponse` and enqueues the reply: `enqueue_invitation_reply` currently verifies row existence only. A newer cancellation/update can otherwise arrive between shell read and enqueue despite a correct display.

## Historical reconstruction without losing local replies

- Add nullable ordering/occurrence columns and a parser-version marker. NULL legacy occurrence is UNKNOWN, not proof of a master series.
- Keep `reponse` and `reponse_epoch` intact. `write_invitation` already preserves them on upsert; reuse that guarantee. Never clear the entire invitations table as migration.
- Mark only existing invitation rows as requiring metadata refresh. A bounded background/on-open calendar refresh should refetch the original message through the existing captured MailboxIdentity guard, then rewrite the extracted invitation. Keep cached HTML readable offline during refresh.
- A plain `scanned=0` is ineffective: `store/sql.rs:82-119` uses only body existence. Deleting all cached bodies would violate the local-first/performance intent and is unnecessary. A separate indexed repair queue/version predicate can remain bounded to invitations and avoid a global scan of HTML/FTS.
- Use existing fetch and write transaction paths to keep attachments/preview/invitation coherent. If scheduling a body invalidation instead, do so only for known invitation keys and explicitly weigh temporary offline body loss. Do not delete local replies with the stale cache.
- Repair derived cancellation only once enough metadata is known. While identity/version is incomplete, show an unverified card and disable unsafe RSVP if conflicting cancellation evidence exists; do not manufacture recurrence identity from DTSTART or metadata defaults.
- Reopening the store, failed fetch, mailbox reset, absent remote message and UID reuse must leave the marker safe/retryable or terminate it explicitly. Never mark a failed migration complete globally.

## Maintained test matrix for RED then GREEN

1. Parser: DTSTAMP UTC; absent/invalid timestamp; SEQUENCE absent zero and invalid/negative policy; RID UTC, DATE, TZID, floating seconds, unknown TZID; moved DTSTART does not alter RID; multi-VEVENT and RANGE explicitly recognized.
2. For each of the six arrival permutations of REQUEST 0, CANCEL 1, REQUEST 2, latest applicable request remains active. Repeat same-SEQUENCE cases with DTSTAMP older/newer, exact ties and missing timestamp policy.
3. Organizer isolation (including missing organizer, case changes), account isolation, UID isolation, occurrence A versus B, instance CANCEL versus master, whole-series CANCEL versus instances, higher-sequence replacement, restart and repeated re-fetch.
4. REPLY round-trip preserves RID value type/timezone/seconds while preserving UID, SEQUENCE, organizer and local attendee. Unsupported shape cannot enqueue an RSVP.
5. Historical database with bad `annule=1` from CANCEL 1 / REQUEST 2: bounded reconstruction in either order repairs state; local reply and epoch survive; cached HTML remains offline; no reparsing of ordinary mails; failure is retryable; second startup does not enqueue completed work.
6. Synthetic mailbox reset during historical refresh cannot attach old recurrence metadata or enqueue RSVP onto reused UID. A cancellation committed after view read but before enqueue blocks that enqueue atomically.
7. Whole invitation E2E spec: a supported occurrence offers the intended RSVP; stale/cancelled/ambiguous cards do not; historical refresh state is understandable. Retain forwarded-invitation coverage.

## Chief Engineer decisions to include in lot plan

- Scope choice: single occurrence + whole series, with explicit non-actionable fallback for multi-VEVENT/RANGE/unresolved identity (recommended), or full recurrence scheduling support now.
- Equal-version conflict / absent DTSTAMP behavior: conservative unverified state and blocked RSVP when conflicting evidence cannot be ordered (recommended), rather than arbitrarily choosing arrival or giving CANCEL an undocumented tie-break.
- Organizer handoff stays an explicit unsupported trust transition unless a separate verified handoff feature is chosen. Same UID from another organizer must not automatically cancel an existing organizer's event.
- Historical repair UX: preserve offline body and local reply, bounded refresh on open/background, show uncertainty until reconstructed (recommended). This entails a small UI state and its early visual validation.

These are proposed application policies, not a claim of complete iTIP support. Confirm the chosen subset in the focused plan before production edits.

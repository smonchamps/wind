# ADR 0035 — The MailServer trait cleaned

Date: 2026-09-04 · Status: accepted (PLAN-AUDIT-V3 E6, audit 3.7)
· Extends [ADR 0033](0033-poll-policy-lives-in-the-core.md) (settles
  the trait question it deferred).

## Context

The audit's item 3.7 asked four things of the network boundary:
`capabilities()` separated, `Folder` completed with its `delimiter`,
`fetch_body_html`/`fetch_recipients` removed, and the attachment MIME
index made stable (D-30's neighborhood). The E6 recon re-measured each
on the current tree.

## Decisions

- **`fetch_body_html` removed**: it was a unit duplicate of
  `fetch_bodies_html` (the audit's own finding); its one production
  caller (`load_body`) rides the batched path with a one-element
  slice. One fetch surface, not two.
- **`fetch_recipients` removed**: `fetch_envelopes` already returns
  `to_addrs`/`cc_addrs` — the recipients backfill has used it all
  along, and the shell's on-demand "Reply all" path now does too. A
  method that re-asks for a subset of what another returns is a trap
  for adapters.
- **`Folder` gains `delimiter: Option<String>`**: the imap crate
  parses the LIST delimiter and `name_to_folder` dropped it — the
  model stops lying about the hierarchy. No consumer is added; the
  field is there for the day a folder tree needs it.
- **D-30's user-facing gap repaired**: the `pieces-calendrier` repair
  widens with a second criterion — a stored body carrying a
  `BEGIN:VCALENDAR` marker without a calendar attachment row is
  re-read at the repair moment (one bounded pass, never per poll).
  Proven RED first. D-30 closes; what remains of the audit's
  rank-stability fact is re-scoped as its own debt line: the
  attachment rank is a function of the adapter's inline filters —
  frozen in practice, and any future filter change must carry a
  rank migration.
- **`capabilities()` REFUSED** (§2.6, Chief-Engineer decision at this
  step): the trait already expresses an absent capability as an honest
  sentinel — `changes_since` → `None` without CONDSTORE,
  `folders_with_status` → `None` without LIST-STATUS — each tested. A
  separate capability struct would be a second source of truth the
  adapters must keep consistent with those same answers, for no
  current consumer. Reopens if a third capability-flavored method
  appears on the trait.
- **`CycleConnection` stays a separate trait**: the two IMAP-inherent
  capabilities (sent-folder heuristics, the drafts pull with its
  HTML-sanitizing boundary) are not portable server operations — no
  fake server has a reason to fake them, and the draft half would drag
  mail-render toward the core. ADR 0033's deferral resolves as: the
  split is the design, not a stopgap.

## Named limits

- The attachment rank/filter coupling stays (its debt line carries the
  reopening condition); addressing parts by structural MIME path would
  require a rank migration of stored rows — not worth it while the
  filters are frozen.
- `logout()` remains outside both traits (ownership — the core only
  ever borrows the connection).

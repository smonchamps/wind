# ADR 0024 — iCalendar parser: the `calcard` crate, in a pure `mail-ical` crate

Date: 2026-08-22 · Status: accepted (CE decision D1, PLAN-INVITATIONS)

## Context

Handling meeting invitations (PLAN-INVITATIONS) requires reading the
iCalendar (RFC 5545) of `text/calendar` parts and generating iTIP
replies (`METHOD:REPLY`, RFC 5546). The hard point is **timezone
resolution**: Google emits IANA TZIDs (`Europe/Paris`),
Outlook/Exchange **Windows** TZIDs ("Romance Standard Time") — a wrong
resolution would show a wrong meeting time, the worst possible lie for
this feature.

## Decision

**The `calcard` 0.3.11 crate** (Stalwart Labs — the home of
`mail-parser`, already in the repository), with
`default-features = false`, in a **pure `mail-ical` crate** (zero I/O,
zero clock — the DTSTAMP comes from the caller): parser + REPLY
generator, the application only ever sees `Invitation` and
`reponse_itip`.

## Tie-break (set-based, 2026-08-22)

Two throw-away spikes on a shared corpus of 6 fixtures (Google/IANA,
Outlook/Windows TZID, bare UTC, all-day, CANCEL, recurrence) + a REPLY
generation trial (75-byte folding, CRLF, re-parse):

| Criterion | A — `calcard` | B — homegrown parser + `chrono-tz` |
|---|---|---|
| Corpus correctness | 71/71 PASS | 81/81 PASS |
| Windows TZID | full table embedded | homegrown table (~140 CLDR entries to maintain) |
| Binary weight (arm64, release) | +1.73 MiB | +1.36 MB (the tz database dominates both) |
| Cost of ownership | **~150 lines of glue** | **~600-700 lines owned** (a real bug paid for while writing the spike) |
| Parse time | 2-8 µs | 2.6-6.7 µs |

Correctness and speed tied, comparable weight (the < 15 MB installer
budget holds with room to spare): by the §2.3 rule, the alternative
does not beat the hypothesis — cost of ownership is what decides.

## The trap, engraved

`TzResolver::resolve_or_default` falls back to `Tz::Floating` for a
TZID outside the tables: the time would be treated as UTC — **a
silent shift**, measured at the spike ("Zone Perso Wind" probe: 09:00
rendered as 09:00Z instead of 08:00Z). `mail-ical` therefore calls
**`resolve()`** and handles the `None`: the time becomes
`Quand::Flottant`, displayed AS IS with the note "organizer's local
time" — never a lying conversion (guard D1, held by test). Embedded
VTIMEZONEs are not interpreted (calcard resolves by TZID NAME); a
producer with a proprietary TZID falls into this honest fallback.

## Consequences

- Sixth crate of the workspace (`crates/mail-ical`), consumed by
  `mail-core` (§4 pattern: pure and testable decision, I/O elsewhere).
- Dependencies: `calcard` + `chrono-tz` (+ `mail-builder`, transitive).
- The spikes' corpus is checked in as tests (`crates/mail-ical/tests/`),
  replayed at every gate.

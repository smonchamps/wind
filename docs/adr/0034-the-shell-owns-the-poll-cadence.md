# ADR 0034 — The shell owns the poll cadence

Date: 2026-09-04 · Status: accepted (PLAN-AUDIT-V3 E5, audit 3.2,
Chief-Engineer decision D4)
· Extends [ADR 0021](0021-full-cycle-cadence.md) (the figures — 30 min
  full, 5 min light, sleep-wake at 120 s — are unchanged) and
  [ADR 0033](0033-poll-policy-lives-in-the-core.md) (the tick's
  DECISION is policy and joins the core).

## Context

The audit's item 3.2 said "tokio scheduler on the shell side"; the
re-measured finding (this plan's §1.2) was sharper: the entire cadence
was JS `setInterval` in App.svelte — full cycle, light pass, the
sleep-wake detector, the poll on network return, plus a 10 s
whole-list `list_drafts` poll (D-52 item 3). A window closed to the
tray, or a busy renderer, and no cadence existed but the IDLE
watcher's.

## Decision

- **The DECISION of a tick is core policy**:
  `mail_core::cycle::Cadence` — full/light intervals, wake-lag
  detection, network-return, and the reality rearms `ran_full`/
  `ran_light` — a pure state machine under three unit tests.
- **The CLOCK is the shell's**: one thread (`poll::spawn_scheduler`,
  15 s tick) asks the cadence what is due and invokes the SAME Tauri
  commands the UI's timers invoked, in the same sequences (full cycle
  → outbox flush → draft reflection; light pass → flush) — behavior
  identical by construction. Offline, or with a cycle in flight, a
  tick does nothing; `network_state` kicks the light pass on the
  offline→online transition.
- **The commands rearm the cadence**: a cycle run by the UI (startup
  after `connect()`, the manual button) counts as reality — the next
  tick never doubles it. The UI keeps the manual gestures, the
  startup trigger, its watchdog, and the 5 s resting probe, which
  already reloads the views when the generation moves.
- **The drafts poll dies** (D-52 item 3): `ui_state` carries a cheap
  drafts revision (count, latest edit, largest id); the UI fetches the
  actual list only when it moves.

## Named limits

- The scheduler waits for at least one connected account and never
  consumes the cadence before that; startup mail is the UI's startup
  cycle, as before.
- The drafts revision misses an in-place edit that changes neither
  count, `updated_epoch` nor max id — every current write path bumps
  `updated_epoch`, so the case is theoretical; the UI still probes
  directly after its own draft gestures.
- The 15 s tick bounds how late a due pass can start; the previous
  JS timers had the same order of slack.

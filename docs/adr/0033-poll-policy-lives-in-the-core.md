# ADR 0033 — Poll policy lives in the core

Date: 2026-09-04 · Status: accepted (PLAN-AUDIT-V3 E4, audit 3.1)
· Extends [ADR 0019](0019-commands-off-the-main-thread.md) (the pump
  discipline is unchanged — what runs off the pump merely changed
  crates).

## Context

The full audit of 2026-09-01 found the entire per-account poll
pipeline living in the Tauri shell: `run_sync` (~320 lines — INBOX
poll, inventory, disk guard, guarded folder sweep, thread headers,
recipients, drafts, echo reconciliation), `poll_inbox`, and their
helpers sat in `commands.rs`, and the IDLE watcher imported
`crate::commands` for seven symbols. Consequence: the poll POLICY —
what to poll, in what order, what to skip, when to warn — had **no
test reachable without the Tauri shell**, while `mail-core` already
owned every pure decision it delegates to (`must_poll`,
`sync_order`, `disk_shortfall`, `horizon_epoch`).

## Decision

- The orchestration moves to **`mail-core::cycle`**: `run_sync` and
  `poll_inbox` are generic over the server and a new **`CycleHooks`
  trait** — the five things only a shell can do come in through it:
  progress bookkeeping for the status bar (mailbox/phase/mail
  counter/generation), the arrival notification, the trace line, the
  once-per-session CONDSTORE warning memory, and the disk-space probe.
  `NoHooks` is the honest no-shell default; the tests drive a full
  cycle on `FakeServer` with it — proven RED first (the function did
  not exist in the core).
- The shell keeps what is genuinely its own, in a new
  **`apps/desktop/src/poll.rs`**: connection and authentication
  (`connect_imap`), the backoff table, the per-account locks, the
  cycle loop (`poll_cycle`) and its tally, and `ShellHooks` — the
  `CycleHooks` implementation over `SyncShared` + `AppHandle`.
- **The watcher no longer imports `commands`**: it calls `poll`. The
  dependency `watcher → commands` (seven symbols) is dead.

- The full pipeline needs two capabilities `MailServer` never carried
  (the Sent-folder heuristic and the drafts pull are INHERENT
  `ImapServer` methods): a narrower **`CycleConnection: MailServer`**
  trait in `cycle.rs` names exactly those, `ShellServer` (in
  `poll.rs`, the one place that knows both crates) delegates to
  `ImapServer`, and `FakeServer` answers the honest "no such folder"
  default. Widening `MailServer` itself is E6's subject (audit 3.7),
  not this move's.
- `logout()` stays at the shell: the core borrows the connection
  (`&mut S`), and `ImapServer::logout(self)` consumes it — ownership
  of the connection never enters the core.

## Named limits

- The cycle functions return `Result<_, String>` at their boundary —
  the error typing of E3 stops at the shell's `CommandError`; typing
  the core cycle's errors is not this ADR's subject.
- The scheduler still lives where it lived (the UI's timers) until E5
  — this ADR moves the POLICY, not the CADENCE.
- `SyncOutcome` in the core carries no session type: token refresh
  stays paired shell-side, where the connection is made (mail-core
  does not depend on mail-auth).

# State — Wind's handover snapshot

> **This document is rewritten at every job — that is its function.**
> Version delivered, next job, field figures, open trade-offs,
> deferrals: everything volatile lives here. The method, the invariants
> and the traps live in [STANDARD.md](STANDARD.md) — they are not
> rewritten, they are amended. The closed jobs' narrative lives in
> [docs/archives/](archives/) — the plans themselves, and
> [STATE-HISTORY-2026-09.md](archives/STATE-HISTORY-2026-09.md), the
> frozen long-form history this file carried until 2026-09-07 (audit
> Lot 6 E16d, C07).

## The product, in one paragraph

A Rust workspace of six `mail-*` crates (core/SQLite store, IMAP, SMTP,
OAuth, iCalendar, HTML rendering) under a Tauri 2 shell
(`apps/desktop`: `commands.rs` IPC + `ui-v2/`, Svelte — ADR 0015), a
Playwright e2e suite driving the real WebView2 window, and four shipped
platform triples (Windows x64/arm64, macOS Intel/Apple Silicon). What
each platform has actually proven lives in the
[support and proof matrix](MACOS-BUILD.md#support-and-proof-matrix-lot-5-e15e-audit-g06).
Budgets and their dated 2026-09-07 baseline: STANDARD §3. The field
database reads 12.49 GB (2026-09-07, Lot 5 E14).

## The audit program of 2026-09-06 — CLOSED 2026-09-08

Six lots over [AUDIT-2026-09-06.md](AUDIT-2026-09-06.md), all
delivered, field-validated, pushed, CI green, **shipped in 0.20.0**
and `/close`d ([program plan](PLAN-AUDIT-2026-09.md)):

- Lots 1–2 (2026-09-06): `e95adb4`, `0374e43`; their own green CIs.
- Lot 3 ([plan](PLAN-AUDIT-2026-09-LOT3.md), `9f4a9b1`, field
  "1-11 OK"), Lot 4 ([plan](PLAN-AUDIT-2026-09-LOT4.md), `400de5e`),
  Lot 5 ([plan](PLAN-AUDIT-2026-09-LOT5.md), `2e096d6`/`516180e`/
  `46c1463`), Lot 6 ([plan](PLAN-AUDIT-2026-09-LOT6.md), `01b9171`) —
  grouped push, CI GREEN 34164333355 (D0 amended by the Chief
  Engineer: the ultra review refused the diff on size, the push was
  ordered without it).
- The one red on the way: the Lot 4 seam guard's FIRST CI run caught
  Lot 5 E14's unguarded `__e2eNoRestart` read (`dfd734c`, guard
  replayed RED→GREEN). The `quality-windows-arm64` job proved itself
  on the same run.
- At close (2026-09-08): D-4 and D-41–D-44 struck; **D-64** (the
  `redesign-feedback-3` in-suite flake) and **D-65** (Lot 3 E9's
  bounded-work residue and ADR 0041's open cycle measurement) opened.
- Dependabot's first ten PRs: nine failed on the seam job only
  (rebase them); the majors (vite 8, eslint 10, rusqlite 0.40, …) fail
  on their own merits and are each a dedicated decision.

**Next, in order**: the `/field` on beta T2's P0 (sync stalled at
72 %, on the tester's trace, T3 as the same-architecture witness) →
the two S bugs sharing one component (the product menu does not close
on an outside click) → the Dependabot triage.

## Delivered version and open field proofs

**0.20.0 PUBLISHED 2026-09-08** (the audit release, lots 1–6; release
commit `baeefc2`, bare tag, Latest; the mac half run on the Air the
same day — its mounted-volume guard fired once, `hdiutil detach`,
rerun clean; `verify-release.ps1`: **everything passes — 11 assets,
4 keys, 4 channel signatures VERIFIED cryptographically, both dmgs
resolve whole**). **Field the same day, Chief Engineer verdict:
"release ok and autoupdate ok on all computers" — the first mac
AUTO-UPDATE (0.19.0 → 0.20.0, ADR 0036's due proof) is PROVEN, both
Windows channels proven again, and the Air runs 0.20.0 (which
supersedes the pending x64-install proof).** Still open:

- the Apple Silicon run proof of PLAN-APPLE-SILICON — **narrowed on
  2026-09-07**: the app RUNS on two Apple Silicon Macs (T2, T3, Gmail,
  24 feedback items sent from the app); still to confirm that they run
  the `_aarch64` binary and not `_x64` under Rosetta (Activity Monitor
  › Kind = "Apple", asked of both);
- the SAC net stays armed (a VISIBLE failure under a real Smart App
  Control refusal — not closable by us);
- D-50, the Microsoft refresh-token confirmation, due ≈ 2026-12-01.

**Beta wave 1** running since 2026-08-31 (five invitations out,
anonymous register T1–T5 with the Chief Engineer); a tester report goes
first, in `/field`. **First feedback wave landed 2026-09-06/07: 24
items from T2 and T3 (both Apple Silicon, Gmail), analyzed and entered
in the Chief Engineer's backlog (out of the repository, PLAN-BETA § 3
bis rule). One P0 open: T2's synchronization stalled at 72 % since the
morning of 2026-09-07, no arrival since 2026-08-25 — next `/field`, on
the tester's trace (`~/Library/Application Support/Wind/`), with T3 as
the same-architecture witness. Two S bugs share one component (the
product menu does not close on an outside click, seen on two screens).**
Follow-up with the silent ones (T4, T5) was due 2026-09-03.
S4 measure closed: ~13 unknown senders/day, peak 19, workstation 1.

## Where the audit's residues live

The lots' stated limits are recorded at their sources: the plans'
close banners, DEBT (D-64, D-65, D-53 under its dated exception, D-2
reopened with figures), and ADR 0041/0043 for the accepted bounds.
Nothing is carried loose in this file.

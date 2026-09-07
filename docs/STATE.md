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

## Active program — [the audit of 2026-09-06](PLAN-AUDIT-2026-09.md)

Six lots over [AUDIT-2026-09-06.md](AUDIT-2026-09-06.md).

- **Lots 1–2 DELIVERED and closed** (2026-09-06): commits `e95adb4`,
  `0374e43`; CI green (34038794672, 34053044227) on Windows and both
  mac triples; field-validated, zero KO.
  [Lot 2 plan](PLAN-AUDIT-2026-09-LOT2.md).
- **Lots 3–5 field-validated and committed locally, NOT pushed**:
  Lot 3 `9f4a9b1` ([plan](PLAN-AUDIT-2026-09-LOT3.md)), Lot 4
  `400de5e` ([plan](PLAN-AUDIT-2026-09-LOT4.md)), Lot 5 E13
  `2e096d6` / E14 `516180e` / E15 `46c1463` (evidence folder
  `92fa3b6`; [plan](PLAN-AUDIT-2026-09-LOT5.md)). **Lot 5 D0 AMENDED by the
  Chief Engineer on 2026-09-07: `/code-review ultra` refused the grouped
  diff on size (464 files, 44,983 lines — over its 8,000-line limit);
  the push was ordered without it. Do not rewrite these commits.**
  Grouped push under way; `/close` of the lots follows the green CI.
- **Lot 6 (E16) DELIVERED and field-validated 2026-09-07, committed
  locally as `01b9171`** ([plan](PLAN-AUDIT-2026-09-LOT6.md), STOP 1 D0–D4, STOP 2
  "OK" zero findings, gate GREEN 681 s): documentation, comments and
  reconciliation — the C01–C08 consolidation record is the plan's §1
  table; DEBT.md re-filed (entries under their true sections, D-7/D-11
  orphans resolved by record); STANDARD/PLAN/STATE brought current.
  Not `/close`d: waits on the grouped push and its green CI (D0).

**The grouped push landed on 2026-09-07/08**: the seam guard's first
CI run refused it — E14's restore read `__e2eNoRestart` unguarded in
`Settings.svelte`, compiled out the transport.js way (`dfd734c`,
RED→GREEN on the guard), then CI GREEN (34164333355) with every new
job proven, `quality-windows-arm64` included. Nine of the ten first
Dependabot PRs failed on that same seam job (rebase them); the majors
(vite 8, eslint 10, rusqlite 0.40, …) fail on their own merits and
are each a dedicated decision — merge none casually.

**Next, in order**: `/close` of Lots 3 and 4 (DEBT: strike D-4,
D-41–D-44; record the stated limits and the `redesign-feedback-3`
in-suite flake) → `/close` of Lot 5, then Lot 6 → the `/field` on
T2's P0 (sync stalled at 72 %).

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

## Stated limits carried by the unclosed lots

Recorded in their plans, to land in DEBT at `/close`: two body-pump
drivers and the renamed-Google-address case (Lot 3); the release
chain's full field proof is the next actual release, dist digests are
compared and said, `InvalidEmailAddress` keeps the address for the
compose surface, the `redesign-feedback-3` reply scenario is a
recurring in-suite flake — 12/12 in isolation (Lot 4); the
`quality-windows-arm64` job is unproven until the grouped push, D-53
(Feed memory) is a dated Chief-Engineer exception with nothing built,
D-2 reopened with figures after its eviction measured worse (Lot 5).

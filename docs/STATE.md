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

Six lots over [AUDIT-2026-09-06.md](archives/AUDIT-2026-09-06.md), all
delivered, field-validated, pushed, CI green, **shipped in 0.20.0**
and `/close`d ([program plan](archives/PLAN-AUDIT-2026-09.md)):

- Lots 1–2 (2026-09-06): `e95adb4`, `0374e43`; their own green CIs.
- Lot 3 ([plan](archives/PLAN-AUDIT-2026-09-LOT3.md), `9f4a9b1`, field
  "1-11 OK"), Lot 4 ([plan](archives/PLAN-AUDIT-2026-09-LOT4.md), `400de5e`),
  Lot 5 ([plan](archives/PLAN-AUDIT-2026-09-LOT5.md), `2e096d6`/`516180e`/
  `46c1463`), Lot 6 ([plan](archives/PLAN-AUDIT-2026-09-LOT6.md), `01b9171`) —
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

## PLAN-SWEEP-2026-09 — CLOSED 2026-09-08

One sweep ([plan](archives/PLAN-SWEEP-2026-09.md)), field 7/7 OK the same day:

- **Dependabot cleared (10/10)**: #14, #7, #8, #12, #10, #15 merged
  after triage (the shared red was the pre-`dfd734c` stale base; #12's
  macOS red a fixture flake, fixed — `8538990`); the four majors
  MIGRATED on main (`63de0dd`): fs4 1.1 (the instance lock is std's
  `File::try_lock` now), rusqlite 0.40 (clamped u64↔i64 boundary,
  16 sites), vite 8 + plugin-svelte 7 (coupled pair, unmergeable one
  by one) — their PRs closed as superseded.
- **Seven beta bugs fixed** (`46ab864`, A137): the menu toggle + flip
  (91/105 — the review found the real macOS root: the trigger came
  from `document.activeElement`, so the outside-click closer NEVER
  fired on WKWebView; it is an `anchor` prop now), the Screener
  verdict never truncated (103), Copy-the-address on the sender
  (110, glyph `content_copy`), Ctrl+F as the universal find + the
  Cleanup group filter (98), the Feed 2 s read-dwell (102), search
  newest-first (111 — BM25 stays for a future sort option).
- Debts opened: D-66 (menu toggle not centralized), D-67 (search
  input drawn twice). Backlog observation: an arrival in the same
  second as Organized-mode activation never waits at the Screener.

**Next, in order**: the `/field` on beta T2's P0 (sync stalled at
72 %, on the tester's trace, T3 as the same-architecture witness) →
the remaining backlog bugs needing other machines (86 OAuth first-run,
100 invitation status, 109 macOS shortcuts — coded here, verified on
the Air) → the Rosetta-or-native check on T2/T3.

## PLAN-BATCH-2026-09 — CLOSED 2026-09-08

One batch ([plan](PLAN-BATCH-2026-09.md), eight Chief-Engineer
decisions D1–D8):
the docs root cleaned (eleven closed plans + two audit reports
archived, HANDOVER.md removed and **D-24 struck**, PLAN.md §9 points
at the living queue here), the **"What's new" window** (a modal on the
first launch after an update, the new version's changelog section,
bilingual per D5 — `CHANGELOG.fr.md` is born, §2.9 now writes BOTH
entries per release, with a test-enforced net), and **pictures on the
feedback form** (jpg/png, three max, journaled with the send in one
transaction). Review: 8 findings fixed — the sharpest: an absent
`whats_new_seen` pref on a database WITH accounts is the pre-feature
fleet updating in, and SHOWS the debut window. **STOP 2 "All ok" and
CI GREEN (run 34229853815) the same day — closed**; commit `6aa2a49`.
**Shipped in 0.21.0 the same day**, both changelog entries written at
release prep (§2.9's first bilingual release), the debut window
proven on every computer at the update.

## Delivered version and open field proofs

**0.21.0 PUBLISHED 2026-09-08** (the batch + sweep release; release
commit `85c34b0`, published from `3d57bec`, bare tag, Latest; both
halves in one draft — 13 assets with the two attestations — promoted
by `publish-release.ps1` after its four-signature proof). **Field the
same day, Chief-Engineer verdict: "All ok, update ok on all
computers" — which is also the fleet-wide proof of the "What's new"
debut window (every pre-feature install showed the 0.21.0 notes on
first launch, by design).** Release-day finding, fixed in the line:
the tauri CLI rewrites `apps/desktop/Cargo.toml` with LF endings and
`git status --porcelain` flags the EOL-only difference — both
make-release dirty-guards now compare CONTENT (`git diff --quiet` +
`--cached` + untracked), the release-net fixtures model it. One
transient network failure at the publication push (curl 28), resumed
cleanly. Previous: **0.20.0 PUBLISHED 2026-09-08** (the audit release, lots 1–6; release
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

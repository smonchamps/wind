# PLAN-SWEEP-2026-09 — Dependabot triage + the reproducible bug wave

Status: **GO (Chief Engineer, 2026-09-08) — in progress**
Statement (Chief Engineer, 2026-09-08): "Implement all the pull requests from Github
and fix all the bugs listed in the Wind backlog on Notion."

## 1. Finding (measured 2026-09-08)

### GitHub — 10 open PRs, all Dependabot (2026-09-07), no human PRs

There is nothing to "implement": every open PR is a dependency bump.
This is the "Dependabot triage" already named as a next step in STATE.

| PR | Bump | CI verdict |
|---|---|---|
| #14 | base64 0.22.1 → 0.23.1 | **all green** |
| #7 | @playwright/test 1.61.1 → 1.62.1 | **all green** |
| #8 | svelte 5.56.8 → 5.57.0 (minor group) | red: `build ui-v2` only |
| #10 | eslint 9 → **10** (major) | red: `build ui-v2` only |
| #15 | rfd 0.16 → **0.17** (Rust) | red: `build ui-v2` only — a Rust bump failing a JS check is suspicious; log not yet read |
| #12 | cargo minor+patch group (12 crates) | red: one macOS x86_64 leg only |
| #9 | @sveltejs/vite-plugin-svelte 6 → **7** | red: all Rust legs + ui-v2 |
| #11 | vite 7 → **8** | red: all Rust legs + ui-v2 |
| #13 | fs4 0.13 → **1.1** | red: all Rust legs |
| #16 | rusqlite 0.37 → **0.40** | red: all Rust legs + ui-v2 |

Fact not yet explained: #8/#10/#15 share the same single red
(`build ui-v2`); #12's single macOS red may be flake (e2e/local flake
lesson does not apply — this is clippy+tests). Logs to be read at E1.

### Notion "Backlog Wind" — 40 open items, of which 11 are Bugs

Bugs, grouped by where they can be reproduced (nothing verified yet —
each keeps its own Phase 0 before any fix):

**A. Reproducible on this workstation (expected):**
- ID 91 — [Spring cleaning] "…" menu does not close on outside click (S)
- ID 105 — [Settings – Portier] "Modifier" menu does not close on outside click (S)
- ID 103 — [Settings – Portier] verdict masked when sender address too long (S)
- ID 110 — [Reading] cannot copy an email address from the reading pane (S)
- ID 111 — [Search] name search returns oldest-first (S)
- ID 98 — [Spring cleaning] Ctrl+F does not work (M)
- ID 102 — [Kiosque] email marked read on open / "show images" (repro to confirm)

**B. Needs another machine or account (cannot verify here, §7.1):**
- ID 106 — **P0** [Sync macOS] INBOX stalled at 72 % (Mona, L) — the
  standing /field track; needs instrumentation + the tester's traces.
- ID 86 — Gmail OAuth first-run says authorizations missing (needs a fresh account)
- ID 100 — invitation status set from another client not reflected (needs a second client + calendar)
- ID 109 — macOS shortcuts (no Suppr key; ⌘ vs Ctrl) (S — code here, field on the Air)

The other 29 open items are Features/Improvements/Ideas — out of a
"fix all the bugs" statement.

## 2. Scope refusals (§2.6)

- **Not** implementing features/improvements from the backlog: the
  statement says bugs; each feature is its own /job with its own plan.
- **Not** fixing ID 106 inside this sweep: it is L, P0, and lives on
  the tester's machine — it deserves its own /field with
  instrumentation, not a slot in a wave.
- **Not** merging the four major bumps blind (#9, #11, #13, #16): each
  is a migration (vite 8, plugin-svelte 7, fs4 1.x, rusqlite 0.40) with
  real API changes; each is a small job with its own gate, or is closed
  until wanted.

## Progress (2026-09-08)

- **E1 delivered.** Merged after triage: #14, #7 (local full gates,
  267/268 e2e green), #8, #10, #15 (rebase cured the stale-base seam
  red; full CI green), #12 (macOS red = fixture flake, rerun green +
  local full gate). The flake: `mail-auth
  failed_browser_open_keeps_the_same_callback_alive`, 1 s fixture
  deadline on a loaded runner, seen twice (PRs #12, #15) — fixed in
  this sweep (10 s / 5 s). Majors #9/#11/#13/#16 rebased; still red on
  the Rust/ui legs — the migrations remain (D2: in scope).
- **E2 wave A delivered (code + tests).** Grouped RED measured
  (103: verdict box outside the row; 102: premature mark; 111: Rust
  test RED on the old order; 91/105: toggle RED), grouped GREEN 15/15
  e2e + mail-core 589 + mail-auth 42. Findings along the way:
  - 91/105's real root was TWO defects: no toggle on the trigger
    (five screens pasted the same opener without it) and the
    window-bottom clamp sliding a tall menu OVER its trigger
    (Settings) — flip above the trigger now.
  - A same-second edge observed (not fixed): an arrival within the
    activation second of Organized mode is treated as pre-mode mail
    (`date <= epoch`, store.rs) and never waits at the Screener.
- A137 in the System journal; new glyph `content_copy` (92).
- **Fresh-eyes review (2026-09-08, /code-review high, 10 findings, all
  confirmed fixed the same session):** the big one — `Menu.svelte`
  derived its trigger from `document.activeElement`, so on macOS
  (clicks do not focus buttons) the outside-click closer NEVER fired:
  very likely the tester's actual field defect. The trigger is now an
  explicit `anchor` prop passed by all seven openers; the flip checks
  it fits (else the old bottom clamp); re-anchoring an open menu
  replays bounding/flip/focus. Thread: address menu keyed by message
  (a thread repeats its sender), keydown swallow narrowed to
  Enter/Space, menu closed on thread change, system focus ring
  restored. App: Ctrl+F handled before the modal bail, visible
  declarers only (Settings › Screener now declares one). Cleanup:
  filter reset at session boundaries, open ⋯ closed on filtering.
  Feed: dwell seam accepts 0, hidden window cancels dwells (alt-tab is
  not reading). `organized-mode.spec.js` migrated to the dwell
  contract. Stale search docs/bench labels updated.
- **Debts recorded (for DEBT.md at close):** the second-click toggle
  lives in seven openers, not in Menu (the deeper fix — Menu owning
  toggle + outside-click via the anchor, parents dropping
  stopPropagation — touches nine consumers; deferred); the 32 px
  search-input drawing is duplicated (Cleanup + Settings, no shared
  class); List/Feed/PaperTrail/Screener toggle copies have no e2e;
  Feed retries a stale-version mark every dwell while in view.
- **Observation (not fixed):** an arrival within the same second as
  Organized-mode activation never waits at the Screener
  (`date <= epoch`, store.rs:1600).

## 3. Steps

- **E1 — Dependabot triage.** Read the red logs. Merge the greens
  (#14, #7) after a local full gate on each merge. Diagnose the shared
  `build ui-v2` red on #8/#10/#15 and the #12 macOS leg; merge what
  turns green, report verdicts on the rest. Gate: green CI on main
  after each merge.
- **E2 — Bug wave A (local repro).** For each of 91, 105, 103, 110,
  111, 98, 102: reproduce (RED e2e shown), fix, System journal A-n in
  the same commit. Grouped inner-loop runs per wave; full gate at the
  end of the wave. Early visual STOP for anything that changes a
  rendered surface.
- **E3 — Field checklist** (STOP 2) for wave A + the macOS items to
  route (109 code + Air verification; 86/100 repro protocol with the Chief Engineer).
- **E4 — Documentation & close.** Backlog statuses updated in Notion,
  STATE amended, memory updated.

## 4. Chief Engineer decisions

- **D1 — Dependabot greens.** Merge #14 and #7 after a local gate?
  → **Chief Engineer, 2026-09-08: "Yes, merge + triage"** — merge #14 and #7 after
  a local gate each; read the red logs of #8/#10/#15/#12 and merge
  what turns green.
- **D2 — Dependabot majors (#9 vite 8, #11 plugin-svelte 7, #13 fs4,
  #16 rusqlite).**
  → **Chief Engineer, 2026-09-08: "Migrate now in this sweep"** — the four
  migrations are in scope of this job (scope refusal #3 above is
  overridden by this decision).
- **D3 — Bug wave scope.**
  → **Chief Engineer, 2026-09-08: "Yes, wave A as listed"** — 91, 105, 103, 110,
  111, 98, 102. 109 stays out (needs the Air for the field pass).
- **D4 — ID 106 (P0 sync).**
  → **Chief Engineer, 2026-09-08: "Yes, dedicated /field"** — stays the standing
  P0 track with instrumentation + Mona's traces, outside this sweep.

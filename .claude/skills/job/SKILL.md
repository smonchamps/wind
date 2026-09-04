---
name: job
description: Run a Wind job end to end from a statement "Bug: …" or "Feature: …" — investigation on the evidence, set-based design, plan with CE decisions, two manual validations, TDD implementation, full gate, field, documentation, commit and green CI.
---

# /job — the standard workflow of a Wind job

The argument is the statement: `Bug: …` or `Feature: …`. The method of
STANDARD §2 applies in full; this skill is its operating sequence.
**Two stops are mandatory** — the plan validation and the field
validation. Nothing bypasses them.

## Phase 0 — Investigation (genchi genbutsu first)

- **Bug**: reproduce and **measure** before any hypothesis. If
  reproducing requires the CE's machine or accounts (reminder §7.1: you
  cannot read their database), ask for the measurement and wait. A
  symptom is not a cause: trace it to the root (model: A38, commits
  9ebd7b2 → 5698641 — the belt first, the root next).
- **Feature**: read the System (`docs/design/system.dc.html`, the only
  normative document — A18), the ADRs concerned (frozen decisions,
  STANDARD §5), the state of the code. On a wide area, the built-in
  `Explore`/`Plan` agents do the reconnaissance — **on Sonnet 5** (Haiku
  for pure sweeping); the top tier is reserved for design, verification
  and reviews (WORKFLOW, axis M).

## Phase 1 — Design

1. Written **finding**: the facts, the figures, what is proven.
2. **Set-based if there is a hard point**: several options, decided on
   measurements — throw-away spikes in `spikes/`, through the `spike`
   agent (one agent per option, in an isolated worktree). Model: ADR
   0004. The alternative must beat the hypothesis *clearly* to unseat it.
3. **Prototype if UI**: a study mockup (Claude Design project,
   `.dc.html`), never normative — the DC-D4 spirit: its substance is
   poured into the System, the study file does not enter the repository.
4. **Explicit scope refusals**: what we do not do, and why (§2.6).
   Saying no is the default behavior.
5. **Write `docs/PLAN-XXX.md`** in the canonical form of the
   repository's plans: finding, scope, options and measured verdicts,
   steps E1-En with their gate, and a **§ CE decisions** listing every
   call that belongs to the Chief Engineer (numbered D1, D2, …).

## ⛔ STOP 1 — CE validation of the plan

Present the plan, then ask the decisions of the § CE decisions **one by
one** (AskUserQuestion). Record the answers in the PLAN, word for word,
with the date. **No production code before the GO.**

## Phase 2 — Implementation

- Step by step, in the order of the plan.
- **Strict TDD**: the test fails (RED, shown) before the implementation
  (GREEN). If a RED can teach nothing (trivial pure function), say so —
  never fake it.
- **Targeted inner loop** (§2.4): during implementation, play only the
  impacted spec(s), **as whole files** (never `-g` on an e2e), and group
  the runs by wave — one grouped RED for the wave, one grouped GREEN.
  The full gate is played only at the moments stated in Phase 3 — not
  at every increment.
- **DC-D2**: every commit that touches the UI amends
  `docs/design/system.dc.html` in the **same commit** (journal A-n).
- **Early visual STOP (UI)**: as soon as the first minimal TDD
  increment renders something, ask the CE for a verdict on the look —
  before rolling out the rest. A whole UI job was cancelled in the field
  for lack of this stop (sort bar, A58).
- **Early measured STOP (perf)**: before/after measurement from the
  first increment, CE call on the figure — never roll out a whole perf
  job without an intermediate measured STOP (~1 M tokens thrown away on
  perf-lecture without it).
- Zero `unwrap()`/`expect()` in production; `thiserror` in the crates,
  `anyhow` in the apps. Pure, testable decision, I/O elsewhere (STANDARD
  §4 pattern).

## Phase 3 — Quality

1. **Fresh-eyes review**: `/code-review high` on the diff, once the
   implementation is complete, before the final commit. Fix what is
   confirmed.
2. **Full gate**: `/gate`. A red = andon — we stop, we fix, then a
   **partial re-gate** (the red step + what the fix can impact, upstream
   included if Rust); the final full gate before the commit is still
   due. Never `--no-verify` without an explicit CE decision.

## ⛔ STOP 2 — field validation

Hand the CE a **field checklist**: what to look at, gestures to play,
expected figures, budgets to re-measure (STANDARD §3 — a budget exceeded
stops the line). Provide **systematically, at that moment, the
PowerShell commands needed to run the field test** (launching the app,
build, preparing the accounts, measurements) — ready to copy, one per
block. The committed scripts come first — never a regenerated one-liner
for what they cover: `scripts\field.ps1` (state of the workstation:
database, installed version, OAuth credentials, traces) and
`scripts\run-wind.ps1` (release launch WITH trace — it encodes the §9
trap: the bare exe traces nothing). Wait for the verdict. A field
finding → fix **the same day**, in the same session, then re-gate and a
new field pass.

## Phase 4 — Documentation

- System journal: an A-n for every notable fact (DC-D2).
- PLAN-XXX updated (delivered steps, commits, verdicts).
- **ADR** if a structuring decision (`docs/adr/`, short, model 0004).
- STATE amended (the state of the project, budgets re-measured);
  STANDARD amended if a new trap or lesson (§7, §9).
- Persistent memory updated (state of the job, absolute dates).

## Phase 5 — Commit, push, CI

- Message: `type: description`, in English, body carrying the figures
  and the reasoning, **never a Co-Authored-By** (§2.8).
- Push and CI watch **in the background, never in the foreground**: the
  push (the pre-push hook replays the gate, ~3 min) and `gh run watch`
  run as background tasks; the session goes on and **announces the CI
  verdict when it lands**. Local e2e can flake, the CI is the reference.
  The job is closed only on a green CI; then `/close`.

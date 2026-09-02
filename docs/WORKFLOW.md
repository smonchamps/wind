# User guide — Wind's standardized workflow

> Installed on 2026-08-15 (commit `961aab7`, CE decisions D1/D2/D3).
> This document explains **how to use it**; the method itself lives in
> [STANDARD.md](STANDARD.md) §2 and takes precedence over everything.
> The skills live in `.claude/skills/`, versioned in the repository:
> amending them is a commit like any other.

## Overview

One command carries the full flow; three others serve the moments that
come back. The user is the Chief Engineer (*shusa*): the workflow stops
dead at the two places where the decision is theirs.

```
/job Bug: …  or  Feature: …
   │
   ├─ Phase 0  Investigation — reproduce, measure, read (never assume)
   ├─ Phase 1  Design — finding, figured set-based (spike agent),
   │           prototype if UI, PLAN-XXX.md with a § CE decisions
   │
   ├─ ⛔ STOP 1  The CE validates the plan and makes the decisions, one by one
   │
   ├─ Phase 2  TDD implementation, step by step (DC-D2 in the same commit)
   ├─ Phase 3  /code-review high (once), then /gate — one red = andon
   │
   ├─ ⛔ STOP 2  The CE validates in the field, on a figured checklist
   │             (a finding → fix the same day, re-gate, re-field)
   │
   ├─ Phase 4  Documentation — journal A-n, PLAN, ADR, ETAT, memory
   └─ Phase 5  Commit → push + CI watch in the background
               → CI verdict announced by the session
```

## Which command for which situation

| Situation | Command | Example |
|---|---|---|
| A defect to investigate or a feature to deliver | `/job` | `/job Bug: 5 s freeze at startup from the command line` |
| A finding made just now in the field, narrow scope | `/field` | `/field the accent strokes reappear after a click` |
| Check the state before a commit, or after a fix | `/gate` | `/gate` |
| A finished job, field validated, green CI | `/close` | `/close PLAN-SPAM` |

`/field` is the fast lane of the genchi genbutsu loop — but if the root
turns out to be deep or the scope widens, the session switches to
`/job` by itself: speed does not exempt from design.

## What the workflow expects from the Chief Engineer

The CE has only **four gestures**; everything else is carried by the
session.

1. **Launch**: one sentence — `Bug: …` or `Feature: …`. No need to
   recall the method, TDD, the gates: they are in the standard.
2. **⛔ STOP 1 — arbitrate the plan.** The session presents
   `PLAN-XXX.md` and asks the decisions of the § CE decisions one by
   one. Answering is all: the answers are recorded in the PLAN, word for
   word, dated. No code exists before this GO.
3. **⛔ STOP 2 — validate in the field.** The session hands over a
   checklist: gestures to play on the real accounts, expected figures,
   budgets to re-measure. It provides **systematically, at that moment,
   the PowerShell commands needed to run the field test** (launching the
   app, build, preparing the accounts, measurements) — ready to copy,
   one per block. Say what is seen — a finding triggers the fix the same
   day, in the same session.
4. **Provide the measurements the session cannot take**: reminder
   STANDARD §7.1, it reads neither the real database nor the banner.
   When Phase 0 needs a field figure, it asks for it and waits.

## The built-in guarantees (no longer need prompting)

Every skill carries the rules paid for over the project:

- **Strict TDD** — RED shown before GREEN; a RED that teaches nothing
  is said, never faked.
- **DC-D2** — every UI commit amends `docs/design/systeme.dc.html` in
  the same commit (journal A-n).
- **Full gate, never the tests alone** — the thirteen steps of
  [/gate](../.claude/skills/gate/SKILL.md), `coherence-systeme`
  included, played in one call by `scripts/gate.ps1`; fmt replayed
  after any mechanical replacement.
- **E2E flaky locally** — a local red is cross-checked (`gh run list`)
  before suspecting a regression: the CI is the reference.
- **Commits** — `type: description`, in English, body carrying figures
  and reasoning, never a Co-Authored-By.
- **Green CI mandatory** — the job is closed only after `gh run watch`
  is green on the pushed commit; push and CI watch happen **in the
  background**, the session announces the verdict (never a wait in the
  foreground — kaizen 2026-08-23, ~3.5 h of blocked wall measured over
  12 days).

## Model policy (kaizen 2026-08-23, CE-validated — axis M)

The rule fits in two lines; it also preserves the Fable quota for what
needs it.

- **Job = Fable 5, invariant.** Design, tracing to the root, TDD,
  review: never hard design on a lesser model (perf-lecture precedent,
  not proven but suspect).
- **Mechanical session = Sonnet 5.** Docs/ETAT/CHANGELOG, Notion, CI
  watch, scripted release, memory consolidation: the CE opens these
  sessions on Sonnet 5 (the app's model selector). Measured baseline:
  the mechanical work ran 100 % at the top rate (M1, target ≤ 5 % of the
  top-tier cost outside jobs).

## The `spike` agent — set-based exploration

When the design meets a hard point, the decision is made on figures
(STANDARD §2.2-2.3): **one `spike` agent per option**, in an isolated
worktree, each building a throw-away prototype in `spikes/` and
reporting a protocol and measurements — never an opinion. The main
session compares the reports, the CE decides. Model:
[ADR 0004](adr/0004-moteur-de-recherche-fts5.md).

It is the **only** custom agent, on purpose: splitting design,
implementation or documentation into separate agents would lose, at
every hand-off, the context that makes the quality of the commits. One
thread carries the finding all the way to the green CI.

### Model of the agents (kaizen 2026-08-23 — axis M)

Measured baseline: 100 % of the subagents ran top-tier, including pure
sweeping. From now on, when launching an agent (`model` parameter of the
Agent tool):

- **Exploration / reconnaissance** (`Explore`, `Plan`, code search):
  **Sonnet 5**; **Haiku** for pure sweeping (locating files, counting
  occurrences).
- **Verification, review, `spike`**: unchanged — top tier or the
  session's model; they are the best defect detectors of the workflow
  (a review caught an FTS5 index rebuild of ~13 GB), we do not touch
  them.

## Amending the workflow

The standard is not frozen — it is *standard work*: it improves by
kaizen, on facts. A skill that chafes in use is amended by an ordinary
commit (`chore:`), with the finding that motivates the change in the
message body. This document is amended in the same commit as the skill
it describes.

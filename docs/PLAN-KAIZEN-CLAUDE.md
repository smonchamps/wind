# PLAN-KAIZEN-CLAUDE — optimizing the use of Claude Code on Wind

> Kaizen job opened on 2026-08-23, on the audit of the 46 sessions from
> August 11 to 23 (deterministic transcript extraction + multi-agent
> analysis with adversarial verification on repository evidence; 7
> recommendations out of 44 rejected at verification). Object: lower
> the token cost, the time to process prompts and the execution time of
> the /job→/gate→/close workflow, **without touching the quality
> level** — the full gate before commit, green CI, TDD shown and the
> field STOP 2 are invariants, not adjustment variables.

---

## Finding — measured baseline (2026-08-11 → 2026-08-23, 12 days)

### Volumes

| Measurement | Value |
|---|---|
| Sessions / CE prompts / assistant turns | 46 / 478 / 17,479 (**36.6 turns per prompt**) |
| Total cost (input equivalents: cacheRead ×0.1, cacheCreate ×1.25, output ×5) | **876 M** — cacheRead 68%, cacheCreate 18%, output 14% |
| Raw cacheRead | 5.96 Bn tokens; **top 10 sessions = 62%** of the volume |
| Average context re-read per turn | marathons 410–540 k; short sessions 75–140 k |
| Sessions compacted / cleanly closed | 2 compactions out of 46; sessions of 90.4 h, 37.3 h, 26.3 h, 25.4 h of wall time |
| /job jobs over the period | 15 invocations, ~14 jobs closed → **~60 M input equiv. per job** |

### Time

| Measurement | Value |
|---|---|
| API latency per call | ~3 s median, **flat** from 106 k to 534 k of context (the cache pays off) |
| e2e launches | 243; among the 85 > 30 s: median 74 s, p90 159 s, max 217 s |
| `git push` (pre-push hook replays the gate) | 42 > 30 s, median 118 s, max 164 s — in the foreground |
| `gh run watch` / CI watching | 28 > 30 s, median 141 s; ~20 min of wall time blocked per dense day |
| Full gates per job | up to 10+; each gate = 9 sequential tool calls (8 orchestration turns lost) |
| Rebuild per e2e launch | `construireV2` bumps the mtime of `main.rs` → recompiles + links on every launch, even without a change |

### Agents and models

| Measurement | Value |
|---|---|
| Agent launches (Agent tool) | 85 over the period; 141 subagent transcripts, 5,301 API calls |
| Agent cost | **92 M input equiv., i.e. ~9.5% of the total** (968 M main thread + agents) — secondary vein, volume already sound (one-thread-only + spike doctrine) |
| Subagent model | **100% top-tier** (Opus 5: 3,098 calls, Fable 5: 2,100) — including exploration agents |
| Main-thread model | Fable 5: 72% of messages, Opus 4.8: 28%, Sonnet 5: ~0% — mechanical work (docs, releases, Notion, CI watching) runs at the top rate |
| Agent waste identified | timing, not volume: 2 high-effort reviews (~8 agents each) paid for on designs later invalidated by measurement |

### Recurring losses identified (with evidence, cf. audit)

- Multi-job sessions never closed: the context of a closed job is
  billed again on every turn of the next one.
- 9 empty `/job` launches → a wasted round trip each.
- ~15 re-requests of the STOP 2 PowerShell commands, after they were
  codified.
- 11 re-runs of the full suite to settle ONE flake (the
  spec-in-isolation rule already existed).
- 2 high-effort reviews paid for on designs later invalidated by
  measurement (research job); 1 whole UI job cancelled in the field
  (sort bar); ~1 M tokens thrown away on read-perf without an
  intermediate measured STOP.
- Cascading PowerShell 5.1 friction (one-liners regenerated instead of
  versioned scripts).

---

## Numeric goals — horizon: review on 2026-09-06 (2 weeks)

Three axes, nine indicators. The baseline is the Aug 11–23 window; the
control measurement is the Aug 24 – Sep 6 window, normalized per closed
job to cancel out activity variation.

### Axis T — tokens

| Indicator | Baseline | Target | Main lever |
|---|---|---|---|
| T1. Input equiv. **per increment** (E-step; D5 of 2026-09-09 — see § Per-increment attribution) | ~60 M per job | **≤ 35 M (−40%)** per increment, **E0 counted apart** | T2+T3+T4 combined |
| T2. Average context re-read per turn (any session) | 410–540 k (marathons) | **≤ 200 k** | /close = session boundary; /compact at STOPs |
| T3. Sessions closed or compacted ≤ 24 h of wall time; multi-job sessions | 8+ marathons; multi-job common | **100%; zero** | final step of /close |
| T4. Assistant turns per Chief Engineer prompt — **driven sessions only**, a session at 0 prompts is excluded (D6 of 2026-09-09) | 36.6 | **≤ 25 (−30%)** | scripted gate (−8 turns/gate), grouped waves |
| T5. ~~Output tokens per session, at comparable activity~~ — **WITHDRAWN as an indicator (D8, 2026-09-09)**: inconclusive, and the instrument cannot resolve what it was asked | week-1 ref. | — | the `Concise` output style stays or goes as a plain preference, no longer measured |

### Axis P — prompt processing time, constant quality

| Indicator | Baseline | Target | Main lever |
|---|---|---|---|
| P1. Wall time blocked in the foreground on commands > 60 s | ~3.5 h / 12 d (push, watch, e2e) | **≤ 15 min / 2 wks** | systematic background (Monitor), Claude announces the CI verdict |
| P2. Re-runs to settle an e2e flake | up to 11 | **≤ 2** (whole spec in isolation, once) | /gate compliance reminder; retries:1 |
| P3. Avoidable round trips (empty /job, STOP 2 re-request) | 9 + ~15 | **0** | stated in the argument; non-compliance flagged |
| Quality guardrail (must NOT degrade) | — | KO findings at STOP 2 per job and red CI: **stable or falling** | invariants unchanged |

### Axis W — workflow execution time (121 e2e)

| Indicator | Baseline | Target | Main lever |
|---|---|---|---|
| W1. Full gate (wall time, timed) | **4 min 34 s** (W0 measurement of 2026-08-23, warm cargo cache, 121/121 e2e — the 9–12 min estimate was pessimistic) | **≤ 6 min** (already held at W0 — tightening the target is a CE call at the review) | memoized rebuild, gate.ps1, nextest (if measured as a win) |
| W2. Inner loop: 1 e2e spec | med. 74 s (dominated by rebuild) | **≤ 45 s** | memoized rebuild + conditional bump |
| W3. Full gates **per increment** (same denominator as T1, D5) | 10+ per job | **≤ 3** | codified targeted loop + partial re-gate |
| W4. Cumulative gate time per job (**per job**, unchanged by D5) | ~100 min | **≤ 25 min** | W1×W3 |
| W5. Docs-only push | ~2 min (whole gate) | **≤ 30 s** | pre-push fast path |

### Axis M — models and agents (CE-validated on 2026-08-23)

| Indicator | Baseline | Target | Main lever |
|---|---|---|---|
| M1. Share of cost on top-tier model **outside jobs** (mechanical sessions + exploration agents) | ~10–15% of the total | **≤ 5%** | "mechanical session = Sonnet 5" rule; exploration agents downgraded |
| M2. High-effort reviews per job | up to 3 (2 on discarded designs) | **1, at convergence** | already carried by wave 1 (T1) |
| M3. Agent share of an increment's cost (**reported, not capped** — D9 of 2026-09-09) | 9.5% at the baseline, **35.3%** in the 29/08–06/09 window | no target: **beyond ~40% of an increment, its plan entry says what the agents found** — a finding, or nothing | the per-increment attribution of D5 already separates main thread from subagents |
| Guardrail | — | jobs (design, root-cause, TDD) stay on Fable 5 — never hard design work on a lesser model (read-perf precedent, unproven but suspect) | — |

Expected gain from axis M: **−10 to −15% of total cost**, cumulative
with axis T, without touching job quality. The *number* of agents is
not a lever: 9.5% of the cost, and set-based spikes as much as
multi-angle reviews are the best defect detectors in the workflow (a
review is what caught the ~13 GB FTS5 index rebuild).

---

## Countermeasures — three waves

### Wave 0 — reference measurement (before any change, ½ day)

1. Commit `scripts/measure-sessions.mjs` (adapted from the audit
   script: tokens, turns, average context, commands > 30 s per
   category, per session) — you only steer what you measure.
2. Time ONE reference full gate, warm cargo cache (STANDARD §9: the
   warm cache lies, note the cache state) → fixes W1.

   **✓ Done on 2026-08-23** (warm cargo cache, no code change in
   flight): **total 274.3 s (4 min 34 s)** — fmt 0.7 s, build-ui 2.8 s,
   contrast 0.3 s, coherence 0.3 s, main-thread guard 0.3 s, clippy
   2.9 s, Rust tests 9.2 s (--all-targets), doc 1.3 s, **e2e 256.3 s
   (121/121)**. The e2e is 93% of the wall time: the dominant lever is
   indeed wave 2 (memoized rebuild, template seed base) — not gate.ps1
   (the first 8 steps weigh only 18 s, the gain of gate.ps1 remains
   the tool-turn orchestration, −8 turns/gate).

### Wave 1 — behaviors and skills, zero production code (day 1, one `chore:` commit per amendment)

| # | Countermeasure | File(s) | Indicators served |
|---|---|---|---|
| 1 | `/close`: last step "write the CHANGELOG entry (if a release is coming), then **close this session**; the next subject opens on STATE.md" | `.claude/skills/close/SKILL.md` | T1 T2 T3 |
| 2 | `/job` and `/field`: targeted inner loop — impacted spec(s) **as a whole file** (never `-g`), 2 grouped runs per wave (grouped RED, grouped GREEN); full gate ONCE before commit | `job/SKILL.md`, `field/SKILL.md`, sentence in STANDARD §2.4 | W3 W4 T4 |
| 3 | `/gate`: partial re-gate after a fixed red (red step + whatever the fix can impact, upstream included if Rust); final full gate before commit unchanged | `gate/SKILL.md`, `job/SKILL.md` | W3 W4 |
| 4 | `/gate` and `/job` Phase 5: push + `gh run watch` **in the background**, verdict announced by the session; never wait on CI in the foreground. **Extended by D7 (2026-09-09) to every e2e run expected beyond ~60 s** — the full suite and a grouped multi-spec wave; a single targeted spec (13.5–19 s since wave 2) stays in the foreground | `gate/SKILL.md`, `job/SKILL.md` | P1 |
| 5 | `/job`: early visual STOP (UI: appearance verdict after the first minimal TDD increment); early measured STOP (perf: before/after measurement on the first increment, CE call) | `job/SKILL.md` | T1 P3 |
| 6 | CE discipline (no commit): full statement in the `/job` argument; evidence at the first statement; non-compliance flagged rather than re-requested; only one writing session at a time | — | P3 T4 |
| 7 | Model policy in WORKFLOW.md: **job = Fable 5** (invariant); **mechanical session** (docs/STATE/CHANGELOG, Notion, CI watching, scripted release, memory consolidation) **= Sonnet 5**; also preserves the Fable quota for jobs | `docs/WORKFLOW.md` | M1 |
| 8 | Exploration/research agents downgraded (Sonnet 5, Haiku for pure scanning); verification, review and `spike` agents unchanged (top-tier / session model) | `.claude/agents/`, WORKFLOW.md | M1 |

### Wave 2 — small technical jobs (week 1, in order of return)

| # | Countermeasure | Expected gain | File(s) |
|---|---|---|---|
| 1 | Memoize `construireV2` per suite process + bump `main.rs` conditioned on the hash of the dist **and** of tauri.conf.json | 3–8 min/suite; 25–40 s/spec; carries W1 and W2 | `e2e/rebuild-v2.mjs`, `e2e/launch.mjs` |
| 2 | `scripts/gate.ps1` fail-fast, 9 steps in hook order, **without** the `/dev/null` redirections (the numeric verdict must come out); `/gate` runs it in one call | −8 turns/gate; carries T4 | new script + `gate/SKILL.md` |
| 3 | `retries:1` in Playwright + every flaky logged in the gate verdict (inseparable: a flaky does not turn the run red); andon = plain red | 5–15 min/flake; carries P2 | `e2e/playwright.config.js`, `gate/SKILL.md` |
| 4 | Docs-only fast path in pre-push: skip steps 6–8 (clippy, Rust tests, e2e) if the diff ⊆ `docs/**` + `*.md`, **excluding `docs/design/**`** (DC-D6); keep the steps in seconds | W5 | `.githooks/pre-push` |
| 5 | `scripts/field.ps1` + `scripts/run-wind.ps1` compatible with PS 5.1 (CLIENT_ID, OneDrive-safe paths, UTF-8 traces written by the app) | removes the terminal-friction class | new scripts, referenced at STOP 2 |
| 6 | Template seed base copied per spec instead of ~14 `cargo run --example` per suite | 15–35 s/suite | `e2e/launch.mjs` |
| 7 | `cargo-nextest` on `--all-targets`: **measure before/after** (expected gain inter-binaries, ~20 binaries); adopt only if the figure justifies it; `--doc` unchanged | ~15–25 s/gate if confirmed | `gate.ps1`, pre-push, ci.yml |

#### Wave 2 rollout — **CLOSED on 2026-08-23, field complete** (order D3)

> Commits `ceb59c4` (the 7 countermeasures) + `a3ed285` (TTL freshness
> of the templates — a red paid for at the push gate, fixed within the
> session). GO in the field from the CE on 2026-08-23 (checklist 3/3:
> field.ps1, run-wind.ps1 with proven trace, spec 30.3 s). Green CI run
> 32642956082. **Kaizen figures for the job**: 6.8 M input equiv. (T1;
> baseline ~60 M/job), average context 181 k/turn (T2 ✓), 4 full gates
> played (W3 — including 1 pre-push red), 0 KO at STOP 2, 0 red CI
> (guardrail ✓).

| # | Verdict | Measurement |
|---|---|---|
| 1 | **✓ delivered** — `empreinteDist` (dist + conf, sha1) + conditioned bump + per-suite-process memo (`rebuild-v2.mjs`, 4 node tests) | **W2: 74 s → 13.5–19 s** of wall time per spec (`refonte-retours-7`, warm cache) |
| 2 | **✓ delivered** — `scripts/gate.ps1`, 9 fail-fast steps, numeric verdict per step, PS exceptions rendered as a named red; `/gate` calls it in one turn | −8 orchestration turns per gate (T4) |
| 3 | **✓ delivered** — `retries: 1` + "flaky = logged in the verdict, plain red = andon" written into the skill | carries P2 |
| 4 | **✓ delivered** — docs-only fast path for pre-push (⊆ `docs/**`+`*.md`, outside `docs/design/**`; new/removed ref ⇒ full gate; iteration by line, never by word) | W5 — to be timed at the first docs-only push |
| 5 | **✓ delivered** — `scripts/field.ps1` (workstation state: base, version, OAuth User **and** session, traces) + `scripts/run-wind.ps1` (build via `build-wind.mjs` — the single home of rebuild pitfalls — then `cargo run` which HOLDS the trace handle, §9); referenced at `/job`'s STOP 2 | field.ps1 proven on this workstation (0.7.0, base 11.83 GB) |
| 6 | **✓ delivered** — seed templates (key = seeder exe + recipe, **TTL 30 min** — the seeders freeze the clock at construction time: relative days AND `derniere_synchro` "2 min ago"; a day-level key produced a red at the push gate, fixed the same day), copied per spec, built off to the side + renamed | included in W2; ~14 `cargo run --example` → 1 build / 30 min |
| — | **W1 re-measured after E1+E2+E6**: full gate via `gate.ps1`, 121/121, zero flaky | **4 min 34 s (W0) → 1 min 43 s** (103 s; e2e 256 → 86 s), warm cargo cache at both measurements |
| 7 | **✗ rejected on the figure** — `cargo test --all-targets` measured at **9.3 s** warm cache: the expected gain (15-25 s) exceeds the whole item; nextest is neither installed nor adopted | — |

Fresh-eyes review, 8 angles (2026-08-23): 10 findings, 8 fixed before
the gate (stale dist of the field launcher, frozen template dates,
word-splitting and ref deletion in the hook, memoized zombies, WAL
sidecars of the template, false ABSENT OAuth, silent PS exception), 2
logged: **double-encoded gate** (pre-push sh + gate.ps1 — two homes,
to unify if they still diverge) and **root `build.rs` lead**
(`cargo:rerun-if-changed` on the dist would make the bump unnecessary
— to investigate outside this window, `tauri_build` behavior to be
proven first).

### Wave 3 — structural (to be planned, outside the measurement window)

1. Move the repository out of OneDrive (existing doctrine of
   `install-workstation.ps1`) — at a moment with no uncommitted work in
   flight; re-point Claude's memory (project key = path).
2. Self-hosted x64 runner for an e2e CI job — ADR 0005 plans this
   switch; trigger: closed-beta milestone. Takes the 121 tests off the
   local blocking path.

### Optional countermeasure T5 — `Concise` output style (to be worked out in a future session)

**✓ Enabled on the evening of 2026-08-28** (start of week 2), in
`~/.claude/settings.json` — the actual user-scope file,
`settings.local.json` existing only at the project level. Takes effect
on the next sessions.

Claude Code setting: `"outputStyle": "Concise"` (user scope; requires
Claude Code v2.1.237+; the `/output-style` command no longer exists,
go through `/config` or the desktop app's Settings > Claude Code).
Effect: shorter responses by default, less narration; the numeric
verdicts of skills, error reports and confirmations stay complete.

Trial protocol — output is only 14% of the cost, it is a top-up, not a
lever; it is therefore paid for in measurement, not conviction:

1. Window week 1: baseline without Concise (already under way).
2. Week 2: enable Concise, as close as possible to the same activity
   mix.
3. At the 2026-09-06 review: compare, via
   `scripts/measure-sessions.mjs`, output tokens per session at
   comparable activity, AND the quality guardrail (KO at STOP 2, red
   CI, CE re-requests for detail). Adopted if a clear drop with no
   degradation; otherwise withdrawn.

### Leads investigated and rejected (do not re-investigate)

sccache (degrades warm incremental builds); shared WebView2 window
between specs (shared state, STANDARD §7.1/7.5); gate delegated to
hosted CI (ADR 0005); settling an e2e flake via `gh run` (CI never
runs any e2e).

---

## Per-increment attribution (D5, 2026-09-09)

The unit of T1 and W3 is the **E-step**, anchored on its closing
commit — a mapping the plans already record as standing discipline
(Lot 5: "E13 committed locally as `2e096d6`", E14 → `516180e`,
E15 → `46c1463`). No new discipline is required of a job; only the
tooling changes.

For E-step *k* with closing commit *c(k)*:

- **interval(k)** = ( t(c(k−1)) , t(c(k)) ], where t() is the commit
  timestamp. The lower bound of the first E-step is the plan file's
  own creation commit.
- **T1(k)** = Σ `equiv(usage)` over every assistant turn — main
  thread **and** subagents — timestamped inside interval(k).
  `equiv` is unchanged: input + 0.1×cache_read + 1.25×cache_create
  + 5×output.
- **W3(k)** = number of `gate.ps1` invocations inside interval(k).

**E0 — counted apart, never averaged in.** Investigation, set-based
design, STOP 1 and the settling of the Chief Engineer's decisions
all happen before
E1's commit and would otherwise inflate E1, presenting design cost as
implementation cost. E0 = job opening → E1's commit, reported on its
own line. Without this the amended indicator is worse than the one it
replaces.

**Comparability.** A job with one E-step and one commit gives one
interval covering the whole job: the figure is bit-for-bit the pre-D5
one. Nothing already recorded needs re-reading.

**Known limits, stated rather than corrected:**

- **Sub-increments are not separable**: E14a/E14b/E14c all closed in
  `516180e`. The finest attributable grain is the E-step. Finer would
  mean a commit per sub-step — a change to the commit discipline,
  refused as not worth it.
- **Interleaved turns fall in the bucket**: a Notion pass or an
  unrelated question between two commits is billed to the increment.
  Reported as the bucket total plus the share coming from sessions
  that touched the plan, rather than filtered.
- **A red gate replayed after a commit** lands in the next bucket
  (W3 only).

**Tooling owed**: a `--by-commit <range>` mode in
`scripts/measure-sessions.mjs` — the per-turn timestamps and usage
are already parsed, subagent transcripts included, so this is
bucketing existing records against `git log --format=%H|%cI`. Due
before the next weekly measurement, which cannot be filled without
it. **D6 rides with it**: the aggregate turns/prompt line must skip
sessions at 0 prompts instead of averaging their turns onto the other
sessions' prompts. **D9 too**: the same pass already separates main
thread from subagents, so the agent share per increment (M3) is one
more column, not a second traversal.

## Measurement and review (PDCA)

- **At every /close**: note in the job's PLAN the 3 figures **per
  increment** — input equiv. (T1), full gates played (W3), KO findings
  at STOP 2 (quality guardrail) — plus the E0 line. A single-increment
  job yields one line, identical to the pre-D5 figure.
- **Weekly (Friday)**: re-run `scripts/measure-sessions.mjs` over the
  week, fill in the tracking table below. **Fired by a scheduled task
  since D10 (2026-09-09)** — `kaizen-weekly-review`, Friday 17:47
  local, held in `~/.claude/scheduled-tasks/`. It measures, writes the
  week's row, replays the two documentary gate steps and reports; it
  never commits. It runs on **this** workstation by necessity: the
  transcripts it reads live under `~/.claude/projects/` and no remote
  agent can see them (the machine-specific path already noted at D2).
- **Review on 2026-09-06**: indicator by indicator, met / missed /
  cause; countermeasures that did not produce their figure are amended
  or withdrawn (standard work: keep what is measured to work).

### Weekly measurement S1 — 2026-08-28 (window 24–28/08, 13 sessions)

`node scripts/measure-sessions.mjs --depuis 2026-08-24 --jusqua 2026-08-28`:
121 CE prompts, 5,430 turns, 283 M input equiv. main thread + 22.3 M
agents (7.3%). Jobs closed during the week with figures in the PLAN:
RETOURS-9 (11.4 M, 2 gates), RETOURS-10 (2 gates 2.1–2.2 min),
RETOURS-11 (29.1 M, 4 gates), ELEMENTS (29.6 M, 7 gates).

Reading the gaps:

- **What holds**: T1 (every job ≤ 30 M, −50% and more vs baseline), W1
  (gates 2.1–2.6 min with a suite grown from 121 to 148 e2e), M2 (1
  review per job), quality guardrail (field KO fixed the same day, 0
  red CI).
- **T3 missed (3 sessions > 24 h)**: including the kaizen session
  itself (81d387ca, 141 h of wall time — reopened at every rite instead
  of a fresh thread) and 6e998992 (24.2 h, 55.7 M). The "close at
  /close" rule is not yet a reflex for sessions outside a job.
- **T2 missed (364 k/turn)**: a direct consequence of T3 — sessions
  that run long carry 300–484 k of context.
- **T4 blurred (44.9)**: the indicator mixes agentic sessions (a02fb764:
  519 turns / 0 prompts) with CE-driven work; to be re-read at the
  review per driven session. RETOURS-9 (320 turns / 1 prompt) is on
  the contrary the intended mode: one full statement, zero follow-up.
- **P1 missed (100 min > 30 s in the foreground)**: dominated by e2e
  runs played in the foreground (54 min, including b8eb0fe7: 14 runs /
  38 min) — the "background beyond 60 s" instruction (wave 1.4) covers
  CI but not yet e2e suites during implementation.
- **M1 half applied**: agents 40% downgraded (Sonnet 553 + Haiku 7 out
  of 1,411 calls) ✓, but the main thread still 100% top-tier (Opus 5 +
  Fable 5) — the "mechanical session = Sonnet 5" rule from
  WORKFLOW.md has not been used a single time yet.
- **T5 ("without Concise" reference)**: 7.09 M output tokens over the
  window, ~1,464 per turn, ~591 k per session. Concise enabled this
  evening for week 2, per the protocol.

### Weekly measurement S2 — 2026-09-09 (window 29/08–06/09, 26 sessions)

`node scripts/measure-sessions.mjs --since 2026-08-29 --until 2026-09-06`:
137 Chief Engineer prompts, 11,140 turns, 586.5 M input equiv. main
thread + 319.6 M in 276 agent transcripts (**35.3%**). Jobs closed
during the window, with the figures on their own plans:
APPLE-SILICON (5.5 M, 3 gates), RETOURS-15 (20 M, 2 gates + 2
andon-stopped), MACOS (22.4 M, ~9 gates), AUDIT-V3 (87 M, ~27
gates), ENGLISH-SWITCH (434 M over 8 sessions, 12+ gates).

Reading the gaps:

- **What holds**: the quality guardrail (0 KO at the final STOP 2 on
  all five jobs, 0 red CI), M2 (1 review per job), T1 and W3 **on
  ordinary jobs** (5.5 / 20 / 22.4 M, 2–3 gates), and M1's agent half
  — the downgrade went 40% → **87%** (Sonnet 13,821 calls against
  2,031 on Fable).
- **T1 and W3 split in two (D5)**: the three ordinary jobs hold the
  target; the two multi-day *programs* blow it by an order of
  magnitude (AUDIT-V3 87 M / ~27 gates, ENGLISH-SWITCH 434 M / 12+).
  The indicator was defined "per closed job" on a window that
  contained only jobs; a program is not a job, and the figure says
  nothing useful about it as it stands.
- **T4 invalid as computed (81.3, D6)**: the window holds two
  autonomous sessions at **0 Chief Engineer prompts** (dcc2f031: 436
  turns; 7f4bb0d2: 521 turns), whose turns land on the other
  sessions' prompts. The blur was already flagged at S1 and never fixed; the
  number measures the session mix, not the workflow.
- **P1 worsened, same cause as S1 (320 min, D7)**: e2e runs in the
  foreground are **173 of the 320 minutes** (114 runs, max 603 s),
  shell 97 min, cargo 43 min. Push and CI, which wave 1.4 did cover,
  weigh 7 min together — the countermeasure works exactly where it
  was applied and was never extended to the e2e suites, which is
  what S1 already recommended.
- **T2 unchanged (371 k against 364 k)**: T3 improved (3 sessions
  > 24 h → 1) without moving T2, so the long-session hypothesis is
  not the whole cause; the ordinary sessions themselves carry
  250–470 k.
- **M1's main-thread half still untouched**: 11,137 top-tier calls
  (Fable 5: 6,115, Fable 5.1: 4,789, Opus 5: 233), zero Sonnet. The
  "mechanical session = Sonnet 5" rule of WORKFLOW.md has now gone
  two full windows without a single use.
- **Agents, 9.5% → 35.3% of the cost**: the model axis (M1) worked
  while the *volume* tripled. The plan judged "the number of agents
  is not a lever" at 9.5%; that judgment was made on a figure that no
  longer holds (D9).
- **T5 (Concise, trial closed)**: output per turn 1,528 → **1,561
  (+2%)**, per session 609 k → 669 k. No drop. Method note: the S1
  row was measured on the evening of 2026-08-28 and missed two late
  sessions of its own window; re-measured on 2026-09-09 with the same
  accounting, week 1 reads 9.13 M / 1,528 per turn over 15 sessions,
  and it is those two figures that are compared here.

### Tracking table

| Indicator | Baseline | Target | Wk. 1 | Wk. 2 | Verdict |
|---|---|---|---|---|---|
| T1 input equiv. / job | ~60 M | ≤ 35 M | 11–30 M ✓ | jobs 5.5 / 20 / 22.4 M ✓ — programs 87 M, 434 M ✗ | split, cf. D5 |
| T2 average context / turn | 410–540 k | ≤ 200 k | 364 k ✗ | 371 k ✗ | missed, cause not settled |
| T3 sessions > 24 h not closed | 8+ | 0 | 3 ✗ | 1 ✗ | missed, improving |
| T4 turns / prompt | 36.6 | ≤ 25 | 44.9 ✗ (blurred, cf. note) | 81.3 ✗ (2 sessions at 0 prompts) | indicator invalid, cf. D6 |
| P1 wall time blocked > 60 s in the foreground | ~3.5 h | ≤ 15 min | 100 min (> 30 s) ✗ | 320 min (> 30 s) ✗ — e2e 173 min | worsened, cf. D7 |
| P2 re-runs / flake | ≤ 11 | ≤ 2 | no flake to settle — | no flake settled in the window — | not exercised |
| P3 avoidable round trips | 24 | 0 | 0 observed ✓ | not instrumented | no figure |
| W1 full gate | 4 min 34 s (W0) | ≤ 6 min | 2.1–2.6 min (148 e2e) ✓ | not timed in the window | no figure |
| W2 1 e2e spec | 74 s | ≤ 45 s | 13.5–19 s (wave 2) ✓ | not timed in the window | no figure |
| W3 full gates / job | 10+ | ≤ 3 | 2 / 2 / 4 / 7 ~ | jobs 3 / 2 ✓ — programs 9 / 12+ / ~27 ✗ | split, cf. D5 |
| W5 docs-only push | ~2 min | ≤ 30 s | 7.6 s ✓ (commit b8d8aa0) | not timed in the window | no figure |
| ~~T5 (opt.) output tokens / session (Concise)~~ | wk. 1 ref. | drop without loss of quality | 9.13 M; 1,528/turn (re-measured 09-09; 7.09 M as recorded on 28/08) | 17.39 M; 1,561/turn (+2%) | **withdrawn (D8)** — instrument cannot resolve it |
| M1 top-tier cost outside jobs | ~10–15% | ≤ 5% | agents 40% downgraded; thread 0% ~ | agents 87% ✓; thread ~100% top-tier ✗ | half met |
| M2 high-effort reviews / job | up to 3 | 1 | 1 ✓ | 1 where recorded ✓ | held |
| Quality: KO at STOP 2 / red CI | ref. previous week | stable or ↓ | KO fixed same day; 0 red CI ✓ | 0 KO on the five jobs; 0 red CI ✓ | held |

---

## Review of 2026-09-09 — window due 2026-09-06 (D4), held three days late

Cause of the delay, recorded as a fact and not as an excuse: the audit
program of 2026-09-06, then PLAN-SWEEP and PLAN-BATCH, then the 0.20.0
and 0.21.0 releases occupied 06→08/09 end to end. The review is a rite
of the kaizen; it was displaced by the work it exists to measure,
which is itself a finding about the rite's placement (D10).

**Verdict, indicator by indicator** — cf. the tracking table for the
figures; here only met / missed and the cause.

- **Met, keep as is**: the quality guardrail (0 KO at the final STOP 2
  on the five jobs, 0 red CI — the invariant the whole kaizen is
  forbidden to trade), M2, and W1/W2/W5 as *delivered mechanisms*
  (wave 2 holds; simply not re-timed in this window).
- **Met on jobs, meaningless on programs**: T1, W3 → the denominator
  is wrong, not the countermeasure (D5).
- **Missed, cause identified, countermeasure exists but was never
  extended**: P1 (D7) — S1 named the cause, the amendment was never
  written, the figure tripled. This is the one clear process failure
  of the window.
- **Missed, cause not settled**: T2 — T3 improved without moving it,
  so "long sessions" is not the whole explanation; needs a finding of
  its own before any new countermeasure.
- **Invalid as computed**: T4 (D6) — measures the session mix, not
  the workflow. To be amended or withdrawn; it must not be carried a
  third window in this state.
- **Half applied, two windows running**: M1 — the agent half is a
  clear win (87%), the main-thread half has never been used once. A
  rule that survives two windows without a single application is
  either unusable as written or unowned (D9).
- **Trial concluded, condition not met**: T5 (D8).

**Standard work** (the plan's own rule: what is measured to work is
kept, the rest is amended or withdrawn) says three countermeasures
must now be amended — P1's scope, T4's definition, T1/W3's
denominator — one withdrawn (T5), and one rule re-decided or dropped
(M1's main-thread half). All six calls are the Chief Engineer's,
opened below as D5–D10 and awaiting reply.

---

## § CE Decisions

- **D1 — Session thresholds.** Average context ≤ 200 k (T2) and
  closing ≤ 24 h of wall time (T3): validate or adjust both
  thresholds.
  *CE reply (2026-08-23): "D1 OK" — thresholds validated.*
- **D2 — Measurement script in the repository.** Commit
  `scripts/measure-sessions.mjs` (it reads local transcripts under
  `~/.claude/projects/…`, a machine-specific path — like
  `install-workstation.ps1`): yes / no.
  *CE reply (2026-08-23): "yes" — the script will be committed at
  wave 0.*
- **D3 — Order of wave 2.** The proposed order (memoized rebuild
  first): validate or reorder.
  *CE reply (2026-08-23): "OK for the proposed order."*
- **D4 — Review window.** PDCA review on 2026-09-06: validate or move.
  *CE reply (2026-08-23): "OK for September 6." (Clarification
  recorded: this is not the end of the kaizen — it is the review of
  the measurement window; countermeasures that hold their figure stay,
  the others are amended or withdrawn.)*
- **D5 — T1/W3 denominator.** "Per closed job" says nothing about a
  multi-day program (AUDIT-V3, ENGLISH-SWITCH). Proposal: keep the
  target for an ordinary job, and budget a program per *increment*
  (E-step) rather than per program. Validate, adjust, or accept that
  programs sit outside the indicator.
  *Chief Engineer reply (2026-09-09): "I am convinced, no need to run a
  prototype. Let's go for a per increment approach" — adopted. T1 and
  W3 are per increment from this date, E0 counted apart; the method
  is fixed in § Per-increment attribution, W4 keeps its per-job
  denominator, and the `--by-commit` mode is owed before the next
  weekly measurement.*
- **D6 — T4 definition.** Compute turns / prompt over CE-driven
  sessions only (exclude sessions at 0 prompts), as already flagged
  at S1; or withdraw the indicator. It cannot be carried unchanged a
  third window.
  *Chief Engineer reply (2026-09-09): "exclude 0 prompt session" —
  adopted. T4 is computed over driven sessions only; the indicator
  definition is amended in axis T, the filter is owed in
  `measure-sessions.mjs` with D5's bucketing.*
- **D7 — Extend wave 1.4 to the e2e suites.** "Background beyond
  60 s" covers push and CI (7 min in the window) but not e2e during
  implementation (173 min of the 320). Proposal: an e2e suite is run
  in the background, the session announces the verdict — same rule,
  same wording, one more command class.
  *Chief Engineer reply (2026-09-09): "yes, extend" — adopted and
  **written the same day** into `job/SKILL.md` (inner loop) and
  `gate/SKILL.md` (verdict rules), the threshold set at ~60 s so a
  targeted single spec stays in the foreground. The countermeasure is
  the amendment, not the decision: S1 named this cause and left it
  unwritten, and the figure tripled.*
- **D8 — T5 (Concise) verdict.** No drop measured (+2% per turn).
  The protocol says withdrawn. Confound to weigh before ruling: the
  week-2 mix was dominated by a translation program, poorly
  comparable with week 1. Withdraw, or re-run the trial over a
  comparable window.
  *Chief Engineer reply (2026-09-09): "ok with your recommendation" —
  **withdrawn as an indicator**, and not because Concise failed: a
  +2% difference on the 14% of cost that output represents cannot be
  resolved against an activity mix nobody controls, and a re-run
  would meet the same wall. The output style stays or goes as a plain
  preference of the Chief Engineer's; the kaizen stops tracking it.*
- **D9 — Agents, 9.5% → 35.3% of the cost.** The plan states "the
  number of agents is not a lever" — judged at 9.5%. The figure has
  changed; the judgment has not been re-tested. Re-test, or restate
  it explicitly at the new figure.
  *Chief Engineer reply (2026-09-09): "ok with your recommendation" —
  **no cap on the number of agents**, which would attack the
  workflow's best defect detector (the review that caught the ~13 GB
  FTS5 index rebuild; guardrail held at 0 KO / 0 red CI over the
  window). Instead M3: the agent share is reported per increment, and
  beyond ~40% the increment's plan entry states what the agents
  found. The rise is a consequence of wave 1.8 — cheap agents get
  used more — so the question is return, not volume.*
- **D10 — Placement of the review rite.** The 2026-09-06 review was
  displaced by three days of shipping. Proposal: bind the review to a
  `/close` (the first close after the due date) rather than to a
  calendar date, so it cannot be crowded out silently.
  *Chief Engineer reply (2026-09-09): "maybe we could spawn a planned
  job [...] that reviews the kaizen every friday" — adopted, and
  better than the `/close` binding: a close is not guaranteed to
  happen in a given week, a Friday is. Delivered the same day as the
  scheduled task `kaizen-weekly-review` (see § Measurement and
  review). Known limit, stated: the task fires only while the desktop
  app is open, and runs on next launch if it was closed — a missed
  Friday is late, never skipped.*

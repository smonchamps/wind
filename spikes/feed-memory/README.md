# Spike D-53 — Feed memory (PLAN-AUDIT-2026-09-LOT5 § 3.2)

Throw-away, measured spike (STANDARD §2.2-2.3). Nothing here is
production code; the production tree is untouched except
`apps/desktop/ui-v2/src/Feed.svelte` (and `lib/body.js` for option B)
swapped in place by `run-option.sh` inside THIS worktree only.

- `bench.mjs` — the protocol (rest → Feed page 1 → ten pages scrolled
  → back to Inbox → +25 s), driven through `e2e/launch.mjs`; one JSON
  line per phase in `raw/<option>-<ko>k-run<n>.jsonl`.
- `ram-detail.ps1` — per-process breakdown of the same instance
  (`e2e/measure-ram.ps1` gives the headline figure).
- `options/Feed-{A,B,C}.svelte`, `options/body-B.js` — the variants.
- `patches/<option>.patch` — `git diff` of each variant against `2ade544`.
- `run-option.sh <A|B|C> <run> [ko]` — one run.
- `raw/` — every log, raw (cargo, npm, bench).
- `REPORT.md` — protocol, figures, limits, industrialization estimates.

Screenshots: only `raw/C-100k-run3-10pages.png` (the overlap defect of option C) is kept in the repository; the other nineteen (page 1 and ten pages per run, proof of rendering) were dropped at commit time — 5 MB of images in a public repository.

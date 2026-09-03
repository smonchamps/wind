# ADR 0027 — The theme table is short and living; "Mona" enters

Date: 2026-08-29 · Status: accepted

> **Addendum (2026-08-29, A95)**: the theme is renamed
> "Innamoramento" by the CE the same day, before any release —
> identifiers `innamoramento`/`innamoramento-nuit`, a written migration
> of the old persisted ids. Decisions D1-D3 below are unchanged;
> "Mona" remains its birth name here.

## Context

V7 (ADR 0026, 2026-08-24) had fixed the table at "two themes, and only
two". The CE asks for a third theme, "Mona" — main color `#AD204C`,
tile color `#A0868F`, in light and in dark. The investigation on the
evidence measured, at the gate's exact bench (`e2e/contraste.mjs`, same
pairs, same thresholds):

- `#AD204C` holds **6.80:1 on white** — accent AND brand of light, as
  is (A8's split is moot); it is also the hex of `--rep-rose`, a
  coincidence accepted as such;
- `#A0868F` as `--tuile` verbatim is **arithmetically impossible**:
  2.04:1 under `ink2` (threshold 4.5), 1.88:1 under the worst shared
  marker (threshold 3); at night, pure white gives only 3.33:1 — no
  ink can hold.

## Decision (CE, 2026-08-29)

- **D1** — V7 is **amended**: "we can add or remove themes from time
  to time". The table stays SHORT (never the return of the 28 Wada);
  Mona and Mona · night are the 3rd and 4th themes.
- **D2** — the tile **declines the hue** of `#A0868F` by polarity
  (`#EFDFE4` light / `#2C2126` night), the exact Elements gesture on
  its own tile.
- **D3** — the night accent is `#E58BA4` (lightened at constant hue,
  the `#1A7A7A` → `#3FA39C` pattern).

## What this overturns, and what holds

Overturned: the letter of V7 ("only two"). Holds: its spirit — one
direction per theme, each theme measured WHOLE (17 tokens × 2
polarities) at the common thresholds, never a combinatorial run; the
24 `--rep-*` remain the single table per polarity (their night block is
served by `[data-theme$="-nuit"]`, like A44's `color-scheme`); the A42
mechanics (OS-follow, polarity derived, never persisted) unchanged.
System log: **A94**.

## Reversibility

Removing a theme = the reverse path of the same contract ("What
adoption costs" in the System): CSS blocks, sheet, catalogs,
`NOMBRE_ATTENDU`, two `toHaveCount`, the doc's table — and the
migration guard of `theme.js` learns the removal (V7's pattern: the
polarity survives, everything else falls back to the default).

## Evidence

Contrast **440 pairs, 0 failure** (220 → 440 — 110 per new theme: 38
token pairs, 60 marker × background, 12 glyphs); coherence 4 themes /
68 tokens, the doc says what is delivered; migration guard **proven by
breaking it** (without `mona-nuit` in its list, a persisted choice was
rewritten `elements-nuit` — RED shown, then GREEN); e2e of the two
impacted specs 66/66; CE visual STOP of 2026-08-29: GO on real
captures, light + dark.

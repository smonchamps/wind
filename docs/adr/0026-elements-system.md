# ADR 0026 — The "Elements" System replaces "Clarity / Wada"

Date: 2026-08-24 · Status: accepted

## Context

The CE had a new art direction investigated — "Elements": one single
shape rule everywhere, markers on the geometric center, one single
distance decided, no optical correction. Two spikes judged it on the
evidence ([`spikes/direction-elements/`](../../spikes/direction-elements/README.md),
`spikes/marque-hitofude/`): contrast bench 74 measurements 0 failure
(palette corrected to the minimum at constant hue, remedy A8), 78
glyphs redrawn over three rounds of CE feedback, 7 animated signatures
compared, disc centering measured at 0.00 px. The spike's generator
produced a complete System (`systeme.v2.dc.html`) carrying 14 CE
decisions in the log (V1-V14) — including zero radius (V14), validated
in the field on 2026-08-24 on the real render.

## Decision

The "Elements" System **becomes the reference System**
(`docs/design/system.dc.html`, the path the gates read) and **the UI
delivers it** (PLAN-ELEMENTS, E1-E5). CE decisions of 2026-08-24:

- **D1** — the HTML is THE source, hand-edited (DC-D2 unchanged); the
  generator stays frozen as a spike (trace); the old System is
  archived (`docs/archives/systeme.v1.dc.html`); the V series is
  closed, the log continues in A-n (A79 = the adoption).
- **D2** — glyph attribution: "Wind's original set, drawn after
  Material Symbols (Google, Apache 2.0)"; the LICENSE Apache stays in
  the repository as provenance.
- **D3** — the three merge families stay differentiated by their
  current markers (verdict at STOP E2: sufficient at 16 px, no
  redraw).
- **D4** — the reduced 24 masters are delivered; the 16 tier (74
  drawings + 12 tiers 10-12) is a recorded debt (D-35), to be reopened
  if the field sees the blur).
- **D5** — vehicle: **0.9.0, MINOR**; the OAuth proof on the second
  workstation (deferred from 0.8.0) can happen on 0.9.0.

## What this overturns, and what holds

Overturned: A42 (28 themes → 2), A28/A36/A40/A52-signature (the
hitofude stroke dies — replaced by the disc/ring pair, V2), A29 point
2 (nav's solid badge → bare number + row disc, V4), A30 (the brand
moves to the Elements glyph/tile, V1/V11), the radii (V14: zero, three
shape tokens on `html`, platform exception 15/64).
Holds: A3 (one icon, one meaning — the dedicated marker set, the
reserved ones), A8 (never color alone; two palette corrections at
minimum), A18 (reinforced: the icon inventory is checked by the gate
in BOTH directions, tracings included), A52 (the % inside the text),
A61 (the body's light slab), A74 (the marker color chart, V5 against
doctrine — accessibility wins).

## Reversibility

V14 rewinds in one line (restore 10px/6px/2px to the three tokens).
The full return to Wada would be a job (the archive
`systeme.v1.dc.html` and the git history carry everything) — accepted:
the 28-theme table is no longer maintained or measured.

## Evidence

Five gate-green commits (`fb32238`, `fa45db7`, `3aa8a2d`, `84d46ea`,
`fed73e5` — history rewritten before push: an accent in a commit
message, STANDARD §2.9), 124/124 e2e at every step, four CE visual
STOPs the same day (base, icons, shapes, brand), contrast 220 pairs 0
failure. Full field validation at PLAN-ELEMENTS's STOP 2.

# Wind's icons — provenance

Since PLAN-ELEMENTS (V8, CE decision D2 of 2026-08-24), Wind ships
**no icon font at all**: the 78 glyphs are an **original set**, drawn
in the "Elements" grammar (24 grid, 2-unit stroke, flat caps, sharp
joins), served as inline SVG by
`apps/desktop/ui-v2/src/Icon.svelte` from the catalogue
`apps/desktop/ui-v2/src/lib/icons.js`.

**The normative inventory lives in the System**: the survey of the
"Icônes" section of `docs/design/systeme.dc.html` — one glyph, one
meaning, one use. The gate `e2e/system-coherence.mjs` keeps the survey
and the catalogue equal in both directions and checks every path (A18).

The shapes are **drawn after** Material Symbols (Google), published
under the Apache 2.0 license — kept here ([LICENSE](LICENSE)) for
provenance. The user-facing notice lives in Settings > About
(`settings.iconsValue`).

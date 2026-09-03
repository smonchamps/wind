# ADR 0020 — The reading thread: one object, two frames, exclusivity in the store

**Date**: 2026-08-16 · **Status**: adopted (UI v3, A43, commit 16f06e6)

## Context

The CE verdict from the annotation session of 2026-08-16 puts the
thread in cards INSIDE the reading pane, and screen 03 stays the full
screen of the same content. Question D4 ("what becomes of screen 03?")
was settled by the CE: "a coexistence that is nothing but a size
change of the same objects."

## Decision

1. **One component** (`Fil.svelte`) carries all the thread's drawing;
   **one module state** (`lib/fil.svelte.js`, runes) carries messages,
   expansion, body, attachments, images — shared between the frames.
2. **Frame exclusivity is structural**: `fil.cadre`
   (`null | 'volet' | 'plein'`) is the ONLY switch; each frame renders
   `{#if fil.cadre === le-sien}`. No local visibility boolean — the
   first version had three, reconciled by hand, desynchronized on the
   first uncovered path (archiving from the shortcut on screen 03,
   layout switch).
3. **Enlarge/shrink reload nothing** (`agrandirFil`/`reduireFil`);
   **open always reloads** (`ouvrirFil` — memoization had hidden the
   user's own reply); **close purges** (`fermerFil`, importable from
   anywhere, all layout modes).

## Consequences

- The P1 "opening" stopwatch measures selection → thread displayed
  (`thread_messages` included, attachments excluded) — series rebased
  (D-12).
- Changing frame remounts the iframes (D-13, accepted).
- The e2e specs anchor on the frame (`volet-lecture`/`conversation`)
  and assert the object's uniqueness (`fil-sujet` → count 1).

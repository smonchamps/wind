---
name: field
description: Handle a field finding from the Chief Engineer — reproduce, trace it to the root, fix it the same day with a test, an amendment of the System and a full gate. The fast lane, same guards as /job.
---

# /field — a field finding is fixed the same day

The argument is the finding, as seen in the field. This is the genchi
genbutsu loop of STANDARD §2.5: field feedback is fixed **the same day**
(model: the WAL, ADR 0011; the accent strokes, A37/A38).

## Steps

1. **Reproduce** — understand the exact mechanism before touching the
   code. If reproducing requires the CE's machine, ask for the precise
   measurement or manipulation and wait. Never a fix on a hypothesis.
2. **Trace to the root** — a symptom fixed at the surface comes back
   elsewhere (lesson 9ebd7b2 → 5698641: the shortcut blur only covered
   e/Del; the root was the focus left by the click). If the root is deep
   or the scope widens, **switch to `/job`** — the fast lane does not
   exempt from design.
3. **TDD**: a test that fails on the finding (e2e if it is a journey,
   Rust if it is the core), then the fix. The test asserts on the fact
   observed in the field, not on the implementation. Targeted inner
   loop (§2.4): the impacted spec(s) **as whole files** (never `-g`),
   RED then GREEN — the full gate comes after, once, before the commit.
4. **DC-D2**: if the UI is touched, the System is amended in the same
   commit — a sentence in the journal (A-n), the rule updated where it
   lives.
5. **Full gate**: `/gate`. Then commit (`fix:`, the mechanism and the
   remedy in the message body); push and `gh run watch` **in the
   background** — the session goes on and announces the CI verdict when
   it lands.
6. **Closure**: offer the CE to replay the gesture in the field to
   confirm — providing **systematically the PowerShell commands needed
   for that field test** (launching the app, build, preparing the
   accounts, measurements), ready to copy, one per block. Update the
   memory if the finding closes or reopens a job.

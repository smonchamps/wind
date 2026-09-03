# ADR 0021 — Full cycle cadence (S-D4 settled)

**Date**: 2026-08-17 · **Status**: accepted (Chief Engineer GO on the
field measurement of 2026-08-17, PLAN-RETOURS-2 §1)

## Context

Field feedback from the Chief Engineer: the Gmail sync is "too long."
Per-phase measurement on the real mailbox (Gmail account, ~52 folders,
`run_sync` trace, debug build):

```
INBOX 3.4s · inventory 16.4s · 52 folders (46 skipped) 31.2s · threads 7.8s · drafts 8.9s
```

Re-measured **in release** (debugging inflates the CPU phases; the
release app is a *windows* subsystem with no console — trace
redirected `2> file`):

```
INBOX 5.0s · inventory 12.6s · 52 folders (30 skipped) 109.8s · threads 0.0s · drafts 7.6s   ≈ 135 s
```

Reading: guarded polling (ADR 0017) **works** — most folders are
skipped, not every folder is walked. But the cost is **~5 s per
CHANGED folder** (network, likely Gmail throttling — identical
debug/release: 6 folders = 31 s, 22 folders = 110 s), plus STATUS for
all 52 folders at inventory (Gmail does not announce LIST-STATUS →
~52 sequential STATUS calls). On Gmail, many views move often ("All
Mail," Important, categories, labels): a full cycle **swings from ~8 s
to ~135 s** depending on how many folders changed. At 135 s every
5 min, the app was syncing **~45% of the time** — it is the cadence,
not the walk, that makes it "too long."

Now **the IDLE watcher (ADR 0018) is active in production**: it holds
INBOX in **real time** (`EXISTS` → targeted light pass). The 5-minute
full cycle dated from BEFORE IDLE, when it was the only arrival path
for mail. Since then, it only serves what IDLE does NOT cover — the
other folders, drafts, the deletion diff, flags. ADR 0018 §7 had left
the question open: "the full-cycle cadence will be re-discussed once
IDLE is active (S-D4)."

## Decision

1. **The full cycle moves from 5 min to 30 min.** INBOX does not
   depend on it for freshness — IDLE pushes it in real time.
2. **A light pass (STATUS INBOX only, a few seconds) runs every 5 min
   as a NET.** An IDLE watcher can drop without having reconnected
   yet — reading a dead socket "hangs" silently (ADR 0018 §Context).
   The net guarantees INBOX stays fresh within 5 min even in that
   case. It self-cancels during a full cycle (`enSynchro`): never two
   polls of the same INBOX at once.
3. **"All Mail" stays synced.** Excluding it would have lightened
   every cycle but **broken the Archive view** and made any mail
   archived from another device disappear from Wind (All Mail is the
   only repository of an archived message). **ADR 0010 ("everything is
   synced") is preserved.** The cadence alone divides the sustained
   load by 6 with no loss.

## Consequences

- **Sustained sync load ÷6** on an account with many folders. The
  felt "too long" drops: the expensive sweep runs 6× less often,
  INBOX stays real time.
- **Changes to OTHER folders made elsewhere** (a label reorganized, an
  archive, a draft written on another device) can wait **up to
  30 min** instead of 5. Accepted: this is not incoming mail (covered
  by IDLE + the 5-minute net).
- **Budget to re-measure in the field, in release** (debugging inflates
  the CPU phases) — the manual gesture and waking from sleep still
  force an immediate poll.

## Set aside — excluding Gmail's virtual views

Dropping Important / Starred from the sweep was safe (no unique mail,
not shown in Wind's nav) but, **once the cadence is at 30 min**, the
gain becomes marginal (these views are swept only 6× less often
anyway), for a real code cost: the core's `Folder` type does not carry
the notion of a "Gmail view," it would need extending, propagating
`\Important`/`\Flagged` flag detection into the IMAP adapter, and
touching the scope logic next to ADR 0010. Deferred (§2.6) — to reopen
if a field measurement shows the cadence alone is not enough.

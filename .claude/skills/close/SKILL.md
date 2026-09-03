---
name: close
description: Close a Wind job — verify the field validation and the green CI, mark the PLAN closed, amend STATE, record the debt, update the persistent memory.
---

# /close — the standardized close-out of a job

The argument is the plan to close (`PLAN-XXX`). A job is closed on
facts only; verify every condition before writing anything.

## Conditions (all of them, otherwise say what is missing and stop)

1. **Field validated** by the CE, explicitly, on their real accounts —
   an increment not validated in the field is not delivered (§2.5).
2. **Green CI** on the last pushed commit (`gh run list`).
3. **Clean tree**: nothing of the job is waiting for a commit.

## Writings

1. **`docs/PLAN-XXX.md`**: header "**JOB CLOSED on YYYY-MM-DD — full
   field validation**", with the commits, the date of the CE GO, the
   field touch-ups if any and their A-n (model: PLAN-WADA).
2. **`docs/STATE.md`**: the state reflects the delivered job; budgets
   re-measured if touched; lessons learnt added where they live.
3. **Debt**: what is deferred goes to `docs/DETTE.md`, named, with the
   reason for the deferral (§2.6 — a deferral is written down).
4. **ADR** if a structuring decision has none yet.
5. **Persistent memory**: the plan's file moves to "closed", absolute
   date, salient facts; the `MEMORY.md` index follows.
6. If these writings make a commit: `docs:`, then push and green CI.
7. **Kaizen figures of the job** (PLAN-KAIZEN-CLAUDE): note on the
   closed PLAN the input equivalent consumed (T1,
   `scripts/measure-sessions.mjs`), the number of full gates played (W3)
   and the KO findings at STOP 2 (quality guard).

## Last step — close the session

1. If a release is coming: **write the CHANGELOG entry now** (§2.9: it
   ALWAYS precedes `make-release.ps1`).
2. Then **close this session** — a closed job does not stay in context:
   billing it again at every turn of the next job is the first waste the
   kaizen measured. The next subject opens in a fresh session, on the
   reading of STATE.md.

## End of a phase

If the job closes a **phase** of the product PLAN: propose the closing
review `docs/PHASEn.md` (delivered against the plan, budgets
re-measured, lessons, assumed deferrals, GO/NO-GO) — it is a CE
decision, do not write it without their GO.

# ADR 0001 — Workspace structure: two members only

Date: 2026-07-11 · Status: accepted

## Context

The plan ([PLAN.md](../PLAN.md), §3) eventually calls for `mail-core`, `mail-protocols`,
`sync-server`, `apps/desktop` and `apps/web`. Simplicity is a key quality
characteristic of the product — it applies to the code too.

## Decision

The workspace holds only what is needed today:

- `crates/mail-core` — the domain, with no UI or network dependency;
- `apps/desktop` — a binary shell, the future Tauri app (Phase 1).

`mail-protocols` will only be extracted from `mail-core` once several
protocol implementations exist; `sync-server` and `apps/web` will only
appear in Phase 4. Creating these crates now would be dead stock (muda):
frozen interfaces before anything has been learned from the Phase 0 spikes.

The initial spike `src/main.rs` (IMAP with a hardcoded password) is removed,
per the plan (§9.1).

## Consequences

- Fewer boundaries to maintain while the domain stays small.
- The future extraction of `mail-protocols` is a plain module move, kept
  honest by the rule "`mail-core` depends on no UI".
- Shared workspace lints: `unsafe_code = "forbid"`, `unwrap`/`expect`
  forbidden outside tests.

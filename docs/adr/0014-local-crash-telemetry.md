# ADR 0014 — Crash telemetry: local, opt-in, no content

Date: 2026-07-26 · Status: accepted — **validated in the field the same
day**: a self-test panicked with a fake address in the message, and the
`crash-*.json` written on the real machine did not carry it. The field
also revealed a defect, fixed right away (§6).

## Context

Phase 5 ([PLAN.md](../PLAN.md) §5) plans an "opt-in crash telemetry".
Reconnaissance done on the code, three facts framed the decision:

1. **We start from a blank page**: no logging or telemetry
   infrastructure (`tracing`, `log`, `sentry`: absent), no panic hook,
   no preferences table.
2. **A "crash" here is narrow**: `unsafe_code = "forbid"` on the whole
   workspace, `unwrap_used`/`expect_used` at warning and zero in
   production → a crash of our code is almost always a **Rust panic**,
   catchable (`panic = "unwind"` by default). Most are even intercepted
   (Tauri for the commands, our `spawn_blocking` for the tasks); real
   crashes are rare.
3. **The privacy surface is real and precise**: a naive report
   (`format!("{err:?}")`) would leak — `Error::InvalidEmailAddress`
   holds **an address**. This is the §9 trap of the handover (a
   diagnostic that leaked identifiers).

## Decisions (arbitrated by the Chief Engineer)

1. **Destination: local file only.** One file per crash in
   `app_data_dir/crashes/`. **No network, no third party.** The app
   shows the reports; the user decides whether to send them. Fits the
   beta model where the Chief Engineer reviews every report himself.
   Against Sentry (sends to a third party, external dependency) and
   against a self-hosted endpoint (to set up/maintain), both ruled out
   for v1.

2. **Scope: Rust backend panics only.** A panic hook catches them —
   where the logic lives. Native crashes (minidumps) and JS errors are
   out of scope for v1: disproportionate complexity for code with no
   `unsafe`.

3. **Opt-in, off by default**, revocable, asked **once** (state
   "unset" → banner; then never again).

4. **The panic message is DROPPED.** It is the only free-text field
   that could carry personal data. The report keeps only **code and
   environment** artifacts: `file:line` location, symbol stack,
   app/OS versions, timestamp. Provably free of personal data, rather
   than a redaction by pattern that would be fragile to prove.

## Architecture — the project's pattern

- **Pure, in `mail-core`** ([`crash.rs`](../../crates/mail-core/src/crash.rs)):
  `redact(RawPanic) -> CrashReport` drops the message. Zero dependency,
  zero I/O. This is the proven core.
- **Platform, in the desktop app** ([`telemetry.rs`](../../apps/desktop/src/telemetry.rs)):
  the panic hook, consent, writing the file, the commands.

**Two hard rules, drawn from the reconnaissance:**
- **The hook never touches the database.** It may be the cause of the
  panic, or hold a poisoned lock. Consent therefore lives in a
  **file** (`telemetry.json`), read at startup into an `AtomicBool`
  the hook consults; the report is written with plain `std::fs`.
- **The hook never panics in turn** (a panic during a panic =
  `abort`): everything in it is wrapped (`catch_unwind`) and free of
  `unwrap`.

## Proofs

- **In-memory invariant** (`mail-core`,
  `le_rapport_n_emporte_aucune_donnee_du_message`): a message with an
  address + subject → the report, scanned through its `Debug`
  representation (so covering any future field too), keeps none of it.
- **On-DISK invariant** (desktop,
  `le_fichier_ecrit_ne_contient_aucune_donnee_du_message`): what
  really matters — the file's serialized bytes carry neither the
  address nor the subject, but keep the location.
- **Usefulness preserved** (`le_rapport_garde_de_quoi_situer_le_bug`):
  the location and the stack survive, otherwise the report would be
  empty.
- **E2E tightness**: in test, consent forced to `disabled` and zero
  reports (`DISCOVERY_DB_PATH` guard); one E2E holds the absence of
  both banners.

The `redact` implementation is **trivial** (one field dropped): no RED
that would teach anything (§2.4), the value is the permanent invariant
held by the two tests above.

## Field validation (to be played)

The hook can only be proven in situation (as with the notifications,
§7.2). A debug-only command, `telemetry_selftest_panic`, triggers the
panic — its body does not exist in release. Protocol, in a **debug**
session (`cargo run -p discovery-desktop`):

1. At startup, the opt-in banner appears → **Enable**.
2. Open the WebView console (F12) and invoke:
   `window.__TAURI__.core.invoke('telemetry_selftest_panic')`.
3. Reopen the app: the banner "Discovery ran into a problem" appears
   → **Open the folder**.
4. Open `crash-*.json`: check that it carries the location and the
   stack, and **NOT** `faux@exemple.fr` nor "secret".
5. Redo from an "unset" state (delete `telemetry.json`) choosing
   **No thanks**: no file should appear even after the self-test.

## Field finding (2026-07-26) — the double panic

The self-test proved the redaction (the fake address was absent from
the file), but revealed a defect: **a panic on the main thread produces
TWO**. The original (at the bug site) tries to unwind, crosses the
WebView2 FFI boundary (nounwind), and triggers a second `cannot unwind`
panic that aborts the process. The hook ran for both; the two files
carried the same second-resolution timestamp → **the second one (the
abort, useless) overwrote the first (the bug, useful)**. The Chief
Engineer opened a report pointing at `core/panicking.rs` instead of the
real site.

Fixed the same day, two guards:
- **`SEQ` counter** in the file name: two reports from the same second
  no longer collide (fix on the merits, tested).
- **Secondary-panic filter** (`is_secondary_nounwind`): the runtime's
  `cannot unwind` is not written — best effort, since it depends on a
  runtime message; if it changes, one report too many gets written
  (never one too few, thanks to the counter). Tested.

Lesson (handover §9): the capture proved itself correct, but the
**environment's behavior** (double panic at the FFI boundary) only
showed up in the field — not in a unit test.

## Consequences and limits assumed

- **A very early panic** (before `.setup`, where the hook installs)
  is not captured. Rare, and pre-consent anyway.
- **No automatic aggregation**: the price of "local file". Assumed for
  the beta; a sending channel can follow if the field asks for it.
- **The dropped message** loses some panics that were sometimes
  readable ("index out of bounds"). Deliberate choice: the guarantee
  of no leak comes first, the location is enough in the overwhelming
  majority of cases.
- **`explorer` hardcoded** to open the folder: a Windows dependency,
  consistent with the target (no mobile, no web here).

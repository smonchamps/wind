# E2E — the journeys, the nets and the benches

Rewritten at PLAN-BASCULE-ANGLAIS E6a (2026-09-03, Chief Engineer
decision D24): the page it replaces described the v1 selector contract
and the v1 journeys, gone since the redesign.

## What drives what

The specs drive the **real Tauri window** through CDP (WebView2), with
no `tauri-driver` and no `msedgedriver`: the application is launched
with `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=<port>`
and Playwright attaches with `connectOverCDP`. The port is **free and
chosen by the OS at each launch** ([port-cdp.mjs](port-cdp.mjs)): two
suites played at the same time from two worktrees share no state
(PLAN-ISOLATION-E2E — a fixed port once let one suite drive the other's
window).

Determinism by construction ([launch.mjs](launch.mjs)):

- a **throw-away seeded database** (`WIND_DB_PATH`) — never the user's;
- a fake account with an **invalid token** (`WIND_E2E_ACCOUNT`) —
  offline by construction: the outbox journals and never sends;
- the OAuth configuration **removed from the environment** of the process
  under test — no test can reach a real account, even by accident. The
  variable list lives in [isolation-oauth.json](isolation-oauth.json),
  one contract applied by [isolation.mjs](isolation.mjs) to every
  launcher — suite, benches, diagnostics, probe.

The debug build of the app and the seeders (the Rust examples of
`mail-core`) are compiled once per run by [global-setup.mjs](global-setup.mjs)
through [rebuild-v2.mjs](rebuild-v2.mjs), outside any spec's timeout;
[global-teardown.mjs](global-teardown.mjs) gives the machine its theme
back if the "follow the OS" test was killed mid-flight.

The suite launches the app **in English by default** (D22 — the
product's default since D4); [redesign-language.spec.js](tests/redesign-language.spec.js)
plays the French round trip (detection, switch, reload, exact French
forms). Anchors on French *fixture* text (the seeded decor) carry the
`lang:fr` marker for the language ratchet.

## Running

Prerequisites: Node 20+, Rust, WebView2 (present on Windows 11); Python 3
for the freeze probe only.

```powershell
cd e2e
npm install
npm test
```

`npm test` plays the node nets first (`*.test.mjs`), then the Playwright
specs (`tests/*.spec.js`, one worker, serial inside a file, one retry —
a flaky test is counted, never red: [flaky.mjs](flaky.mjs) reads
`test-results/report.json` and `scripts/gate.ps1` prints `flaky: N`).
Never play a single test with `-g`: a spec is a serial journey, it is
played as a whole file.

## The gate

The specs **do not run in the hosted CI**: a GitHub runner cannot open a
WebView2 window (measured — [ADR 0005](../docs/adr/0005-e2e-gate-outside-hosted-ci.md)).
They are played by the versioned `pre-push` hook, armed once per clone:

```powershell
git config core.hooksPath .githooks
```

The hook runs ONE gate, [`scripts/gate.ps1`](../scripts/gate.ps1)
(fmt, ui-v2 build and lint, the textual nets below, clippy, the Rust
tests, then this suite). Never `--no-verify` without a Chief Engineer
decision.

## The nets (seconds, played on the docs-only path too)

| Net | What it refuses |
|---|---|
| [contrast.mjs](contrast.mjs) | a WCAG pair under threshold in any theme (System A8) |
| [system-coherence.mjs](system-coherence.mjs) | a token, theme or glyph the System says and the code does not ship, or the reverse (DC-D6) |
| [main-thread-guard.mjs](main-thread-guard.mjs) | a blocking Tauri command on the message pump (PLAN-GELS) |
| [language-gate.mjs](language-gate.mjs) | any RISE of French markers per file against `language-baseline.json` (PLAN-BASCULE-ANGLAIS E1a) |
| [ipc-contract.mjs](ipc-contract.mjs) | a Tauri command registered, defined and called by name out of step |
| [docs-links.mjs](docs-links.mjs) | a dead relative markdown link |
| [dom-contract.test.mjs](dom-contract.test.mjs) | a test id a spec selects that no component renders |
| [catalogs.test.mjs](catalogs.test.mjs), [placeholders.test.mjs](placeholders.test.mjs), [language.test.mjs](language.test.mjs) | the two catalogues out of step, a `{placeholder}` without its parameter, the language detection |
| [port-cdp.test.mjs](port-cdp.test.mjs), [rebuild-v2.test.mjs](rebuild-v2.test.mjs), [apply-dom.test.mjs](apply-dom.test.mjs), [apply-e2e.test.mjs](apply-e2e.test.mjs) | the tooling itself |

## The DOM contract

A spec selects the UI by `data-testid`, by a few state classes and by
accessible name, never by layout. The names are one table,
[`scripts/rename/dom.csv`](../scripts/rename/dom.csv) (test ids,
classes, `data-*` attribute names, `window.__e2e*` seams), and
`dom-contract.test.mjs` asserts that every id a spec selects is rendered
by a component. Changing a name here is changing the gate: the component
and the specs move in the same commit.

## Benches, probes and tools (never during a gate)

- [measure-v2.mjs](measure-v2.mjs) — startup, first page, RAM
  (`MEASURE_DB`, `MEASURE_ACCOUNTS`, `MEASURE_REUSE`); STANDARD §9: a
  warm cache lies, measure cold.
- [measure-scroll.mjs](measure-scroll.mjs), [measure-scrollbar.mjs](measure-scrollbar.mjs),
  [scroll-gesture.mjs](scroll-gesture.mjs) — the deep-scroll figures
  (PLAN-DEFILEMENT-PROFOND).
- [measure-ram.ps1](measure-ram.ps1) — RAM of ONE running instance: the
  private working sets of `wind-desktop.exe` and its WebView2 processes,
  summed (`-AppPid`, `-Profil`).
- [freeze-probe.py](freeze-probe.py) — freezes of the message pump
  (`python e2e/freeze-probe.py <base.db>`, a database OUTSIDE the
  repository). Never during a gate: the e2e launcher kills every
  `wind-desktop` under `target\` (a false crash, code -1).
- [capture-onboarding.mjs](capture-onboarding.mjs) — the onboarding
  fixture screenshots; [diag-v2.mjs](diag-v2.mjs) — the list-page budget
  split into its three floors (core query, IPC, Svelte render), played
  AFTER `measure-v2.mjs` on its binary and database;
  [dark-toggle.ps1](dark-toggle.ps1) — toggles the Windows theme for the
  "follow the OS" test; [browser-args.mjs](browser-args.mjs) — the single
  source of the WebView2 arguments (launch, benches, probe);
  [tokens.mjs](tokens.mjs) — the parser of `system.css` the nets share.

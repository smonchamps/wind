# ADR 0002 — Desktop shell: Tauri 2 confirmed at the skeleton gate

Date: 2026-07-12 · Status: accepted · **Gate 1 re-measured: held (see bottom of page)**

## Context

The plan ([PLAN.md](../PLAN.md) §2.4) set Tauri 2 as the starting hypothesis
for the Windows shell, against Slint/egui (native) and Electron. The Phase 0
close-out review ([PHASE0.md](../archives/PHASE0.md) §3) required it to be
validated first thing in Phase 1: it is the most structuring hypothesis not
yet spiked.

## Measurements (release build, skeleton: window + static frontend + core wired in)

| Metric | Measurement | Budget (PLAN.md §1) | Verdict |
|---|---|---|---|
| Startup → usable window* | **613 ms** | < 1 s | ✅ |
| Total private memory | **164 MB** (5.7 app + 158.6 WebView2) | < 200 MB | ✅ margin 36 MB |
| Executable size | **8.15 MB** | installer < 15 MB | ✅ trajectory holds |

\* measured from the start of `main()` to the frontend's first `invoke` (DOM ready).

Memory methodology: sum of the **private bytes** (`PrivateMemorySize64`)
of the main process and the 6 child WebView2 processes. The sum of
*working sets* (329 MB) overestimates by counting shared pages between
processes several times over — it is the private measurement that holds.

## Decision

**Tauri 2 is confirmed** as the desktop shell. The memory cost is almost
entirely the fixed WebView2 overhead (~159 MB); our code adds ~6 MB on top.
In exchange: an 8 MB executable (Electron: ~80-150 MB), the WebView2 runtime
already present on Windows 11, and the web UI reusable in Phase 4.

## Consequences and watch points

- **The RAM margin is only 36 MB for an empty window.** Gate 1
  (virtualized list, 50,000 messages) mandatorily re-measures; if the
  budget breaks, the documented plan B remains Slint/egui — hence the
  importance of keeping the UI "dumb" and the domain in `mail-core`.
- The shell's CSP is `default-src 'self'` from the skeleton onward: no
  inline script or style, even for us. (Later relaxed for `img-src` and
  `style-src`: a `srcdoc` document inherits the CSP of its host — see
  `mail_render::email_document`; scripts stay `'self'` only.)
- The icon is a generated placeholder (32×32); a real visual identity
  will come with Phase 5.

## Re-measure at gate 1 — 50,000 messages (2026-07-12)

Virtualized list (~40 DOM rows regardless of volume, 200-row pages served
by SQLite on scroll) on a synthetic INBOX of 50,000 envelopes
(`examples/seed_inbox`, a 5.4 MB database written in 0.9 s):

| Metric | Empty database | 50,000 messages | Budget |
|---|---|---|---|
| **Resident** private memory | 85.0 MB | **84.5 MB** | < 200 MB ✅ |
| Startup → usable window | — | **348 ms** | < 1 s ✅ |

**Volume costs nothing in RAM** (delta −0.5 MB): virtualization fully
isolates memory from the number of messages.

Methodology correction: the skeleton measurement used **committed**
private bytes (`PrivateMemorySize64`), which Chromium inflates with
reservations that are never resident (observed variance: 250-375 MB on
identical load). The measurement that holds is the **private working set**
(WMI `WorkingSetPrivate`, averaged after 30 s of stabilization) — this is
what the user sees in Task Manager. On this basis, the real margin is
~115 MB, not 36.

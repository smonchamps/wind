# ADR 0015 — UI v2 foundation: Svelte, single web front, carried everywhere by Tauri 2

Date: 2026-08-10 · Status: accepted — arbitrated by the Chief Engineer
on measurements (throw-away spike, two engines: desktop Blink **and**
Android-class).

## Context

A "Clarity" design handoff, done separately from the code, sets the
visual direction of the redesign. What binds Discovery is NOT its
prototype's technical choices (a homegrown runtime, not reused) but the
**rules of its System document**: 14 tokens driving all color (hot
theme toggle, 9 themes), one unique **signature** (surface + 2 px accent
seam on the left + warm shadow), **two** radii (10 surfaces / 6
controls), **one** elevation, typography graded **by size** (weight 600
reserved for display), Material Symbols Rounded icons, single-line
truncation (constant height).

A new requirement frames the decision: v2 must port **simply and
efficiently to web, Linux, macOS, iOS and Android** — *not necessarily
the same tech per platform*.

Frozen constraints that act as the cutting edge:
1. **`mail-core` is the only brain** (ADR 0001) — the UI displays a
   state, emits intents. The core already compiles for all 6 targets.
2. **Budgets = gates** (PLAN §1): startup < 1 s, opening < 50 ms,
   **list page < 100 ms**, RAM < 200 MB, at **256,312** messages in a
   virtualized list.
3. **Mail HTML rendered in a sandbox** (iframe + CSP, remote images
   blocked, never `innerHTML`) — security invariant, non-negotiable.
4. **Web = server-side engine** (Phase 4): keeping a web front makes
   this phase nearly free.
5. **The E2E gate drives the real webview via CDP** (ADR 0005).

v1 (Windows, Tauri 2 + vanilla JS, ADR 0002) **stays in place**; this
ADR decides the **foundation of the v2 redesign**, not its timeline.

## The set-based grid

Seven technical families, sorted into three **porting strategies** —
the decisive fact being: the core is already shared, so the only cost
rewritten N times is **the System**.

- **A — a single web front**, carried by Tauri 2 (Win/Linux/macOS/iOS/
  Android via system webviews) + browser (web). System written **once**.
- **B — a single cross-platform native front** (Flutter / Dioxus /
  Slint): once, but mail-HTML via an embedded webview per platform, a
  new test harness, and (Flutter) a new language + FFI.
- **C — shared core + native face per platform** (web+desktop;
  SwiftUI; Compose): System written **2-3 times**.

**Strategy A retained as the spine**: least regret (reaches all 6
targets with the substrate already in place), and it closes no door —
a native face per platform (towards C) can be added *later and only if
a measurement demands it*, without touching the core. B (Flutter) only
displaces A if native mobile rendering is **required as early as v2**:
it is not.

Left to settle was the **web face of A**: ① vanilla + Web Components,
② Svelte, ③ Rust → WASM (Leptos/Dioxus). Throw-away spike
[`spikes/ui-socle-v2`](../../spikes/ui-socle-v2/RAPPORT.md), single hard
point — **virtualized list at 256,312 messages** (rich row of the
System) + **theme toggle** — measured identically in two engines.

## Decisive measurements

**Desktop** (headless Edge = Chromium; p95, 300 windows spread over
depth, 60 toggles):

| | page p95 | theme p50/p95 | JS heap | gz weight |
|---|--:|--:|--:|--:|
| **① vanilla** | 1.5 ms | 0.2 / 0.3 ms | ~1 MB | **5.2 KB** |
| **② Svelte** | 2.4 ms | 0.2 / 0.3 ms | ~7 MB | **16.5 KB** |
| **③ WASM** *(estimated †)* | ≈ ①② | ≈ ①② | +a few MB | **~150-400 KB** |

**Android-class** (REAL Blink engine from Android System WebView,
390×844 viewport DPR 3, touch, **CPU ×6 = entry level**):

| | mount | page p95 | theme p95 | JS heap |
|---|--:|--:|--:|--:|
| **① vanilla** | 5.6 ms | **22.5 ms** | 5.3 ms | ~12 MB |
| **② Svelte** | 20.1 ms | **29.0 ms** | 4.4 ms | ~27 MB |

† **③ not built** (neither Rust nor a WASM toolchain in the
environment): render and toggle ≈ ①② by construction (emits real
DOM+CSS); weight is a supported estimate based on Leptos/Dioxus'
published sizes. Never a made-up figure.

Three lessons, confirmed in both engines:
1. **Rendering is neutralized** by windowing (≤ 20 rows in the DOM,
   whatever the total) and by a theme driven by CSS variables (a
   browser restyle, not framework JS). Even at CPU ×6, page p95 stays
   at **22-29 ms — 3 to 4× under the 100 ms budget**.
2. **The decision therefore does NOT hinge on speed**, but on
   **shipped weight**, **baseline memory**, and **maintenance
   ergonomics**. On those axes: **① < ② < ③**.
3. The **framework tax** becomes visible when the CPU is slow (mount
   ② 20 ms vs ① 6 ms) but does not push ② out of any budget.

## Decision (arbitrated by the Chief Engineer)

1. **UI v2 foundation = ② Svelte 5**, within **Strategy A**: a single
   web front, carried by Tauri 2 (desktop + iOS + Android via system
   webviews) and by the browser (web, server-side engine — Phase 4).
   The **System is expressed once.** Rationale: both viable engines
   hold every budget; the surcharge of ② (16 KB, ~7 MB, 20 ms mount at
   ×6) is **trivial** against < 1 s / < 200 MB; and the decisive
   advantage across **the whole** app is **maintainability** — the v1
   `app.js` already runs 1,638 lines of imperative DOM, the pain ②
   removes.

2. **③ (Rust → WASM) ruled out of the foundation.** Heaviest weight and
   memory, riskiest maturity (Tauri + Leptos + mobile), for **zero
   rendering gain**; mobile drives the point home (WASM parse/compile
   at cold startup on a slow CPU, the worst case). To be reopened only
   if the team strategically decides "Rust everywhere".

3. **① (vanilla) is the documented fallback**, on the same rule set
   (CSS tokens, windowing, trivial toggle), should a case weigh "zero
   build / zero dependency / absolute minimal weight" above everything
   (extreme mobile target).

4. **The UI ↔ core boundary is a transport port** with two
   implementations: **in-process** (Tauri IPC / FFI, desktop + mobile)
   and **remote** (HTTP/WS, web). Written once, it makes the foundation
   portable and keeps `mail-core` free of any UI dependency (ADR 0001
   intact).

## Watch points and guardrails (measured/named, not assumed)

- **iOS / WKWebView = field validation DUE.** Not tested here: WebKit
  is a DIFFERENT engine from Blink (distinct DOM/CSS profile), macOS
  simulator only. **The real remaining unknown** — to validate on real
  Apple hardware or a device farm before any iOS shipment. Deferral
  assumed, as with ADR 0004's plan B.
- **Real mobile hardware** (GPU/compositor, RAM pressure, thermals) and
  **compositor scroll smoothness** are only approximated by CPU
  throttling: to confirm on device.
- **Fixed row height**: the spike freezes 104 px (the System's
  truncation rule wants a "constant height"). Variable-height chips
  would require a *measured* virtualization — an engineering decision
  **equal across the three families**, so with no effect on this
  choice, to be settled at implementation.
- **Material Symbols vendored locally** (subset of the System's 34
  glyphs): offline-first and the CSP forbid the Google Fonts CDN at
  runtime.
- **Mobile collision with ADR 0010.** Full synchronization (database up
  to ~13 GB) is untenable on a phone → a windowed/on-demand sync
  policy. **A core decision, not a UI one**: to be handled by a
  separate ADR before mobile shipment.
- **Vault and OS push**: `keyring` is desktop; iOS Keychain / Android
  Keystore to integrate; IDLE yields to APNs/FCM.
- **The System's layouts = desktop, 3 columns.** Tokens / typography /
  signature are portable; the layouts are not. A **compact/touch**
  variant (single column, nav drawer, ≥ 44 px targets, gestures) is
  design work **due** before any mobile shipment.
- **E2E gate**: the redesign renames the DOM → freeze stable
  `data-testid`s so redesign and tests move in lockstep; a **mobile**
  test harness will come (the current desktop CDP does not cover iOS).

## Consequences

- v1 (Windows, vanilla JS) **stays in place** until the switch; the
  migration will happen screen by screen following a phase plan —
  **tokens first** (invisible foundation, v1 app repainted to prove the
  token system end to end), then Settings modal + persistence, then
  layouts.
- New build dependency (**Svelte + Vite**) confined to the UI app.
  `mail-core` stays free of any UI dependency (ADR 0001) — invariant
  intact.
- **Extends ADR 0002** (Tauri desktop shell): Tauri **2**, desktop
  **and** mobile targets via system webviews; the web stays served by
  the same server-side engine (Phase 4).
- The spike [`spikes/ui-socle-v2`](../../spikes/ui-socle-v2/RAPPORT.md)
  played its role — eliminating ③, proving ①/② on two engines.
  **Throw-away**: deletable once this ADR is read.

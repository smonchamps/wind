# Lot 4 — Dependencies, build and release guarantees

Opened: 2026-09-07. Baseline: `9f4a9b1` (Lot 3 committed locally, field
"1-11 OK", push deferred by D10 of that lot).
Parent: [audit program](PLAN-AUDIT-2026-09.md), E11–E12.
Method: [WORKFLOW](WORKFLOW.md), [job](../.claude/skills/job/SKILL.md).

**Status: at STOP 2 since 2026-09-07 — implemented, reviewed, gated; field
verdict awaited. No delivery claim.**

## 1. Findings

Measured on this workstation on 2026-09-07 (read-only reconnaissance, two
agents, then hand-checked):

- **Dependencies (S03).** `npm audit` in `apps/desktop/ui-v2`: four advisories,
  one high (Vite `server.fs.deny` bypass on Windows) and three moderate (Vite
  optimized-deps traversal, launch-editor NTLM disclosure, esbuild dev-server
  cross-origin), all on Vite 5.4.21 / esbuild 0.21.5 and the two Svelte plugin
  packages. Every fix npm proposes is a semver-major jump (Vite 5→7 or 8,
  plugin-svelte 4→6 or 7). `e2e` is clean. Dependabot covers GitHub Actions
  only. All advisories concern development servers; nothing ships in the
  binary.
- **CI coverage (A10).** Four jobs: `quality` (Windows: fmt, clippy, tests,
  doc tests), `quality-macos` (two triples), `ui-v2` (Ubuntu: build, the seven
  structural nets, lint), `audit` (`cargo audit`). Absent: `npm audit`, and the
  **ten standalone Node suites** (`node --test`, 33 cases since Lot 3) that only
  the local gate's step 13 runs. `assert-dist-clean.mjs` runs at release only.
- **Diagnostics (S05, A05).** One trace module (`apps/desktop/src/trace.rs`,
  `wind.log`, 1 MB truncation) with 22 free-text call sites and no redaction:
  the §6.8 promise ("no subject, no sender, no body") rests on every caller.
  `Error::InvalidEmailAddress(String)` displays the raw address; a past field
  finding already caught an address in `wind.log`. Across the IPC boundary only
  one error is typed (`remote_message_too_large`); everything else is prose the
  UI cannot branch on. One string-prefix decision remains in production:
  `is_connection_error` (`starts_with("connection ")`) decides whether a
  reconnect refreshes the OAuth token.
- **Release identity (B29).** `make-release.ps1` bumps the version files, builds
  from the working tree, then commits *only* the bump files: any other local
  edit is baked into the binaries and never committed. It never checks the tree
  is clean. `release-macos.sh` checks a clean tree and the bumped version but
  not that HEAD is the tagged commit.
- **Latest before the matrix (B30).** `gh release create … --latest` runs with
  the two Windows keys; the six macOS assets and their two keys arrive later
  from the Air. Between the two, mac updaters read a manifest without their key.
  `verify-release.ps1` reports the missing mac half as "NOT PRESENT" and can
  still exit 0.
- **Signature proof (B31).** `verify-release.ps1` hands the raw `.sig` asset to
  `minisign -x`; Tauri writes `.sig` files as base64 of the minisign text (the
  same wrapper the script *does* decode for the pubkey), so the crypto check
  cannot pass; and when `minisign` is absent the script prints "NOT PROVEN"
  without counting a failure, so the final verdict passes with no proof.
  `minisign` is not installed here; the workspace already builds
  `minisign-verify 0.2.5` for the Tauri updater.
- **Gate duplication (A09, D-32, D-33).** D-32 is already paid in the code
  (`.githooks/pre-push` only calls `gate.ps1 -DocsOnly`) but the register still
  describes two encodings; its own reopening condition (a 10th step) fired
  silently at 13 steps. D-33 stands: `build.rs` declares no dependency on
  `ui-v2/dist`, `tauri.conf.json` has no `beforeBuildCommand`, so a bare
  `cargo tauri build` embeds whatever dist is on disk. The "rebuild clean, then
  assert, then build" sequence exists twice (PowerShell and bash).
- **Documentation drift.** STANDARD §2.10 still describes five assets, two keys
  and an unprovable signature; MACOS-BUILD and the scripts say eleven and four.

## 2. Scope and acceptance

| Step | Work and proof | Coverage | State |
|---|---|---|---|
| E11a | Coherent major upgrade of Vite and the Svelte plugin on measured spikes; lockfile; `npm audit` clean; build, lint and the whole UI suite green | S03 | Spikes measured; awaiting D2 |
| E11b | CI: `npm audit` (both trees) and the ten Node suites as jobs; `assert-dist-clean` after the CI build; Dependabot on npm and cargo | A10 | Planned |
| E11c | Diagnostics: one sanitizer inside `trace` (addresses masked, CR/LF collapsed, length bound, secrets stripped), typed error categories across the IPC (`code`, `retryable`, `scope`) with the prose kept as detail, `Error::Connection` replacing the string prefix; maintained tests that a synthetic address, subject and injected newline never reach `wind.log` | S05, A05 | Planned |
| E12a | Release identity: clean-tree refusal, bump committed *before* the builds, per-platform attestation (tag, commit, `Cargo.lock` and dist hashes) uploaded as an asset; the mac script refuses a HEAD that is not the release commit | B29 | Planned |
| E12b | Publication order: draft release, full matrix uploaded, `latest.json` completed last, promotion to Latest only after a passing verification; explicit `-WindowsOnly` override that states what it drops | B30 | Awaiting D1 |
| E12c | Signature proof: a small workspace tool on `minisign-verify` decodes the base64 wrapper and verifies every channel against the pubkey of `tauri.conf.json`; `verify-release.ps1` fails without proof, `-Structural` names the incomplete mode; tamper/wrong-key/missing fixtures | B31 | Planned |
| E12d | Build inputs: `beforeBuildCommand`/`beforeDevCommand` rebuild the dist clean of seams; `build.rs` dependency on the dist if the spike proves it re-embeds; the shared rebuild-and-assert step in one script called by both release scripts; D-32 closed in the register, D-33 settled on the measurement; STANDARD §2.10 corrected | A09, D-32, D-33 | Spike measured |

Each row needs maintained failing assertions where a behavior changes,
release-script fixtures with fake `git`/`gh`/artifacts (no real upload), the
final gate and, at STOP 2, the field. Field proof of the release chain itself
is the next actual release (a version is not created by this lot).

## 3. Design and measured options

### E11a — the build chain (set-based, two spikes)

| Option | What | Measured |
|---|---|---|
| A | Vite 7 + vite-plugin-svelte 6 (Rollup, the conservative major) | see § 5, spike A |
| B | Vite 8 + vite-plugin-svelte 7 (Rolldown, npm's own proposal) | see § 5, spike B |

Both are recorded in § 5 with versions, audit result, build warnings, build
time, dist size and the Node suites. The choice is D2. The e2e suite is then
replayed whole on the retained option before the gate.

### E11b — CI

- `ui-v2` job gains `npm audit --audit-level=<D3>` for `apps/desktop/ui-v2`
  and `e2e`, and `node scripts/assert-dist-clean.mjs` after a `VITE_E2E=0`
  build (the release guard, proven on every push, not on release day).
- New `node-tests` job on `windows-latest` (the platform the gate proves):
  `node --test` on the ten suites, without Playwright. The Playwright suite
  stays local (no WebView2 on hosted runners, unchanged).
- Dependabot: `npm` for the two trees and `cargo` for the workspace, weekly,
  grouped minor/patch — majors arrive as reviewable PRs, never as `audit fix`.

### E11c — diagnostics

- **Sanitizer at the sink**, not at each caller: `trace()` masks anything
  shaped like an address (`<address>`), strips `Bearer …`, `password=…` and
  `access_token=…` shapes, collapses CR/LF to `⏎`, and bounds a line to 512
  bytes. Proven by tests feeding a synthetic address, subject, token and
  injected newlines through the real writer and reading `wind.log` back.
- **Typed categories across the IPC.** `CommandError` serializes
  `{ code, message, retryable, scope }` for every `mail_core::Error`,
  `SendError` and `AuthError` variant (`network`, `refusal`, `auth`,
  `throttle`, `stale`, `disk`, `uncertain`, `local`, …) keeping the message as
  detail; the UI branches on `code` and reads the message only as text. The
  existing `remote_message_too_large` shape stays.
- **`Error::Connection`** in mail-core replaces the `starts_with("connection ")`
  decision of the reconnect path; both adapters raise it at connect time.
- Scope refusal: no support-bundle feature, no log viewer, no change to the
  1 MB truncation (D4 of PLAN-AUDIT-V1 stands).

### E12a/b/c — the release chain

1. `make-release.ps1`: refuse a non-clean tree (`git status --porcelain`
   empty, except nothing); refuse a branch other than `main`; bump, **commit
   and tag first**, then build both channels from that HEAD; write
   `attestation-windows.json` (version, tag, commit, `Cargo.lock` SHA-256,
   dist SHA-256, triples, signature digests) and upload it with the assets;
   create the release as a **draft** with the Windows keys.
2. `release-macos.sh`: additionally refuse `git rev-parse HEAD` ≠ the tag's
   commit; write and upload `attestation-macos.json`.
3. `publish-release.ps1 <v>` (new, the last gesture): runs the full
   verification on the draft (eleven assets, four keys, four cryptographic
   signature proofs, both attestations at the tag commit), then and only then
   `gh release edit <v> --draft=false --latest`. Until it runs, the previous
   Latest stays complete for every updater.
4. `verify-release.ps1`: calls the workspace tool for each channel; a missing
   proof is a failure; `-Structural` prints the explicitly incomplete verdict
   and exits 1 by design when cryptographic proof was expected.
5. `tools/release-verify` (workspace member, `minisign-verify` + `base64`,
   both already in the lock): `verify <pubkey-b64> <sig-b64-file> <artifact>`.
   Fixtures: a valid pair generated in test with a throwaway key, a tampered
   artifact, a wrong key, a missing file — all four must fail loudly.
6. Script fixtures: a PowerShell test harness with fake `git`/`gh` on the PATH
   proving the clean-tree refusal, the commit-before-build order, the draft
   creation, and the promotion refusal when any check fails.

### E12d — build inputs and gate sharing

- `tauri.conf.json`: `beforeBuildCommand` and `beforeDevCommand` run the
  ui-v2 build (`VITE_E2E=0`) then `assert-dist-clean.mjs`; the two release
  scripts call the same `scripts/build-dist-clean.mjs` instead of their own
  sequences (one file, as `assert-dist-clean.mjs` already is).
- `build.rs` gains `rerun-if-changed` on `ui-v2/dist` and `tauri.conf.json`
  **only if spike C proves** a dist change re-embeds through a bare
  `cargo build`; otherwise D-33 stays with the measurement written in.
- D-32 closed in the register with the evidence; STANDARD §2.10 rewritten to
  the eleven-asset, four-key, locally-proven chain.

Explicit refusals (§2.6): no code signing (Authenticode/notarization, D-60 and
the SAC memory stay); no CI-side release build; no Vite migration beyond what
the retained option requires; no rewrite of the trace into a structured event
bus — the sanitizer at the sink is the whole of S05's remedy here, typed codes
the whole of A05's for this lot.

## 4. Chief Engineer decisions

Asked one by one on 2026-09-07, after the § 5 spikes; answers verbatim
(the option labels were French in the dialog):

- **D1, 2026-09-07:** “Brouillon puis Latest (recommandé)”. <!-- lang:fr -->
  The release is created as a draft; Latest is promoted by
  `publish-release.ps1` only after the complete four-key matrix passes the
  cryptographic verification. `-WindowsOnly` exists as an explicit, talkative
  exception for a day without the Air.
- **D2, 2026-09-07:** “A : Vite 7 (recommandé)”. <!-- lang:fr -->
  Vite 7.3.6 + vite-plugin-svelte 6.2.4, the Rollup line; Vite 8 is the next
  major, not this one.
- **D3, 2026-09-07:** “moderate (recommandé)”. <!-- lang:fr -->
  `npm audit --audit-level=moderate` blocks CI on both trees.
- **D4, 2026-09-07 (STOP 2), verbatim:** “Ne faire apparaitre le bouton qu'en cas de connexion indisponible.” <!-- lang:fr -->
  The generic account's "Repair connection" button, permanent since Lot 3
  (D7 there), read as an alert on a healthy account: it now shows only with
  "Connection unavailable" or "Disconnected", like the OAuth "Reconnect".
  The repair itself (frozen identity, password/ports) is unchanged.

The questions as asked:

- **D1 — publication order.** Proposed: the release is created as a draft;
  Latest is promoted only after the complete four-key matrix passes the
  verification, from `publish-release.ps1`. Consequence: the Windows channel
  is no longer public before the mac half runs (same day, two machines). A
  `-WindowsOnly` override exists for the day the Air is unavailable and prints
  what it drops (mac clients see no update). *Recommended: yes.*
- **D2 — build chain target.** Option A (Vite 7, Rollup) or B (Vite 8,
  Rolldown), decided on the § 5 figures. *Recommendation stated in § 5 once
  measured.*
- **D3 — `npm audit` blocking level in CI.** `moderate` (zero baseline after
  the upgrade; any new advisory blocks, a false positive becomes a job) or
  `high` (moderate advisories only reported). *Recommended: moderate.*

## 5. Verification and field

Record RED/GREEN figures here as they occur. At completion: one fresh review,
full `scripts/gate.ps1`, then STOP 2 with the committed `scripts/field.ps1`
and `scripts/run-wind.ps1` commands and a numbered field checklist (the
diagnostics and the dev/build path are field-observable; the release chain is
proven by fixtures now and by the next actual release).

### Spikes — 2026-09-07

Two isolated worktrees, same machine, Node 24.14 / npm 11.19; baseline
Vite 5.4.21 measured first in each. Neither option needs a single change to
`vite.config.js`, `svelte.config.js`, the ESLint config or the sources; in
both, the one-line `npm install vite@N …` fails with ERESOLVE (the old
inspector package pins the peer) and the reproducible form is
`npm uninstall vite @sveltejs/vite-plugin-svelte` then `npm install --save-dev
vite@N @sveltejs/vite-plugin-svelte@M`.

| Measure | Baseline (Vite 5.4.21) | A: Vite 7.3.6 + plugin 6.2.4 | B: Vite 8.2.2 + plugin 7.3.0 |
|---|---:|---:|---:|
| `npm audit` advisories | 1 high + 3 moderate | **0** | **0** |
| `[vite-plugin-svelte]` warnings (gate red) | 0 | 0 | 0 |
| Build wall clock, warm median | ~3.0 s | ~3.0 s | ~2.4 s |
| `index-*.js` / `.css` bytes | 343,096 / 86,245 | 343,630 / 86,241 | 340,882 / 85,840 |
| Lint | green | green | green |
| Node suites (nine at `a58c1cf`, `e2e` installed) | 28/28 | 28/28 | 28/28 |
| Seam guard (`VITE_E2E` 0 / 1) | pass / seams present | pass / seams present | pass / seams present |
| Toolchain underneath | Rollup + esbuild 0.21 | Rollup 4.63 + esbuild 0.28 | Rolldown 1.2 + oxc + lightningcss |
| Bundle delta vs baseline | — | esbuild minifier rewrites only | new minifiers (oxc, lightningcss) |

Neither spike ran Playwright or the desktop build; the retained option is
proven by the whole e2e suite in the gate and by the field launch.
**Recommendation for D2: option A.** It removes every advisory with the
same bundler family and the smallest byte delta; B's gains (about 0.6 s of
build, 0.6 % of bytes) touch no product budget and bring a new minifier pair
whose runtime behavior in WebView2 is unmeasured. B stays the natural next
major once Vite 7 reaches end of support.

**Spike C (D-33), isolated worktree, own target directory, debug profile,
n = 1 per step.** With `println!("cargo:rerun-if-changed=ui-v2/dist")` in
`build.rs`, a bare `cargo build -p wind-desktop` reruns the build script and
recompiles `main.rs` when a file is added to the dist (B6: 7.4 s, re-embedded,
proven by decompressing the brotli blob out of the exe); without the line, a
new file is invisible (B3: 0.5 s, bare `Finished`, not embedded). A content
change of an existing file was already tracked by tauri-codegen's
`include_bytes!` dependency (B2). No perpetual rerun: no-op builds settle at
0.5–0.6 s with or without the line. `tauri-build 2.6.3` watches the dist only
under its optional `codegen` feature, which Wind does not enable; it already
watches `tauri.conf.json` (absolute path), so that second line is redundant.
Decision: the one dist line goes in; D-33 closes on this measurement.

E11a applied on the main tree after D2: Vite 7.3.6, plugin 6.2.4, `npm audit`
0/0 on both trees, build 2.04 s with zero warnings, lint green.

### Implementation increments — 2026-09-07

- **E11a.** Vite 7.3.6 + vite-plugin-svelte 6.2.4 in the main tree by the
  two-step install; `npm audit` 0/0 on both trees; build 2.04 s, zero
  warnings; lint green. No source or config change.
- **E11b.** CI: `npm audit --audit-level=moderate` on `ui-v2` and `e2e` (D3),
  `scripts/build-dist-clean.mjs` after the build (the release seam guard on
  every push), a `node-tests` job on `windows-latest` running the twelve
  Node suites without Playwright; Dependabot on both npm trees and the Cargo
  workspace, minor/patch grouped. The CI change is proven by the next push.
- **E12d.** `build.rs` emits `rerun-if-changed=ui-v2/dist` (spike C);
  `tauri.conf.json` declares `beforeBuildCommand`/`beforeDevCommand` on
  `scripts/build-dist-clean.mjs`, and both release scripts dropped their
  copied sequence. A bare `cargo build -p wind-desktop` after the hook build
  recompiled the app in 23.6 s. D-32 and D-33 closed in the register.
- **E12c.** `tools/release-verify` (workspace member; `minisign-verify`,
  `base64`, `sha2`, all already in the lockfile). Fixtures signed with a
  throwaway `cargo tauri signer` key (private key never committed): valid,
  tampered, wrong key, broken wrapper, missing file, and the old script's
  exact mistake (the raw wrapper refused by a minisign parser) — **6 tests**,
  clippy clean; the CLI answers `VALID`/`REFUSED` with exit 0/2.
- **E12a/b.** `scripts/release-lib.mjs` (attestation, expected matrix,
  promotion rule; **5 Node tests** covering the complete draft, the missing
  mac half, the missing proof, the crossed signature, the foreign commit,
  differing lockfiles or UI builds, replaced bytes, and the Windows-only
  exception). `make-release.ps1`: main-only, clean-tree refusal before any
  write, lockfile refreshed and release commit BEFORE the builds, tree
  re-checked after them, attestation, `-Yes`, push + bare tag pushed, DRAFT
  Release. `release-macos.sh`: draft lookup, HEAD must be the tag's commit,
  attestation uploaded. `publish-release.ps1` (new): downloads the draft's
  assets, proves every channel with `release-verify`, applies the promotion
  rule, promotes with `--draft=false --latest`; `-WindowsOnly` explicit.
  `verify-release.ps1`: reads drafts as a failure, requires the mac half
  unless `-WindowsOnly`, checks both attestations at the tag's commit,
  verifies every signature with `release-verify`, `-Structural` names the
  incomplete mode; the verdict fails when any proof is missing.
  `e2e/release-scripts.test.mjs` runs the real `make-release.ps1` against
  fake `git`/`gh`/`cargo`/`rustup` stubs on the PATH (**3 tests**): a dirty
  tree or a foreign branch refused before any bump or build; the release
  commit before the first build; the tag pushed before a `--draft`, never
  `--latest`, Release carrying the attestation. **The net was proven red**:
  against the pre-lot-4 script (`git show HEAD:scripts/make-release.ps1`)
  it fails 2/3. All three scripts pass the PowerShell parser; `bash -n`
  passes. ADR 0044; STANDARD §2.10 and file map, MACOS-BUILD rewritten.
- **E11c.** `Error::Connection` typed in mail-core, raised by the IMAP
  connector, matched by `is_connection_error` (no string prefix left).
  `trace::sanitize` at the sink: addresses masked to `<address>`,
  `password=`/`token=`-shaped values and bearer tokens redacted, CR/LF
  collapsed, 512-byte bound; two maintained tests read `wind.log` back
  after a hostile line (synthetic address, subject, token, injected line).
  `CommandError` serializes `{code, message, retryable[, limit]}` for every
  core, send and auth error (fourteen codes), the bare-string shape kept
  for string errors; `transport.js` keeps the string face and exposes
  `code`/`retryable`. Two maintained Rust tests on the wire shape.

### Fresh-eyes review — 2026-09-07

Four finder angles over the staged diff (line-by-line, removed behavior,
cross-file tracer, cleanup/altitude/conventions), each measured what it
claimed. Confirmed and corrected before the final gate:

1. **Tauri hooks ran from the wrong directory** (all three angles, measured
   with an `echo %CD%` hook: the bare-string form runs from `ui-v2`, so
   `../../scripts/…` resolved to a path that does not exist and every
   `cargo tauri build`/`dev` failed at the hook). Fixed with the object form
   `{script, cwd: "../.."}` and `wait: true` for the dev hook (the bare form
   spawns it without waiting, racing the Rust build on a half-written dist).
   Measured after the fix: the real hook builds and asserts the dist from
   the repository root.
2. **`2>$null` on native commands under Windows PowerShell 5.1** with
   `$ErrorActionPreference = Stop` is a terminating error (measured): the
   tag creation, and the tag lookups of `publish-release.ps1` and
   `verify-release.ps1`, would abort with a raw NativeCommandError instead
   of their verdict, and the resumption after a partial failure was blocked
   by an existing tag. Fixed: the tag is checked with `rev-parse --verify`
   (exit code only) and reused when it sits at the release commit, refused
   at another; `gh` lookups go through `cmd /c … 2>nul`.
3. **The mac script demanded both `main` and HEAD = tag**, incompatible with
   the documented `git checkout <version>` (detached HEAD). Fixed: the
   identity is the tag's commit, checked out detached; "from main" now means
   the commit is on `origin/main`'s history (`merge-base --is-ancestor`), and
   the attestation records `main` on that proof.
4. **Cross-platform digests**: a Windows checkout under autocrlf hashes a
   CRLF `Cargo.lock` and `index.html` where the Mac hashes LF, so the two
   attestations could never agree. Fixed: text inputs are digested with LF
   endings; the lockfile equality stays a refusal (same commit, same lock),
   while a differing dist digest is **said, not refused** (each platform
   builds its own UI; byte identity across operating systems is not a
   promise Vite makes). Node regression on the CRLF lockfile.
5. **The sanitizer** (line-by-line angle, reproduced): a `Bearer ` marker
   followed by a break advanced into the middle of a multibyte character
   and panicked inside the sink; accented addresses were not addresses to
   the ASCII scan. Fixed (char-boundary resume, Unicode alphanumerics); a
   maintained test covers an accented local part, an accented domain, the bare marker and
   two tokens on one line. A first version of the same test looped forever
   on the marker itself — caught by the raw-logged run.
6. **The release matrix lived in three files** (cleanup angle): now one,
   `release-lib.mjs channels <version>`, consumed by both PowerShell scripts;
   the Node suite list likewise lives once (`test:node`) for the gate and
   CI. `release-verify` is built once per script run and called as an exe
   (a build error can no longer read as a failed proof). Dead code removed
   (`sha256` subcommand and its dependency, the unread `windows_only`
   attestation field, the untestable `-Yes` on the irreversible promotion);
   the "builds modified the tree" guard got the harness test it lacked.
7. **Retry semantics belong to the core** (A01): `Error::code`/`retryable`
   and `SendError::code`/`retryable` live in mail-core with their test; the
   shell only serializes.

Recorded, not changed: `Error::InvalidEmailAddress` keeps the address in its
message because the compose surface must show the user which recipient is
invalid; the sink's sanitizer (now Unicode-aware) is the belt for `wind.log`,
and the IPC message is the user's own input. `console.error(…, err)` in the
UI now prints a `String` object for typed errors (devtools only).

After the corrections: core **567**, desktop **60**, release-verify **5**
passed; clippy clean; ten Node release tests pass (the harness stays red
against the pre-lot script); all three PowerShell scripts parse; `bash -n`
passes. The first full gate (before these corrections) was red only on one
e2e scenario, `redesign-feedback-3` "replying targets the chosen message",
whose retry crashed the Playwright worker; the whole file passed **3/3 in
17.8 s** in isolation and is recorded as a flake, not a defect. The final
full gate follows, unchanged.

## Full gate and STOP 2 handoff — 2026-09-07

Two gates preceded the green one: the first (before the review's
corrections) was red on one e2e scenario, `redesign-feedback-3` "replying
targets the chosen message" (retry worker crash; the whole file passed 3/3
in isolation, recorded as a flake); the second was red at step 7 because the
accented test addresses of `trace.rs` and one line of this plan counted as
French markers (four `lang:fr` markers on test fixtures, one neutral
wording). The final gate then ran unchanged, `scripts/gate.ps1`, exit 0 in
**866 seconds** on a visibly slow machine that day (e2e 12.8 min against
5–9 usually).

| Step | Verdict and figures | Seconds |
|---|---|---:|
| 1. Rust format | GREEN | 0.8 |
| 2. UI build and lint | GREEN; Vite 7.3.6, no build or lint warning | 7.9 |
| 3. Contrasts | GREEN; four themes, 440 pairs | 0.1 |
| 4. System coherence | GREEN; four themes, 68 token values | 0.2 |
| 5. Main-thread guard | GREEN; 126 commands checked | 0.2 |
| 6. Script syntax | GREEN; JavaScript, PowerShell (three release scripts) and shell | 4.2 |
| 7. Language ratchet | GREEN; 356 files, 1967 markers against 1974 baseline | 0.5 |
| 8. IPC contract | GREEN; 125 defined/registered, 112 called by name | 0.1 |
| 9. Documentation links | GREEN; 78 files, 422 relative links | 0.2 |
| 10. Clippy | GREEN; all targets including `release-verify`, warnings denied | 5.0 |
| 11. Rust tests | GREEN; 895 passed, four ignored | 59.0 |
| 12. Rust documentation tests | GREEN; no runnable doctest | 5.6 |
| 13. Node and UI tests | GREEN; 43 Node passed; 261 UI passed, four flaky, one optional benchmark skipped | 782.0 |

Rust totals: auth 42, core 567, iCal 16 + 7, IMAP 115, render 34, SMTP 47,
release-verify 5, desktop 60, core examples 2. The four UI flakies passed on
their automatic retry and are recorded, not erased: delivery-states "offline
enqueue", generic-username "settings identity frozen", modal-focus "settings
contains focus", redesign-screen02 "notifications preference". None touches
this lot's code paths (the lot changes no UI surface); the load of the day
is the suspected cause, as the e2e duration shows.

**Real-world proof of the verifier, before the field**: after the gate,
`verify-release.ps1` gained `-Unattested` (a release published before this
lot carries no attestation, said explicitly) and was run on the published
0.19.0: eleven assets, four keys, **four channel signatures VERIFIED** by
`release-verify` against the pubkey of `tauri.conf.json`, every URL resolving
whole, signatures pairwise distinct. B31 is thus proven on the real release,
not only on the throwaway-key fixtures. That script edit came after the
green gate; the full gate is replayed on the final tree before the commit.

Field handoff (nothing here touches a real account; the release chain itself
is proven by fixtures and by the 0.19.0 verification, its full field proof
being the next actual release):

1. **The application on the new build chain (E11a).** Launch through the
   committed script and use Wind for an ordinary session (read, reply,
   search, Settings): no visual or behavioral difference is expected from the
   Vite 7 build; anything odd is a KO.
2. **The trace keeps its promise (E11c).** After that session, open
   `wind.log` next to the database (its path is printed by `field.ps1`):
   no address, no subject, one line per event. The check below must print
   nothing.
3. **Typed errors (E11c).** Go offline and click Sync: the status bar reads
   "Sync failed" as before; the notice wording is unchanged (the codes ride
   underneath, invisible).
4. **The build hooks (E12d), optional, about two minutes.** From
   `apps/desktop`, `cargo tauri build --debug --no-bundle` prints
   "dist rebuilt clean: no __e2e in the bundle" before compiling.
5. **The verifier on the published release (E12c).** Run the verification of
   0.19.0 below: the verdict must end with "4 channel signature(s) verified".

After closing Wind:

```powershell
powershell -ExecutionPolicy Bypass -File "C:\Users\smonc\OneDrive\Documents\Repositories\wind\scripts\field.ps1"
```

```powershell
powershell -ExecutionPolicy Bypass -File "C:\Users\smonc\OneDrive\Documents\Repositories\wind\scripts\run-wind.ps1"
```

```powershell
Select-String -Path "$env:APPDATA\dev.elements.wind\wind.log" -Pattern '@[A-Za-z0-9.-]+\.[A-Za-z]{2,}'
```
(typed in a PowerShell window as is — the first handoff wrapped it in a
`powershell -Command "…"` whose inner quotes were mangled, item 3 below.)

```powershell
powershell -ExecutionPolicy Bypass -File "C:\Users\smonc\OneDrive\Documents\Repositories\wind\scripts\verify-release.ps1" 0.19.0 -Unattested
```

Confirmation gate on the final tree (after `-Unattested` and the handoff
documents): **GREEN in 1137 s**, same Rust/Node totals, 261 UI passed with
five flaky retries (composer-html rich paste, enqueue-recovery lost
acknowledgement, feed-images past page 0, generic-repair connected account,
generic-username frozen form), none in this lot's code paths; the machine's
load that day doubled the e2e duration (12.8 then 17 minutes). Recorded, not
erased.

Await the per-item field verdict before commit. Per D10 of Lot 3 the push is
grouped with the audit's other changes; CI proves the new jobs at that push.
Lot 4 is not delivered; Lots 5–6 remain open.

## STOP 2 field findings — 2026-09-07

The Chief Engineer replayed items 1 to 3 (4 and 5 not yet):

1. **KO, fixed the same day — `[Gmail]` and an imported label shown as
   "Messages" issues.** Settings > Accounts listed, for both Gmail accounts,
   `Messages · [Gmail]` and `Messages · smonchamps@outlook.com/Archive` with
   `server refusal: No Response: [NONEXISTENT] Unknown Mailbox …` and an
   automatic retry every scheduled sync. Root: Gmail lists those folders,
   then answers `NO [NONEXISTENT]` on SELECT; before Lot 3 that refusal
   went silently to the trace every cycle, since Lot 3 it became a partial
   failure with a retry the user can never make succeed. Fix at the
   adapter's boundary: the RFC 5530 response code `[NONEXISTENT]` (which
   the pinned parser does not type) becomes `Error::NoSuchMailbox`; the
   cycle then marks the folder non-selectable until the next inventory,
   traces it masked, records **no** diagnostic, and prunes the diagnostics of
   any folder outside the sync scope (the rows already on the Chief
   Engineer's machine disappear at the next full cycle). Regressions:
   `a_folder_refused_as_nonexistent_leaves_the_scope_without_a_diagnostic`
   (core cycle, with a stale row to prune) and the typed response-code test
   in mail-imap. Written with the fix: the red is the field screen itself.
   Not touched: why Gmail lists `smonchamps@outlook.com/Archive` at all (a
   label named after an import, with a `/`), which the server alone knows.
2. **KO, fixed the same day — images broken in an OpenAI newsletter from
   an always-allowed sender.** The `.eml` handed by the Chief Engineer was
   parsed locally: five images, **three over `http://`** (a SendGrid CDN,
   which serves TLS: HEAD 200) and two over `https://`; no `srcset`, no
   `cid:`, no CSS `url()`. The renderer kept the cleartext addresses under
   "allow" while the iframe's CSP admits `https:` only (audit 2026-09-01:
   never cleartext), so the three showed as broken icons. Fix in the
   sanitizer, where the policy already lives: a remote image address is
   upgraded to `https://` (never fetched in clear, never left broken).
   RED shown on the maintained test
   `a_cleartext_image_is_upgraded_to_https_when_remote_images_are_allowed`
   (the `http://` survived), GREEN after. A host without TLS would still
   fail its image, said as the residual limit.
3. **Tooling — the `wind.log` check.** The command was handed wrapped in
   `powershell -Command "…"` and its inner quotes broke; the plain
   PowerShell line above replaces it.

Rust after the fix: core cycle 9/9, IMAP typed-code test, desktop 60/60,
clippy clean; whole core and IMAP suites replayed in the raw log. The final
full gate is due after item 2 is settled.

Gate after the two field fixes: a first run was red on `redesign-feedback-3`
"replying targets the chosen message" (the retry left an empty recipient);
the whole file then passed **12/12 in isolation** (four repetitions, under a
second each): a load flake, not a defect — other sessions were consuming the
workstation that afternoon. The next full gate, unchanged, went **GREEN in
314 s**: Rust 898 passed (four ignored), 43 Node, 264 UI with one flaky
retry (recipient-routing Bcc-only), the e2e step back to 3.9 minutes.

Second field pass on finding 1 (cards still present, no `nonexistent` line in
the trace): the first fix inspected the error's **Display**, which the IMAP
crate prefixes with "No Response: ", so the `[NONEXISTENT]` code was never
seen. The adapter now reads the code from the response's `information`
field and keeps the shown text; the mail-imap test drives the real wire path
(the fake server answers a tagged `NO [NONEXISTENT] …` to SELECT, the
adapter returns `NoSuchMailbox`). Finding 2 (images) confirmed OK in the
field; the 0.19.0 verification passed on the Chief Engineer's run with four
signatures verified. IMAP 116, core cycle 9/9, clippy clean; final gate due
after this finding's field confirmation.

## STOP 2 verdict — 2026-09-07

Items replayed by the Chief Engineer: **1 OK** after the second pass
(cards gone), **2 OK** (the five images), **3 OK** (no address in
`wind.log`), **5 OK** (the 0.19.0 verification: four signatures verified);
item 4 (the build hook, optional) not played — the hook is proven by the
measured run and by CI's seam guard. One product decision came with the
verdict, **D4** (repair button only when the connection needs it), applied
the same day with its e2e inversion. Zero KO left open.

Final gate on the final tree: **GREEN in 314 s** — Rust 898 passed (four
ignored), 43 Node, 264 UI with one flaky retry. That retry is again
`redesign-feedback-3` "replying targets the chosen message": the scenario
passes 12/12 in isolation and fails only inside the whole suite, three
times today; it is recorded as a recurring in-suite flake to take into the
debt register at closure (a race between the composer's opening and its
reply context under load is the lead), not as a defect of this lot.

Committed locally; the push waits for the grouped push of the audit (D10 of
Lot 3), and with it CI and the closure of Lots 3 and 4.

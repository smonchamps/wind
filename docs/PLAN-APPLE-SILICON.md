# PLAN-APPLE-SILICON — a native Apple Silicon build (debt D-62)

> Statement (Chief Engineer, 2026-09-05): *"A user using Apple Silicon
> wants to enter the beta. How to build an Apple Silicon version?"* —
> GO on `/job Feature: Apple Silicon build (D-62)` the same day.

**JOB CLOSED on 2026-09-06 — full field validation** (E0 measured by
the Chief Engineer on the Intel Air: arm64 links in 13 min 24 s, `file`
says arm64, the bundler names the dmg `_aarch64`; the RUN proof is the
tester's install at 0.19.0, accepted at the GO). One commit `0ef4f81`,
CI green 33995481451 — all five jobs, the new aarch64 leg passing the
full test suite NATIVELY on Apple Silicon for the first time. ADR
0037, debt D-62 paid. Kaizen: ~3.0 M input equivalents main thread +
2.5 M agents in one 0.9 h session (121 turns, 3 prompts), 3 full
gates (1 red — the gate's own bash lookup, fixed —, 1 green, 1
pre-push replay), **0 KO at STOP 2**.

Status: STOP 1 played on 2026-09-05 — D1-D5 settled (§6), GO the same
day.
---

## 1. Finding (sweep of 2026-09-05)

Everything mac-shaped in the repository names ONE triple,
`x86_64-apple-darwin`, in nine places — and every one of them was
written with the second arch in mind:

- `scripts/release-macos.sh:28` — `TRIPLE=` constant; the asset names
  `Wind_<v>_x64.{dmg,app.tar.gz,sig}` and the `darwin-x86_64` key are
  derived from it in one block (steps 3-5).
- `scripts/patch-manifest.mjs` — already takes a fifth `platform`
  argument (default `darwin-x86_64`); its crossing guard (identical
  signature under another key) is N-way. **Nothing to change.**
- `scripts/verify-release.ps1:76` — "a third asset family (Apple
  Silicon, D-62) is one array away": the mac asset list, the
  key ↔ assets consistency check and the `$platforms` table are the
  three lines to extend.
- `.github/workflows/ci.yml:61` — `quality-macos` runs on
  `macos-latest`, which IS an Apple Silicon runner, and names
  `--target x86_64-apple-darwin` on purpose (tests under Rosetta 2,
  proving the triple that ships). A native aarch64 pass is the
  runner's own arch: no Rosetta, real runtime.
- `docs/MACOS-BUILD.md` — Intel-checked (§0 `uname -m`), one build
  command, "8 assets and 3 platform keys", limit line "one arch only".
- `docs/BETA.md:20` / `docs/BETA.fr.md:22` — "Apple Silicon Macs can
  run it through Rosetta; a native build will come with demand";
  line 150/162 "macOS (Intel-native)".
- `README.md:9`, `CHANGELOG.md` 0.19.0 entry (unreleased: "Intel;
  Apple Silicon Macs run it through Rosetta"), ADR 0036 decision 1
  ("one triple only"), `docs/DEBT.md` D-62.

**The updater's key**: `tauri-plugin-updater 2.10.1` builds the
manifest key as `{target}-{arch}` from `cfg!(target_arch)`
(`src/updater.rs:1318,1344`) — an aarch64 build looks up
`darwin-aarch64`. Verified in the pinned crate source.

**Native code in the dependency graph** (Cargo.lock): `libsqlite3-sys`
(bundled C, via `cc`), `ring` (C + asm, via `cc`), `objc2`,
`security-framework`, `keyring apple-native`. All are built by the
`cc` crate, which passes `-arch arm64` when the target differs from
the host on Apple; Xcode's SDK is fat. An Intel Mac cross-builds
arm64 with the same toolchain — Tauri's documented path
(`--target aarch64-apple-darwin`). **Not yet measured on the Air**
— that is the one hard point (§3, E0).

**No Apple Silicon machine in the fleet.** The Chief Engineer's Air is
Intel; the requester's machine is the only arm64 Mac. The Air cannot
run an arm64 binary: the first install BY THE TESTER is the field
proof of the arm64 build, exactly as the mac update path is only
provable at the second mac release (ADR 0036).

**The tester is not blocked today**: the x64 dmg runs under Rosetta 2
on Apple Silicon (macOS offers Rosetta at first launch). Unmeasured;
documented in BETA.md since PLAN-MACOS.

## 2. Scope

**In:** a third asset family — `Wind_<v>_aarch64.dmg`,
`Wind_<v>_aarch64.app.tar.gz` + `.sig` — built on the Intel Air by
`release-macos.sh` next to the x64 family; the `darwin-aarch64` key in
`latest.json`; `verify-release.ps1` extended (11 assets, 4 keys, the
aarch64 key ↔ assets stand or fall together); the mac CI job proving
BOTH triples; the guides and the debt register.

**Refusals (§2.6):**

- **No universal binary.** One dmg for both archs halves the asset
  count but doubles every download and update (two binaries in one),
  and the updater plugin keys on `{target}-{arch}` — a universal
  tarball would have to be published under TWO keys with ONE
  signature, which is precisely what the crossing guard forbids
  (make-release trap 3, patch-manifest). Two families, one per arch,
  is the Windows model (ADR 0023) transposed.
- **No CI release build.** `macos-latest` could build arm64 natively,
  but the minisign key and the three OAuth secrets would have to
  live in the secrets of a PUBLIC repository, and a second signing
  path would exist — refused at PLAN-MACOS §3, unchanged.
- **No Rosetta measurement.** The x64-under-Rosetta path is the
  tester's fallback if E0 fails; it is not a deliverable.
- **No second build machine.** The Air builds both.

## 3. Options weighed

- **Where the arm64 binary is built**: (a) cross on the Intel Air
  (`rustup target add aarch64-apple-darwin`, `--target`) — one
  machine, one script, ~13 min more per release; (b) universal build
  (`--target universal-apple-darwin`, lipo of both) — refused above;
  (c) CI — refused above. → (a), decision D1.
- **The CI net**: (a) a second `quality-macos` job (matrix over the
  two triples: x64 under Rosetta as today, aarch64 native) — real
  aarch64 runtime coverage, mac minutes double (free on a public
  repo); (b) clippy only on aarch64 in the existing job — no runtime
  proof. → (a), decision D2.
- **Asset naming**: `aarch64` (Tauri's bundler output name and the
  manifest key) vs `arm64` (the Windows asset name, Apple's own
  word). → decision D3.

## 4. Steps

- **E0 — the measurement (on the Air, by the Chief Engineer)**: the
  cross-link has never been attempted. Before any script change ships,
  one test build on the Air proves it, or fails loudly:

  ```bash
  rustup target add aarch64-apple-darwin
  ```

  ```bash
  cd ~/wind/apps/desktop && cargo tauri build --target aarch64-apple-darwin --config '{"bundle":{"createUpdaterArtifacts":false}}'
  ```

  Report: exit status, wall time (budget: the 13 min of the x64 cold
  build), and `file target/aarch64-apple-darwin/release/bundle/macos/Wind.app/Contents/MacOS/wind-desktop`
  must print `arm64`. A link failure names its crate: that crate is
  the job's real hard point and gets a spike.
- **E1 — `release-macos.sh`**: a `TRIPLES` loop over
  `x86_64-apple-darwin` and `aarch64-apple-darwin`; the arch label
  (per D3) derived per triple; the dmg glob pins version AND arch;
  patch-manifest called once per triple with its platform key; the
  closing message names 11 assets and 4 keys. Two Tauri password
  prompts per run (one per build) — stated in the guide.
- **E2 — `verify-release.ps1`**: `$macExpected` becomes two families;
  the presence test and the key ↔ assets consistency are per family
  (a release with x64 published and aarch64 missing is a broken
  release, not a "not yet", since the script publishes both or dies);
  the `$platforms` table gains `darwin-aarch64`. Proven by breaking
  it: the checks are replayed against 0.18.x (no mac assets) and, at
  the 0.19.0 release, against the live manifest.
- **E3 — CI**: `quality-macos` becomes a matrix job over the two
  triples (`targets:` and every `--target` from the matrix), names
  kept distinct in the check list. Proven by the first green run on
  aarch64 (a native runtime pass of the 469 mail-core tests).
- **E4 — docs**: MACOS-BUILD (`rustup target add` in §4, the two test
  builds in §7, §9's counts, the limit line dropped, the §0 Intel
  check reworded: the guide's machine is STILL the Intel Air, it just
  builds both); BETA.md + BETA.fr.md (the aarch64 dmg line, "which
  Mac" via About This Mac > Chip, line 150/162); README; CHANGELOG
  0.19.0 entry; ADR 0036 decision 1 amended (two triples, one
  machine); DEBT D-62 struck; STATE.

**TDD note**: the job is scripts and configuration. The pure decision
that already had a unit-shaped guard (`patch-manifest.mjs`) needs no
change; `release-macos.sh` and `verify-release.ps1` have no test
harness in the repository (`bash -n` and PowerShell parse are the
gate's nets). A RED here teaches nothing; the proof is the replay
against the live release (E2) and the CI run (E3), stated as such.

Gate: full `/gate` on Windows (it parses the `.sh` and the `.ps1`);
CI green on all jobs; field = E0 on the Air now, then the tester's
first install at the 0.19.0 release (STOP 2 is split: the build proof
on the Air, the run proof by the tester).

## 4 bis. Delivery record

**E1-E4 implemented on 2026-09-05, one commit** (hash in STATE).
E1: `release-macos.sh` loops `TRIPLES` — both builds before the first
upload, six assets in one `gh release upload`, two `patch-manifest`
calls, one manifest re-upload; the arch and key words come from two
functions, not `declare -A` (macOS `/bin/bash` is 3.2 — a trap caught
at review before release day); the rustup target is checked in the
fail-fast block; the closing counts derive from the arrays. E2:
`verify-release.ps1` — two mac families from one `$macArchs` list,
the `darwin-aarch64` key ↔ assets consistency, the `$platforms` table
derived from the same list, and one `ResolvesWhole` helper serving
the updater artifacts AND both dmgs (the fresh-eyes review's catch:
the aarch64 dmg was the one asset no check covered). Replayed against
the live 0.18.0 (no mac assets): 20 PASS, the "not yet" path intact.
E3: `quality-macos` is a matrix over the two triples, one rust-cache
key per leg. E4: MACOS-BUILD (§4 target add, §7 the arm64 test build
+ `file`, §9 two prompts / 11 assets / 4 keys, limits), BETA +
BETA.fr (the aarch64 dmg line, the Chip line of About This Mac),
README, CHANGELOG 0.19.0, ADR 0036 header cross-link + **ADR 0037**,
DEBT D-62 struck.

Fresh-eyes review (`/code-review high`): ten findings, seven fixed
(the dmg gap, the shared helper, the triplicated table, the `.sig`
content check restored, a header comment, derived counts, the ADR
form), one deferred to STATE at close (its stale "8 assets/3 keys"
pointer), two refused with reason (the CI check rename — main has no
required checks, verified via the API; the twice-built ui-v2 dist —
minutes on a free public runner against a job dependency).

## 5. Field checklist (STOP 2)

**On the Air (Chief Engineer), before the commit** — E0 above: exit
status, wall time, `file` output.

**At the 0.19.0 release** — `release-macos.sh 0.19.0` uploads both
families; `verify-release.ps1 0.19.0` says 11 assets, 4 keys, every
signature distinct, every URL 200.

**By the Apple Silicon tester (the run proof)** — install the aarch64
dmg, the Gatekeeper gesture, add an account, read, send; Activity
Monitor > Kind column shows "Apple", not "Intel". The update path is
proven at the following release, as for x64.

## 5 bis. Field record (E0, the Air, 2026-09-06)

**The cross-link holds.** `cargo tauri build --target aarch64-apple-darwin`
on the Intel Air: cargo 13 min 00 s, 13 min 24 s wall
(2366 s user, 306 % CPU) — the x64 cold figure was 13 min 08 s; `file`
says `Mach-O 64-bit executable arm64`; the bundler wrote
`Wind_0.18.0_aarch64.dmg` — D3's word confirmed by the tool itself,
the dmg glob and the asset name agree. Ad-hoc signing ran on the arm64
binary as on x64. Build proof done; the run proof is the tester's.

## 6. Chief-Engineer decisions

| # | Question | Answer (date) |
|---|----------|---------------|
| D1 | Build the arm64 binary by cross-compiling on the Intel Air (one machine, one script, ~13 min more per release), rather than a universal binary or CI? | **"Cross on the Air"** (Chief Engineer, 2026-09-05) — `rustup target add aarch64-apple-darwin`, `release-macos.sh` loops over both triples. |
| D2 | CI: the mac job becomes a matrix over both triples (aarch64 native on the runner, real runtime coverage; mac minutes double, free on a public repo)? | **"Matrix, both triples"** (Chief Engineer, 2026-09-05) — x64 under Rosetta as today plus aarch64 native on the runner. |
| D3 | Asset name: `Wind_<v>_aarch64.dmg` (Tauri's bundler name, the manifest key) or `Wind_<v>_arm64.dmg` (the Windows asset's word)? | **`Wind_<v>_aarch64.dmg`** (Chief Engineer, 2026-09-05) — Tauri's bundler name, the manifest key word. |
| D4 | Vehicle: the aarch64 family ships in 0.19.0 (unreleased, its CHANGELOG entry amended), so the first mac release already carries both archs? | **"Yes, 0.19.0"** (Chief Engineer, 2026-09-05) — the CHANGELOG entry amended; the first mac release carries both archs. |
| D5 | E0 measurement: run the aarch64 test build on the Air now and report (status, time, `file` output) before the commit — or accept the cross-link on Tauri's word and measure at the release? | **"Yes, measure now"** (Chief Engineer, 2026-09-05) — E0 on the Air before the commit; result recorded in §5 bis. |

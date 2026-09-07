# Spike A02 — the global command lock under heavy work: A (as shipped) vs B

Measured on 2026-09-07 in an isolated worktree at commit `a58c1cf` (the
worktree was created before Lots 3 and 4 landed on the main tree; the lock
model — `state.commands` taken by `off_pump` — is identical at `2ade544`).
The comparison is internal to that commit. Throw-away; not gated.

Files: `harness.mjs` (Playwright over CDP, gestures invoked through the
UI's own transport), `prepare-fixture.py`, `summarize.py`, `option-B.patch`
(195 lines, `apps/desktop/src/commands.rs` only), `raw/run-A.json`,
`raw/run-B.json`, `raw/smoke-A.json`. Binaries, profile and the
`node_modules` junction were not copied.

## 1. Protocol

| Item | Value |
|---|---|
| Machine | Snapdragon X X1E80100, 12 cores, 15.6 GB RAM, NVMe SSD, Windows 11 Home 26200 (arm64) |
| Build | `cargo build -p wind-desktop --release`, aarch64-pc-windows-msvc, own target dir. A: 2 m 09 s, B: 1 m 01 s, 0 warnings. ui-v2 dist of 2026-09-07 17:14 |
| Fixture | `C:\mesure\banc200k.db` (200,000 envelopes in one INBOX of account 1, 500 cached bodies on uids 199,501–200,000, 160,000 threads). One fresh copy per run; the original never opened for writing. Two accounts in the file (id 2 has no mailbox) |
| Mutation | body of uid 199,696 replaced by a 10,485,662-byte synthetic newsletter HTML; a 25 MiB random file for `attach_files` |
| Launch | real window on the copy (`WIND_DB_PATH`), production browser args + CDP port, isolated WebView2 profile, OAuth vars purged; accounts offline |
| Timing | `performance.now()` around `window.__TAURI__.core.invoke`: IPC + pump + lock wait + work |
| Gestures | **open** = `thread_messages` then `message_body` on a row with a cached body (sum); **page** = `list_category(reception, offset (n+1)×200, limit 200)`; **pin** = `toggle_pin`. 30 reps, interleaved |
| Loads | **idle**; **sanitize** = `message_body` on the 10 MB body in a tight loop; **attach** = `attach_files(null, 25 MiB)` + `delete_draft` in a loop; **both** |
| Order | A: idle, sanitize, attach, both; then B identically on a fresh copy. 2 s of load before the first gesture, 6 s drain after |
| Cache | warm (fresh copy of a 166 MB file); identical for A and B |
| Competing processes | one foreign `wind-desktop.exe` (< 1 s CPU) present during both runs; no cargo, no other node during any measurement |

**Substitution, stated.** The real body backfill cannot be driven offline
(IMAP required; the fake server is test-only). More decisive: `backfill_bodies`
does not take `state.commands` — it holds `state.bodies_backfill` and touches
the commands lock only for `connected_jobs`; its contention with gestures is
SQLite writer contention (`busy_timeout` 30 s), which B leaves untouched. The
substitute load is the sanitize path that IS under the lock: `message_body`
→ `body_view` → `sanitize_with` (the D-1 site) on a 10 MB body.

## 2. Figures (ms, n = 30 per cell)

| Load | Gesture | A p50 / p95 / max | B p50 / p95 / max |
|---|---|---|---|
| idle | open | 7.5 / 15.3 / 27.5 | 9.7 / 25.4 / 27.4 |
| idle | page | 5.6 / 8.3 / 24.6 | 6.4 / 9.6 / 27.5 |
| idle | pin | 17.5 / 25.6 / 32.8 | 16.2 / 24.5 / 26.8 |
| sanitize | open | 465.5 / 691.6 / 774.3 | 12.0 / 68.2 / 250.2 |
| sanitize | page | 149.1 / 673.9 / 716.5 | 8.0 / 26.6 / 66.5 |
| sanitize | pin | 168.8 / 654.3 / 674.6 | 18.6 / 25.0 / 162.9 |
| attach | open | 858.7 / 1473.6 / 1721.5 | 845.6 / 1140.7 / 1758.9 |
| attach | page | 85.2 / 828.5 / 847.5 | 9.8 / 826.9 / 844.1 |
| attach | pin | 846.9 / 1645.5 / 1676.5 | 751.7 / 980.3 / 1074.4 |
| both | open | 1414.0 / 1922.4 / 2240.8 | 421.6 / 881.3 / 962.0 |
| both | page | 659.8 / 954.3 / 1461.9 | 23.0 / 293.8 / 497.0 |
| both | pin | 730.2 / 1127.0 / 1256.4 | 263.4 / 724.0 / 733.8 |

Heavy work per iteration as seen from the page, and iterations overlapping
the window:

| Load | Heavy op | A p50 / p95 / max · n | B p50 / p95 / max · n |
|---|---|---|---|
| sanitize | message_body 10 MB | 599.8 / 721.3 / 749.9 · 52 | 734.4 / 747.8 / 747.8 · 6 |
| attach | attach_files + delete_draft | 873.6 / 1576.5 / 2022.8 · 66 | 882.6 / 1084.4 / 1119.6 · 50 |
| both | message_body 10 MB | 767.6 / 1165.5 / 1459.9 · 110 | 847.5 / 1082.7 / 1090.2 · 27 |
| both | attach_files + delete_draft | 1442.1 / 2063.1 / 2239.4 · 58 | 440.8 / 911.6 / 1082.9 · 48 |

Zero load errors in either run; the 3-rep smoke run of A agrees with the
full run. Under the sanitize load, A's gestures wait about one sanitize
(p50 open 466 ms vs a 600 ms iteration); in B they stay within tens of ms
(max 250). Under the attach load, B changes little for open/pin: the 25 MiB
blob INSERT stays under the lock by definition and the part moved out costs
7.5 ms warm; `page` (a pure read) drops 85 → 10 ms p50 with the same p95.
B's small `n` under sanitize is because its gestures finished in about 2 s.

## 3. The diff applied for B (`option-B.patch`)

- `message_body` cached path: `off_pump` #1 = open store, `verify`, read
  body + image guard + attachment count + invitation → bare `spawn_blocking`
  = sanitize + document assembly → `off_pump` #2 = re-open and `verify`
  again (the re-validate). Network path unchanged.
- `attach_files` draft path: the E8 path check and `fs::read` on a bare
  `spawn_blocking`; the locked closure receives bytes and keeps only SQLite
  work. `edit_token` path unchanged.

## 4. Limits

1. Warm cache, fresh copy (STANDARD §9): a cold disk would lengthen the
   store opens in both, and A's `attach_files` critical section more than
   B's. Not measured cold.
2. The load is a substitute: the real backfill never holds the commands
   lock; its effect is SQLite-writer-bound and untouched by B.
3. Lock wait and SQLite-write wait are not separated: `toggle_pin` is a
   writer; its ~750 ms in B/attach could be either.
4. Twelve cores: B's unlocked sanitize ran on a free core; on a two-core
   machine it competes for CPU with the gesture — B/sanitize is an upper
   bound on the benefit.
5. Window length differs by construction; per-gesture n = 30 regardless.
6. A foreign Wind instance and OneDrive were resident; idle figures
   (5–18 ms) bound the noise floor.
7. One machine (arm64), one day, one run per variant plus a smoke run.
8. One mailbox in the fixture: the multi-account merge of `list_category`
   is not exercised.
9. Baseline commit `a58c1cf`, not `2ade544` (see the header).

## 5. Industrialization cost of B (estimate)

| Site (`commands.rs`, post-patch lines) | Heavy step | Needed |
|---|---|---|
| `message_body` cached | sanitize | done in the patch; re-verify ≈ one extra store open + `verify` |
| `message_body` network | sanitize | same split |
| `echo_body` | sanitize | split; re-validate the echo still exists |
| `feed_cards` | sanitize per card in a loop | batch outside, re-check page identity |
| `citation_reply`, `forward_context` | sanitize of the quoted body | split; re-validate `version` |
| `composition_boundary`, `prepare_composer_html` | sanitize of user content | pure functions; re-validate the draft's `updated_epoch` |
| `attach_files` draft | `fs::read` | done — the blob INSERT stays, B does not shorten it |
| `attach_edit_files` | `fs::read` | same hoist |
| `save_attachment` | `fs::write` | write outside after reading bytes under the lock |
| `install_downloaded` | `fs::write` | off the gesture path; optional |

Cross-cutting: a typed unlocked helper (owned inputs in, owned outputs out,
no `Store` reachable) instead of a hand-rolled `spawn_blocking`; the
re-validate discipline at every unlocked site or the TOCTOU the lock exists
for reopens; a product decision on fail vs retry-once on mismatch; an e2e
net that breaks if a sanitize lands back under the lock. About ten sites.
What B does not buy: anything SQLite-write-bound (the 25 MiB blob insert,
the backfill's writes, `toggle_pin` behind a writer) — a different option,
outside this spike's question.

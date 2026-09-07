# Spike A02 — the global command lock under heavy work

Throw-away spike (STANDARD §2.2-2.3) for option A02 of
PLAN-AUDIT-2026-09 lot 5 §3.1: does `state.commands: Arc<Mutex<()>>`
(`apps/desktop/src/main.rs:157`, taken by `off_pump` at
`commands.rs:5896-5909`) measurably delay interactive gestures while
heavy work runs?

- **A** = as shipped.
- **B** = file IO and HTML sanitizing moved OUT of the critical
  section; the lock covers only the SQLite read-decide-write pair
  (`option-B.patch`, applied to `apps/desktop/src/commands.rs` in the
  spike's worktree only).

The measurement and its protocol: [REPORT.md](REPORT.md). Raw figures:
`raw/run-A.json`, `raw/run-B.json` (+ the harness logs and the two
raw cargo logs).

## Files

| File | Role |
|---|---|
| `prepare-fixture.py` | one disposable copy of `C:\mesure\banc200k.db` per run, a ~10 MB synthetic body injected on uid 199696, the 25 MiB attach file |
| `harness.mjs` | launches the real window on the copy (`WIND_DB_PATH`), attaches over CDP, replays the three gestures 30× under each load through `window.__TAURI__.core.invoke`, writes the raw JSON |
| `summarize.py` | the A-vs-B table from the raw JSON files |
| `option-B.patch` | the diff applied for B |
| `bin/` | the two release binaries measured (gitignored) |
| `node_modules` | a junction to `e2e/node_modules` of the main checkout (playwright for CDP) — nothing installed, nothing modified there |

## Replaying

```powershell
# from spikes/global-lock, $S = a scratch dir outside OneDrive
python prepare-fixture.py C:\mesure\banc200k.db $S\run-A.db $S\attach-25MiB.bin | Out-File -Encoding utf8 $S\prep-A.json
node harness.mjs --variant A --exe bin\wind-desktop-A.exe --db $S\run-A.db --big-uid 199696 --body-uids $S\prep-A.json --attach $S\attach-25MiB.bin --out raw\run-A.json --reps 30
# same for B with bin\wind-desktop-B.exe and a FRESH copy
python summarize.py raw\run-A.json raw\run-B.json
```

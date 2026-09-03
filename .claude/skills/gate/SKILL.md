---
name: gate
description: Replay Wind's full gate (fmt, ui-v2 build + lint, contrasts, System coherence, main-thread guard, script syntax, language ratchet, IPC contract, markdown links, clippy, Rust tests, e2e) and report the raw facts. Mandatory before any commit — never the tests alone.
---

# /gate — the full gate, from the fastest to the slowest

The order is the one of the `.githooks/pre-push` hook (fail early), plus
the System coherence check played in CI. Run **everything**, report the
raw facts — figures, failure outputs — without softening.

**The full gate runs in ONE call** (fail-fast, figured verdict per step —
never the 13 commands as separate tool turns):

```
powershell -ExecutionPolicy Bypass -File scripts/gate.ps1
```

The 13 steps of the script, for a partial re-gate (then replayed one by
one, only the steps concerned):

```
cargo fmt --all -- --check
(cd apps/desktop/ui-v2 && npm run build && npm run lint)   # zero warnings; eslint no-undef
node e2e/contraste.mjs                          # WCAG pairs (A8)
node e2e/coherence-systeme.mjs                  # System ↔ system.css, value for value
node e2e/garde-thread-principal.mjs             # no blocking command on the pump (PLAN-GELS)
node --check e2e/*.mjs scripts/*.mjs            # + the PowerShell parser on every .ps1
node e2e/language-gate.mjs                      # French ratchet: no rise per file
node e2e/ipc-contract.mjs                       # generate_handler! == #[tauri::command] == appel('…')
node e2e/docs-links.mjs                         # every relative markdown link resolves
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets            # --all-targets is NOT decorative (examples/)
cargo test --workspace --doc
(cd e2e && npm test)                            # the real window, CDP WebView2
```

## Rules

- **A red = andon.** We stop, we fix. A clippy warning or a ui-v2 build
  warning is a red.
- **Partial re-gate after a fix**: replay the red step and what the fix
  can impact — if Rust moved, upstream too (fmt, clippy, Rust tests); if
  the UI moved, ui-v2 build, contrasts, coherence, e2e. The **final full
  gate before the commit is still due**, unchanged — the partial re-gate
  is only for the fixing loop.
- **After a sed/mechanical replacement, always replay `fmt`** — the red
  CI of 2026-08-14 comes from there.
- **Red E2E locally ≠ regression.** The suite flakes on this machine
  (WebView2 profile, OneDrive, load). Playwright retries once by itself
  (`retries: 1`): a test that comes out **"flaky" is recorded in the gate
  verdict**, as is — the gate stays green but the fact is stated. A
  frank red (two failures in a row): replay the **spec as a whole file,
  in isolation, ONCE** — never the full suite to settle a flake; if the
  doubt persists, `gh run list` — **the CI is the reference**. The known
  flake: the ghost draft (documented at commit 0956c85).
- **The gate's toolchain must be the CI's** (`rust-toolchain.toml` +
  pinned ref in `ci.yml`) — STANDARD §7.4.
- The final verdict is the list: each step, green or red, with its
  figures (number of tests, warnings, pairs, values).
- **Never a CI wait in the foreground**: `git push` (the pre-push replays
  the gate) and `gh run watch` run in the background; the session
  announces the verdict when it lands.

# ADR 0005 — The E2E gate lives outside hosted CI: pre-push hook

Date: 2026-07-21 · Status: accepted

## Context

Two defects shipped while announcing "clippy and tests green" escaped every
automatic check:

- the **SMTP port ignored** for a generic account (`relay()` hardwired an
  implicit TLS 465) — no test exercises a network adapter;
  [since: `mail-smtp` now has two, which reach a fake server on an
  ephemeral port — the gap was narrower than it looked];
- the **add-account menu displayed permanently** (`#add-menu`, an ID
  specificity overriding `[hidden]`) — invisible without driving the UI.

Both were catchable by the existing E2E suite ([`e2e/`](../../e2e/README.md)),
which drives the real window over CDP. The only thing missing was the
**obligation** to run it. Hence the aimed-for countermeasure: an automatic
E2E gate.

## The hypothesis tested, then killed

Initial hypothesis: run the E2E suite in GitHub CI (`windows-latest`), as a
required check. **Three measured runs refuted it.**

| Run | Countermeasure tried | Result |
|---|---|---|
| 1 | Bare E2E job | `CDP unreachable on port 9222 after 30 s`, with no clue at all (the app was launched with `stdio: 'ignore'`) |
| 2 | App output captured, wait raised to 90 s, waiting for the **page** rather than the port | `CDP unreachable after 90 s` · **`(the application wrote nothing to its output)`** · process **not dead** |
| 3 | Verification + installation of the WebView2 runtime | **`WebView2 present, version 150.0.4078.65`** · same failure |

Facts established, by elimination:

1. the binary builds and launches — the process **lives** for 90 s;
2. the **WebView2 runtime is present** on the runner;
3. the window **never** initializes, with no error, no exit code, not a
   single byte on stdout/stderr;
4. it is neither slowness (90 s), nor the WebView2 profile, nor a race
   between the CDP port and page creation — all three were fixed and
   ruled out.

What remains is the structural cause: **a hosted GitHub runner does not
offer the interactive desktop session WebView2 needs to create a window.**
This is not fixable by configuration.

## Decision

**The E2E suite does not run in hosted CI.** It is played by a versioned
Git **`pre-push`** hook ([`.githooks/pre-push`](../../.githooks/pre-push)),
enabled by `git config core.hooksPath .githooks`.

The hook runs the full gate, fastest to slowest: `cargo fmt --check`,
`cargo clippy -D warnings`, `cargo test --workspace`, then the 10 E2E
journeys. If it passes, CI is green by construction.

Hosted CI keeps what it is reliably good at: `quality` (fmt + clippy +
tests) and `audit` (CVE).

### Why not a "informational" red E2E job

An andon that screams permanently stops being an andon: it stops being
watched, and the day it flags a real defect, nobody hears it. A durably
red job is worse than no job.

### Why not (yet) a self-hosted runner

This is the rigorous solution: the gate would stay **in CI and blocking**,
not bypassable. It is set aside for now under the rule of right-sizing —
on a single-developer project, it adds an agent to install and maintain,
and requires the machine to be on, to guard against a bypass
(`--no-verify`) that this same developer would have to inflict on
themselves voluntarily. **To be revisited as soon as a second contributor
arrives**: that is where the weakness becomes real.

## Consequences

- The hook is **versioned**, hence shared: a new machine only has to run
  `git config core.hooksPath .githooks` (documented in
  [`e2e/README.md`](../../e2e/README.md)).
- [`.gitattributes`](../../.gitattributes) forces **LF** line endings on
  `.githooks/**`: in CRLF, the shebang breaks ("bad interpreter").
- **Weakness accepted and named**: `git push --no-verify` bypasses the
  gate, and it only protects the machines that have enabled it.
- The fixes made to the harness during the investigation are kept and
  useful locally: application output captured and echoed back on failure,
  process-death detection, waiting for the page rather than the port (a
  real race, which the warm start had been masking).
- **Switch if the context changes**: self-hosted runner (blocking gate
  regained) as soon as a second contributor or a public release justifies
  it.

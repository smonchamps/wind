# ADR 0032 — One TLS stack: rustls with the platform verifier

Date: 2026-09-04 · Status: accepted (PLAN-AUDIT-V3 E1, Chief-Engineer decision D2)

## Context

The full audit of 2026-09-01 found **three TLS stacks in one Windows
binary**: `native-tls` for IMAP (SChannel — the Windows certificate
store), `rustls` with webpki roots for SMTP (via lettre) and OAuth (via
oauth2/reqwest), and `rustls-platform-verifier` in the Tauri updater.
Concrete failure: a corporate CA installed in the Windows store works
in IMAP and fails in SMTP/OAuth — the same account half-connects on a
corporate network. The audit left the stack choice to the Chief Engineer
(decision #6).

## Options — settled at the spike (`spikes/tls-stack/`, 2026-09-04)

| Option | Corporate CA | Stacks | Verdict |
|---|---|---|---|
| A. native-tls everywhere | yes | 1 (+ updater's rustls) | fallback |
| **B. rustls + rustls-platform-verifier everywhere** | yes | **1** | **retained — proven feasible as-is, no version bump** |
| C. status quo | IMAP only | 3 | the finding itself |

Spike facts, at the pinned versions: lettre 0.11.22 exposes the hook as
the cargo feature `rustls-platform-verifier` (the Default certificate
store then routes through the platform verifier); oauth2 5.0.0's
executor accepts a reqwest client built with `use_preconfigured_tls`;
the imap crate is generic over any `Read + Write` stream, so
`rustls::StreamOwned` drops in where `native_tls::TlsStream` sat.
9/9 live handshakes green (imap.gmail.com:993, smtp.gmail.com:465,
accounts.google.com), certificate validation by the Windows store.

## Decision

- **One TLS stack: rustls 0.23 + rustls-platform-verifier** — the
  Windows certificate store is the verifier everywhere (IMAP, SMTP,
  OAuth), exactly as the updater already does. `native-tls` leaves the
  workspace; no OpenSSL-adjacent code remains
  (only `openssl-probe`, a path-probing helper with no OpenSSL linked).
- **`ring` as the SOLE crypto provider** (`rustls` with
  `default-features = false`): rustls's default would add aws-lc-rs
  next to the tree's existing ring, and with two providers rustls's
  auto-selection **panics at the first handshake** — measured at the
  spike.
- mail-imap builds the TLS stream by hand (`tls_stream()` in
  `lib.rs`), handshake driven to completion at connect under the socket
  timeouts: a certificate refusal surfaces at `connect_client`, not at
  the first command. The hand-built STARTTLS path is unchanged.
- The net: `the_workspace_ships_one_tls_stack` (mail-imap, proven RED
  before the swap) — the lockfile must contain
  `rustls-platform-verifier` and must not contain `native-tls` or
  `openssl-sys`.

## Named limits

- Two reqwest majors still coexist (0.12 for oauth2, 0.13 for the
  updater) — same rustls 0.23 under both, so one TLS stack holds; the
  duplicate reqwest is a dependency-hygiene subject, not a TLS one.
- webpki-roots remains in the lockfile (named by reqwest's default
  rustls path) but is bypassed at runtime by the preconfigured client.
- Behavior under a TLS-inspection middlebox is untested; the platform
  verifier is precisely what makes such roots work when IT installs
  them.
- Live STARTTLS on port 143 was not exercised at the spike (Gmail is
  993); the rustls stream slots into the unchanged hand-rolled STARTTLS
  code, and the existing timeout/behavior tests cover the path.

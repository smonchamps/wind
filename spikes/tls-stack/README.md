# Spike TLS-STACK — one TLS stack for Wind? (PLAN-AUDIT-V3 E1, D2)

Throw-away spike, outside the production workspace. Question: can the
three TLS users (mail-imap, mail-smtp, mail-auth) all run on
**rustls + rustls-platform-verifier** (Windows cert store as verifier —
the updater's existing stack)? Proof by compilation at the pinned
versions, then a live handshake test.

## Pinned versions (production `Cargo.lock`, 2026-09-04)

| Crate | Version | TLS today |
|---|---|---|
| lettre | 0.11.22 | rustls + **webpki roots** |
| oauth2 | 5.0.0 | reqwest 0.12.28, rustls + **webpki roots** |
| imap | 3.0.0-alpha.15 | native-tls 0.2.18 (stream built by hand in `crates/mail-imap/src/lib.rs`) |
| rustls | 0.23.41 | one single 0.23 in the whole tree |
| rustls-platform-verifier | 0.7.0 | already present — pulled by reqwest **0.13.4** (Tauri updater) |

## Per-crate verdict (compiled, not read)

1. **lettre 0.11.22 — hook exists: YES, as a feature flag** (not a
   caller-supplied `ClientConfig`: `InnerTlsParameters` is `pub(crate)`,
   no constructor takes a config). Enabling the cargo feature
   `rustls-platform-verifier` (the implicit feature of lettre's optional
   dependency `rustls-platform-verifier = "0.7"`) makes
   `TlsParametersBuilder::build_rustls()` route
   `CertificateStore::Default` through
   `rustls_platform_verifier::Verifier::new_with_extra_roots`
   (lettre `tls.rs:540-548`). No version bump needed. Compiles and
   constructs at runtime:
   `TlsParameters::builder(domain).build_rustls()`.
2. **oauth2 5.0.0 / reqwest 0.12.28 — hook exists: YES.**
   `reqwest::blocking::ClientBuilder::use_preconfigured_tls(rustls::ClientConfig)`
   (feature `rustls-tls`) accepts a config built with
   `rustls::ClientConfig::builder().with_platform_verifier()?`, and
   `oauth2::SyncHttpClient` is implemented for that
   `reqwest::blocking::Client` (proven by a generic-bound assertion that
   compiles). Caveat: `use_preconfigured_tls` takes `impl Any` — a rustls
   major mismatch between the app and reqwest surfaces at **runtime**,
   not compile time; the live GET proves 0.23/0.23 matches today.
3. **imap 3.0.0-alpha.15 — hook exists: YES, trivially.**
   `imap::Client::new` is generic over any `Read + Write` stream;
   `rustls::StreamOwned<ClientConnection, TcpStream>` drops in exactly
   where `native_tls::TlsStream<TcpStream>` sits today in
   `connect_client` (`crates/mail-imap/src/lib.rs:82-118`). Built with
   `default-features = false`: the spike's lockfile contains **no
   native-tls at all**.

## rustls version alignment

- **No duplicate rustls major**: production lock has exactly one rustls
  (0.23.41); the spike resolves one (0.23.43). lettre pins
  `rustls 0.23.18+`, rustls-platform-verifier 0.7 wants 0.23 — aligned.
- **Two reqwest majors coexist** in production (0.12.28 for oauth2,
  0.13.4 for the Tauri updater). Both sit on rustls 0.23, so no TLS
  conflict — but it is a duplicate-dependency finding in its own right.
- **Crypto provider trap (measured, it bit the spike)**: a direct
  `rustls = "0.23"` dependency with default features pulls **aws-lc-rs**
  next to the tree's **ring** (0.17.14, the only provider in the
  production lock); with two providers, rustls's auto-selection panics
  at the first handshake. Fix: `default-features = false,
  features = ["ring", "std", "tls12", "logging"]` — after which
  everything runs on ring with no `install_default()` needed.
- webpki-roots stays in the tree (lettre's `rustls-tls` feature and
  reqwest's default rustls path both name it) but is bypassed at
  runtime; lettre could drop it by using features
  `["rustls", "rustls-platform-verifier"]` instead of `"rustls-tls"`.

## Live test

Protocol: this x64 Windows 11 workstation (the dev machine), home
network, 2026-09-04; debug build; 3 repetitions per target, sequential,
one process; TCP connect and TLS handshake timed separately where the
stream is hand-built; no auth, greeting/HTTP-200 read only. Certificate
validation is the platform verifier (Windows cert store) in all three.

| Target | run 1 | run 2 | run 3 | Result |
|---|---|---|---|---|
| imap.gmail.com:993 handshake | 53 ms | 13 ms | 13 ms | greeting OK, 3/3 |
| smtp.gmail.com:465 handshake | 24 ms | 11 ms | 14 ms | `220 smtp.gmail.com ESMTP`, 3/3 |
| GET accounts.google.com openid-configuration (total) | 41 ms | 10 ms | 11 ms | HTTP 200, 1399 bytes, 3/3 |

Run 1 of each series carries first-use costs (ring init, session-cache
empty, OS cert-store first query); runs 2-3 are the steady state.

What would invalidate these figures: another network (proxy, TLS
inspection middlebox — the platform verifier is precisely what makes
corporate roots work, but it is untested here); a Windows machine with a
damaged cert store; providers other than Gmail (only Gmail endpoints
were probed); release-build timings (this is debug — absolute times are
upper bounds).

## Limits

- lettre's platform-verifier path was proven by constructing
  `TlsParameters` and reading lettre's source for the wiring; the live
  465 handshake used the same verifier config through raw rustls, not
  through a full lettre transport send (no auth, no mail).
- STARTTLS on 143 was not exercised live (Gmail path is 993); the
  rustls stream slots into the existing hand-rolled STARTTLS code
  unchanged, but that path's live proof is pending.
- `BoundedStream`/`InnerSocket` in mail-imap needs a
  `socket()` impl for `rustls::StreamOwned` (it is `get_ref()`-shaped,
  same as native-tls) — mechanical, not proven here.

## Estimated industrialization cost

Small. mail-smtp: one feature added to the lettre line in the root
`Cargo.toml`. mail-auth: build the reqwest client with
`use_preconfigured_tls` and pass it as the oauth2 executor (the hook
oauth2 5 already exposes). mail-imap: replace the
`native_tls::TlsConnector` block in `connect_client` with
`ClientConnection`/`StreamOwned` (~20 lines incl. the `InnerSocket`
impl), drop `native-tls` from the workspace. Plus the rustls dependency
line with `default-features = false` + `ring` (the provider trap above)
and the timeout/reconnect test suite of mail-imap replayed as-is.

## Verdict

**B feasible as-is** — no version bump needed: lettre 0.11.22 (feature
`rustls-platform-verifier`), oauth2 5.0.0 + reqwest 0.12.28
(`use_preconfigured_tls`), imap 3.0.0-alpha.15 (generic stream), one
rustls 0.23, ring as sole provider. 9/9 live handshakes OK on the
Windows cert store.

## Rerun

```
cd spikes/tls-stack
cargo run            # compilation proofs only
cargo run -- --live  # + live handshakes (network required)
```

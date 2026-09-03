# ADR 0025 — OAuth credentials compiled into the release binary

Date: 2026-08-23 · Status: accepted (PLAN-RETOURS-9, CE decision D1)

## Context

Wind read `GOOGLE_CLIENT_ID`/`GOOGLE_CLIENT_SECRET`/
`MICROSOFT_CLIENT_ID` **at runtime** (`std::env::var`, a single point:
`Authenticator::from_env`). Field finding of 2026-08-23 (second x64
workstation): on a workstation without `setx`, sign-in fails with a
developer-facing message. A beta tester will never run `setx` — to fix
before the closed beta.

## Options

| Option | Verdict |
|---|---|
| **Compiled at release build** via `option_env!` on DEDICATED names `WIND_RELEASE_*` | **Retained.** Zero user gesture; clean public repository (the values never enter it); a dev build embeds nothing; the practice of mature clients — native app client ids are not secrets in the strict sense. |
| Config file shipped alongside the exe | Rejected: one more piece to sign/distribute, modifiable, carried by NSIS. |
| Documented status quo | Rejected: that is the finding itself. |

## Decision

- Two **data** fields on `Provider` (`embedded_client_id`,
  `embedded_client_secret`), filled by
  `option_env!("WIND_RELEASE_{PREFIX}_...")`. Microsoft **never**
  embeds a secret (public client, ADR 0006).
- **The runtime variable takes priority** (`resolve_credential`:
  runtime → embedded → error; an empty variable does not count) — dev
  workstations and e2e isolation (variable purge, `isolation-oauth.json`)
  keep their lever.
- `WIND_RELEASE_*` are set ONLY by `make-release.ps1`, **for the sole
  duration of the two builds**, removed in `finally` — the fresh-eyes
  review showed that, left in the process, they made the
  `dev_builds_embed_no_credentials` test fail red in the pre-push gate
  of the final push: the release would have blocked itself, and the
  gate's debug binary would have embedded the credentials. Presence of
  all three values verified BEFORE the builds (all-or-nothing, ADR
  0023's D7).
- Guard: `dev_builds_embed_no_credentials` (provider.rs) — every
  dev/test/CI build must prove `embedded_* == None`.
- Failure message rewritten for both readers: "OAuth credentials
  missing from this binary — install an official Wind release; in
  development, set {VAR}."

## Named limits

- The script's `$oauth` table duplicates the `option_env!` calls of
  `provider.rs` (DEBT D-34): a provider added on the Rust side must
  also be added on the script side, or its release ships without a
  credential.
- ~~**Field proof deferred**: the next release (0.8.0) must connect an
  account on a workstation WITHOUT `setx` — that is what closes the
  arbitration.~~ **DONE on 2026-08-25**: an account connected on the
  second workstation from a published release, without any `setx` —
  the arbitration is **closed**. The proof slipped by two versions (it
  was expected at 0.8.0, it came after 0.9.0): the decision itself
  stands as it was made.

# Lot 3 E8 reconnaissance (2026-09-06)

Baseline: a58c1cf. Read-only production investigation; no real account, vault, credential, mail database or external network used. No tests run, so the statements below are static findings, not claimed RED measurements. WORKFLOW/job, STANDARD and ADRs 0006/0030/0032 consulted. Report is scratch, not a delivery document.

## Current findings and narrow remedies

### B17: legacy refresh rotation still lost

`crates/mail-auth/src/lib.rs:191-232`: authenticate_silent computes a rotated refresh token, passes it to finish (which persists it), then the from_legacy branch writes the OLD `refresh` at lines 225-228 over the new token. `authenticate_silent_legacy`, lines 237-251, calls finish with no refresh and unconditionally persists the old token. Both paths still fail the audited invariant.

Remedy: compute one effective token (`response.refresh_token` or existing token), resolve/verify identity, persist this token once to the final account entry, and only then remove the legacy entry. Normal nonlegacy exchange with no rotation need not rewrite. Keep vault key/service strings frozen. Restrict the unkeyed Gmail legacy fallback to its actual provider; Microsoft should not try a Gmail legacy token. Preserve the Discovery-to-Wind bridge's copy-before-delete ordering.

Introduce an injected vault port (read/write/delete, memory fixture with recorded order/failpoints) used by the production orchestration, not just a separate pure helper that mirrors it. Token exchange/identity can be injected or a loopback provider fixture can be used. Avoid tests calling the existing ignored OS-vault bridge.

Meaningful RED wave:
- Per-account missing + legacy old + exchange returns rotated => final per-account value rotated, legacy removed, exactly one final write.
- Empty registry legacy adoption + rotated => same.
- No rotation => old migrated correctly.
- Final vault write fails => legacy survives; no published session.
- Legacy delete fails => current entry still contains newest token; next launch prefers it.
- Identity/scope/exchange failure => no legacy deletion.
- Provider/identity mismatch cannot overwrite another account under a repair gesture.

Scope note: after a remote provider rotates and a local vault write fails, no local algorithm can guarantee the old token remains valid remotely. Report the persistence error and retain local source; do not promise restart recovery that cannot be proven. Avoid adding a second plaintext token journal.

### B18: manual browser URL points at a dropped listener

`crates/mail-auth/src/flow.rs:113` binds the listener; lines 132-134 return BrowserFallback when opening the browser fails. All stack-owned consent state (listener, PKCE verifier) is dropped. `apps/desktop/src/commands.rs:383-390` stringifies the error; `AccountDesk.svelte:61-72` only displays it. No pending consent object or resume command exists.

Remedy: a consent session owns listener, PKCE verifier, CSRF state, deadline and cancellation; automatic open failure becomes a status update carrying the same URL rather than the end of the session. Offer a copy/open link and Cancel while the original worker continues waiting. Never restart a separate listener for the original URL. Publish a flow id/status so stale UI events cannot complete or cancel another consent. Final resolution consumed once. Keep URLs out of logs (auth URL contains flow secrets/state); structured status rather than `AuthError::BrowserFallback(url)` stringification.

Smallest compatible implementation can keep the current blocking worker and inject a notification/cancellation port, with a shell-owned flow id and status. Splitting begin/wait is also viable, but must retain a single owner. No need to replace oauth2 or provider descriptors.

Tests: injected browser opener fails, URL observed, real loopback callback succeeds once on the SAME port; cancellation releases listener/registration; timeout releases them; duplicate callback cannot exchange again; reopening/closing Settings does not abandon a hidden forever-running consent. Whole AccountDesk/reconnect E2E files exercise user copy/cancel and stale flow identity with synthetic IPC states.

### B19: callback deadline and validation still incomplete

`flow.rs:223-269`: deadline checked only after WouldBlock in accept (233); a continuously supplied connection bypasses it. `read_line` (246-249) has unbounded allocation and a per-read 2 s timeout, so a trickle resets that bound. `respond` (284-290) has no write timeout and propagates browser-side errors into the whole flow. Wrong state is checked only AFTER wait returns (137-138), so a wrong callback consumes the listener. Error query (257-259) aborts without checking state. `parse_redirect_query` (274) does not require GET/root callback path and silently resolves duplicate keys through HashMap collection.

Remedy: single absolute deadline checked at every accept/read/write iteration; bounded request-line bytes; short per-connection work budget bounded by remaining consent time; read and write timeouts; validate method/path/unique parameters and expected state inside the accept loop. Ignore malformed, unrelated, wrong-state or disconnected clients, including false error callbacks. State-matching provider refusal ends the flow. HTTP acknowledgement failure after a valid code should not discard a successfully validated callback. Keep loopback-only bind, PKCE, redirect host provider policy and 300 s consent timeout already approved in earlier plan.

Suggested implementation constants to validate rather than present as measurements: 8 KiB request-line cap, 1-2 s per-client budget, <=100 ms cancellation polling. A short per-client deadline prevents one trickling connection from occupying all 300 s; a global deadline alone would still deny legitimate callbacks during the attack.

RED fixtures (loopback, synthetic strings): a 200 ms consent budget with an accepted silent/trickling peer returns close to that budget; continuous garbage acceptance does not extend it; >8 KiB line is rejected and a following valid callback succeeds; wrong-state code and wrong-state error precede a correct callback; truncated line/client reset doesn't abort valid flow; duplicate state/code rejected; cancelling releases listener. Avoid five-minute tests and unbounded writer threads. Existing tests at flow.rs:310 and :329 cover idle timeout and one mute connection only; they do not cover these paths.

### B24: generic repair is absent in UI, although core UPSERT already preserves id

`commands.rs:445-448` explicitly tells generic users to remove/re-add. Settings :731-737 calls this same OAuth command for every disconnected account. There is no account configuration read/edit IPC. `AccountDesk.svelte:86-96` always sends username null; backend supports username but the UI cannot supply it.

Important precision: `Store::create_generic_account` (`crates/mail-core/src/store.rs:1073-1106`) already UPDATEs configuration on email conflict and returns the existing id. A cross-connection test around store/tests.rs:2530 proves id/config preservation on re-add. Therefore B24 is not a missing SQL capability and a destructive migration is unnecessary. A new explicit update-by-account-id API should reject missing/non-generic accounts and immutable email changes; it must not silently convert an OAuth row using the current UPSERT's `provider = imap`.

Remedy: Settings generic row offers a compact connection editor, prefills server/port/username from nonsecret config, keeps password blank (blank can explicitly mean unchanged), and validates both protocols before committing. Account id, email, drafts, edit snapshots, attachments, outbox Message-IDs, preferences and pending intentions remain attached. Clear password from the UI on save/cancel/close; never round-trip the stored password to the webview. Do not allow a changed field to be submitted while the verification of a previous snapshot is completing.

Lifecycle requirements:
- Capture existing account and Ticket under `off_pump` commands lock; retain its Lease through probes and persistence. Register new/unknown-identity flows as Lot 2 already does.
- Final publication under commands lock checks current Ticket and account provider/id. A concurrent account retirement cancels admission; no late session/vault resurrection.
- Current Registry only invalidates jobs on removal. Merely replacing `state.accounts[email]` leaves existing watcher/jobs holding old credentials. A repair must stop/drain relevant old work and rotate the generation, preserving already-started SMTP decisions. Reuse/refine the Lot 2 generation mechanism rather than key only by account id or email. Never wait for the caller's own lease to drain; do not hold a Registration while invoking current Retirement.wait (it waits for ALL registrations, causing self-deadlock).
- Keep the old usable config/session on failed probes. Vault and SQLite are not atomic together: preserve/compensate old password if config persistence fails; if rollback fails, retain data and surface an explicit recovery error. No transient success before final persistence.

Endpoint identity is a real design issue: changing IMAP host/username can point the existing account at a different mailbox; equal UIDVALIDITY across independent servers does not prove identity. Existing pending UID actions/draft source references must not be automatically replayed there. Choose a same-mailbox repair policy versus explicit server migration/reconciliation. Merely retaining the SQL id is insufficient safety. Password/SMTP-only repair does not require remote UID reassignment. This needs a stated plan decision, not a silent implementation assumption.

RED fixtures: synthetic file DB with draft + files + queued/interrupted sends + pending action, edit session and prefs; repair keeps all ids/counts/content. Probe failure leaves config/vault/cache unchanged. Late completion after retirement cannot write; two simultaneous repair attempts cannot overwrite in reverse order; watcher using old Ticket cannot resume or replace current session; account B unaffected; wrong provider/unknown id rejected. Core existing UPSERT test alone does not prove these shell/vault paths.

### B25: setup still tests IMAP only

`commands.rs:571-587` connects/logs out IMAP and writes the password; SMTP host/port at 567-568 are only persisted, never probed. The UI says checking, then considers the account added. `crates/mail-smtp/src/lib.rs:87-115` ALREADY provides password authentication followed by `transport.test_connection()`; no send call is necessary.

Remedy: invoke both existing IMAP and SMTP authentication probes independently against the submitted snapshot, with bounded timeouts, then return per-protocol results. No MAIL FROM/RCPT/DATA or IMAP mailbox mutation. For default all-or-nothing admission, persist nothing unless both succeed. Probe SMTP even when IMAP fails if the intended UI promises independent diagnostics. Close/drop test transports. Existing TLS mode policy (465 implicit, other SMTP ports STARTTLS; IMAP connector's policy) and platform certificate verification must remain intact.

Meaningful RED wave uses injected probes + memory vault + synthetic Store to show SMTP failed after IMAP success is not accepted; reverse outcome reports both protocols; neither result writes config or secrets; both pass publish once. A loopback wire fixture records that setup emits authentication/NOOP/logout only and contains zero delivery commands. Existing mail-smtp tests already contain transport witnesses and custom TLS fixtures; reuse a test-only connector seam rather than disabling production certificate validation.

## Product decisions actually needed

1. D6 received from parent, verbatim CE (2026-09-06): « Exiger les deux connexions avant d’enregistrer la configuration (recommandé). Une réparation échouée conserve l’ancienne configuration. » Settled: both independent probes must pass; no receive-only addition. Failed verification must publish neither configuration nor credential. Stage the submitted config/password in memory during probes; old persistent state remains untouched. After both pass, under the final account-generation guard, compensate the vault if SQLite publication fails. Cross-resource crash atomicity cannot be honestly claimed by compensation alone: if the requirement includes power loss between writes, store a versioned vault credential candidate first and commit a nonsecret credential-version reference with config in one SQLite transaction, retaining the previous referenced credential until successful cleanup. This needs an explicit migration preserving the existing generic-password:email read path, never a plaintext password in SQLite. A mere write-vault-then-SQLITE sequence with best-effort rollback does not fully guarantee D6 on persistence failure/crash.
2. Generic IMAP endpoint/username changes: decide whether Lot 3 repairs only the same mailbox (recommended bounded scope with explicit migration deferral) or also supports mailbox migration with pending-action reconciliation. UI wording and safety must match this decision.
3. New repair panel and manual browser fallback controls require a study prototype/early visual verdict under job skill. Geometry can reuse Settings inline cards; no new icon needed. Flow expiration stays the previously approved 5 minutes; cancellation is part of the audited recommendation, not a fresh product debate.

## Verification boundary

No tests were executed during reconnaissance and no production file was changed. Existing mail-auth ignored vault test must remain unrun. No live OAuth consent, external network or delivery is needed for maintained RED/GREEN proof. Real credential repair/consent remains STOP 2 field evidence owned by the user.

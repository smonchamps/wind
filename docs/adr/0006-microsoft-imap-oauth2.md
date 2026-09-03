# ADR 0006 — Microsoft 365: IMAP+OAuth2 confirmed, Graph as a priced plan B

Date: 2026-07-21 · Status: accepted

## Context

Phase 0 had explicitly **deferred** this decision to Phase 3
([PHASE0.md](../archives/PHASE0.md) §3), the plan's set-based grid
([PLAN.md](../PLAN.md) §2.4) setting *Graph API* as the Chief Engineer's
hypothesis, on elimination criteria: **reliability, quotas, effort**.

Two facts had to be established before deciding — neither could be settled
by reasoning:

1. **Is IMAP+OAuth2 still supported?** The argument that would have forced
   Graph was "IMAP is doomed".
2. **Is SMTP AUTH open?** Without it, the golden rule "never a lost send"
   ([ADR 0003](0003-smtp-outbox.md)) has no support on the Microsoft side
   left, and sending would have to go through Graph.

## Research: the "IMAP is doomed" argument is false

Microsoft has never deprecated the *protocols* — only **Basic Auth**
(completed end of 2022 for IMAP/POP/EAS). What is moving in 2026 concerns
only SMTP AUTH **over Basic Auth**: disabled by default end of December
2026, final retirement announced for H2 2027. For new tenants, **OAuth is
explicitly the recommended method**.

Sources: [Deprecation of Basic authentication in Exchange Online](https://learn.microsoft.com/en-us/exchange/clients-and-mobile-in-exchange-online/deprecation-of-basic-authentication-exchange-online) ·
[Updated SMTP AUTH Basic Authentication Deprecation Timeline](https://techcommunity.microsoft.com/blog/exchange/updated-exchange-online-smtp-auth-basic-authentication-deprecation-timeline/4489835)

## Measurements — real Outlook.com account ([`spikes/microsoft`](../../spikes/microsoft/README.md))

| Measurement | Result |
|---|---|
| Scopes **granted** | `IMAP.AccessAsUser.All` + `SMTP.Send` — no partial consent |
| Refresh token | received → silent reconnection possible |
| IMAP XOAUTH2 connection | 389–551 ms |
| LIST (41 folders) | 54–144 ms |
| **SMTP AUTH** (`smtp.office365.com:587` STARTTLS) | **OPEN**, 0.8–1.2 s |

## Decision

**IMAP+OAuth2 is confirmed** for Microsoft 365 / Outlook.com in v1; **Graph
stays the plan B**, documented and priced.

The tie-breaking rule is the one from [ADR 0004](0004-fts5-search-engine.md):
the alternative must beat the hypothesis **clearly**. Here, Graph does not —
its one decisive advantage ("IMAP is doomed") is refuted, and the effort
asymmetry is overwhelming:

| | IMAP+OAuth2 | Graph |
|---|---|---|
| Sync engine, outbox and its golden rules, drafts, storage | **reused with zero new lines** | to rewrite against REST |
| Adapters | already parameterized per host (`connect_xoauth2(host, port, …)`) | new `MailServer` + `MailTransport` adapter |
| Remaining work | ~~endpoints/scopes per provider, hosts per account~~ → **done**; the add-account UI remains | pagination, delta, quotas, a whole new model |

### The authentication layer, generalized

`GmailAuth` was a name that lied: the class carried Google hardcoded in its
constants, and the application its servers in its own. The flow is now
**unique**; what distinguishes a provider is described as **data** in
`mail-auth::provider` — endpoints, scopes, the consent-verification rule,
the redirect host, the client-secret policy, the identity strategy, the
IMAP/SMTP servers.

Three choices are worth freezing here:

- **The descriptors are tested against the spike**, not against the docs.
  The Microsoft values in the code are the ones a real account actually
  accepted.
- **Three identifiers are pinned by tests**: the vault key
  (`gmail-refresh:`), the database value (`accounts.provider = "gmail"`)
  and their uniqueness across providers. None of these renames breaks
  anything at compile time; all of them would silently disconnect existing
  accounts.
- **Microsoft does not deliver account identity** within the measured
  scope perimeter: `Identity::Declared`, the address is entered by hand.
  The `openid profile email` + `graph.microsoft.com/oidc/userinfo` path is
  documented in the code but **not measured** — hence not adopted.

### The two traps, frozen here

1. **The scopes are those of the Outlook RESOURCE**, not Graph's short
   names — `https://outlook.office.com/IMAP.AccessAsUser.All` and
   `https://outlook.office.com/SMTP.Send`, plus `offline_access`.
2. **SMTP is on 587 STARTTLS**, never implicit 465. ~~The XOAUTH2 path of
   `mail-smtp` still hardcodes 465.~~ **Closed out**: both authentication
   modes now go through a single path (`transport_builder`), and two
   offline tests prove the requested port is the one actually reached. The
   duplication that had left the `fb11538` fix benefit only the password
   path no longer exists.

## Unexpected consequence: archiving

Exchange announces `\Drafts`, `\Junk`, `\Sent` and `\Trash`, but **neither
`\Archive` nor `\All`** — even though the `Archive` folder exists and is
used (13 subfolders on the measured account). The anti-destruction
guardrail of [`e37a105`](../../crates/mail-imap/src/convert.rs) would
therefore have **refused** to archive on any Microsoft account — correct
behavior, but the feature unavailable.

Hence a **by-name fallback** (`archive`, `archives`) after the announced
attributes: a deliberate exception to the "never a hardcoded name" rule,
**justified by measurement and by measurement alone**. Frozen priority
order: `\Archive` → `\All` → known name → refuse.

## Field validation (2026-07-21, real Microsoft account)

The full journey was played **from the application**, not from the bench.
All five points pass:

| | Verified |
|---|---|
| Add account | browser consent, address declared, badge shown |
| Sync | Outlook messages come up in the unified inbox |
| Backfill | the banner resumes on the new account |
| Reconnect | **silent** on relaunch — the vault's refresh token holds |
| Send | **a real message goes out over 587/STARTTLS** |

The last point matters most, and it could not have been obtained any other
way. Bug #3's fix had only been proven **against a fake server**: the
tests show which port is reached, never that a message actually leaves.
It is field validation, and field validation alone, that closes this loop
— exactly the role assigned to it.

The silent reconnection validates, in passing, two decisions made without
direct measurement: `offline_access` alone is indeed enough to obtain a
refresh token on the Microsoft side (where Google requires
`access_type=offline` + `prompt=consent`), and the disjoint vault prefixes
let providers coexist.

**Not resolved**: `Identity::Declared`. The entered address worked, which
says nothing about the OIDC path — it remains unmeasured, hence not
adopted.

## Consequences

- ~~Productionization follows: generalize the per-provider OAuth layer
  (`GmailAuth` is frozen on Google), pull the hosts out of `commands.rs`'s
  constants, fix the SMTP XOAUTH2 port.~~ **Done and field-validated.**
  Microsoft is a first-class provider.
- **Risk named**: SMTP AUTH is open on the measured tenant, but Microsoft
  closes it by default on some enterprise tenants. The case will show up
  as a connection refusal, not a loss — the outbox keeps the message. To
  be handled in beta if the case occurs.
- ~~**Debt spotted**: folder names come back in undecoded modified
  UTF-7** (`Actualit&AOk-`).~~ **Closed out**: `mail-imap::mutf7` decodes
  per RFC 3501 §5.1.3, with an explicit rule — decoding is for the **eye**
  and for **comparisons**, never for the protocol. The wire name stays the
  one sent back to the server; the two coexist.

  Immediate, unsought effect: the archiving by-name fallback now
  recognizes an `Archiv&AOk-s` folder. On a French-language server with
  no `\Archive` attribute — exactly the Exchange case that motivated this
  fallback — archiving had until now been unavailable.
- **Switch to Graph** if any of these three signals appears: SMTP AUTH
  massively closed among beta users, an announced IMAP OAuth retirement,
  or IMAP quotas becoming prohibitive at scale. The bench stays in
  `spikes/microsoft`, re-runnable.

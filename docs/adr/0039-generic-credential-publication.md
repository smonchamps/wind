# ADR 0039 — Publish generic credentials through an atomic database reference

Date: 2026-09-06. Status: implemented for Lot 3; final review and field pending.
Scope authority: [Lot 3 D6/D7](../PLAN-AUDIT-2026-09-LOT3.md).

## Context

Writing a password before updating SQLite can break a previously usable account
when the second write fails. Reversing the order leaves a new configuration with
an old password. Compensating writes alone do not survive process termination.
Four maintained SQL regressions reproduced partial publication and identity
changes through the old generic-account upsert.

## Decision

Keep two bounded OS-vault slots per email (`generic-password-v1:a:<email>` and
`generic-password-v1:b:<email>`). SQLite stores only the active slot name together
with the nonsecret connection configuration. A NULL slot continues to read the
historical `generic-password:<email>` key, including the Discovery service bridge.

After both protocol probes succeed, and with account publication serialized,
write the inactive slot. Commit its reference and settings in one SQLite
transaction. Only after that commit may the previous slot be removed. Failed
publication leaves the old reference unchanged; orphan cleanup is best effort.
Two fixed slots bound residue even if cleanup repeatedly fails. Account removal
forgets both slots and the legacy key. No password is stored in SQLite or sent
back to the UI.

Repair targets an existing generic account id and preserves its email, IMAP host
and login username. It drains the old incarnation before publication, without
holding its own lease or a provisioning registration during that wait. Failed
verification leaves the old incarnation untouched; failed publication reopens
admission with a fresh incarnation and the old session. SMTP delivery uncertainty
and existing local data stay intact.

## Consequences and limits

A process stopping before the SQLite commit still reads its old credential on
restart; after commit it reads the already-written candidate. OS-vault durability
and SQLite durability remain their respective platform guarantees. No claim is
made about recovery from a provider independently invalidating an old password.
Downgrading to a binary unaware of the slot reference requires reconnecting the
generic account; local mail is retained. No automatic mailbox migration is added.

Memory-vault failpoints cover staging/publication/cleanup failures, while SQLite
fixtures verify identity guards, transactional rollback, retained drafts/files,
interrupted sends, pending actions, preferences and migration idempotence.

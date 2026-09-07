# Lot 3 reconnaissance: E9 and B14

Read-only inspection on 2026-09-06, after Lot 2. No production edits, real accounts, credentials, databases, external network, or tests run. Findings below are source-established; RED protocols are proposed, not claimed results. Read WORKFLOW, job skill, STANDARD product/budgets/architecture and ADR 0010. The requested Sonnet/Fable labels are unavailable; this reconnaissance used the inherited session model.

## B14: resumed initial inventory

`crates/mail-core/src/sync.rs:194` (`initial_sync`) obtains authoritative UIDs, filters known UIDs out, fetches missing envelopes, then returns deleted=0. The caller `sync:183` marks initialized via `update_state`. Existing test `an_initial_sync_cut_at_batch_2_resumes_at_batch_2:884` preserves all server messages and cannot detect this defect.

Minimal change: preserve the complete UID set separately; reconcile local absence before declaring initialization complete; return actual deleted count. Reuse `Store::remove_absent` carefully: `store.rs:1674` deliberately deletes pending actions for absent UIDs, whereas valid-UID intentions must survive. Lot 2 orphan effect trigger preserves uncertain remote mutations when their old pending row disappears. Do not erase the action journal indiscriminately or reset the whole mailbox to solve a resume.

RED: existing 6-message/batch-2 fixture, fail second fetch, expunge UID6, resume; assert local inventory [5..1], deleted=1, no refetch UID5, pending intent for still-present UID preserved, uncertain action-effect receipt retained if its source becomes absent. Also no initialized flag after failed authoritative inventory. Inventory is authoritative only for its point in time; a later remote deletion is caught next cycle.

## S04: allocations and received message limits

Exact paths:

- `apps/desktop/src/commands.rs:4822` (`attach_edit_files`) and `4907` (legacy `attach_files`) use `std::fs::read` before core enforces remaining 25 MB attachment budget. Both paths need the same bounded reader. Open first, validate metadata on the opened handle, and `take(remaining+1)` during read; the actual-read sentinel catches growth/TOCTOU. Check per-draft remaining budget before each file; retain the existing partial-success/refused list behavior. Avoid using path metadata followed by unlimited read.
- `crates/mail-imap/src/lib.rs:934` fetches RFC822.SIZE, substitutes 0 for missing sizes, batches by `BODY_BATCH_BYTES`, fetches complete MIME, then accumulates every parsed body in a final Vec. `bounded_batches:1261` deliberately admits arbitrarily large single messages. An advertised-size check alone is not a memory guarantee against underreported or unsolicited response bytes.
- `fetch_attachment:976` fetches full BODY.PEEK[] for one attachment. `fetch_draft:570` also fetches full MIME without size check. Direct body loading `mail-core/body.rs:12` uses the same body adapter, so an adapter-only backfill cap must not leave direct body/draft/PJ paths unprotected.
- Actual locked dependency is imap 3.0.0-alpha.15. Its `client.rs:1535..1641` collects the whole tagged response in Vec using `read_until(LF, into)` and reparses incomplete literals. Public `uid_fetch` returns already-allocated response storage. A limit applied to `fetch.body()` is too late. BoundedStream in our `mail-imap/lib.rs:206` is above TLS and currently limits only read time; that is a viable location for a byte/deadline guard controlled per command. A quota hit must poison/discard the connection, not reset a partially consumed response and issue another command.

Viable hard-point options to measure, not silently decide:

1. Add absolute raw MIME ceiling and command response quota above TLS, retain complete MIME parser, process one bounded group before returning to core. Simplest integrity boundary, but introduces an explicit oversized-message limitation that conflicts with an unconditional reading of ADR 0010 completeness. User-visible oversized status must retain envelope and allow other work.
2. Fetch MIME structure/parts with bounded partial reads; persist trusted server part path plus transfer encoding beside existing local attachment index, avoid downloading unrelated parts for one PJ. Preserves larger-message access better, but needs MIME numbering/encoding equivalence proof on nested multipart, message/rfc822, inline and attachment parts. Must still enforce response quota because server replies can exceed requests.

Do not present partial FETCH alone or checking RFC822.SIZE as an absolute allocation bound. Do not promise the 200 MB whole-app private-memory budget from a raw-byte ceiling alone: sanitizer, decoded MIME/base64 images and simultaneous workers multiply it.

Shared synthetic spike/RED corpus: honest oversized RFC822.SIZE; missing size; claimed small size then large literal; long line without newline; many compliant bodies whose aggregate exceeds cap; malformed MIME; nested attachments and message/rfc822; growing local file; cap-1/cap/cap+1. Measure actual bytes accepted from stream, peak private working set on isolated process, returned buffer maxima, unrelated eligible item progress, and decoded PJ equality. No external service needed, loopback fixture and generated temp files only.

## B20: attempted work, fairness and deadlines

All three pumps in `crates/mail-core/src/backfill.rs` (`backfill_bodies:114`, `backfill_recipients:198`, `backfill_thread_headers:244`) loop while fetched<budget. The attempted HashSet prevents an infinite repeat inside a pass but grows the SQL LIMIT and scans the whole finite backlog if server omits results. A received error exits the Result and loses the accounting of requests already attempted.

Budgets are shared incorrectly again at caller boundaries: `cycle.rs` thread/recipient loops subtract report.fetched; `commands.rs:5947` `run_backfill_all` does the same across folders/accounts and does not consume failed work. Simply changing the inner loop leaves caller-level overruns. Simply stopping after N attempts leaves the same missing newest N entries starving older mail on every pass.

Minimal coherent contract: report attempted, fetched, remaining, typed issue(s), bytes and exhausted reason; charge budget before network dispatch, including error/missing paths; pass a shared mutable work budget across folder/account boundaries or always return spent budget with an outcome. Persistent retry eligibility or a stable per-kind mailbox cursor must move past misses/errors and later revisit them. A cursor should be keyset-based, not an OFFSET over a shrinking pending list. Per-item retry times prevent a permanently unreadable message from owning every first batch; per-account/folder fairness is required so a permanently replenished INBOX cannot starve another account forever.

Elapsed budget checks between batches only bound admission, not a single blocking command. Real absolute command deadline belongs at adapter/socket read boundary; name these guarantees separately. Typed resource refusal must not be hammered again on every 5-minute cycle.

RED: 1,000 cached envelopes, no served bodies, budget=4: at most 4 attempted UIDs (current code attempts 1,000). Repeat across all three pumps. Two folders/two accounts, first attempted batch errors: global attempts stay within budget, later eligible work eventually proceeds. Reopen Store between passes, a missing newest UID later becomes available, and it is eventually fetched. Fake clock/byte source proves admission/deadline/byte exhaustion without sleeps.

## B21: honest persistent partial status

`cycle.rs::SyncOutcome` holds Vec<String> problems. `poll.rs:681` calls note_outcome(success=true) for every Ok outcome even when a folder, draft import, recipients, disk guard or body pass failed; backoff is cleared. `main.rs:86` explicitly stores account backoff in process memory. `App.svelte:242` derives failure only from the latest report; a successful light INBOX report replaces older full-cycle errors. `App.svelte:580..607` body pump ignores report.errors and finally clears all backfill indicators, even when incomplete. `run_backfill_all` does not apply folder retry policy. Per-folder sync reports are discarded as Ok(_) by full-cycle sweep, which also prevents flag-only generation publication outside INBOX.

Recommended scope: durable core status by account + optional mailbox + operation kind, with category, latest failure/success, attempts, next eligible time and bounded user-facing detail. Clear only the exact operation that succeeded. Maintain connection reachability separately from successful full synchronization. Keep existing incoming-mail success independent from failed background work: do not mark every account fully offline merely because a folder refuses. Use typed retry policy (transient exponential, authentication awaits repair, resource limitation awaits condition/manual retry) instead of substring inference. UI reads this snapshot and exposes recoverable partial status/count/details; a light success must not erase an unrelated folder issue. Manual retry should bypass transient wait intentionally but not revive quarantined destructive actions from Lot 2.

RED: INBOX success/Archive failure then light success retains Archive issue and retry deadline; reopen Store preserves it; success of Archive clears only Archive. Backfill misses/failures retain incomplete indication, body count remains truthful. Connection failure then success reconciles Settings connected state. E2E intercepted statuses assert line is never 'all messages up to date' while unresolved eligible work has failed.

## B23: bounded eventual flag convergence

`sync.rs:23` default recent window=500; `incremental_sync:299` always chooses same recent UIDs. `flags_pass:338` also selects only recent local UIDs. `cycle.rs:620` skips unchanged non-INBOX folders without a flag pass. INBOX has a quiet-status exception in poll_inbox; other folders do not. Existing test `the_flag_window_is_bounded_to_the_most_recent_uids` explicitly expects old flags to remain stale. `Store::apply_flags:1773` already preserves live pending intentions using NOT EXISTS pending_actions refusee=0; retain that safeguard.

Use persistent UID keyset cursor per mailbox and generation, stable snapshot high-water UID per rotation, reset cursor on namespace reset, and run maintenance for otherwise-skipped folders. Advance cursor only after safely applying that batch; misses disappear through authoritative inventory later. A new arrival must not reset progress to the newest UID forever. Carry changed count into generation for every folder, not only INBOX.

Tradeoff options needing common measured protocol:

- Fixed 500 total: split recent + rotating work (e.g. 250/250). Retains one bounded command but reduces the existing immediate recent window.
- Preserve recent 500, add rotation 500 when due. Keeps present near-term behavior, roughly doubles a serviced mailbox's flag entries; use global account/folder scheduling to avoid a round trip per each of 64 folders on every light pass.
- Pure rotating 500 is a control: old flags converge, recent changes can wait a whole rotation; likely violates current user expectation.

Bound must be stated in successful eligible passes, and wall-time only under app-open, online, server-responsive assumptions. Current scheduler: full 30 min, light 5 min (`poll.rs:797..798`). Pure arithmetic example (not measurement): 200,000 old UIDs / 500 = 400 passes, i.e. 200 hours at one full pass per 30 min, before any folder fairness factor. A nominal rotation can therefore meet 'eventually' yet be operationally useless. Do not sell an arbitrary constant as a maximum useful delay without CE choice. Measure command counts, bytes, cumulative service time and maximum revisit lag on a generated 200k/64-folder inventory; include continual arrivals, UID holes, folder addition/removal, restart and generation reset.

RED: 501+ UIDs mutate oldest flag, prove convergence within chosen pass count; quiet Archive STATUS with old changed flag; pending MarkUnseen survives fetched Seen; restart continues cursor; UIDVALIDITY reset does not fetch obsolete UID; CONDSTORE stays zero extra maintenance requests; stale cursor batch errors do not skip permanently.

## B27: write reserve before every batch

`cycle.rs::run_sync` calls poll_inbox before disk estimate. Later pending=announced-account_message_count means 100% cached envelopes/0% bodies yields pending=0 and `disk_shortfall(0,0)==None`. Guard only skips non-INBOX folder sweep; subsequent header/draft work and separate `commands.rs::run_backfill_all` proceed. `body.rs` direct cache writes and remote draft import also have no reserve admission. Existing 50 KiB/message is a historical estimate, not a bound; disk requirement includes SQLite WAL/FTS and draft/PJ snapshots.

Use core pure write-admission policy plus shell volume probe before remote-cache batches, preserving a measured reserve for local drafts/outbox. Include known MIME/decoded bytes and conservative SQLite/WAL overhead; re-probe each batch because concurrent workers consume the same volume. A stale cached free-space value cannot guarantee reserve. Resource exhaustion must stop recoverably and appear in durable partial status; no envelope deletion or false complete status. Missing probe remains explicitly distinguishable from full disk (ADR 0010 says current missing-probe behavior is permissive; changing that availability policy needs explicit decision).

Measure synthetic SQLite growth including -wal across representative envelope, body, FTS and PJ inserts; select reserve from the resulting evidence and agreed local-intention capacity, not invented safety percentages. A quota cannot guarantee successful local save against arbitrary external writers; SQLite errors still must retain UI content as delivered in Lot 1.

RED: injected low-space hook before first INBOX batch -> zero remote cache writes; all envelopes present but missing body backlog -> body pass paused; drop available bytes between batches -> first persists/second does not begin; local draft remains savable under chosen reserve; raise available -> resume exact backlog. Remote draft/PJ write accounting needs its own scenario.

## Actual CE decisions vs routine implementation

Already approved by parent plan: eliminate these defects, keep pending intent safe, truthful state, bounds and eventual convergence. No extra permission required for B14 or ordinary implementation details. New product tradeoffs requiring concrete measured options: accepted oversized-message behavior/MIME ceiling vs complete part streaming; useful maximum flag revisit delay and cost; reserve capacity/low-space policy if it changes whether incoming mail/drafts are admitted; any visible partial-status design gets early visual verdict. Do not ask CE to approve low-level table names, HashSet replacement or tests. Do not claim all E9 solved by front-end error text, one attempt counter or size preflight alone.

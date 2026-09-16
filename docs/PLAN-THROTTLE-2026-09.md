# PLAN-THROTTLE-2026-09 — a Gmail throttle lets the account breathe, and says so

> **JOB CLOSED on 2026-09-16 — full field validation.** STOP 1 GO on the
> seven Chief Engineer decisions D1–D7 (2026-09-15); STOP 2 on the
> workstation's real accounts (2026-09-16): the simulated throttle
> cooldown and the simulated spent daily budget both said in the status
> bar and in Settings, no alert, the other account unaffected, the
> manual Sync lifting the hold — **"all checks ok"**; one same-day field
> touch-up (the IDLE watcher's hold noise, A150 covers the surface).
> Commits `bafba6b` (the job, 30 files, +1775/−117, carrying the Apple
> Silicon run proof docs) and `a0b9cb6` (`chore(deps)`: rustls 0.23.45
> for RUSTSEC-2026-0285, published 2026-09-14 — the first CI run
> `35076384205` was red on the audit leg alone, for a reason outside the
> job). **CI GREEN run 35077788756**, all seven legs. Review: 10
> findings, 9 fixed, 1 residue. D-17 closed. Rides 0.22.0 (D7).
>
> **Kaizen (PLAN-KAIZEN-CLAUDE)**: `--by-commit 262f833..a0b9cb6` —
> **T1 = 48.7 M** input equiv. (40.8 M main thread + 7.9 M in 8 agent
> transcripts, agent share 16.3 %) on the one-commit job `bafba6b` (the
> tool's E0 row IS the job — D12's known limit; the E-steps were not
> separable by commit), 1.9 M for the lockfile bump. **W3**: the tool
> counts 7 gates; by hand, **2 explicit full gates** (6.4 min before
> the review wave, 8 min after) plus the two pre-push replays, the rest
> partial re-gates of the fixing loop. **Quality guard**: 1 KO at STOP 2
> (the watcher noise, fixed the same day), 0 red CI on the job's own
> account (the audit red was an advisory of the day). Turns/prompt 38.3
> over the 8 sessions the range touches.

> Statement (Deputy Chief Engineer, 2026-09-15, from the backlog and
> the field): *"Bug: a Gmail throttle during message download is caught
> then dropped — the UI freezes without a word, and the throttle is
> mistaken for a dead token and triggers the refresh + reconnect it
> should avoid (DEBT D-17, backlog #130, priority 3)."*
>
> Field fact that reopens D-17 (its own clause: *"a Gmail account
> throttled/locked after a large initial sync"*): tester **T2** (Apple
> Silicon, Gmail — the account of the P0 #106) reported on Wind 0.21.0,
> 2026-09-08 18:34, the banner (French interface, given here in the
> English catalogue's words) *"3 actions could not be completed. Check
> the messages and folders concerned. No Response: System Error
> (Failure) [THROTTLED]"* and wrote that she did not understand the
> message nor what she was supposed to do.

Opened: 2026-09-15. Baseline: `262f833` (main, 0.21.0 published,
0.22.0 changelog written and unreleased).
Method: [WORKFLOW](WORKFLOW.md), [job](../.claude/skills/job/SKILL.md).

---

## 1. Finding (what was verified on the evidence)

Read on this workstation, on the code at `262f833`; every claim carries
its file and line. D-17 was written on 2026-08-17 against code that has
since moved: half of it is paid, the other half has changed shape.

### F1 — A throttle is typed as a DEFINITIVE refusal

`server_err` ([mail-imap/src/lib.rs:1371](../crates/mail-imap/src/lib.rs))
turns every tagged `NO`/`BAD` into `Error::Refusal`, except
`[NONEXISTENT]` ([`refusal_from`, :1386](../crates/mail-imap/src/lib.rs)).
The doc comment of `Error::Refusal`
([mail-core/src/error.rs:45-52](../crates/mail-core/src/error.rs)) still
says *"Everything else (`Server`) is deemed transient: network,
**throttling**, timeout"* — the throttle was assumed to travel as
`Server`. It does not: Gmail answers a throttle with a tagged `NO`, so
since audit E3 it lands in `Refusal`, the "retrying will not change
anything" class. **Comment and behavior diverged.**

Two shapes of the same server, both to recognize:

- the token stays in the text when imap-proto does not know the code —
  `resp_text` ([imap-proto 0.16.7, rfc3501/mod.rs:615-626]) keeps an
  unknown `[THROTTLED]` inside `information`; T2's string shows it at
  the **tail**: `System Error (Failure) [THROTTLED]`;
- the token is **parsed away** when it is `[ALERT]` (`ResponseCode::Alert`,
  types.rs:126): Gmail's documented lockouts read `NO [ALERT] Account
  exceeded command or bandwidth limits. (Failure)` and `NO [ALERT] Too
  many simultaneous connections. (Failure)` — `refusal_from` reads
  `information` only, so the alert code is invisible and only the
  phrase remains.

Detection must therefore read **both** the code and the text, and match
tokens anywhere, not at the head.

### F2 — The action journal quarantines a throttled gesture for good

`replay_actions` ([mail-core/src/sync.rs:322-360](../crates/mail-core/src/sync.rs)):
`Err(Error::Refusal)` → `refuse_action` → `refusee = 1`
([store.rs:2156](../crates/mail-core/src/store.rs)). Nothing ever sets
`refusee` back to 0 (grep: only a NEW gesture on the same message
replaces the row, `store/tests.rs:501`). The slot then shows
`notice.refusedActions` + the raw `last_error`
([App.svelte:689](../apps/desktop/ui-v2/src/App.svelte),
[mutations.rs:38](../crates/mail-core/src/mutations.rs)).

This is T2's banner, exactly: three of her own gestures (archive,
delete, flag…) quarantined forever by a "not now" answer, the advice
*"check the messages and folders concerned"* pointing at nothing wrong,
and a raw IMAP string appended. A transient failure (`Server`) would
instead have stopped the replay and kept the queue
(`note_action_failure`, five strikes before quarantine, store.rs:2141).

### F3 — D-17's "mute break" is paid; the reasons stay raw

The backfill UI loop reads its error channel now
([App.svelte:604](../apps/desktop/ui-v2/src/App.svelte): `if
(report.errors.length) await probeSync()`), and every per-mailbox
failure is settled as an operation issue
([backfill.rs:289,387](../crates/mail-core/src/backfill.rs)), backed off
30 s × 2ⁿ capped at 30 min ([operations.rs:30-55](../crates/mail-core/src/operations.rs)),
surfaced as *"Synchronization incomplete · N issue(s)"* with a "View
details" gesture ([App.svelte:209-212](../apps/desktop/ui-v2/src/App.svelte))
and listed in Settings with the raw reason and the retry time
([Settings.svelte:853-858](../apps/desktop/ui-v2/src/Settings.svelte)).
Paid at PLAN-AUDIT-V1 E3. What remains of (1): the reason is the server's
English string, verbatim — the user "does not understand the message".
One gap left: the account-level connect failure of `run_backfill_all`
([commands.rs:6957-6968](../apps/desktop/src/commands.rs)) only fills
`summary.errors`, never an issue — it relies on the cycle's own
connection notice to be seen.

### F4 — The dead-token guard lets a throttle through, bounded

`connect_imap_with_stop` ([poll.rs:496-518](../apps/desktop/src/poll.rs)):
only `Error::Connection` skips the refresh. A `NO` at AUTHENTICATE
(Gmail's lockout answers) is a `Refusal` → `authenticate_silent` (an
OAuth refresh) + a second connect, refused again. Bounded by the
account backoff `wait_after_failures` (poll.rs:405-411: 0, 0, 10, 20,
40, 60 min) and skipped by the cycle, the light pass and the IDLE
watcher while it runs (poll.rs:691, :578; watcher.rs:107). So: one
refresh + two connects per knock, at most every hour in steady state —
**hammering bounded, not absent**, and every knock re-opens the door to
F5.

### F5 — Nothing lets the account breathe: the pump re-spends the quota

Google documents a **2,500 MB/day IMAP download cap** per account; past
it, IMAP is suspended for **1 h, up to 24 h**
([Gmail bandwidth limits](https://knowledge.workspace.google.com/admin/gmail/gmail-bandwidth-limits);
the "too many connections / exceeded command or bandwidth limits" lockout
is reported at 24 h in the wild). Wind's full sync (ADR 0010) downloads
every body: the pump `backfillBodies`
([App.svelte:589-620](../apps/desktop/ui-v2/src/App.svelte)) plays
120 s passes of `WorkBudget` 64 MB / 120 s each
([work_budget.rs:20-22](../crates/mail-core/src/work_budget.rs)),
re-armed at start and at every generation, with **no daily volume
bound**. On a large mailbox it reaches the cap within the day; Gmail
then refuses everything — **INBOX polls included** — until the
suspension lifts; Wind knocks again within the hour (F4), the knock
succeeds once the suspension lifts, the pump restarts at full speed and
re-spends the next day's quota. The account never catches up and
receives nothing in between.

That is the shape of #106 on the same account: *"mails only since
August 25, INBOX at 72 % since this morning, 0 conversations on Sync"*
— a **hypothesis**, consistent with every fact, not yet a proof (§7.1:
no way to see her database from here).

### F6 — The trace exists; the folder named in the ask did not

**Corrected on 2026-09-15 at STOP 2 preparation** (the first reading of
this section said "the shipped binary writes no trace file" — wrong,
found by looking at this workstation's data folder). The `mesure`
spans ([main.rs:188-215](../apps/desktop/src/main.rs)) are indeed
stderr-only, but the **field trace** of PLAN-AUDIT-V1 E9
([trace.rs](../apps/desktop/src/trace.rs)) writes every `trace(…)` line
to **`wind.log` next to the database**, bounded to 1 MB, sanitized (no
address, no secret, no body). Polls, the pass after a gesture, the
drain, the watchers and their errors land there — a throttled cycle
leaves its line. What the ask got wrong is the folder:
`~/Library/Application Support/Wind/` does not exist; the app data dir
is `~/Library/Application Support/dev.elements.wind/`
([tauri.conf.json:5](../apps/desktop/tauri.conf.json)), holding
`wind.db`, `wind.log` and `maj.log`.

So the P0's ask is `wind.log` from that folder (plus the Settings
screenshot, which shows the same facts on screen). A lesson for the
method, not only for this job: a finding about "what exists on a
machine" is checked on a machine, not deduced from the build flags.

### F7 — What the user reads

Three surfaces show raw server text: the refused-actions notice (F2),
the Settings issue reasons (F3), and — indirectly — nothing at all
while an account is suspended (the progress line keeps its last
figure). Backlog #119 ("understand what went wrong") is the wide
version of this; this plan covers only the throttle class.

---

## 2. Scope

The job delivers the three parts of the statement, on the shape the
code has today:

1. **Type the throttle** — a transient server refusal, recognized from
   the response code and the text: `[THROTTLED]`, `[UNAVAILABLE]`,
   `[LIMIT]`, `[INUSE]`, `[OVERQUOTA]` (RFC 5530) and Gmail's phrases
   *"bandwidth limits"*, *"Too many simultaneous connections"*,
   *"exceeded command or bandwidth limits"*. `[NONEXISTENT]` unchanged.
2. **Let the account breathe** — on a throttle anywhere (connect, sync,
   backfill, action replay): no OAuth refresh (F4), the action queue
   survives (F2), and the account enters a **cooldown** during which
   no IMAP work of any kind is attempted (cycle, light pass, IDLE
   watcher, backfill pump, replay). The manual Sync gesture stays an
   order (`force`), as today.
3. **Say it** — the progress line names the pause and its end
   (*"Gmail is limiting downloads · resumes at 14:30"*), the throttle
   class carries a localized reason in the notice and in Settings
   instead of the raw string, and the refused-actions notice never
   fires for a throttle.
4. **Optional, D4** — a **daily download budget** for the Gmail
   backfill (prevention: the only measure that stops the daily re-lock
   of F5). Per account, persisted, INBOX arrivals exempt; the progress
   line says *"Catching up paused until tomorrow — Gmail's daily
   limit"*.
5. **The P0's field ask, corrected (D1)** — `wind.log` from
   `~/Library/Application Support/dev.elements.wind/` (the folder the
   first ask got wrong), the Settings › Accounts screenshot, Activity
   Monitor › Kind (for #122), and a Quit/Relaunch. Written for the Chief
   Engineer, not sent by the session (the identities stay with the
   Chief Engineer).

### Refusals (what we do not do, and why)

- **A spike that hits Gmail's cap on purpose**: it would lock the Chief Engineer's
  own account out of IMAP for up to 24 h (§2.6; and §7.1 forbids
  reproducing on T2's). The thresholds come from Google's document;
  the field proof comes from T2 after the release.
- **Localizing every raw reason (#119)**: this plan localizes the
  throttle class only; the generic "what to do next" is its own job.
- **Microsoft's throttling**: no field evidence, no documented figure
  read; the typed transient class will catch RFC 5530 codes there too,
  nothing provider-specific is written for it.
- **A diagnostic file the user can SEND** (F6, corrected): `wind.log`
  exists but nobody can find it without a path — a "Save a diagnostic
  file" gesture is a product decision with its own privacy questions
  (ADR 0014 lineage) — opened as a backlog item (D6), not built here.
- **Re-typing the existing quarantine rules** (five strikes, uncertain
  effects): untouched; only the throttle stops counting as a strike.

---

## 3. Design sketches — options on figures

### Hard point: how long does the account breathe? (D2)

Google: suspension **1 h typical, up to 24 h**; bandwidth resets
**daily**. No measurement of our own is possible without locking an
account (refusal above), so the options are compared on the documented
figures and on what each costs the user:

| Option | Behavior | Knocks during a 24 h lockout | Delay after a 1 h suspension |
|---|---|---|---|
| A · fixed 1 h | one hour, every time | 24 | ≤ 1 h |
| **B · doubling** (recommended) | 1 h → 2 → 4 → 8 → 16 → 24 h cap, reset on the first success | 5 | ≤ 1 h |
| C · until the next day | pause to local midnight | 1 | up to 23 h — punishes the 1 h case |

B keeps the 1 h case cheap and bounds the 24 h case to five knocks,
each of them ONE connect without a refresh (F4 fixed). Implemented as
the pure `wait_after_throttle(strikes)` next to `wait_after_failures`,
tested the same way (poll.rs:999-1011).

### The daily budget (D4)

2,500 MB/day is the cap; the backfill must leave room for INBOX polls,
reads of bodies on click and the user's other clients. Proposal:
**2,000 MB/day per Gmail account**, counted from `WorkBudget`'s bytes
(already measured per pass, work_budget.rs:15), persisted in `prefs`
as `(day, bytes)` per account, reset at local midnight; when spent, the
pump stops for the day and the progress line says so. Generic IMAP and
Microsoft: no budget (no documented cap). Cost: one pref, one pure
`daily_budget_left(day, spent)` decision, one line in the progress line.

### Where it shows (D5)

The System's slot grammar (system.dc.html §"1 · The notice slot") is
for what needs a gesture; the **progress line** (§"2") is for a state
the user waits out. A throttle asks nothing of the user → progress
line, muted, with the resume time; Settings keeps the localized reason
under the account; no alert glyph. The `refusedActions` notice, which
IS an alert, stops firing for throttles because they no longer
quarantine (F2).

---

## 4. Steps and gates

| Step | Delivers | RED first | Gate |
|---|---|---|---|
| E1 | `Error::Throttled(String)` in mail-core; `server_err`/`refusal_from` recognize code + text (tail token, `[ALERT]` + phrase); `is_transient()` | mail-imap unit tests: `[THROTTLED]` at the tail, `[UNAVAILABLE]` at the head, `[ALERT]`+"bandwidth limits", `[NONEXISTENT]` unchanged, plain `NO` still `Refusal` | `cargo test -p mail-imap -p mail-core` |
| E2 | Action journal: a throttle stops the replay, counts no strike, quarantines nothing; operation issues: a throttle's `retry_at` follows the cooldown | sync.rs `a_throttled_action_keeps_the_queue_and_counts_no_strike`; operations.rs cooldown test | same |
| E3 | Shell: `should_refresh_token(&Error)` pure guard (no refresh on Throttled or Connection); account **cooldown** table in `AppState` shared by cycle, light pass, watcher, backfill pump and replay; `wait_after_throttle` | poll.rs unit tests on the two pure functions; the pump skip has a store-level test | `cargo test -p wind-desktop` |
| E4 | Progress line "resumes at HH:MM", localized throttle reason (notice + Settings), both catalogues, System journal **A150**, `sync_progress` carries `cooldown_until` | e2e via a fixture seam (`window.syncFixture`, the `connectionFixture` pattern) asserting the visible text; catalogue placeholder test | targeted spec, then the full gate |
| E5 (D4) | Daily budget: pref, pure decision, pump stop, progress line text | mail-core test on `daily_budget_left`; e2e on the text | full gate |
| E6 (D1) | The corrected field ask for #106 (`wind.log` in the right folder), written in the plan for the Chief Engineer to send; D-17 closed with its residue; STATE amended | — (docs) | docs gate |

Full gate ONCE before the commit (`/gate`); `/code-review high` once
at convergence. Version: MINOR is not triggered (no new capability
visible as such — a pause that says so is a fix); it rides the next
release (D7).

---

## 6. Delivery record (2026-09-15)

| Step | Delivered | Proof |
|---|---|---|
| E1 | `Error::Throttled` (mail-core `error.rs`, code `throttled`, retryable); `refusal_from` + `server_err` recognize the seven markers anywhere in the text, `BYE` included; `should_refresh_token` | mail-imap `throttle_tests` (5), mail-core `error::tests` — RED on the missing variant first |
| E2 | `throttle.rs`: `wait_after_throttle`, `note_throttle` (one strike per knock), `cooldown_until`, `cooldowns`, `clear_throttle`; `operation_due` gated by the cooldown; `settle_operation` writes it on `Throttled` and clears it on success; `retry_operations` lifts it; `replay_actions` stops on a throttle without a strike; `replay_removal` drops the effect on a throttled transfer | `throttle::tests` (4), `sync::tests::a_throttled_action_keeps_the_queue_and_counts_no_strike` — RED shown (3 failures) then GREEN, 600/600 |
| E3 | `ConnectFailure` typed door (`connect_imap_typed`), `on_hold`/`hold_if_throttled`/`hold_reason`, read before the door in the full cycle, the light pass (scheduler and manual, `force` lifts the hold), the IDLE watcher and the backfill pump; `sync_progress.cooldowns` | `poll::tests` (2 new) — RED (13 compile errors) then GREEN, 5/5 |
| E4 | `status.throttled`, `settings.cooldown` (both catalogues); the progress line says the pause first; Settings › Accounts line; `__e2eCooldowns` seam; A150 | e2e `throttle-cooldown` test 1 — RED ("Sync failed" with the seam armed) then GREEN, 16 s |
| E5 | `GMAIL_DAILY_DOWNLOAD_BUDGET` 2,000 MB, `daily_download_left`, the day's tally pref `download.{id}`, `WorkBudget::bytes_left`; the pump skips a spent Gmail account and reports `paused_until` (next local midnight); `status.dailyPaused` | `throttle::tests` (2 new) — RED (compile) then GREEN; e2e test 2 through `__e2eBackfill`, 2/2 in 37 s |
| E6 | D-17 closed with its residue; CHANGELOG 0.22.0 both languages; STATE; this record | docs gates: links 269/269, ratchet 1661 (no rise), System coherence |

### Review (`/code-review high`, 2026-09-15) — 10 findings, 9 fixed

Eight angles, one-vote verification. The sharpest: **the INBOX poll
re-typed every error as `Server` before settling**, so a throttle on the
INBOX replay — the field case — never reached the cooldown hook (three
angles found it independently; a cycle-level test now drives
`poll_inbox` with a throttling fake). Also fixed: RFC 5530's
`[OVERQUOTA]`/`[LIMIT]`/`[INUSE]` were typed as throttles (a full mailbox
would have silenced the account for a day — they stay refusals, and
Gmail's "too many simultaneous connections", which clears in seconds, is
not a throttle either); a hold returned as an error fed the in-memory
backoff, which outlived the cooldown by up to an hour (`poll_cycle`
skips a held account before any work, never as a failure); a success
inside a running cooldown erased it (only an expired one clears); a
throttle still inserted an operation issue and raised the "incomplete"
alert against D5 (a throttle is the cooldown only); the gesture pass and
the draft reflection knocked during a hold (one scheduler door,
`open_door`); the two prefs were missing from the account purge list (a
reused id would have started on hold); the manual Sync reset the strike
counter (it lifts the wait, keeps the strikes); the daily pause was
derived from an all-accounts pump report and never refreshed at
midnight (now a per-account cooldown of kind `daily_budget`, computed on
every probe, on the same channel and Settings line; a DST gap at
midnight no longer loses the date; less than one pass left counts as
spent). Skipped, recorded as residue: `should_refresh_token`'s polarity
(negative — pre-existing; a positive authentication class needs its own
error variant).

### STOP 2 (2026-09-16) — field, on the workstation's real accounts

Check 2 (a simulated throttle cooldown, one pref written with the app
closed): the status bar named the account, said the server was limiting
downloads and gave the resume time (11:05), with no red dot; Settings
carried the same line; account 2 was unaffected — the Chief Engineer's
words, translated; the manual Sync lifted the hold and synced account 1
(`52 folders (50 skipped)`). Two readings corrected on the way: the
first INSERT kept the `<ID>` placeholder literally (no account matched
— the checklist now says "the number from step 1"); and the checklist
expected the strike row to survive the click — wrong: the click lifts
the wait, and the sync that follows SUCCEEDED, which is the server
serving again, so the strikes reset; only a throttle on that click
escalates (the unit test `a_manual_retry_lifts_the_cooldown_but_keeps_the_strikes`
covers exactly that branch). One finding fixed the same day: the IDLE
watcher treated the hold as a dropped session and traced "reconnecting
in 60 s" once a minute for the whole hold — it now sleeps on hold like
it does on a backoff, without a line. Also seen in the field, and
counted: `download.1 = 39.7 MB` — the daily tally counting the real
Gmail account's catch-up, unprompted. Left as is, pre-existing: the
"catching up · 45 to go · 99 %" line on bodies the server no longer
holds (already in the 10:00 trace, before any pref).

Check 3 (the daily budget, the tally overwritten with a spent day):
the status bar named the account, said the daily download limit was
reached and gave the resume date (17 Sept., 00:00), no red dot,
Settings the same. **Verdict, Chief Engineer, 2026-09-16: "all checks
ok"** — checks 4 (trace), 5 (plain use on both real accounts) and 6
(clean-up) included.

**Final full gate (2026-09-16), GREEN in 8 min (478 s)**: 13/13, clippy
0 warnings, 957 Rust tests over 33 targets, e2e 291 passed + 1 flaky
retried green (`recipient-routing` "Cc-only composition reaches the
durable offline queue" — the composer, unrelated to this job). The
first full gate of the job (2026-09-15, before the review wave) read
6.4 min, 292/292, 0 flaky. W3: 2 full gates for the job.

Stated limits: the cooldown and the budget are proven at the unit and
seam level — the real Gmail throttle was never provoked (refusal §2);
the field proof is T2's account after 0.22.0. The `[ALERT]` phrase is
Google's documented wording, not a capture from a live session. The
daily tally counts the pump's bytes only: the arrivals' bodies, the
header and recipient passes and reads on click ride on the 500 MB of
room left under the cap.

## 5. § Chief Engineer decisions

- **D1 — The P0's field ask.** Asked as "the trace does not exist
  (F6)" — corrected the same day: it exists, as `wind.log` in
  `~/Library/Application Support/dev.elements.wind/`. The ask: that
  file, Settings › Accounts › issue details (screenshot), Activity
  Monitor › Kind (for #122), then Quit/Relaunch and say whether the
  percentage moves. GO / other?
- **D2 — Cooldown after a throttle.** A fixed 1 h · **B doubling 1 →
  24 h, reset on success (Recommended)** · C until the next day.
- **D3 — A throttled gesture in the action journal.** **(a) stops the
  replay, counts no strike, queue kept (Recommended)** · (b) counts as
  a transient strike like today's `Server` (five → quarantine).
- **D4 — Daily download budget for the Gmail backfill** (2,000 MB/day
  per account, INBOX exempt). GO / not now (react only, E1–E4).
- **D5 — Where the pause shows.** **Progress line + Settings reason,
  no alert (Recommended)** · a notice in the slot.
- **D6 — A diagnostic file in the shipped app.** Open a backlog item
  (the beta is blind without telemetry or trace) / no.
- **D7 — Release vehicle.** Hold 0.22.0 for this job (#166 option b,
  testers take one update) / ship 0.22.0 now, this lands in 0.23.0.

### Answers (STOP 1, 2026-09-15)

- **D1**: *"I already have sent the message and I am waiting for the
  reply"* — the re-routed ask is with T2; E6 records it, sends nothing.
- **D2**: *"B doubling 1h→24h (Recommended)"*.
- **D3**: *"No strike, queue kept (Recommended)"*.
- **D4**: *"GO (Recommended)"* — the daily budget is in scope (E5).
- **D5**: *"Progress line + Settings (Recommended)"*.
- **D6**: *"Open the backlog item (Recommended)"* — a diagnostic file
  in the shipped app, opened in the Notion backlog at Phase 4.
- **D7**: *"Hold 0.22.0 for this job"* — this job rides 0.22.0; its
  changelog entry is added to the unreleased section, both languages.

# PLAN-BETA — the closed beta (Phase 5, PLAN §4)

> Opened on 2026-08-27 (PLAN-RETOURS-11 R3, decisions D7-D9). Goal:
> 20-50 real users, every piece of feedback read by the CE (genchi
> genbutsu), weekly kaizen on **observed** friction. Gate 5: two weeks
> without a critical defect → launch.

## 1. What is ready (found on 2026-08-27)

- **Delivery chain proven**: public repository, dual-arch releases
  signed with minisign, auto-update proven on both workstations across
  three consecutive versions (0.9.0 → 0.11.0), scripted verification
  18/18 (`scripts/verify-release.ps1`).
- **First-launch journey** (PLAN-RETOURS-8): a fresh tester is guided
  in four steps.
- **OAuth compiled into the release** (ADR 0025, proven without
  `setx`).
- **Update failure VISIBLE** (PLAN-SIGNATURE): no more silent closing.
- **Tester guide**: [BETA.md](BETA.md) — installation, SmartScreen,
  Smart App Control, the Google "unverified" screen, how to give
  feedback (the three-line form).

## 2. The two accepted risks, and how they are handled

- **D-39 — installer not Authenticode-signed**: on a workstation with
  Smart App Control `On`, installation is a per-binary lottery (proven
  on 08-26/27). Handling: the guide SAYS so, every refusal is expected
  feedback and is counted; the first MAJ refused on a SAC workstation
  will prove the net of PLAN-SIGNATURE (proof still owed). The
  underlying lever (signing) stays frozen — closed validation outside
  the USA/Canada.
- **Google app in production NOT VERIFIED** (CE finding, D8 of
  2026-08-27): no prior registration of testers, but a deterrent screen
  at the first Gmail login. Handling: the guide explains it and gives
  the path ("Advanced settings"). The CASA file stays the critical path
  of the PUBLIC launch, outside the beta — detailed in §4.

## 3. The actions

Checked as they happen; CE actions are marked **[CE]**.

- [x] Tester guide committed to the repository (BETA.md) — 2026-08-27.
- [x] **[CE]** The feedback address (D7): **feedback-wind@fcts.io** —
  settled in the field on 2026-08-28. The main channel is now INSIDE
  the app: the **Feedback** button in the header (A91) sends by email
  from the tester's own account; the address stays in the guide as a
  fallback (Wind blocked at install).
- [x] **[CE]** The address `feedback-wind@fcts.io` **receives** — CE
  finding of 2026-08-29: the mail sent on the 28th did arrive, the
  defect was a DNS configuration propagation delay, not an alias
  outage. The blocker of 2026-08-28 is lifted; the invitation track is
  open.
- [x] **[CE]** First wave (D9): 5-10 close contacts — invite them by
  personal email with the link to the guide
  (https://github.com/smonchamps/wind/blob/main/docs/BETA.md). Aim for
  at least ONE Smart App Control `On` workstation and ONE Gmail
  account: both risks of §2 must be tested early. **Wave opened on
  2026-08-31**: named register in **§3 bis**, sample invitation
  message in **§3 ter**, guide re-read and corrected the same day
  (five-step journey, "Organized" section). **The five invitations
  went out on 2026-08-31**; the register still needs filling in as
  replies come.
- [ ] **[CE]** Read every piece of feedback; confirmed friction enters
  the repository through `/job` or `/field` (the weekly kaizen of PLAN
  §4 — the mechanics already exist, nothing new).
- [ ] Widen to 20-50 once the first wave is running (installation
  proven, feedback coming in, no open critical defect).
- [ ] Count SAC refusals (D-39): if a tester is blocked at install,
  log workstation/version/date in the D-39 debt register — that is the
  measurement that will reopen the signing job.
- [ ] Gate 5: two weeks without a critical defect → prepare the launch
  (with, on its path: CASA, signing).

## 3 bis. Wave 1 (D9) — the register of the five

> Opened on 2026-08-31. Five close contacts, no more: the wave is read
> by hand. Two postures are MANDATORY in the batch (§2) — at least
> **one Smart App Control `On` workstation** and **at least one Gmail
> account**. Without them, the wave tests neither risk.

Release in force at the wave's launch: **0.15.0** (published on
2026-08-30, x64 + arm64, `latest.json` in place).

**The register is ANONYMOUS, and that is a rule, not modesty**
(CE decision of 2026-08-31): the repository is PUBLIC. The names and
addresses of the five stay with the CE, outside the repository; here
live only the rank pseudonym, the workstation's posture and the facts.
No feedback copied into the repository carries a name, an address or
message content — only the fact, the version, the date.

| # | Workstation (arch. / SAC) | Account targeted | Invited on | Installed on | Feedback |
|---|---------------------|-------------|-----------|-------------|---------|
| T1 | x64 / **SAC `On`** | **Gmail** | 2026-08-31 | **2026-08-31** | — |
| T2 | — | — | **2026-08-31** | — | — |
| T3 | — | — | **2026-08-31** | — | — |
| T4 | — | — | **2026-08-31** | — | — |
| T5 | — | — | **2026-08-31** | — | — |

**The five invitations WENT OUT on 2026-08-31** (CE finding of the
day). **The two mandatory postures of §2 are covered as of T1** (x64,
Smart App Control `On`, Gmail account, installed the same day): the
wave does test both accepted risks. Two new facts follow from this,
and they do not carry the same weight:

- **D-39 — 0.15.0 x64 PASSES on a SAC `On` workstation that is not the
  CE's.** First favorable verdict recorded off the development
  workstation. It closes nothing: the verdict is rendered **per
  binary** (by hash), so it says NOTHING about the next version — that
  is exactly the lottery. What it does open, on the other hand: this
  workstation is the bench that was missing for the **measurement owed
  by the PLAN-SIGNATURE net** (visible update failure under a real SAC
  refusal condition). To watch for at the first MAJ this workstation
  refuses.
- **The Google "app not verified" screen is tested**: T1 connected a
  Gmail account and the install succeeded the same day. Still to know
  is whether the screen made them hesitate — that is a question to
  ask, the guide (§2) does not prove it was read.

Next deadline: **follow-up with the silent ones on 2026-09-03** (the
three-day rule below).

Rules for keeping the register:

- **The correspondence stays with the CE.** The T1-T5 ↔ person mapping
  is written nowhere in the repository.
- **Invited on**: the date the invitation message (§3 ter) was sent.
- **Installed on**: the date the tester SAYS Wind launched — not an
  assumption. A silence of more than three days gets one follow-up,
  then is logged as such.
- **Installation refusal**: a line in the **D-39** debt register
  (workstation, version, date) — that is the measurement that will
  reopen the signing job, and it only counts if it is written down.
- **Feedback**: every piece of feedback that is acted on enters the
  repository through `/field` (same-day finding) or `/job` (underlying
  friction); the checkbox carries the number of the PLAN that handled
  it.

## 3 ter. The invitation message (template)

> Sent personally, one recipient per email — never as a collective
> blind copy: a tester invited in a batch replies in a batch, meaning
> never.

**Subject**: Wind — want to give it a try?

```
Hi <first name>,

I'm asking you to be a guinea pig. Wind is the Windows email client
I'm working on: fast, plain, local — your messages stay on your
machine, no telemetry. It is in closed beta, there are five of you.

The install guide (5 minutes, everything is in it):
https://github.com/smonchamps/wind/blob/main/docs/BETA.md

Two warnings await you, and they are normal — the guide explains them:
Windows may show "Windows protected your PC" (the installer is not
signed yet), and Google an "app not verified" screen if you connect a
Gmail account (the audit is under way). On some recent PCs, the
install may be refused with NO way around it: if that happens to you,
tell me — it's valuable feedback, not a mistake on your part, and
above all don't disable anything.

There is a toggle in the header called "Organized": it's the new
thing in this version — it opens three destinations (Inbox, Feed,
Paper trail) and a Screener where YOU decide, sender by sender. Read
§3 of the guide before you use it: the three destinations move
nothing at your provider, but the Screener's "No" does act on your
real mailbox (by default: the trash, for messages that arrive
afterward). That's what I'm most interested in your opinion on.

What I expect from you: use it for real, for a few days, and tell me
what's wrong. The "Feedback" button top right sends it directly. The
most useful feedback fits in three lines: what you were doing, what
you expected, what happened. Unclear text or a missing gesture count
just as much as a bug.

Thanks,
<signature>
```

## 4. The Google verification file (path to the PUBLIC launch)

> Added on 2026-08-28 on CE decision. **Nothing here blocks the beta**:
> the 5-10 close contacts go through the unverified published app
> (§2). This file is the path to the PUBLIC launch — it enters
> PLAN-BETA because its timelines are long and its first milestone (a
> domain) is not yet in place.

The form is refused outright without a **domain**: the GitHub
repository cannot stand in for a homepage (Google explicitly excludes
platform links). That is the missing brick, and everything else
depends on it.

- [ ] **[CE]** **The domain**, and its ownership proven in Google
  Search Console (brand verification: 2-3 business days announced).
  Lead: `fcts.io`, already chosen for the feedback address (§3).
- [ ] **[CE]** **Public homepage** on that domain: accessible without
  logging in, clearly linked to Wind.
- [ ] **Privacy policy** on the SAME domain: how Wind accesses, uses,
  stores and shares Google data, and **Limited Use** compliance (no
  advertising, no resale, no model training, no human reading). Wind
  is in a strong position — nothing leaves the workstation, no network
  telemetry (ADR 0014) — but the position must be WRITTEN, not
  inferred.
- [ ] **Exact consent screen**: name, logo, support email, links — all
  consistent with the domain.
- [ ] **Scope-by-scope justification**, at least privilege. The
  expected sticking point: `https://mail.google.com/` is the broadest
  scope, and the reviewer will ask why not `gmail.readonly` +
  `gmail.send`. The answer is verifiable — **XOAUTH2 over IMAP/SMTP
  accepts only that one** — but it must be in the file.
- [ ] **Demo video**: unlisted YouTube, in English, showing the full
  OAuth journey, the app name on the consent screen, the **`client_id`
  readable in the address bar**, and each restricted scope at work.

**Open question, ahead of any cost.** The CASA security assessment
(Tier 2, an App Defense Alliance-approved lab, ~$540 recorded for the
DAST scan, **to redo every 12 months**) is conditioned by Google on a
precise criterion: it is owed if the app "accesses or has the
capability to access Google user data **from or through a server**."
**Wind has no server** — tokens in the OS vault, IMAP/SMTP direct
workstation ↔ Google, no backend that sees a message pass through. If
the exemption covers this architecture, only the list above remains.
Public documentation does not settle it (the CASA page does not detail
its exceptions; third-party sources claim the opposite without
distinguishing pure clients). The verification form asks the server
question, so does the lab. **To be settled BEFORE paying for or
planning anything**: this is what would reopen the "long, costly"
claim of [PLAN.md](PLAN.md) §2.3 — made in Phase 0, never verified
since.

## 5. What the beta does not do (refusal §2.6)

- No network telemetry nor remote crash reporting (ADR 0014 holds:
  local and opt-in). Feedback goes through the D7 email.
- No GitHub Issues channel imposed on testers (D7) — the public
  repository stays open to whoever prefers it, without making it a
  requirement.
- No separate "beta" build: testers install the CURRENT release and
  live the real auto-update — that is what is being tested.

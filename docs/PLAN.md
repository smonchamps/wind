# Chief Engineer Plan — A simple, high-performance email client (Windows + Web)

> Written in the spirit of the Toyota *shusa*: a Chief Engineer who carries the product
> vision, freezes the hard points early, explores alternatives in parallel (set-based
> engineering), and builds quality into the process (jidoka) rather than at the end of
> the line.

---

## 1. The concept paper (the CE's vision)

**Product promise:** *"Your mail, instantly."* An email client that starts in under
one second, where every action (open, archive, search) responds in under 100 ms, and
that works offline as well as online.

**Target user:** the demanding professional or individual, 1 to 4 accounts (Gmail,
Outlook/Microsoft 365, generic IMAP), tired of the heaviness of Outlook and webmail.

**What the product IS:**
- Fast: performance IS the feature, not an optimization.
- Simple: read, sort, search, write. Nothing else at launch.
- Reliable: never lose mail, never send a phantom message, offline-first.
- Secure: credentials in the OS vault, sanitized HTML, remote images blocked by default.

**What the product IS NOT (v1):** no calendar, no chat, no built-in AI, no plugins,
no mobile. The CE refuses any scope creep: every addition is paid for in speed and
reliability.

**Numeric targets (the equivalent of Toyota's "target costing"):**

| Metric | Target | Measured from |
|---|---|---|
| Cold start (usable window) | < 1 s | Phase 1 |
| Opening a message | < 50 ms | Phase 1 |
| Search over 100 000 messages | < 100 ms | Phase 3 |
| RAM in everyday use | < 200 MB | Phase 1 |
| Local database (3 accounts, bodies backfilled) | < 1 GB | Phase 3 ([ADR 0007](adr/0007-body-backfill.md)) |
| List scrolling | 60 fps | Phase 1 |
| Windows installer size | < 15 MB | **measured: 4.75 MB** (NSIS, 2026-07-21) |
| Data loss | 0, proven by crash-recovery tests | Phase 2 |

These budgets are **blocking gates**: a phase does not end if a budget is exceeded
(andon — stop the line).

---

## 2. Phase 0 — Kentou: study and front-loading (2 to 3 weeks)

At Toyota, hard problems get solved BEFORE development, not during it.

### 2.1 Genchi genbutsu — go see for yourself in the field
- Take apart 5 existing clients: Outlook (heaviness), Thunderbird (architecture),
  Superhuman (perceived speed, shortcuts), Mailspring (C++ mailsync engine + JS UI),
  Hey (product opinions). Note what makes each one slow or fast.
- Interview 8 to 10 real users: what is the most frustrating moment of their email
  day? (Hypothesis to validate: the morning sort and search.)

### 2.2 Research & reuse (mandatory before writing code)
Do not rewrite what already exists and works:
- **Pimalaya `email-lib`** (Rust): a proven IMAP/SMTP/Maildir abstraction (base of the Himalaya CLI).
- **Stalwart crates**: `mail-parser`, `mail-send`, `mail-builder`, `imap-codec` — production-quality MIME parsing/sending.
- **Delta Chat `core`** (Rust): the best open-source IMAP sync engine in Rust; to study for sync patterns, not necessarily to integrate.
- Candidate crates: `async-imap`, `oauth2`, `keyring` (Windows Credential Manager), `rusqlite`/`sqlx` + FTS5, `tantivy` (search alternative), `ammonia` (HTML sanitization), `lettre`.

### 2.3 The 4 hard problems to solve by spike (throw-away prototype, 2-4 days each)
1. **Sync engine**: incremental IMAP (CONDSTORE/QRESYNC when available), local/server
   reconciliation, replayable offline action queue. This is the core of the product.
2. **The web bridge**: a browser CANNOT speak IMAP (no raw TCP socket). The web
   client therefore requires a sync backend service. A structuring decision to
   freeze here.
3. **HTML rendering of emails**: sanitization (XSS), blocking remote images,
   isolation (iframe sandbox / webview CSP), without breaking newsletter layout.
4. **OAuth2 Gmail/Microsoft**: desktop flow (loopback PKCE), token storage, and
   above all the **Google restricted-scopes verification process (CASA audit)** —
   long, costly, to start very early.

### 2.4 Set-based concurrent engineering — explore, then eliminate
Explore in parallel, converge by elimination on measured criteria (no opinions,
figures):

| Decision | Option A | Option B | Option C | Elimination criterion |
|---|---|---|---|---|
| Windows shell | **Tauri 2 (WebView2)** | Slint/egui native | Electron | RAM, size, dev speed, web reuse |
| Shared UI | **Shared TS/React desktop+web** | Separate native UIs | — | double-maintenance cost |
| Local storage | **SQLite + FTS5** | SQLite + Tantivy | Maildir files | search perf on 100k msgs |
| Microsoft access | **IMAP+OAuth** ✅ settled | ~~Graph API~~ (plan B) | both | reliability, quotas, effort |
| Web | Shared sync backend | Rust core in WASM + WebSocket proxy | — | infra cost, privacy |

The options in bold are the CE's starting hypotheses; the spikes confirm or kill
them. **Microsoft access was settled against the initial hypothesis**: the spike
refuted Graph's decisive argument ("IMAP is doomed") and measured an overwhelming
asymmetry of effort — see [ADR 0006](adr/0006-microsoft-imap-oauth2.md). Graph
remains plan B, costed, with its three switch-over signals.

**Phase 0 deliverable:** finalized concept paper + frozen decisions + perf budgets
validated on prototypes. **Gate:** design review; v1 is coded only after.

---

## 3. Target architecture (hypothesis to validate in Phase 0)

```
┌────────────────────────┐   ┌────────────────────────┐
│   Desktop Windows      │   │        Web             │
│   Tauri 2 + WebView2   │   │   SPA (same TS UI)     │
│   UI TypeScript        │   │                        │
└───────────┬────────────┘   └───────────┬────────────┘
            │ IPC Tauri                  │ HTTPS/WebSocket
┌───────────▼────────────┐   ┌───────────▼────────────┐
│      mail-core (Rust)  │   │  sync-server (Rust)    │
│  ─ domain (Message,    │   │  = same mail-core,     │
│    Thread, Folder…)    │   │  hosted, multi-tenant  │
│  ─ sync engine         │   └───────────┬────────────┘
│  ─ action queue        │               │
│  ─ SQLite + FTS5       │        IMAP / SMTP / Graph
│  ─ IMAP/SMTP/OAuth     │
└───────────┬────────────┘
     IMAP / SMTP / Graph
```

**Key principle: one brain only.** `mail-core` (Rust crate) holds 100% of the
business logic, sync, and storage. The desktop embeds it locally; the web runs it
server-side. The UI (TypeScript) is shared between both targets and stays "dumb":
it displays state and emits intents.

**Cargo workspace layout:**

```
wind/
├── crates/
│   ├── mail-core/        # domain + sync + storage (zero UI dependency)
│   ├── mail-protocols/   # IMAP, SMTP, Graph, OAuth (behind traits)
│   └── sync-server/      # HTTP/WS exposure of mail-core (phase 4)
├── apps/
│   ├── desktop/          # Tauri 2
│   └── web/              # SPA (phase 4)
└── docs/                 # this plan, ADRs, perf budgets
```

**Data model (inspired by the JMAP model, saner than the IMAP model):**
`Account`, `Mailbox`, `Email` (envelope separate from body), `Thread`,
`PendingAction`. "Envelopes-first" sync: the list is usable immediately, bodies
are loaded on demand and cached.

**Security (non-negotiable points):**
- Never a hardcoded or plaintext credential: OAuth tokens in the Windows
  Credential Manager via `keyring`; IMAP passwords encrypted at rest.
- TLS everywhere (`rustls`), no unencrypted fallback.
- HTML sanitized by `ammonia` + sandboxed iframe + strict CSP; remote images
  blocked by default; no execution of JS from mail, ever.
- `cargo audit` + `cargo deny` in CI.

---

## 4. Phased development plan (pulled flow, quality gates)

Each phase delivers a product that is **usable and tested**, not a stack of
layers. Jidoka rule: any data-loss or security defect stops the line.

### Phase 1 — Walking skeleton: "I read my mail" (4-5 weeks)
- One Gmail account via OAuth PKCE; INBOX envelope sync to SQLite.
- Tauri shell: virtualized list + reading a message (sanitized HTML).
- Full CI from day 1: fmt, clippy `-D warnings`, tests, coverage ≥ 80%,
  `cargo audit`, automated perf-budget benchmarks.
- **Gate 1:** startup < 1 s, RAM < 200 MB, list at 60 fps on 50 000 real messages.

### Phase 2 — Triage and writing: "I work in my mail" (4-5 weeks)
- Actions: read/unread, archive, delete, move, flag — optimistic in the UI,
  replayable offline action queue with reconciliation.
- Compose, reply, forward; SMTP sending with an "outbox" queue (never a lost
  send); synced drafts.
- Full keyboard shortcuts (Superhuman's weapon).
- **Gate 2:** zero action loss proven by network-outage/crash tests; E2E of
  critical journeys (read, sort, reply) green.

### Phase 3 — Search, multi-account, scale (4 weeks)
- Full-text FTS5 search (< 100 ms on 100k messages), from/to/date/attachment
  filters.
- Multi-account (Gmail + Microsoft + generic IMAP), unified inbox.
- Attachments, Windows notifications, conversation threading.
- **Gate 3:** budgets held with 3 accounts / 200 000 combined messages.

### Phase 4 — Web (5-6 weeks)
- `sync-server`: the same `mail-core` exposed over HTTP/WebSocket, multi-tenant,
  encryption at rest, sessions.
- The same TypeScript UI deployed as an SPA; functional parity for
  reading/triage/writing.
- **Gate 4:** full security review of the server (it holds users' tokens —
  critical surface); pentest before any opening.

### Phase 5 — Hardening and beta (3-4 weeks)
- MSIX installer + signed auto-update; opt-in crash telemetry.
- Closed beta, 20-50 users; the CE goes through every piece of feedback
  (genchi genbutsu).
- Kaizen: a weekly iteration on observed frictions, not imagined ones.
- **Gate 5:** 2 weeks without a critical defect → launch.

**Early-start milestone (from Phase 0):** the Google verification file (Gmail
restricted scopes) and Azure AD app registration — the audit timelines (several
months for Google/CASA) are on the critical path to public launch.

---

## 5. Built-in quality (jidoka) — standing rules

1. Systematic TDD: the sync engine is developed against a **simulated IMAP
   server** (fixtures of real-world quirks: Gmail, Outlook, Dovecot, OVH…).
2. Coverage ≥ 80% on `mail-core`; E2E (Playwright on the UI) covers critical
   journeys; property tests (`proptest`) on parsing and reconciliation.
3. Perf budgets in CI: a benchmark that regresses past budget = red build.
4. Zero `unwrap()` in production, typed errors (`thiserror`) in the crates,
   context (`anyhow`) in the apps.
5. Code review on everything, security review on: auth, external content
   parsing, storage, HTML rendering.

---

## 6. Organization and cadence (obeya)

- **The Chief Engineer** owns the concept paper, arbitrates every trade-off
  against the target user, and has the final word on scope. Default reflex:
  say no.
- Target team: 1 CE, 2 Rust devs (core/protocols), 1-2 TypeScript devs (UI),
  occasional design + security support. (Solo? The plan holds, phases stretch
  ~×2.)
- **Weekly obeya**: perf budgets displayed, progress by phase, top 3 risks,
  decisions to make. Any budget deviation is handled the same week.
- Every structuring decision = a short ADR in `docs/adr/`.

---

## 7. Major risks and countermeasures

| Risk | Impact | Countermeasure |
|---|---|---|
| Google restricted-scopes audit (CASA): long, costly | Blocks public Gmail launch | Start the file in Phase 0; beta limited to 100 users in the meantime |
| Real-world IMAP server quirks | Endless sync bugs | Fixture suite per provider; matrix of tested servers; QRESYNC optional |
| HTML rendering: security vs fidelity | XSS or broken newsletters | Phase 0 spike; corpus of 500 real emails as the rendering test set |
| The web doubles the surface (infra, security, cost) | Delay, security risk | Web in Phase 4 only, after a solid desktop; dedicated security review |
| Scope creep | Slow, late product | The concept paper lists the explicit NOs; the CE arbitrates |
| WebView2 missing/broken on some machines | Crash at startup | Evergreen runtime + detection/installation on first launch |

---

## 8. Measuring success

- **Product:** perf budgets held continuously (§1); crash-free sessions > 99.5 %.
- **Beta usage:** ≥ 60% of testers still use it as their main client after
  30 days; reduced morning triage time (measured, not self-reported).
- **Engineering:** lead time for a fix < 48 h; zero critical defect open > 7 days.

---

## 9. Immediate next actions

1. Restructure the repo into a Cargo workspace (`crates/mail-core`,
   `apps/desktop`) — the current `src/main.rs` (hardcoded password) is
   replaced by the OAuth spike.
2. Launch the 4 Phase 0 spikes (§2.3) and the set-based grid (§2.4).
3. Create the Google Cloud project + Azure AD app; open the Google
   verification file.
4. Schedule the user interviews (genchi genbutsu).
5. Set up CI (fmt, clippy, tests, coverage, audit, bench).

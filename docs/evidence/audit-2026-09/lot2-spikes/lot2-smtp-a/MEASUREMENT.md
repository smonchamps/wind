# SMTP option A — conservative public-error classifier

Measured 2026-09-06 on Windows 10.0.26200 ARM64, rustc 1.97.1
(aarch64-pc-windows-msvc). Isolated worktree baseline: d13811c. No production
files changed. Standalone workspace pins lettre 0.11.22 with Wind's features,
pool disabled. Cargo resolved 105 packages offline; its standalone lockfile is
retained and is not asserted identical to the production transitive lockfile.

## Protocol

Ten loopback TCP dialogues, three repetitions each. Each creates a fresh
SmtpTransport and a bounded fixture thread on 127.0.0.1 with an ephemeral port.
No TLS, credentials, real addresses, delivery services or external traffic.
Two recipients use example.invalid. Read/write timeout: two seconds; accept
deadline: five seconds. Every fixture thread is joined. Raw results and first
transcripts are in results.txt. All repetitions produced identical classes
and complete-DATA counts. Complete DATA means the fixture read the terminating
dot; it does not imply acceptance in scenarios returning an explicit refusal.

Baseline reproduces Wind's current mapping. Candidate A keeps SmtpTransport
unchanged: require final 250 for Sent; explicit refusals preserve the existing
4xx/authentication-5xx policy; unclassified failures become Unknown. Pinned
public client/TLS/shutdown predicates are treated as safe pre-submission
errors, but their branches are not exercised by this plaintext fixture.

## Results

| Scenario | Baseline | Candidate A | Complete DATA per attempt |
|---|---|---|---:|
| Greeting EOF | Transient | Unknown | 0 |
| MAIL response EOF | Transient | Unknown | 0 |
| First RCPT response EOF | Transient | Unknown | 0 |
| First RCPT accepted, second 450 | Transient | Transient | 0 |
| First RCPT accepted, second 550 | Permanent | Permanent | 0 |
| EOF after complete DATA, acknowledgement lost | Transient | Unknown | 1 |
| Final 451 | Transient | Transient | 1 |
| Final 550 | Permanent | Permanent | 1 |
| Final 250, QUIT response EOF | Sent | Sent | 1 |
| Final 354 | Sent | Unknown | 1 |

Greeting, MAIL, RCPT and final-DATA EOF share exactly these exposed predicates:
status=None, response=true, client=false, tls=false, timeout=false. Their
error text is also identical: response error: incomplete response. The final
250 result survives failed QUIT. Neither second-recipient refusal issues DATA.

Replay simulates two flush opportunities, retrying only Transient. The fixture
accepts the payload and drops its acknowledgement on every attempt, keeping
the same Message-ID: baseline submits two complete payloads (one duplicate),
candidate submits one (zero duplicates). This is a policy/transport simulation,
not a persisted outbox integration test.

Whole successful measurement: 33 TCP connections in 61 ms (30 scenario
repetitions plus three replay attempts). The measured time excludes compilation
and is a correctness-fixture cost, not production network latency. Warm build
after fixture correction reported 0.70 s. The initial standalone compilation
was not timed reliably, so no cold-build claim is made.

## Size and integration cost

After rustfmt: 253 Rust source lines total. Candidate classifier is lines
30–48, 19 physical lines; result enum adds seven lines. Remaining source is
baseline policy, dialogue fixture, measurement and replay. No new dependency
is required in production. Industrialization still requires a core Unknown
error variant, atomic persisted quarantine with its reason, UI state mapping,
and outbox restart/replay tests. Those changes are outside this spike and their
cost is not measured here.

## Limits

No STARTTLS/implicit TLS, password/XOAUTH2, real provider, timeout, mid-body
reset, persistent storage, or concurrent-process behavior was exercised.
Client/TLS/shutdown exceptions are based on pinned-source inspection only;
shutdown is unreachable without pool. Nine of thirty scenario repetitions
(greeting/MAIL/RCPT EOF) quarantine a proven unsubmitted message. This is the
measured conservative cost, not an estimated real-world frequency. Changing
lettre's version, features, error construction or send implementation requires
repeating the classification audit and fixture.

The first fixture run exposed Windows' inheritance of nonblocking mode on an
accepted socket. Setting that accepted socket to blocking mode corrected the
fixture; only the subsequent complete run is recorded in results.txt.

## Reproduction

From the main repository:

```powershell
cargo run --offline --manifest-path .claude/worktrees/lot2-smtp-a/spikes/lot2-smtp-a/Cargo.toml --target-dir target/lot2-spikes
```

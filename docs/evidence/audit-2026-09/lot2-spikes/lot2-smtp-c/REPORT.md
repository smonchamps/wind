# Option C: explicit SMTP transaction through lettre public APIs

2026-09-06, baseline d13811c. Standalone throw-away crate, no production edits.

## Protocol

Windows NT 10.0.26200.0; ARMv8 64-bit Family 8 Model 1 Revision 201, Qualcomm (PROCESSOR_IDENTIFIER; precise CPU query denied by host). rustc 1.97.1 (8bab26f4f 2026-07-14), debug build. lettre pinned exactly 0.11.22, same six enabled production features and no default features. Root `target/lot2-spikes` shared dependency build cache. Offline compilation completed in 1.79 seconds including shared target lock wait (warm dependency cache; not a clean build benchmark).

Each case opens a new ephemeral 127.0.0.1 TCP listener and a fresh client connection. Plaintext fixtures only; synthetic sender and two recipients under example.test, no credentials or external services. Client timeout 500 ms, server timeout 2 s. Three repetitions per case, 30 assertions of outcome and 30 of payload count; initial run and saved second run both pass. CSV is the saved second run. Elapsed time includes connection, fixture commands and thread join, excludes listener bind and thread creation. Tiny ASCII payload and loopback timings are not network-performance predictions.

Run from the Wind root:

```powershell
cargo run --offline --manifest-path .claude/worktrees/lot2-smtp-c/spikes/lot2-smtp-c/Cargo.toml --target-dir target/lot2-spikes
```

## Saved measurements

| Fixture | Outcome in 3/3 runs | Complete payloads per run | Min / median / max microseconds |
|---|---|---:|---:|
| Greeting EOF | Retry | 0 | 299 / 421 / 1379 |
| MAIL EOF | Retry | 0 | 329 / 456 / 495 |
| First RCPT EOF | Retry | 0 | 390 / 416 / 445 |
| Second RCPT 450 after first accepted | Retry | 0 | 418 / 487 / 487 |
| Second RCPT 550 after first accepted | Refused | 0 | 389 / 507 / 708 |
| EOF after complete DATA payload | Unknown | 1 | 403 / 449 / 477 |
| Final DATA 451 | Retry | 1 | 418 / 463 / 1214 |
| Final DATA 550 | Refused | 1 | 466 / 481 / 528 |
| Final DATA 250, then QUIT EOF | Sent | 1 | 500 / 560 / 572 |
| Final DATA 354 (unexpected positive response) | Unknown | 1 | 504 / 582 / 760 |

For both second-recipient rejections, fixture additionally checks EOF before any DATA command: accepting one recipient has not delivered anything. A complete payload count is receipt through the SMTP terminating dot, not a server-side delivery assertion. Explicit final negative replies classify rejection; absent/invalid final replies preserve uncertainty. Only final 250 reports Sent; losing the QUIT response cannot undo confirmed acceptance.

## API proof and measured code size

Inspected pinned `lettre-0.11.22/src/transport/smtp/client/connection.rs`, `commands.rs`, `extension.rs`, and `error.rs` before prototyping. Public `SmtpConnection::connect`, `command(Mail/Rcpt/Data)`, `message`, and `quit` suffice. `message` owns dot escaping and termination. `Error::is_transient` and `is_permanent` identify explicit 4xx and 5xx replies. A local payload-start boundary disambiguates all other errors. The client does not retain or pool failed connections.

126 physical source lines: 44 lines through outcome classification and transaction, 80 fixture/driver lines plus separator. There is no lettre fork or extra dependency. MAIL requires 250, RCPT permits 250/251/252, DATA requires 354, final acceptance requires 250. This protects against `read_response` treating all positive SMTP classes as Ok, including unexpected final 354.

## Limits and industrialization cost

TLS, mandatory encryption policy, STARTTLS re-EHLO, authentication, OAuth token refresh, SMTPUTF8 and 8BITMIME capability negotiation are **not implemented or measured**. They remain Wind's responsibility when replacing its high-level SmtpTransport call with SmtpConnection. MIME construction, persistent queue ownership, account deletion, process crashes, retries across restarts, Sent-folder reconciliation, recipient editing, cancellation and UI treatment are outside this spike. AUTH errors need a dedicated policy; Retry here denotes delivery-safe, not a recommendation to retry invalid configuration indefinitely.

The exact 44-line transaction is not production-ready. Industrialization entails reproducing the high-level connector's TLS/auth setup with existing platform verifier and timeouts, copying/adapting lettre's public-extension checks for internationalized addresses/body, preserving structured error causes, exposing outcome types, plus unit/integration coverage of each phase. Estimated scope: one SMTP adapter refactor and tests for setup policies, capability checks and all transaction outcomes; queue integration remains separate. No development-time estimate is measured by this spike.

Only orderly EOF and explicit final negative replies were injected. Partial socket writes, TLS records, timeouts, malformed/multiline replies, server capability changes and network proxies were not measured. Classification routes errors returned by `message` without explicit negative SMTP replies to Unknown even if no payload byte was written; it is deliberately conservative because the public method exposes no byte-level progress. High-level `connect` consumes greeting/EHLO internally; malformed positive setup replies are not separately validated here, although no payload is sent before the explicit command sequence.

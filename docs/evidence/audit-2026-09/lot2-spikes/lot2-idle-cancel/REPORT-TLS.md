# TLS completion of the IDLE cancellation spike

Date: 2026-09-06. Windows ARM64, rustc 1.97.1.
No production code, verifier, account data or external connection touched.

## Fixture and trust

The test uses the exact production rustls version, 0.23.41, with ring and TLS 1.2
support enabled, and imap 3.0.0-alpha.15. A fresh RSA root and localhost server
certificate were generated with .NET CertificateRequest by make-certs.ps1.
No OS certificate store was modified. Only the synthetic client config's
RootCertStore trusts that root, using normal certificate/name verification.
The server listens exclusively on 127.0.0.1. The key in certs is a disposable
fixture private key, not an account credential. TLS tests use negotiated default
TLS behavior; TLS 1.2 was not separately forced.

```
cargo run --offline --bin tls
```

Full evidence: src/bin/tls.rs and measurements-tls.csv.
The earlier cleartext executable is now selected explicitly:
`cargo run --offline --bin lot2-idle-cancel-spike` (optionally `-- quiet`).

## Methods and results

Both placements retain a logical IDLE heartbeat of 180 s, a logical fallback of
120 s when imap restores None, and 100 ms native TCP read slices.
- Above: retry timeouts around StreamOwned::read.
- Below: retry timeouts in the transport Read that StreamOwned itself calls.

The timer starts when the token is stopped. Thread completion includes the IDLE
handle destructor. Ten repetitions per placement and cancellation phase:

| Placement | Phase | Min ms | Median ms | Max ms |
|---|---|---:|---:|---:|
| Above rustls | quiet IDLE | 76.035 | 80.192 | 89.659 |
| Above rustls | waiting for final reply to DONE | 74.639 | 79.731 | 86.613 |
| Above rustls | partial TLS record | 76.676 | 80.852 | 83.738 |
| Below rustls | quiet IDLE | 69.376 | 78.209 | 83.722 |
| Below rustls | waiting for final reply to DONE | 75.738 | 81.002 | 89.137 |
| Below rustls | partial TLS record | 78.971 | 80.577 | 82.358 |

For partial records, the server builds an encrypted application record, sends its
first seven bytes (five-byte header plus two bytes of encrypted payload), then
withholds the rest until the test worker has completed cancellation. No full
plaintext EXISTS is available during this phase.

A second partial-record experiment DOES NOT cancel: it withholds the rest for
350 ms, then sends the remainder. All 20 runs recover the intact EXISTS, exit the
watch normally and exchange DONE. Each run absorbs three native read timeouts;
no DONE is sent while the record is incomplete. This is evidence that routine
slice timeouts, including timeouts surfaced above rustls, do not corrupt this
partial-record path on the tested version.

Cancellation is TERMINAL. Every cancelled session is dropped and never reused.
The experiment does not clear the token to claim a cancelled TLS/IMAP stream can
be resumed safely. In particular, an unfinished DONE dialogue and buffered
partial replies make reuse unnecessary and unsafe as an application contract.

## Important crate behavior

Cancellation during IDLE or a partial record reports ConnectionAborted.
Cancellation while the destructor waits for DONE's final reply still leaves the
already-produced `wait_while` result as Ok(MailboxChanged): imap Handle::drop
ignores terminate errors. In every such run the token was false at completion.
Therefore check the stopped token AGAIN after watch returns, before any refresh,
light pass, session publication or reconnect. Do not infer permission to continue
from Ok(MailboxChanged).

Both TLS prototypes check cancellation before Read, Write and flush. This stops
the destructor from initiating a new DONE after cancellation of a quiet IDLE.
When DONE was already sent, its response Read is interrupted in the same bound.
This does not prove cancellation of a write already blocked inside the OS; the
fixture never fills a socket send buffer.

## Wakeup cost: 10-second quiet witnesses

Each placement was observed for 10 seconds without cancellation or server data,
then released by a single EXISTS. No premature completion was allowed.

| Placement | Native read calls during 10 s | Native timeouts | New StreamOwned::read entries | DONE before release |
|---|---:|---:|---:|---:|
| Above | 90 | 90 | 90 | 0 |
| Below | 90 | 90 | 0 | 0 |

Below remains inside the original StreamOwned::read while the native read loop
wakes. Above re-enters StreamOwned after every native timeout. Both have roughly
nine local wakeups per second per idle connection on this Windows timer. These
are counter measurements, NOT CPU-time, battery or energy measurements. There
was no extra heartbeat, DONE, login or IMAP command during either quiet window.

The CSV window counters apply to quiet/partial_resume experiments; other rows
leave those columns at zero because their window is cancellation latency.

## Placement tradeoffs and remaining limits

Above rustls minimally extends the existing BoundedStream. The tests prove its
ordinary slice retries preserve a fragmented record. It repeatedly traverses
StreamOwned::read and its complete_io machinery (90 extra entries/10 s here).
A deadline surrounding a full plaintext Read differs from a deadline reset at
each underlying TCP read; choose and test that contract explicitly. Checking only
outer Write cannot stop a native write already in progress, nor intercept every
TLS internal read/write initiated while another outer method is executing.

Below rustls keeps polling below TLS; slice timeouts are hidden from rustls until
the logical deadline. It also provides one transport-level place for cancellation
checks when TLS needs I/O from inside Write/flush/handshake. This needs a different
StreamOwned socket type and adaptation of InnerSocket/set_read_timeout. It is a
larger integration than extending BoundedStream. The measured latency does not
establish a significant advantage over above; the wakeup counter difference is
real, but no CPU/energy benefit was measured.

Neither placement changes the production heartbeat. The token must apply ONLY to
the dedicated watcher connection, not ordinary polling or SMTP transports.

Not measured: macOS, Windows x64, a forced TLS 1.2 session, socket send-buffer
saturation, TLS handshake cancellation, DNS/connect cancellation, sleep/backoff
interruptibility, or a complete 180-second logical timeout. Do not turn these
results into a whole-account quiescence guarantee. Existing account generation,
locking and job-drain rules remain necessary.

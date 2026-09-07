# Lot 2: cancelling a blocked IMAP IDLE read

Date: 2026-09-06. Isolated spike only. No production file changed by this spike.
Host: Windows ARM64 (`aarch64-pc-windows-msvc`), rustc 1.97.1.
Pinned protocol crate: imap = 3.0.0-alpha.15, no default TLS feature.
Transport: synthetic cleartext TCP on 127.0.0.1 only; fabricated login strings.

## Result

A cloned TCP socket's shutdown(Both) DOES NOT promptly interrupt the already
blocked original recv on this measured Windows host. Sharing the same socket
handle with Arc<TcpStream> and shutting it down is also unsuccessful.
Do not ship either mechanism as proven cancellation.

A cancellation token checked between local socket read waits of at most 100 ms
exits in 74.963-89.963 ms after cancellation (10 runs), including Drop of the
imap IDLE handle. The logical heartbeat stays 180 seconds. A separate quiet
witness proves these read wakeups do not exit IDLE before a server event.

| Mode | Runs | Min ms | Median ms | Max ms | Outcome |
|---|---:|---:|---:|---:|---|
| Atomic alive flag only | 10 | 302.193 | 304.749 | 314.944 | Still blocked at 300 ms; server EXISTS releases it |
| try_clone + shutdown(Both) | 10 | 2979.242 | 2984.622 | 2989.550 | Server EXISTS at 3 s releases it; not shutdown |
| Arc same handle + shutdown(Both) | 10 | 2972.893 | 2982.451 | 2990.645 | Same failure |
| Token + 100 ms local read slices | 10 | 74.963 | 81.590 | 89.963 | ConnectionAborted before any server event |
| Token read slices, no cancellation | 10 | 301.839 | 308.099 | 314.198 | Still blocked at 300 ms; server EXISTS releases it |

The witness deliberately does not wait 180 seconds: it checks continued blocking
at 300 ms and then sends EXISTS. The shutdown variants use a 3-second emergency
server release so a failed mechanism cannot hang the experiment for minutes.
Their completion is MailboxChanged, demonstrating that shutdown did not cancel
the pending read. No claim is made about a fully elapsed 180-second heartbeat.
The CSV `still_blocked_at_300ms` is measured only for witness modes; false in the
shutdown modes does NOT mean early completion. Use completion_us and outcome.

## Reproduction

Run from this directory:

```
cargo run --offline
cargo run --offline -- quiet
```

Measurements are in measurements-final.csv and measurements-quiet.csv.
measurements.csv retains the earlier three-mode replication (same negative
shutdown result). main.rs includes production-equivalent timeout-floor behavior,
real imap Session::idle, timeout(180s), keepalive(false), and Handle::drop in the
completion timing. It uses fabricated IMAP LOGIN and IDLE responses.

## Production API implications (design only)

Current mail-imap lib.rs:
- connect_client: line 49, creates TcpStream at 64 and passes ownership to
  tls_stream at 86 or 111. A TCP clone would have to be obtained before this move.
- tls_stream: line 122, drives TLS handshake to completion before returning.
- BoundedStream: line 186, currently wraps bare or rustls stream, sets timeouts on
  the ORIGINAL socket, and enforces the 120 s floor when imap requests None.
- connect_xoauth2 / connect_password: lines 336 / 355, currently return only an
  ImapServer. New watcher-specific cancellation, if approved, must be carried
  through these constructors and connect_client, or configured in the stored
  transport through a shared control. Ordinary polling connections keep None.
- watch: line 567, blocks in Session::idle wait_while with the 180 s heartbeat.

A token-aware transport would preserve a logical timeout separately from the
100 ms native socket wait; retry WouldBlock/TimedOut until the original logical
read deadline, and return ConnectionAborted when the token is stopped. Simply
setting a 100 ms timeout and returning it to imap would incorrectly produce a
100 ms heartbeat and extra DONE/IDLE traffic. That is NOT the tested algorithm.
The crate's IDLE callback cannot observe a cancellation flag while no server
response arrives, as the witness demonstrates.

The current watcher already owns an Arc<AtomicBool>; that token can be scoped to
its dedicated IMAP connection. Never attach it to SMTP or apply thread-wide I/O
cancellation to a thread that can perform unrelated operations. This experiment
makes no SMTP connection and does not test SMTP cancellation.

## Limits before implementation

- TLS was NOT exercised. Existing BoundedStream is outside StreamOwned and the
  handshake occurs before that wrapper; prove timeout retry/cancellation through
  the pinned rustls version with a synthetic TLS peer before adopting the sliced
  read design. Cancellation during handshake, login, DNS and connect is not
  established here.
- The prototype checks cancellation in Read, not Write/flush. Handle::drop sends
  DONE after wait_while returns. It was quick on loopback; a blocked TLS/socket
  write is a separate risk. A production token-aware writer should refuse new
  writes on cancellation, then drop the dedicated connection, and needs tests.
- The code uses portable std socket APIs but macOS was NOT run. No measured macOS
  compatibility or shutdown latency is claimed. Run this same spike on both Mac
  architectures; do not extrapolate Windows shutdown behavior to Unix.
- Idle read slicing introduces roughly 10 local wakeups per second per connected
  idle watcher, with no extra protocol commands. CPU/energy impact was NOT
  measured. The 100 ms figure is an experimental candidate, not a product budget.
- This does not solve watcher sleep/backoff delays (5 s sleep, up to 60 s retry),
  nor the initial light-pass operation. Account removal must distinguish stopping
  IDLE from quiescing all account jobs.
- Set stop state without holding a lock needed by the worker. Never join a watcher
  while holding accounts, watchers, per-account job, or database locks. Existing
  reconcile holds watchers; remove/take controls under lock, release it, then wait.
- A connect/token-registration race must be handled: if removal stops a watcher
  before it publishes its transport control, the newly created transport must
  see the already-stopped token before issuing further work. Reconnection needs
  the same account-generation fence; an old worker must not update fresh sessions.

## Dependency evidence

imap alpha.15 extensions/idle.rs:
- init sends IDLE and reads the continuation before applying the IDLE timeout.
- wait_inner blocks in Session::readline; the callback runs on parsed responses.
- wait_while applies the logical timeout and restores None on exit.
- Handle::drop calls terminate: writes DONE and waits for its tagged response.

Microsoft describes shutdown(SD_RECEIVE) as disallowing subsequent recv calls,
not as a promise to cancel a recv already pending:
https://learn.microsoft.com/en-us/windows/win32/api/winsock/nf-winsock-shutdown

The measurements, rather than this documentation inference, decide the negative
verdict for shutdown on this host.

TLS follow-up: REPORT-TLS.md supersedes the earlier untested-TLS limitation with measured above/below-rustls fixtures. It preserves the macOS and whole-account cancellation limits.

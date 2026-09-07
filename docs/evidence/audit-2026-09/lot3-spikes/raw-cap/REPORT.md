# Lot 3 S04: bounded complete-MIME throwaway spike

Follow-up: `CANDIDATE-CEILINGS.md` reports parameterized 8/16/32 MiB measurements with actual conversion/sanitization. The 2 MiB results here are the retained initial snapshot; the harness now accepts optional cap/conversion arguments and links mail-render, so rerun memory baselines can differ from this initial snapshot.

2026-09-06. Isolated detached worktree at a58c1cf. This directory contains throwaway code only; no production edits, commits, real messages, accounts, credentials, external services, or full gate. Workflow and job skill read before experimentation.

## Result

An actual `imap 3.0.0-alpha.15` parser protected at its plaintext `Read` boundary refused a lying/missing-size 8 MiB literal, 8 MiB newline-free response, and three-body aggregate at exactly **2,105,344 admitted response bytes**. `uid_fetch` returned no bodies on these errors. The same unbounded parser admitted the full 8,388,659-byte literal response or 8,388,608-byte newline-free response. All poisoned-session NOOP attempts failed, and an independent eligible 64 KiB message completed on a replacement session.

This establishes a workable input-volume boundary with the existing complete-MIME parser. It does **not** establish a 2 MiB heap bound, a whole-app 200 MB memory bound, or complete accessibility of large mail. The 2 MiB raw ceiling and 8 KiB protocol allowance are experimental instruments, not proposed or approved product budgets.

## Evidence and reproduction

- `results.json`: all 34 fresh-process measured bounded/control records.
- `measured-table.md`: generated per-case bytes, process memory and elapsed comparison.
- `corpus/manifest.json`: generated file sizes and SHA-256 identities, shared with the partial-fetch sibling.
- `src/main.rs`: actual pinned IMAP parser + stream quota, metadata admission, raw semantic ceiling, MIME decode, poison and independent progress.
- `corpus.py`, `measure.ps1`, `report.py`: repeatable corpus, external Windows process measurements and assertions.

Run from this worktree with PowerShell:

```powershell
Set-Location C:\Users\smonc\OneDrive\Documents\Repositories\wind\target\lot3-spikes\raw-cap
python spikes/raw-cap/corpus.py
cargo build --offline --manifest-path spikes/raw-cap/Cargo.toml --target-dir spikes/raw-cap/target
./spikes/raw-cap/measure.ps1
python spikes/raw-cap/report.py
```

Individual case:

```powershell
./spikes/raw-cap/target/debug/lot3-raw-cap-spike.exe lying-8m bounded C:/Users/smonc/OneDrive/Documents/Repositories/wind/target/lot3-spikes/raw-cap/spikes/raw-cap/corpus
```

Windows local run; rustc 1.97.1, cargo 1.97.1, Python 3.14.3; debug build. Standalone workspace and target, exact IMAP alpha15 and mail-parser 0.11.5, generated local Cargo.lock. Other transitive versions resolved offline into that isolated lock may differ from the production lock. No source cherry-picks intended.

## Protocol and what each measurement means

The synthetic `Read + Write` server releases command replies only after command flush. It streams fixture files and repeated bytes, avoiding constructing an 8 MiB server buffer in the measured process. There is no socket or TLS handshake: this is generated plaintext at the location immediately **above TLS and below the IMAP buffered reader/parser**, the same intended location as the production `BoundedStream`. TLS ciphertext buffering, socket timeout behavior, network latency, SELECT and real authentication are outside this experiment.

Every run starts a new executable process, logs in with fixed synthetic strings, performs one RFC822.SIZE FETCH, and (unless honestly oversized) one complete BODY.PEEK[] FETCH. The generated size variants are honest, absent, and lying small (1,024). Body command budget is 2,097,152 + 8,192 bytes; metadata has its own bounded command allowance. Each read is truncated to remaining allowance before it reaches the IMAP parser. Exhaustion and deadline set persistent poison; writes then fail. A completed response whose raw MIME exceeds 2,097,152 is refused semantically, without poisoning an already synchronized connection.

`body_command_admitted_bytes` counts actual plaintext handed to IMAP, including literal framing/tagged completion, not advertised sizes. `max_transport_read` is at most 8,192 bytes in these runs. `returned_body_max` and `returned_body_sum` measure the borrowed raw slices observable only **after uid_fetch returns**; they do not measure internal Vec capacity. Error returns expose no successful Fetches, so their returned maximum is zero even though a partial response was allocated. A cap+1 MIME can return within the protocol allowance and then be semantically refused: the largest returned bounded raw slice in the corpus is therefore 2,097,153, with zero parsed messages for that case. The raw ceiling is an acceptance boundary; the response quota is the earlier input-allocation defense.

`elapsed_ms` includes size fetch, body fetch and MIME parsing; excludes login, NOOP check, replacement session and the final 400 ms sampler hold. One run per cell, no distribution; debug timings are illustrative, not production latency estimates. Normal/preflight command counts are 3/2 including LOGIN, or 2/1 without LOGIN. The explicit NOOP probe adds one attempted command, but zero server-visible commands after poison. Replacement progress costs a further 3 commands including its synthetic LOGIN. Real reconnect costs are not measured.

The deadline fixture sleeps up to 5 ms per read against a 20 ms absolute body deadline, with a remaining-time transport wait. Windows scheduling makes elapsed time exceed exactly 20 ms. This proves synthetic deadline exhaustion and poison propagation through alpha15; **it does not prove real TLS/socket cancellation**. Production must set each underlying socket read timeout from remaining absolute time and recheck after reads, in addition to retaining the existing timeout floor. Checks only between commands do not bound a blocked read.

`measure.ps1` samples the child using an independent PowerShell process. `os_peak_working_set_bytes` reads Windows/.NET PeakWorkingSet64 (OS peak resident working set, includes shared pages). `os_peak_paged_memory_bytes` reads PeakPagedMemorySize64 (OS peak paged memory/process commit metric, not private resident working set). `sampled_peak_private_commit_bytes` is merely the largest observed PrivateMemorySize64; the 2 ms requested poll interval is not guaranteed, and short peaks can be missed. **Exact peak private working set is unavailable in this harness.** Reported peaks include runtime, fixture file I/O, IMAP parsing, MIME decoding, and the subsequent 64 KiB replacement check. No in-process estimated byte counter is mislabeled as process memory.

## Findings

1. Source root: cached `imap-3.0.0-alpha.15/src/client.rs:1535` creates `Vec::new`, `read_response_onto` repeatedly calls `readline`, and `readline:1640` calls `read_until(LF, into)`. Incomplete literals continue growing the same response. `uid_fetch:658` obtains this completed response before returning Fetches. `types/fetch.rs:146` exposes body as borrowed bytes. Therefore checking `fetch.body().len()` alone is too late. The measured unbounded 8 MiB and no-newline cases exercise both paths.
2. Honest 8 MiB metadata avoided the body command entirely. Missing or false size still reached the quota at 2,105,344 bytes. The guard does not trust RFC822.SIZE for safety.
3. An aggregate of three individually eligible 1 MiB messages exhausted the same command quota and returned no bodies, including the first fully received one. Bounding only each message is insufficient. Production must bound group sum and consume/cache each bounded group before starting the next; returning all backfill bodies in one final Vec retains an unbounded aggregate. Hostile/unsolicited responses still require the stream quota even with honest group planning.
4. The 82-byte malformed multipart fixture was accepted by mail-parser in both modes (one parsed Message). A volume cap does not validate MIME or make the permissive parser strict.
5. The 7,332-byte nested multipart/mixed fixture contains a message/rfc822 with multipart/alternative and an inner base64 attachment, plus outer base64 attachment. Both modes recover inner.bin **620/620 bytes** and outer.bin **4,096/4,096 bytes** exactly against independent generated binary files. This is full-message parser behavior with no part-number translation. It is evidence for this corpus only.
6. Missing-size raw cap−1 and cap messages completed; cap+1 response fit the command allowance but was rejected by the raw semantic ceiling. Because its tagged completion was consumed, NOOP succeeded safely in that case. Quota/deadline errors poison permanently; the test never re-arms an errored session.
7. Every case allowed a subsequent independent 64 KiB message on a replacement session. This demonstrates a continuation mechanism, not persistent scheduler fairness. Production still needs durable resource-refusal state and fair retry/admission so the same oversized UID/group is not retried forever ahead of other mail.

## Tradeoff for the parent comparison

**Advantages supported here:** small implementation boundary below the pinned parser; catches size lies, absent size, newline-free responses and aggregate responses; keeps existing complete-MIME decode and nested attachment semantics; normal message content uses one size FETCH plus one body FETCH; honest oversize saves body transfer.

**Costs/limitations supported here:** a hard raw ceiling makes large bodies, imported drafts and attachment access unavailable through this path, even when the desired text/attachment is small; quota failure loses the whole in-flight group and connection; reconnection has real unmeasured costs; accepted bounded raw input can still multiply during parsing, charset conversion, base64 decode, HTML sanitization, images, final Vec accumulation and concurrent workers. The largest working-set peak within bounded cases differed from the raw quota, demonstrating why admitted bytes are not process memory.

This option needs an explicit product behavior for oversize mail (retain envelope, durable visible resource limitation, allow unrelated mail) and coverage of direct body, draft and attachment full-MIME paths. It cannot silently preserve an unconditional promise to retrieve every RFC message. No winner or ceiling is selected by this spike.

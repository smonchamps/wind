# Lot 3 S04 option: bounded partial MIME / attachment parts

2026-09-06. Throwaway spike on detached `a58c1cf28da9f4f9282fd7147d2c7a9a6f7e91bb`.
Only this worktree's `spikes/partial-mime/` was changed. No real messages,
accounts, vault, credentials, external services, production edits, commits, or gate.
Read `docs/WORKFLOW.md` and `.claude/skills/job/SKILL.md` as requested.

## Verdict

**A selected MIME part can be downloaded and decoded to disk with bounded
working buffers, without assembling the complete message.** Six selected-part
experiments preserve exact bytes against the pinned current `mail-parser`
0.11.5: base64 inner and outer attachments and the seven-bit attached
message/rfc822, on both the shared small fixture and a 17,219,602-byte large
variant. The large inner binary is 8,388,608 decoded bytes: 178 commands,
63,858-byte maximum base64 input (65,536-byte allocated capacity),
47,892-byte maximum allocated decoded block, zero raw reconstruction capacity, 7,577,600
bytes measured client peak working set.

**This prototype is not production ready or a complete partial-MIME replacement.**
An adversarial origin test demonstrates silent corruption: the pinned
`imap::types::Fetch::body()` / `section()` public getters discard the response
`<offset>` when returning data, and `Fetch::fetch` is private. The server returns
each chunk from offset zero, falsely echoing zero, and the prototype reports
success with the wrong final hash. A production implementation needs a checked
response origin, section, UID, and multiplicity contract, through a supported
dependency change or a lower-level bounded protocol adapter. Merely adding
partial requests to the current getters is insufficient.

**Reconstructing full raw MIME from partial reads fails the global RAM objective.**
Although the response allocation is bounded, an 8 MiB message produces an 8 MiB
raw Vec and a 15,634,432-byte peak working set; 2 MiB + 1 byte gives a 4 MiB Vec
capacity. Keeping the current full-message parser therefore still scales with
message size. Spooling first would move reconstruction to disk, but loading that
spool into the same full-message parser would restore the large allocation.

## Protocol and measurement

`run.py` creates a synthetic loopback IMAP peer in the Python runner. Every
measurement executes a **new, separate Rust client process**; fixture generation,
server buffers, and full-parser oracles are outside the measured process. Rust
uses the exact locked production versions imap 3.0.0-alpha.15, imap-proto 0.16.7,
and mail-parser 0.11.5. Standalone Cargo workspace and local target directory.

The common corpus generator is copied verbatim from sibling raw-cap's generator.
The three main raw sizes, boundary sizes, malformed message, and nested fixture
retain shared hashes in `corpus/manifest.json`. The extra large nested message
has SHA-256 `605bbf6f728abc31f589bde1fe5d83a007290f5bdf660511cd2cbee192b7e3d1`.
It contains an 8 MiB inner binary and 4 MiB unrelated outer binary. The runner
serves original section bytes for attached message/rfc822; serializing that
section with Python's mail library would alter bytes and invalidate the oracle.
The fixture-specific section extractor is deliberately **not** a general server.

Every data command requests `BODY.PEEK[section]<offset.65536>`. A short chunk
ends transfer; exact multiples require one empty EOF command. RFC822.SIZE is
observed, never used as an allocation bound or trusted EOF. A `Read` wrapper above
the socket admits at most **2,105,344 bytes per command** (2 MiB + 8 KiB response
overhead), including framing and aggregate/unsolicited bodies. This is a shared
experimental measurement device, **not an approved product budget**. Reads cap
the requested slice before the pinned library can append it to its response Vec.
There is no total-message ceiling in this option.

Metrics in `results/measurements.json`: 29 cases, exact commands, total plaintext
bytes admitted, maximum admitted per response, largest transport read and returned
literal, raw Vec capacity, decode input size, hashes, Windows process memory,
elapsed time, poisoning and reuse outcomes. Total admitted includes greeting and
login; command counts exclude login and include size and structure queries.
`GetProcessMemoryInfo` measures native PeakWorkingSetSize and PeakPagefileUsage;
PrivateUsage is current private bytes at the end, **not peak private bytes**.
The internal imap Vec capacity is private and was not directly instrumented.
Peak working set is not equivalent to Wind's whole-app private-memory budget.

Base64 decoding drops whitespace and retains incomplete quartets across chunks,
then writes decoded blocks directly to a file. Seven-bit sections write directly.
No rendering/sanitization, inline CID embedding, SQLite/WAL, TLS, app workers,
or filesystem reserve contention is included. A 3 s socket read timeout is present;
an absolute command/operation deadline and total-disk quota are **not** implemented.

## Measured results

Representative final run; bytes are exact, times are single-run local wall time
including handshake and disk writes, with scheduler noise. Remote latency is not
measured. Command counts and hashes are the useful comparison here.

| Case | Commands | Actual bytes admitted | Largest response admitted | Raw Vec capacity | Peak working set | Time ms |
|---|---:|---:|---:|---:|---:|---:|
| raw 64 KiB, honest size | 3 | 65,776 | 65,601 | 65,536 | 7,196,672 | 3.0 |
| raw 1 MiB, honest size | 18 | 1,049,879 | 65,607 | 1,048,576 | 8,384,512 | 7.1 |
| raw 8 MiB, honest size | 130 | 8,398,007 | 65,609 | 8,388,608 | 15,634,432 | 41.7 |
| raw 2 MiB + 1, absent size | 34 | 2,099,588 | 65,608 | 4,194,304 | 11,444,224 | 10.0 |
| nested inner base64, 620 decoded | 3 | 1,622 | 914 | 0 | 7,045,120 | 2.9 |
| nested attached message, 1,293 bytes | 3 | 2,066 | 1,358 | 0 | 7,069,696 | 2.2 |
| large inner base64, 8 MiB decoded | 178 | 11,493,147 | 65,613 | 0 | 7,577,600 | 159.5 |
| large outer base64, 4 MiB decoded | 90 | 5,746,696 | 65,609 | 0 | 7,569,408 | 170.0 |
| large attached message, 11,479,593 bytes | 178 | 11,493,240 | 65,611 | 0 | 7,430,144 | 218.2 |

Honest, absent and lying size (1,024) all reconstruct the same exact raw hashes
for 64 KiB / 1 MiB / 8 MiB. Boundary 2 MiB - 1 / exact / + 1 all succeed with
no message cap. Malformed MIME reconstructs exactly and has the same parser
outcome as direct parsing; that establishes parity for this malformed fixture,
not validity or general malformed MIME safety. All reconstructed healthy raw
fixtures match direct parser results byte-for-byte in the attachment hashes.

For the large inner part, 11,493,147 admitted bytes versus 17,219,602 raw-message
bytes avoids roughly 5.73 MB of unrelated wire content. Serial partial fetches
add significant RTT cost: at the repository's previously measured 192 ms RTT,
178 commands imply **34.176 seconds of serial RTT alone**. This is arithmetic,
not a new network measurement. Larger chunks, batching or pipelining change
this tradeoff and require their own response/aggregate budget proof.

## Numbering, encoding, and exact-byte evidence

The parsed BODYSTRUCTURE traversal persists these paths and transfer encodings
in JSON. The targeted path selection is explicit in the harness; a production
mapping from currently saved local attachment indexes is not implemented.

| Server path | Encoding | Existing parser identity in shared fixture | Small decoded bytes | Large decoded bytes | Exact hash equality |
|---|---|---|---:|---:|---|
| `2` | SevenBit | top-level index `0`, forwarded.eml | 1,293 | 11,479,593 | yes, both |
| `2.2` | Base64 | nested attached-message index `0/0`, inner.bin | 620 | 8,388,608 | yes, both |
| `3` | Base64 | top-level index `1`, outer.bin | 4,096 | 4,194,304 | yes, both |

The distinction matters: existing `convert::attachment_bytes` enumerates the
top-level `attachments()` only. It exposes forwarded.eml as index 0, not inner.bin
as another top-level attachment. The recursive `0/0` notation is an **oracle
notation**, not a current Wind attachment ID. The same top-level semantics must
be retained if parts are used as a transport optimization. Root attachment filters
also exclude CID images and unnamed calendars; this corpus has neither, so its
oracle has the same kept attachments. Inline/calendar filtering, unusual
dispositions, missing filenames, charset conversion, quoted-printable state,
binary CTE, encoded message/rfc822, and server-vs-local MIME parse disagreement
remain unproven. Do not infer universal part-index equivalence from six successes.

## Adversarial responses and recovery

| Peer behavior | Outcome | Max response admitted | Max literal returned to caller | Poison / NOOP reuse |
|---|---|---:|---:|---|
| 8 MiB literal despite 64 KiB partial request | response quota error | 2,105,344 | 0 | poisoned / rejected |
| 70,000-byte literal despite 65,536 request | explicit partial-size refusal | 70,065 | 70,000 | poisoned / rejected |
| 8 MiB line without LF | response quota error | 2,105,344 | 0 | poisoned / rejected |
| three 1 MiB bodies in one response | response quota error | 2,105,344 | 0 | poisoned / rejected |
| repeated first 64 KiB with response origin zero | **false success, wrong raw hash** | 65,602 | 65,536 | not poisoned; exposed defect |

Quota failures have client peak working set about 11.15 MB: a 2.1 MB admitted-byte
cap is not a 2.1 MB total-memory promise. On every quota/oversized failure the
partially consumed connection is permanently poisoned; a fresh connection then
downloads an unrelated 64 KiB message successfully. This proves isolation and
fresh-connection progress in the fixture; it does not implement mailbox fairness,
persisted retry state or production connection replacement.

The wrong-offset case is an intentional failing design result recorded as
`status=ok`, `raw_equality=false` and `parser_equality=true`. The parser
summary (attachment hashes and part count) alone cannot detect body corruption;
the full raw hash does.
The harness asserts that corruption is reproduced. A successful test command
therefore means the **negative finding was reproduced**, not that the partial
adapter passed an integrity gate.

## Remaining costs / boundaries

1. Preserve and verify returned origin, UID and section before accepting bytes.
   Current public getters cannot provide all of that evidence. Duplicate/missing
   responses and premature EOF need explicit typed handling rather than this
   throwaway harness's assertions. BODYSTRUCTURE cannot be assumed truthful.
2. Production part identity must bind mailbox UIDVALIDITY + UID + server path +
   transfer encoding to the current UI's attachment index. Structure and chunk
   parsing require depth/count/CPU bounds as well as admitted bytes.
3. Part-to-file API differs from current `fetch_attachment -> Vec<u8>` and
   full-message draft/body parsing. Returning the final file as a Vec would erase
   the demonstrated memory benefit. Body HTML, inline resources, and draft import
   still need a bounded MIME strategy and semantic tests.
4. Endless valid-sized chunks remain an unbounded operation and disk write;
   request budgets, absolute deadlines, cancellation, disk reserve/re-probe,
   partial-file cleanup, and continuation scheduling are separate obligations.
5. Stream read-ahead and unsolicited bytes around command boundaries need an
   integration proof in the actual TLS/session wrapper. Here all peer responses
   are sequential and quota arming uses a shared counter before each command.

## Reproduce

From PowerShell (synthetic-only):

```powershell
Set-Location 'C:\Users\smonc\OneDrive\Documents\Repositories\wind\target\lot3-spikes\parts'
```

```powershell
cargo build --offline --release --manifest-path spikes/partial-mime/Cargo.toml --target-dir spikes/partial-mime/target
```

```powershell
python spikes/partial-mime/run.py
```

The runner regenerates corpus and result files only in this spike. It asserts
healthy raw/parser equality, six selected-part equalities, quota bounds,
poison/reuse behavior, and the known wrong-offset corruption. No full gate ran.
Source evidence: `crates/mail-imap/src/lib.rs:976` explains the current complete
MIME attachment strategy; `convert.rs:631` fixes current index extraction;
registry `imap-3.0.0-alpha.15/src/types/fetch.rs:94,146,192` demonstrates the hidden
origin; `client.rs:1535..1641` demonstrates full tagged-response accumulation.

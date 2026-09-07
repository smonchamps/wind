# Measured raw MIME ceiling candidates: 8 / 16 / 32 MiB

Extension requested by parent on 2026-09-06. Measurements are in the same isolated throwaway worktree; production source is unchanged. Initial 2 MiB observations remain archived in `results.json`/`REPORT.md`. This extension parameterizes the quota and does not designate any ceiling as product policy.

## Concrete comparison

See `candidate-table.md` for nine parser-only versus conversion comparisons and `candidate-results.json` for all **51 fresh-process runs**. Exact fixture identities are in `corpus/candidate-manifest.json`.

| Candidate raw ceiling | Response quota | Plain text parser-only peak working set | Plain text plus conversion/sanitization peak working set | Largest accepted raw message | Refused access begins |
|---|---:|---:|---:|---:|---:|
| 8 MiB | 8,396,800 B | 14,745,600 B | 57,004,032 B | 8,388,608 B | 8,388,609 B |
| 16 MiB | 16,785,408 B | 23,117,824 B | 107,315,200 B | 16,777,216 B | 16,777,217 B |
| 32 MiB | 33,562,624 B | 40,017,920 B | 208,003,072 B | 33,554,432 B | 33,554,433 B |

These are **one-process measurements**, not guarantees or whole-app measurements. The 32 MiB candidate reached 208,039,936 bytes working set on simple HTML conversion (about 198.40 MiB) and 202,792,960 bytes peak paged memory. This synthetic single-message overlap already approaches or exceeds a nominal 200 MB metric depending on units/metric, before app baseline and concurrent workers; it cannot substantiate compliance with the app budget. The 8 and 16 MiB figures also do not prove app compliance.

Ceiling tradeoff is concrete: 8 MiB refuses more messages but showed lower conversion peaks; 16 MiB accepts raw messages through 16,777,216 bytes at roughly twice the large-text conversion peak; 32 MiB admits larger mail at substantially higher conversion cost. No winner is selected here. A chosen ceiling controls total MIME, including MIME headers and base64 expansion, **not decoded attachment size**. Roughly 33% base64 expansion plus line breaks/headers means a raw ceiling cannot also promise a decoded attachment of that same size.

## Boundary and abuse results

For **each** candidate, the harness generated raw cap−1, cap and cap+1 sizes under honest, absent and lying-small (1,024-byte) RFC822.SIZE responses, plus a **64 MiB** raw literal under all three metadata variants. That is 36 boundary/abuse processes, plus 15 additional content/conversion processes (plain parser-only shares its boundary row).

- All cap−1/cap cases completed, regardless of metadata variant.
- Honest cap+1 and honest 64 MiB were refused before issuing BODY.PEEK[].
- Absent/lying cap+1 completed the framed response inside the 8 KiB protocol allowance, then were refused by the semantic raw cap, before MIME decode. The synchronized connection remained usable.
- Absent/lying 64 MiB hit exactly **cap + 8,192 bytes** admitted, returned no bodies, permanently poisoned that session, and rejected the NOOP probe.
- All 51 runs completed an unrelated 64 KiB message in a replacement session.

Normal run costs remain one size FETCH and one full BODY.PEEK[] FETCH (3 commands including the synthetic LOGIN). Honest refusal costs only the size FETCH (2 including LOGIN). The continuation check adds 3 synthetic commands; real TLS reconnect overhead remains unmeasured.

## Raw, decoded and sanitized memory

Content corpus at each exact raw ceiling:

1. Plain UTF-8 text, one long body line.
2. HTML containing one large text node inside a paragraph.
3. Multipart/mixed: a large HTML text body and base64 application/octet-stream attachment. The decoded attachment is exactly half the raw ceiling: **4, 8 or 16 MiB**. Every decoded byte is checked against the independent known sequence `byte[i] = i % 256`, in parser-only and conversion runs. This avoids allocating another large reference array in the measured process.

The release executable uses the pinned production `imap 3.0.0-alpha.15` and `mail-parser 0.11.5`. It imports the unmodified local **mail-render::sanitize** from this worktree (ammonia 4.1.4) for current sanitizer behavior. This is not a fake sanitizer or an estimated string multiplier.

`parse` mode performs actual full-MIME parse/base64 decode. `convert` mode additionally materializes `body_html(0).into_owned()` and `body_text(0).into_owned()`, copies decoded attachments using `contents().to_vec()`, and calls `mail_render::sanitize(&html)`. These mirror primitives used by current body/draft conversion and rendering. To make the measured overlap explicit, raw FETCH storage, parsed MIME, owned text/HTML, owned attachment copies and sanitized output are held simultaneously during conversion.

This is a **conservative explicit overlap model**, not an execution of the whole production mail-imap adapter: private `convert::draft_from_raw`, CID image inlining, cache/SQLite, RPC serialization, browser DOM, renderer images and multiple workers are not invoked. Consequently the result is neither the exact cost of a specific production UI path nor a worst-case upper bound. There are no inline base64 images, huge numbers of MIME parts, dense HTML nodes or adversarial CSS in these fixtures. Those can create different amplification.

Machine-readable `decoded_attachment_bytes`, `copied_attachment_bytes`, `owned_text_bytes`, `owned_html_bytes`, `sanitized_html_bytes`, `parse_decode_ms` and `conversion_ms` separate represented data from measured total process memory. For example, the 32 MiB mixed fixture produces a 16 MiB decoded attachment plus a 16 MiB owned copy and about 10.11 MiB in each text/HTML form; its conversion peak working set was 126,914,560 bytes. Giant plain/HTML text peaked higher despite having no attachment.

## Measurement limits

Same generated file-backed plaintext protocol, independent Windows PowerShell process metrics and final 400 ms hold as the original report. **Exact peak private working set remains unavailable**. OS peak working set includes shared resident pages; peak paged memory is a different commit-related metric. Sampled private commit may miss short peaks and is not substituted for exact private resident peak. The measured process does not allocate the generated 64 MiB server fixture as a buffer; it streams it from disk.

All candidate runs use an **optimized release build**; initial 2 MiB results used debug, so compare within the candidate table. One run per cell, local file I/O and shared machine load; some first-access timings were noisy (8 MiB HTML parser-only took 716.4 ms while its separately launched converted run took 99.5 ms). Phase measurements identify decode/conversion cost, but these are not latency percentiles or network benchmarks. The ordinary synthetic command deadline is 60 s in the extended harness so candidate measurements do not accidentally turn the previous 5 s test allowance into policy. The dedicated 20 ms deadline case remains available.

## Exact reproduction

```powershell
Set-Location C:\Users\smonc\OneDrive\Documents\Repositories\wind\target\lot3-spikes\raw-cap
python spikes/raw-cap/corpus.py
python spikes/raw-cap/candidate-corpus.py
cargo build --release --offline --manifest-path spikes/raw-cap/Cargo.toml --target-dir spikes/raw-cap/target
./spikes/raw-cap/measure-candidates.ps1
python spikes/raw-cap/report-candidates.py
```

One candidate, explicit 16 MiB raw cap and conversion:

```powershell
./spikes/raw-cap/target/release/lot3-raw-cap-spike.exe honest-mixed bounded C:/Users/smonc/OneDrive/Documents/Repositories/wind/target/lot3-spikes/raw-cap/spikes/raw-cap/corpus 16777216 convert
```

The new CLI's optional fourth positional argument is raw cap bytes (default retains 2 MiB for old commands), and fifth is `parse` or `convert`. `measure-candidates.ps1` uses fresh executables with hidden windows and writes per-run JSON/stdout plus aggregate JSON. `report-candidates.py` checks all boundary/refusal/quota/progress/attachment assertions and produces the comparison table. No gate, production implementation, commit or CE decision was performed.

// Main-thread gate (PLAN-GELS E1, decision D1): in Tauri 2, a command
// declared WITHOUT `async` runs on the main thread — the one driving
// the Windows message pump. Any synchronous command that opens the
// database, touches a file, or hits the keyring therefore freezes the
// window for its whole duration (finding of 2026-08-15: freezes of 2
// to 4.6 s at startup, 25.2 s cumulated over 40 s — the window "not
// responding"). And `async` alone is not enough: the blocking body
// must go through `off_pump` (spawn_blocking + command lock), or it
// pins a tokio worker — the freeze just moves from the window to the
// IPC queue.
//
//   node main-thread-guard.mjs   -> offending commands + verdict
//
// The rule is INVERTED on purpose: every `#[tauri::command]` command
// is `async fn`, EXCEPT the pure state commands named below. A list of
// blocking markers would not hold: it would miss the command that
// blocks through a helper (`queue_removal` opens the database for
// `archive_message`). The exemption, by contrast, is visible and must
// be justified.
//
// Two guards on the instrument itself (the "ink2" bug, paid twice —
// contrast.mjs:24, system-coherence.mjs:41): the regex accepts
// parameterized attributes, `pub(crate)`, and digits; and every
// textual occurrence of `#[tauri::command` must match a catch — zero
// catches or a count mismatch is a FAILURE, never a silent green.
//
// The fix is always the same: turn the command into `async fn` with
// its body through `off_pump` — never extend the exemption without the
// same proof of purity.
import { readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';

const root = path.resolve(import.meta.dirname, '..');
const sources = path.join(root, 'apps', 'desktop', 'src');

// The exempted commands, each with its reason for being synchronous:
// - sync_activity: VERY short locks (three text Mutexes written once
//   per account/mailbox per cycle) — not atomics, but windows of a few
//   microseconds; the probe's budget (`freeze-probe.py`, 150 ms) would
//   catch any drift;
// - migration_progress: reads shared atomics;
// - migration_cancel: writes an atomic;
// - network_state: an atomic, plus ONE short `sync_reculs.lock()`
//   (clearing a small map when the network comes back) — the same
//   safeguard as sync_activity: the probe measures, the budget
//   decides;
// - app_version: reads the in-memory manifest;
// - open_link: DETACHED ShellExecuteW (open::that_detached) — true
//   ONLY with the `shellexecute-on-windows` feature of the `open`
//   crate (apps/desktop/Cargo.toml); without it, synchronous
//   powershell.exe on the pump (2026-09-01 audit);
// - telemetry_selftest_panic: does not block, it PANICS — and ADR 0014
//   validated the double-panic on the MAIN THREAD (WebView2 FFI
//   boundary): moving it would change what the self-test exercises.
const PURE_COMMANDS = new Set([
  'sync_activity',
  'migration_progress',
  'migration_cancel',
  'network_state',
  'app_version',
  'open_link',
  'telemetry_selftest_panic',
]);

// What BLOCKS for sure: the database (each command opens its own
// connection), files, the OS vault. If an exempted command comes near
// one, it loses the exemption. (Best-effort detection — indirect help
// escapes it, which is why the exemption is a LIST, not a heuristic.)
const MARKERS = ['Store::', 'db_path(', 'std::fs', 'File::', 'keyring', 'read_to_string'];

// PLAN-AUDIT-V1 E5 (2026-09-01 audit S1-2): the guard used to stop at
// the `async` keyword — seventeen commands opened the database, read
// the vault, or wrote a file INSIDE the async body, outside
// `off_pump`: the block moved from the pump to a tokio worker
// (workers = cores) and escaped the command lock (ADR 0019).
// Rule: the body of an async command, once the `off_pump(...)` and
// `spawn_blocking(...)` calls are STRIPPED (balanced parentheses), is
// nothing but glue — none of these markers belong there.
// `db_path(` is NOT in it: since E5 it is a pure read (OnceLock, the
// folder is created on first call) — it stays in MARKERS for the
// exempted commands, which must not even name the database.
// `lock_accounts` and `veilleur::reconcilier` are memory locks of a
// few microseconds, not I/O: the probe (`freeze-probe.py`) decides.
const GLUE_MARKERS = [
  ...MARKERS.filter((m) => m !== 'db_path('),
  'auth_for(',
  'connected_jobs(',
  'account_email(',
  'mail_render::sanitize',
  'connect_imap(',
  'trace_maj(',
];
const OFF_PUMP = ['off_pump(', 'spawn_blocking('];

// Strips every `name(...)` call from the text, with balanced
// parentheses — what remains is the glue the command runs itself, on
// the async worker.
function withoutCalls(text, names) {
  let remaining = text;
  for (const name of names) {
    let departure = remaining.indexOf(name);
    while (departure !== -1) {
      const opening = departure + name.length - 1;
      let depth = 0;
      let end = opening;
      for (; end < remaining.length; end += 1) {
        if (remaining[end] === '(') depth += 1;
        else if (remaining[end] === ')') {
          depth -= 1;
          if (depth === 0) break;
        }
      }
      remaining = remaining.slice(0, departure) + remaining.slice(end + 1);
      departure = remaining.indexOf(name);
    }
  }
  return remaining;
}

let failures = 0;
const failure = (message) => {
  failures += 1;
  console.log(`FAILURE ${message}`);
};

// Extracts a function's body by brace balancing, starting from the
// offset of its first `{`. Assumed heuristic: an unmatched brace
// inside a string would throw off the bound — the exempted bodies are
// short and re-read at every addition to PURE_COMMANDS.
function body(text, start) {
  let depth = 0;
  for (let i = start; i < text.length; i += 1) {
    if (text[i] === '{') depth += 1;
    else if (text[i] === '}') {
      depth -= 1;
      if (depth === 0) return text.slice(start, i + 1);
    }
  }
  return text.slice(start);
}

let attributes = 0;
let catches = 0;
for (const file of readdirSync(sources).filter((f) => f.endsWith('.rs'))) {
  const text = readFileSync(path.join(sources, file), 'utf8');
  attributes += (text.match(/#\[tauri::command/g) ?? []).length;
  const commands = text.matchAll(
    /#\[tauri::command[^\]]*\]\s*(?:#\[[^\]]*\]\s*|\/\/[^\n]*\n\s*)*pub(?:\([^)]*\))?\s+(async\s+)?fn\s+([A-Za-z0-9_]+)/g,
  );
  for (const catch_ of commands) {
    catches += 1;
    const [, isAsync, name] = catch_;
    if (isAsync) {
      if (PURE_COMMANDS.has(name)) {
        failure(
          `${file}: \`${name}\` is async but is in the pure-command exemption — remove one or the other`,
        );
        continue;
      }
      // E5: async is not enough — the blocking part must go through off_pump.
      const asyncOpening = text.indexOf('{', catch_.index + catch_[0].length);
      if (asyncOpening === -1) continue;
      const glue = withoutCalls(body(text, asyncOpening), OFF_PUMP);
      const inTheGlue = GLUE_MARKERS.filter((m) => glue.includes(m));
      if (inTheGlue.length > 0) {
        failure(
          `${file}: async command \`${name}\` touches ${inTheGlue.join(', ')} OUTSIDE off_pump/spawn_blocking — blocks a tokio worker without the command lock (ADR 0019)`,
        );
      }
      continue;
    }
    if (!PURE_COMMANDS.has(name)) {
      failure(
        `${file}: synchronous command \`${name}\` runs on the main thread — turn it into \`async fn\` + \`off_pump\` (or prove its purity and exempt it)`,
      );
      continue;
    }
    const opening = text.indexOf('{', catch_.index + catch_[0].length);
    if (opening === -1) continue;
    const inner = body(text, opening);
    const found = MARKERS.filter((m) => inner.includes(m));
    if (found.length > 0) {
      failure(
        `${file}: \`${name}\` is exempted as pure but touches ${found.join(', ')} — turn it into \`async fn\` + \`off_pump\``,
      );
    }
  }
}

// The instrument checks itself like everything else (PASSATION §9):
// every attribute must have its catch, and zero commands means the
// gate no longer checks anything (folder moved, shape changed).
if (catches === 0) {
  failure('no command found — the gate no longer checks anything (folder moved? shape changed?)');
} else if (catches !== attributes) {
  failure(
    `${attributes} #[tauri::command] attributes but ${catches} catches — the regex is missing commands`,
  );
}

if (failures > 0) {
  console.log(`\n${failures} defect(s) on the main thread.`);
  process.exitCode = 1;
} else {
  console.log(
    `OK: ${catches} commands checked, none blocking on the main thread.`,
  );
}

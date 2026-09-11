// Tests for the kaizen's per-increment attribution (PLAN-KAIZEN-CLAUDE,
// decisions D5, D6 and D9 of 2026-09-09).
//
// D5: T1 and W3 are counted per increment (E-step), each anchored on its
// closing commit — interval(k) = ( t(c(k-1)) , t(c(k)) ]. D6: turns per
// prompt is computed over Chief-Engineer-driven sessions only. D9: the
// agent share of an increment is reported, never capped.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import {
  bucketByCommit,
  agentShare,
  turnsPerPrompt,
  isGateRun,
  readSessions,
  equiv,
} from '../scripts/measure-sessions.mjs';

const at = (iso) => new Date(iso);
const turn = (iso, equiv, extra = {}) => ({ ts: at(iso), equiv, ...extra });

// E0 is not a special case: it is simply the first commit passed in — by
// convention the plan's own commit, which closes the design phase.
const COMMITS = [
  { sha: 'aaa1111', ts: at('2026-09-01T10:00:00Z'), label: 'E0' },
  { sha: 'bbb2222', ts: at('2026-09-02T10:00:00Z'), label: 'E1' },
  { sha: 'ccc3333', ts: at('2026-09-03T10:00:00Z'), label: 'E2' },
];
const FROM = at('2026-09-01T08:00:00Z');

test('a turn between two commits is billed to the later one', () => {
  const { buckets } = bucketByCommit(
    [turn('2026-09-02T09:00:00Z', 5)],
    COMMITS,
    { from: FROM },
  );
  assert.equal(buckets[0].equiv, 0, 'E0 must not absorb it');
  assert.equal(buckets[1].equiv, 5, 'E1 owns the interval that ends at its commit');
  assert.equal(buckets[2].equiv, 0);
});

test('the upper bound is inclusive, the lower bound is not', () => {
  const { buckets } = bucketByCommit(
    [turn('2026-09-02T10:00:00Z', 7)],
    COMMITS,
    { from: FROM },
  );
  assert.equal(buckets[1].equiv, 7, 'a turn at t(c) belongs to c');
  assert.equal(buckets[2].equiv, 0, 'and never to the next increment');
});

test('work after the last commit lands in an open bucket, not in the last increment', () => {
  const { buckets, open } = bucketByCommit(
    [turn('2026-09-04T09:00:00Z', 11)],
    COMMITS,
    { from: FROM },
  );
  assert.equal(buckets[2].equiv, 0, 'E2 is closed by its commit');
  assert.equal(open.equiv, 11);
});

test('a turn before the job opened is outside the job entirely', () => {
  const { buckets, open } = bucketByCommit(
    [turn('2026-08-31T23:00:00Z', 99)],
    COMMITS,
    { from: FROM },
  );
  const billed = buckets.reduce((a, b) => a + b.equiv, 0) + open.equiv;
  assert.equal(billed, 0);
});

test('agents are separated from the main thread, and the share is reported (D9)', () => {
  const { buckets } = bucketByCommit(
    [
      turn('2026-09-02T09:00:00Z', 30),
      turn('2026-09-02T09:30:00Z', 70, { agent: true }),
    ],
    COMMITS,
    { from: FROM },
  );
  assert.equal(buckets[1].equiv, 30, 'main thread');
  assert.equal(buckets[1].agentEquiv, 70, 'subagents');
  assert.equal(agentShare(buckets[1]), 0.7);
});

test('an increment with no cost at all has a share of zero, not NaN', () => {
  const { buckets } = bucketByCommit([], COMMITS, { from: FROM });
  assert.equal(agentShare(buckets[0]), 0);
});

test('gate runs are counted per increment and cost nothing themselves (W3)', () => {
  const { buckets } = bucketByCommit(
    [
      turn('2026-09-02T08:00:00Z', 0, { gate: true }),
      turn('2026-09-02T09:00:00Z', 0, { gate: true }),
      turn('2026-09-03T09:00:00Z', 0, { gate: true }),
    ],
    COMMITS,
    { from: FROM },
  );
  assert.equal(buckets[1].gates, 2);
  assert.equal(buckets[2].gates, 1);
  assert.equal(buckets[1].equiv, 0, 'a gate marker carries no tokens of its own');
});

test('commits given out of order are still bucketed chronologically', () => {
  const shuffled = [COMMITS[2], COMMITS[0], COMMITS[1]];
  const { buckets } = bucketByCommit(
    [turn('2026-09-02T09:00:00Z', 4)],
    shuffled,
    { from: FROM },
  );
  assert.deepEqual(buckets.map((b) => b.label), ['E0', 'E1', 'E2']);
  assert.equal(buckets[1].equiv, 4);
});

// --- D6 -------------------------------------------------------------------

test('turns per prompt excludes a session that carries no prompt (D6)', () => {
  const sessions = [
    { prompts: 0, turns: 400 }, // autonomous: its turns belong to no prompt
    { prompts: 10, turns: 200 },
  ];
  const r = turnsPerPrompt(sessions);
  assert.equal(r.ratio, 20, 'not 60 — the autonomous turns are not averaged in');
  assert.equal(r.excluded, 1);
  assert.equal(r.driven, 1);
});

test('turns per prompt is undefined rather than infinite when nothing was driven', () => {
  const r = turnsPerPrompt([{ prompts: 0, turns: 12 }]);
  assert.equal(r.ratio, null);
  assert.equal(r.excluded, 1);
});

// --- W3: what counts as a gate run ---------------------------------------
// Found by running the report on a real range (2026-09-10): a bucket
// claimed 2 gates where 1 had run. Reading the script — `grep ...
// scripts/gate.ps1` — was counted as playing it.

test('only an invocation of the gate counts, never a mention of it', () => {
  assert.equal(isGateRun('powershell -ExecutionPolicy Bypass -File scripts/gate.ps1'), true);
  assert.equal(isGateRun('pwsh -File C:/wind/scripts/gate.ps1'), true);
  assert.equal(isGateRun('grep -n "language" scripts/gate.ps1'), false, 'reading it is not running it');
  assert.equal(isGateRun('cat scripts/gate.ps1 | head -20'), false);
  assert.equal(isGateRun('git diff scripts/gate.ps1'), false);
  assert.equal(isGateRun(''), false);
});

test('the gate must be the head of a command, not text quoted inside one', () => {
  // The heredoc that wrote this very file embedded the invocation as a
  // string, and was billed as a gate run (measured 2026-09-10).
  const heredoc = [
    'cat >> t.test.mjs <<EOF',
    "  assert.equal(isGateRun('powershell -ExecutionPolicy Bypass -File scripts/gate.ps1'), true);",
    'EOF',
  ].join('\n');
  assert.equal(isGateRun(heredoc), false, 'writing about the gate is not playing it');
  assert.equal(
    isGateRun('cd "C:/wind" && powershell -ExecutionPolicy Bypass -File scripts/gate.ps1 2>&1'),
    true,
    'the real invocation, after a cd',
  );
  assert.equal(isGateRun('powershell -File scripts/gate.ps1 | tail -5'), true, 'piped, still a run');
});

// A conversation too long to continue is forked: the child transcript
// REPLAYS the parent's history, same uuids, same timestamps. The script
// read both files and billed the shared stretch twice — 454 turns and
// 21.4 M input equiv. over the window of 2026-09-11, and 11 full gates
// where 7 had run (field finding, D13). Same entry, counted once.
test('a forked session replaying its parent is counted once, not twice', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'wind-measure-'));
  const usage = { input_tokens: 100, cache_read_input_tokens: 0, cache_creation_input_tokens: 0, output_tokens: 20 };
  const turnLine = (uuid, iso) => JSON.stringify({
    uuid, timestamp: iso, type: 'assistant',
    message: { model: 'claude-fable-5', usage },
  });
  const shared = [
    turnLine('u-1', '2026-09-09T10:00:00.000Z'),
    turnLine('u-2', '2026-09-09T10:01:00.000Z'),
    turnLine('u-3', '2026-09-09T10:02:00.000Z'),
  ];
  writeFileSync(join(dir, 'parent.jsonl'), [...shared, turnLine('u-4', '2026-09-09T10:03:00.000Z')].join('\n'));
  // The fork: the same three entries verbatim, then its own work.
  writeFileSync(join(dir, 'fork.jsonl'), [...shared, turnLine('u-5', '2026-09-09T11:00:00.000Z')].join('\n'));

  const sessions = await readSessions(dir);
  const turns = sessions.reduce((a, s) => a + s.turns, 0);
  const total = sessions.reduce((a, s) => a + equiv({
    input_tokens: s.input, cache_read_input_tokens: s.cacheRead,
    cache_creation_input_tokens: s.cacheCreate, output_tokens: s.output,
  }), 0);
  assert.equal(turns, 5, 'three shared entries plus one own entry each');
  assert.equal(total, equiv(usage) * 5, 'the shared stretch is billed once');
  rmSync(dir, { recursive: true, force: true });
});

test('the earlier file claims the shared entries, so the reading is reproducible', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'wind-measure-'));
  const line = (uuid, iso) => JSON.stringify({
    uuid, timestamp: iso, type: 'assistant',
    message: { model: 'claude-fable-5', usage: { input_tokens: 10, output_tokens: 0 } },
  });
  // 'b-parent' sorts after 'a-fork' by name; the first timestamp must decide.
  writeFileSync(join(dir, 'b-parent.jsonl'), [line('s-1', '2026-09-09T08:00:00.000Z'), line('s-2', '2026-09-09T08:01:00.000Z')].join('\n'));
  writeFileSync(join(dir, 'a-fork.jsonl'), [line('s-2', '2026-09-09T08:01:00.000Z'), line('s-3', '2026-09-09T09:00:00.000Z')].join('\n'));

  const sessions = await readSessions(dir);
  const parent = sessions.find(s => s.id === 'b-parent');
  const fork = sessions.find(s => s.id === 'a-fork');
  assert.equal(parent.turns, 2, 'the file that opened first keeps the shared entry');
  assert.equal(fork.turns, 1, 'the fork carries only what is its own');
  rmSync(dir, { recursive: true, force: true });
});

test('entries with no uuid are never confused with one another', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'wind-measure-'));
  const bare = (iso) => JSON.stringify({
    timestamp: iso, type: 'assistant',
    message: { model: 'claude-fable-5', usage: { input_tokens: 10, output_tokens: 0 } },
  });
  writeFileSync(join(dir, 'one.jsonl'), [bare('2026-09-09T08:00:00.000Z'), bare('2026-09-09T08:01:00.000Z')].join('\n'));
  const sessions = await readSessions(dir);
  assert.equal(sessions[0].turns, 2, 'dedup keys on the uuid; without one, nothing is dropped');
  rmSync(dir, { recursive: true, force: true });
});

test('a gate replayed into the fork is one gate, not two', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'wind-measure-'));
  const gate = JSON.stringify({
    uuid: 'g-1', timestamp: '2026-09-09T10:00:00.000Z', type: 'assistant',
    message: {
      model: 'claude-fable-5', usage: { input_tokens: 1, output_tokens: 0 },
      content: [{ type: 'tool_use', id: 't1', name: 'Bash', input: { command: 'powershell -ExecutionPolicy Bypass -File scripts/gate.ps1' } }],
    },
  });
  writeFileSync(join(dir, 'parent.jsonl'), gate);
  writeFileSync(join(dir, 'fork.jsonl'), gate);
  const sessions = await readSessions(dir);
  const gates = sessions.flatMap(s => s.events).filter(e => e.gate).length;
  assert.equal(gates, 1, 'W3 counts the run, not the transcripts that recorded it');
  rmSync(dir, { recursive: true, force: true });
});

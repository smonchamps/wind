// Tests for the kaizen's per-increment attribution (PLAN-KAIZEN-CLAUDE,
// decisions D5, D6 and D9 of 2026-09-09).
//
// D5: T1 and W3 are counted per increment (E-step), each anchored on its
// closing commit — interval(k) = ( t(c(k-1)) , t(c(k)) ]. D6: turns per
// prompt is computed over Chief-Engineer-driven sessions only. D9: the
// agent share of an increment is reported, never capped.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  bucketByCommit,
  agentShare,
  turnsPerPrompt,
  isGateRun,
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
